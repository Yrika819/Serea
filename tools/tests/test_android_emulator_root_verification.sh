#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/bin"
cat > "$tmp/bin/adb" <<'ADB'
#!/usr/bin/env bash
set -u
shift 2 # -s SERIAL
command="$1"; shift
case "${ADB_SCENARIO:?}:$command" in
  empty:root|already:root) echo 0 >> "$ADB_CALLS"; exit 0 ;;
  offline-once:root)
    count="$(wc -l < "$ADB_CALLS")"
    echo 0 >> "$ADB_CALLS"
    if [[ "$count" == 0 ]]; then echo 'error: device offline'; exit 1; fi
    exit 0 ;;
  denied:root) echo 'adbd cannot run as root in production builds'; exit 1 ;;
  offline-forever:root) echo 'error: device offline'; exit 1 ;;
  *:root) exit 1 ;;
  empty:shell|already:shell|offline-once:shell) echo 'uid=0(root) gid=0(root) groups=0(root)'; exit 0 ;;
  uid2000:shell) echo 'uid=2000(shell) gid=2000(shell) groups=2000(shell)'; exit 0 ;;
  reconnect:reconnect) echo 0 >> "$ADB_CALLS"; exit 0 ;;
  reconnect:get-state) echo device; exit 0 ;;
  offline-forever:reconnect) echo 0 >> "$ADB_CALLS"; exit 0 ;;
  offline-forever:get-state) echo offline; exit 0 ;;
  *) echo "unexpected adb invocation: $command $*" >&2; exit 1 ;;
esac
ADB
chmod +x "$tmp/bin/adb"
source "$root/tools/android_emulator_lifecycle.sh"
export PATH="$tmp/bin:$PATH"
export ADB_CALLS="$tmp/calls"

expect_pass() {
  local scenario="$1"; export ADB_SCENARIO="$scenario"
  serea_restart_adbd_and_require_root emulator-5554 "$tmp/$scenario.log" 3
  grep -Fq 'root_uid_verified=true' "$tmp/$scenario.log"
}
expect_fail() {
  local scenario="$1"; export ADB_SCENARIO="$scenario"
  if serea_restart_adbd_and_require_root emulator-5554 "$tmp/$scenario.log" 3 2>"$tmp/$scenario.err"; then
    echo "Expected $scenario to fail" >&2; exit 1
  fi
}

expect_pass empty
expect_pass already
expect_pass offline-once
expect_fail denied
expect_fail uid2000
expect_fail offline-forever

# Ensure the harness still requires exact target identity and sends SIGKILL (not force-stop)
# after the root helper succeeds.
grep -Fq 'target_process_record" == "$target_pid_before $package"' "$root/tools/run_android_inflight_consent_process_death.sh"
grep -Fq 'shell kill -9 "$target_pid_before"' "$root/tools/run_android_inflight_consent_process_death.sh"
if grep -Fq 'shell am force-stop "$package"' "$root/tools/run_android_inflight_consent_process_death.sh"; then
  # The only force-stop is before the harness starts; termination remains explicit SIGKILL.
  [[ "$(grep -Fc 'shell am force-stop "$package"' "$root/tools/run_android_inflight_consent_process_death.sh")" == 1 ]]
fi
echo 'PASS: adb root output, UID verification, transient recovery, permanent failure, target PID and SIGKILL guards'
