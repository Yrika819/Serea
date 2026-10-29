#!/usr/bin/env python3
"""Verify permission separation for offline and loopback-only instrumentation APKs."""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import sys
import zipfile
from pathlib import Path

INTERNET = "android.permission.INTERNET"
NATIVE_LIBRARY = "libserea_local_llama.so"


def inspect_apk(aapt: Path, apk: Path, expected_package: str, internet_expected: bool) -> tuple[str, bool, str]:
    if not apk.is_file() or apk.stat().st_size == 0:
        raise ValueError(f"APK is missing or empty: {apk}")
    result = subprocess.run(
        [str(aapt), "dump", "permissions", str(apk)],
        check=True,
        capture_output=True,
        text=True,
    )
    lines = result.stdout.splitlines()
    packages = [line.partition(":")[2].strip() for line in lines if line.startswith("package:")]
    if packages != [expected_package]:
        raise ValueError(f"Unexpected package in {apk}: expected={expected_package!r} actual={packages!r}")
    internet_granted = any(INTERNET in line for line in lines if line.startswith("uses-permission:"))
    if internet_granted != internet_expected:
        raise ValueError(
            f"Unexpected INTERNET permission in {apk}: expected={internet_expected} actual={internet_granted}"
        )
    digest = hashlib.sha256(apk.read_bytes()).hexdigest()
    return expected_package, internet_granted, digest


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--aapt", type=Path, required=True)
    parser.add_argument("--offline-target", type=Path, required=True)
    parser.add_argument("--offline-test", type=Path, required=True)
    parser.add_argument("--http-target", type=Path, required=True)
    parser.add_argument("--http-test", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    args = parser.parse_args()

    try:
        if not args.aapt.is_file() or not args.aapt.stat().st_mode & 0o111:
            raise ValueError(f"aapt is not an executable file: {args.aapt}")
        records = []
        for name, apk_path, package, internet_expected in (
            ("offline_target", args.offline_target, "app.serea.android.isolated", False),
            ("offline_test", args.offline_test, "app.serea.android.isolated.test", False),
            ("http_target", args.http_target, "app.serea.android.isolated", True),
            ("http_test", args.http_test, "app.serea.android.isolated.test", True),
        ):
            actual_package, internet_granted, digest = inspect_apk(
                args.aapt, apk_path, package, internet_expected
            )
            records.append((name, apk_path, actual_package, internet_granted, digest))
        for name, apk_path, _, _, _ in records:
            if name.startswith("http_"):
                with zipfile.ZipFile(apk_path) as archive:
                    if any(Path(item).name == NATIVE_LIBRARY for item in archive.namelist()):
                        raise ValueError(f"HTTP test-only APK unexpectedly contains {NATIVE_LIBRARY}: {apk_path}")
        args.evidence.parent.mkdir(parents=True, exist_ok=True)
        with args.evidence.open("w", encoding="utf-8") as output:
            for name, _, package, internet_granted, digest in records:
                output.write(
                    f"{name}_package={package}\n{name}_internet={'GRANTED' if internet_granted else 'DENIED'}\n"
                    f"{name}_apk_sha256={digest}\n"
                )
            output.write("http_test_native_jni=ABSENT\n")
        print("android_http_test_apk_permissions=PASS offline=DENIED http_fixture=GRANTED native_jni=ABSENT")
        return 0
    except (OSError, ValueError, subprocess.CalledProcessError, zipfile.BadZipFile) as error:
        print(f"android_http_test_apk_permissions=FAIL reason={error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
