#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ANDROID_ROOT="$ROOT/android"
SDK_ROOT="${ANDROID_HOME:-${ANDROID_SDK_ROOT:?Set ANDROID_HOME or ANDROID_SDK_ROOT}}"
ANDROID_AVD_HOME="${ANDROID_AVD_HOME:-${ANDROID_USER_HOME:-$HOME/.android}/avd}"
export ANDROID_AVD_HOME
ADB="$SDK_ROOT/platform-tools/adb"
EMULATOR="$SDK_ROOT/emulator/emulator"
AVDMANAGER="$SDK_ROOT/cmdline-tools/latest/bin/avdmanager"
ARTIFACT_ROOT="${SEREA_R25_ARTIFACT_DIR:-$ROOT/../Serea-phase-r25-evidence}"
TARGET_APK="$ANDROID_ROOT/app/build/outputs/apk/isolated/app-isolated.apk"
TEST_APK="$ANDROID_ROOT/app/build/outputs/apk/androidTest/isolated/app-isolated-androidTest.apk"
IME_APK="$ANDROID_ROOT/r25-ime-test/build/outputs/apk/debug/r25-ime-test-debug.apk"
TARGET_PACKAGE="app.serea.android.isolated"
TEST_PACKAGE="app.serea.android.isolated.test"
IME_PACKAGE="app.serea.android.r25testime"
IME_ID="$IME_PACKAGE/app.serea.android.diagnostics.RealImeService"
IME_AUTHORITY="$IME_PACKAGE.diagnostics"
TEST_CLASS="app.serea.android.diagnostics.IndependentImeCompositionTest"
TEST_METHOD="${SEREA_R25_METHOD:-}"
API="${SEREA_R25_API:-26}"
RUN_ID="R25-API-${API}-$(date -u +%Y%m%dT%H%M%SZ)"
AVD_NAME="serea-r25-api-${API}"
OUT="$ARTIFACT_ROOT/$RUN_ID"
COMMIT="$(git -C "$ROOT" rev-parse HEAD)"
TREE="$(git -C "$ROOT" rev-parse HEAD^{tree})"

if [[ "$API" != 26 && "$API" != 35 ]]; then echo "Unsupported API: $API" >&2; exit 2; fi
mkdir -p "$OUT"
for apk in "$TARGET_APK" "$TEST_APK" "$IME_APK"; do
    test -s "$apk" || { echo "Missing APK: $apk; build :app:assembleIsolatedAndroidTest :r25-ime-test:assembleDebug first." >&2; exit 2; }
done

printf 'run_id=%s\napi=%s\ncommit=%s\ntree=%s\ntarget_apk_sha256=%s\ntest_apk_sha256=%s\nime_apk_sha256=%s\n' \
    "$RUN_ID" "$API" "$COMMIT" "$TREE" \
    "$(sha256sum "$TARGET_APK" | cut -d' ' -f1)" \
    "$(sha256sum "$TEST_APK" | cut -d' ' -f1)" \
    "$(sha256sum "$IME_APK" | cut -d' ' -f1)" > "$OUT/metadata.txt"
cp "$TARGET_APK" "$OUT/target.apk"
cp "$TEST_APK" "$OUT/test.apk"
cp "$IME_APK" "$OUT/test-ime.apk"

"$AVDMANAGER" create avd --force --name "$AVD_NAME" --package "system-images;android-${API};google_apis;x86_64" --device pixel_2 </dev/null >"$OUT/avdmanager.log" 2>&1
"$EMULATOR" -avd "$AVD_NAME" -port 5554 -wipe-data -no-accel -no-window -no-audio -no-boot-anim -gpu swiftshader_indirect >"$OUT/emulator.log" 2>&1 &
EMULATOR_PID=$!
SERIAL=emulator-5554
ORIGINAL_IME=""
cleanup() {
    status=$?
    trap - EXIT INT TERM
    set +e
    if "$ADB" -s "$SERIAL" get-state >/dev/null 2>&1 && [[ -n "$ORIGINAL_IME" ]]; then
        if [[ -n "$ORIGINAL_IME" && "$ORIGINAL_IME" != null ]]; then
            "$ADB" -s "$SERIAL" shell ime set "$ORIGINAL_IME" >>"$OUT/ime-restore.log" 2>&1
        elif [[ "$ORIGINAL_IME" == null ]]; then
            "$ADB" -s "$SERIAL" shell settings delete secure default_input_method >>"$OUT/ime-restore.log" 2>&1
        fi
        current_ime="$($ADB -s "$SERIAL" shell settings get secure default_input_method | tr -d '\r')"
        printf 'original=%s\nrestored=%s\n' "$ORIGINAL_IME" "$current_ime" >>"$OUT/ime-restore.log"
        if [[ "$current_ime" == "$IME_ID" ]]; then
            "$ADB" -s "$SERIAL" shell ime set com.google.android.inputmethod.latin/com.android.inputmethod.latin.LatinIME >>"$OUT/ime-restore.log" 2>&1
        fi
        "$ADB" -s "$SERIAL" shell ime disable "$IME_ID" >>"$OUT/ime-restore.log" 2>&1
        "$ADB" -s "$SERIAL" shell ime list -s >"$OUT/ime-list-restored.txt"
        "$ADB" -s "$SERIAL" emu kill >/dev/null 2>&1
    fi
    wait "$EMULATOR_PID" 2>/dev/null
    "$AVDMANAGER" delete avd --name "$AVD_NAME" >>"$OUT/ime-restore.log" 2>&1
    if [[ $status -eq 0 ]] && grep -Fq "$IME_ID" "$OUT/ime-list-restored.txt" 2>/dev/null; then status=1; fi
    exit "$status"
}
trap cleanup EXIT INT TERM

