#!/usr/bin/env bash
set -uo pipefail

if [[ "$#" -ne 2 ]]; then
  echo "Usage: $0 {26|35} REPEAT_COUNT" >&2
  exit 2
fi
api_level="$1"
repeat_count="$2"
if [[ "$api_level" != 26 && "$api_level" != 35 ]] || [[ ! "$repeat_count" =~ ^[1-9][0-9]*$ ]]; then
  echo "Expected API 26 or 35 and a positive repeat count." >&2
  exit 2
fi

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
evidence_root="$GITHUB_WORKSPACE/root-cause/focused/api$api_level"
mkdir -p "$evidence_root"

EXPECTED_API_LEVEL="$api_level"
SEREA_EXPECTED_ANDROID_API="$api_level"
export EXPECTED_API_LEVEL SEREA_EXPECTED_ANDROID_API
source "$repo_root/tools/assert_disposable_android_emulator.sh" || exit 1

if [[ "$api_level" == 35 ]]; then
  test_filter='app.serea.android.ChatScreenTest#demoLabelAndConversationTitleAreVisible,app.serea.android.ChatScreenTest#completedDemoConversationSurvivesActivityRecreation,app.serea.android.ChatScreenTest#currentConversationDeletionRequiresConfirmationAndCancelKeepsMessages,app.serea.android.ChatScreenTest#confirmedCurrentConversationDeletionRemovesSelectedConversation,app.serea.android.ChatScreenTest#deleteAllConfirmationCancelKeepsConversationAndConfirmClearsHistory'
else
  test_filter='app.serea.android.SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges'
fi

ready_epoch_ms="$(date +%s%3N)"
launch_epoch_ms="$FOCUSED_EMULATOR_LAUNCH_START_EPOCH_MS"
startup_elapsed_ms=$((ready_epoch_ms - launch_epoch_ms))
{
  printf 'emulator_launch_started_utc=%s\n' "$FOCUSED_EMULATOR_LAUNCH_START_UTC"
  printf 'emulator_ready_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)"
  printf 'emulator_startup_elapsed_ms=%s\n' "$startup_elapsed_ms"
  printf 'api=%s\n' "$api_level"
} > "$evidence_root/emulator-startup.txt"
cat "$evidence_root/emulator-startup.txt" >> "$GITHUB_STEP_SUMMARY"

package='app.serea.android.isolated'
target_apk="$repo_root/android/app/build/outputs/apk/isolated/app-isolated.apk"
if [[ ! -s "$target_apk" ]]; then
  echo "Expected isolated APK is missing: $target_apk" >&2
  exit 1
fi

