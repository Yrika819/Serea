#!/usr/bin/env python3
"""Protect Native CI artifacts from raw instrumentation and logcat uploads."""

import re
import sys
from pathlib import Path


RAW_DIAGNOSTIC_NAMES = (
    "android-instrumentation.txt",
    "android-logcat-diagnostic.log",
    "native-runtime-metrics.log",
    "emulator.log",
    "adb-devices.txt",
)

SAFE_UPLOAD_PATHS = {
    "evidence/g8-local-native/source.txt",
    "evidence/g8-local-native/gradle-native-build.log",
    "evidence/g8-local-native/gradle-jvm-focused.log",
    "evidence/g8-local-native/junit/TEST-app.serea.android.LocalModelChatAdapterTest.xml",
    "evidence/g8-local-native/junit/TEST-app.serea.android.LocalModelNativeRuntimeTest.xml",
    "${{ steps.native_e2e.outputs.evidence_dir }}/preflight.txt",
    "${{ steps.native_e2e.outputs.evidence_dir }}/source-and-runtime.txt",
    "${{ steps.native_e2e.outputs.evidence_dir }}/model-stage.txt",
    "${{ steps.native_e2e.outputs.evidence_dir }}/junit/native-runtime-instrumentation.xml",
    "${{ steps.native_e2e.outputs.evidence_dir }}/junit-verification.txt",
    "${{ steps.native_e2e.outputs.evidence_dir }}/instrumentation-command-status.txt",
    "${{ steps.native_e2e.outputs.evidence_dir }}/native-runtime-metrics-summary.txt",
    "${{ steps.native_e2e.outputs.evidence_dir }}/cleanup-boundary.txt",
    "evidence/native-arm64-guest/host-and-image.txt",
    "evidence/native-arm64-guest/startup-summary.txt",
    "${{ steps.arm64_native_e2e.outputs.evidence_dir }}/native-runtime-metrics-summary.txt",
    "${{ steps.arm64_native_e2e.outputs.evidence_dir }}/source-and-runtime.txt",
    "${{ steps.arm64_native_e2e.outputs.evidence_dir }}/junit/native-runtime-instrumentation.xml",
    "${{ steps.arm64_native_e2e.outputs.evidence_dir }}/junit-verification.txt",
    "${{ steps.arm64_native_e2e.outputs.evidence_dir }}/cleanup-boundary.txt",
}


def validate_workflow(workflow: str) -> list[str]:
    problems: list[str] = []
    path_blocks = re.findall(
        r"(?m)^[ \t]*path:[ \t]*\|[ \t]*\n((?:[ \t]+[^\n]*\n)+)", workflow
    )
    uploaded_paths = "\n".join(path_blocks)
    if any(name in uploaded_paths for name in RAW_DIAGNOSTIC_NAMES):
        problems.append("workflow must not retain or upload raw runtime diagnostics")
    path_entries = {line.strip() for line in uploaded_paths.splitlines() if line.strip()}
    unexpected_paths = path_entries - SAFE_UPLOAD_PATHS
    if unexpected_paths:
        problems.append("artifact upload contains paths outside the exact-file allowlist")
    if any(re.search(r"[*?\[\]]", path) for path in path_entries):
        problems.append("artifact upload must not use glob metacharacters")
    missing = SAFE_UPLOAD_PATHS - path_entries
    if missing:
        problems.append("safe Native evidence upload allowlist is incomplete")
    return problems


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: test-upload-allowlist.py WORKFLOW", file=sys.stderr)
        return 2
    workflow = Path(sys.argv[1]).read_text(encoding="utf-8")
    problems = validate_workflow(workflow)
    if problems:
        print("; ".join(problems), file=sys.stderr)
        return 1

    # Prove that the guard rejects raw diagnostics, guest/evidence directory globs,
    # and omission of the sanitized JUnit test report.
    safe_arm_file = "            evidence/native-arm64-guest/host-and-image.txt\n"
    bad_fixtures = (
        workflow.replace(
            safe_arm_file,
            "            evidence/native-arm64-guest/emulator.log\n",
            1,
        ),
        workflow.replace(
            safe_arm_file,
            "            evidence/native-arm64-guest/\n",
            1,
        ),
        workflow.replace(
            safe_arm_file,
            "            evidence/native-arm64-guest/*\n",
            1,
        ),
        workflow.replace(
            safe_arm_file,
            "            ${{ steps.arm64_native_e2e.outputs.evidence_dir }}/**\n",
            1,
        ),
        workflow.replace(
            safe_arm_file,
            "            ${{ steps.arm64_native_e2e.outputs.evidence_dir }}\n",
            1,
        ),
        workflow.replace(
            "${{ steps.arm64_native_e2e.outputs.evidence_dir }}/junit/native-runtime-instrumentation.xml\n",
            "",
            1,
        ),
    )
    if any(not validate_workflow(fixture) for fixture in bad_fixtures):
        print("artifact guard accepted a raw path, directory wildcard, or missing JUnit report", file=sys.stderr)
        return 1
    print("native_artifact_allowlist=PASS raw_logs=EXCLUDED explicit_paths=PASS sanitized_junit=REQUIRED")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
