#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/assert_disposable_android_emulator.sh"

: "${EXPECTED_API_LEVEL:=${SEREA_EXPECTED_ANDROID_API:-}}"
: "${SEREA_CONSENT_DEATH_RUN_ID:?Set a unique host run ID.}"
[[ "$SEREA_CONSENT_DEATH_RUN_ID" =~ ^[A-Za-z0-9-]{1,120}$ ]]
package=app.serea.android.isolated
test_package=app.serea.android.isolated.test
component="$package/app.serea.android.googleauth.PrivateGoogleInFlightDeathHarnessActivity"
sender_component="$package/app.serea.android.googleauth.PrivateGoogleConsentSenderTestActivity"
evidence="${SEREA_CONSENT_DEATH_EVIDENCE_DIR:-$(dirname "$0")/../android/app/build/outputs/androidTest-results/api${EXPECTED_API_LEVEL}/inflight-consent-process-death}"
mkdir -p "$evidence"
printf 'NOT_RUN\n' > "$evidence/status.txt"
adb_cmd=(adb -s "$ANDROID_SERIAL")

fail() {
  printf 'FAIL\n' > "$evidence/status.txt"
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

pid_for() {
  "${adb_cmd[@]}" shell ps -A -o PID,NAME | awk -v name="$1" 'NR > 1 && $2 == name { print $1 }'
}

package_path="$("${adb_cmd[@]}" shell pm path "$package" | tr -d '\r')"
[[ -n "$package_path" ]] || fail 'isolated target package is not installed'
test_pid_before="$(pid_for "$test_package")"
os_release="$("${adb_cmd[@]}" shell getprop ro.build.version.release | tr -d '\r')"
sdk="$("${adb_cmd[@]}" shell getprop ro.build.version.sdk | tr -d '\r')"
[[ "$sdk" == "$EXPECTED_API_LEVEL" ]] || fail "device API $sdk did not match expected API $EXPECTED_API_LEVEL"

{
  printf 'run_id=%s\nhead=%s\napi=%s\nos_release=%s\nbuild_fingerprint=%s\nserial=%s\ntarget_package=%s\ninstrumentation_package=%s\ninstrumentation_pid_before=%s\n' \
    "$SEREA_CONSENT_DEATH_RUN_ID" "$(git -C "$(dirname "$0")/.." rev-parse HEAD)" "$sdk" "$os_release" \
    "$("${adb_cmd[@]}" shell getprop ro.build.fingerprint | tr -d '\r')" "$ANDROID_SERIAL" "$package" "$test_package" "${test_pid_before:-absent}"
} > "$evidence/provenance.txt"

# The host shell controls the target independently and never signals the AndroidJUnitRunner package.
"${adb_cmd[@]}" shell am force-stop "$package"
"${adb_cmd[@]}" logcat -c
"${adb_cmd[@]}" shell am start -W -n "$component" \
  --es phase in-flight-A --es token "$SEREA_CONSENT_DEATH_RUN_ID" > "$evidence/start-A.txt" 2>&1 || fail 'could not start Consent A harness'
grep -Fxq 'Status: ok' "$evidence/start-A.txt" || fail 'ActivityManager did not start the Consent A harness'

for _ in $(seq 1 30); do
  "${adb_cmd[@]}" logcat -d -s SereaConsentDeath:I > "$evidence/in-flight-logcat.txt"
  if grep -F "IN_FLIGHT:$SEREA_CONSENT_DEATH_RUN_ID:request=request-A:generation=41:session=session-A:pid=" "$evidence/in-flight-logcat.txt" >/dev/null; then break; fi
  sleep 1
done
grep -Fq "IN_FLIGHT:$SEREA_CONSENT_DEATH_RUN_ID:request=request-A:generation=41:session=session-A:pid=" "$evidence/in-flight-logcat.txt" || fail 'A did not enter the synthetic in-flight Consent state'
"${adb_cmd[@]}" shell dumpsys activity activities > "$evidence/activities-in-flight.txt"
"${adb_cmd[@]}" shell dumpsys window windows > "$evidence/windows-in-flight.txt"
grep -Fq "$sender_component" "$evidence/activities-in-flight.txt" || fail 'synthetic Consent sender was not the active Activity'
target_pid_before="$(pid_for "$package")"
[[ -n "$target_pid_before" ]] || fail 'target process PID was missing before OS process death'
printf 'target_pid_before=%s\n' "$target_pid_before" >> "$evidence/provenance.txt"

# This is a real ActivityManager process death while the synthetic sender is foreground.
"${adb_cmd[@]}" shell am force-stop "$package"
target_pid_after="$(pid_for "$package")"
[[ -z "$target_pid_after" ]] || fail "target process survived force-stop with PID $target_pid_after"
printf 'target_pid_after_kill=absent\n' >> "$evidence/provenance.txt"

"${adb_cmd[@]}" shell am start -W -n "$component" \
  --es phase verify-B --es token "$SEREA_CONSENT_DEATH_RUN_ID" > "$evidence/start-B.txt" 2>&1 || fail 'could not restart app for request B'
grep -Fxq 'Status: ok' "$evidence/start-B.txt" || fail 'ActivityManager did not start request B verification'
for _ in $(seq 1 30); do
  "${adb_cmd[@]}" logcat -d -s SereaConsentDeath:I > "$evidence/recovery-logcat.txt"
  if grep -F "STALE_REJECTED:$SEREA_CONSENT_DEATH_RUN_ID:request=request-B:generation=42:session=session-B:pid=" "$evidence/recovery-logcat.txt" >/dev/null; then break; fi
  sleep 1
done
grep -Fq "STALE_REJECTED:$SEREA_CONSENT_DEATH_RUN_ID:request=request-B:generation=42:session=session-B:pid=" "$evidence/recovery-logcat.txt" || fail 'request B did not reject stale A consent and picker results'
if grep -Fq "RESULT_DELIVERED:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/recovery-logcat.txt"; then
  printf 'delivered\n' > "$evidence/activity-result-delivery.txt"
else
  printf 'not-delivered-by-android-after-force-stop\n' > "$evidence/activity-result-delivery.txt"
fi
"${adb_cmd[@]}" shell dumpsys activity activities > "$evidence/activities-after-restart.txt"
"${adb_cmd[@]}" shell dumpsys window windows > "$evidence/windows-after-restart.txt"

test_pid_after="$(pid_for "$test_package")"
printf 'instrumentation_pid_after=%s\n' "${test_pid_after:-absent}" >> "$evidence/provenance.txt"
[[ "$test_pid_before" == "$test_pid_after" ]] || fail 'instrumentation package process changed during target process death check'
target_pid_after_restart="$(pid_for "$package")"
[[ -n "$target_pid_after_restart" && "$target_pid_after_restart" != "$target_pid_before" ]] || fail 'target process did not return with a new PID'
printf 'target_pid_after_restart=%s\n' "$target_pid_after_restart" >> "$evidence/provenance.txt"
printf 'PASS\n' > "$evidence/status.txt"
printf 'PASS: API %s OS process death while synthetic Consent A was foreground; ActivityResult=%s; stale A rejected against B. Evidence: %s\n' \
  "$sdk" "$(cat "$evidence/activity-result-delivery.txt")" "$evidence"
