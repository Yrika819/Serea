#!/usr/bin/env bash

# Return success only when the raw instrumentation command, diagnostic captures, parser,
# and the exact expected no-op/native terminal markers all succeeded.
instrumentation_gate_passes() {
    local instrumentation_status="$1"
    local logcat_status="$2"
    local parser_status="$3"
    local timeout_cleanup="$4"
    local verification_file="$5"
    local marker
    local -a required_markers=(
        'noop_result=PASS'
        'native_result=PASS'
        'failures=0'
        'errors=0'
        'skipped=0'
    )

    if ((instrumentation_status != 0 || parser_status != 0)); then
        return 1
    fi
    if [[ "$logcat_status" != PASS || "$timeout_cleanup" == FAIL ]]; then
        return 1
    fi
    for marker in "${required_markers[@]}"; do
        if ! grep -Fxq "$marker" "$verification_file"; then
            return 1
        fi
    done
    return 0
}
