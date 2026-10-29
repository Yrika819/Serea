#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/assert_disposable_android_emulator.sh"

: "${EXPECTED_API_LEVEL:=${SEREA_EXPECTED_ANDROID_API:-}}"
: "${SEREA_ISOLATED_INSTRUMENTATION_FINISHED:?Set to 1 only after the isolated instrumentation command has exited.}"
: "${SEREA_FAKE_GOOGLE_DEATH_TOKEN:?Provide a unique token for this disposable process-death run.}"
test "$SEREA_ISOLATED_INSTRUMENTATION_FINISHED" = 1
[[ "$SEREA_FAKE_GOOGLE_DEATH_TOKEN" =~ ^[A-Za-z0-9-]{1,120}$ ]]

package="app.serea.android.isolated"
test_package="app.serea.android.isolated.test"
activity="$package/.FakeGoogleProcessDeathHarnessActivity"
tag="SereaFakeGoogleDeath"
token="$SEREA_FAKE_GOOGLE_DEATH_TOKEN"
evidence="${SEREA_FAKE_GOOGLE_DEATH_EVIDENCE_DIR:-$(dirname "$0")/../android/app/build/outputs/androidTest-results/api${EXPECTED_API_LEVEL}/fake-google-process-death}"
mkdir -p "$evidence"
printf 'NOT_RUN\n' > "$evidence/status.txt"

adb_cmd=(adb -s "$ANDROID_SERIAL")
test -n "$("${adb_cmd[@]}" shell pm path "$package" | tr -d '\r')"

# The app process also hosts the AndroidJUnitRunner in this variant. Refuse to stop it while a
# test process remains; the host script is called only after Gradle/instrumentation has returned.
test_pid="$("${adb_cmd[@]}" shell pidof "$test_package" 2>/dev/null | tr -d '\r' || true)"
if [[ -n "$test_pid" ]]; then
  printf 'FAIL: isolated instrumentation process is still present: %s\n' "$test_pid" | tee "$evidence/status.txt" >&2
  exit 1
fi
{
  printf 'api=%s\nserial=%s\ntarget_package=%s\ninstrumentation_package=%s\ninstrumentation_process_before=%s\ntoken=%s\n' \
    "$EXPECTED_API_LEVEL" "$ANDROID_SERIAL" "$package" "$test_package" "${test_pid:-absent}" "$token"
} > "$evidence/host-guard.txt"

pid_of_target() {
  "${adb_cmd[@]}" shell pidof "$package" 2>/dev/null | tr -d '\r' || true
}

wait_target_stopped() {
  local observed
  for _ in $(seq 1 30); do
    observed="$(pid_of_target)"
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
  "${adb_cmd[@]}" shell am start -W -f 0x10008000 -n "$activity" \
    --es phase "$phase" --es token "$token" > "$evidence/$phase-am-start.txt" 2>&1
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
run_phase seed
seed_pid="$(pid_of_target)"
test -n "$seed_pid"
printf '%s\n' "$seed_pid" > "$evidence/seed-pid.txt"

printf 'Force-stopping only %s after instrumentation has exited.\n' "$package"
"${adb_cmd[@]}" shell am force-stop "$package"
wait_target_stopped
printf 'absent\n' > "$evidence/post-force-stop-pid.txt"

test_pid="$("${adb_cmd[@]}" shell pidof "$test_package" 2>/dev/null | tr -d '\r' || true)"
if [[ -n "$test_pid" ]]; then
  printf 'FAIL: isolated instrumentation process appeared during target process-death check: %s\n' "$test_pid" | tee "$evidence/status.txt" >&2
  exit 1
fi

printf 'Relaunching a fresh Activity and checking in-memory auth/session state.\n'
run_phase verify
verify_pid="$(pid_of_target)"
test -n "$verify_pid"
printf '%s\n' "$verify_pid" > "$evidence/verify-pid.txt"

printf 'PASS\n' > "$evidence/status.txt"
printf 'PASS: Fake Google consent/session OS process-death boundary on API %s.\n' "$EXPECTED_API_LEVEL"
printf 'Evidence: %s\n' "$evidence"
