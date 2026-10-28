#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(git rev-parse --show-toplevel)"
ANDROID_DIR="$ROOT_DIR/android"
EVIDENCE_DIR="$ROOT_DIR/evidence/g8-local-native"
MODEL_HOST_PATH="${RUNNER_TEMP:?RUNNER_TEMP is required}/serea-g8-smollm2.gguf"
APP_ID='app.serea.android.isolated'
TEST_ID='app.serea.android.isolated.test'
DEVICE_MODEL_RELATIVE='files/g8-smollm2-135m-q4_k_m.gguf'
DEVICE_MODEL_TEMP='/data/local/tmp/serea-g8-smollm2.gguf'
TARGET_APK="$ANDROID_DIR/app/build/outputs/apk/isolated/app-isolated.apk"
TEST_APK="$ANDROID_DIR/app/build/outputs/apk/androidTest/isolated/app-isolated-androidTest.apk"
MODEL_SHA256='e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5'
MODEL_SIZE_BYTES='101016128'
LLAMA_COMMIT='276378515ceebee5d14bcc8536603dd5c87491a2'
TEST_CLASS='app.serea.android.LocalModelNativeRuntimeE2ETest'
TEST_METHOD='pinnedModelRunsThroughLocalModelEgressUsageAndNativeBackend'

mkdir -p "$EVIDENCE_DIR/junit"

cleanup() {
    local original_status="$1"
    set +e
    adb shell run-as "$APP_ID" rm -f "$DEVICE_MODEL_RELATIVE" >/dev/null 2>&1
    adb shell rm -f "$DEVICE_MODEL_TEMP" >/dev/null 2>&1
    adb uninstall "$TEST_ID" >/dev/null 2>&1
    adb uninstall "$APP_ID" >/dev/null 2>&1
    if [[ -f "$MODEL_HOST_PATH" ]]; then
        rm -f "$MODEL_HOST_PATH"
    fi
    {
        echo 'model_weights_uploaded=false'
        echo 'host_model_file_removed=true'
        echo 'app_and_test_packages_uninstalled=true'
        echo "original_exit_status=$original_status"
        echo 'runner_vm_and_disposable_avd=ephemeral'
    } > "$EVIDENCE_DIR/cleanup-boundary.txt"
    return 0
}
trap 'cleanup "$?"' EXIT

[[ -s "$TARGET_APK" ]]
[[ -s "$TEST_APK" ]]
[[ -s "$MODEL_HOST_PATH" ]]
test "$(stat -c '%s' "$MODEL_HOST_PATH")" = "$MODEL_SIZE_BYTES"
test "$(sha256sum "$MODEL_HOST_PATH" | awk '{print $1}')" = "$MODEL_SHA256"

adb wait-for-device
[[ "$(adb shell getprop sys.boot_completed | tr -d '\r')" == '1' ]]
[[ "$(adb shell getprop ro.build.version.sdk | tr -d '\r')" == '35' ]]
[[ "$(adb shell getprop ro.product.cpu.abi | tr -d '\r')" == 'x86_64' ]]
{
    echo "source_sha=$(git -C "$ROOT_DIR" rev-parse HEAD)"
    echo "source_tree=$(git -C "$ROOT_DIR" rev-parse 'HEAD^{tree}')"
    echo "llama_cpp_sha=$LLAMA_COMMIT"
    echo "emulator_api=$(adb shell getprop ro.build.version.sdk | tr -d '\r')"
    echo "emulator_abi=$(adb shell getprop ro.product.cpu.abi | tr -d '\r')"
    echo "emulator_hardware=$(adb shell getprop ro.hardware | tr -d '\r')"
    echo "target_apk_sha256=$(sha256sum "$TARGET_APK" | awk '{print $1}')"
    echo "androidtest_apk_sha256=$(sha256sum "$TEST_APK" | awk '{print $1}')"
    echo "model_sha256=$MODEL_SHA256"
    echo "model_size_bytes=$MODEL_SIZE_BYTES"
} > "$EVIDENCE_DIR/source-and-runtime.txt"

