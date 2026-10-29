#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(git rev-parse --show-toplevel)"
ANDROID_DIR="$ROOT_DIR/android"
MODEL_HOST_PATH="${RUNNER_TEMP:?RUNNER_TEMP is required}/serea-g8-smollm2.gguf"
APP_ID='app.serea.android.isolated'
TEST_ID='app.serea.android.isolated.test'
DEVICE_MODEL_RELATIVE='cache/g8-smollm2-135m-q4_k_m.gguf'
DEVICE_MODEL_TEMP='/data/local/tmp/serea-g8-smollm2.gguf'
TARGET_APK="$ANDROID_DIR/app/build/outputs/apk/isolated/app-isolated.apk"
TEST_APK="$ANDROID_DIR/app/build/outputs/apk/androidTest/isolated/app-isolated-androidTest.apk"
MODEL_SHA256='e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5'
MODEL_SIZE_BYTES='101016128'
LLAMA_COMMIT='276378515ceebee5d14bcc8536603dd5c87491a2'
TEST_CLASS='app.serea.android.LocalModelNativeRuntimeE2ETest'
TEST_METHOD='pinnedModelRunsThroughLocalModelEgressUsageAndNativeBackend'
NOOP_TEST_CLASS='app.serea.android.SereaNativeRunnerNoOpTest'
NOOP_TEST_METHOD='androidJUnit4RunnerInstantiatesAndTargetsIsolatedApp'
TEST_RUNNER='app.serea.android.SereaIsolatedAndroidJUnitRunner'
EVIDENCE_DIR=''
CANONICAL_RUNNER_TEMP=''
DEVICE_SERIAL=''
EVIDENCE_DIR_CREATED=0
APP_PACKAGE_OWNED=0
TEST_PACKAGE_OWNED=0
APP_PRIVATE_MODEL_OWNED=0
DEVICE_TEMP_MODEL_OWNED=0
HOST_MODEL_OWNED=0

adb_device() {
    adb -s "$DEVICE_SERIAL" "$@"
}

package_state() {
    local package="$1"
    local include_uninstalled="${2:-0}"
    local packages
    local -a query=(shell pm list packages)
    if [[ "$include_uninstalled" == 1 ]]; then query+=(-u); fi
    query+=("$package")
    if ! packages="$(adb_device "${query[@]}" 2>/dev/null)"; then
        return 2
    fi
    packages="$(printf '%s' "$packages" | tr -d '\r')"
    if [[ "$packages" == "package:$package" ]]; then
        return 0
    fi
    if [[ -z "$packages" ]]; then
        return 1
    fi
    return 2
}

