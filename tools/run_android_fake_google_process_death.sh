#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/assert_disposable_android_emulator.sh"
source "$(dirname "$0")/android_package_path.sh"

: "${EXPECTED_API_LEVEL:=${SEREA_EXPECTED_ANDROID_API:-}}"
: "${SEREA_ISOLATED_INSTRUMENTATION_FINISHED:?Set to 1 only after the isolated instrumentation command has exited.}"
: "${SEREA_FAKE_GOOGLE_DEATH_TOKEN:?Provide a unique token for this disposable process-death run.}"
test "$SEREA_ISOLATED_INSTRUMENTATION_FINISHED" = 1
[[ "$SEREA_FAKE_GOOGLE_DEATH_TOKEN" =~ ^[A-Za-z0-9-]{1,120}$ ]]

package="app.serea.android.isolated"
test_package="app.serea.android.isolated.test"
# The isolated variant changes applicationId, not the Kotlin namespace. Use the
# class's namespace explicitly; the relative component resolves under the
# suffixed app ID and points at a class that does not exist.
activity="$package/app.serea.android.FakeGoogleProcessDeathHarnessActivity"
tag="SereaFakeGoogleDeath"
token="$SEREA_FAKE_GOOGLE_DEATH_TOKEN"
evidence="${SEREA_FAKE_GOOGLE_DEATH_EVIDENCE_DIR:-$(dirname "$0")/../android/app/build/outputs/androidTest-results/api${EXPECTED_API_LEVEL}/fake-google-process-death}"
target_apk="${SEREA_ISOLATED_TARGET_APK:-app/build/outputs/apk/isolated/app-isolated.apk}"
mkdir -p "$evidence"
printf 'NOT_RUN\n' > "$evidence/status.txt"

fail_campaign() {
  printf 'FAIL\n' > "$evidence/status.txt"
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

adb_cmd=(adb -s "$ANDROID_SERIAL")
if [[ ! -f "$target_apk" ]]; then
  fail_campaign "the isolated target APK is missing: $target_apk"
fi
if ! target_apk_sha256="$(sha256sum "$target_apk" | awk '{print $1}')" || [[ ! "$target_apk_sha256" =~ ^[[:xdigit:]]{64}$ ]]; then
  fail_campaign 'could not calculate a valid SHA-256 for the isolated target APK.'
fi
if ! test_package_paths="$(ANDROID_SERIAL="$ANDROID_SERIAL" android_package_path "$test_package")"; then
  printf 'FAIL\n' > "$evidence/status.txt"
  echo 'Could not verify the isolated instrumentation package state.' >&2
  exit 1
fi
if [[ -n "$test_package_paths" ]]; then
  printf 'FAIL\n' > "$evidence/status.txt"
  echo 'The isolated instrumentation package remains installed after the test stage.' >&2
  exit 1
fi
if ! package_paths="$(ANDROID_SERIAL="$ANDROID_SERIAL" android_package_path "$package")"; then
  printf 'FAIL\n' > "$evidence/status.txt"
  echo 'Could not verify the isolated target package state.' >&2
  exit 1
fi
if [[ -z "$package_paths" ]]; then
  if ! "${adb_cmd[@]}" install "$target_apk" > "$evidence/target-install.log" 2>&1; then
    printf 'FAIL\n' > "$evidence/status.txt"
    echo 'Could not install the isolated target APK for the Fake Google process-death harness.' >&2
    cat "$evidence/target-install.log" >&2
    exit 1
  fi
  if ! package_paths="$(ANDROID_SERIAL="$ANDROID_SERIAL" android_package_path "$package")" || [[ -z "$package_paths" ]]; then
    printf 'FAIL\n' > "$evidence/status.txt"
    echo 'The isolated target package was not visible after APK installation.' >&2
    exit 1
  fi
fi
package_paths="${package_paths//$'\r'/}"
if [[ "$package_paths" == *$'\n'* || "$package_paths" != package:* ]]; then
  printf 'FAIL\n' > "$evidence/status.txt"
  echo 'The isolated target APK install returned multiple or malformed package paths.' >&2
  exit 1
fi
package_path="${package_paths#package:}"
if ! installed_apk_sha256="$("${adb_cmd[@]}" shell sha256sum "$package_path" 2>/dev/null | awk 'NR == 1 { print $1 }' | tr -d '\r')" ||
  [[ ! "$installed_apk_sha256" =~ ^[[:xdigit:]]{64}$ ]]; then
  fail_campaign 'could not calculate a valid SHA-256 for the installed isolated target APK.'
fi
if [[ "$installed_apk_sha256" != "$target_apk_sha256" ]]; then
  fail_campaign 'the installed isolated target APK did not match the campaign-built APK.'
fi
{
  printf 'target_package=%s\n' "$package"
  printf 'target_apk=%s\n' "$target_apk"
  printf 'target_apk_sha256=%s\n' "$target_apk_sha256"
  printf 'installed_apk_path=%s\n' "$package_path"
  printf 'installed_apk_sha256=%s\n' "$installed_apk_sha256"
} > "$evidence/target-apk-provenance.txt"

query_package_pid() {
  local target_package="$1" query_result pid_query_script
  case "$target_package" in
    "$package"|"$test_package") ;;
    *) printf 'Refusing an unexpected process package query.\n' >&2; return 1 ;;
  esac
  # adb shell reconstructs remote argv. Passing a multiline script to `sh -c`
  # together with a synthetic $0 caused API 26 to execute `serea-pid-query` as
  # a command while leaving `-c` without its script. Pipe the script to plain
  # remote sh instead; only the two fixed package names above are interpolated.
  # pidof's no-match status is normalized to a sentinel; tool/transport errors
  # remain nonzero and are treated as inability to prove absence.
  pid_query_script="pids=\$(pidof '$target_package' 2>/dev/null)
