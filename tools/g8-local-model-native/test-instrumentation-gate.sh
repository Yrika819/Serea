#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(git rev-parse --show-toplevel)"
source "$ROOT_DIR/tools/g8-local-model-native/instrumentation-gate.sh"
TEST_ROOT="$(mktemp -d "${RUNNER_TEMP:-/tmp}/serea-instrumentation-gate-test.XXXXXX")"
trap 'rm -rf "$TEST_ROOT"' EXIT

verification="$TEST_ROOT/junit-verification.txt"
cat > "$verification" <<'REPORT'
noop_result=PASS
native_result=PASS
failures=0
errors=0
skipped=0
REPORT

if ! instrumentation_gate_passes 0 PASS 0 NOT_NEEDED "$verification"; then
    echo 'FAIL: a fully successful instrumentation gate was rejected' >&2
    exit 1
fi

assert_rejected() {
    local name="$1" instrumentation_status="$2" logcat_status="$3"
    local parser_status="$4" timeout_cleanup="$5" report="$6"
    if instrumentation_gate_passes "$instrumentation_status" "$logcat_status" \
        "$parser_status" "$timeout_cleanup" "$report"; then
        echo "FAIL: gate accepted '$name'" >&2
        exit 1
    fi
}

assert_rejected command_failure 1 PASS 0 NOT_NEEDED "$verification"
assert_rejected parser_failure 0 PASS 1 NOT_NEEDED "$verification"
assert_rejected logcat_failure 0 FAIL 0 NOT_NEEDED "$verification"
assert_rejected timeout_cleanup_failure 0 PASS 0 FAIL "$verification"

for marker in noop_result=PASS native_result=PASS failures=0 errors=0 skipped=0; do
    awk -v absent="$marker" '$0 != absent' "$verification" > "$TEST_ROOT/verification.next"
    mv "$TEST_ROOT/verification.next" "$verification"
    assert_rejected "missing-$marker" 0 PASS 0 NOT_NEEDED "$verification"
    printf '%s\n' "$marker" >> "$verification"
done

echo 'instrumentation_gate=PASS success_and_fail_closed_cases=10'
