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
printf 'NOT_RUN\n' > "$evidence/activity-result-delivery.txt"
printf 'PrivateGoogleConsentRelayTestActivity.StartIntentSenderForResult -> SyntheticConsentSenderActivity -> PrivateGoogleInFlightDeathHarnessActivity.ActivityResultLauncher\n' > "$evidence/activity-result-route.txt"
adb_cmd=(adb -s "$ANDROID_SERIAL")

fail() {
  printf 'FAIL\n' > "$evidence/status.txt"
  if [[ -f "$evidence/result-delivery-logcat.txt" ]] &&
      grep -Fq "RESULT_DELIVERED:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/result-delivery-logcat.txt" &&
      [[ "$(cat "$evidence/activity-result-delivery.txt")" == NOT_RUN ]]; then
    printf 'FAIL\n' > "$evidence/activity-result-delivery.txt"
  fi
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
  if grep -F "IN_FLIGHT:$SEREA_CONSENT_DEATH_RUN_ID:request=request-A:generation=1:session=session-A:pid=" "$evidence/in-flight-logcat.txt" >/dev/null; then break; fi
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
sender_lifecycle_line="$(grep -F "SENDER_CREATED:$SEREA_CONSENT_DEATH_RUN_ID:pid=$test_pid_before:instance=" "$evidence/sender-logcat.txt" | tail -n 1 || true)"
[[ -n "$sender_lifecycle_line" ]] || fail 'sender creation event with its PID and instance nonce was missing'
sender_instance_nonce="${sender_lifecycle_line##*:instance=}"
[[ "$sender_instance_nonce" =~ ^[A-Fa-f0-9-]{36}$ ]] || fail 'sender instance nonce was invalid'
sender_created_count="$(grep -Fc "SENDER_CREATED:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/sender-logcat.txt" || true)"
[[ "$sender_created_count" == 1 ]] || fail "synthetic sender was created $sender_created_count times before target kill"
printf 'external_sender_pid_before_target_kill=%s\n' "$test_pid_before" >> "$evidence/provenance.txt"
printf 'sender_instance_nonce=%s\n' "$sender_instance_nonce" >> "$evidence/provenance.txt"
grep -E "SENDER_(CREATED|RESUMED|IN_FLIGHT):$SEREA_CONSENT_DEATH_RUN_ID:pid=$test_pid_before:instance=$sender_instance_nonce" "$evidence/sender-logcat.txt" > "$evidence/lifecycle-events.txt"
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

# `am kill` only terminates cached/background processes and can refuse a process whose task
# contains the visible relay. On this disposable userdebug AVD, restart adbd as root and SIGKILL
# only the target PID. This preserves OS task/result records and leaves the sender Activity and
# its separate process alive, allowing Android's saved ActivityResult route to be exercised.
"${adb_cmd[@]}" root > "$evidence/adb-root.txt" 2>&1 || fail 'could not obtain root shell on disposable emulator'
grep -Fq 'restarting adbd as root' "$evidence/adb-root.txt" || grep -Fq 'already running as root' "$evidence/adb-root.txt" || fail 'adb root did not confirm root shell'
"${adb_cmd[@]}" shell id > "$evidence/adb-root-id.txt" 2>&1
grep -Fq 'uid=0(root)' "$evidence/adb-root-id.txt" || fail 'adb shell is not root on disposable emulator'
target_process_record="$("${adb_cmd[@]}" shell ps -A -o PID,NAME | awk -v pid="$target_pid_before" 'NR > 1 && $1 == pid { print $1 " " $2 }' | tr -d '\r')"
[[ "$target_process_record" == "$target_pid_before $package" ]] || fail 'target PID or process name changed immediately before SIGKILL'
printf 'SIGKILL_PRECHECK:%s:pid=%s:process=%s\n' "$SEREA_CONSENT_DEATH_RUN_ID" "$target_pid_before" "$package" > "$evidence/target-sigkill-precheck.txt"
"${adb_cmd[@]}" shell kill -9 "$target_pid_before" > "$evidence/target-sigkill.txt" 2>&1 || fail 'OS SIGKILL failed for the target PID'
printf 'target_termination_method=adb root then kill -9 %s\n' "$target_pid_before" >> "$evidence/provenance.txt"
target_pid_after='pending'
for _ in $(seq 1 30); do
  target_pid_after="$(pid_for "$package")"
  [[ -z "$target_pid_after" || "$target_pid_after" != "$target_pid_before" ]] && break
  sleep 1
done
[[ -z "$target_pid_after" || "$target_pid_after" != "$target_pid_before" ]] || fail "target process survived SIGKILL with PID $target_pid_after"
test_pid_after_kill="$(pid_for "$test_package")"
[[ "$test_pid_before" == "$test_pid_after_kill" ]] || fail 'external sender/instrumentation package process was terminated with the target'
printf 'target_pid_after_SIGKILL=%s\n' "${target_pid_after:-absent}" >> "$evidence/provenance.txt"
printf 'external_sender_pid_after_target_death=%s\n' "$test_pid_after_kill" >> "$evidence/provenance.txt"
printf 'TARGET_SIGKILL:%s:pid=%s:sender_pid=%s:instance=%s\n' "$SEREA_CONSENT_DEATH_RUN_ID" "$target_pid_before" "$test_pid_after_kill" "$sender_instance_nonce" >> "$evidence/lifecycle-events.txt"
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
  if grep -F "B_READY:$SEREA_CONSENT_DEATH_RUN_ID:request=request-B:generation=1:session=session-B:status=CONSENT_PENDING:" "$evidence/recovery-logcat.txt" >/dev/null; then break; fi
  sleep 1
done
grep -Fq "B_READY:$SEREA_CONSENT_DEATH_RUN_ID:request=request-B:generation=1:session=session-B:status=CONSENT_PENDING:" "$evidence/recovery-logcat.txt" || fail 'request B did not enter its fresh pending authorization state'
grep -Fq "RECOVERED_DISCONNECTED:$SEREA_CONSENT_DEATH_RUN_ID:status=DISCONNECTED:pending=null:calendarSession=null:" "$evidence/recovery-logcat.txt" || fail 'authorization state was restored after process death instead of starting disconnected'
grep -Fq "B_READY:$SEREA_CONSENT_DEATH_RUN_ID:request=request-B:generation=1:session=session-B:status=CONSENT_PENDING:" "$evidence/recovery-logcat.txt" || fail 'request B identity or pending state was not recorded'
grep -Fq 'factory=1:authorize=1:result=0:cancel=0:token=0' "$evidence/recovery-logcat.txt" || fail 'request B had unexpected pre-result gateway operations'
grep -F "B_READY:$SEREA_CONSENT_DEATH_RUN_ID:request=request-B:generation=1:session=session-B:" "$evidence/recovery-logcat.txt" >> "$evidence/lifecycle-events.txt"
target_pid_after_restart="$(pid_for "$package")"
[[ -n "$target_pid_after_restart" && "$target_pid_after_restart" != "$target_pid_before" ]] || fail 'request B did not run in a new target process'
printf 'target_pid_after_restart=%s\n' "$target_pid_after_restart" >> "$evidence/provenance.txt"
printf 'TARGET_RESTARTED:%s:pid=%s:sender_pid=%s:instance=%s\n' "$SEREA_CONSENT_DEATH_RUN_ID" "$target_pid_after_restart" "$test_pid_after_kill" "$sender_instance_nonce" >> "$evidence/lifecycle-events.txt"
printf 'B_READY:%s:pid=%s:sender_pid=%s:instance=%s:request=request-B:generation=1:session=session-B\n' "$SEREA_CONSENT_DEATH_RUN_ID" "$target_pid_after_restart" "$test_pid_after_kill" "$sender_instance_nonce" >> "$evidence/lifecycle-events.txt"
# Complete the original sender only after B is pending, allowing the OS to exercise A's saved
# ActivityResult route against the new request.
printf 'SENDER_COMPLETION_REQUESTED:%s:pid=%s:instance=%s\n' "$SEREA_CONSENT_DEATH_RUN_ID" "$test_pid_before" "$sender_instance_nonce" >> "$evidence/lifecycle-events.txt"
"${adb_cmd[@]}" shell am broadcast --receiver-foreground \
  -n "$test_package/app.serea.android.googleauth.SyntheticConsentSenderCompletionReceiver" \
  -a app.serea.android.test.action.COMPLETE_SYNTHETIC_CONSENT \
  --es token "$SEREA_CONSENT_DEATH_RUN_ID" --ei targetPid "$test_pid_before" \
  --es instanceNonce "$sender_instance_nonce" > "$evidence/complete-old-sender.txt" 2>&1 || fail 'could not request completion of the original external Consent A'
grep -Fq 'Broadcast completed' "$evidence/complete-old-sender.txt" || fail 'Android did not dispatch the sender completion broadcast'
grep -Fq "COMPLETION_BROADCAST_ACCEPTED:$SEREA_CONSENT_DEATH_RUN_ID:pid=$test_pid_before:instance=$sender_instance_nonce" <("${adb_cmd[@]}" logcat -d -s SereaSyntheticConsent:I) || fail 'test-only completion receiver did not accept the verified original sender'
"${adb_cmd[@]}" shell am broadcast --receiver-foreground \
  -n "$test_package/app.serea.android.googleauth.SyntheticConsentSenderCompletionReceiver" \
  -a app.serea.android.test.action.COMPLETE_SYNTHETIC_CONSENT \
  --es token "$SEREA_CONSENT_DEATH_RUN_ID" --ei targetPid "$test_pid_before" \
  --es instanceNonce "$sender_instance_nonce" > "$evidence/duplicate-sender-completion.txt" 2>&1 || fail 'duplicate sender completion probe could not be dispatched'
for _ in $(seq 1 10); do
  "${adb_cmd[@]}" logcat -d -s SereaSyntheticConsent:I > "$evidence/duplicate-completion-logcat.txt"
  grep -Fq "SENDER_COMPLETION_REJECTED:$SEREA_CONSENT_DEATH_RUN_ID:pid=$test_pid_before:instance=$sender_instance_nonce" "$evidence/duplicate-completion-logcat.txt" && break
  sleep 1
done
grep -Fq "SENDER_COMPLETION_REJECTED:$SEREA_CONSENT_DEATH_RUN_ID:pid=$test_pid_before:instance=$sender_instance_nonce" "$evidence/duplicate-completion-logcat.txt" || fail 'duplicate completion request was not rejected'
grep -F "SENDER_COMPLETION_REJECTED:$SEREA_CONSENT_DEATH_RUN_ID:pid=$test_pid_before:instance=$sender_instance_nonce" "$evidence/duplicate-completion-logcat.txt" >> "$evidence/lifecycle-events.txt"
for _ in $(seq 1 30); do
  "${adb_cmd[@]}" logcat -d -s SereaConsentDeath:I SereaSyntheticConsent:I > "$evidence/result-delivery-logcat.txt"
  grep -Fq "SENDER_COMPLETED:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/result-delivery-logcat.txt" && break
  current_target_pid="$(pid_for "$package")"
  [[ "$current_target_pid" == "$target_pid_after_restart" ]] || fail 'B target process did not remain observable during the Activity Result window'
  sleep 1
done
sender_launch_count="$(grep -Fc "SENDER_IN_FLIGHT:$SEREA_CONSENT_DEATH_RUN_ID:pid=" "$evidence/result-delivery-logcat.txt" || true)"
[[ "$sender_launch_count" == 1 ]] || fail "synthetic Consent sender launched $sender_launch_count times"
relay_launch_count="$(grep -Fc "IN_FLIGHT:$SEREA_CONSENT_DEATH_RUN_ID:request=request-A:generation=1:session=session-A:" "$evidence/result-delivery-logcat.txt" || true)"
[[ "$relay_launch_count" == 1 ]] || fail "Consent Relay pending IntentSender was launched $relay_launch_count times"
if grep -Fq "SENDER_UNEXPECTED_NEW_INTENT:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/result-delivery-logcat.txt"; then
  fail 'original Sender received an unexpected ActivityManager reactivation Intent'
fi
sender_created_count="$(grep -Fc "SENDER_CREATED:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/result-delivery-logcat.txt" || true)"
[[ "$sender_created_count" == 1 ]] || fail "synthetic Consent sender onCreate ran $sender_created_count times"
sender_completion_count="$(grep -Fc "SENDER_COMPLETED:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/result-delivery-logcat.txt" || true)"
[[ "$sender_completion_count" == 1 ]] || fail "synthetic Consent sender completed $sender_completion_count times"
grep -Fq "SENDER_COMPLETION_REQUESTED:$SEREA_CONSENT_DEATH_RUN_ID:pid=$test_pid_before:instance=$sender_instance_nonce" "$evidence/result-delivery-logcat.txt" || fail 'original sender instance did not accept the authenticated completion request'
grep -Fq "SENDER_COMPLETED:$SEREA_CONSENT_DEATH_RUN_ID:pid=$test_pid_before:instance=$sender_instance_nonce:code=-1:payload=synthetic-consent-result-A" "$evidence/result-delivery-logcat.txt" || fail 'original sender did not preserve A result code and payload'
grep -F "COMPLETION_BROADCAST_ACCEPTED:$SEREA_CONSENT_DEATH_RUN_ID:pid=$test_pid_before:instance=$sender_instance_nonce" "$evidence/result-delivery-logcat.txt" >> "$evidence/lifecycle-events.txt"
grep -F "SENDER_COMPLETION_REQUESTED:$SEREA_CONSENT_DEATH_RUN_ID:pid=$test_pid_before:instance=$sender_instance_nonce" "$evidence/result-delivery-logcat.txt" >> "$evidence/lifecycle-events.txt"
grep -F "SENDER_COMPLETED:$SEREA_CONSENT_DEATH_RUN_ID:pid=$test_pid_before:instance=$sender_instance_nonce" "$evidence/result-delivery-logcat.txt" >> "$evidence/lifecycle-events.txt"
grep -Fq "SENDER_COMPLETED:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/result-delivery-logcat.txt" || fail 'external Consent A did not complete through its original Activity instance'
for _ in $(seq 1 30); do
  "${adb_cmd[@]}" logcat -d -s SereaConsentDeath:I SereaSyntheticConsent:I > "$evidence/result-delivery-logcat.txt"
  if grep -F "RESULT_DELIVERED:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/result-delivery-logcat.txt" >/dev/null; then break; fi
  current_target_pid="$(pid_for "$package")"
  [[ "$current_target_pid" == "$target_pid_after_restart" ]] || fail 'B target process did not remain alive during the Activity Result callback observation window'
  sleep 1
done
callback_line="$(grep -F "RESULT_DELIVERED:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/result-delivery-logcat.txt" | tail -n 1 || true)"
callback_count="$(grep -Fc "RESULT_DELIVERED:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/result-delivery-logcat.txt" || true)"
[[ "$callback_count" -le 1 ]] || fail "Activity Result callback was delivered $callback_count times"
grep -F "RESULT_DELIVERED:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/result-delivery-logcat.txt" >> "$evidence/lifecycle-events.txt" || true
grep -F "STALE_REJECTED:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/result-delivery-logcat.txt" >> "$evidence/lifecycle-events.txt" || true
if [[ -n "$callback_line" ]]; then
  printf 'RESULT_DELIVERED:%s:target_pid=%s:sender_pid=%s:instance=%s:identity=request-A/generation-1/session-A/code-OK/payload-A\n' "$SEREA_CONSENT_DEATH_RUN_ID" "$target_pid_after_restart" "$test_pid_before" "$sender_instance_nonce" >> "$evidence/lifecycle-events.txt"
  printf 'STALE_REJECTED:%s:target_pid=%s:sender_pid=%s:instance=%s:B=request-B/generation-1/session-B/status=CONSENT_PENDING\n' "$SEREA_CONSENT_DEATH_RUN_ID" "$target_pid_after_restart" "$test_pid_before" "$sender_instance_nonce" >> "$evidence/lifecycle-events.txt"
fi
delivery=DELIVERY_UNKNOWN
if [[ -n "$callback_line" ]]; then
  grep -Fq "RESULT_DELIVERED:$SEREA_CONSENT_DEATH_RUN_ID:request=request-A:generation=1:session=session-A:code=-1:payload=synthetic-consent-result-A:" <<< "$callback_line" || fail 'delivered Activity Result did not preserve original A identity, result code, and payload'
  grep -Fq "STALE_REJECTED:$SEREA_CONSENT_DEATH_RUN_ID:request=request-B:generation=1:session=session-B:status=CONSENT_PENDING:" "$evidence/result-delivery-logcat.txt" || fail 'delivered Activity Result did not leave B pending and unchanged'
  grep -Fq 'authorize=1:result=0:cancel=0:token=0' "$evidence/result-delivery-logcat.txt" || fail 'old Activity Result caused unexpected authorization, cancellation, result, or token calls'
  delivery=DELIVERED_REJECTED
  printf 'DELIVERED_REJECTED\n' > "$evidence/activity-result-callback.txt"
  printf '%s\n' "$delivery" > "$evidence/activity-result-delivery.txt"
else
  # Only classify non-delivery when the saved A harness was actually restored and resumed after
  # the Sender finished; absence of a callback without that lifecycle proof is unknown.
  completion_line="$(grep -Fn "SENDER_COMPLETED:$SEREA_CONSENT_DEATH_RUN_ID:pid=$test_pid_before:instance=$sender_instance_nonce" "$evidence/result-delivery-logcat.txt" | tail -n 1 | cut -d: -f1)"
  recreated_line="$(grep -Fn "A_RECREATED_NO_RELAUNCH:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/result-delivery-logcat.txt" | tail -n 1 | cut -d: -f1 || true)"
  resumed_line="$(grep -Fn "HARNESS_RESUMED:$SEREA_CONSENT_DEATH_RUN_ID:phase=in-flight-A:" "$evidence/result-delivery-logcat.txt" | tail -n 1 | cut -d: -f1 || true)"
  if [[ -n "$completion_line" && -n "$recreated_line" && -n "$resumed_line" && "$recreated_line" -gt "$completion_line" && "$resumed_line" -gt "$completion_line" ]]; then
    delivery=NOT_DELIVERED
    printf 'NOT_DELIVERED: A harness restored and resumed after Sender completion; no callback in 30s while target PID %s remained alive\n' "$target_pid_after_restart" > "$evidence/activity-result-callback.txt"
    printf '%s\n' "$delivery" > "$evidence/activity-result-delivery.txt"
  else
    delivery=DELIVERY_UNKNOWN
    printf 'DELIVERY_UNKNOWN: no callback in 30s, but A restore/resume after Sender completion was not proven\n' > "$evidence/activity-result-callback.txt"
    printf '%s\n' "$delivery" > "$evidence/activity-result-delivery.txt"
    fail 'Activity Result delivery remained unknown after the bounded observation window'
  fi
fi
if [[ "$delivery" == NOT_DELIVERED ]]; then
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
  grep -Fq "STALE_REPLAY_REJECTED:$SEREA_CONSENT_DEATH_RUN_ID:request=request-B:generation=1:session=session-B:status=CONSENT_PENDING:" "$evidence/recovery-logcat.txt" || fail 'old identity replay changed B pending state'
  grep -Fq 'authorize=1:result=0:cancel=0:token=0' "$evidence/recovery-logcat.txt" || fail 'old identity replay caused unexpected authorization, cancellation, result, or token calls'
  printf 'synthetic_old_identity_replay=REJECTED;activity_result_delivery=NOT_DELIVERED\n' > "$evidence/stale-replay-classification.txt"
fi
printf '%s\n' "$delivery" > "$evidence/activity-result-delivery.txt"
"${adb_cmd[@]}" shell am start -W -f 0x18000000 -n "$component" \
  --es phase complete-B --es token "$SEREA_CONSENT_DEATH_RUN_ID" > "$evidence/complete-B-synthetic-result.txt" 2>&1 || fail 'could not reactivate request B harness for its independent synthetic Consent result'
grep -Fxq 'Status: ok' "$evidence/complete-B-synthetic-result.txt" || fail 'ActivityManager did not start the request B result verifier'
for _ in $(seq 1 30); do
  "${adb_cmd[@]}" logcat -d -s SereaConsentDeath:I SereaSyntheticConsent:I > "$evidence/B-valid-result-logcat.txt"
  grep -Fq "B_VALID_RESULT_DELIVERED:$SEREA_CONSENT_DEATH_RUN_ID:request=request-B:generation=1:session=session-B:status=AUTHORIZED:" "$evidence/B-valid-result-logcat.txt" && break
  sleep 1
done
grep -Fq "B_SYNTHETIC_RESULT_CREATED:$SEREA_CONSENT_DEATH_RUN_ID-B:" "$evidence/B-valid-result-logcat.txt" || fail 'independent B Synthetic Result Activity was not created'
grep -Fq "B_VALID_RESULT_DELIVERED:$SEREA_CONSENT_DEATH_RUN_ID:request=request-B:generation=1:session=session-B:status=AUTHORIZED:" "$evidence/B-valid-result-logcat.txt" || fail 'request B did not accept its own synthetic result through the Android Activity Result route'
grep -Fq 'resultCalls=1:cancel=0:token=0:googleTransport=not-constructed:calendarSession=absent' "$evidence/B-valid-result-logcat.txt" || fail 'B synthetic result accessed a token or Calendar session, or had unexpected gateway operations'
grep -F "B_VALID_RESULT_DELIVERED:$SEREA_CONSENT_DEATH_RUN_ID:" "$evidence/B-valid-result-logcat.txt" >> "$evidence/lifecycle-events.txt"
printf 'B_VALID_RESULT_DELIVERED:%s:target_pid=%s:sender_pid=%s:instance=%s:B=request-B/generation-1/session-B/status=AUTHORIZED\n' "$SEREA_CONSENT_DEATH_RUN_ID" "$target_pid_after_restart" "$test_pid_before" "$sender_instance_nonce" >> "$evidence/lifecycle-events.txt"
"${adb_cmd[@]}" shell dumpsys activity activities > "$evidence/activities-after-restart.txt"
"${adb_cmd[@]}" shell dumpsys window windows > "$evidence/windows-after-restart.txt"

test_pid_after="$(pid_for "$test_package")"
printf 'external_sender_pid_after_result=%s\n' "${test_pid_after:-absent}" >> "$evidence/provenance.txt"
if [[ "$test_pid_after" == "$test_pid_before" ]]; then
  printf 'sender_process_after_completion=original-pid-retained:%s\n' "$test_pid_after" >> "$evidence/provenance.txt"
elif [[ -z "$test_pid_after" ]]; then
  printf 'sender_process_after_completion=process-exited-after-original-Activity-finished\n' >> "$evidence/provenance.txt"
else
  # The original Activity's completion already carries its verified PID and nonce. Android may
  # reclaim that now-empty process after finish; record a later process separately instead of
  # claiming that a newly launched receiver belongs to the original Sender instance.
  printf 'sender_process_after_completion=other-process:%s;original_sender_pid=%s\n' "$test_pid_after" "$test_pid_before" >> "$evidence/provenance.txt"
fi
target_pid_final="$(pid_for "$package")"
[[ "$target_pid_final" == "$target_pid_after_restart" ]] || fail 'target process changed after B request verification'
printf 'PASS\n' > "$evidence/status.txt"
printf 'PASS: API %s target-process SIGKILL during real Private Consent Relay; ActivityResult=%s; stale result protected against B. Evidence: %s\n' \
  "$sdk" "$delivery" "$evidence"
