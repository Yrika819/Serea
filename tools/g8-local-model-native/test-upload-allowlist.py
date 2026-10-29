#!/usr/bin/env python3
"""Protect Native CI artifacts from raw instrumentation and logcat uploads."""

import sys
from pathlib import Path


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: test-upload-allowlist.py WORKFLOW", file=sys.stderr)
        return 2
    workflow = Path(sys.argv[1]).read_text(encoding="utf-8")
    forbidden = (
        "android-instrumentation.txt",
        "android-logcat-diagnostic.log",
        "native-runtime-metrics.log",
    )
    if any(name in workflow for name in forbidden):
        print("workflow must not list raw runtime logs as upload paths", file=sys.stderr)
        return 1
    if "path: ${{ steps.native_e2e.outputs.evidence_dir }}\n" in workflow:
        print("whole native evidence directory must not be uploaded", file=sys.stderr)
        return 1
    required_safe_files = (
        "${{ steps.native_e2e.outputs.evidence_dir }}/junit-verification.txt",
        "${{ steps.native_e2e.outputs.evidence_dir }}/native-runtime-metrics-summary.txt",
        "${{ steps.arm64_native_e2e.outputs.evidence_dir }}/native-runtime-metrics-summary.txt",
    )
    missing = [name for name in required_safe_files if name not in workflow]
    if missing:
        print("safe Native evidence upload allowlist is incomplete", file=sys.stderr)
        return 1
    print("native_artifact_allowlist=PASS raw_instrumentation_and_logcat=EXCLUDED")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
