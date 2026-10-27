#!/usr/bin/env python3
"""Guard both diagnostic Gradle entry points to the standalone PoC project."""

from pathlib import Path
import re


SCRIPT = Path(__file__).with_name("run-diagnostic.sh")
source = SCRIPT.read_text(encoding="utf-8")
WORKFLOW = SCRIPT.parents[3] / ".github/workflows/g3-ondevice-poc.yml"
workflow = WORKFLOW.read_text(encoding="utf-8")


def function_body(name: str) -> str:
    match = re.search(
        rf"^{re.escape(name)}\(\) \{{(.*?)(?=^[A-Za-z_][A-Za-z0-9_]*\(\) (?:\{{|\()|^case\b|\Z)",
        source,
        flags=re.MULTILINE | re.DOTALL,
    )
    if match is None:
        raise SystemExit(f"missing shell function: {name}")
    return match.group(1)


runner = re.search(
    r"^run_poc_gradle\(\) \(\n(.*?)^\)",
    source,
    flags=re.MULTILINE | re.DOTALL,
)
if runner is None:
    raise SystemExit("missing run_poc_gradle subshell")
runner_body = runner.group(1)
if 'cd "$POC_DIR"' not in runner_body or 'exec ./gradlew "$@"' not in runner_body:
    raise SystemExit("run_poc_gradle must execute ./gradlew from POC_DIR")

for caller in ("prepare_build", "run_model_test"):
    body = function_body(caller)
    if body.count("run_poc_gradle ") != 1:
        raise SystemExit(f"{caller} must use run_poc_gradle exactly once")
    if "gradlew" in body:
        raise SystemExit(f"{caller} must not invoke gradlew directly")

test_source = SCRIPT.parents[1] / "app/src/androidTest/java/app/serea/g3poc/OnDeviceInferenceTest.kt"
android_test = test_source.read_text(encoding="utf-8")
if "InstrumentationRegistry.getInstrumentation().targetContext" not in android_test:
    raise SystemExit("AndroidTest must use InstrumentationRegistry instrumentation targetContext")
if "InstrumentationRegistry.getTargetContext()" in android_test:
    raise SystemExit("obsolete getTargetContext API call must not return")
for variable in ("pssAfterLoadKb", "pssAfterGenerationKb", "pssAfterCloseKb"):
    if not re.search(rf"var {variable} = 0L\b", android_test):
        raise SystemExit(f"{variable} must use Long for Debug.getPss()")

if "var racingLoad: kotlinx.coroutines.Deferred<Boolean>? = null" not in android_test:
    raise SystemExit("load-close race must return an expected result instead of failing its async child")
racing_load_start = android_test.find("racingLoad = async(Dispatchers.IO) {")
racing_close_start = android_test.find("racingClose = async(Dispatchers.IO)", racing_load_start)
if racing_load_start < 0 or racing_close_start < 0:
    raise SystemExit("load-close race async request boundaries are missing")
racing_load_body = android_test[racing_load_start:racing_close_start]
cancel_catch = racing_load_body.find("catch (cancelled: CancellationException)")
expected_catch = racing_load_body.find("catch (failure: IllegalStateException)")
if not all(
    marker in racing_load_body
    for marker in (
        "racingEngine.load(model.absolutePath)",
        "false",
        'if (failure.message == "Engine closed before model load") true else throw failure',
        "throw cancelled",
    )
) or cancel_catch < 0 or expected_catch < 0 or cancel_catch > expected_catch:
    raise SystemExit("race test must contain only the expected close rejection, preserve cancellation, and propagate unexpected failures")
if "val loadRejectedByClose = withTimeout(15_000) { checkNotNull(racingLoad).await() }" not in android_test:
    raise SystemExit("load-close race must await the successful Boolean result")

if 'EXPECTED_EMULATOR_API_LEVEL="${G3_API_LEVEL:-35}"' not in source:
    raise SystemExit("diagnostic runner must take its expected API from G3_API_LEVEL")
if '[[ "$actual_api_level" == "$EXPECTED_EMULATOR_API_LEVEL" ]]' not in source:
    raise SystemExit("diagnostic runner must reject an Emulator at the wrong selected API")
if 'echo "emulator_api: $actual_api_level"' not in source:
    raise SystemExit("diagnostic evidence must record the API actually reported by Android")
