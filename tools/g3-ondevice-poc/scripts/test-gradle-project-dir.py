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

print("runner_and_androidtest_source_contract=PASS gradle_callers=prepare_build,run_model_test load_close_async_failure_isolated=true cancellation_preserved=true unexpected_failure_preserved=true explicit_api26_or_35=true x86_64_acceleration_required=true")
