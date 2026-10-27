#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
POC_DIR="$ROOT_DIR/tools/g3-ondevice-poc"
APP_DIR="$POC_DIR/app"
EVIDENCE_DIR="${G3_EVIDENCE_DIR:?G3_EVIDENCE_DIR must point to a disposable evidence directory}"
LLAMA_COMMIT="276378515ceebee5d14bcc8536603dd5c87491a2"
LLAMA_SOURCE="${RUNNER_TEMP:?RUNNER_TEMP is required}/g3-llama.cpp"
APP_ID="app.serea.g3poc"
TEST_ID="app.serea.g3poc.test"
MODEL_SHA256="e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5"
MODEL_SIZE_BYTES="101016128"
MODEL_REPOSITORY_REVISION="1076b85d135017971845b19b8d07df1479155988"
MODEL_URL="https://huggingface.co/ggml-org/SmolLM2-135M-GGUF/resolve/${MODEL_REPOSITORY_REVISION}/SmolLM2-135M-Q4_K_M.gguf"
EXPECTED_EMULATOR_API_LEVEL="${G3_API_LEVEL:-35}"
DEVICE_MODEL_TEMP="/data/local/tmp/g3-smollm.gguf"
DEVICE_MODEL_RELATIVE="files/g3-smollm.gguf"
APP_APK="$APP_DIR/build/outputs/apk/debug/app-debug.apk"
TEST_APK_DIR="$APP_DIR/build/outputs/apk/androidTest/debug"
DOWNLOAD_TEST_DIRECTORY=''
G3_MODEL_HOST_PATH=''
G3_MODEL_PRIVATE_PATH_MAY_EXIST=0
G3_APP_INSTALLED=0
G3_RESULTS_DIR=''
CLEANUP_ADB_TIMEOUT_SECONDS=10

mkdir -p "$EVIDENCE_DIR"
if [[ "$EXPECTED_EMULATOR_API_LEVEL" != '26' && "$EXPECTED_EMULATOR_API_LEVEL" != '35' ]]; then
    echo "Unsupported G3 Emulator API selection: $EXPECTED_EMULATOR_API_LEVEL" >&2
    exit 2
fi

run_poc_gradle() (
    cd "$POC_DIR"
    exec ./gradlew "$@"
)

capture_optional_emulator_cpu_pressure() {
    local output_path="$1"
    local cleanup_mode="${2:-false}"
    local raw_output
    local raw_error
    raw_output="$(mktemp "${RUNNER_TEMP}/g3-cpu-pressure-output.XXXXXX")"
    raw_error="$(mktemp "${RUNNER_TEMP}/g3-cpu-pressure-error.XXXXXX")"
    local command_status=0
    local cleanup_log="$EVIDENCE_DIR/cleanup-adb/cpu-pressure.log"
    if [[ "$cleanup_mode" == 'true' ]]; then
        if cleanup_adb cpu-pressure shell cat /proc/pressure/cpu; then
            cp "$cleanup_log" "$raw_output"
        else
            command_status=$?
            cp "$cleanup_log" "$raw_output"
        fi
    elif adb shell cat /proc/pressure/cpu > "$raw_output" 2> "$raw_error"; then
        command_status=0
    else
        command_status=$?
    fi
    if [[ "$command_status" -eq 0 ]]; then
        cp "$raw_output" "$output_path"
        printf 'available=true\n' > "${output_path}.status"
    else
        {
            echo 'available=false'
            echo "exit_status=$command_status"
            cat "$raw_output"
            cat "$raw_error"
        } > "$output_path"
        printf 'available=false\nexit_status=%s\n' "$command_status" > "${output_path}.status"
    fi
    rm -f "$raw_output" "$raw_error" || true
    return 0
}

