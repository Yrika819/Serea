#!/usr/bin/env bash
set -uo pipefail

api_level="${1:-}"
if [[ "$api_level" != 26 && "$api_level" != 35 ]]; then
  echo "Usage: $0 {26|35}" >&2
  exit 2
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_sha='b1e7a7359045d716e9f903d85c1291dff8453520'
source_repo="$GITHUB_WORKSPACE/serea-pr100"
evidence_root="$GITHUB_WORKSPACE/root-cause/suite-state/api$api_level"
package='app.serea.android.isolated'
test_package='app.serea.android.isolated.test'
target_apk="$source_repo/android/app/build/outputs/apk/isolated/app-isolated.apk"
failures=0

capture_system_health() {
  local label="$1"
  local destination="$evidence_root/system-health-$label"
  mkdir -p "$destination"
  # Preserve each raw command output so host-side analysis can distinguish a
  # launcher ANR overlay from an app Activity or IME focus failure.
  adb shell dumpsys window > "$destination/dumpsys-window.txt" 2> "$destination/dumpsys-window.stderr" || true
  adb shell dumpsys activity activities > "$destination/dumpsys-activity-activities.txt" 2> "$destination/dumpsys-activity-activities.stderr" || true
  adb shell dumpsys activity lastanr > "$destination/dumpsys-activity-lastanr.txt" 2> "$destination/dumpsys-activity-lastanr.stderr" || true
  adb shell dumpsys activity exit-info com.google.android.apps.nexuslauncher > "$destination/dumpsys-launcher-exit-info.txt" 2> "$destination/dumpsys-launcher-exit-info.stderr" || true
  adb shell dumpsys meminfo com.google.android.apps.nexuslauncher > "$destination/dumpsys-launcher-meminfo.txt" 2> "$destination/dumpsys-launcher-meminfo.stderr" || true
  adb logcat -b events -d -v threadtime > "$destination/logcat-events-raw.txt" 2> "$destination/logcat-events.stderr" || true
  adb logcat -b system -d -v threadtime > "$destination/logcat-system-raw.txt" 2> "$destination/logcat-system.stderr" || true
  python3 - "$destination" <<'PY'
from pathlib import Path
import re
import sys

directory = Path(sys.argv[1])
window = (directory / "dumpsys-window.txt").read_text(encoding="utf-8", errors="replace")
summary = []
for line in window.splitlines():
    if re.search(r"mCurrentFocus|mFocusedWindow|mFocusedApp", line):
        summary.append(line.strip())
(directory / "focus-owner-summary.txt").write_text("\n".join(summary) + "\n", encoding="utf-8")
pattern = re.compile(r"ANR in|am_anr|Application Not Responding|not responding", re.IGNORECASE)
for filename in ("logcat-events-raw.txt", "logcat-system-raw.txt", "dumpsys-activity-lastanr.txt"):
    lines = (directory / filename).read_text(encoding="utf-8", errors="replace").splitlines()
    matches = [line for line in lines if pattern.search(line)]
    (directory / f"{filename}.anr-extract.txt").write_text("\n".join(matches[-300:]) + "\n", encoding="utf-8")
PY
}

