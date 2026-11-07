#!/usr/bin/env bash
set -u

diagnostics_root="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/native-jni-diagnostics"
mkdir -p "$diagnostics_root"

set +e
./gradlew --no-daemon :app:connectedIsolatedAndroidTest --stacktrace -PsereaIsolatedNativeRuntime=true "-PsereaLlamaCppSource=${RUNNER_TEMP:-${TMPDIR:-/tmp}}/llama.cpp" -PsereaNativeRuntimeAbi=x86_64 -Pandroid.testInstrumentationRunnerArguments.class=app.serea.android.NativeJniReleaseContractTest
gradle_status=$?

adb logcat -d -v threadtime '*:E' > "$diagnostics_root/logcat-errors.txt" 2>&1 || true
if [[ -d app/build/outputs/androidTest-results/connected/isolated ]]; then
  cp -R app/build/outputs/androidTest-results/connected/isolated "$diagnostics_root/test-results" || true
fi
if [[ -d app/build/reports/androidTests/connected/isolated ]]; then
  cp -R app/build/reports/androidTests/connected/isolated "$diagnostics_root/test-report" || true
fi
exit "$gradle_status"
