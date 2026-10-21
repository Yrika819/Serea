#!/usr/bin/env bash
set -euo pipefail

token="${1:?sentinel token required}"
phase="${2:?phase must be seed or verify}"
if [[ ! "$token" =~ ^[A-Za-z0-9-]{1,120}$ || "$phase" != seed && "$phase" != verify ]]; then
  echo "Invalid data sentinel arguments." >&2
  exit 2
fi

source "$(dirname "$0")/assert_disposable_android_emulator.sh"
test "$(adb shell getprop ro.build.version.sdk | tr -d '\r')" = "${EXPECTED_API_LEVEL:?EXPECTED_API_LEVEL must be set}"
package='app.serea.android.demo'
receiver="$package/app.serea.android.ProcessDeathHarnessReceiver"
action='app.serea.android.action.PROCESS_DEATH_HARNESS'
request_id="sentinel-${BASHPID}-$(date +%s%N)-${RANDOM}"
test -n "$(adb shell pm path "$package" | tr -d '\r')"

# A freshly installed app is in Android's stopped state. Explicitly include it
# so the receiver can perform the read-only/guarded sentinel phase before any
# Activity has been launched.
adb shell am broadcast -f 0x20 -n "$receiver" -a "$action" \
  --es phase "${phase}-sentinel" --es token "$token" --es requestId "$request_id"
for _ in $(seq 1 30); do
  result_log=$(adb logcat -d -s SereaProcessDeath:V)
  if grep -F "FAIL:$token:${phase}-sentinel:$request_id:" <<< "$result_log" >/dev/null; then
    printf '%s\n' "$result_log" >&2
    exit 1
  fi
  if grep -F "PASS:$token:${phase}-sentinel:$request_id" <<< "$result_log" >/dev/null; then
    echo "PASS: demo package history and model_selection sentinel $phase verified; no demo history mutation was requested."
    exit 0
  fi
  sleep 1
done
echo "Sentinel phase $phase did not report completion." >&2
adb logcat -d -s SereaProcessDeath:V >&2
exit 1
