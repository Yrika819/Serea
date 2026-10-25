#!/usr/bin/env bash
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${SEREA_R3_OUT:?Set SEREA_R3_OUT}"
CASE="${SEREA_R3_CASE:?Set SEREA_R3_CASE}"
TARGET=app.serea.android.isolated
TEST=app.serea.android.isolated.test
IME=app.serea.android.r25testime
IME_COMPONENT="$IME/app.serea.android.diagnostics.RealImeService"
RUNNER="$TEST/app.serea.android.SereaIsolatedAndroidJUnitRunner"
SMOKE=app.serea.android.diagnostics.PhaseR25StartupSmokeTest#runnerStarted
TEST_CLASS=app.serea.android.diagnostics.IndependentImeCompositionTest

mkdir -p "$OUT/anr"
exec > >(tee -a "$OUT/driver.txt") 2>&1
printf 'case=%s\nsource_commit=%s\nsource_tree=%s\n' "$CASE" "$(git -C "$ROOT" rev-parse HEAD)" "$(git -C "$ROOT" rev-parse 'HEAD^{tree}')" > "$OUT/case.txt"
adb get-serialno > "$OUT/serial.txt" 2>&1 || true
adb shell getprop ro.build.fingerprint > "$OUT/fingerprint.txt" 2>&1 || true
adb shell getprop sys.boot_completed > "$OUT/boot-completed.txt" 2>&1 || true
adb shell settings get system font_scale > "$OUT/font-scale.txt" 2>&1 || true
adb shell settings get secure default_input_method > "$OUT/default-ime-before.txt" 2>&1 || true
adb shell dumpsys activity activities > "$OUT/activity-before.txt" 2>&1 || true
adb shell dumpsys input_method > "$OUT/input-method-before.txt" 2>&1 || true
adb shell cat /proc/pressure/cpu > "$OUT/guest-cpu-pressure.txt" 2>&1 || true
adb shell cat /proc/pressure/memory > "$OUT/guest-memory-pressure.txt" 2>&1 || true
adb shell dumpsys meminfo > "$OUT/meminfo-before.txt" 2>&1 || true
adb shell dumpsys cpuinfo > "$OUT/cpuinfo-before.txt" 2>&1 || true
adb shell ps -A > "$OUT/processes-before.txt" 2>&1 || true
adb shell pm list packages > "$OUT/packages-before.txt" 2>&1 || true
ORIGINAL_IME="$(adb shell settings get secure default_input_method 2>/dev/null | tr -d '\r')"
printf 'original_ime=%s\n' "$ORIGINAL_IME" >> "$OUT/case.txt"
if [[ "$CASE" == case0-no-ime-installed ]] && adb shell pm list packages 2>/dev/null | grep -Fq "package:$IME"; then
  echo 'Case 0 unexpectedly has the Test IME installed.' > "$OUT/state-setup-failed.txt"
  exit 12
fi

if [[ "$CASE" != case0-no-ime-installed ]]; then
  adb install -r "$SEREA_R3_APKS/r25-ime-test-debug.apk" > "$OUT/install-ime.txt" 2>&1 || exit 10
fi
adb install -r "$SEREA_R3_APKS/app-isolated.apk" > "$OUT/install-target.txt" 2>&1 || exit 10
adb install -r "$SEREA_R3_APKS/app-isolated-androidTest.apk" > "$OUT/install-test.txt" 2>&1 || exit 10
if [[ "$CASE" == case1-ime-installed-disabled ]]; then
  adb shell pm path "$IME" > "$OUT/ime-package-path.txt" 2>&1 || exit 11
  adb shell ime disable "$IME_COMPONENT" > "$OUT/ime-disable-initial.txt" 2>&1 || true
fi
if [[ "$CASE" == case2-ime-enabled-not-selected || "$CASE" == case3-ime-selected ]]; then
  adb shell ime enable "$IME_COMPONENT" > "$OUT/ime-enable.txt" 2>&1 || exit 11
fi
if [[ "$CASE" == case3-ime-selected ]]; then
  adb shell ime set "$IME_COMPONENT" > "$OUT/ime-select.txt" 2>&1 || exit 12
fi
SELECTED_IME="$(adb shell settings get secure default_input_method 2>/dev/null | tr -d '\r')"
ENABLED_IMES="$(adb shell settings get secure enabled_input_methods 2>/dev/null | tr -d '\r')"
printf 'selected_ime_for_case=%s\n' "$SELECTED_IME" >> "$OUT/case.txt"
printf 'enabled_imes_for_case=%s\n' "$ENABLED_IMES" >> "$OUT/case.txt"
if [[ "$CASE" == case1-ime-installed-disabled && "$ENABLED_IMES" == *"$IME_COMPONENT"* ]]; then
  echo 'Case 1 test IME remains enabled.' > "$OUT/state-setup-failed.txt"
  exit 12
fi
if [[ "$CASE" == case2-ime-enabled-not-selected && ( "$ENABLED_IMES" != *"$IME_COMPONENT"* || "$SELECTED_IME" == "$IME_COMPONENT" ) ]]; then
  echo 'Case 2 expected test IME enabled but not selected.' > "$OUT/state-setup-failed.txt"
  exit 12
fi
if [[ "$CASE" == case3-ime-selected && ( "$ENABLED_IMES" != *"$IME_COMPONENT"* || "$SELECTED_IME" != "$IME_COMPONENT" ) ]]; then
  echo 'Case 3 expected test IME enabled and selected.' > "$OUT/state-setup-failed.txt"
  exit 12
