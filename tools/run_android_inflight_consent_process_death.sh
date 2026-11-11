#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/assert_disposable_android_emulator.sh"

: "${EXPECTED_API_LEVEL:=${SEREA_EXPECTED_ANDROID_API:-}}"
: "${SEREA_CONSENT_DEATH_RUN_ID:?Set a unique host run ID.}"
[[ "$SEREA_CONSENT_DEATH_RUN_ID" =~ ^[A-Za-z0-9-]{1,120}$ ]]
package=app.serea.android.isolated
test_package=app.serea.android.isolated.test
component="$package/app.serea.android.googleauth.PrivateGoogleInFlightDeathHarnessActivity"
sender_component="$test_package/app.serea.android.googleauth.SyntheticConsentSenderActivity"
test_apk="app/build/outputs/apk/androidTest/isolated/app-isolated-androidTest.apk"
evidence="${SEREA_CONSENT_DEATH_EVIDENCE_DIR:-$(dirname "$0")/../android/app/build/outputs/androidTest-results/api${EXPECTED_API_LEVEL}/inflight-consent-process-death}"
mkdir -p "$evidence"
printf 'NOT_RUN\n' > "$evidence/status.txt"
printf 'PrivateGoogleConsentRelayTestActivity.StartIntentSenderForResult -> SyntheticConsentSenderActivity -> PrivateGoogleInFlightDeathHarnessActivity.ActivityResultLauncher\n' > "$evidence/activity-result-route.txt"
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
[[ -s "$test_apk" ]] || fail 'isolated AndroidTest APK is missing'
if ! "${adb_cmd[@]}" install -t -r "$test_apk" > "$evidence/install-test-apk.txt" 2>&1; then
  fail 'could not install the isolated AndroidTest APK for the external consent sender'
fi
test_apk_path="$("${adb_cmd[@]}" shell pm path "$test_package" | tr -d '\r' | sed -n 's/^package://p')"
test_apk_expected="$(sha256sum "$test_apk" | awk '{print $1}')"
test_apk_actual="$("${adb_cmd[@]}" shell sha256sum "$test_apk_path" 2>/dev/null | awk 'NR == 1 { print $1 }')"
[[ -n "$test_apk_path" && "$test_apk_actual" == "$test_apk_expected" ]] || fail 'installed AndroidTest APK did not match the campaign artifact'
test_pid_before="$(pid_for "$test_package")"
os_release="$("${adb_cmd[@]}" shell getprop ro.build.version.release | tr -d '\r')"
sdk="$("${adb_cmd[@]}" shell getprop ro.build.version.sdk | tr -d '\r')"
[[ "$sdk" == "$EXPECTED_API_LEVEL" ]] || fail "device API $sdk did not match expected API $EXPECTED_API_LEVEL"
checkout_head="$(git -C "$(dirname "$0")/.." rev-parse HEAD)"
candidate_head="$checkout_head"
if [[ -n "${GITHUB_EVENT_PATH:-}" && -f "$GITHUB_EVENT_PATH" ]]; then
  candidate_head="$(python3 - "$GITHUB_EVENT_PATH" "$checkout_head" <<'PY'
import json, sys
try:
    event = json.load(open(sys.argv[1], encoding="utf-8"))
    print(event.get("pull_request", {}).get("head", {}).get("sha") or sys.argv[2])
except Exception:
    print(sys.argv[2])
PY
)"
fi

{
  printf 'run_id=%s\nhead=%s\ncheckout_head=%s\napi=%s\nos_release=%s\nbuild_fingerprint=%s\nserial=%s\ntarget_package=%s\ninstrumentation_package=%s\ninstrumentation_apk_sha256=%s\ninstrumentation_pid_before=%s\n' \
    "$SEREA_CONSENT_DEATH_RUN_ID" "$candidate_head" "$checkout_head" "$sdk" "$os_release" \
    "$("${adb_cmd[@]}" shell getprop ro.build.fingerprint | tr -d '\r')" "$ANDROID_SERIAL" "$package" \
    "$test_package" "$test_apk_actual" "${test_pid_before:-absent}"
} > "$evidence/provenance.txt"

# The host shell controls the target independently and never signals the AndroidJUnitRunner package.
"${adb_cmd[@]}" shell am force-stop "$package"
"${adb_cmd[@]}" shell am start -W -n "$component" \
  --es phase in-flight-A --es token "$SEREA_CONSENT_DEATH_RUN_ID" > "$evidence/start-A.txt" 2>&1 || fail 'could not start Consent A harness'
grep -Fxq 'Status: ok' "$evidence/start-A.txt" || fail 'ActivityManager did not start the Consent A harness'

