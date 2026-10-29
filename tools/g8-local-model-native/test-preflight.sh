#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(git rev-parse --show-toplevel)"
TEST_ROOT="$(mktemp -d "${RUNNER_TEMP:-/tmp}/serea-g8-preflight-test.XXXXXX")"
trap '/bin/rm -rf "$TEST_ROOT"' EXIT
FAKE_BIN="$TEST_ROOT/fakebin"
mkdir -p "$FAKE_BIN"

cat > "$FAKE_BIN/adb" <<'ADB'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == devices ]]; then
    case "${FAKE_ADB_MODE:-safe}" in
        physical) printf 'List of devices attached\nphysical-1\tdevice\n' ;;
        multiple) printf 'List of devices attached\nemulator-5554\tdevice\nphysical-1\tdevice\n' ;;
        *) printf 'List of devices attached\nemulator-5554\tdevice\n' ;;
    esac
    exit 0
fi
if [[ "${1:-}" == -s ]]; then shift 2; fi
if [[ "${1:-}" == shell && "${2:-}" == getprop ]]; then
    case "${3:-}" in
        ro.build.version.sdk) printf '35\n' ;;
        ro.kernel.qemu) printf '1\n' ;;
        sys.boot_completed) printf '1\n' ;;
        ro.product.cpu.abi) printf 'x86_64\n' ;;
        ro.hardware) printf 'ranchu\n' ;;
        ro.build.fingerprint) printf 'generic/sdk_phone64_x86_64/emu64x:35\n' ;;
        *) exit 8 ;;
    esac
    exit 0
fi
if [[ "${1:-}" == shell && "${2:-}" == pm && "${3:-}" == list && "${4:-}" == packages ]]; then
    if [[ "${FAKE_ADB_MODE:-safe}" == query-error ]]; then exit 9; fi
    retained_query=false
    if [[ "${5:-}" == -u ]]; then
        retained_query=true
        package="${6:-}"
    else
        package="${5:-}"
    fi
    if [[ "${FAKE_ADB_MODE:-safe}" == reused-installed && "$package" == app.serea.android.isolated ]]; then
        printf 'package:%s\n' "$package"
    fi
    if [[ "${FAKE_ADB_MODE:-safe}" == reused-retained && "$retained_query" == true && "$package" == app.serea.android.isolated ]]; then
        printf 'package:%s\n' "$package"
    fi
    if [[ "${FAKE_ADB_MODE:-safe}" == query-malformed && "$package" == app.serea.android.isolated ]]; then
        printf 'warning: package query incomplete\n'
    fi
    exit 0
fi
if [[ "${1:-}" == shell && "${2:-}" == test ]]; then exit 0; fi
echo "unexpected adb mutation or invocation: $*" >&2
exit 99
ADB
chmod +x "$FAKE_BIN/adb"

run_preflight() {
    local mode="$1"
    env PATH="$FAKE_BIN:$PATH" FAKE_ADB_MODE="$mode" CI=true GITHUB_ACTIONS=true \
        GITHUB_RUN_ID=123 SEREA_EXPECTED_ANDROID_API=35 SEREA_DISPOSABLE_CI_EMULATOR=1 \
        bash "$ROOT_DIR/tools/g8-local-model-native/preflight.sh"
}

safe_output="$(run_preflight safe)"
grep -Fxq 'package_ids=ABSENT' <<< "$safe_output"
if run_preflight physical >"$TEST_ROOT/physical.out" 2>"$TEST_ROOT/physical.err"; then
    echo 'physical target unexpectedly passed preflight' >&2
    exit 1
fi
grep -Fq 'Refusing non-emulator' "$TEST_ROOT/physical.err"
if run_preflight multiple >"$TEST_ROOT/multiple.out" 2>"$TEST_ROOT/multiple.err"; then
    echo 'multiple targets unexpectedly passed preflight' >&2
    exit 1
fi
grep -Fq 'Expected exactly one connected Android device' "$TEST_ROOT/multiple.err"
if run_preflight reused-installed >"$TEST_ROOT/reused-installed.out" 2>"$TEST_ROOT/reused-installed.err"; then
    echo 'installed package namespace unexpectedly passed preflight' >&2
    exit 1
fi
grep -Fq 'Refusing pre-existing isolated package' "$TEST_ROOT/reused-installed.err"
if run_preflight reused-retained >"$TEST_ROOT/reused-retained.out" 2>"$TEST_ROOT/reused-retained.err"; then
    echo 'retained uninstalled package namespace unexpectedly passed preflight' >&2
    exit 1
fi
grep -Fq 'Refusing pre-existing isolated package' "$TEST_ROOT/reused-retained.err"
if run_preflight query-error >"$TEST_ROOT/query-error.out" 2>"$TEST_ROOT/query-error.err"; then
    echo 'package query error unexpectedly passed preflight' >&2
    exit 1
fi
grep -Fq 'Could not verify isolated package absence' "$TEST_ROOT/query-error.err"
if run_preflight query-malformed >"$TEST_ROOT/query-malformed.out" 2>"$TEST_ROOT/query-malformed.err"; then
    echo 'malformed package query unexpectedly passed preflight' >&2
    exit 1
fi
grep -Fq 'Unexpected package query response' "$TEST_ROOT/query-malformed.err"

sentinel="$TEST_ROOT/do-not-delete/sentinel"
mkdir -p "$(dirname "$sentinel")"
printf 'owner-data\n' > "$sentinel"
if env PATH="$FAKE_BIN:$PATH" RUNNER_TEMP="$TEST_ROOT/runner" G8_EVIDENCE_DIR="$(dirname "$sentinel")" \
        bash "$ROOT_DIR/tools/g8-local-model-native/run-e2e.sh" \
        >"$TEST_ROOT/evidence-override.out" 2>"$TEST_ROOT/evidence-override.err"; then
    echo 'caller-controlled evidence directory unexpectedly accepted' >&2
    exit 1
fi
grep -Fq 'G8_EVIDENCE_DIR is not accepted' "$TEST_ROOT/evidence-override.err"
[[ "$(cat "$sentinel")" == owner-data ]]
[[ "$(find "$TEST_ROOT" -maxdepth 2 -type f -name 'sentinel' | wc -l)" == 1 ]]

echo 'native_preflight=PASS physical=rejected multiple=rejected installed_and_retained_packages=rejected package_query_error_and_malformed_output=closed unsafe_evidence_override=rejected_no_mutation=true'
