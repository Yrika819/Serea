#!/usr/bin/env bash
set -u -o pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ANDROID_ROOT="$ROOT/android"
SDK_ROOT="${ANDROID_HOME:-${ANDROID_SDK_ROOT:?Set ANDROID_HOME or ANDROID_SDK_ROOT}}"
ANDROID_AVD_HOME="${ANDROID_AVD_HOME:-${ANDROID_USER_HOME:-$HOME/.android}/avd}"
export ANDROID_AVD_HOME
ADB="$SDK_ROOT/platform-tools/adb"
EMULATOR="$SDK_ROOT/emulator/emulator"
AVDMANAGER="$SDK_ROOT/cmdline-tools/latest/bin/avdmanager"
OUT_ROOT="${SEREA_R25_ARTIFACT_DIR:-$ROOT/../Serea-phase-r25-evidence}"
TARGET_APK="$ANDROID_ROOT/app/build/outputs/apk/isolated/app-isolated.apk"
TEST_APK="$ANDROID_ROOT/app/build/outputs/apk/androidTest/isolated/app-isolated-androidTest.apk"
IME_APK="$ANDROID_ROOT/r25-ime-test/build/outputs/apk/debug/r25-ime-test-debug.apk"
TARGET="app.serea.android.isolated"
TEST="app.serea.android.isolated.test"
IME_PACKAGE="app.serea.android.r25testime"
IME_ID="$IME_PACKAGE/app.serea.android.diagnostics.RealImeService"
RUN_ID="R25-API-35-ANR-$(date -u +%Y%m%dT%H%M%SZ)"
AVD="serea-r25-anr-api-35"
OUT="$OUT_ROOT/$RUN_ID"

for apk in "$TARGET_APK" "$TEST_APK" "$IME_APK"; do
    test -s "$apk" || { echo "Missing APK: $apk; build the R2.5 APKs first." >&2; exit 2; }
done
mkdir -p "$OUT"
printf 'run_id=%s\napi=35\ncommit=%s\ntree=%s\n' "$RUN_ID" "$(git -C "$ROOT" rev-parse HEAD)" "$(git -C "$ROOT" rev-parse HEAD^{tree})" >"$OUT/metadata.txt"
for spec in "target:$TARGET_APK" "test:$TEST_APK" "test-ime:$IME_APK"; do
    name="${spec%%:*}"; apk="${spec#*:}"
    printf '%s_apk_sha256=%s\n' "$name" "$(sha256sum "$apk" | cut -d' ' -f1)" >>"$OUT/metadata.txt"
    cp "$apk" "$OUT/$name.apk"
done

"$AVDMANAGER" create avd --force --name "$AVD" --package system-images\;android-35\;google_apis\;x86_64 --device pixel_2 </dev/null >"$OUT/avdmanager.log" 2>&1
"$EMULATOR" -avd "$AVD" -port 5554 -wipe-data -no-accel -no-window -no-audio -no-boot-anim -gpu swiftshader_indirect >"$OUT/emulator.log" 2>&1 &
EMU_PID=$!
SERIAL=emulator-5554
ORIGINAL_IME=""
cleanup() {
    status=$?
    trap - EXIT INT TERM
    set +e
    if "$ADB" -s "$SERIAL" get-state >/dev/null 2>&1 && [[ -n "$ORIGINAL_IME" ]]; then
        if [[ "$ORIGINAL_IME" == null ]]; then
            "$ADB" -s "$SERIAL" shell settings delete secure default_input_method >>"$OUT/cleanup.txt" 2>&1
        else
            "$ADB" -s "$SERIAL" shell ime set "$ORIGINAL_IME" >>"$OUT/cleanup.txt" 2>&1
        fi
        "$ADB" -s "$SERIAL" shell ime disable "$IME_ID" >>"$OUT/cleanup.txt" 2>&1
        "$ADB" -s "$SERIAL" shell ime list -s >"$OUT/ime-list-restored.txt"
        "$ADB" -s "$SERIAL" emu kill >/dev/null 2>&1
    fi
    wait "$EMU_PID" 2>/dev/null
    "$AVDMANAGER" delete avd --name "$AVD" >>"$OUT/cleanup.txt" 2>&1
    exit "$status"
}
trap cleanup EXIT INT TERM

