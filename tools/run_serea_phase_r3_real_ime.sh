#!/usr/bin/env bash
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${SEREA_R3_OUT:?Set SEREA_R3_OUT}"
APKS="${SEREA_R3_APKS:?Set SEREA_R3_APKS}"
API="${SEREA_R3_API:?Set SEREA_R3_API}"
TARGET=app.serea.android.isolated
TEST=app.serea.android.isolated.test
IME=app.serea.android.r25testime
IME_COMPONENT="$IME/app.serea.android.diagnostics.RealImeService"
RUNNER="$TEST/app.serea.android.SereaIsolatedAndroidJUnitRunner"
CLASS=app.serea.android.diagnostics.IndependentImeCompositionTest

mkdir -p "$OUT/anr"
exec > >(tee -a "$OUT/driver.txt") 2>&1
printf 'api=%s\nsource_commit=%s\nsource_tree=%s\n' "$API" "$(git -C "$ROOT" rev-parse HEAD)" "$(git -C "$ROOT" rev-parse 'HEAD^{tree}')" > "$OUT/metadata.txt"
sha256sum "$APKS"/*.apk > "$OUT/apk-sha256.txt"
adb shell getprop ro.build.fingerprint > "$OUT/fingerprint.txt" 2>&1 || true
adb shell settings get system font_scale > "$OUT/font-scale.txt" 2>&1 || true
adb shell getprop sys.boot_completed > "$OUT/boot-completed.txt" 2>&1 || true
adb shell cat /proc/pressure/cpu > "$OUT/guest-cpu-pressure.txt" 2>&1 || true
adb shell cat /proc/pressure/memory > "$OUT/guest-memory-pressure.txt" 2>&1 || true
adb shell dumpsys meminfo > "$OUT/meminfo-before.txt" 2>&1 || true
adb shell dumpsys cpuinfo > "$OUT/cpuinfo-before.txt" 2>&1 || true
adb shell pm list packages > "$OUT/packages-before.txt" 2>&1 || true
ORIGINAL_IME="$(adb shell settings get secure default_input_method 2>/dev/null | tr -d '\r')"
printf 'original_ime=%s\n' "$ORIGINAL_IME" >> "$OUT/metadata.txt"
adb install -r "$APKS/r25-ime-test-debug.apk" > "$OUT/install-ime.txt" 2>&1 || exit 10
adb install -r "$APKS/app-isolated.apk" > "$OUT/install-target.txt" 2>&1 || exit 10
adb install -r "$APKS/app-isolated-androidTest.apk" > "$OUT/install-test.txt" 2>&1 || exit 10
adb shell pm grant "$TEST" app.serea.android.r25testime.permission.DIAGNOSTICS > "$OUT/permission-grant.txt" 2>&1 || true
adb shell ime enable "$IME_COMPONENT" > "$OUT/ime-enable.txt" 2>&1 || exit 11
adb shell ime set "$IME_COMPONENT" > "$OUT/ime-select.txt" 2>&1 || exit 12
adb shell ime list -s > "$OUT/ime-list-selected.txt" 2>&1 || true
adb shell dumpsys package "$IME" > "$OUT/ime-package.txt" 2>&1 || true
adb shell dumpsys activity activities > "$OUT/activity-before.txt" 2>&1 || true
adb shell dumpsys input_method > "$OUT/input-method-before.txt" 2>&1 || true
adb logcat -d -v threadtime > "$OUT/logcat-startup.txt" 2>&1 || true
adb logcat -c >/dev/null 2>&1 || true
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
  adb root > "$OUT/adb-root.txt" 2>&1 || true
  adb wait-for-device >/dev/null 2>&1 || true
  adb pull /data/anr "$OUT/anr" > "$OUT/anr-pull.txt" 2>&1 || true
  adb shell dumpsys activity lastanr > "$OUT/lastanr.txt" 2>&1 || true
  adb shell settings get secure default_input_method > "$OUT/default-ime-after.txt" 2>&1 || true
  if [[ "$ORIGINAL_IME" == null || -z "$ORIGINAL_IME" ]]; then
    adb shell settings delete secure default_input_method > "$OUT/ime-restore.txt" 2>&1 || true
  else
    adb shell ime set "$ORIGINAL_IME" > "$OUT/ime-restore.txt" 2>&1 || true
  fi
  adb shell ime disable "$IME_COMPONENT" >> "$OUT/ime-restore.txt" 2>&1 || true
  adb shell ime list -s > "$OUT/ime-list-restored.txt" 2>&1 || true
  RESTORED_IME="$(adb shell settings get secure default_input_method 2>/dev/null | tr -d '\r')"
  printf 'expected=%s\nrestored=%s\n' "$ORIGINAL_IME" "$RESTORED_IME" > "$OUT/ime-restoration-verification.txt"
  if [[ "$RESTORED_IME" == "$ORIGINAL_IME" ]] && ! grep -Fq "$IME_COMPONENT" "$OUT/ime-list-restored.txt"; then
    echo 'verified=true' >> "$OUT/ime-restoration-verification.txt"
  else
    echo 'verified=false' >> "$OUT/ime-restoration-verification.txt"
  fi
  for method in B_realImeCompositionWithoutSynchronizationBoundaries C_realImeCompositionWithSynchronizationBoundaries; do
    python3 "$ROOT/tools/instrumentation_to_junit.py" "$OUT/$method-instrumentation.txt" "$OUT/$method-junit.xml" "$method" || true
  done
  printf 'driver_exit=%s\n' "$status" >> "$OUT/metadata.txt"
  return 0
}
trap finish EXIT

run_case() {
  local method="$1" result token
  local output="$OUT/$method-instrumentation.txt"
  token="serea-r3-${GITHUB_RUN_ID:-local}-${GITHUB_RUN_ATTEMPT:-1}-api${API}-${method}-$(date +%s)-${RANDOM}"
  printf 'method=%s start_utc=%s\n' "$method" "$(date -u +%FT%TZ)" | tee -a "$OUT/method-times.txt"
  # Each B/C case gets a fresh target namespace and a one-use runner marker.
  adb shell pm clear "$TARGET" > "$OUT/$method-pm-clear.txt" 2>&1 || return 1
  EXPECTED_API_LEVEL="$API" bash "$ROOT/tools/prepare_isolated_instrumentation_target.sh" "$token" > "$OUT/$method-isolated-target-seed.txt" 2>&1 || return 1
  set +e
  timeout --signal=TERM 240s adb shell am instrument -w -r -e class "$CLASS#$method" -e sereaCampaignToken "$token" "$RUNNER" > "$output" 2>&1
  result=$?
  set -e
  printf 'method=%s am_instrument_exit=%s end_utc=%s\n' "$method" "$result" "$(date -u +%FT%TZ)" | tee -a "$OUT/method-times.txt"
  if [[ "$result" -ne 0 ]] || ! grep -q 'INSTRUMENTATION_STATUS_CODE: 0' "$output" || ! grep -q 'INSTRUMENTATION_STATUS: numtests=1' "$output" || ! grep -q 'INSTRUMENTATION_CODE: -1' "$output" || grep -q 'FAILURES!!!' "$output"; then
    return 1
  fi
}

if ! run_case B_realImeCompositionWithoutSynchronizationBoundaries; then exit 20; fi
if ! run_case C_realImeCompositionWithSynchronizationBoundaries; then exit 21; fi
echo 'real_ime_BC=PASS' >> "$OUT/metadata.txt"
