#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(git rev-parse --show-toplevel)"
RUN_E2E="$ROOT_DIR/tools/g8-local-model-native/run-e2e.sh"
TEST_ROOT="$(mktemp -d "${RUNNER_TEMP:-/tmp}/serea-g8-cleanup-test.XXXXXX")"
TEMPLATE_BIN="$TEST_ROOT/template-bin"
mkdir -p "$TEMPLATE_BIN"
trap '/bin/rm -rf "$TEST_ROOT"' EXIT

cat > "$TEMPLATE_BIN/adb" <<'ADB'
#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == -s ]]; then shift 2; fi
if [[ "${1:-}" == get-state ]]; then
    if [[ "${FAKE_ADB_OFFLINE:-0}" == 1 ]]; then
        echo 'error: device offline' >&2
        exit 1
    fi
    echo 'device'
    exit 0
fi
if [[ "${1:-}" == shell && "${2:-}" == pm && "${3:-}" == list && "${4:-}" == packages ]]; then
    include_uninstalled=false
    if [[ "${5:-}" == -u ]]; then
        include_uninstalled=true
        package="${6:-}"
    else
        package="${5:-}"
    fi
    if [[ "${FAKE_ADB_OFFLINE:-0}" == 1 ]]; then
        echo 'adb: device offline' >&2
        exit 1
    fi
    [[ "${FAIL_APP_PRIVATE_PACKAGE_QUERY:-0}" != 1 || "$package" != app.serea.android.isolated || \
       "$include_uninstalled" == true ]] || exit 28
    [[ "${FAIL_CLEANUP_QUERY:-0}" != 1 || "$include_uninstalled" != true ]] || exit 27
    if [[ "${MALFORMED_CLEANUP_QUERY:-0}" == 1 && "$include_uninstalled" == true ]]; then
        printf 'warning: package query incomplete\n'
        exit 0
    fi
    emit_matches() {
        local state_file="$1" package_line package_name
        [[ -f "$state_file" ]] || return 0
        while IFS= read -r package_line; do
            [[ "$package_line" == package:* ]] || continue
            package_name="${package_line#package:}"
            [[ "$package_name" == *"$package"* ]] && printf '%s\n' "$package_line"
        done < "$state_file"
        return 0
    }
    emit_matches "$ADB_STATE/packages"
    if [[ "$include_uninstalled" == true ]]; then
        emit_matches "$ADB_STATE/retained-packages"
    fi
    exit 0
fi
if [[ "${1:-}" == shell && "${2:-}" == run-as && "${4:-}" == rm ]]; then
    [[ "${FAIL_APP_PRIVATE_RM:-0}" != 1 ]] || exit 21
    /bin/rm -f "$ADB_STATE/app-private-model"
    exit 0
fi
if [[ "${1:-}" == shell && "${2:-}" == run-as && "${4:-}" == test ]]; then
    [[ ! -e "$ADB_STATE/app-private-model" ]]
    exit
fi
if [[ "${1:-}" == shell && "${2:-}" == rm && "${3:-}" == -f ]]; then
    [[ "${FAIL_TEMP_RM:-0}" != 1 ]] || exit 22
    /bin/rm -f "$ADB_STATE/device-temp-model"
    exit 0
fi
if [[ "${1:-}" == shell && "${2:-}" == test ]]; then
    [[ ! -e "$ADB_STATE/device-temp-model" ]]
    exit
fi
if [[ "${1:-}" == uninstall ]]; then
    package="${2:-}"
    if [[ "$package" == app.serea.android.isolated.test && "${FAIL_TEST_UNINSTALL:-0}" == 1 ]]; then exit 23; fi
    if [[ "$package" == app.serea.android.isolated && "${FAIL_APP_UNINSTALL:-0}" == 1 ]]; then exit 24; fi
    if [[ "${RETAIN_AFTER_UNINSTALL:-0}" == 1 ]]; then
        printf 'package:%s\n' "$package" >> "$ADB_STATE/retained-packages"
    fi
    awk -v line="package:$package" '$0 != line' "$ADB_STATE/packages" > "$ADB_STATE/packages.next"
    mv "$ADB_STATE/packages.next" "$ADB_STATE/packages"
    exit 0
fi

echo "unexpected adb invocation: $*" >&2
exit 99
ADB

cat > "$TEMPLATE_BIN/rm" <<'RM'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${FAIL_EVIDENCE_RM:-0}" == 1 && "${1:-}" == -rf && "${2:-}" == -- && \
      "${3:-}" == "$RUNNER_TEMP/serea-g8-native-cleanup-test" ]]; then
    exit 26