cleanup_adb() {
    local label="$1"
    shift
    local log_directory="$EVIDENCE_DIR/cleanup-adb"
    local output_path="$log_directory/${label}.log"
    local command_text
    local command_status=0
    local outcome='failed'
    mkdir -p "$log_directory"
    printf -v command_text '%q ' adb "$@"
    if timeout --foreground --signal=TERM --kill-after=2s \
        "${CLEANUP_ADB_TIMEOUT_SECONDS}s" adb "$@" > "$output_path" 2>&1; then
        printf 'cleanup_adb.%s=success command=%s\n' "$label" "$command_text" \
            >> "$EVIDENCE_DIR/model-cleanup.txt"
        return 0
    else
        command_status=$?
        if [[ "$command_status" -eq 124 || "$command_status" -eq 137 || "$command_status" -eq 143 ]]; then
            outcome='timeout'
        fi
        printf 'cleanup_adb.%s=%s exit_status=%s command=%s\n' \
            "$label" "$outcome" "$command_status" "$command_text" \
            >> "$EVIDENCE_DIR/model-cleanup.txt"
        return 1
    fi
}

cleanup_emulator() {
    local primary_status="$1"
    local final_status="$primary_status"
    local cleanup_status=0
    trap - EXIT INT TERM
    set +e
    mkdir -p "$EVIDENCE_DIR/cleanup-adb"
    echo 'cleanup_started=true' > "$EVIDENCE_DIR/model-cleanup.txt"
    if cleanup_adb get-state get-state; then
        if [[ -n "$G3_RESULTS_DIR" && -d "$G3_RESULTS_DIR" ]]; then
            mkdir -p "$EVIDENCE_DIR/androidTest-results"
            cp -a "$G3_RESULTS_DIR"/. "$EVIDENCE_DIR/androidTest-results/" || cleanup_status=1
        fi
        cleanup_adb logcat logcat -d -v threadtime || cleanup_status=1
        cp "$EVIDENCE_DIR/cleanup-adb/logcat.log" "$EVIDENCE_DIR/logcat.txt" || cleanup_status=1
        capture_optional_emulator_cpu_pressure "$EVIDENCE_DIR/emulator-cpu-pressure-after.txt" true
        if cleanup_adb emulator-meminfo shell cat /proc/meminfo; then
            cp "$EVIDENCE_DIR/cleanup-adb/emulator-meminfo.log" "$EVIDENCE_DIR/emulator-meminfo-after.txt" || cleanup_status=1
        else
            cp "$EVIDENCE_DIR/cleanup-adb/emulator-meminfo.log" "$EVIDENCE_DIR/emulator-meminfo-after.txt" || true
            cleanup_status=1
        fi
        if cleanup_adb app-meminfo shell dumpsys meminfo "$APP_ID"; then
            cp "$EVIDENCE_DIR/cleanup-adb/app-meminfo.log" "$EVIDENCE_DIR/app-meminfo-after.txt" || cleanup_status=1
        else
            cp "$EVIDENCE_DIR/cleanup-adb/app-meminfo.log" "$EVIDENCE_DIR/app-meminfo-after.txt" || true
            cleanup_status=1
        fi
        if [[ "$G3_MODEL_PRIVATE_PATH_MAY_EXIST" -eq 1 ]]; then
            if cleanup_adb target-package-presence shell pm list packages "$APP_ID"; then
                if grep -q "^package:${APP_ID}$" "$EVIDENCE_DIR/cleanup-adb/target-package-presence.log"; then
                    if cleanup_adb app-private-remove shell run-as "$APP_ID" rm -f "$DEVICE_MODEL_RELATIVE"; then
                        if cleanup_adb app-private-verify shell run-as "$APP_ID" test '!' -e "$DEVICE_MODEL_RELATIVE"; then
                            echo 'app_private_model_removed=true' >> "$EVIDENCE_DIR/model-cleanup.txt"
                        else
                            echo 'app_private_model_removed=false' >> "$EVIDENCE_DIR/model-cleanup.txt"
                            cleanup_status=1
                        fi
                    else
                        echo 'app_private_model_removed=false' >> "$EVIDENCE_DIR/model-cleanup.txt"
                        cleanup_status=1
                    fi
                else
                    # The test runner has already uninstalled the disposable target APK.
                    # PackageManager removal deletes its private files directory.
                    echo 'target_package_absent_after_test_runner=true' >> "$EVIDENCE_DIR/model-cleanup.txt"
                    echo 'app_private_model_removed=true' >> "$EVIDENCE_DIR/model-cleanup.txt"
                    echo 'app_private_model_removal_reason=target_package_uninstalled_by_test_runner' >> "$EVIDENCE_DIR/model-cleanup.txt"
                fi
            else
                echo 'app_private_model_removed=false' >> "$EVIDENCE_DIR/model-cleanup.txt"
                cleanup_status=1
            fi
        fi
        if cleanup_adb emulator-temp-remove shell rm -f "$DEVICE_MODEL_TEMP"; then
            if cleanup_adb emulator-temp-verify shell test '!' -e "$DEVICE_MODEL_TEMP"; then
                echo 'emulator_temp_model_removed=true' >> "$EVIDENCE_DIR/model-cleanup.txt"
            else
                echo 'emulator_temp_model_removed=false' >> "$EVIDENCE_DIR/model-cleanup.txt"
                cleanup_status=1
            fi
        else
            echo 'emulator_temp_model_removed=false' >> "$EVIDENCE_DIR/model-cleanup.txt"
            cleanup_status=1
        fi
        if [[ "$G3_APP_INSTALLED" -eq 1 ]]; then
            for package in "$TEST_ID" "$APP_ID"; do
                local package_label="package-path-${package//./-}"
                local uninstall_label="uninstall-${package//./-}"
                if cleanup_adb "$package_label" shell pm list packages "$package"; then
                    if grep -q '^package:' "$EVIDENCE_DIR/cleanup-adb/${package_label}.log"; then
                        cleanup_adb "$uninstall_label" uninstall "$package" || cleanup_status=1
                    else
                        echo "${package_label}_already_absent=true" >> "$EVIDENCE_DIR/model-cleanup.txt"
                    fi
                else
                    cleanup_status=1
                fi
            done
        fi
    else
        echo 'emulator_unreachable_during_cleanup=true' >> "$EVIDENCE_DIR/model-cleanup.txt"
        cleanup_status=1
        if [[ "$primary_status" -eq 0 ]]; then final_status=1; fi
    fi
    if [[ -n "$G3_MODEL_HOST_PATH" && -f "$G3_MODEL_HOST_PATH" ]]; then
        python3 - "$G3_MODEL_HOST_PATH" <<'PY'
from pathlib import Path
import sys
Path(sys.argv[1]).unlink(missing_ok=True)
PY
        if [[ -e "$G3_MODEL_HOST_PATH" ]]; then cleanup_status=1; fi
    fi
    if [[ "$cleanup_status" -ne 0 && "$primary_status" -eq 0 ]]; then final_status=1; fi
    {
        echo "primary_exit_status=$primary_status"
        echo "final_exit_status=$final_status"
    } >> "$EVIDENCE_DIR/model-cleanup.txt"
    G3_MODEL_HOST_PATH=''
    G3_MODEL_PRIVATE_PATH_MAY_EXIST=0
    G3_APP_INSTALLED=0
    G3_RESULTS_DIR=''
    if [[ -z "$G3_MODEL_HOST_PATH" && "$G3_MODEL_PRIVATE_PATH_MAY_EXIST" -eq 0 && "$G3_APP_INSTALLED" -eq 0 && -z "$G3_RESULTS_DIR" ]]; then
        echo 'cleanup_state_reset=true' >> "$EVIDENCE_DIR/model-cleanup.txt"
    else
        echo 'cleanup_state_reset=false' >> "$EVIDENCE_DIR/model-cleanup.txt"
        if [[ "$primary_status" -eq 0 ]]; then final_status=1; fi
    fi
    exit "$final_status"
}