timeout 180 "$ADB" -s "$SERIAL" wait-for-device
deadline=$((SECONDS + 900))
while :; do
    boot="$($ADB -s "$SERIAL" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')"
    logboot="$($ADB -s "$SERIAL" shell getprop sys.logbootcomplete 2>/dev/null | tr -d '\r')"
    font="$($ADB -s "$SERIAL" shell settings get system font_scale 2>/dev/null | tr -d '\r' || true)"
    ime="$($ADB -s "$SERIAL" shell settings get secure default_input_method 2>/dev/null | tr -d '\r' || true)"
    if [[ ( "$boot" == 1 || "$logboot" == 1 ) && ( "$font" == null || "$font" =~ ^[0-9]+([.][0-9]+)?$ ) && ( "$ime" == null || "$ime" == */* ) ]]; then break; fi
    if (( SECONDS >= deadline )); then echo "API $API emulator boot timed out." >&2; exit 1; fi
    sleep 2
done
ORIGINAL_IME="$ime"
"$ADB" -s emulator-5554 root >"$OUT/adb-root.log" 2>&1 || true
timeout 60 "$ADB" -s emulator-5554 wait-for-device || true
{
    printf 'fingerprint=%s\nfont_scale_setting=%s\nfont_scale_effective=%s\nboot_complete=%s\nlog_boot_complete=%s\ndefault_ime_before=%s\n' \
        "$($ADB -s "$SERIAL" shell getprop ro.build.fingerprint | tr -d '\r')" "$font" "${font/null/1.0}" "$boot" "$logboot" "$ime"
} >> "$OUT/metadata.txt"

"$ADB" -s "$SERIAL" install "$IME_APK" >"$OUT/install-ime.log"
"$ADB" -s "$SERIAL" install "$TARGET_APK" >"$OUT/install-target.log"
"$ADB" -s "$SERIAL" install "$TEST_APK" >"$OUT/install-test.log"
"$ADB" -s "$SERIAL" shell dumpsys package "$TEST_PACKAGE" | rg -A 4 'app.serea.android.r25testime.permission.DIAGNOSTICS' >"$OUT/diagnostics-permission.txt" || true
"$ADB" -s "$SERIAL" shell pm list packages | rg "$TARGET_PACKAGE|$TEST_PACKAGE|$IME_PACKAGE" > "$OUT/installed-packages.txt"
campaign_token="${RUN_ID}-$(date +%s%N)"
"$ADB" -s "$SERIAL" shell run-as "$TARGET_PACKAGE" mkdir -p no_backup
"$ADB" -s "$SERIAL" shell "run-as $TARGET_PACKAGE sh -c 'printf %s $campaign_token > no_backup/serea-instrumentation-token'"
"$ADB" -s "$SERIAL" logcat -c >/dev/null 2>&1 || true

if [[ -n "$TEST_METHOD" ]]; then
    test_selector="$TEST_CLASS#$TEST_METHOD"
else
    test_selector="$TEST_CLASS"
fi
set +e
"$ADB" -s "$SERIAL" shell am instrument -w -r -e sereaCampaignToken "$campaign_token" -e class "$test_selector" \
    "$TEST_PACKAGE/app.serea.android.SereaIsolatedAndroidJUnitRunner" 2>&1 | tee "$OUT/instrumentation.txt"
result=${PIPESTATUS[0]}
set -e

"$ADB" -s "$SERIAL" logcat -d -v threadtime >"$OUT/full-logcat.txt"
rg 'SereaImeR25|SereaRealIme|AndroidRuntime|ActivityManager.*ANR|InputMethodManagerService' "$OUT/full-logcat.txt" >"$OUT/logcat.txt" || true
"$ADB" -s "$SERIAL" shell dumpsys activity activities >"$OUT/activity-activities.txt" 2>&1 || true
"$ADB" -s "$SERIAL" shell dumpsys input_method >"$OUT/input-method.txt" 2>&1 || true
"$ADB" -s "$SERIAL" shell ps -A >"$OUT/processes.txt" 2>&1 || true
"$ADB" -s "$SERIAL" shell cat /proc/pressure/cpu >"$OUT/cpu-pressure.txt" 2>&1 || true
"$ADB" -s "$SERIAL" shell cat /proc/pressure/memory >"$OUT/memory-pressure.txt" 2>&1 || true
"$ADB" -s "$SERIAL" shell ime list -s >"$OUT/ime-list-after.txt"
"$ADB" -s "$SERIAL" shell ls -l /data/anr >"$OUT/anr-files.txt" 2>&1 || true
mkdir -p "$OUT/anr"
"$ADB" -s "$SERIAL" pull /data/anr "$OUT/anr" >"$OUT/anr-pull.txt" 2>&1 || true
if [[ $result -ne 0 ]] || grep -Eq 'FAILURES!!!|shortMsg=Process crashed\.' "$OUT/instrumentation.txt"; then exit 1; fi