fi
for argument in "$@"; do
    if [[ "${FAIL_HOST_MODEL_RM:-0}" == 1 && "$argument" == "$RUNNER_TEMP/serea-g8-smollm2.gguf" ]]; then
        exit 25
    fi
done
exec /bin/rm "$@"
RM
chmod +x "$TEMPLATE_BIN/adb" "$TEMPLATE_BIN/rm"

prepare_case() {
    local case_name="$1"
    CASE_ROOT="$TEST_ROOT/$case_name"
    RUNNER_TEMP="$CASE_ROOT/runner"
    FAKE_BIN="$RUNNER_TEMP/fakebin"
    EVIDENCE_DIR="$RUNNER_TEMP/serea-g8-native-cleanup-test"
    ADB_STATE="$CASE_ROOT/adb-state"
    GITHUB_OUTPUT="$CASE_ROOT/github-output"
    mkdir -p "$RUNNER_TEMP" "$FAKE_BIN" "$EVIDENCE_DIR" "$ADB_STATE"
    cp "$TEMPLATE_BIN/adb" "$TEMPLATE_BIN/rm" "$FAKE_BIN/"
    printf '%s\n' 'serea-cleanup-test-v1' > "$RUNNER_TEMP/.serea-cleanup-test-root"
    : > "$GITHUB_OUTPUT"
    printf '%s\n' 'package:app.serea.android.isolated' \
        'package:app.serea.android.isolated.test' > "$ADB_STATE/packages"
    : > "$ADB_STATE/retained-packages"
    : > "$ADB_STATE/app-private-model"
    : > "$ADB_STATE/device-temp-model"
    : > "$RUNNER_TEMP/serea-g8-smollm2.gguf"
}

run_case() {
    local case_name="$1"
    local failure_assignment="${2:-}"
    local primary_status="${3:-0}"
    local secondary_failure_assignment="${4:-}"
    local status
    local -a command=(env \
        PATH="$FAKE_BIN:$PATH" \
        RUNNER_TEMP="$RUNNER_TEMP" \
        ADB_STATE="$ADB_STATE" \
        GITHUB_OUTPUT="$GITHUB_OUTPUT" \
        SEREA_CLEANUP_TEST_MODE=1 \
        SEREA_CLEANUP_TEST_PRIMARY_STATUS="$primary_status")
    if [[ -n "$failure_assignment" ]]; then
        command+=("$failure_assignment")
    fi
    if [[ -n "$secondary_failure_assignment" ]]; then
        command+=("$secondary_failure_assignment")
    fi
    command+=("$RUN_E2E")
    if "${command[@]}" > "$CASE_ROOT/stdout.log" 2> "$CASE_ROOT/stderr.log"; then
        status=0
    else
        status=$?
    fi
    printf '%s\n' "$status"
}

# Normal cleanup must verify every final resource state.
prepare_case success
status="$(run_case success)"
[[ "$status" == 0 ]]
report="$EVIDENCE_DIR/cleanup-boundary.txt"
grep -Fxq 'host_model_rm_command=PASS' "$report"
grep -Fxq 'host_model_final_state=ABSENT_VERIFIED' "$report"
grep -Fxq 'app_private_owner_package_state=INSTALLED' "$report"
grep -Fxq 'app_private_owner_package_query_exit_code=0' "$report"
grep -Fxq 'app_private_owner_package_query_matches=package:app.serea.android.isolated;package:app.serea.android.isolated.test' "$report"
grep -Fxq 'app_private_owner_package_query_diagnostic=NONE' "$report"
grep -Fxq 'device_connectivity_state=DEVICE' "$report"
grep -Fxq 'device_connectivity_exit_code=0' "$report"
grep -Fxq 'app_private_model_final_state=ABSENT_VERIFIED' "$report"
grep -Fxq 'device_temp_model_final_state=ABSENT_VERIFIED' "$report"
grep -Fxq 'test_package_final_state=ABSENT_VERIFIED' "$report"
grep -Fxq 'app_package_final_state=ABSENT_VERIFIED' "$report"
grep -Fxq 'cleanup_outcome=PASS' "$report"
grep -Fxq 'cleanup_verified=true' "$GITHUB_OUTPUT"
[[ ! -e "$RUNNER_TEMP/serea-g8-smollm2.gguf" ]]
[[ ! -e "$ADB_STATE/app-private-model" && ! -e "$ADB_STATE/device-temp-model" ]]
[[ ! -s "$ADB_STATE/packages" ]]
[[ ! -s "$ADB_STATE/retained-packages" ]]