test_optional_emulator_cpu_pressure() {
    capture_optional_emulator_cpu_pressure "$EVIDENCE_DIR/emulator-cpu-pressure-test.txt"
}

test_cleanup_after_early_failure() {
    G3_MODEL_HOST_PATH="$RUNNER_TEMP/g3-host-model-fixture.bin"
    G3_MODEL_PRIVATE_PATH_MAY_EXIST=1
    G3_APP_INSTALLED=1
    G3_RESULTS_DIR="$RUNNER_TEMP/no-android-test-results"
    printf 'synthetic cleanup fixture only\n' > "$G3_MODEL_HOST_PATH"
    trap 'cleanup_emulator "$?"' EXIT
    exit 23
}

fetch_bounded() {
    local url="$1"
    local output_path="$2"
    local maximum_bytes="$3"
    local curl_status=0
    curl --fail --silent --show-error --location --retry 2 \
        --max-filesize "$maximum_bytes" --output "$output_path" "$url" || curl_status=$?
    if [[ "$curl_status" -ne 0 ]]; then
        return "$curl_status"
    fi
    local actual_bytes
    actual_bytes="$(stat -c '%s' "$output_path")"
    (( actual_bytes <= maximum_bytes ))
}

test_download_bound() {
    DOWNLOAD_TEST_DIRECTORY="$(mktemp -d)"
    cleanup_download_fixture() {
        python3 - "$DOWNLOAD_TEST_DIRECTORY" <<'PY'
from pathlib import Path
import shutil
import sys
shutil.rmtree(Path(sys.argv[1]), ignore_errors=True)
PY
    }
    trap cleanup_download_fixture EXIT
    python3 - "$DOWNLOAD_TEST_DIRECTORY" <<'PY'
from pathlib import Path
import sys
Path(sys.argv[1], "synthetic-oversize.bin").write_bytes(b"x" * 33)
PY
    local destination="$DOWNLOAD_TEST_DIRECTORY/download.bin"
    local status=0
    fetch_bounded "file://$DOWNLOAD_TEST_DIRECTORY/synthetic-oversize.bin" "$destination" 32 || status=$?
    [[ "$status" -eq 63 ]]
    local partial_bytes=0
    if [[ -f "$destination" ]]; then
        partial_bytes="$(stat -c '%s' "$destination")"
    fi
    (( partial_bytes <= 32 ))
    printf 'bounded_download_test=PASS fixture_bytes=33 maximum_bytes=32 curl_status=%s partial_bytes=%s\n' \
        "$status" "$partial_bytes"
}

