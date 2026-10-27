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
  # run-as executes under the installed isolated app UID. Do not pass a script
  # through `adb shell ... sh -c`: adb and the device shell each parse command
  # strings, and the script can escape run-as as a result. Verify run-as's
  # actual UID and physical working directory before sending any write command.
  app_uid="$(adb shell run-as "$package" id -u | tr -d '\r')" || exit 1
  app_data_dir="$(adb shell run-as "$package" pwd | tr -d '\r')" || exit 1
  if [[ ! "$app_uid" =~ ^[0-9]+$ || "$app_uid" -lt 10000 ]]; then
    echo "Refusing instrumentation seed: run-as returned an unexpected UID '$app_uid'." >&2
    exit 1
  fi
  if [[ "$app_data_dir" != "/data/user/0/$package" && "$app_data_dir" != "/data/data/$package" ]]; then
    echo "Refusing instrumentation seed: run-as working directory '$app_data_dir' is not the target package data directory." >&2
    exit 1
  fi
  echo "seedTarget=package:$package uid:$app_uid dataDir:$app_data_dir"

  # Feed the shell program on stdin. The token is restricted above to a safe
  # alphanumeric/hyphen alphabet before it is placed in this quoted assignment.
  if {
    printf "token='%s'\n" "$token"
    cat <<'SEED_SCRIPT'
set -eu
expected_dir='/data/user/0/app.serea.android.isolated'
legacy_dir='/data/data/app.serea.android.isolated'
actual_dir="$(pwd -P)"
if [ "$actual_dir" != "$expected_dir" ] && [ "$actual_dir" != "$legacy_dir" ]; then
  echo "Refusing seed from unexpected directory: $actual_dir" >&2
  exit 50
fi
for directory in shared_prefs files databases no_backup; do
  if [ -L "$directory" ]; then exit 40; fi
  if [ -e "$directory" ]; then
    [ -d "$directory" ] || exit 41
    [ -z "$(ls -A "$directory")" ] || exit 42
  fi
done
if [ -e files/profileInstalled ] || [ -L files/profileInstalled ]; then exit 43; fi
marker='no_backup/serea-instrumentation-token'
temporary='no_backup/.serea-instrumentation-token.tmp'
[ ! -e "$marker" ] && [ ! -L "$marker" ] || exit 45
[ ! -e "$temporary" ] && [ ! -L "$temporary" ] || exit 46
if [ ! -e no_backup ]; then
  mkdir no_backup
fi
[ -d no_backup ] && [ ! -L no_backup ] && [ -w no_backup ] && [ -x no_backup ] || exit 44
(umask 077; set -C; printf '%s' "$token" > "$temporary")
mv "$temporary" "$marker"
[ -f "$marker" ] && [ ! -L "$marker" ] || exit 47
[ "$(cat "$marker")" = "$token" ] || exit 48
[ "$(ls -A no_backup)" = serea-instrumentation-token ] || exit 49
[ ! -e files/profileInstalled ] && [ ! -L files/profileInstalled ] || exit 51
printf 'SEED_OK uid=%s dataDir=%s marker=%s\\n' "$(id -u)" "$(pwd)" "$marker"
SEED_SCRIPT
  } | adb shell run-as "$package" sh; then
    seed_result=0
  else
    seed_result=$?
    echo "Host seed failed inside run-as for $package (exit=$seed_result)." >&2
    exit "$seed_result"
  fi
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