uninstall_and_verify() {
    local package="$1"
    local owned="$2"
    local command_variable="$3"
    local state_variable="$4"
    local before_state after_state command_status
    local -n command_result="$command_variable"
    local -n final_state="$state_variable"

    if [[ "$owned" != 1 ]]; then
        if package_state "$package" 1; then
            command_result=NOT_TOUCHED_UNEXPECTED_PRESENT
            final_state=NOT_VERIFIED
            CLEANUP_FAILED=1
        else
            command_status=$?
            if [[ "$command_status" == 1 ]]; then
                command_result=NOT_NEEDED_NOT_OWNED
                final_state=ABSENT_VERIFIED
            else
                command_result=NOT_VERIFIED_PACKAGE_QUERY_FAILED
                final_state=NOT_VERIFIED
                CLEANUP_FAILED=1
            fi
        fi
        return
    fi

    # Use the installed-only view to decide whether `adb uninstall` is appropriate.
    # The `-u` view is reserved for final verification of installed and retained records.
    if package_state "$package" 0; then
        before_state=INSTALLED
    else
        command_status=$?
        case "$command_status" in
            1) before_state=ABSENT ;;
            *) before_state=UNKNOWN ;;
        esac
    fi

    if [[ "$before_state" == INSTALLED ]]; then
        if adb_device uninstall "$package" >/dev/null 2>&1; then
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

    if package_state "$package" 1; then
        after_state=INSTALLED_OR_RETAINED
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
    local app_private_owner_package_state=NOT_CHECKED
    local test_uninstall_command=NOT_ATTEMPTED test_package_state=NOT_VERIFIED
    local app_uninstall_command=NOT_ATTEMPTED app_package_state=NOT_VERIFIED
    local report_path report_tmp report_write_status=PASS cleanup_result output_write_status=NOT_ATTEMPTED
    local -a cleanup_report
    CLEANUP_FAILED=0

    trap - EXIT
    set +e

    if [[ "$APP_PRIVATE_MODEL_OWNED" == 1 ]]; then
        if [[ "$APP_PACKAGE_OWNED" != 1 ]]; then
            app_private_owner_package_state=NOT_OWNED
            app_private_remove_command=NOT_VERIFIED_OWNER_PACKAGE_NOT_OWNED
            app_private_state=NOT_VERIFIED
            CLEANUP_FAILED=1
        elif package_state "$APP_ID"; then
            app_private_owner_package_state=INSTALLED
            if adb_device shell run-as "$APP_ID" rm -f "$DEVICE_MODEL_RELATIVE" >/dev/null 2>&1; then
                app_private_remove_command=PASS
            else
                app_private_remove_command=FAIL
                CLEANUP_FAILED=1
            fi
            if [[ "$app_private_remove_command" == PASS ]] && \
                adb_device shell run-as "$APP_ID" test '!' -e "$DEVICE_MODEL_RELATIVE" >/dev/null 2>&1; then
                app_private_state=ABSENT_VERIFIED
            else
                app_private_state=NOT_VERIFIED
                CLEANUP_FAILED=1
            fi
        else
            package_status=$?
            if [[ "$package_status" == 1 ]]; then
                app_private_owner_package_state=ABSENT
                app_private_remove_command=NOT_VERIFIED_OWNER_PACKAGE_ABSENT
            else
                app_private_owner_package_state=QUERY_FAILED
                app_private_remove_command=NOT_VERIFIED_OWNER_PACKAGE_QUERY_FAILED
            fi
            app_private_state=NOT_VERIFIED
            CLEANUP_FAILED=1
        fi
    else
        if [[ "$APP_PACKAGE_OWNED" == 1 ]] && package_state "$APP_ID"; then
            app_private_owner_package_state=INSTALLED
            if adb_device shell run-as "$APP_ID" test '!' -e "$DEVICE_MODEL_RELATIVE" >/dev/null 2>&1; then
                app_private_remove_command=NOT_NEEDED_NOT_STAGED
                app_private_state=ABSENT_VERIFIED
            else
                app_private_remove_command=NOT_VERIFIED_NOT_STAGED
                app_private_state=NOT_VERIFIED
                CLEANUP_FAILED=1
            fi
        else
            if [[ "$APP_PACKAGE_OWNED" != 1 ]]; then
                app_private_owner_package_state=NOT_OWNED
                app_private_remove_command=NOT_APPLICABLE
                app_private_state=NOT_APPLICABLE
            else
                package_status=$?
                if [[ "$package_status" == 1 ]]; then
                    app_private_owner_package_state=ABSENT
                    app_private_remove_command=NOT_VERIFIED_OWNER_PACKAGE_ABSENT
                else
                    app_private_owner_package_state=QUERY_FAILED
                    app_private_remove_command=NOT_VERIFIED_OWNER_PACKAGE_QUERY_FAILED
                fi
                app_private_state=NOT_VERIFIED
                CLEANUP_FAILED=1
            fi
        fi
    fi

    if [[ "$DEVICE_TEMP_MODEL_OWNED" == 1 ]]; then
        if adb_device shell rm -f "$DEVICE_MODEL_TEMP" >/dev/null 2>&1; then
            temp_remove_command=PASS
        else
            temp_remove_command=FAIL
            CLEANUP_FAILED=1
        fi
    else
        temp_remove_command=NOT_NEEDED_NOT_STAGED
    fi
    if [[ "$temp_remove_command" == PASS || "$temp_remove_command" == NOT_NEEDED_NOT_STAGED ]] && \
        adb_device shell test '!' -e "$DEVICE_MODEL_TEMP" >/dev/null 2>&1; then
        temp_state=ABSENT_VERIFIED
    else
        temp_state=NOT_VERIFIED
        CLEANUP_FAILED=1
    fi

    uninstall_and_verify "$TEST_ID" "$TEST_PACKAGE_OWNED" test_uninstall_command test_package_state
    uninstall_and_verify "$APP_ID" "$APP_PACKAGE_OWNED" app_uninstall_command app_package_state

    if [[ "$HOST_MODEL_OWNED" == 1 ]] && rm -f -- "$MODEL_HOST_PATH"; then
        host_remove_command=PASS
    elif [[ "$HOST_MODEL_OWNED" != 1 ]]; then
        host_remove_command=NOT_OWNED
    else
        host_remove_command=FAIL
        CLEANUP_FAILED=1
    fi
    if [[ "$host_remove_command" == NOT_OWNED ]]; then
        host_absence=NOT_APPLICABLE
    elif [[ "$host_remove_command" == PASS && ! -e "$MODEL_HOST_PATH" ]]; then
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
        "app_private_owner_package_state=$app_private_owner_package_state"
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
        ! grep -Fxq "app_private_owner_package_state=$app_private_owner_package_state" "$report_tmp" || \
        ! grep -Fxq "app_private_model_final_state=$app_private_state" "$report_tmp" || \
        ! grep -Fxq "device_temp_model_final_state=$temp_state" "$report_tmp" || \
        ! grep -Fxq "test_package_final_state=$test_package_state" "$report_tmp" || \
        ! grep -Fxq "app_package_final_state=$app_package_state" "$report_tmp"; then
        report_write_status=FAIL
    elif ! mv "$report_tmp" "$report_path"; then
        report_write_status=FAIL
    elif ! grep -Fxq "cleanup_outcome=$cleanup_result" "$report_path" || \
        ! grep -Fxq "host_model_final_state=$host_absence" "$report_path" || \
        ! grep -Fxq "app_private_owner_package_state=$app_private_owner_package_state" "$report_path" || \
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
        elif printf 'evidence_dir=%s\ncleanup_verified=true\n' "$EVIDENCE_DIR" >> "$GITHUB_OUTPUT" && \
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
            "app_private_owner_package_state=$app_private_owner_package_state" \
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
        if [[ "$EVIDENCE_DIR_CREATED" != 1 || -L "$EVIDENCE_DIR" || \
              "$(cd "$EVIDENCE_DIR/.." 2>/dev/null && pwd -P)" != "$CANONICAL_RUNNER_TEMP" ]] || \
              ! rm -rf -- "$EVIDENCE_DIR" || [[ -e "$EVIDENCE_DIR" ]]; then
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

