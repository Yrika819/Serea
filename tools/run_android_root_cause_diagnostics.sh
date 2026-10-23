#!/usr/bin/env bash
set -uo pipefail

api_level="${1:-}"
if [[ "$api_level" != 26 && "$api_level" != 35 ]]; then
  echo "Usage: $0 {26|35}" >&2
  exit 2
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
baseline_sha='b1e7a7359045d716e9f903d85c1291dff8453520'
baseline_repo="$RUNNER_TEMP/serea-pr100-baseline"
evidence_root="$GITHUB_WORKSPACE/root-cause/android/api$api_level"
mkdir -p "$evidence_root"
failures=0

EXPECTED_API_LEVEL="$api_level"
SEREA_EXPECTED_ANDROID_API="$api_level"
export EXPECTED_API_LEVEL SEREA_EXPECTED_ANDROID_API
source "$repo_root/tools/assert_disposable_android_emulator.sh" || exit 1

if ! git -C "$repo_root" cat-file -e "$baseline_sha^{commit}"; then
  git -C "$repo_root" fetch --no-tags origin "$baseline_sha" || exit 1
fi
git -C "$repo_root" worktree add --detach "$baseline_repo" "$baseline_sha" || exit 1
trap 'git -C "$repo_root" worktree remove --force "$baseline_repo" >/dev/null 2>&1 || true' EXIT

run_stage() {
  local source_label="$1" source_repo="$2" stage="$3" test_filter="$4"
  local output_dir="$evidence_root/$stage" token package='app.serea.android.isolated'
  local target_apk="$source_repo/android/app/build/outputs/apk/isolated/app-isolated.apk"
  local gradle_status
  mkdir -p "$output_dir"
  printf 'NOT_RUN\n' > "$output_dir/status.txt"
  printf 'source=%s\nsource_sha=%s\napi=%s\nstage=%s\ntest_filter=%s\nstarted_utc=%s\n' \
    "$source_label" "$(git -C "$source_repo" rev-parse HEAD)" "$api_level" "$stage" \
    "$test_filter" "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" > "$output_dir/manifest.txt"

  rm -rf "$source_repo/android/app/build/outputs/androidTest-results/connected"
  (
    cd "$source_repo/android" || exit 1
    ./gradlew --no-daemon :app:assembleIsolated
  ) > "$output_dir/assemble.log" 2>&1
  local assemble_status=$?
  printf '%s\n' "$assemble_status" > "$output_dir/assemble-exit-code.txt"
  if [[ "$assemble_status" -ne 0 || ! -s "$target_apk" ]]; then
    printf 'ASSEMBLE_FAIL\n' > "$output_dir/status.txt"
    failures=1
    return
  fi

  adb logcat -c >/dev/null 2>&1 || true
  adb uninstall app.serea.android.isolated.test >/dev/null 2>&1 || true
  adb uninstall "$package" >/dev/null 2>&1 || true
  if ! adb install "$target_apk" > "$output_dir/install.log" 2>&1; then
    printf 'INSTALL_FAIL\n' > "$output_dir/status.txt"
    failures=1
    return
  fi
  token="serea-root-cause-api${api_level}-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}-${stage}"
  EXPECTED_API_LEVEL="$api_level" bash "$source_repo/tools/prepare_isolated_instrumentation_target.sh" "$token" \
    > "$output_dir/seed.log" 2>&1
  local seed_status=$?
  printf '%s\n' "$seed_status" > "$output_dir/seed-exit-code.txt"
  if [[ "$seed_status" -ne 0 ]]; then
    printf 'SEED_FAIL\n' > "$output_dir/status.txt"
    failures=1
    return
  fi

  local args=(--no-daemon
    "-Pandroid.testInstrumentationRunnerArguments.sereaCampaignToken=$token"
    -Pandroid.testInstrumentationRunnerArguments.listener=app.serea.android.SereaInstrumentationLifecycleListener)
  local observer_pid=''
  if [[ -n "$test_filter" ]]; then
    args+=("-Pandroid.testInstrumentationRunnerArguments.class=$test_filter")
  fi
  if [[ "$api_level" == 35 && "$source_label" == instrumented ]]; then
    args+=(-Pandroid.testInstrumentationRunnerArguments.rootCauseHostInput=true)
    python3 "$repo_root/tools/android_host_tap_observer.py" "$output_dir" \
      > "$output_dir/tap-observer.log" 2>&1 &
    observer_pid=$!
    sleep 1
  fi
  (
    cd "$source_repo/android" || exit 1
    ./gradlew "${args[@]}" connectedIsolatedAndroidTest
  ) > "$output_dir/gradle.log" 2>&1
  gradle_status=$?
  if [[ -n "$observer_pid" ]]; then
    kill "$observer_pid" >/dev/null 2>&1 || true
    wait "$observer_pid" >/dev/null 2>&1 || true
  fi
  printf '%s\n' "$gradle_status" > "$output_dir/gradle-exit-code.txt"
  printf 'finished_utc=%s\nexit_code=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" "$gradle_status" >> "$output_dir/manifest.txt"
  if [[ -d "$source_repo/android/app/build/outputs/androidTest-results/connected" ]]; then
    cp -a "$source_repo/android/app/build/outputs/androidTest-results/connected" "$output_dir/junit"
  fi
  adb logcat -d -v raw -s \
    SereaRootCauseRaw:I SereaRootCauseFocusOrder:I SereaChatWindowFocus:I AndroidRuntime:E \
    > "$output_dir/logcat-raw.txt" 2>&1 || true
  python3 "$repo_root/tools/summarize_android_root_cause.py" \
    "$output_dir/logcat-raw.txt" "$output_dir/focus-owner-summary.txt" \
    > "$output_dir/host-extraction.log" 2>&1 || true
  if [[ "$gradle_status" -eq 0 ]]; then
    printf 'GRADLE_PASS\n' > "$output_dir/status.txt"
  else
    printf 'GRADLE_FAIL\n' > "$output_dir/status.txt"
    failures=1
  fi
  echo "stage=$stage result=$(cat "$output_dir/status.txt") source=$source_label api=$api_level exit=$gradle_status"
}

