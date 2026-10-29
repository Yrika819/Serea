#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(git rev-parse --show-toplevel)"
APP_ID='app.serea.android.isolated'
TEST_ID='app.serea.android.isolated.test'
DEVICE_MODEL_TEMP='/data/local/tmp/serea-g8-smollm2.gguf'
EXPECTED_NATIVE_ABI="${SEREA_EXPECTED_NATIVE_ABI:-x86_64}"
[[ "$EXPECTED_NATIVE_ABI" == 'x86_64' || "$EXPECTED_NATIVE_ABI" == 'arm64-v8a' ]]

source "$ROOT_DIR/tools/assert_disposable_android_emulator.sh"
DEVICE_SERIAL="$ANDROID_SERIAL"
getprop() {
    adb -s "$DEVICE_SERIAL" shell getprop "$1" | tr -d '\r'
}

[[ "$(getprop sys.boot_completed)" == '1' ]]
[[ "$(getprop ro.product.cpu.abi)" == "$EXPECTED_NATIVE_ABI" ]]
hardware="$(getprop ro.hardware)"
fingerprint="$(getprop ro.build.fingerprint | tr '[:upper:]' '[:lower:]')"
[[ "$hardware" == ranchu || "$hardware" == goldfish ]]
[[ "$fingerprint" == *generic* || "$fingerprint" == *sdk_gphone* || "$fingerprint" == *sdk_phone* ]]

for package in "$APP_ID" "$TEST_ID"; do
    if ! packages="$(adb -s "$DEVICE_SERIAL" shell pm list packages -u "$package")"; then
        echo "Could not verify isolated package absence for $package; refusing installation." >&2
        exit 3
    fi
    packages="$(printf '%s' "$packages" | tr -d '\r')"
    if [[ "$packages" == "package:$package" ]]; then
        echo "Refusing pre-existing isolated package $package; no package data was changed." >&2
        exit 3
    fi
    if [[ -n "$packages" ]]; then
        echo "Unexpected package query response for $package; refusing installation." >&2
        exit 3
    fi
done

if ! adb -s "$DEVICE_SERIAL" shell test '!' -e "$DEVICE_MODEL_TEMP" >/dev/null 2>&1; then
    echo 'Refusing a pre-existing device temporary model path.' >&2
    exit 3
fi

printf 'device_serial=%s\napi=35\nabi=%s\nhardware=%s\nfingerprint=%s\npackage_ids=ABSENT\ntemp_model=ABSENT\n' \
    "$DEVICE_SERIAL" "$EXPECTED_NATIVE_ABI" "$hardware" "$fingerprint"