if [[ "${SEREA_CLEANUP_TEST_MODE:-0}" == 1 ]]; then
    test_marker="$RUNNER_TEMP/.serea-cleanup-test-root"
    expected_adb="$RUNNER_TEMP/fakebin/adb"
    expected_rm="$RUNNER_TEMP/fakebin/rm"
    [[ -f "$test_marker" && "$(cat "$test_marker")" == 'serea-cleanup-test-v1' ]]
    [[ "$(realpath "$(command -v adb)")" == "$(realpath "$expected_adb")" ]]
    [[ "$(realpath "$(command -v rm)")" == "$(realpath "$expected_rm")" ]]
    DEVICE_SERIAL='fake-test-target'
    EVIDENCE_DIR="$RUNNER_TEMP/serea-g8-native-cleanup-test"
    mkdir -p "$EVIDENCE_DIR/junit"
    CANONICAL_RUNNER_TEMP="$(cd "$RUNNER_TEMP" && pwd -P)"
    EVIDENCE_DIR_CREATED=1
    APP_PACKAGE_OWNED=1
    TEST_PACKAGE_OWNED=1
    APP_PRIVATE_MODEL_OWNED=1
    DEVICE_TEMP_MODEL_OWNED=1
    HOST_MODEL_OWNED=1
    trap 'cleanup "$?"' EXIT
    exit "${SEREA_CLEANUP_TEST_PRIMARY_STATUS:-0}"
fi

if [[ -n "${G8_EVIDENCE_DIR+x}" ]]; then
    echo 'G8_EVIDENCE_DIR is not accepted; evidence must use a fresh runner-temporary directory.' >&2
    exit 2
fi

[[ -s "$TARGET_APK" ]]
[[ -s "$TEST_APK" ]]
[[ -s "$MODEL_HOST_PATH" ]]
test "$(stat -c '%s' "$MODEL_HOST_PATH")" = "$MODEL_SIZE_BYTES"
test "$(sha256sum "$MODEL_HOST_PATH" | awk '{print $1}')" = "$MODEL_SHA256"

preflight_output="$(bash "$ROOT_DIR/tools/g8-local-model-native/preflight.sh")"
DEVICE_SERIAL="$(sed -n 's/^device_serial=//p' <<< "$preflight_output")"
hardware="$(sed -n 's/^hardware=//p' <<< "$preflight_output")"
fingerprint="$(sed -n 's/^fingerprint=//p' <<< "$preflight_output")"
[[ "$DEVICE_SERIAL" =~ ^emulator-[0-9]+$ ]]
ANDROID_SERIAL="$DEVICE_SERIAL"
export ANDROID_SERIAL