run_repeated() {
  local source_label="$1" source_repo="$2" base_stage="$3" test_filter="$4" repeats="$5"
  for repetition in $(seq 1 "$repeats"); do
    run_stage "$source_label" "$source_repo" "$base_stage-$repetition" "$test_filter"
  done
}

if [[ "$api_level" == 35 ]]; then
  send_filter='app.serea.android.ChatScreenTest#demoLabelAndConversationTitleAreVisible,app.serea.android.ChatScreenTest#completedDemoConversationSurvivesActivityRecreation,app.serea.android.ChatScreenTest#currentConversationDeletionRequiresConfirmationAndCancelKeepsMessages,app.serea.android.ChatScreenTest#confirmedCurrentConversationDeletionRemovesSelectedConversation,app.serea.android.ChatScreenTest#deleteAllConfirmationCancelKeepsConversationAndConfirmClearsHistory'
  run_repeated baseline "$baseline_repo" baseline-send-tests "$send_filter" 3
  run_stage baseline "$baseline_repo" baseline-full-suite ''
  run_repeated instrumented "$repo_root" instrumented-send-tests "$send_filter" 3
  run_stage instrumented "$repo_root" instrumented-full-suite ''
else
  target_filter='app.serea.android.SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges'
  class_filter='app.serea.android.SereaAppShellTest'
  run_repeated baseline "$baseline_repo" baseline-target "$target_filter" 3
  run_stage baseline "$baseline_repo" baseline-class "$class_filter"
  run_stage baseline "$baseline_repo" baseline-full-suite ''
  run_repeated instrumented "$repo_root" instrumented-target "$target_filter" 3
  run_stage instrumented "$repo_root" instrumented-class "$class_filter"
  run_stage instrumented "$repo_root" instrumented-full-suite ''
fi

printf 'completed_utc=%s\nfailures_observed=%s\n' \
  "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" "$failures" > "$evidence_root/campaign-summary.txt"
echo "Root-cause Android API $api_level stages complete; failures_observed=$failures"
exit "$failures"