recover_disposable_launcher_anr() {
  local window_dump="$evidence_root/system-health-focused-before-1/dumpsys-window.txt"
  local recovery_dir="$evidence_root/system-health-launcher-recovery"
  [[ "$api_level" == 35 ]] || return 0
  if ! rg -q 'mCurrentFocus=.*Application Not Responding: com\.google\.android\.apps\.nexuslauncher' "$window_dump"; then
    echo 'Launcher ANR recovery not applied: focused-before-1 had no matching ANR window.' \
      > "$evidence_root/system-health-launcher-recovery/status.txt"
    return 0
  fi

  if [[ "${SEREA_DISPOSABLE_CI_EMULATOR:-}" != 1 || "${GITHUB_ACTIONS:-}" != true ]]; then
    echo 'Refusing system-app recovery outside the disposable GitHub Actions emulator.' \
      > "$evidence_root/system-health-launcher-recovery/status.txt"
    return 1
  fi
  mkdir -p "$recovery_dir"
  {
    printf 'action=force-stop-and-restart-default-home-only-after-observed-launcher-anr\n'
    printf 'observed_window_dump_sha256='; sha256sum "$window_dump" | awk '{print $1}'
    printf 'started_at_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)"
    printf 'launcher_apk_path:\n'; adb shell pm path com.google.android.apps.nexuslauncher
  } > "$recovery_dir/recovery-manifest.txt" 2>&1
  if ! adb shell pm path com.google.android.apps.nexuslauncher | grep -q '^package:'; then
    echo 'Launcher package is absent; no recovery action was taken.' > "$recovery_dir/status.txt"
    return 1
  fi
  adb shell am force-stop com.google.android.apps.nexuslauncher \
    > "$recovery_dir/force-stop.stdout.txt" 2> "$recovery_dir/force-stop.stderr.txt"
  printf '%s\n' "$?" > "$recovery_dir/force-stop.exit-code.txt"
  adb shell am start -a android.intent.action.MAIN -c android.intent.category.HOME \
    > "$recovery_dir/start-home.stdout.txt" 2> "$recovery_dir/start-home.stderr.txt"
  printf '%s\n' "$?" > "$recovery_dir/start-home.exit-code.txt"

  local recovered=false
  for _ in $(seq 1 30); do
    adb shell dumpsys window > "$recovery_dir/dumpsys-window-poll.txt" 2> "$recovery_dir/dumpsys-window-poll.stderr.txt" || true
    if ! rg -q 'mCurrentFocus=.*Application Not Responding: com\.google\.android\.apps\.nexuslauncher' \
      "$recovery_dir/dumpsys-window-poll.txt"; then
      recovered=true
      break
    fi
    sleep 1
  done
  capture_system_health launcher-recovery
  printf 'recovered=%s\nfinished_at_utc=%s\n' "$recovered" "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" \
    >> "$recovery_dir/recovery-manifest.txt"
  if [[ "$recovered" == true ]]; then
    echo 'PASS: observed launcher ANR overlay cleared on the disposable emulator; no app data was cleared.' \
      > "$recovery_dir/status.txt"
    return 0
  fi
  echo 'FAIL: launcher ANR overlay remained after the isolated recovery attempt.' > "$recovery_dir/status.txt"
  return 1
}

EXPECTED_API_LEVEL="$api_level"
SEREA_EXPECTED_ANDROID_API="$api_level"
export EXPECTED_API_LEVEL SEREA_EXPECTED_ANDROID_API
source "$repo_root/tools/assert_disposable_android_emulator.sh" || exit 1

if [[ "$(git -C "$source_repo" rev-parse HEAD)" != "$source_sha" ]]; then
  echo "Refusing to run: tested subject SHA is not the fixed PR #100 HEAD." >&2
  exit 1
fi
if [[ "$(adb shell getprop ro.build.version.sdk | tr -d '\r')" != "$api_level" ]]; then
  echo 'Emulator API does not match the selected matrix API.' >&2
  exit 1
fi
if [[ ! -s "$target_apk" ]]; then
  echo "Prebuilt isolated APK is missing: $target_apk" >&2
  exit 1
fi

expected_apks="$evidence_root/apk-sha256.txt"
current_apks="$evidence_root/apk-sha256-current.txt"
snapshot_apks() {
  find "$source_repo/android/app/build/outputs/apk" -type f -name '*.apk' -print0 | sort -z | xargs -0 sha256sum
}
verify_apks_unchanged() {
  snapshot_apks > "$current_apks"
  if ! cmp -s "$expected_apks" "$current_apks"; then
    echo 'APK hashes changed during the campaign.' >&2
    diff -u "$expected_apks" "$current_apks" >&2 || true
    return 1
  fi
}