timeout 180 "$ADB" -s "$SERIAL" wait-for-device || exit 1
deadline=$((SECONDS + 1200))
while :; do
    boot="$("$ADB" -s "$SERIAL" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')"
    font="$("$ADB" -s "$SERIAL" shell settings get system font_scale 2>/dev/null | tr -d '\r' || true)"
    ime="$("$ADB" -s "$SERIAL" shell settings get secure default_input_method 2>/dev/null | tr -d '\r' || true)"
    if [[ "$boot" == 1 && ( "$font" == null || "$font" =~ ^[0-9]+([.][0-9]+)?$ ) && ( "$ime" == null || "$ime" == */* ) ]]; then break; fi
    if (( SECONDS >= deadline )); then echo "API 35 emulator boot timed out" >&2; exit 1; fi
    sleep 2
done
ORIGINAL_IME="$ime"
printf 'fingerprint=%s\nfont_scale=%s\nboot_complete=%s\noriginal_ime=%s\n' \
    "$("$ADB" -s "$SERIAL" shell getprop ro.build.fingerprint | tr -d '\r')" "$font" "$boot" "$ime" >>"$OUT/metadata.txt"
"$ADB" -s "$SERIAL" root >>"$OUT/adb-root.txt" 2>&1 || true
timeout 60 "$ADB" -s "$SERIAL" wait-for-device || true
"$ADB" -s "$SERIAL" install "$TARGET_APK" >"$OUT/install-target.txt" 2>&1 || exit 1
"$ADB" -s "$SERIAL" install "$TEST_APK" >"$OUT/install-test.txt" 2>&1 || exit 1
campaign_token="$RUN_ID-$(date +%s%N)"
"$ADB" -s "$SERIAL" shell run-as "$TARGET" mkdir -p no_backup
"$ADB" -s "$SERIAL" shell "run-as $TARGET sh -c 'printf %s $campaign_token > no_backup/serea-instrumentation-token'"

run_case() {
    case_name="$1"
    case_dir="$OUT/$case_name"
    mkdir -p "$case_dir"
    printf 'case=%s\nmonotonic_host_ns=%s\ninstalled_ime=%s\nenabled_ime=' "$case_name" "$(date +%s%N)" \
        "$("$ADB" -s "$SERIAL" shell pm list packages | rg -q "^package:$IME_PACKAGE$" && echo yes || echo no)" >"$case_dir/state.txt"
    "$ADB" -s "$SERIAL" shell ime list -s >>"$case_dir/state.txt" 2>&1
    printf 'selected_ime=%s\n' "$("$ADB" -s "$SERIAL" shell settings get secure default_input_method | tr -d '\r')" >>"$case_dir/state.txt"
    "$ADB" -s "$SERIAL" shell log -t SereaR25 "CASE_BEGIN $case_name"
    "$ADB" -s "$SERIAL" logcat -c >/dev/null 2>&1 || true
    "$ADB" -s "$SERIAL" shell am force-stop "$TARGET" >/dev/null 2>&1 || true
    "$ADB" -s "$SERIAL" shell am force-stop "$TEST" >/dev/null 2>&1 || true
    set +e
    timeout 180 "$ADB" -s "$SERIAL" shell am instrument -w -r \
        -e sereaCampaignToken "$campaign_token" \
        -e class app.serea.android.diagnostics.PhaseR25StartupSmokeTest \
        "$TEST/app.serea.android.SereaIsolatedAndroidJUnitRunner" >"$case_dir/instrumentation.txt" 2>&1
    result=$?
    set -e
    printf 'host_exit=%s\n' "$result" >>"$case_dir/state.txt"
    sleep 8
    "$ADB" -s "$SERIAL" logcat -d -v threadtime >"$case_dir/logcat.txt" 2>&1 || true
    "$ADB" -s "$SERIAL" shell dumpsys activity lastanr >"$case_dir/lastanr.txt" 2>&1 || true
    "$ADB" -s "$SERIAL" shell dumpsys activity processes >"$case_dir/activity-processes.txt" 2>&1 || true
    "$ADB" -s "$SERIAL" shell dumpsys input_method >"$case_dir/input-method.txt" 2>&1 || true
    "$ADB" -s "$SERIAL" shell ps -A >"$case_dir/processes.txt" 2>&1 || true
    "$ADB" -s "$SERIAL" shell cat /proc/pressure/cpu >"$case_dir/cpu-pressure.txt" 2>&1 || true
    "$ADB" -s "$SERIAL" shell cat /proc/pressure/memory >"$case_dir/memory-pressure.txt" 2>&1 || true
    "$ADB" -s "$SERIAL" shell ls -la /data/anr >"$case_dir/anr-files.txt" 2>&1 || true
    mkdir -p "$case_dir/anr"
    "$ADB" -s "$SERIAL" pull /data/anr "$case_dir/anr" >"$case_dir/anr-pull.txt" 2>&1 || true
    "$ADB" -s "$SERIAL" shell pidof "$TARGET" >"$case_dir/target-pid.txt" 2>&1 || true
    "$ADB" -s "$SERIAL" shell pidof "$TEST" >"$case_dir/test-pid.txt" 2>&1 || true
    printf 'boot=%s\n' "$("$ADB" -s "$SERIAL" shell getprop sys.boot_completed | tr -d '\r')" >>"$case_dir/state.txt"
}

run_case case0-no-ime-installed
"$ADB" -s "$SERIAL" install "$IME_APK" >"$OUT/install-ime.txt" 2>&1 || exit 1
run_case case1-ime-installed-disabled
"$ADB" -s "$SERIAL" shell ime enable "$IME_ID" >"$OUT/ime-enable.txt" 2>&1 || true
run_case case2-ime-enabled-not-selected
"$ADB" -s "$SERIAL" shell ime set "$IME_ID" >"$OUT/ime-select.txt" 2>&1 || true
run_case case3-ime-selected

echo "Artifacts: $OUT"
