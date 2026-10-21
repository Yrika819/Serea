#!/usr/bin/env bash
set -euo pipefail

token="${1:?campaign token required}"
if [[ ! "$token" =~ ^[A-Za-z0-9-]{1,120}$ ]]; then
  echo "Invalid instrumentation campaign token." >&2
  exit 2
fi
source "$(dirname "$0")/assert_disposable_android_emulator.sh"
test "$(adb shell getprop ro.build.version.sdk | tr -d '\r')" = "${EXPECTED_API_LEVEL:?EXPECTED_API_LEVEL must be set}"
package='app.serea.android.isolated'
receiver="$package/app.serea.android.ProcessDeathHarnessReceiver"
action='app.serea.android.action.PROCESS_DEATH_HARNESS'
request_id="campaign-seed-${BASHPID}-$(date +%s%N)-${RANDOM}"
test -n "$(adb shell pm path "$package" | tr -d '\r')"
# The isolated package has just been installed and is still stopped. Include it
# explicitly so only the guarded fresh-storage marker receiver is started.
adb shell am broadcast -f 0x20 -n "$receiver" -a "$action" \
  --es phase campaign-seed --es token "$token" --es requestId "$request_id"
for _ in $(seq 1 30); do
  result_log=$(adb logcat -d -s SereaProcessDeath:V)
  if grep -F "FAIL:$token:campaign-seed:$request_id:" <<< "$result_log" >/dev/null; then
    printf '%s\n' "$result_log" >&2
    exit 1
  fi
  if grep -F "PASS:$token:campaign-seed:$request_id" <<< "$result_log" >/dev/null; then
    echo "PASS: empty isolated app storage marked for one disposable instrumentation run."
    exit 0
  fi
  sleep 1
done
echo "Instrumentation campaign marker did not report completion." >&2
adb logcat -d -s SereaProcessDeath:V >&2
exit 1