for _ in $(seq 1 30); do
  "${adb_cmd[@]}" logcat -d -s SereaConsentDeath:I > "$evidence/in-flight-logcat.txt"
  if grep -F "IN_FLIGHT:$SEREA_CONSENT_DEATH_RUN_ID:request=request-A:generation=41:session=session-A:pid=" "$evidence/in-flight-logcat.txt" >/dev/null; then break; fi
  sleep 1
done
grep -Fq "IN_FLIGHT:$SEREA_CONSENT_DEATH_RUN_ID:request=request-A:generation=1:session=session-A:pid=" "$evidence/in-flight-logcat.txt" || fail 'A did not enter the Private Google ViewModel in-flight Consent state'
for _ in $(seq 1 30); do
  "${adb_cmd[@]}" logcat -d -s SereaSyntheticConsent:I > "$evidence/sender-logcat.txt"
  if grep -F "SENDER_IN_FLIGHT:$SEREA_CONSENT_DEATH_RUN_ID:pid=" "$evidence/sender-logcat.txt" >/dev/null; then break; fi
  sleep 1
done
grep -Fq "SENDER_IN_FLIGHT:$SEREA_CONSENT_DEATH_RUN_ID:pid=" "$evidence/sender-logcat.txt" || fail 'external synthetic Consent sender did not become active'
test_pid_before="$(pid_for "$test_package")"
[[ -n "$test_pid_before" ]] || fail 'external sender/test package process PID was missing'
printf 'external_sender_pid_before_target_kill=%s\n' "$test_pid_before" >> "$evidence/provenance.txt"
"${adb_cmd[@]}" shell dumpsys activity activities > "$evidence/activities-in-flight.txt"
"${adb_cmd[@]}" shell dumpsys window windows > "$evidence/windows-in-flight.txt"
grep -Fq "$sender_component" "$evidence/activities-in-flight.txt" || fail 'synthetic Consent sender was not the active Activity'
grep -Fq 'PrivateGoogleConsentRelayTestActivity' "$evidence/activities-in-flight.txt" || fail 'the isolated Activity Result relay was not retained under the synthetic sender'
grep -Eq "(mResumedActivity:|topResumedActivity=).*${sender_component//\//\\/}" "$evidence/activities-in-flight.txt" || fail 'synthetic Consent sender was not resumed while A was in flight'
if grep -Fq 'mCurrentFocus=Window{' "$evidence/windows-in-flight.txt"; then
  grep -F 'mCurrentFocus=Window{' "$evidence/windows-in-flight.txt" | grep -Fq "$sender_component" || fail 'synthetic Consent sender did not own window focus while A was in flight'
else
  # Android 15 omits the legacy mCurrentFocus line from `dumpsys window windows`.
  # The resumed Activity assertion above plus this sender-specific visible surface
  # establishes that the external sender is foreground without relying on that line.
  awk -v needle="$sender_component" '
    /^  Window #[0-9]+ Window\{/ {
      if (found) exit
      in_sender = index($0, needle) > 0
      if (in_sender) found = 1
    }
    in_sender { print }
  ' "$evidence/windows-in-flight.txt" > "$evidence/sender-window-in-flight.txt"
  grep -Fq 'mViewVisibility=0x0' "$evidence/sender-window-in-flight.txt" || fail 'synthetic sender window was not visible on API without mCurrentFocus'
  grep -Fq 'isOnScreen=true' "$evidence/sender-window-in-flight.txt" || fail 'synthetic sender window was not on-screen on API without mCurrentFocus'
  grep -Fq 'isVisible=true' "$evidence/sender-window-in-flight.txt" || fail 'synthetic sender window was not visible on API without mCurrentFocus'
  grep -Fq 'Surface: shown=true' "$evidence/sender-window-in-flight.txt" || fail 'synthetic sender surface was not shown on API without mCurrentFocus'
fi
target_pid_before="$(pid_for "$package")"
[[ -n "$target_pid_before" ]] || fail 'target process PID was missing before OS process death'
[[ "$test_pid_before" != "$target_pid_before" ]] || fail 'host-side synthetic sender and Serea target shared one process'
printf 'target_pid_before=%s\n' "$target_pid_before" >> "$evidence/provenance.txt"
"${adb_cmd[@]}" logcat -d -s SereaConsentDeath:I > "$evidence/pre-kill-result-check.txt"
if grep -Fq "RESULT_DELIVERED:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/pre-kill-result-check.txt"; then
  fail 'Consent Activity Result completed before target process death; A was not in flight at kill time'
fi
printf 'no-ActivityResult-callback-before-kill\n' >> "$evidence/provenance.txt"