export_device_diagnostics() {
  local output_dir="$1"
  local file_list="$output_dir/device-files.txt"
  local export_log="$output_dir/device-export.log"
  local device_path relative_path destination export_status=0
  adb shell run-as "$package" find files/root-cause -type f \
    > "$file_list" 2> "$export_log"
  if [[ "$?" -ne 0 ]]; then
    printf 'find_exit=1\n' >> "$export_log"
    return 0
  fi
  while IFS= read -r device_path; do
    [[ "$device_path" == files/root-cause/* ]] || continue
    relative_path="${device_path#files/root-cause/}"
    destination="$output_dir/app-files/$relative_path"
    mkdir -p "$(dirname "$destination")"
    if ! adb exec-out run-as "$package" cat "$device_path" \
      > "$destination" 2>> "$export_log"; then
      printf 'export_failed=%s\n' "$device_path" >> "$export_log"
      export_status=1
    fi
  done < "$file_list"
  printf 'export_status=%s\n' "$export_status" >> "$export_log"
  return 0
}

run_stage() {
  local stage="$1"
  local test_filter="$2"
  local output_dir="$evidence_root/$stage"
  local token="serea-suite-api${api_level}-${GITHUB_RUN_ID:-local}-${GITHUB_RUN_ATTEMPT:-1}-${stage}"
  local started gradle_status seed_status observer_pid='' log_capture_status=0
  mkdir -p "$output_dir"
  printf 'NOT_RUN\n' > "$output_dir/status.txt"
  {
    printf 'api=%s\nsource_sha=%s\ntest_subject_sha=%s\n' "$api_level" "$source_sha" "$(git -C "$source_repo" rev-parse HEAD)"
    printf 'diagnostic_checkout_sha=%s\nstage=%s\ntest_filter=%s\n' "$(git -C "$repo_root" rev-parse HEAD)" "$stage" "$test_filter"
    printf 'apk_sha256_manifest=%s\nstage_started_utc=%s\n' "$expected_apks" "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)"
    printf 'emulator_build=%s\n' "$(adb shell getprop ro.build.fingerprint | tr -d '\r')"
    printf 'emulator_size=%s\n' "$(adb shell wm size 2>&1 | tr '\n' ' ')"
    printf 'emulator_density=%s\n' "$(adb shell wm density 2>&1 | tr '\n' ' ')"
  } > "$output_dir/manifest.txt"

  verify_apks_unchanged || { echo 'APK verification failed before stage.' > "$output_dir/status.txt"; failures=1; return; }
  adb logcat -c >/dev/null 2>&1 || true
  adb uninstall "$test_package" > "$output_dir/uninstall-test.log" 2>&1 || true
  adb uninstall "$package" > "$output_dir/uninstall-target.log" 2>&1 || true
  started="$(date +%s%3N)"
  if ! adb install "$target_apk" > "$output_dir/install-target.log" 2>&1; then
    echo 'INSTALL_FAIL' > "$output_dir/status.txt"
    failures=1
    return
  fi
  printf 'install_elapsed_ms=%s\n' "$(( $(date +%s%3N) - started ))" >> "$output_dir/manifest.txt"

  started="$(date +%s%3N)"
  EXPECTED_API_LEVEL="$api_level" bash "$source_repo/tools/prepare_isolated_instrumentation_target.sh" "$token" \
    > "$output_dir/seed.log" 2>&1
  seed_status=$?
  printf 'seed_exit_code=%s\nseed_elapsed_ms=%s\n' "$seed_status" "$(( $(date +%s%3N) - started ))" >> "$output_dir/manifest.txt"
  if [[ "$seed_status" -ne 0 ]]; then
    echo 'SEED_FAIL' > "$output_dir/status.txt"
    failures=1
    return
  fi

  rm -rf "$source_repo/android/app/build/outputs/androidTest-results/connected"
  local gradle_args=(--no-daemon
    "-Pandroid.testInstrumentationRunnerArguments.sereaCampaignToken=$token"
    -Pandroid.testInstrumentationRunnerArguments.listener=app.serea.android.SereaInstrumentationLifecycleListener)
  if [[ -n "$test_filter" ]]; then
    gradle_args+=("-Pandroid.testInstrumentationRunnerArguments.class=$test_filter")
  fi
  gradle_args+=(-Pandroid.testInstrumentationRunnerArguments.rootCauseHostDiagnostics=true)
  local observer_args=()
  if [[ "$api_level" == 35 ]]; then
    gradle_args+=(-Pandroid.testInstrumentationRunnerArguments.rootCauseHostInput=true)
    observer_args+=(--inject-system-taps)
  fi
  python3 "$repo_root/tools/android_host_tap_observer.py" "$output_dir" "${observer_args[@]}" \
    > "$output_dir/tap-observer.log" 2>&1 &
  observer_pid=$!
  sleep 1
  started="$(date +%s%3N)"
  (
    cd "$source_repo/android" || exit 1
    ./gradlew "${gradle_args[@]}" connectedIsolatedAndroidTest
  ) > "$output_dir/gradle.log" 2>&1
  gradle_status=$?
  kill "$observer_pid" >/dev/null 2>&1 || true
  wait "$observer_pid" >/dev/null 2>&1 || true
  printf 'gradle_exit_code=%s\ntest_elapsed_ms=%s\n' "$gradle_status" "$(( $(date +%s%3N) - started ))" >> "$output_dir/manifest.txt"
  printf 'stage_finished_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" >> "$output_dir/manifest.txt"

  if [[ -d "$source_repo/android/app/build/outputs/androidTest-results/connected" ]]; then
    cp -a "$source_repo/android/app/build/outputs/androidTest-results/connected" "$output_dir/junit"
  fi
  export_device_diagnostics "$output_dir"
  if [[ "$stage" == full-suite && -d "$output_dir/junit" ]]; then
    python3 "$source_repo/tools/verify_android_instrumentation.py" "$output_dir/junit" \
      --minimum 105 --exact-count 105 \
      > "$output_dir/junit-verification.log" 2>&1 || true
  fi
  adb logcat -d -v raw -s \
    SereaRootCauseRaw:I SereaRootCauseFocusOrder:I SereaChatWindowFocus:I SereaChatFocusRestore:I \
    SereaTestLifecycle:I AndroidRuntime:E > "$output_dir/logcat-raw.txt" 2>&1 || log_capture_status=$?
  printf 'log_capture_exit_code=%s\n' "$log_capture_status" >> "$output_dir/manifest.txt"
  python3 "$repo_root/tools/summarize_android_root_cause_files.py" \
    "$output_dir/app-files" "$output_dir/focus-owner-summary.txt" \
    > "$output_dir/focus-owner-extraction.log" 2>&1 || true
  python3 "$repo_root/tools/summarize_android_suite_neighbors.py" \
    "$output_dir/logcat-raw.txt" "$output_dir/junit" \
    > "$output_dir/suite-order-summary.txt" 2>&1 || true
  verify_apks_unchanged || { echo 'APK_HASH_CHANGED' > "$output_dir/status.txt"; failures=1; return; }

  if [[ "$gradle_status" -eq 0 ]]; then
    echo 'PASS' > "$output_dir/status.txt"
  else
    echo 'FAIL' > "$output_dir/status.txt"
    failures=1
  fi
  printf 'result=%s\n' "$(cat "$output_dir/status.txt")" >> "$output_dir/manifest.txt"
  echo "api=$api_level stage=$stage result=$(cat "$output_dir/status.txt") gradle_exit=$gradle_status"
}

if [[ "$api_level" == 35 ]]; then
  focused_filter='app.serea.android.ChatScreenTest#demoLabelAndConversationTitleAreVisible,app.serea.android.ChatScreenTest#completedDemoConversationSurvivesActivityRecreation,app.serea.android.ChatScreenTest#currentConversationDeletionRequiresConfirmationAndCancelKeepsMessages,app.serea.android.ChatScreenTest#confirmedCurrentConversationDeletionRemovesSelectedConversation,app.serea.android.ChatScreenTest#deleteAllConfirmationCancelKeepsConversationAndConfirmClearsHistory'
  capture_system_health boot-before-focused
  run_stage focused-before-1 "$focused_filter"
  capture_system_health focused-before-1
  recover_disposable_launcher_anr || failures=1
  run_stage focused-before-2 "$focused_filter"
  run_stage focused-before-3 "$focused_filter"
  run_stage chat-screen-class app.serea.android.ChatScreenTest
  run_stage full-suite ''
  run_stage focused-after-1 "$focused_filter"
  run_stage focused-after-2 "$focused_filter"
  run_stage focused-after-3 "$focused_filter"
else
  focused_filter='app.serea.android.SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges'
  run_stage focused-before-1 "$focused_filter"
  run_stage focused-before-2 "$focused_filter"
  run_stage focused-before-3 "$focused_filter"
  run_stage app-shell-class app.serea.android.SereaAppShellTest
  run_stage full-suite ''
  run_stage focused-after-1 "$focused_filter"
  run_stage focused-after-2 "$focused_filter"
  run_stage focused-after-3 "$focused_filter"
fi

{
  printf 'api=%s\nsource_sha=%s\ntest_subject_sha=%s\n' "$api_level" "$source_sha" "$(git -C "$source_repo" rev-parse HEAD)"
  printf 'pr104_run_id=39979381035\npr104_source_sha=b1e7a7359045d716e9f903d85c1291dff8453520\n'
  printf 'apk_sha256_manifest=%s\n' "$expected_apks"
  printf 'completed_utc=%s\nfailures_observed=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" "$failures"
  for stage in "$evidence_root"/*; do
    [[ -d "$stage" && -f "$stage/status.txt" ]] || continue
    printf '%s=%s\n' "$(basename "$stage")" "$(cat "$stage/status.txt")"
  done
} > "$evidence_root/campaign-summary.txt"
exit "$failures"