collect_host_metadata() {
    local output="$EVIDENCE_DIR/host-metadata.txt"
    {
        echo 'recorded_at_utc:'
        date -u +'%Y-%m-%dT%H:%M:%SZ'
        echo 'runner_os:'
        cat /etc/os-release
        echo 'runner_arch:'
        uname -m
        echo 'runner_cpu:'
        lscpu
        echo 'runner_vcpu_count:'
        nproc
        echo 'runner_cpu_quota:'
        for quota in /sys/fs/cgroup/cpu.max /sys/fs/cgroup/cpu/cpu.cfs_quota_us /sys/fs/cgroup/cpu/cpu.cfs_period_us; do
            if [[ -r "$quota" ]]; then echo "$quota=$(cat "$quota")"; fi
        done
        echo 'runner_memory:'
        free -h
        echo 'runner_memoryinfo:'
        cat /proc/meminfo
        echo 'runner_cpu_pressure:'
        cat /proc/pressure/cpu
        echo 'runner_kvm:'
        if [[ -e /dev/kvm ]]; then
            stat -Lc 'device=%n owner=%U group=%G mode=%a' /dev/kvm
            echo "kvm_readable=$(test -r /dev/kvm && echo true || echo false)"
            echo "kvm_writable=$(test -w /dev/kvm && echo true || echo false)"
        else
            echo '/dev/kvm: absent'
        fi
        id
        echo 'emulator_version:'
        "$ANDROID_HOME/emulator/emulator" -version 2>&1 || true
        echo 'emulator_acceleration_check:'
        "$ANDROID_HOME/emulator/emulator" -accel-check 2>&1 || true
        echo 'sdk_packages:'
        "$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" --list_installed 2>&1 || true
        echo 'ndk_version:'
        if [[ -r "$ANDROID_HOME/ndk/28.2.13676358/source.properties" ]]; then
            sed -n 's/^Pkg.Revision *= */\0/p' "$ANDROID_HOME/ndk/28.2.13676358/source.properties"
        else
            echo 'NDK package not installed'
        fi
        echo 'cmake_version:'
        "$ANDROID_HOME/cmake/3.31.6/bin/cmake" --version | head -n 1 || true
        echo 'ninja_version:'
        "$ANDROID_HOME/cmake/3.31.6/bin/ninja" --version || true
    } > "$output" 2>&1
}

