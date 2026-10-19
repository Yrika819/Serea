#!/usr/bin/env bash
set -euxo pipefail

package="app.serea.android.demo"
activity="$package/app.serea.android.MainActivity"
receiver="$package/app.serea.android.ProcessDeathHarnessReceiver"
action="app.serea.android.action.PROCESS_DEATH_HARNESS"

command -v adb >/dev/null
adb get-state >/dev/null
test "$(adb shell getprop ro.build.version.sdk | tr -d '\r')" = "${EXPECTED_API_LEVEL:?EXPECTED_API_LEVEL must be set}"
before_path=$(adb shell pm path "$package" | tr -d '\r')
test -n "$before_path"
adb logcat -c

phase() {
  echo "Process-death harness: starting phase $1"
  adb shell am broadcast -n "$receiver" -a "$action" --es phase "$1"
  for _ in $(seq 1 30); do
    if adb logcat -d -s SereaProcessDeath:I | grep -F "PASS:$1" >/dev/null; then return 0; fi
    if adb logcat -d -s SereaProcessDeath:E | grep -F "FAIL:$1:" >/dev/null; then
      adb logcat -d -s SereaProcessDeath:E
      return 1
    fi
    sleep 1
  done
  echo "Process-death harness phase $1 did not report completion." >&2
  adb logcat -d -s SereaProcessDeath:I SereaProcessDeath:E >&2
  return 1
}

phase prepare
echo "Process-death harness: checking prepared encrypted database"
adb shell run-as "$package" ls -la databases
adb shell run-as "$package" test -s databases/serea-history.db
echo "Process-death harness: force-stopping target app without clearing data"
adb shell am force-stop "$package"
after_path=$(adb shell pm path "$package" | tr -d '\r')
test "$before_path" = "$after_path"
adb shell run-as "$package" test -s databases/serea-history.db
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

echo "PASS: Android OS Process Death recovery on API ${EXPECTED_API_LEVEL} (persisted history, pending recovery without replay, deletion, encrypted read, package/data preserved)."