# The relay Activity remains in the task stack underneath the foreground synthetic sender, so
# Android 8 may classify its process as non-killable for `am kill`. `am force-stop` asks
# ActivityManager to terminate the target package while leaving the separate test/sender package
# alive; it is an OS process death, and we still measure whether the saved result route survives.
"${adb_cmd[@]}" shell am force-stop --user 0 "$package" > "$evidence/am-force-stop.txt" 2>&1
printf 'target_termination_method=ActivityManager am force-stop --user 0 %s\n' "$package" >> "$evidence/provenance.txt"
target_pid_after='pending'
for _ in $(seq 1 30); do
  target_pid_after="$(pid_for "$package")"
  [[ -z "$target_pid_after" ]] && break
  sleep 1
done
[[ -z "$target_pid_after" ]] || fail "target process survived ActivityManager force-stop with PID $target_pid_after"
test_pid_after_kill="$(pid_for "$test_package")"
[[ "$test_pid_before" == "$test_pid_after_kill" ]] || fail 'external sender/instrumentation package process was terminated with the target'
printf 'target_pid_after_force_stop=absent\n' >> "$evidence/provenance.txt"
printf 'external_sender_pid_after_target_death=%s\n' "$test_pid_after_kill" >> "$evidence/provenance.txt"
"${adb_cmd[@]}" shell dumpsys activity activities > "$evidence/activities-after-kill.txt"
"${adb_cmd[@]}" shell dumpsys window windows > "$evidence/windows-after-kill.txt"

"${adb_cmd[@]}" shell am start -W -f 0x18000000 -n "$component" \
  --es phase verify-B --es token "$SEREA_CONSENT_DEATH_RUN_ID" > "$evidence/start-B.txt" 2>&1 || fail 'could not restart app for request B'
grep -Fxq 'Status: ok' "$evidence/start-B.txt" || fail 'ActivityManager did not start request B verification'
grep -Fq "Activity: $component" "$evidence/start-B.txt" || fail 'request B did not start its own harness Activity in a separate task'
"${adb_cmd[@]}" shell dumpsys activity activities > "$evidence/activities-after-B-before-result.txt"
"${adb_cmd[@]}" shell dumpsys window windows > "$evidence/windows-after-B-before-result.txt"
grep -Fq "$sender_component" "$evidence/activities-after-B-before-result.txt" || fail 'external Consent A was not preserved while request B was active'
for _ in $(seq 1 30); do
  "${adb_cmd[@]}" logcat -d -s SereaConsentDeath:I > "$evidence/recovery-logcat.txt"
  if grep -F "B_READY:$SEREA_CONSENT_DEATH_RUN_ID:request=request-B:generation=1:session=session-B:pid=" "$evidence/recovery-logcat.txt" >/dev/null; then break; fi
  sleep 1
done
grep -Fq "B_READY:$SEREA_CONSENT_DEATH_RUN_ID:request=request-B:generation=1:session=session-B:pid=" "$evidence/recovery-logcat.txt" || fail 'request B did not enter its fresh pending authorization state'
grep -Fq "RECOVERED_DISCONNECTED:$SEREA_CONSENT_DEATH_RUN_ID:status=DISCONNECTED:pending=null:calendarSession=null:" "$evidence/recovery-logcat.txt" || fail 'authorization state was restored after process death instead of starting disconnected'
# Complete the original sender only after B is pending, allowing the OS to exercise A's saved
# ActivityResult route against the new request.
"${adb_cmd[@]}" shell am start -W -f 0x04000000 -n "$sender_component" \
  --es token "$SEREA_CONSENT_DEATH_RUN_ID" --es action complete > "$evidence/complete-old-sender.txt" 2>&1 || fail 'could not complete external Consent A after B started'
grep -Fxq 'Status: ok' "$evidence/complete-old-sender.txt" || fail 'ActivityManager did not reactivate the external Consent sender'
for _ in $(seq 1 30); do
  "${adb_cmd[@]}" logcat -d -s SereaConsentDeath:I SereaSyntheticConsent:I > "$evidence/result-delivery-logcat.txt"
  grep -Fq "SENDER_COMPLETED:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/result-delivery-logcat.txt" && break
  sleep 1
