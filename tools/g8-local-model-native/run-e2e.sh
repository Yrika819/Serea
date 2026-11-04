#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(git rev-parse --show-toplevel)"
ANDROID_DIR="$ROOT_DIR/android"
EVIDENCE_DIR="${G8_EVIDENCE_DIR:-$ROOT_DIR/evidence/g8-local-native}"
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

package_state() {
    local package="$1"
    local packages
    if ! packages="$(adb shell pm list packages "$package" 2>/dev/null)"; then
        return 2
    fi
    if printf '%s\n' "$packages" | tr -d '\r' | grep -Fxq "package:$package"; then
        return 0
    fi
    return 1
}

uninstall_and_verify() {
    local package="$1"
    local command_variable="$2"
    local state_variable="$3"
    local before_state after_state command_status
    local -n command_result="$command_variable"
    local -n final_state="$state_variable"

    if package_state "$package"; then
        before_state=INSTALLED
    else
        command_status=$?
        case "$command_status" in
            1) before_state=ABSENT ;;
            *) before_state=UNKNOWN ;;
        esac
    fi

    if [[ "$before_state" == INSTALLED ]]; then
        if adb uninstall "$package" >/dev/null 2>&1; then
            command_result=PASS
        else
            command_result=FAIL
            CLEANUP_FAILED=1
        fi
    elif [[ "$before_state" == ABSENT ]]; then
        command_result=NOT_NEEDED_ALREADY_ABSENT
    else
        command_result=NOT_VERIFIED_PACKAGE_QUERY_FAILED
        CLEANUP_FAILED=1
    fi

    if package_state "$package"; then
        after_state=INSTALLED
    else
        command_status=$?
        case "$command_status" in
            1) after_state=ABSENT ;;
            *) after_state=UNKNOWN ;;
        esac
    fi

    if [[ "$after_state" == ABSENT && "$before_state" != UNKNOWN && "$command_result" != FAIL ]]; then
        final_state=ABSENT_VERIFIED
    else
        final_state=NOT_VERIFIED
        CLEANUP_FAILED=1
    fi
}

