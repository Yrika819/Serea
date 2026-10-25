#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SDK_ROOT="${ANDROID_HOME:-${ANDROID_SDK_ROOT:?Set ANDROID_HOME or ANDROID_SDK_ROOT}}"
ANDROID_AVD_HOME="${ANDROID_AVD_HOME:-${ANDROID_USER_HOME:-$HOME/.android}/avd}"
export ANDROID_AVD_HOME
ADB="$SDK_ROOT/platform-tools/adb"
EMULATOR="$SDK_ROOT/emulator/emulator"
AVDMANAGER="$SDK_ROOT/cmdline-tools/latest/bin/avdmanager"
ARTIFACT_ROOT="${SEREA_R2_ARTIFACT_DIR:-$ROOT/artifacts/phase-r2}"
TARGET_APK="$ROOT/android/app/build/outputs/apk/isolated/app-isolated.apk"
TEST_APK="$ROOT/android/app/build/outputs/apk/androidTest/isolated/app-isolated-androidTest.apk"
IME_ID="app.serea.android.isolated.test/app.serea.android.diagnostics.RealImeService"
TEST_CLASS="app.serea.android.diagnostics.IndependentImeCompositionTest"
COMMIT="$(git -C "$ROOT" rev-parse HEAD)"
TREE="$(git -C "$ROOT" rev-parse HEAD^{tree})"

for api in 26 35; do
    run_id="R2-API-${api}-$(date -u +%Y%m%dT%H%M%SZ)"
    name="serea-r2-api-${api}"
    out="$ARTIFACT_ROOT/$run_id"
    mkdir -p "$out"
    test -s "$TARGET_APK" && test -s "$TEST_APK" || {
        echo "Missing isolated target or test APK. Build :app:assembleIsolatedAndroidTest first." >&2
        exit 2
    }
    printf 'run_id=%s\napi=%s\ncommit=%s\ntree=%s\ntarget_apk_sha256=%s\ntest_apk_sha256=%s\n' \
        "$run_id" "$api" "$COMMIT" "$TREE" \
        "$(sha256sum "$TARGET_APK" | cut -d' ' -f1)" \
        "$(sha256sum "$TEST_APK" | cut -d' ' -f1)" > "$out/metadata.txt"

    "$AVDMANAGER" create avd --force --name "$name" --package "system-images;android-${api};google_apis;x86_64" --device pixel_2 </dev/null >"$out/avdmanager.log" 2>&1
    "$EMULATOR" -avd "$name" -port 5554 -wipe-data -no-accel -no-window -no-audio -no-boot-anim -gpu swiftshader_indirect >"$out/emulator.log" 2>&1 &
    emulator_pid=$!
    serial=emulator-5554
    original_ime=""
    cleanup() {
        set +e
        restore_failed=0
        if "$ADB" -s "$serial" get-state >/dev/null 2>&1; then
            if [[ -n "$original_ime" && "$original_ime" != "null" ]]; then
                "$ADB" -s "$serial" shell ime set "$original_ime" >>"$out/ime-restore.log" 2>&1
            elif [[ "$original_ime" == "null" ]]; then
                "$ADB" -s "$serial" shell settings delete secure default_input_method >>"$out/ime-restore.log" 2>&1
            fi
            restored_ime="$("$ADB" -s "$serial" shell settings get secure default_input_method | tr -d '\r')"
            printf 'original=%s\nrestored=%s\n' "$original_ime" "$restored_ime" >>"$out/ime-restore.log"
            if [[ "$restored_ime" != "$original_ime" ]]; then restore_failed=1; fi
            "$ADB" -s "$serial" shell ime disable "$IME_ID" >>"$out/ime-restore.log" 2>&1
            "$ADB" -s "$serial" shell ime list -s >"$out/ime-list-restored.txt"
            if grep -Fq "$IME_ID" "$out/ime-list-restored.txt"; then restore_failed=1; fi
            "$ADB" -s "$serial" emu kill >/dev/null 2>&1
        fi
        wait "$emulator_pid" 2>/dev/null
        "$AVDMANAGER" delete avd --name "$name" >/dev/null 2>&1
        set -e
        return "$restore_failed"
    }
    trap cleanup EXIT INT TERM

    timeout 180 "$ADB" -s "$serial" wait-for-device
    boot_deadline=$((SECONDS + 600))
    while :; do
        boot_ready="$("$ADB" -s "$serial" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')"
        log_boot_ready="$("$ADB" -s "$serial" shell getprop sys.logbootcomplete 2>/dev/null | tr -d '\r')"
        if [[ "$boot_ready" == "1" || "$log_boot_ready" == "1" ]]; then
            if ready_font_scale="$("$ADB" -s "$serial" shell settings get system font_scale 2>/dev/null | tr -d '\r')" && \
                ready_ime="$("$ADB" -s "$serial" shell settings get secure default_input_method 2>/dev/null | tr -d '\r')"; then
                if [[ "$ready_font_scale" =~ ^[0-9]+([.][0-9]+)?$ && ( "$ready_ime" == "null" || "$ready_ime" == */* ) ]]; then
                    break
                fi
            fi
        fi
        if (( SECONDS >= boot_deadline )); then echo "Emulator API $api did not finish booting." >&2; exit 1; fi
        sleep 2
    done
    original_ime="$ready_ime"
    {
        printf 'fingerprint=%s\nfont_scale=%s\nboot_complete=%s\nlog_boot_complete=%s\ndefault_ime_before=%s\n' \
            "$("$ADB" -s "$serial" shell getprop ro.build.fingerprint | tr -d '\r')" \
            "$ready_font_scale" \
            "$("$ADB" -s "$serial" shell getprop sys.boot_completed | tr -d '\r')" \
            "$("$ADB" -s "$serial" shell getprop sys.logbootcomplete | tr -d '\r')" "$original_ime"
    } >> "$out/metadata.txt"

    "$ADB" -s "$serial" install "$TARGET_APK" >"$out/install-target.log"
    "$ADB" -s "$serial" install "$TEST_APK" >"$out/install-test.log"
    token="${run_id}-$(date +%s%N)"
    "$ADB" -s "$serial" shell run-as app.serea.android.isolated sh -c \
        "mkdir -p no_backup && printf '%s' '$token' > no_backup/serea-instrumentation-token"
    "$ADB" -s "$serial" logcat -c
    set +e
    "$ADB" -s "$serial" shell am instrument -w -r \
        -e sereaCampaignToken "$token" \
        -e class "$TEST_CLASS" \
        app.serea.android.isolated.test/app.serea.android.SereaIsolatedAndroidJUnitRunner \
        2>&1 | tee "$out/instrumentation.txt"
    result=${PIPESTATUS[0]}
    set -e
    "$ADB" -s "$serial" logcat -d -v threadtime -s SereaImeR2:I SereaRealIme:I > "$out/ime-logcat.txt"
    "$ADB" -s "$serial" shell ime list -s > "$out/ime-list-after.txt"
    if [[ $result -ne 0 ]]; then
        echo "Instrumentation failed for API $api; evidence: $out" >&2
        exit "$result"
    fi
    cleanup
    trap - EXIT INT TERM
done