prepare_build() {
    echo 'model_downloaded=false' > "$EVIDENCE_DIR/model-status.txt"
    mkdir -p "$LLAMA_SOURCE"
    if [[ ! -d "$LLAMA_SOURCE/.git" ]]; then
        git -C "$LLAMA_SOURCE" init
        git -C "$LLAMA_SOURCE" remote add origin https://github.com/ggml-org/llama.cpp.git
    fi
    git -C "$LLAMA_SOURCE" fetch --depth 1 origin "$LLAMA_COMMIT"
    git -C "$LLAMA_SOURCE" checkout --detach FETCH_HEAD
    local actual_commit
    actual_commit="$(git -C "$LLAMA_SOURCE" rev-parse HEAD)"
    [[ "$actual_commit" == "$LLAMA_COMMIT" ]]
    echo "$actual_commit" > "$EVIDENCE_DIR/llama-source-head.txt"
    git -C "$ROOT_DIR" rev-parse HEAD > "$EVIDENCE_DIR/serea-source-head.txt"

    local build_log="$EVIDENCE_DIR/gradle-build.log"
    if ! run_poc_gradle --no-daemon --console=plain \
        -Pg3LlamaCppSource="$LLAMA_SOURCE" \
        :app:assembleDebug :app:assembleDebugAndroidTest > "$build_log" 2>&1; then
        tail -n 160 "$build_log"
        return 1
    fi

    [[ -s "$APP_APK" ]]
    local -a test_apks
    mapfile -t test_apks < <(find "$TEST_APK_DIR" -maxdepth 1 -type f -name '*-androidTest.apk' -print)
    [[ "${#test_apks[@]}" -eq 1 ]]
    local test_apk="${test_apks[0]}"
    sha256sum "$APP_APK" > "$EVIDENCE_DIR/app-apk.sha256"
    sha256sum "$test_apk" > "$EVIDENCE_DIR/androidtest-apk.sha256"
    printf 'target_apk=%s\ninstrumentation_apk=%s\n' \
        "$(basename "$APP_APK")" "$(basename "$test_apk")" > "$EVIDENCE_DIR/apk-identities.txt"
    "$ANDROID_HOME/build-tools/35.0.0/aapt" dump permissions "$APP_APK" > "$EVIDENCE_DIR/app-apk-permissions.txt"
    if grep -q 'android.permission.INTERNET' "$EVIDENCE_DIR/app-apk-permissions.txt"; then
        echo 'Standalone PoC target APK must not request INTERNET permission' >&2
        return 1
    fi
    cp "$POC_DIR/model-fixture.json" "$EVIDENCE_DIR/model-fixture.json"
    echo 'apk_build_and_androidtest_compile=PASS' > "$EVIDENCE_DIR/build-status.txt"
}