mkdir -p "$RUNNER_TEMP"
CANONICAL_RUNNER_TEMP="$(cd "$RUNNER_TEMP" && pwd -P)"
EVIDENCE_DIR="$(mktemp -d "$CANONICAL_RUNNER_TEMP/serea-g8-local-native.XXXXXX")"
if [[ -L "$EVIDENCE_DIR" || "$(cd "$EVIDENCE_DIR/.." && pwd -P)" != "$CANONICAL_RUNNER_TEMP" ]]; then
    echo 'Fresh evidence directory did not resolve directly under RUNNER_TEMP.' >&2
    exit 4
fi
EVIDENCE_DIR_CREATED=1
mkdir -p "$EVIDENCE_DIR/junit"
HOST_MODEL_OWNED=1
trap 'cleanup "$?"' EXIT
printf '%s\n' "$preflight_output" > "$EVIDENCE_DIR/preflight.txt"
{
    echo "source_sha=$(git -C "$ROOT_DIR" rev-parse HEAD)"
    echo "source_tree=$(git -C "$ROOT_DIR" rev-parse 'HEAD^{tree}')"
    echo "llama_cpp_sha=$LLAMA_COMMIT"
    echo "emulator_serial=$DEVICE_SERIAL"
    echo "emulator_api=$(adb_device shell getprop ro.build.version.sdk | tr -d '\r')"
    echo "emulator_abi=$(adb_device shell getprop ro.product.cpu.abi | tr -d '\r')"
    echo "emulator_hardware=$hardware"
    echo "emulator_fingerprint=$fingerprint"
    echo "target_apk_sha256=$(sha256sum "$TARGET_APK" | awk '{print $1}')"
    echo "androidtest_apk_sha256=$(sha256sum "$TEST_APK" | awk '{print $1}')"
    echo "model_sha256=$MODEL_SHA256"
    echo "model_size_bytes=$MODEL_SIZE_BYTES"
} > "$EVIDENCE_DIR/source-and-runtime.txt"

campaign_token="g8-native-${GITHUB_RUN_ID:?}-${GITHUB_RUN_ATTEMPT:?}"
if adb_device install "$TARGET_APK" > "$EVIDENCE_DIR/adb-install-target.log" 2>&1; then
    APP_PACKAGE_OWNED=1
else
    echo 'Target APK install failed; it will not be treated as an owned package.' >&2
    exit 1
fi
if adb_device install "$TEST_APK" > "$EVIDENCE_DIR/adb-install-test.log" 2>&1; then
    TEST_PACKAGE_OWNED=1
else
    echo 'AndroidTest APK install failed; it will not be treated as an owned package.' >&2
    exit 1
fi
EXPECTED_API_LEVEL=35 bash "$ROOT_DIR/tools/prepare_isolated_instrumentation_target.sh" "$campaign_token" \
    > "$EVIDENCE_DIR/isolated-target-seed.log" 2>&1
device_data_dir="$(adb_device shell run-as "$APP_ID" pwd | tr -d '\r')"
case "$device_data_dir" in
    "/data/user/0/$APP_ID"|"/data/data/$APP_ID") ;;
    *) echo 'Unexpected app-private data root' >&2; exit 1 ;;
esac
DEVICE_TEMP_MODEL_OWNED=1
adb_device push "$MODEL_HOST_PATH" "$DEVICE_MODEL_TEMP" > "$EVIDENCE_DIR/adb-model-stage.log" 2>&1
APP_PRIVATE_MODEL_OWNED=1
adb_device shell run-as "$APP_ID" mkdir -p cache
adb_device shell run-as "$APP_ID" cp "$DEVICE_MODEL_TEMP" "$DEVICE_MODEL_RELATIVE"
adb_device shell run-as "$APP_ID" chmod 0400 "$DEVICE_MODEL_RELATIVE"
staged_hash="$(adb_device exec-out run-as "$APP_ID" cat "$DEVICE_MODEL_RELATIVE" | sha256sum | awk '{print $1}')"
[[ "$staged_hash" == "$MODEL_SHA256" ]]
echo "app_private_model_sha256=$staged_hash" > "$EVIDENCE_DIR/model-stage.txt"

