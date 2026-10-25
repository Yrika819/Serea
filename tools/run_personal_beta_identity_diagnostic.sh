#!/usr/bin/env bash
set -Eeuo pipefail

evidence="${SEREA_PERSONAL_BETA_EVIDENCE_DIR:-${RUNNER_TEMP:-/tmp}/serea-personal-beta-identity-diagnostic}"
export SEREA_PERSONAL_BETA_EVIDENCE_DIR="$evidence"
install -d -m 700 "$evidence"

collect_emulator_logs() {
  adb devices -l > "$evidence/adb-final-devices.txt" 2>&1 || true
  adb logcat -d -v threadtime > "$evidence/identity-logcat.txt" 2>&1 || true
  if [[ -f "${RUNNER_TEMP:-/tmp}/emulator.log" ]]; then
    cp "${RUNNER_TEMP:-/tmp}/emulator.log" "$evidence/emulator.log" || true
  fi
}
trap 'status=$?; collect_emulator_logs; exit "$status"' EXIT

{
  echo 'ADB connection preflight:'
  adb version
  adb devices -l
  printf 'api_level='
  adb shell getprop ro.build.version.sdk
  printf 'qemu_property='
  adb shell getprop ro.kernel.qemu
  printf 'pre_root_shell_uid='
  adb shell id -u
  printf 'boot_completed='
  adb shell getprop sys.boot_completed
} > "$evidence/adb-connection.txt" 2>&1

demo_apk='android/app/build/outputs/apk/debug/app-debug.apk'
(cd android && ./gradlew --no-daemon --no-build-cache --no-configuration-cache :app:assembleDebug)
test -s "$demo_apk"
adb install "$demo_apk" 2>&1 | tee "$evidence/demo-seed.log"
demo_token="identity-sentinel-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}"
demo_request="identity-seed-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}"
adb logcat -c
adb shell am broadcast -f 0x20 \
  -n app.serea.android.demo/app.serea.android.ProcessDeathHarnessReceiver \
  -a app.serea.android.action.PROCESS_DEATH_HARNESS \
  --es phase seed-sentinel --es token "$demo_token" --es requestId "$demo_request" \
  2>&1 | tee -a "$evidence/demo-seed.log"

seed_ok=false
for attempt in $(seq 1 60); do
  adb logcat -d -s SereaProcessDeath:V > "$evidence/demo-seed-logcat.txt" 2>&1 || true
  if grep -F "PASS:$demo_token:seed-sentinel:$demo_request" "$evidence/demo-seed-logcat.txt" >/dev/null; then
    seed_ok=true
    break
  fi
  if grep -F "FAIL:$demo_token:seed-sentinel:$demo_request:" "$evidence/demo-seed-logcat.txt" >/dev/null; then
    cat "$evidence/demo-seed-logcat.txt" >&2
    exit 1
  fi
  sleep 1
done
if [[ "$seed_ok" != true ]]; then
  cat "$evidence/demo-seed-logcat.txt" >&2
  echo 'Demo sentinel seed did not complete successfully.' >&2
  exit 1
fi
echo 'PASS: phase=demo-existing-data-seeded' | tee -a "$evidence/identity-run.log"

set +e
bash tools/test_personal_beta_identity.sh 2>&1 | tee -a "$evidence/identity-run.log"
identity_status=${PIPESTATUS[0]}
set -e
exit "$identity_status"
