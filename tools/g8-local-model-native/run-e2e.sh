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
EXPECTED_NATIVE_ABI="${SEREA_EXPECTED_NATIVE_ABI:-x86_64}"
[[ "$EXPECTED_NATIVE_ABI" == 'x86_64' || "$EXPECTED_NATIVE_ABI" == 'arm64-v8a' ]]
export SEREA_EXPECTED_NATIVE_ABI="$EXPECTED_NATIVE_ABI"
EVIDENCE_DIR=''
CANONICAL_RUNNER_TEMP=''
DEVICE_SERIAL=''
EVIDENCE_DIR_CREATED=0
APP_PACKAGE_OWNED=0
TEST_PACKAGE_OWNED=0
APP_PRIVATE_MODEL_OWNED=0
DEVICE_TEMP_MODEL_OWNED=0
HOST_MODEL_OWNED=0
PACKAGE_QUERY_EXIT_CODE=NOT_RUN
PACKAGE_QUERY_MATCHES=NOT_RUN
PACKAGE_QUERY_DIAGNOSTIC=NOT_RUN
DEVICE_CONNECTIVITY_STATE=NOT_CHECKED
DEVICE_CONNECTIVITY_EXIT_CODE=NOT_RUN
CURRENT_STAGE=SCRIPT_INIT
PRIMARY_FAILURE_STATUS=0
PRIMARY_FAILURE_LINE=NOT_REPORTED
PRIMARY_FAILURE_STAGE=NOT_REPORTED
CLEANUP_IN_PROGRESS=0

record_primary_failure() {
    local status="$1" line="$2"
    if [[ "$CLEANUP_IN_PROGRESS" != 1 ]] && ((PRIMARY_FAILURE_STATUS == 0)); then
        PRIMARY_FAILURE_STATUS="$status"
        PRIMARY_FAILURE_LINE="$line"
        PRIMARY_FAILURE_STAGE="$CURRENT_STAGE"
    fi
    return 0
}

set -E
trap 'record_primary_failure "$?" "$LINENO"' ERR

classify_adb_failure() {
    local message="${1,,}"
    case "$message" in
        *'device offline'*) echo 'DEVICE_OFFLINE' ;;
        *'device not found'*|*'no devices/emulators found'*) echo 'DEVICE_NOT_FOUND' ;;
        *'device unauthorized'*|*'unauthorized'*) echo 'DEVICE_UNAUTHORIZED' ;;
        *'unable to connect to adb daemon'*|*'cannot connect to daemon'*|*'failed to connect to daemon'*) echo 'ADB_DAEMON_UNAVAILABLE' ;;
        *'timed out'*|*'timeout'*) echo 'ADB_COMMAND_TIMEOUT' ;;
        *) echo 'ADB_COMMAND_FAILED' ;;
    esac
}

capture_device_connectivity() {
    local state_output state_status normalized_state
    if [[ -z "$DEVICE_SERIAL" ]]; then
        DEVICE_CONNECTIVITY_STATE=NOT_AVAILABLE_NO_SERIAL
        DEVICE_CONNECTIVITY_EXIT_CODE=NOT_RUN
        return 0
    fi
    if state_output="$(timeout 5s adb -s "$DEVICE_SERIAL" get-state 2>&1)"; then
        DEVICE_CONNECTIVITY_EXIT_CODE=0
        normalized_state="$(printf '%s' "$state_output" | tr -d '\r[:space:]')"
        case "$normalized_state" in
            device) DEVICE_CONNECTIVITY_STATE=DEVICE ;;
            offline) DEVICE_CONNECTIVITY_STATE=OFFLINE ;;
            unauthorized) DEVICE_CONNECTIVITY_STATE=UNAUTHORIZED ;;
            *) DEVICE_CONNECTIVITY_STATE=UNEXPECTED_STATE ;;
        esac
    else
        state_status=$?
        DEVICE_CONNECTIVITY_EXIT_CODE="$state_status"
        DEVICE_CONNECTIVITY_STATE="$(classify_adb_failure "$state_output")"
    fi
}

adb_device() {
    adb -s "$DEVICE_SERIAL" "$@"
}