# Invoke the already-installed instrumentation component directly. The previous connected-test
# attempt ended with both APKs absent, so the script could not verify app-private fixture cleanup or
# retain JUnit/logcat cause details. This direct invocation keeps the disposable target under this
# script's ownership until cleanup and evidence verification complete.
instrumentation_listing="$(adb_device shell pm list instrumentation | tr -d '\r')"
instrumentation_component="$TEST_ID/$TEST_RUNNER"
if ! grep -Fq "instrumentation:$instrumentation_component (target=$APP_ID)" <<< "$instrumentation_listing"; then
    printf '%s\n' "$instrumentation_listing" > "$EVIDENCE_DIR/instrumentation-manifest.txt"
    echo 'Expected isolated instrumentation component is absent from Package Manager.' >&2
    exit 1
fi
printf '%s\n' "$instrumentation_listing" > "$EVIDENCE_DIR/instrumentation-manifest.txt"

adb_device logcat -c
instrumentation_log="$EVIDENCE_DIR/android-instrumentation.txt"
class_filter="$NOOP_TEST_CLASS#$NOOP_TEST_METHOD,$TEST_CLASS#$TEST_METHOD"
set +e
timeout --signal=TERM --kill-after=20s 30m \
    adb -s "$DEVICE_SERIAL" shell am instrument -w -r \
    -e sereaCampaignToken "$campaign_token" \
    -e class "$class_filter" \
    "$instrumentation_component" > "$instrumentation_log" 2>&1
instrumentation_status=$?
set -e

timeout_cleanup=NOT_NEEDED
if [[ "$instrumentation_status" == 124 || "$instrumentation_status" == 137 ]]; then
    if [[ "$TEST_PACKAGE_OWNED" == 1 && "$APP_PACKAGE_OWNED" == 1 ]] && \
        adb_device shell am force-stop "$TEST_ID" >/dev/null 2>&1 && \
        adb_device shell am force-stop "$APP_ID" >/dev/null 2>&1; then
        timeout_cleanup=PASS
    else
        timeout_cleanup=FAIL
    fi
fi

logcat_status=PASS
if ! adb_device logcat -d -v threadtime '*:E' \
    'SereaIsolatedRunner:I' 'SereaTestLifecycle:I' 'SereaLocalNativeRuntime:I' \
    > "$EVIDENCE_DIR/android-logcat-diagnostic.log"; then
    logcat_status=FAIL
fi
if [[ -s "$EVIDENCE_DIR/android-logcat-diagnostic.log" ]]; then
    tail -c 1048576 "$EVIDENCE_DIR/android-logcat-diagnostic.log" \
        > "$EVIDENCE_DIR/android-logcat-diagnostic.log.bounded"
    mv "$EVIDENCE_DIR/android-logcat-diagnostic.log.bounded" \
        "$EVIDENCE_DIR/android-logcat-diagnostic.log"
fi

parser_status=0
python3 "$ROOT_DIR/tools/g8-local-model-native/verify-instrumentation-stream.py" \
    "$instrumentation_log" \
    "$EVIDENCE_DIR/junit/native-runtime-instrumentation.xml" \
    "$EVIDENCE_DIR/junit-verification.txt" \
    "$NOOP_TEST_CLASS" "$TEST_CLASS" || parser_status=$?
printf 'adb_instrumentation_exit=%s\nlogcat_capture=%s\nstream_parse_exit=%s\ntimeout_cleanup=%s\n' \
    "$instrumentation_status" "$logcat_status" "$parser_status" "$timeout_cleanup" \
    > "$EVIDENCE_DIR/instrumentation-command-status.txt"
if [[ "$instrumentation_status" == 124 || "$instrumentation_status" == 137 ]]; then
    echo 'instrumentation_timeout=EXPIRED' >> "$EVIDENCE_DIR/instrumentation-command-status.txt"
else
    echo 'instrumentation_timeout=NOT_EXPIRED' >> "$EVIDENCE_DIR/instrumentation-command-status.txt"
fi

source "$ROOT_DIR/tools/g8-local-model-native/instrumentation-gate.sh"
if ! instrumentation_gate_passes "$instrumentation_status" "$logcat_status" \
    "$parser_status" "$timeout_cleanup" "$EVIDENCE_DIR/junit-verification.txt"; then
    echo 'Android instrumentation diagnostic or native inference did not pass; captured output follows.' >&2
    tail -n 240 "$instrumentation_log" >&2 || true
    tail -n 240 "$EVIDENCE_DIR/android-logcat-diagnostic.log" >&2 || true
    exit 1
fi

adb_device logcat -d -v threadtime -s SereaLocalNativeRuntime:I > "$EVIDENCE_DIR/native-runtime-metrics.log"