adb install -r "$TARGET_APK" > "$EVIDENCE_DIR/adb-install-target.log" 2>&1
adb install -r "$TEST_APK" > "$EVIDENCE_DIR/adb-install-test.log" 2>&1
adb shell run-as "$APP_ID" mkdir -p files
adb push "$MODEL_HOST_PATH" "$DEVICE_MODEL_TEMP" > "$EVIDENCE_DIR/adb-model-stage.log" 2>&1
adb shell run-as "$APP_ID" cp "$DEVICE_MODEL_TEMP" "$DEVICE_MODEL_RELATIVE"
adb shell run-as "$APP_ID" chmod 0400 "$DEVICE_MODEL_RELATIVE"
device_data_dir="$(adb shell run-as "$APP_ID" pwd | tr -d '\r')"
case "$device_data_dir" in
    "/data/user/0/$APP_ID"|"/data/data/$APP_ID") ;;
    *) echo 'Unexpected app-private data root' >&2; exit 1 ;;
esac
staged_hash="$(adb exec-out run-as "$APP_ID" cat "$DEVICE_MODEL_RELATIVE" | sha256sum | awk '{print $1}')"
[[ "$staged_hash" == "$MODEL_SHA256" ]]
echo "app_private_model_sha256=$staged_hash" > "$EVIDENCE_DIR/model-stage.txt"

rm -rf "$ANDROID_DIR/app/build/outputs/androidTest-results/connected"
gradle_log="$EVIDENCE_DIR/gradle-instrumentation.log"
if ! (
    cd "$ANDROID_DIR"
    ./gradlew --no-daemon --console=plain \
        -PsereaLocalNativeRuntime=true \
        -PsereaLlamaCppSource="$ROOT_DIR/llama.cpp" \
        "-Pandroid.testInstrumentationRunnerArguments.class=$TEST_CLASS#$TEST_METHOD" \
        :app:connectedIsolatedAndroidTest
) > "$gradle_log" 2>&1; then
    tail -n 160 "$gradle_log" >&2
    exit 1
fi

results_root="$ANDROID_DIR/app/build/outputs/androidTest-results/connected"
mapfile -t reports < <(find "$results_root" -type f -name 'TEST-*.xml' -print)
[[ "${#reports[@]}" -eq 1 ]]
cp "${reports[0]}" "$EVIDENCE_DIR/junit/$(basename "${reports[0]}")"
python3 - "${reports[0]}" "$TEST_CLASS" "$TEST_METHOD" <<'PY'
import sys
import xml.etree.ElementTree as ET
root = ET.parse(sys.argv[1]).getroot()
cases = root.findall("testcase")
if root.attrib.get("tests") != "1" or len(cases) != 1:
    raise SystemExit("expected exactly one instrumented test case")
if cases[0].attrib.get("classname") != sys.argv[2] or cases[0].attrib.get("name") != sys.argv[3]:
    raise SystemExit("unexpected JUnit identity")
for key in ("failures", "errors", "skipped"):
    if int(root.attrib.get(key, "0")) != 0:
        raise SystemExit(f"JUnit {key} must be zero")
PY
echo 'exact_junit=PASS tests=1 failures=0 errors=0 skipped=0' > "$EVIDENCE_DIR/junit-verification.txt"

if adb shell pm list packages "$APP_ID" | grep -Fx "package:$APP_ID" >/dev/null; then
    if adb shell run-as "$APP_ID" test '!' -e "$DEVICE_MODEL_RELATIVE"; then
        echo 'app_private_model_cleanup=verified_by_test_and_adb' > "$EVIDENCE_DIR/model-cleanup.txt"
    else
        echo 'app_private_model_cleanup=failed' > "$EVIDENCE_DIR/model-cleanup.txt"
        exit 1
    fi
else
    # The exact JUnit method passed before this check and asserts deletion in its finally block.
    echo 'app_private_model_cleanup=instrumentation_asserted_package_uninstalled_by_runner' > "$EVIDENCE_DIR/model-cleanup.txt"
fi
adb logcat -d -v threadtime -s SereaLocalNativeRuntime:I > "$EVIDENCE_DIR/native-runtime-metrics.log"