print_sanitized_instrumentation_summary() {
    local status_file="$EVIDENCE_DIR/instrumentation-command-status.txt"
    local junit_file="$EVIDENCE_DIR/junit-verification.txt"
    if [[ -s "$status_file" ]]; then
        echo 'instrumentation_command_summary_begin' >&2
        grep -E '^(adb_instrumentation_exit|logcat_capture|stream_parse_exit|timeout_cleanup|instrumentation_timeout)=' \
            "$status_file" >&2 || true
        echo 'instrumentation_command_summary_end' >&2
    fi
    if [[ -s "$junit_file" ]]; then
        echo 'instrumentation_junit_summary_begin' >&2
        grep -E '^(evidence_kind|stream_outcome|instrumentation_code|reported_numtests|tests|failures|errors|skipped|noop_result|native_result|native_initialization_error|noop_initialization_error|unexpected_identity_count|terminal_status|native_initialization_exception_types|native_initialization_stack_present|failure_exception_types)=' \
            "$junit_file" >&2 || true
        echo 'instrumentation_junit_summary_end' >&2
    fi
}

package_state() {
    local package="$1"
    local include_uninstalled="${2:-0}"
    local packages line exact_count line_count unique_count invalid_output=0 query_error query_error_file
    local -a query=(shell pm list packages)
    query_error_file="$EVIDENCE_DIR/package-query.stderr"
    # Android's package filter is substring-based; the isolated app ID is a prefix of the
    # isolated test ID. Validate rows and match the exact package record instead of requiring
    # the whole filtered output to contain only one row.
    if [[ "$include_uninstalled" == 1 ]]; then query+=(-u); fi
    query+=("$package")
    : > "$query_error_file"
    if packages="$(adb_device "${query[@]}" 2>"$query_error_file")"; then
        PACKAGE_QUERY_EXIT_CODE=0
        PACKAGE_QUERY_DIAGNOSTIC=NONE
    else
        PACKAGE_QUERY_EXIT_CODE=$?
        query_error="$(cat "$query_error_file" 2>/dev/null || true)"
        PACKAGE_QUERY_DIAGNOSTIC="$(classify_adb_failure "$query_error")"
        PACKAGE_QUERY_MATCHES=QUERY_COMMAND_FAILED
        return 2
    fi
    packages="$(printf '%s' "$packages" | tr -d '\r')"
    if [[ -z "$packages" ]]; then
        PACKAGE_QUERY_MATCHES=EMPTY
        return 1
    fi

    while IFS= read -r line; do
        if [[ ! "$line" =~ ^package:[A-Za-z0-9_.]+$ ]]; then
            invalid_output=1
        fi
    done <<< "$packages"
    if [[ "$invalid_output" == 1 ]]; then
        PACKAGE_QUERY_MATCHES=INVALID_OUTPUT
        return 2
    fi
    PACKAGE_QUERY_MATCHES="$(printf '%s\n' "$packages" | LC_ALL=C sort -u | paste -sd ';' -)"
    line_count="$(printf '%s\n' "$packages" | wc -l | tr -d ' ')"
    unique_count="$(printf '%s\n' "$packages" | LC_ALL=C sort -u | wc -l | tr -d ' ')"
    exact_count="$(grep -Fxc "package:$package" <<< "$packages" || true)"
    if [[ "$line_count" != "$unique_count" || "$exact_count" -gt 1 ]]; then
        return 2
    fi
    if [[ "$exact_count" == 1 ]]; then
        return 0
    fi
    return 1
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
    local app_private_owner_package_query_exit_code=NOT_RUN
    local app_private_owner_package_query_matches=NOT_RUN
    local app_private_owner_package_query_diagnostic=NOT_RUN
    local test_uninstall_command=NOT_ATTEMPTED test_package_state=NOT_VERIFIED
    local app_uninstall_command=NOT_ATTEMPTED app_package_state=NOT_VERIFIED
    local report_path report_tmp report_write_status=PASS cleanup_result output_write_status=NOT_ATTEMPTED
    local raw_diagnostics_removed=true raw_diagnostic
    local -a cleanup_report
    CLEANUP_FAILED=0

    trap - EXIT
    CLEANUP_IN_PROGRESS=1
    set +e
    CURRENT_STAGE=FAILURE_CLEANUP
    if ((PRIMARY_FAILURE_STATUS == 0)) && ((original_status != 0)); then
        PRIMARY_FAILURE_STATUS="$original_status"
        PRIMARY_FAILURE_STAGE=UNCLASSIFIED_EXIT
    fi
    capture_device_connectivity
    for raw_diagnostic in \
        "$EVIDENCE_DIR/android-instrumentation.txt" \
        "$EVIDENCE_DIR/android-logcat-diagnostic.log" \
        "$EVIDENCE_DIR/android-logcat-diagnostic.log.bounded"; do
        if [[ -e "$raw_diagnostic" || -L "$raw_diagnostic" ]]; then
            if ! rm -f -- "$raw_diagnostic" || [[ -e "$raw_diagnostic" || -L "$raw_diagnostic" ]]; then
                raw_diagnostics_removed=false
                CLEANUP_FAILED=1
            fi
        fi
    done

    if [[ "$APP_PRIVATE_MODEL_OWNED" == 1 ]]; then
        if [[ "$APP_PACKAGE_OWNED" != 1 ]]; then
            app_private_owner_package_state=NOT_OWNED
            app_private_remove_command=NOT_VERIFIED_OWNER_PACKAGE_NOT_OWNED
            app_private_state=NOT_VERIFIED
            CLEANUP_FAILED=1
        elif package_state "$APP_ID"; then
            app_private_owner_package_state=INSTALLED
            app_private_owner_package_query_exit_code=$PACKAGE_QUERY_EXIT_CODE
            app_private_owner_package_query_matches=$PACKAGE_QUERY_MATCHES
            app_private_owner_package_query_diagnostic=$PACKAGE_QUERY_DIAGNOSTIC
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
            app_private_owner_package_query_exit_code=$PACKAGE_QUERY_EXIT_CODE
            app_private_owner_package_query_matches=$PACKAGE_QUERY_MATCHES
            app_private_owner_package_query_diagnostic=$PACKAGE_QUERY_DIAGNOSTIC
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
            app_private_owner_package_query_exit_code=$PACKAGE_QUERY_EXIT_CODE
            app_private_owner_package_query_matches=$PACKAGE_QUERY_MATCHES
            app_private_owner_package_query_diagnostic=$PACKAGE_QUERY_DIAGNOSTIC
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
                app_private_owner_package_query_exit_code=$PACKAGE_QUERY_EXIT_CODE
                app_private_owner_package_query_matches=$PACKAGE_QUERY_MATCHES
                app_private_owner_package_query_diagnostic=$PACKAGE_QUERY_DIAGNOSTIC
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
        "app_private_owner_package_query_exit_code=$app_private_owner_package_query_exit_code"
        "app_private_owner_package_query_matches=$app_private_owner_package_query_matches"
        "app_private_owner_package_query_diagnostic=$app_private_owner_package_query_diagnostic"
        "device_connectivity_state=$DEVICE_CONNECTIVITY_STATE"
        "device_connectivity_exit_code=$DEVICE_CONNECTIVITY_EXIT_CODE"
        "primary_failure_status=$PRIMARY_FAILURE_STATUS"
        "primary_failure_line=$PRIMARY_FAILURE_LINE"
        "primary_failure_stage=$PRIMARY_FAILURE_STAGE"
        "app_private_model_rm_command=$app_private_remove_command"
        "app_private_model_final_state=$app_private_state"
        "device_temp_model_rm_command=$temp_remove_command"
        "device_temp_model_final_state=$temp_state"
        "test_package_uninstall_command=$test_uninstall_command"
        "test_package_final_state=$test_package_state"
        "app_package_uninstall_command=$app_uninstall_command"
        "app_package_final_state=$app_package_state"
        "cleanup_outcome=$cleanup_result"
        "raw_diagnostics_removed=$raw_diagnostics_removed"
        "original_exit_status=$original_status"
        'runner_vm_and_disposable_avd=ephemeral'
    )
    if ! printf '%s\n' "${cleanup_report[@]}" > "$report_tmp"; then
        report_write_status=FAIL
    elif ! grep -Fxq "cleanup_outcome=$cleanup_result" "$report_tmp" || \
        ! grep -Fxq "raw_diagnostics_removed=$raw_diagnostics_removed" "$report_tmp" || \
        ! grep -Fxq "original_exit_status=$original_status" "$report_tmp" || \
        ! grep -Fxq "host_model_final_state=$host_absence" "$report_tmp" || \
        ! grep -Fxq "app_private_owner_package_state=$app_private_owner_package_state" "$report_tmp" || \
        ! grep -Fxq "app_private_owner_package_query_exit_code=$app_private_owner_package_query_exit_code" "$report_tmp" || \
        ! grep -Fxq "app_private_owner_package_query_matches=$app_private_owner_package_query_matches" "$report_tmp" || \
        ! grep -Fxq "app_private_owner_package_query_diagnostic=$app_private_owner_package_query_diagnostic" "$report_tmp" || \
        ! grep -Fxq "device_connectivity_state=$DEVICE_CONNECTIVITY_STATE" "$report_tmp" || \
        ! grep -Fxq "device_connectivity_exit_code=$DEVICE_CONNECTIVITY_EXIT_CODE" "$report_tmp" || \
        ! grep -Fxq "primary_failure_status=$PRIMARY_FAILURE_STATUS" "$report_tmp" || \
        ! grep -Fxq "primary_failure_line=$PRIMARY_FAILURE_LINE" "$report_tmp" || \
        ! grep -Fxq "primary_failure_stage=$PRIMARY_FAILURE_STAGE" "$report_tmp" || \
        ! grep -Fxq "app_private_model_final_state=$app_private_state" "$report_tmp" || \
        ! grep -Fxq "device_temp_model_final_state=$temp_state" "$report_tmp" || \
        ! grep -Fxq "test_package_final_state=$test_package_state" "$report_tmp" || \
        ! grep -Fxq "app_package_final_state=$app_package_state" "$report_tmp"; then
        report_write_status=FAIL
    elif ! mv "$report_tmp" "$report_path"; then
        report_write_status=FAIL
    elif ! grep -Fxq "cleanup_outcome=$cleanup_result" "$report_path" || \
        ! grep -Fxq "raw_diagnostics_removed=$raw_diagnostics_removed" "$report_path" || \
        ! grep -Fxq "host_model_final_state=$host_absence" "$report_path" || \
        ! grep -Fxq "app_private_owner_package_state=$app_private_owner_package_state" "$report_path" || \
        ! grep -Fxq "app_private_owner_package_query_exit_code=$app_private_owner_package_query_exit_code" "$report_path" || \
        ! grep -Fxq "app_private_owner_package_query_matches=$app_private_owner_package_query_matches" "$report_path" || \
        ! grep -Fxq "app_private_owner_package_query_diagnostic=$app_private_owner_package_query_diagnostic" "$report_path" || \
        ! grep -Fxq "device_connectivity_state=$DEVICE_CONNECTIVITY_STATE" "$report_path" || \
        ! grep -Fxq "device_connectivity_exit_code=$DEVICE_CONNECTIVITY_EXIT_CODE" "$report_path" || \
        ! grep -Fxq "primary_failure_status=$PRIMARY_FAILURE_STATUS" "$report_path" || \
        ! grep -Fxq "primary_failure_line=$PRIMARY_FAILURE_LINE" "$report_path" || \
        ! grep -Fxq "primary_failure_stage=$PRIMARY_FAILURE_STAGE" "$report_path" || \
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
            "app_private_owner_package_query_exit_code=$app_private_owner_package_query_exit_code" \
            "app_private_owner_package_query_matches=$app_private_owner_package_query_matches" \
            "app_private_owner_package_query_diagnostic=$app_private_owner_package_query_diagnostic" \
            "device_connectivity_state=$DEVICE_CONNECTIVITY_STATE" \
            "device_connectivity_exit_code=$DEVICE_CONNECTIVITY_EXIT_CODE" \
            "primary_failure_status=$PRIMARY_FAILURE_STATUS" \
            "primary_failure_line=$PRIMARY_FAILURE_LINE" \
            "primary_failure_stage=$PRIMARY_FAILURE_STAGE" \
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
        if [[ -s "$EVIDENCE_DIR/instrumentation-command-status.txt" || \
              -s "$EVIDENCE_DIR/junit-verification.txt" ]]; then
            echo 'failure_instrumentation_command_status_begin' >&2
            print_sanitized_instrumentation_summary
            echo 'failure_instrumentation_command_status_end' >&2
        fi
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
    CURRENT_STAGE=CLEANUP_TEST_MODE
    if [[ "${SEREA_CLEANUP_TEST_DIAGNOSTICS:-0}" == 1 || \
          "${SEREA_CLEANUP_TEST_GATE_FAILURE:-0}" == 1 ]]; then
        printf '%s\n' \
            'adb_instrumentation_exit=1' \
            'logcat_capture=PASS' \
            'stream_parse_exit=1' \
            'timeout_cleanup=NOT_NEEDED' \
            'instrumentation_timeout=NOT_EXPIRED' \
            > "$EVIDENCE_DIR/instrumentation-command-status.txt"
        printf '%s\n' \
            'evidence_kind=raw_android_instrumentation_status' \
            'stream_outcome=TEST_STATUSES_PRESENT' \
            'instrumentation_code=0' \
            'reported_numtests=2' \
            'tests=2' \
            'failures=1' \
            'errors=0' \
            'skipped=0' \
            'noop_result=PASS' \
            'native_result=FAIL_OR_NOT_REACHED' \
            'native_initialization_error=ABSENT' \
            'noop_initialization_error=ABSENT' \
            'unexpected_identity_count=0' \
            'terminal_status=synthetic#diagnostic:-2' \
            'generated_content=DO_NOT_PRINT' \
            > "$EVIDENCE_DIR/junit-verification.txt"
    fi
    if [[ "${SEREA_CLEANUP_TEST_CREATE_RAW_DIAGNOSTICS:-0}" == 1 ]]; then
        printf '%s\n' 'synthetic prompt output must not be retained' > "$EVIDENCE_DIR/android-instrumentation.txt"
        printf '%s\n' 'synthetic raw log line must not be retained' > "$EVIDENCE_DIR/android-logcat-diagnostic.log"
    fi
    if [[ "${SEREA_CLEANUP_TEST_GATE_FAILURE:-0}" == 1 ]]; then
        printf '%s\n' 'raw_generated_output_marker=DO_NOT_PRINT' > "$EVIDENCE_DIR/android-instrumentation.txt"
        printf '%s\n' 'raw_logcat_output_marker=DO_NOT_PRINT' > "$EVIDENCE_DIR/android-logcat-diagnostic.log"
        CURRENT_STAGE=INSTRUMENTATION_GATE
        echo 'Android instrumentation diagnostic or native inference did not pass; sanitized summary follows.' >&2
        print_sanitized_instrumentation_summary
        record_primary_failure 1 "$LINENO"
        exit 1
    fi
    if [[ "${SEREA_CLEANUP_TEST_FAIL_COMMAND:-0}" == 1 ]]; then
        CURRENT_STAGE=TEST_PRIMARY_COMMAND_FAILURE
        false
    fi
    exit "${SEREA_CLEANUP_TEST_PRIMARY_STATUS:-0}"
