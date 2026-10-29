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


def validate_workflow(workflow: str) -> list[str]:
    problems: list[str] = []
    path_blocks = re.findall(
        r"(?m)^[ \t]*path:[ \t]*\|[ \t]*\n((?:[ \t]+[^\n]*\n)+)", workflow
    )
    uploaded_paths = "\n".join(path_blocks)
    if any(name in uploaded_paths for name in RAW_DIAGNOSTIC_NAMES):
        problems.append("workflow must not retain or upload raw runtime diagnostics")
    # An artifact path that names a whole evidence directory bypasses the explicit
    # per-file allowlist, even if a second artifact currently lists safe files.
    directory_uploads = (
        r"(?m)^[ \t]*(?:-\s*)?evidence/native-arm64-guest/?[ \t]*$",
        r"(?m)^[ \t]*(?:-\s*)?\$\{\{\s*steps\.(?:native_e2e|arm64_native_e2e)\.outputs\.evidence_dir\s*\}\}[ \t]*$",
    )
    if any(re.search(pattern, uploaded_paths) for pattern in directory_uploads):
        problems.append("artifact upload must use explicit evidence-file paths, not a directory")
    if "path: ${{ steps.native_e2e.outputs.evidence_dir }}\n" in workflow:
        problems.append("whole native evidence directory must not be uploaded")
    required_safe_files = (
        "${{ steps.native_e2e.outputs.evidence_dir }}/junit-verification.txt",
        "${{ steps.native_e2e.outputs.evidence_dir }}/native-runtime-metrics-summary.txt",
        "${{ steps.arm64_native_e2e.outputs.evidence_dir }}/native-runtime-metrics-summary.txt",
        "${{ steps.arm64_native_e2e.outputs.evidence_dir }}/junit/native-runtime-instrumentation.xml",
        "evidence/native-arm64-guest/host-and-image.txt",
        "evidence/native-arm64-guest/startup-summary.txt",
    )
    missing = [name for name in required_safe_files if name not in workflow]
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

    # Prove that the guard rejects a raw diagnostic file, both evidence-directory
    # wildcard spellings, and omission of the sanitized JUnit test report.
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