assert_cleanup_failure() {
    local case_name="$1"
    local failure_assignment="$2"
    local primary_status="${3:-0}"
    local status
    prepare_case "$case_name"
    status="$(run_case "$case_name" "$failure_assignment" "$primary_status")"
    if [[ "$primary_status" != 0 ]]; then
        [[ "$status" == "$primary_status" ]]
    else
        [[ "$status" != 0 ]]
    fi
    grep -Fq 'cleanup_outcome=FAIL' "$CASE_ROOT/stderr.log"
    [[ ! -e "$EVIDENCE_DIR" ]]
    if grep -Eq 'host_model_file_removed=true|app_and_test_packages_uninstalled=true|cleanup_outcome=PASS' \
        "$CASE_ROOT/stderr.log"; then
        echo "FAIL: '$case_name' emitted an unconditional success flag" >&2
        exit 1
    fi
    if grep -Fxq 'cleanup_verified=true' "$GITHUB_OUTPUT"; then
        echo "FAIL: '$case_name' exposed cleanup_verified=true" >&2
        exit 1
    fi
}

assert_cleanup_failure app_private_rm FAIL_APP_PRIVATE_RM=1
assert_cleanup_failure device_temp_rm FAIL_TEMP_RM=1
assert_cleanup_failure test_package_uninstall FAIL_TEST_UNINSTALL=1
assert_cleanup_failure app_package_uninstall FAIL_APP_UNINSTALL=1
assert_cleanup_failure host_model_rm FAIL_HOST_MODEL_RM=1
assert_cleanup_failure preserve_primary_status FAIL_TEST_UNINSTALL=1 42

prepare_case retain_primary_failure_context
status="$(run_case retain_primary_failure_context SEREA_CLEANUP_TEST_FAIL_COMMAND=1 1 FAIL_TEMP_RM=1)"
[[ "$status" == 1 ]]
grep -Fxq 'primary_failure_status=1' "$CASE_ROOT/stderr.log"
grep -Fxq 'primary_failure_stage=TEST_PRIMARY_COMMAND_FAILURE' "$CASE_ROOT/stderr.log"
grep -Eq '^primary_failure_line=[0-9]+$' "$CASE_ROOT/stderr.log"
grep -Fxq 'cleanup_outcome=FAIL' "$CASE_ROOT/stderr.log"
! grep -Fxq 'cleanup_verified=true' "$GITHUB_OUTPUT"

prepare_case sanitized_failure_summary
status="$(run_case sanitized_failure_summary SEREA_CLEANUP_TEST_DIAGNOSTICS=1 42 FAIL_TEMP_RM=1)"
[[ "$status" == 42 ]]
grep -Fxq 'primary_failure_status=42' "$CASE_ROOT/stderr.log"
grep -Fxq 'primary_failure_stage=UNCLASSIFIED_EXIT' "$CASE_ROOT/stderr.log"
grep -Fxq 'adb_instrumentation_exit=1' "$CASE_ROOT/stderr.log"
grep -Fxq 'instrumentation_code=0' "$CASE_ROOT/stderr.log"
grep -Fxq 'tests=2' "$CASE_ROOT/stderr.log"
grep -Fxq 'failures=1' "$CASE_ROOT/stderr.log"
grep -Fxq 'native_result=FAIL_OR_NOT_REACHED' "$CASE_ROOT/stderr.log"
if grep -Fq 'generated_content=DO_NOT_PRINT' "$CASE_ROOT/stderr.log"; then
    echo 'FAIL: cleanup failure diagnostics printed raw generated content' >&2
    exit 1
fi
[[ ! -e "$EVIDENCE_DIR" ]]
! grep -Fxq 'cleanup_verified=true' "$GITHUB_OUTPUT"

# `-u` must detect uninstalled records that retain app data, and failed `-u`
# cleanup probes must never publish success evidence.
assert_cleanup_failure retained_package_data RETAIN_AFTER_UNINSTALL=1
assert_cleanup_failure retained_package_query_error FAIL_CLEANUP_QUERY=1
assert_cleanup_failure malformed_retained_package_query MALFORMED_CLEANUP_QUERY=1

