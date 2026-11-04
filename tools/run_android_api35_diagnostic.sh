#!/usr/bin/env bash
# Standalone observational reproduction on a disposable API 35 emulator.
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root/android" || exit 1
evidence=app/build/outputs/androidTest-results/api35-isolation
mkdir -p "$evidence" || exit 1
export EXPECTED_API_LEVEL=35 SEREA_EXPECTED_ANDROID_API=35
printf 'NOT_RUN\n' > "$evidence/result.txt"
printf 'NOT_RUN\n' > "$evidence/test-result.txt"
printf 'INCOMPLETE\n' > "$evidence/diagnostic-result.txt"
printf 'NOT_RUN\n' > "$evidence/ime-cleanup.txt"
source "$root/tools/assert_disposable_android_emulator.sh" || exit 1
source "$root/tools/wait_for_android_default_ime.sh"
source "$root/tools/android_package_path.sh"
wait_for_android_default_ime || { echo 'FAIL: default IME not ready' > "$evidence/result.txt"; exit 1; }
initial_ime=$SEREA_ANDROID_DEFAULT_IME
ime_installed=0
log_pid='' monitor_pid=''
cleanup() {
  [[ -n "$monitor_pid" ]] && kill "$monitor_pid" 2>/dev/null
  [[ -n "$monitor_pid" ]] && wait "$monitor_pid" 2>/dev/null
  [[ -n "$log_pid" ]] && kill "$log_pid" 2>/dev/null
  [[ -n "$log_pid" ]] && wait "$log_pid" 2>/dev/null
  if [[ "$ime_installed" == 1 ]]; then
    adb shell ime set "$initial_ime" > "$evidence/ime-cleanup.log" 2>&1
    adb shell ime disable app.serea.android.r25testime/app.serea.android.diagnostics.RealImeService >> "$evidence/ime-cleanup.log" 2>&1
    adb uninstall app.serea.android.r25testime >> "$evidence/ime-cleanup.log" 2>&1
    restored=$(adb shell settings get secure default_input_method 2>&1); restore_query=$?
    remaining=$(android_package_path app.serea.android.r25testime); path_query=$?
    restored=${restored//$'\r'/}; remaining=${remaining//$'\r'/}
    if [[ "$restore_query" -eq 0 && "$path_query" -eq 0 && "$restored" == "$initial_ime" && -z "$remaining" ]]; then echo PASS > "$evidence/ime-cleanup.txt"; else echo FAIL > "$evidence/ime-cleanup.txt"; fi
  fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
{
  printf 'source_sha=%s\nrun_id=%s\nattempt=%s\nserial=%s\n' "$(git rev-parse HEAD)" "${GITHUB_RUN_ID:-local}" "${GITHUB_RUN_ATTEMPT:-1}" "$ANDROID_SERIAL"
  for apk in app/build/outputs/apk/isolated/app-isolated.apk app/build/outputs/apk/androidTest/isolated/app-isolated-androidTest.apk; do sha256sum "$apk"; done
  adb shell getprop ro.build.fingerprint
} > "$evidence/test-provenance.txt"
inventory=$(adb shell pm list packages -u --user all 2>&1) || { echo "FAIL: package inventory unavailable" > "$evidence/result.txt"; exit 1; }
if ! printf "%s\n" "$inventory" | tr -d "\r" | awk 'NF {if ($0 !~ /^package:[A-Za-z0-9_.]+$/) bad=1; n++} END {exit !(n && !bad)}'; then echo "FAIL: package inventory malformed" > "$evidence/result.txt"; exit 1; fi
for pkg in app.serea.android.isolated app.serea.android.isolated.test app.serea.android.r25testime; do
  path=$(android_package_path "$pkg") || { echo "FAIL: package path query failed" > "$evidence/result.txt"; exit 1; }
  if [[ -n "$path" ]] || printf "%s\n" "$inventory" | tr -d "\r" | grep -Fxq "package:$pkg"; then echo "FAIL: $pkg installed or retained" > "$evidence/result.txt"; exit 1; fi
done
ime_apk=r25-ime-test/build/outputs/apk/debug/r25-test-ime-debug.apk
[[ -s "$ime_apk" ]] || ime_apk=r25-ime-test/build/outputs/apk/debug/r25-ime-test-debug.apk
[[ -s "$ime_apk" ]] && adb install "$ime_apk" > "$evidence/install-ime.log" 2>&1 || { echo 'FAIL: IME install' > "$evidence/result.txt"; exit 1; }
ime_installed=1
adb install app/build/outputs/apk/isolated/app-isolated.apk > "$evidence/install-target.log" 2>&1 || { echo 'FAIL: target install' > "$evidence/result.txt"; exit 1; }
token="serea-api35-${GITHUB_RUN_ID:-1}-${GITHUB_RUN_ATTEMPT:-1}-focused"
bash "$root/tools/prepare_isolated_instrumentation_target.sh" "$token" > "$evidence/seed.log" 2>&1 || { echo 'FAIL: seed' > "$evidence/result.txt"; exit 1; }
if [[ -e app/build/outputs/androidTest-results/connected ]]; then echo 'FAIL: stale connected results' > "$evidence/result.txt"; exit 1; fi
# The disposable emulator has no real accounts/data. Capture bounded system buffers;
# truncation is separately reported and cannot be interpreted as complete evidence.
adb -s "$ANDROID_SERIAL" logcat -c -b main -b system -b crash -b events
(ulimit -f 32768; exec adb -s "$ANDROID_SERIAL" logcat -v threadtime -b main -b system -b crash -b events > "$evidence/logcat.txt" 2> "$evidence/logcat-stderr.txt") &
log_pid=$!
(
  snapshot() {
    local label=$1 filter=$2; shift 2
    local tmp="$evidence/monitor-snapshot.tmp" code semantic=PASS
    timeout 5s adb shell "$@" > "$tmp" 2>&1; code=$?
    if [[ "$code" -ne 0 ]] || grep -Eiq 'Unknown command|Permission Denial|Can.t find service|Bad activity command' "$tmp"; then semantic=INCOMPLETE; fi
    if [[ -n "$filter" ]] && ! grep -Eq "$filter" "$tmp"; then semantic=INCOMPLETE; fi
    if [[ "$label" == EXIT_INFO ]] && ! grep -Fq 'ACTIVITY MANAGER PROCESS EXIT INFO' "$tmp"; then semantic=INCOMPLETE; fi
    if [[ "$label" == INSTRUMENTATION ]] && ! grep -Fq 'ACTIVITY MANAGER' "$tmp"; then semantic=INCOMPLETE; fi
    local selected="$evidence/monitor-selected.tmp" selected_bytes
    if [[ -n "$filter" ]]; then grep -E "$filter" "$tmp" > "$selected"; else cp "$tmp" "$selected"; fi
    selected_bytes=$(wc -c < "$selected")
    if (( selected_bytes > 16000 )); then semantic=INCOMPLETE; fi
    printf '\n%s exit=%s bytes=%s semantic=%s selected_bytes=%s\n' "$label" "$code" "$(wc -c < "$tmp")" "$semantic" "$selected_bytes"
    head -c 16000 "$selected"
    rm -f "$tmp" "$selected"
  }
  while :; do
    printf '\n=== %s ===\n' "$(date -u +%FT%T.%3NZ)"
    printf "ADB_STATE="; timeout 3s adb get-state 2>&1
    printf "DEVICE_UPTIME_BOOT_ID="; timeout 3s adb shell cat /proc/uptime /proc/sys/kernel/random/boot_id 2>&1
    printf "LOGCAT_READER="; if kill -0 "$log_pid" 2>/dev/null; then echo alive; else echo exited; fi
    for pkg in app.serea.android.isolated app.serea.android.isolated.test app.serea.android.r25testime; do printf '%s pid=' "$pkg"; timeout 3s adb shell pidof "$pkg" 2>&1; done
    snapshot INSTRUMENTATION 'ACTIVITY MANAGER|Active instrumentation|Instrumentation #|mClass=|mTargetInfo=|mRunningProcesses=' dumpsys activity processes
    snapshot EXIT_INFO '' dumpsys activity exit-info app.serea.android.isolated
    snapshot DEVICE_SHELL_PROCESSES 'dumpsys|logcat|serea|instrument' ps -A -o PID,PPID,STAT,NAME,ARGS
    snapshot FOCUS 'mCurrentFocus|mFocusedApp' dumpsys window windows
    snapshot INPUT_METHOD 'mCurMethodId|mCurFocusedWindow|mCurEditorInfo|mInputShown' dumpsys input_method
    printf '\nHOST_PROCESSES\n'
    ps -eo pid,ppid,stat,rss,comm | awk 'NR==1 || /java|gradle|adb|emulator|qemu/'
    printf 'MEMORY\n'; head -5 /proc/meminfo
    printf 'CGROUP_MEMBERSHIP\n'; cat /proc/self/cgroup 2>&1
    cgroup_found=0
    for f in /sys/fs/cgroup/memory.current /sys/fs/cgroup/memory.events /sys/fs/cgroup/memory.max /sys/fs/cgroup/memory/memory.usage_in_bytes /sys/fs/cgroup/memory/memory.oom_control; do
      if [[ -r "$f" ]]; then cgroup_found=1; printf '%s\n' "$f"; cat "$f" || printf 'CGROUP_READ_FAIL=%s\n' "$f"; fi
    done
    if [[ "$cgroup_found" == 0 ]]; then echo 'CGROUP_MEMORY=UNAVAILABLE'; fi
    printf 'LOG_BYTES='; wc -c < "$evidence/logcat.txt"
    sleep 2
  done
) > "$evidence/live-monitor.txt" 2>&1 &
monitor_pid=$!
printf 'started_at_utc=%s\n' "$(date -u +%FT%T.%3NZ)" > "$evidence/host-lifecycle.txt"
timeout --signal=INT --kill-after=30s 900s ./gradlew --no-daemon \
  -Pandroid.testInstrumentationRunnerArguments.sereaCampaignToken="$token" \
  -Pandroid.testInstrumentationRunnerArguments.listener=app.serea.android.SereaInstrumentationLifecycleListener \
  -Pandroid.testInstrumentationRunnerArguments.class='app.serea.android.SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges' \
  connectedIsolatedAndroidTest > "$evidence/gradle.log" 2>&1 &
gradle_pid=$!
printf 'timeout_process_pid=%s\n' "$gradle_pid" >> "$evidence/host-lifecycle.txt"
wait "$gradle_pid"; gradle_status=$?
printf 'finished_at_utc=%s\ngradle_exit_code=%s\n' "$(date -u +%FT%T.%3NZ)" "$gradle_status" >> "$evidence/host-lifecycle.txt"
if [[ "$gradle_status" == 124 ]]; then echo 'wrapper_outcome=timeout_INT_sent' >> "$evidence/host-lifecycle.txt"; elif (( gradle_status >= 128 )); then echo 'wrapper_outcome=signal_or_wrapped_child_signal_unresolved' >> "$evidence/host-lifecycle.txt"; else echo 'wrapper_outcome=normal_or_child_exit' >> "$evidence/host-lifecycle.txt"; fi
# Capture process-exit records before any explicit app cleanup. Availability varies by image.
timeout 15s adb shell dumpsys activity exit-info app.serea.android.isolated > "$evidence/exit-info.txt" 2>&1
printf '%s\n' "$?" > "$evidence/exit-info-command-status.txt"
timeout 15s adb shell dumpsys activity processes > "$evidence/instrumentation-after.txt" 2>&1
printf '%s\n' "$?" > "$evidence/instrumentation-after-status.txt"
if [[ -d app/build/outputs/androidTest-results/connected ]]; then mv app/build/outputs/androidTest-results/connected "$evidence/junit"; fi
verify=1
if [[ -d "$evidence/junit" ]]; then
  python3 "$root/tools/verify_android_instrumentation.py" "$evidence/junit" --minimum 1 --exact-count 1 --require-test 'app.serea.android.SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges' > "$evidence/verification.log" 2>&1; verify=$?
else echo 'NOT_RUN: JUnit missing' > "$evidence/verification.log"; fi
monitor_health=PASS; kill -0 "$monitor_pid" 2>/dev/null || monitor_health=FAIL
kill "$monitor_pid" 2>/dev/null; wait "$monitor_pid" 2>/dev/null; monitor_pid=''
log_health=PASS; kill -0 "$log_pid" 2>/dev/null || log_health=FAIL
kill "$log_pid" 2>/dev/null; wait "$log_pid" 2>/dev/null; reader_status=$?; log_pid=''
bytes=$(wc -c < "$evidence/logcat.txt")
printf 'logcat_bytes=%s\nlogcat_limit_bytes=33554432\n' "$bytes" > "$evidence/capture-status.txt"
if (( bytes >= 33554432 )); then echo 'logcat=TRUNCATED' >> "$evidence/capture-status.txt"; else echo 'logcat=BELOW_LIMIT' >> "$evidence/capture-status.txt"; fi
printf "monitor_health=%s\nlog_reader_health=%s\nlog_reader_wait_status=%s\n" "$monitor_health" "$log_health" "$reader_status" >> "$evidence/capture-status.txt"
lifecycle_health=FAIL
identity='app.serea.android.SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges'
if grep -E "SereaTestLifecycle: START\|[0-9]+\|$identity" "$evidence/logcat.txt" >/dev/null && \
   grep -E "SereaTestLifecycle: FINISH\|[0-9]+\|$identity" "$evidence/logcat.txt" >/dev/null && \
   grep -F 'SereaTestLifecycle: RUN_FINISH|' "$evidence/logcat.txt" >/dev/null; then lifecycle_health=PASS; fi
adb_health=FAIL
if grep -Fx 'ADB_STATE=device' "$evidence/live-monitor.txt" >/dev/null && ! grep -E '^ADB_STATE=' "$evidence/live-monitor.txt" | grep -Fvx 'ADB_STATE=device' >/dev/null; then adb_health=PASS; fi
printf 'lifecycle_health=%s\nadb_observation=%s\n' "$lifecycle_health" "$adb_health" >> "$evidence/capture-status.txt"
cleanup
trap - EXIT
test_status=FAIL
if [[ "$gradle_status" -eq 0 && "$verify" -eq 0 ]]; then test_status=PASS; fi
printf '%s\n' "$test_status" > "$evidence/test-result.txt"
instrumentation_health=INCOMPLETE
if grep -Eq '^INSTRUMENTATION exit=0 bytes=[1-9][0-9]* semantic=PASS selected_bytes=[0-9]+$' "$evidence/live-monitor.txt" && \
   grep -F 'Active instrumentation:' "$evidence/live-monitor.txt" >/dev/null && \
   grep -E 'Instrumentation #[0-9]+: .*app\.serea\.android' "$evidence/live-monitor.txt" >/dev/null; then
  instrumentation_health=PASS
fi
snapshot_health=PASS
for label in INSTRUMENTATION EXIT_INFO DEVICE_SHELL_PROCESSES FOCUS INPUT_METHOD; do
  if ! grep -Eq "^${label} exit=0 bytes=[1-9][0-9]* semantic=PASS selected_bytes=[0-9]+$" "$evidence/live-monitor.txt" || \
     grep -E "^${label} .*semantic=INCOMPLETE selected_bytes=[0-9]+$" "$evidence/live-monitor.txt" >/dev/null; then snapshot_health=INCOMPLETE; fi
done
if grep -E 'Unknown command|Error:|find service' "$evidence/instrumentation-after.txt" >/dev/null || \
   ! grep -q 'ACTIVITY MANAGER' "$evidence/instrumentation-after.txt"; then instrumentation_health=INCOMPLETE; fi
exit_info_health=PASS
if grep -Eiq 'Unknown command|Permission Denial|Can.t find service|Bad activity command' "$evidence/exit-info.txt" || ! grep -Fq 'ACTIVITY MANAGER PROCESS EXIT INFO' "$evidence/exit-info.txt"; then exit_info_health=INCOMPLETE; fi
printf 'exit_info_observation=%s\n' "$exit_info_health" >> "$evidence/capture-status.txt"
cgroup_health=PASS
if grep -Eq 'CGROUP_MEMORY=UNAVAILABLE|CGROUP_READ_FAIL=' "$evidence/live-monitor.txt" || ! grep -Eq '^/sys/fs/cgroup/(memory.events|memory/memory.oom_control)$' "$evidence/live-monitor.txt"; then cgroup_health=INCOMPLETE; fi
printf 'cgroup_observation=%s\n' "$cgroup_health" >> "$evidence/capture-status.txt"
printf 'instrumentation_observation=%s\nsnapshot_observation=%s\n' "$instrumentation_health" "$snapshot_health" >> "$evidence/capture-status.txt"
diagnostic_status=INCOMPLETE
if [[ "$monitor_health" == PASS && "$log_health" == PASS && "$lifecycle_health" == PASS && \
      "$adb_health" == PASS && "$instrumentation_health" == PASS && "$snapshot_health" == PASS && "$cgroup_health" == PASS && "$exit_info_health" == PASS && \
      "$bytes" -gt 0 && "$bytes" -lt 33554432 && "$(cat "$evidence/exit-info-command-status.txt")" == 0 && \
      "$(cat "$evidence/instrumentation-after-status.txt")" == 0 && "$(cat "$evidence/ime-cleanup.txt")" == PASS ]]; then
  diagnostic_status=PASS
fi
printf '%s\n' "$diagnostic_status" > "$evidence/diagnostic-result.txt"
printf 'test=%s diagnostic=%s gradle=%s junit=%s ime_cleanup=%s\n' "$test_status" "$diagnostic_status" "$gradle_status" "$verify" "$(cat "$evidence/ime-cleanup.txt")" > "$evidence/result.txt"
[[ "$test_status" == PASS && "$diagnostic_status" == PASS ]]