passes=0
failed_repetitions=0
failures=0
for iteration in $(seq 1 "$repeat_count"); do
  stage="focused-$iteration"
  output_dir="$evidence_root/$stage"
  mkdir -p "$output_dir"
  rm -rf "$repo_root/android/app/build/outputs/androidTest-results/connected"
  stage_started_utc="$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)"
  test_started_utc='NOT_RUN'

  install_started_ms="$(date +%s%3N)"
  adb logcat -c >/dev/null 2>&1 || true
  adb uninstall app.serea.android.isolated.test > "$output_dir/uninstall-test.log" 2>&1 || true
  adb uninstall "$package" > "$output_dir/uninstall-target.log" 2>&1 || true
  if adb install "$target_apk" > "$output_dir/install.log" 2>&1; then
    install_status=0
  else
    install_status=$?
  fi
  install_elapsed_ms=$(( $(date +%s%3N) - install_started_ms ))

  status='READY'
  seed_elapsed_ms=0
  test_elapsed_ms=0
  log_elapsed_ms=0
  seed_started_ms="$(date +%s%3N)"
  if [[ "$install_status" -ne 0 ]]; then
    status='INSTALL_FAIL'
  else
    token="serea-focused-api$api_level-$GITHUB_RUN_ID-$GITHUB_RUN_ATTEMPT-$iteration"
    EXPECTED_API_LEVEL="$api_level" bash "$repo_root/tools/prepare_isolated_instrumentation_target.sh" "$token" \
      > "$output_dir/seed.log" 2>&1
    seed_status=$?
    seed_elapsed_ms=$(( $(date +%s%3N) - seed_started_ms ))
    if [[ "$seed_status" -ne 0 ]]; then
      status='SEED_FAIL'
    else
      args=(--no-daemon
        "-Pandroid.testInstrumentationRunnerArguments.sereaCampaignToken=$token"
        -Pandroid.testInstrumentationRunnerArguments.listener=app.serea.android.SereaInstrumentationLifecycleListener
        "-Pandroid.testInstrumentationRunnerArguments.class=$test_filter")
      observer_pid=''
      if [[ "$api_level" == 35 ]]; then
        args+=(-Pandroid.testInstrumentationRunnerArguments.rootCauseHostInput=true)
        python3 "$repo_root/tools/android_host_tap_observer.py" "$output_dir" \
          > "$output_dir/tap-observer.log" 2>&1 &
        observer_pid=$!
        sleep 1
      fi
      test_started_utc="$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)"
      test_started_ms="$(date +%s%3N)"
      (
        cd "$repo_root/android" || exit 1
        ./gradlew "${args[@]}" connectedIsolatedAndroidTest
      ) > "$output_dir/gradle.log" 2>&1
      gradle_status=$?
      test_elapsed_ms=$(( $(date +%s%3N) - test_started_ms ))
      if [[ -n "$observer_pid" ]]; then
        kill "$observer_pid" >/dev/null 2>&1 || true
        wait "$observer_pid" >/dev/null 2>&1 || true
      fi
      printf '%s\n' "$gradle_status" > "$output_dir/gradle-exit-code.txt"
      if [[ "$gradle_status" -eq 0 ]]; then
        status='PASS'
      else
        status='GRADLE_FAIL'
      fi
    fi
  fi

  log_started_utc="$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)"
  log_started_ms="$(date +%s%3N)"
  adb logcat -d -v raw -s \
    SereaRootCauseRaw:I SereaRootCauseFocusOrder:I SereaChatWindowFocus:I \
    SereaProcessDeath:V SereaProfileVerifier:I AndroidRuntime:E \
    > "$output_dir/logcat-raw.txt" 2>&1
  log_status=$?
  if [[ -d "$repo_root/android/app/build/outputs/androidTest-results/connected" ]]; then
    cp -a "$repo_root/android/app/build/outputs/androidTest-results/connected" "$output_dir/junit"
  fi
  log_elapsed_ms=$(( $(date +%s%3N) - log_started_ms ))
  if [[ "$log_status" -ne 0 && "$status" == 'PASS' ]]; then
    status='LOG_CAPTURE_FAIL'
  fi
  finished_utc="$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)"

  if [[ "$status" == 'PASS' ]]; then
    passes=$((passes + 1))
  else
    failed_repetitions=$((failed_repetitions + 1))
    failures=1
  fi

  {
    printf 'api=%s\n' "$api_level"
    printf 'stage=%s\n' "$stage"
    printf 'iteration=%s\n' "$iteration"
    printf 'test_filter=%s\n' "$test_filter"
    printf 'stage_started_utc=%s\n' "$stage_started_utc"
    printf 'test_started_utc=%s\n' "$test_started_utc"
    printf 'log_capture_started_utc=%s\n' "$log_started_utc"
    printf 'finished_utc=%s\n' "$finished_utc"
    printf 'install_elapsed_ms=%s\n' "$install_elapsed_ms"
    printf 'seed_elapsed_ms=%s\n' "$seed_elapsed_ms"
    printf 'focused_test_elapsed_ms=%s\n' "$test_elapsed_ms"
    printf 'log_capture_elapsed_ms=%s\n' "$log_elapsed_ms"
    printf 'status=%s\n' "$status"
  } > "$output_dir/manifest.txt"
  printf '%s\n' "$status" > "$output_dir/status.txt"
  echo "API $api_level $stage: $status (test ${test_elapsed_ms}ms; logs ${log_elapsed_ms}ms)"
done

{
  printf 'api=%s\n' "$api_level"
  printf 'requested_repetitions=%s\n' "$repeat_count"
  printf 'passed_repetitions=%s\n' "$passes"
  printf 'failed_repetitions=%s\n' "$failed_repetitions"
  printf 'completed_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)"
} > "$evidence_root/campaign-summary.txt"

exit "$failures"
