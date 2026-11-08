#!/usr/bin/env bash
set -euo pipefail

model_path="${1:?usage: run_native_gguf_integration.sh VERIFIED_MODEL_PATH}"
package_name="app.serea.android.isolated"
model_name="serea-r7-smollm2-135m-q4_k_m.gguf"
diagnostics_root="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/native-gguf-diagnostics"
mkdir -p "$diagnostics_root"
test -f "$model_path"
test "$(stat -c %s "$model_path")" = 101016128
test "$(sha256sum "$model_path" | awk '{print $1}')" = e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5

./gradlew --no-daemon :app:installIsolated \
  -PsereaIsolatedNativeRuntime=true \
  "-PsereaLlamaCppSource=${RUNNER_TEMP:-${TMPDIR:-/tmp}}/llama.cpp" \
  -PsereaNativeRuntimeAbi=x86_64

cleanup_device_model() {
  adb shell run-as "$package_name" rm -f "files/$model_name" >/dev/null 2>&1 || true
}
trap cleanup_device_model EXIT

adb shell run-as "$package_name" mkdir -p files
adb exec-in "run-as $package_name sh -c 'cat > files/$model_name'" < "$model_path"
adb shell run-as "$package_name" test -s "files/$model_name"

set +e
./gradlew --no-daemon :app:connectedIsolatedAndroidTest --stacktrace \
  -PsereaIsolatedNativeRuntime=true \
  "-PsereaLlamaCppSource=${RUNNER_TEMP:-${TMPDIR:-/tmp}}/llama.cpp" \
  -PsereaNativeRuntimeAbi=x86_64 \
  -Pandroid.testInstrumentationRunnerArguments.class=app.serea.android.NativeGgufLifecycleTest
gradle_status=$?
set -e

metrics_file="$diagnostics_root/native-gguf-metrics.properties"
if adb shell run-as "$package_name" cat files/native-gguf-metrics.properties \
  > "$metrics_file" 2>/dev/null && [[ -s "$metrics_file" ]]; then
  # Metrics contain timings, PSS, token counts and provenance only; model output is never recorded.
  cat "$metrics_file"
elif [[ "$gradle_status" -eq 0 ]]; then
  echo 'Native GGUF test passed without writing its required measurement record.' >&2
  gradle_status=1
fi
adb shell run-as "$package_name" rm -f "files/$model_name" >/dev/null 2>&1 || true
adb shell am force-stop "$package_name" >/dev/null 2>&1 || true
if find "$diagnostics_root" -type f -iname '*.gguf' -print -quit | grep -q .; then
  echo 'Model weights must never enter diagnostic artifacts.' >&2
  exit 1
fi
if [[ -d app/build/outputs/androidTest-results/connected/isolated ]]; then
  cp -R app/build/outputs/androidTest-results/connected/isolated "$diagnostics_root/test-results" || true
fi
if [[ -d app/build/reports/androidTests/connected/isolated ]]; then
  cp -R app/build/reports/androidTests/connected/isolated "$diagnostics_root/test-report" || true
fi
exit "$gradle_status"