done
sender_launch_count="$(grep -Fc "SENDER_IN_FLIGHT:$SEREA_CONSENT_DEATH_RUN_ID:pid=" "$evidence/result-delivery-logcat.txt" || true)"
[[ "$sender_launch_count" == 1 ]] || fail "synthetic Consent sender launched $sender_launch_count times"
sender_completion_count="$(grep -Fc "SENDER_COMPLETED:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/result-delivery-logcat.txt" || true)"
[[ "$sender_completion_count" == 1 ]] || fail "synthetic Consent sender completed $sender_completion_count times"
grep -Fq "SENDER_COMPLETED:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/result-delivery-logcat.txt" || fail 'external Consent A did not complete through its original Activity instance'
for _ in $(seq 1 30); do
  "${adb_cmd[@]}" logcat -d -s SereaConsentDeath:I SereaSyntheticConsent:I > "$evidence/result-delivery-logcat.txt"
  if grep -F "RESULT_DELIVERED:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/result-delivery-logcat.txt" >/dev/null; then break; fi
  sleep 1
done
callback_line="$(grep -F "RESULT_DELIVERED:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/result-delivery-logcat.txt" | tail -n 1 || true)"
callback_count="$(grep -Fc "RESULT_DELIVERED:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/result-delivery-logcat.txt" || true)"
[[ "$callback_count" -le 1 ]] || fail "Activity Result callback was delivered $callback_count times"
valid_a_result=false
callback_status=none-observed-within-30s
if [[ -n "$callback_line" ]]; then
  grep -Fq "STALE_REJECTED:$SEREA_CONSENT_DEATH_RUN_ID:request=request-B:generation=1:session=session-B:" "$evidence/result-delivery-logcat.txt" || fail 'delivered Activity Result did not leave B unchanged'
  if grep -Fq "RESULT_DELIVERED:$SEREA_CONSENT_DEATH_RUN_ID:request=request-A:generation=1:session=session-A:code=-1:" <<< "$callback_line"; then
    callback_status=valid-A-RESULT_OK-delivered-after-kill-and-sender-completion
    valid_a_result=true
  else
    callback_status=callback-delivered-with-nonmatching-identity-or-result-code
  fi
fi
printf '%s\n' "$callback_status" > "$evidence/activity-result-callback.txt"
if [[ "$valid_a_result" == true ]]; then
  delivery=delivered-valid-A-result-after-kill-and-sender-completion
else
  # No valid A Activity Result was observed. Exercise B's stale-result guard through a separate
  # test-only host injection after the sender is known to have completed; preserve callback status.
  "${adb_cmd[@]}" shell am broadcast --receiver-include-background \
    -n "$package/app.serea.android.googleauth.PrivateGoogleInFlightDeathReplayReceiver" \
    -a app.serea.android.action.REPLAY_OLD_CONSENT_RESULT \
    --es token "$SEREA_CONSENT_DEATH_RUN_ID" > "$evidence/replay-old-A.txt" 2>&1 || fail 'host could not inject old A result into request B after sender completion'
  for _ in $(seq 1 30); do
    "${adb_cmd[@]}" logcat -d -s SereaConsentDeath:I > "$evidence/recovery-logcat.txt"
    grep -Fq "STALE_REPLAY_REJECTED:$SEREA_CONSENT_DEATH_RUN_ID:request=request-B:generation=1:session=session-B:" "$evidence/recovery-logcat.txt" && break
    sleep 1
  done
  grep -Fq "STALE_REPLAY_REJECTED:$SEREA_CONSENT_DEATH_RUN_ID:request=request-B:generation=1:session=session-B:" "$evidence/recovery-logcat.txt" || fail 'request B did not reject the post-completion stale A replay'
  delivery="$callback_status; post-completion-old-identity-replay-rejected"
fi
printf '%s\n' "$delivery" > "$evidence/activity-result-delivery.txt"
"${adb_cmd[@]}" shell dumpsys activity activities > "$evidence/activities-after-restart.txt"
"${adb_cmd[@]}" shell dumpsys window windows > "$evidence/windows-after-restart.txt"

test_pid_after="$(pid_for "$test_package")"
printf 'external_sender_pid_after_result=%s\n' "${test_pid_after:-absent}" >> "$evidence/provenance.txt"
[[ "$test_pid_before" == "$test_pid_after" ]] || fail 'external sender/instrumentation package process changed during target process death check'
target_pid_after_restart="$(pid_for "$package")"
[[ -n "$target_pid_after_restart" && "$target_pid_after_restart" != "$target_pid_before" ]] || fail 'target process did not return with a new PID'
printf 'target_pid_after_restart=%s\n' "$target_pid_after_restart" >> "$evidence/provenance.txt"
printf 'PASS\n' > "$evidence/status.txt"
printf 'PASS: API %s ActivityManager force-stop process death during real Private Consent Relay; ActivityResult=%s; stale result protected against B. Evidence: %s\n' \
  "$sdk" "$delivery" "$evidence"