pidof_status=\$?
case \"\$pidof_status\" in
  0) [ -n \"\$pids\" ] || exit 2; printf '%s\\n' \"\$pids\" ;;
  1) [ -z \"\$pids\" ] || exit 2; printf '__SEREA_PID_ABSENT__\\n' ;;
  *) exit 2 ;;
esac"
  if ! query_result="$(printf '%s\n' "$pid_query_script" | "${adb_cmd[@]}" shell sh)"; then
    printf 'PID query failed for %s.\n' "$target_package" >&2
    return 1
  fi
  query_result="${query_result//$'\r'/}"
  if [[ "$query_result" == '__SEREA_PID_ABSENT__' ]]; then
    return 0
  fi
  if [[ "$query_result" =~ ^[0-9]+([[:space:]]+[0-9]+)*$ ]]; then
    printf '%s' "$query_result"
    return 0
  fi
  printf 'PID query returned malformed output for %s.\n' "$target_package" >&2
  return 1
}

# The app process also hosts the AndroidJUnitRunner in this variant. Refuse to stop it while a
# test process remains; the host script is called only after Gradle/instrumentation has returned.
if ! test_pid="$(query_package_pid "$test_package")"; then
  fail_campaign 'could not prove the isolated instrumentation process is absent.'
fi
if [[ -n "$test_pid" ]]; then
  printf 'FAIL: isolated instrumentation process is still present: %s\n' "$test_pid" | tee "$evidence/status.txt" >&2
  exit 1
fi
if ! initial_target_pid="$(query_package_pid "$package")"; then
  fail_campaign 'could not query the isolated target process before force-stop.'
fi
{
  printf 'api=%s\nserial=%s\ntarget_package=%s\ninstrumentation_package=%s\ninstrumentation_process_before=%s\ntarget_process_before=%s\ntoken=%s\n' \
    "$EXPECTED_API_LEVEL" "$ANDROID_SERIAL" "$package" "$test_package" "${test_pid:-absent}" "${initial_target_pid:-absent}" "$token"
} > "$evidence/host-guard.txt"

