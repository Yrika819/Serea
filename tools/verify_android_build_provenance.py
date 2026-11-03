#!/usr/bin/env python3
"""Validate the APK and provenance manifest produced by Android Build."""
from __future__ import annotations

import argparse
import hashlib
import re
import subprocess
import sys
from pathlib import Path


TARGET_PATH = "android/app/build/outputs/apk/isolated/app-isolated.apk"
TEST_PATH = "android/app/build/outputs/apk/androidTest/isolated/app-isolated-androidTest.apk"
TARGET_PACKAGE = "app.serea.android.isolated"
TEST_PACKAGE = "app.serea.android.isolated.test"
DEBUG_PACKAGE = "app.serea.android.demo"
SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")
ISOLATED_FIELDS = {"isolated_apk_path", "isolated_apk_package", "isolated_apk_sha256"}


class ProvenanceError(ValueError):
    pass


def fail(path: Path, key: str, detail: str, api: int | None = None) -> None:
    prefix = f"API {api} " if api is not None else ""
    raise ProvenanceError(f"{prefix}{path}: key {key}: {detail}")


def parse_records(path: Path, api: int | None = None) -> list[tuple[str, str]]:
    if not path.is_file():
        fail(path, "<file>", "missing provenance file", api)
    records: list[tuple[str, str]] = []
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        if key in ISOLATED_FIELDS or key in {
            "source_commit", "build_checkout_commit", "github_run_id", "github_run_attempt",
            "package", "apk_sha256", "build_lint_instrumentation_compile",
        }:
            records.append((key, value))
    return records


def one(records: list[tuple[str, str]], path: Path, key: str, api: int | None = None) -> str:
    values = [value for record_key, value in records if record_key == key]
    if len(values) != 1:
        fail(path, key, f"occurs {len(values)} times; expected exactly 1", api)
    if not values[0]:
        fail(path, key, "value is empty", api)
    return values[0]


def sha256(path: Path, provenance: Path, key: str, api: int | None = None) -> str:
    if not path.is_file() or path.stat().st_size == 0:
        fail(provenance, key, f"APK is missing or empty: {path}", api)
    return hashlib.sha256(path.read_bytes()).hexdigest()


def package_id(aapt: Path, apk: Path, provenance: Path, key: str, api: int | None = None) -> str:
    try:
        result = subprocess.run([str(aapt), "dump", "badging", str(apk)], check=False,
                                capture_output=True, text=True)
    except OSError as exc:
        fail(provenance, key, f"could not execute aapt: {exc}", api)
    if result.returncode:
        fail(provenance, key, f"aapt failed for {apk}: {result.stderr.strip()}", api)
    matches = re.findall(r"(?m)^package: name='([^']+)'", result.stdout)
    if len(matches) != 1:
        fail(provenance, key, f"aapt returned {len(matches)} package IDs for {apk}; expected exactly 1", api)
    return matches[0]


def verify(args: argparse.Namespace) -> dict[str, str]:
    path = args.provenance
    api = args.api_level
    records = parse_records(path, api)
    for key, expected in (("source_commit", args.source_sha),
                          ("build_checkout_commit", args.build_sha),
                          ("github_run_id", args.run_id)):
        if one(records, path, key, api) != expected:
            fail(path, key, "does not match the expected value", api)
    attempt = one(records, path, "github_run_attempt", api)
    if not attempt.isdigit() or int(attempt) < 1:
        fail(path, "github_run_attempt", "must be a positive integer", api)
    if one(records, path, "build_lint_instrumentation_compile", api) != "PASS":
        fail(path, "build_lint_instrumentation_compile", "is not PASS", api)

    expected_records = [
        ("isolated_apk_path", TARGET_PATH),
        ("isolated_apk_package", TARGET_PACKAGE),
        ("isolated_apk_sha256", ""),
        ("isolated_apk_path", TEST_PATH),
        ("isolated_apk_package", TEST_PACKAGE),
        ("isolated_apk_sha256", ""),
    ]
    actual_records = [(key, value) for key, value in records if key in ISOLATED_FIELDS]
    if len(actual_records) != len(expected_records):
        fail(path, "isolated_apk_path/package/sha256",
             f"isolated APK fields contain {len(actual_records)} records; expected exactly 6 in target/test order", api)
    for index, ((actual_key, actual_value), (expected_key, expected_value)) in enumerate(
            zip(actual_records, expected_records, strict=True), start=1):
        if actual_key != expected_key:
            fail(path, actual_key, f"record {index} is out of order; expected {expected_key}", api)
        if expected_value and actual_value != expected_value:
            fail(path, actual_key, f"record {index} does not match expected value {expected_value}", api)

    target_sha = sha256(args.target_apk, path, "isolated_apk_sha256[target]", api)
    test_sha = sha256(args.test_apk, path, "isolated_apk_sha256[test]", api)
    recorded_target_sha = actual_records[2][1]
    recorded_test_sha = actual_records[5][1]
    for key, value in (("isolated_apk_sha256[target]", recorded_target_sha),
                       ("isolated_apk_sha256[test]", recorded_test_sha)):
        if not SHA256_RE.fullmatch(value):
            fail(path, key, "must be a lowercase SHA-256", api)
    if target_sha != recorded_target_sha:
        fail(path, "isolated_apk_sha256[target]", "does not match the target APK bytes", api)
    if test_sha != recorded_test_sha:
        fail(path, "isolated_apk_sha256[test]", "does not match the test APK bytes", api)

    for apk, expected, key in ((args.target_apk, TARGET_PACKAGE, "isolated_apk_package[target]"),
                               (args.test_apk, TEST_PACKAGE, "isolated_apk_package[test]"),
                               (args.debug_apk, DEBUG_PACKAGE, "package")):
        actual = package_id(args.aapt, apk, path, key, api)
        if actual != expected:
            fail(path, key, f"APK package is {actual}; expected {expected}", api)
    debug_package = one(records, path, "package", api)
    if debug_package != DEBUG_PACKAGE:
        fail(path, "package", f"is {debug_package}; expected {DEBUG_PACKAGE}", api)
    debug_sha = one(records, path, "apk_sha256", api)
    if not SHA256_RE.fullmatch(debug_sha):
        fail(path, "apk_sha256", "must be a lowercase SHA-256", api)
    if sha256(args.debug_apk, path, "apk_sha256", api) != debug_sha:
        fail(path, "apk_sha256", "does not match the debug APK bytes", api)

    return {"build_artifact_attempt": attempt, "target_package": TARGET_PACKAGE,
            "test_package": TEST_PACKAGE, "target_sha256": target_sha, "test_sha256": test_sha}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--provenance", type=Path, required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--build-sha", required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--api-level", type=int, choices=(26, 35))
    parser.add_argument("--target-apk", type=Path, required=True)
    parser.add_argument("--test-apk", type=Path, required=True)
    parser.add_argument("--debug-apk", type=Path, required=True)
    parser.add_argument("--aapt", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        values = verify(args)
        if args.output:
            args.output.write_text("".join(f"{key}={value}\n" for key, value in values.items()), encoding="utf-8")
    except (OSError, ProvenanceError) as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 1
    print("PASS: Build APK provenance, target/test correspondence, hashes, and package IDs validated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
