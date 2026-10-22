#!/usr/bin/env bash
set -euo pipefail

token="${1:?campaign token required}"
mode="${2:-instrumentation}"
if [[ ! "$token" =~ ^[A-Za-z0-9-]{1,120}$ ]]; then
  echo "Invalid instrumentation campaign token." >&2
  exit 2
fi
if [[ "$mode" != instrumentation && "$mode" != process-death ]]; then
  echo "Invalid isolated target preparation mode." >&2
  exit 2
fi
source "$(dirname "$0")/assert_disposable_android_emulator.sh"
test "$(adb shell getprop ro.build.version.sdk | tr -d '\r')" = "${EXPECTED_API_LEVEL:?EXPECTED_API_LEVEL must be set}"
package='app.serea.android.isolated'
receiver="$package/app.serea.android.ProcessDeathHarnessReceiver"
action='app.serea.android.action.PROCESS_DEATH_HARNESS'
request_id="campaign-seed-${BASHPID}-$(date +%s%N)-${RANDOM}"
phase='campaign-seed'
if [[ "$mode" == process-death ]]; then
  phase='process-death-seed'
fi
test -n "$(adb shell pm path "$package" | tr -d '\r')"
if [[ "$mode" == instrumentation ]]; then
  # Seed only the fresh debug-only package namespace. Starting its process here
  # would run ProfileVerifier before UTP's connected-test install and leave a
  # cache timestamp from the pre-UTP package install. run-as writes only this
  # one-use marker and does not launch the app.
  adb shell run-as "$package" sh -c '
    set -eu
    for directory in shared_prefs files databases no_backup; do
      if [ -L "$directory" ]; then exit 40; fi
      if [ -e "$directory" ]; then
        [ -d "$directory" ] || exit 41
        [ -z "$(ls -A "$directory")" ] || exit 42
      fi
    done
    if [ -e files/profileInstalled ] || [ -L files/profileInstalled ]; then exit 43; fi
    mkdir -p no_backup
    [ -d no_backup ] && [ ! -L no_backup ] || exit 44
    marker=no_backup/serea-instrumentation-token
    temporary=no_backup/.serea-instrumentation-token.tmp
    [ ! -e "$marker" ] && [ ! -L "$marker" ] || exit 45
    [ ! -e "$temporary" ] && [ ! -L "$temporary" ] || exit 46
    (umask 077; set -C; printf "%s" "$1" > "$temporary")
    mv "$temporary" "$marker"
    [ -f "$marker" ] && [ ! -L "$marker" ] || exit 47
    [ "$(cat "$marker")" = "$1" ] || exit 48
    [ "$(ls -A no_backup)" = serea-instrumentation-token ] || exit 49
  ' s "$token"
  adb shell run-as "$package" sh -c '
    set -eu
    [ ! -e files/profileInstalled ] && [ ! -L files/profileInstalled ]
  ' s
  echo 'profileVerifierMarkerBeforeUtp=ABSENT'
  echo 'PASS: fresh isolated namespace received a one-use host seed without starting the app.'
  exit 0
fi

# Process Death preparation intentionally starts the debug-only target after
# all UTP installations have completed; it does not precede another install.
adb shell am broadcast -f 0x20 -n "$receiver" -a "$action" \
  --es phase "$phase" --es token "$token" --es requestId "$request_id"
for _ in $(seq 1 30); do
  result_log=$(adb logcat -d -s SereaProcessDeath:V)
  if grep -F "FAIL:$token:$phase:$request_id:" <<< "$result_log" >/dev/null; then
    printf '%s\n' "$result_log" >&2
    exit 1
  fi
  if grep -F "PASS:$token:$phase:$request_id" <<< "$result_log" >/dev/null; then
    echo "PASS: empty isolated app storage authorized for one $mode run."
    exit 0
  fi
  sleep 1
done
echo "Instrumentation campaign marker did not report completion." >&2
adb logcat -d -s SereaProcessDeath:V >&2
exit 1