fi

if [[ -n "${G8_EVIDENCE_DIR+x}" ]]; then
    echo 'G8_EVIDENCE_DIR is not accepted; evidence must use a fresh runner-temporary directory.' >&2
    exit 2
fi

CURRENT_STAGE=INPUT_ARTIFACT_VALIDATION
[[ -s "$TARGET_APK" ]]
[[ -s "$TEST_APK" ]]
[[ -s "$MODEL_HOST_PATH" ]]
test "$(stat -c '%s' "$MODEL_HOST_PATH")" = "$MODEL_SIZE_BYTES"
test "$(sha256sum "$MODEL_HOST_PATH" | awk '{print $1}')" = "$MODEL_SHA256"

CURRENT_STAGE=DEVICE_PREFLIGHT
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
CURRENT_STAGE=SOURCE_IDENTITY
printf '%s\n' "$preflight_output" > "$EVIDENCE_DIR/preflight.txt"
{
    if [[ "$EXPECTED_NATIVE_ABI" == 'arm64-v8a' ]]; then
        echo 'runtime_execution_mode=ARM64_GUEST_SOFTWARE_EMULATED'
    else
        echo 'runtime_execution_mode=X86_64_GUEST_KVM_ACCELERATED'
    fi
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
CURRENT_STAGE=TARGET_APK_INSTALL
if adb_device install "$TARGET_APK" > "$EVIDENCE_DIR/adb-install-target.log" 2>&1; then
    APP_PACKAGE_OWNED=1
else
    record_primary_failure 1 "$LINENO"
    echo 'Target APK install failed; it will not be treated as an owned package.' >&2
    exit 1
fi
CURRENT_STAGE=TEST_APK_INSTALL
if adb_device install "$TEST_APK" > "$EVIDENCE_DIR/adb-install-test.log" 2>&1; then
    TEST_PACKAGE_OWNED=1
else
    record_primary_failure 1 "$LINENO"
    echo 'AndroidTest APK install failed; it will not be treated as an owned package.' >&2
    exit 1
fi
CURRENT_STAGE=TARGET_SEEDING
EXPECTED_API_LEVEL=35 bash "$ROOT_DIR/tools/prepare_isolated_instrumentation_target.sh" "$campaign_token" \
    > "$EVIDENCE_DIR/isolated-target-seed.log" 2>&1
CURRENT_STAGE=MODEL_STAGING
device_data_dir="$(adb_device shell run-as "$APP_ID" pwd | tr -d '\r')"
case "$device_data_dir" in
    "/data/user/0/$APP_ID"|"/data/data/$APP_ID") ;;
    *)
        record_primary_failure 1 "$LINENO"
        echo 'Unexpected app-private data root' >&2
        exit 1
        ;;
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
CURRENT_STAGE=RUNNER_DISCOVERY
instrumentation_listing="$(adb_device shell pm list instrumentation | tr -d '\r')"
instrumentation_component="$TEST_ID/$TEST_RUNNER"
if ! grep -Fq "instrumentation:$instrumentation_component (target=$APP_ID)" <<< "$instrumentation_listing"; then
    printf '%s\n' "$instrumentation_listing" > "$EVIDENCE_DIR/instrumentation-manifest.txt"
    record_primary_failure 1 "$LINENO"
    echo 'Expected isolated instrumentation component is absent from Package Manager.' >&2
    exit 1