fi
adb shell ime list -s > "$OUT/ime-list.txt" 2>&1 || true
adb shell dumpsys package "$IME" > "$OUT/ime-package.txt" 2>&1 || true
adb logcat -d -v threadtime > "$OUT/logcat-startup.txt" 2>&1 || true
adb shell logcat -c >/dev/null 2>&1 || true
adb logcat -v threadtime > "$OUT/logcat-live.txt" 2>&1 &
LOGCAT_PID=$!
finish() {
  status=$?
  kill "$LOGCAT_PID" >/dev/null 2>&1 || true
  wait "$LOGCAT_PID" >/dev/null 2>&1 || true
  adb logcat -d -v threadtime > "$OUT/logcat.txt" 2>&1 || true
  adb shell dumpsys activity activities > "$OUT/activity-after.txt" 2>&1 || true
  adb shell dumpsys input_method > "$OUT/input-method-after.txt" 2>&1 || true
  adb shell dumpsys meminfo > "$OUT/meminfo-after.txt" 2>&1 || true
  adb shell dumpsys cpuinfo > "$OUT/cpuinfo-after.txt" 2>&1 || true
  adb shell cat /proc/pressure/cpu > "$OUT/guest-cpu-pressure-after.txt" 2>&1 || true
  adb shell cat /proc/pressure/memory > "$OUT/guest-memory-pressure-after.txt" 2>&1 || true
  adb shell ps -A > "$OUT/processes-after.txt" 2>&1 || true
  adb root > "$OUT/adb-root.txt" 2>&1 || true
  adb wait-for-device >/dev/null 2>&1 || true
  adb pull /data/anr "$OUT/anr" > "$OUT/anr-pull.txt" 2>&1 || true
  adb shell dumpsys activity lastanr > "$OUT/lastanr.txt" 2>&1 || true
  adb shell settings get secure default_input_method > "$OUT/default-ime-after.txt" 2>&1 || true
  if [[ "$CASE" != case0-no-ime-installed ]]; then
    if [[ "$ORIGINAL_IME" == null || -z "$ORIGINAL_IME" ]]; then
      adb shell settings delete secure default_input_method > "$OUT/ime-restore.txt" 2>&1 || true
    else
      adb shell ime set "$ORIGINAL_IME" > "$OUT/ime-restore.txt" 2>&1 || true
    fi
    adb shell ime disable "$IME_COMPONENT" >> "$OUT/ime-restore.txt" 2>&1 || true
  fi
  RESTORED_IME="$(adb shell settings get secure default_input_method 2>/dev/null | tr -d '\r')"
  printf 'expected=%s\nrestored=%s\n' "$ORIGINAL_IME" "$RESTORED_IME" > "$OUT/ime-restoration-verification.txt"
  if [[ "$RESTORED_IME" == "$ORIGINAL_IME" ]]; then echo verified=true >> "$OUT/ime-restoration-verification.txt"; else echo verified=false >> "$OUT/ime-restoration-verification.txt"; fi
  adb shell ime list -s > "$OUT/ime-list-restored.txt" 2>&1 || true
  python3 "$ROOT/tools/instrumentation_to_junit.py" "$OUT/instrumentation.txt" "$OUT/junit.xml" "$CASE" || true
  printf 'driver_exit=%s\n' "$status" >> "$OUT/case.txt"
  return 0
}
trap finish EXIT

printf 'start_utc=%s\n' "$(date -u +%FT%TZ)" >> "$OUT/case.txt"
printf 'instrumentation_selector=%s\n' "$SMOKE" >> "$OUT/case.txt"
CAMPAIGN_TOKEN="serea-r3-${GITHUB_RUN_ID:-local}-${GITHUB_RUN_ATTEMPT:-1}-${CASE}-$(date +%s)-${RANDOM}"
printf 'campaign_token=%s\n' "$CAMPAIGN_TOKEN" >> "$OUT/case.txt"
SEREA_DISPOSABLE_CI_EMULATOR=1 SEREA_EXPECTED_ANDROID_API=35 EXPECTED_API_LEVEL=35 \
  bash "$ROOT/tools/prepare_isolated_instrumentation_target.sh" "$CAMPAIGN_TOKEN" > "$OUT/isolated-target-seed.txt" 2>&1 || {
  echo 'isolated_target_seed=FAIL' >> "$OUT/case.txt"
  exit 13
}
echo 'isolated_target_seed=PASS' >> "$OUT/case.txt"
set +e
timeout --signal=TERM 180s adb shell am instrument -w -r -e class "$SMOKE" -e sereaCampaignToken "$CAMPAIGN_TOKEN" "$RUNNER" > "$OUT/instrumentation.txt" 2>&1
driver_status=$?
set -e
printf 'am_instrument_exit=%s\ninstrumentation_end_utc=%s\n' "$driver_status" "$(date -u +%FT%TZ)" >> "$OUT/case.txt"
if grep -q 'INSTRUMENTATION_STATUS_CODE: 0' "$OUT/instrumentation.txt" && grep -q 'INSTRUMENTATION_STATUS: numtests=1' "$OUT/instrumentation.txt" && grep -q 'INSTRUMENTATION_CODE: -1' "$OUT/instrumentation.txt" && ! grep -q 'FAILURES!!!' "$OUT/instrumentation.txt"; then
  echo 'startup_smoke_methods=1' >> "$GITHUB_OUTPUT"
  echo 'startup_smoke=PASS' >> "$OUT/case.txt"
  exit 0
fi
echo 'startup_smoke_methods=0' >> "$GITHUB_OUTPUT"
echo 'startup_smoke=NOT_RUN_OR_FAILED' >> "$OUT/case.txt"
exit 1