cleanup() {
    local original_status="$1"
    local package_status app_private_remove_command temp_remove_command
    local host_remove_command host_absence
    local app_private_state=NOT_VERIFIED temp_state=NOT_VERIFIED
    local test_uninstall_command=NOT_ATTEMPTED test_package_state=NOT_VERIFIED
    local app_uninstall_command=NOT_ATTEMPTED app_package_state=NOT_VERIFIED
    local report_path report_tmp report_write_status=PASS cleanup_result output_write_status=NOT_ATTEMPTED
    local -a cleanup_report
    CLEANUP_FAILED=0

    trap - EXIT
    set +e

    if package_state "$APP_ID"; then
        if adb shell run-as "$APP_ID" rm -f "$DEVICE_MODEL_RELATIVE" >/dev/null 2>&1; then
            app_private_remove_command=PASS
        else
            app_private_remove_command=FAIL
            CLEANUP_FAILED=1
        fi
        if [[ "$app_private_remove_command" == PASS ]] && \
            adb shell run-as "$APP_ID" test '!' -e "$DEVICE_MODEL_RELATIVE" >/dev/null 2>&1; then
            app_private_state=ABSENT_VERIFIED
        else
            app_private_state=NOT_VERIFIED
            CLEANUP_FAILED=1
        fi
    else
        package_status=$?
        if [[ "$package_status" == 1 ]]; then
            if package_state "$APP_ID"; then
                app_private_state=NOT_VERIFIED_PACKAGE_PRESENT
                CLEANUP_FAILED=1
            else
                package_status=$?
                if [[ "$package_status" == 1 ]]; then
                    app_private_remove_command=NOT_NEEDED_PACKAGE_ABSENT
                    app_private_state=NOT_APPLICABLE_PACKAGE_ABSENT
                else
                    app_private_remove_command=NOT_VERIFIED_PACKAGE_QUERY_FAILED
                    app_private_state=NOT_VERIFIED
                    CLEANUP_FAILED=1
                fi
            fi
        else
            app_private_remove_command=NOT_VERIFIED_PACKAGE_QUERY_FAILED
            app_private_state=NOT_VERIFIED
            CLEANUP_FAILED=1
        fi
    fi

    if adb shell rm -f "$DEVICE_MODEL_TEMP" >/dev/null 2>&1; then
        temp_remove_command=PASS
    else
        temp_remove_command=FAIL
        CLEANUP_FAILED=1
    fi
    if [[ "$temp_remove_command" == PASS ]] && \
        adb shell test '!' -e "$DEVICE_MODEL_TEMP" >/dev/null 2>&1; then
        temp_state=ABSENT_VERIFIED
    else
        temp_state=NOT_VERIFIED
        CLEANUP_FAILED=1
    fi

    uninstall_and_verify "$TEST_ID" test_uninstall_command test_package_state
    uninstall_and_verify "$APP_ID" app_uninstall_command app_package_state

    if rm -f "$MODEL_HOST_PATH"; then
        host_remove_command=PASS
    else
        host_remove_command=FAIL
        CLEANUP_FAILED=1
    fi
    if [[ "$host_remove_command" == PASS && ! -e "$MODEL_HOST_PATH" ]]; then
        host_absence=ABSENT_VERIFIED
    else
        host_absence=NOT_VERIFIED
        CLEANUP_FAILED=1
    fi

    if ((CLEANUP_FAILED == 0)); then
        cleanup_result=PASS
    else
        cleanup_result=FAIL
    fi
    report_path="$EVIDENCE_DIR/cleanup-boundary.txt"
    report_tmp="$report_path.tmp"
    cleanup_report=(
        'model_weights_uploaded=false'
        "host_model_rm_command=$host_remove_command"
        "host_model_final_state=$host_absence"
        "app_private_model_rm_command=$app_private_remove_command"
        "app_private_model_final_state=$app_private_state"
        "device_temp_model_rm_command=$temp_remove_command"
        "device_temp_model_final_state=$temp_state"
        "test_package_uninstall_command=$test_uninstall_command"
        "test_package_final_state=$test_package_state"
        "app_package_uninstall_command=$app_uninstall_command"
        "app_package_final_state=$app_package_state"
        "cleanup_outcome=$cleanup_result"
        "original_exit_status=$original_status"
        'runner_vm_and_disposable_avd=ephemeral'
    )
    if ! printf '%s\n' "${cleanup_report[@]}" > "$report_tmp"; then
        report_write_status=FAIL
    elif ! grep -Fxq "cleanup_outcome=$cleanup_result" "$report_tmp" || \
        ! grep -Fxq "original_exit_status=$original_status" "$report_tmp" || \
        ! grep -Fxq "host_model_final_state=$host_absence" "$report_tmp" || \
        ! grep -Fxq "app_private_model_final_state=$app_private_state" "$report_tmp" || \
        ! grep -Fxq "device_temp_model_final_state=$temp_state" "$report_tmp" || \
        ! grep -Fxq "test_package_final_state=$test_package_state" "$report_tmp" || \
        ! grep -Fxq "app_package_final_state=$app_package_state" "$report_tmp"; then
        report_write_status=FAIL
    elif ! mv "$report_tmp" "$report_path"; then
        report_write_status=FAIL
    elif ! grep -Fxq "cleanup_outcome=$cleanup_result" "$report_path" || \
        ! grep -Fxq "host_model_final_state=$host_absence" "$report_path" || \
        ! grep -Fxq "app_private_model_final_state=$app_private_state" "$report_path" || \
        ! grep -Fxq "device_temp_model_final_state=$temp_state" "$report_path" || \
        ! grep -Fxq "test_package_final_state=$test_package_state" "$report_path" || \
        ! grep -Fxq "app_package_final_state=$app_package_state" "$report_path"; then
        report_write_status=FAIL
    fi
    if [[ "$report_write_status" != PASS ]]; then
        cleanup_result=FAIL
        CLEANUP_FAILED=1
    fi

    if ((CLEANUP_FAILED == 0)) && [[ "$cleanup_result" == PASS && "$report_write_status" == PASS ]]; then
        if [[ -z "${GITHUB_OUTPUT:-}" ]]; then
            output_write_status=FAIL_NO_GITHUB_OUTPUT
            CLEANUP_FAILED=1
        elif printf '%s\n' 'cleanup_verified=true' >> "$GITHUB_OUTPUT" && \
            grep -Fxq 'cleanup_verified=true' "$GITHUB_OUTPUT"; then
            output_write_status=PASS
        else
            output_write_status=FAIL
            CLEANUP_FAILED=1
        fi
    fi

    if ((CLEANUP_FAILED != 0)); then
        echo 'Cleanup verification failed; no success evidence will be retained:' >&2
        printf '%s\n' \
            'model_weights_uploaded=false' \
            "host_model_rm_command=$host_remove_command" \
            "host_model_final_state=$host_absence" \
            "app_private_model_rm_command=$app_private_remove_command" \
            "app_private_model_final_state=$app_private_state" \
            "device_temp_model_rm_command=$temp_remove_command" \
            "device_temp_model_final_state=$temp_state" \
            "test_package_uninstall_command=$test_uninstall_command" \
            "test_package_final_state=$test_package_state" \
            "app_package_uninstall_command=$app_uninstall_command" \
            "app_package_final_state=$app_package_state" \
            "cleanup_report_write=$report_write_status" \
            "cleanup_verified_output=$output_write_status" \
            'cleanup_outcome=FAIL' \
            "original_exit_status=$original_status" >&2
        # The workflow uploader is configured with `always()`. Remove its
        # evidence source on cleanup failure so it cannot publish success evidence.
        if ! rm -rf "$EVIDENCE_DIR" || [[ -e "$EVIDENCE_DIR" ]]; then
            echo 'cleanup_evidence_source_removal=NOT_VERIFIED' >&2
        else
            echo 'cleanup_evidence_source_removal=VERIFIED' >&2
        fi
        if ((original_status == 0)); then
            exit 1
        fi
        exit "$original_status"
    fi

    exit "$original_status"
}
trap 'cleanup "$?"' EXIT

if [[ "${SEREA_CLEANUP_TEST_MODE:-0}" == 1 ]]; then
    exit "${SEREA_CLEANUP_TEST_PRIMARY_STATUS:-0}"
fi

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

adb logcat -d -v threadtime -s SereaLocalNativeRuntime:I > "$EVIDENCE_DIR/native-runtime-metrics.log"
