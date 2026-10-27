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
DEVICE_MODEL_TEMP="/data/local/tmp/g3-smollm.gguf"
DEVICE_MODEL_RELATIVE="files/g3-smollm.gguf"
APP_APK="$APP_DIR/build/outputs/apk/debug/app-debug.apk"
TEST_APK_DIR="$APP_DIR/build/outputs/apk/androidTest/debug"
DOWNLOAD_TEST_DIRECTORY=''

mkdir -p "$EVIDENCE_DIR"

run_poc_gradle() (
    cd "$POC_DIR"
    exec ./gradlew "$@"
)

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
    local model_host_path="$RUNNER_TEMP/g3-smollm.gguf"
    local model_private_path_may_exist=0
    local app_installed=0
    local runner_status=0
    local device_data_dir=''
    local results_dir="$APP_DIR/build/outputs/androidTest-results/connected"

    cleanup_emulator() {
        runner_status=$?
        trap - EXIT INT TERM
        set +e
        if adb get-state >/dev/null 2>&1; then
            if [[ -d "$results_dir" ]]; then
                mkdir -p "$EVIDENCE_DIR/androidTest-results"
                cp -a "$results_dir"/. "$EVIDENCE_DIR/androidTest-results/" || true
            fi
            adb logcat -d -v threadtime > "$EVIDENCE_DIR/logcat.txt" 2>&1
            adb shell cat /proc/pressure/cpu > "$EVIDENCE_DIR/emulator-cpu-pressure-after.txt" 2>&1
            adb shell cat /proc/meminfo > "$EVIDENCE_DIR/emulator-meminfo-after.txt" 2>&1
            adb shell dumpsys meminfo "$APP_ID" > "$EVIDENCE_DIR/app-meminfo-after.txt" 2>&1
            local cleanup_status=0
            if [[ "$model_private_path_may_exist" -eq 1 ]]; then
                adb shell run-as "$APP_ID" rm -f "$DEVICE_MODEL_RELATIVE" || cleanup_status=1
            fi
            adb shell rm -f "$DEVICE_MODEL_TEMP" || cleanup_status=1
            if [[ "$model_private_path_may_exist" -eq 1 ]]; then
                if adb shell run-as "$APP_ID" test '!' -e "$DEVICE_MODEL_RELATIVE"; then
                    echo 'app_private_model_removed=true' > "$EVIDENCE_DIR/model-cleanup.txt"
                else
                    echo 'app_private_model_removed=false' > "$EVIDENCE_DIR/model-cleanup.txt"
                    cleanup_status=1
                fi
            else
                echo 'app_private_model_staged=false' > "$EVIDENCE_DIR/model-cleanup.txt"
            fi
            if adb shell test '!' -e "$DEVICE_MODEL_TEMP"; then
                echo 'emulator_temp_model_removed=true' >> "$EVIDENCE_DIR/model-cleanup.txt"
            else
                echo 'emulator_temp_model_removed=false' >> "$EVIDENCE_DIR/model-cleanup.txt"
                cleanup_status=1
            fi
            if [[ "$app_installed" -eq 1 ]]; then
                adb uninstall "$TEST_ID" >> "$EVIDENCE_DIR/model-cleanup.txt" 2>&1 || cleanup_status=1
                adb uninstall "$APP_ID" >> "$EVIDENCE_DIR/model-cleanup.txt" 2>&1 || cleanup_status=1
            fi
            if [[ "$cleanup_status" -ne 0 && "$runner_status" -eq 0 ]]; then runner_status=1; fi
        else
            echo 'emulator_unreachable_during_cleanup=true' > "$EVIDENCE_DIR/model-cleanup.txt"
            if [[ "$runner_status" -eq 0 ]]; then runner_status=1; fi
        fi
        if [[ -f "$model_host_path" ]]; then
            python3 - "$model_host_path" <<'PY'
from pathlib import Path
import sys
Path(sys.argv[1]).unlink(missing_ok=True)
PY
        fi
        exit "$runner_status"
    }
    trap cleanup_emulator EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM

    [[ -s "$APP_APK" ]]
    (cd "$ROOT_DIR" && sha256sum --check "$EVIDENCE_DIR/app-apk.sha256")
    (cd "$ROOT_DIR" && sha256sum --check "$EVIDENCE_DIR/androidtest-apk.sha256")
    adb wait-for-device
    [[ "$(adb shell getprop sys.boot_completed | tr -d '\r')" == '1' ]]
    [[ "$(adb shell getprop ro.build.version.sdk | tr -d '\r')" == '35' ]]
    [[ "$(adb shell getprop ro.product.cpu.abi | tr -d '\r')" == 'x86_64' ]]
    {
        echo 'emulator_api: 35'
        echo "emulator_abi: $(adb shell getprop ro.product.cpu.abi | tr -d '\r')"
        echo "emulator_hardware: $(adb shell getprop ro.hardware | tr -d '\r')"
        echo "emulator_fingerprint: $(adb shell getprop ro.build.fingerprint | tr -d '\r')"
        echo "boot_completed: $(adb shell getprop sys.boot_completed | tr -d '\r')"
    } > "$EVIDENCE_DIR/emulator-metadata.txt"
    adb shell cat /proc/pressure/cpu > "$EVIDENCE_DIR/emulator-cpu-pressure-before.txt"
    adb shell cat /proc/meminfo > "$EVIDENCE_DIR/emulator-meminfo-before.txt"

    adb install -r "$APP_APK" > "$EVIDENCE_DIR/adb-install-target.log" 2>&1
    app_installed=1
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
    fetch_bounded "$MODEL_URL" "$model_host_path" "$MODEL_SIZE_BYTES"
    local model_size
    model_size="$(stat -c '%s' "$model_host_path")"
    local model_hash
    model_hash="$(sha256sum "$model_host_path" | awk '{print $1}')"
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
    adb push "$model_host_path" "$DEVICE_MODEL_TEMP" > "$EVIDENCE_DIR/adb-model-stage.log" 2>&1
    model_private_path_may_exist=1
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

    [[ -d "$results_dir" ]]
    python3 - "$results_dir" <<'PY'
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
        echo "Usage: $0 {host-metadata|prepare|execute|host-after|test-download-bound}" >&2
        exit 2
        ;;
esac
