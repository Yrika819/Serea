#!/usr/bin/env python3
"""Guard both diagnostic Gradle entry points to the standalone PoC project."""

from pathlib import Path
import re


SCRIPT = Path(__file__).with_name("run-diagnostic.sh")
source = SCRIPT.read_text(encoding="utf-8")


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

print("gradle_project_directory_test=PASS callers=prepare_build,run_model_test")