prepare_case app_private_package_query_failure
status="$(run_case app_private_package_query_failure FAIL_APP_PRIVATE_PACKAGE_QUERY=1 0 FAIL_EVIDENCE_RM=1)"
[[ "$status" != 0 ]]
grep -Fxq 'app_private_owner_package_state=QUERY_FAILED' "$EVIDENCE_DIR/cleanup-boundary.txt"
grep -Fxq 'app_private_owner_package_query_exit_code=28' "$EVIDENCE_DIR/cleanup-boundary.txt"
grep -Fxq 'app_private_owner_package_query_matches=QUERY_COMMAND_FAILED' "$EVIDENCE_DIR/cleanup-boundary.txt"
grep -Fxq 'app_private_owner_package_query_diagnostic=ADB_COMMAND_FAILED' "$EVIDENCE_DIR/cleanup-boundary.txt"
grep -Fxq 'app_private_model_final_state=NOT_VERIFIED' "$EVIDENCE_DIR/cleanup-boundary.txt"
grep -Fxq 'cleanup_outcome=FAIL' "$EVIDENCE_DIR/cleanup-boundary.txt"
! grep -Fxq 'cleanup_verified=true' "$GITHUB_OUTPUT"

prepare_case adb_device_offline
status="$(run_case adb_device_offline FAKE_ADB_OFFLINE=1 0 FAIL_EVIDENCE_RM=1)"
[[ "$status" != 0 ]]
grep -Fxq 'app_private_owner_package_state=QUERY_FAILED' "$EVIDENCE_DIR/cleanup-boundary.txt"
grep -Fxq 'app_private_owner_package_query_diagnostic=DEVICE_OFFLINE' "$EVIDENCE_DIR/cleanup-boundary.txt"
grep -Fxq 'device_connectivity_state=DEVICE_OFFLINE' "$EVIDENCE_DIR/cleanup-boundary.txt"
grep -Fxq 'cleanup_outcome=FAIL' "$EVIDENCE_DIR/cleanup-boundary.txt"
! grep -Fxq 'cleanup_verified=true' "$GITHUB_OUTPUT"

# An owned app disappearing before app-private cleanup must remain unverified; package uninstall
# success cannot substitute for direct proof that the staged private file was removed.
prepare_case missing_owned_app_package
grep -Fvx 'package:app.serea.android.isolated' "$ADB_STATE/packages" > "$ADB_STATE/packages.next"
mv "$ADB_STATE/packages.next" "$ADB_STATE/packages"
status="$(run_case missing_owned_app_package FAIL_EVIDENCE_RM=1)"
[[ "$status" != 0 ]]
grep -Fxq 'app_private_owner_package_state=ABSENT' "$EVIDENCE_DIR/cleanup-boundary.txt"
grep -Fxq 'app_private_owner_package_query_exit_code=0' "$EVIDENCE_DIR/cleanup-boundary.txt"
grep -Fxq 'app_private_owner_package_query_matches=package:app.serea.android.isolated.test' "$EVIDENCE_DIR/cleanup-boundary.txt"
grep -Fxq 'app_private_model_rm_command=NOT_VERIFIED_OWNER_PACKAGE_ABSENT' "$EVIDENCE_DIR/cleanup-boundary.txt"
grep -Fxq 'app_private_model_final_state=NOT_VERIFIED' "$EVIDENCE_DIR/cleanup-boundary.txt"
grep -Fxq 'cleanup_outcome=FAIL' "$EVIDENCE_DIR/cleanup-boundary.txt"
! grep -Fxq 'cleanup_verified=true' "$GITHUB_OUTPUT"

# A failed removal of the evidence source must still fail closed at the
# workflow boundary; a surviving directory is never enough to authorize upload.
prepare_case evidence_source_rm_failure
status="$(run_case evidence_source_rm_failure FAIL_TEST_UNINSTALL=1 0 FAIL_EVIDENCE_RM=1)"
[[ "$status" != 0 ]]
[[ -d "$EVIDENCE_DIR" ]]
! grep -Fxq 'cleanup_verified=true' "$GITHUB_OUTPUT"
grep -Fq 'cleanup_evidence_source_removal=NOT_VERIFIED' "$CASE_ROOT/stderr.log"
grep -Fq "if: always() && steps.native_e2e.outputs.cleanup_verified == 'true'" \
    "$ROOT_DIR/.github/workflows/g8-local-native-runtime-focused.yml"

# A report write failure cannot publish a success output, even if resource
# cleanup itself succeeded.
prepare_case report_write_failure
mkdir "$EVIDENCE_DIR/cleanup-boundary.txt.tmp"
status="$(run_case report_write_failure)"
[[ "$status" != 0 ]]
! grep -Fxq 'cleanup_verified=true' "$GITHUB_OUTPUT"
grep -Fq 'cleanup_report_write=FAIL' "$CASE_ROOT/stderr.log"
[[ ! -e "$EVIDENCE_DIR" ]]

echo 'cleanup_harness=PASS normal=verified retained_and_malformed_package_states=fail_closed output_gate=fail_closed report_write=fail_closed failures=nonzero primary_status=preserved'
