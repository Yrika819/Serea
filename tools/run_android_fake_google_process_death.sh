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
activity_class="app.serea.android.FakeGoogleProcessDeathHarnessActivity"
activity="$package/$activity_class"
tag="SereaFakeGoogleDeath"
token="$SEREA_FAKE_GOOGLE_DEATH_TOKEN"
evidence="${SEREA_FAKE_GOOGLE_DEATH_EVIDENCE_DIR:-$(dirname "$0")/../android/app/build/outputs/androidTest-results/api${EXPECTED_API_LEVEL}/fake-google-process-death}"
mkdir -p "$evidence"
printf 'NOT_RUN\n' > "$evidence/status.txt"

adb_cmd=(adb -s "$ANDROID_SERIAL")
if ! package_path="$("${adb_cmd[@]}" shell pm path "$package")"; then
  printf 'FAIL\n' > "$evidence/status.txt"
  echo 'Could not verify the isolated target package path.' >&2
  exit 1
fi
package_path="${package_path//$'\r'/}"
if [[ -z "$package_path" ]]; then
  printf 'FAIL\n' > "$evidence/status.txt"
  echo 'The isolated target package is not installed.' >&2
  exit 1
fi

query_package_pid() {
  local target_package="$1" process_table matching_pids
  # Use one remote command and parse the named PID/NAME columns. Passing a multiline script
  # through the remote `sh -c` command is not reliable: Android's shell transport can split it
  # and positional arguments, turning a valid process check into an ambiguous command failure.
  if ! process_table="$("${adb_cmd[@]}" shell ps -A -o PID,NAME 2>&1)"; then
    printf 'Process table query failed while checking %s: %s\n' "$target_package" "$process_table" >&2
    return 1
  fi
  process_table="${process_table//$'\r'/}"
  if ! matching_pids="$(printf '%s\n' "$process_table" | awk -v target="$target_package" '
    NR == 1 {
      for (i = 1; i <= NF; i++) {
        if ($i == "PID") {
          if (pid_column) exit 2
          pid_column = i
        }
        if ($i == "NAME") {
          if (name_column) exit 2
          name_column = i
        }
      }
      if (!pid_column || !name_column || pid_column == name_column) exit 2
      next
    }
    {
      if (NF < pid_column || NF < name_column || $pid_column !~ /^[0-9]+$/) exit 2
      process_rows++
      if ($name_column == target) {
        print $pid_column
      }
    }
    END { if (!pid_column || !name_column || !process_rows) exit 2 }
  ')"; then
    printf 'Process table was malformed while checking %s.\n' "$target_package" >&2
    return 1
  fi
  if [[ -z "$matching_pids" ]]; then
    return 0
  fi
  if [[ "$matching_pids" =~ ^[0-9]+([[:space:]]+[0-9]+)*$ ]]; then
    printf '%s' "$matching_pids"
    return 0
  fi
  printf 'Process table returned malformed PID data for %s.\n' "$target_package" >&2
  return 1
}

fail_campaign() {
  printf 'FAIL\n' > "$evidence/status.txt"
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
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