run_model_test() {
    G3_MODEL_HOST_PATH="$RUNNER_TEMP/g3-smollm.gguf"
    G3_MODEL_PRIVATE_PATH_MAY_EXIST=0
    G3_APP_INSTALLED=0
    local device_data_dir=''
    G3_RESULTS_DIR="$APP_DIR/build/outputs/androidTest-results/connected"
    trap 'cleanup_emulator "$?"' EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM

    [[ -s "$APP_APK" ]]
    (cd "$ROOT_DIR" && sha256sum --check "$EVIDENCE_DIR/app-apk.sha256")
    (cd "$ROOT_DIR" && sha256sum --check "$EVIDENCE_DIR/androidtest-apk.sha256")
    adb wait-for-device
    [[ "$(adb shell getprop sys.boot_completed | tr -d '\r')" == '1' ]]
    local actual_api_level
    actual_api_level="$(adb shell getprop ro.build.version.sdk | tr -d '\r')"
    [[ "$actual_api_level" == "$EXPECTED_EMULATOR_API_LEVEL" ]]
    [[ "$(adb shell getprop ro.product.cpu.abi | tr -d '\r')" == 'x86_64' ]]
    {
        echo "emulator_api: $actual_api_level"
        echo "emulator_abi: $(adb shell getprop ro.product.cpu.abi | tr -d '\r')"
        echo "emulator_hardware: $(adb shell getprop ro.hardware | tr -d '\r')"
        echo "emulator_fingerprint: $(adb shell getprop ro.build.fingerprint | tr -d '\r')"
        echo "boot_completed: $(adb shell getprop sys.boot_completed | tr -d '\r')"
    } > "$EVIDENCE_DIR/emulator-metadata.txt"
    capture_optional_emulator_cpu_pressure "$EVIDENCE_DIR/emulator-cpu-pressure-before.txt"
    adb shell cat /proc/meminfo > "$EVIDENCE_DIR/emulator-meminfo-before.txt"

    adb install -r "$APP_APK" > "$EVIDENCE_DIR/adb-install-target.log" 2>&1
    G3_APP_INSTALLED=1
    mapfile -t test_apks < <(find "$TEST_APK_DIR" -maxdepth 1 -type f -name '*-androidTest.apk' -print)
    [[ "${#test_apks[@]}" -eq 1 ]]
    local test_apk="${test_apks[0]}"
    adb install -r "$test_apk" > "$EVIDENCE_DIR/adb-install-test.log" 2>&1
    for package in "$APP_ID" "$TEST_ID"; do
        remote_apk="$(adb shell pm path "$package" | sed -n '1s/^package://p' | tr -d '\r')"
        [[ -n "$remote_apk" ]]
        installed_hash="$(adb exec-out cat "$remote_apk" | sha256sum | awk '{print $1}')"
        printf 'package=%s\ninstalled_apk_path=%s\ninstalled_apk_sha256=%s\n' \
            "$package" "$remote_apk" "$installed_hash" >> "$EVIDENCE_DIR/installed-apk-hashes.txt"
    done
    expected_app_hash="$(awk '{print $1}' "$EVIDENCE_DIR/app-apk.sha256")"
    installed_target_hash="$(sed -n 's/^installed_apk_sha256=//p' "$EVIDENCE_DIR/installed-apk-hashes.txt" | sed -n '1p')"
    [[ "$installed_target_hash" == "$expected_app_hash" ]]
    printf 'built_target_sha256=%s\ninstalled_target_sha256=%s\n' \
        "$expected_app_hash" "$installed_target_hash" > "$EVIDENCE_DIR/installed-target-apk-verification.txt"
    expected_test_hash="$(awk '{print $1}' "$EVIDENCE_DIR/androidtest-apk.sha256")"
    installed_test_hash="$(sed -n 's/^installed_apk_sha256=//p' "$EVIDENCE_DIR/installed-apk-hashes.txt" | sed -n '2p')"
    [[ "$installed_test_hash" == "$expected_test_hash" ]]
    printf 'built_androidtest_sha256=%s\ninstalled_androidtest_sha256=%s\n' \
        "$expected_test_hash" "$installed_test_hash" > "$EVIDENCE_DIR/installed-androidtest-apk-verification.txt"

    echo 'download_attempted=true' > "$EVIDENCE_DIR/model-status.txt"
    fetch_bounded "$MODEL_URL" "$G3_MODEL_HOST_PATH" "$MODEL_SIZE_BYTES"
    local model_size
    model_size="$(stat -c '%s' "$G3_MODEL_HOST_PATH")"
    local model_hash
    model_hash="$(sha256sum "$G3_MODEL_HOST_PATH" | awk '{print $1}')"
    {
        echo 'downloaded_validated=false'
        echo "repository_revision=$MODEL_REPOSITORY_REVISION"
        echo "size_bytes=$model_size"
        echo "sha256=$model_hash"
    } > "$EVIDENCE_DIR/model-status.txt"
    [[ "$model_size" == "$MODEL_SIZE_BYTES" ]]
    [[ "$model_hash" == "$MODEL_SHA256" ]]
    {
        echo 'downloaded_validated=true'
        echo "repository_revision=$MODEL_REPOSITORY_REVISION"
        echo "size_bytes=$model_size"
        echo "sha256=$model_hash"
    } > "$EVIDENCE_DIR/model-status.txt"

    adb shell run-as "$APP_ID" mkdir -p files
    adb push "$G3_MODEL_HOST_PATH" "$DEVICE_MODEL_TEMP" > "$EVIDENCE_DIR/adb-model-stage.log" 2>&1
    G3_MODEL_PRIVATE_PATH_MAY_EXIST=1
    adb shell run-as "$APP_ID" cp "$DEVICE_MODEL_TEMP" "$DEVICE_MODEL_RELATIVE"
    adb shell run-as "$APP_ID" chmod 0400 "$DEVICE_MODEL_RELATIVE"
    device_data_dir="$(adb shell run-as "$APP_ID" pwd | tr -d '\r')"
    case "$device_data_dir" in
        "/data/user/0/$APP_ID"|"/data/data/$APP_ID") ;;
        *) echo "Unexpected app-private data root: $device_data_dir" >&2; return 1 ;;
    esac
    local private_model_path="$device_data_dir/$DEVICE_MODEL_RELATIVE"
    local staged_hash
    staged_hash="$(adb exec-out run-as "$APP_ID" cat "$DEVICE_MODEL_RELATIVE" | sha256sum | awk '{print $1}')"
    [[ "$staged_hash" == "$MODEL_SHA256" ]]
    echo "app_private_path=$private_model_path" > "$EVIDENCE_DIR/model-stage-metadata.txt"
    echo "app_private_sha256=$staged_hash" >> "$EVIDENCE_DIR/model-stage-metadata.txt"

    local gradle_test_log="$EVIDENCE_DIR/gradle-instrumentation.log"
    if ! run_poc_gradle --no-daemon --console=plain \
        -Pg3LlamaCppSource="$LLAMA_SOURCE" \
        "-Pandroid.testInstrumentationRunnerArguments.modelPath=$private_model_path" \
        "-Pandroid.testInstrumentationRunnerArguments.modelSha256=$MODEL_SHA256" \
        '-Pandroid.testInstrumentationRunnerArguments.class=app.serea.g3poc.OnDeviceInferenceTest#pinnedModelLoadsGeneratesCancelsReleasesAndRejectsCorruptFile' \
        :app:connectedDebugAndroidTest > "$gradle_test_log" 2>&1; then
        tail -n 160 "$gradle_test_log"
        return 1
    fi

    [[ -d "$G3_RESULTS_DIR" ]]
    python3 - "$G3_RESULTS_DIR" <<'PY'
