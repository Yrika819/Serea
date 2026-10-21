#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/assert_disposable_android_emulator.sh"
test "$(adb shell getprop ro.build.version.sdk | tr -d '\r')" = "${EXPECTED_API_LEVEL:?EXPECTED_API_LEVEL must be set}"

package="app.serea.android.isolated"
demo_package="app.serea.android.demo"
activity="$package/app.serea.android.MainActivity"
receiver="$package/app.serea.android.ProcessDeathHarnessReceiver"
action="app.serea.android.action.PROCESS_DEATH_HARNESS"
campaign_token="${SEREA_CAMPAIGN_TOKEN:?SEREA_CAMPAIGN_TOKEN must identify the fresh isolated campaign}"
run_token="$campaign_token"
sentinel_token="${SEREA_SENTINEL_TOKEN:?SEREA_SENTINEL_TOKEN must identify the debug data sentinel}"

test -n "$(adb shell pm path "$demo_package" | tr -d '\r')"
test -n "$(adb shell pm path "$package" | tr -d '\r')"
test -n "$(adb shell pm path "${package}.test" | tr -d '\r')"
EXPECTED_API_LEVEL="$EXPECTED_API_LEVEL" bash "$(dirname "$0")/run_android_data_sentinel.sh" "$sentinel_token" verify
before_path=$(adb shell pm path "$package" | tr -d '\r')
demo_before_path=$(adb shell pm path "$demo_package" | tr -d '\r')
test -n "$before_path"

phase() {
  local phase_name="$1"
  local phase_token="${2:-$run_token}"
  local request_id="process-${BASHPID}-$(date +%s%N)-${RANDOM}"
  echo "Process-death harness: starting phase $phase_name"
  adb shell am broadcast -f 0x20 -n "$receiver" -a "$action" \
    --es phase "$phase_name" --es token "$phase_token" --es requestId "$request_id"
  for _ in $(seq 1 30); do
    local result_log
    result_log=$(adb logcat -d -s SereaProcessDeath:V)
    if grep -F "FAIL:$phase_token:$phase_name:$request_id:" <<< "$result_log" >/dev/null; then
      printf '%s\n' "$result_log" >&2
      return 1
    fi
    if grep -F "PASS:$phase_token:$phase_name:$request_id" <<< "$result_log" >/dev/null; then return 0; fi
    sleep 1
  done
  echo "Process-death harness phase $phase_name did not report completion." >&2
  adb logcat -d -s SereaProcessDeath:V >&2
  return 1
}

verify_database_file() {
  local listing
  listing=$(adb shell run-as "$package" ls -l databases | tr -d '\r')
  printf '%s\n' "$listing"
  printf '%s\n' "$listing" | awk '$5 ~ /^[0-9]+$/ && $5 > 0 && ($NF == "serea-history.db" || $NF == "serea-tasks-process-death-harness.db") { found[$NF]=1 } END { exit !(found["serea-history.db"] && found["serea-tasks-process-death-harness.db"]) }'
}

phase campaign-consume "$campaign_token"
phase prepare
echo "Process-death harness: checking prepared encrypted database"
verify_database_file
echo "Process-death harness: force-stopping target app without clearing data"
adb shell am force-stop "$package"
after_path=$(adb shell pm path "$package" | tr -d '\r')
test "$before_path" = "$after_path"
verify_database_file
adb shell am start -W -n "$activity" >/dev/null
sleep 2
phase verify

# Verify Delete All survives a second real force-stop/relaunch boundary.
phase delete-all
echo "Process-death harness: force-stopping after Delete All"
adb shell am force-stop "$package"
adb shell am start -W -n "$activity" >/dev/null
sleep 2
phase verify-empty

EXPECTED_API_LEVEL="$EXPECTED_API_LEVEL" bash "$(dirname "$0")/run_android_data_sentinel.sh" "$sentinel_token" verify
after_path=$(adb shell pm path "$package" | tr -d '\r')
demo_after_path=$(adb shell pm path "$demo_package" | tr -d '\r')
test "$before_path" = "$after_path"
test "$demo_before_path" = "$demo_after_path"
echo "PASS: isolated Android OS Process Death recovery on API ${EXPECTED_API_LEVEL} (encrypted history, Usage Ledger OUTCOME_UNKNOWN, recovery without replay, deletion, isolated package preserved, demo sentinel preserved)."