fi
printf '%s\n' "$instrumentation_listing" > "$EVIDENCE_DIR/instrumentation-manifest.txt"

CURRENT_STAGE=INSTRUMENTATION_EXECUTION
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

CURRENT_STAGE=INSTRUMENTATION_PARSING
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

rm -f -- "$instrumentation_log" "$EVIDENCE_DIR/android-logcat-diagnostic.log"
if [[ -e "$instrumentation_log" || -L "$instrumentation_log" || \
      -e "$EVIDENCE_DIR/android-logcat-diagnostic.log" || -L "$EVIDENCE_DIR/android-logcat-diagnostic.log" ]]; then
    echo 'Raw Android instrumentation diagnostics could not be removed.' >&2
    record_primary_failure 1 "$LINENO"
    exit 1
fi

source "$ROOT_DIR/tools/g8-local-model-native/instrumentation-gate.sh"
CURRENT_STAGE=INSTRUMENTATION_GATE
if ! instrumentation_gate_passes "$instrumentation_status" "$logcat_status" \
    "$parser_status" "$timeout_cleanup" "$EVIDENCE_DIR/junit-verification.txt"; then
    echo 'Android instrumentation diagnostic or native inference did not pass; sanitized summary follows.' >&2
    print_sanitized_instrumentation_summary
    record_primary_failure 1 "$LINENO"
    exit 1
fi

CURRENT_STAGE=NATIVE_METRICS_CAPTURE
adb_device logcat -d -v threadtime -s SereaLocalNativeRuntime:I \
    | tail -c 262144 \
    | python3 "$ROOT_DIR/tools/g8-local-model-native/summarize-runtime-metrics.py" \
    > "$EVIDENCE_DIR/native-runtime-metrics-summary.txt"
CURRENT_STAGE=RUNTIME_EVIDENCE_COMPLETE