from pathlib import Path
import sys
import xml.etree.ElementTree as ET

root = Path(sys.argv[1])
reports = sorted(root.rglob("TEST-*.xml"))
if len(reports) != 1:
    raise SystemExit(f"expected exactly one JUnit XML report, got {len(reports)}")
suite = ET.parse(reports[0]).getroot()
expected_name = "pinnedModelLoadsGeneratesCancelsReleasesAndRejectsCorruptFile"
expected_class = "app.serea.g3poc.OnDeviceInferenceTest"
cases = suite.findall("testcase")
if int(suite.attrib.get("tests", "0")) != 1 or len(cases) != 1:
    raise SystemExit("expected exactly one instrumented test case")
if cases[0].attrib.get("name") != expected_name:
    raise SystemExit("unexpected test method was executed")
if cases[0].attrib.get("classname") != expected_class:
    raise SystemExit("unexpected JUnit test class was executed")
for key in ("skipped", "failures", "errors"):
    if int(suite.attrib.get(key, "0")) != 0:
        raise SystemExit(f"JUnit {key} must be zero")
PY
    echo 'one_test_passed=true; skipped=0; failures=0; errors=0' > "$EVIDENCE_DIR/junit-verification.txt"
}

case "${1:-}" in
    host-metadata)
        collect_host_metadata
        ;;
    prepare)
        prepare_build
        ;;
    execute)
        run_model_test
        ;;
    test-download-bound)
        test_download_bound
        ;;
    test-cpu-pressure-optional)
        test_optional_emulator_cpu_pressure
        ;;
    test-cleanup-after-early-failure)
        test_cleanup_after_early_failure
        ;;
    host-after)
        {
            echo 'recorded_at_utc:'
            date -u +'%Y-%m-%dT%H:%M:%SZ'
            echo 'runner_cpu_pressure_after_emulator:'
            cat /proc/pressure/cpu
            echo 'runner_memory_after_emulator:'
            free -h
        } > "$EVIDENCE_DIR/host-after-emulator.txt"
        ;;
    *)
        echo "Usage: $0 {host-metadata|prepare|execute|host-after|test-download-bound|test-cpu-pressure-optional|test-cleanup-after-early-failure}" >&2
        exit 2
        ;;
esac