wait_target_stopped() {
  local observed
  for _ in $(seq 1 30); do
    if ! observed="$(query_package_pid "$package")"; then
      fail_campaign 'target PID query failed while verifying force-stop.'
    fi
    if [[ -z "$observed" ]]; then
      return 0
    fi
    sleep 1
  done
  printf 'Target PID remained after force-stop: %s\n' "$observed" >&2
  return 1
}

run_phase() {
  local phase="$1"
  local log_file="$evidence/$phase-logcat.txt"
  if ! "${adb_cmd[@]}" shell am start -W -f 0x10008000 -n "$activity" \
    --es phase "$phase" --es token "$token" > "$evidence/$phase-am-start.txt" 2>&1; then
    cat "$evidence/$phase-am-start.txt" >&2
    return 1
  fi
  # `am start -W` can exit zero even when ActivityManager prints Error type 3.
  # Reject any non-success status before waiting for an app marker, so a bad
  # component cannot masquerade as a seed/ViewModel timeout.
  if ! grep -Fxq 'Status: ok' "$evidence/$phase-am-start.txt" ||
    ! grep -Fq "Activity: $activity" "$evidence/$phase-am-start.txt"; then
    cat "$evidence/$phase-am-start.txt" >&2
    return 1
  fi
  for _ in $(seq 1 30); do
    "${adb_cmd[@]}" logcat -d -s "$tag:I" > "$log_file"
    if grep -F "FAIL:$token:$phase:" "$log_file" >/dev/null; then
      cat "$log_file" >&2
      return 1
    fi
    if grep -F "PASS:$token:$phase:pid=" "$log_file" >/dev/null; then
      return 0
    fi
    sleep 1
  done
  printf 'Timed out waiting for phase %s.\n' "$phase" >&2
  cat "$log_file" >&2
  return 1
}

printf 'Starting Fake consent seed in the isolated app process.\n'
"${adb_cmd[@]}" shell am force-stop "$package"
wait_target_stopped
printf 'absent\n' > "$evidence/pre-seed-pid.txt"
if ! run_phase seed; then
  fail_campaign 'Fake Calendar seed phase failed.'
fi
if ! seed_pid="$(query_package_pid "$package")"; then
  fail_campaign 'target PID query failed after Fake Calendar seed.'
fi
if [[ -z "$seed_pid" ]]; then
  fail_campaign 'target process is absent after the Fake Calendar seed phase.'
fi
printf '%s\n' "$seed_pid" > "$evidence/seed-pid.txt"

printf 'Force-stopping only %s after instrumentation has exited.\n' "$package"
"${adb_cmd[@]}" shell am force-stop "$package"
wait_target_stopped
printf 'absent\n' > "$evidence/post-force-stop-pid.txt"

if ! test_pid="$(query_package_pid "$test_package")"; then
  fail_campaign 'instrumentation PID query failed after target force-stop.'
fi
if [[ -n "$test_pid" ]]; then
  fail_campaign "isolated instrumentation process appeared during target process-death check: $test_pid"
fi

printf 'Relaunching a fresh Activity and checking in-memory auth/session state.\n'
if ! run_phase verify; then
  fail_campaign 'fresh-state verification phase failed.'
fi
if ! verify_pid="$(query_package_pid "$package")"; then
  fail_campaign 'target PID query failed after fresh-state verification.'
fi
if [[ -z "$verify_pid" ]]; then
  fail_campaign 'target process is absent after fresh-state verification.'
fi
printf '%s\n' "$verify_pid" > "$evidence/verify-pid.txt"

printf 'PASS\n' > "$evidence/status.txt"
printf 'PASS: Fake Google consent/session OS process-death boundary on API %s.\n' "$EXPECTED_API_LEVEL"
printf 'Evidence: %s\n' "$evidence"