for marker in (
    "workflow_dispatch:",
    "api_level:",
    "inputs.api_level",
    "[g3-poc-run]",
    "[g3-api=26]",
    "[g3-api=35]",
    'system-images;android-${G3_API_LEVEL};google_apis;x86_64',
    "api-level: ${{ env.G3_API_LEVEL }}",
    "arch: x86_64",
    "disable-linux-hw-accel: false",
):
    if marker not in workflow:
        raise SystemExit(f"workflow must preserve explicit accelerated API selection contract: {marker}")
if "api-level: 35" in workflow:
    raise SystemExit("Emulator API level must not be hardcoded after API selection was added")

prepare = function_body("prepare_build")
for marker in (
    'dump permissions "$APP_APK"',
    'dump permissions "$test_apk"',
    'androidtest-apk-permissions.txt',
    "Standalone PoC target APK must not request INTERNET permission",
    "Standalone PoC AndroidTest APK must not request INTERNET permission",
):
    if marker not in prepare:
        raise SystemExit(f"both generated APK permission surfaces must be recorded and deny INTERNET: {marker}")

if "python3 tools/g3-ondevice-poc/scripts/test-api-selection.py" not in workflow:
    raise SystemExit("G3 workflow must execute host API marker regression tests")
if "python3 tools/g3-ondevice-poc/scripts/api_selection.py \"$GITHUB_EVENT_PATH\" \"$G3_API_LEVEL\"" not in workflow:
    raise SystemExit("G3 workflow must validate event markers against the selected API before SDK setup")
if workflow.find("scripts/api_selection.py") > workflow.find("actions/setup-java"):
    raise SystemExit("API marker validation must happen before SDK setup")

cancellation_start = android_test.find("native abort callback test gate must arm")
cancellation_end = android_test.find("val recoveryStarted", cancellation_start)
if cancellation_start < 0 or cancellation_end < 0:
    raise SystemExit("cancellation test boundaries are missing")
cancellation_test = android_test[cancellation_start:cancellation_end]
for marker in (
    "armAbortCallbackGateForTest()",
    "awaitAbortCallbackGateForTest(30, TimeUnit.SECONDS)",
    "longRun.cancel()",
    "releaseAbortCallbackGateForTest()",
    "abortCallbackCancellationsForTest() > 0",
):
    if marker not in cancellation_test:
        raise SystemExit(f"in-compute cancellation test must gate and assert the native abort callback: {marker}")
if "awaitStartGateForTest" in cancellation_test:
    raise SystemExit("cancellation proof must not use the pre-decode generation-start gate")

source_root = SCRIPT.parents[1] / "app/src"
native_source = source_root / "main/cpp/llama_jni.cpp"
native = native_source.read_text(encoding="utf-8")
abort_start = native.find("bool abort_requested(void * user_data)")
abort_end = native.find("\n}\n", abort_start)
if abort_start < 0 or abort_end < 0:
    raise SystemExit("native llama abort callback implementation is missing")
abort_body = native[abort_start:abort_end]
for marker in (
    "abort_test_gate_changed.wait",
    "generation_cancelled(request.engine_id, request.generation_id)",
    "return cancelled;",
):
    if marker not in abort_body:
        raise SystemExit(f"native abort callback must gate and observe cancellation: {marker}")
callback_install = native.find("llama_set_abort_callback(context, abort_requested, &request)")
first_decode = native.find("llama_decode(context, prompt_batch)")
if callback_install < 0 or first_decode < 0 or callback_install > first_decode:
    raise SystemExit("native cancellation callback must be installed before prompt decode")

network_apis = re.compile(r"java\.net\.|HttpURLConnection|HttpClient|OkHttp|Retrofit|Socket|URL\(")
for suffix in ("*.kt", "*.java"):
    for source_file in source_root.rglob(suffix):
        if network_apis.search(source_file.read_text(encoding="utf-8")):
            raise SystemExit(f"app source must not add network I/O for this local inference PoC: {source_file}")

print("runner_and_androidtest_source_contract=PASS gradle_callers=prepare_build,run_model_test load_close_async_failure_isolated=true cancellation_asserts_native_abort_callback=true target_and_androidtest_internet_denied=true api_marker_validation_pre_sdk=true explicit_api26_or_35=true x86_64_acceleration_required=true app_network_io=false")
