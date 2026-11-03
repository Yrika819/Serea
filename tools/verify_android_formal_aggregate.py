#!/usr/bin/env python3
"""Fail-closed validation for the parallel Android Formal API artifacts."""
from __future__ import annotations

import argparse
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def read_value(path: Path, key: str) -> str:
    require(path.is_file(), f"missing evidence file: {path}")
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith(key + "="):
            return line.split("=", 1)[1]
    raise ValueError(f"missing {key} in {path}")


def read_status(path: Path) -> str:
    require(path.is_file(), f"missing evidence file: {path}")
    return path.read_text(encoding="utf-8").strip()


def verify_junit(root: Path) -> None:
    require(root.is_dir(), f"missing JUnit directory: {root}")
    files = sorted(root.rglob("*.xml"))
    require(files, f"JUnit XML missing: {root}")
    count = skipped = failures = errors = 0
    for file in files:
        tree = ET.parse(file)
        for case in tree.getroot().iter():
            if case.tag.rsplit("}", 1)[-1] == "testcase":
                count += 1
                skipped += sum(x.tag.rsplit("}", 1)[-1] == "skipped" for x in case)
                failures += sum(x.tag.rsplit("}", 1)[-1] == "failure" for x in case)
                errors += sum(x.tag.rsplit("}", 1)[-1] == "error" for x in case)
    require(count == 105, f"JUnit count {count}, expected 105")
    require(skipped == failures == errors == 0, f"JUnit skipped={skipped}, failures={failures}, errors={errors}")


def verify_api(api: int, root: Path, job_result: str, args: argparse.Namespace) -> None:
    evidence = root / f"api{api}"
    require(job_result == "success", f"API {api} job result is {job_result}")
    provenance = evidence / f"api{api}-build-provenance.txt"
    require(read_status(evidence / "api-result.txt") == "PASS", f"API {api} api-result is not PASS")
    for key, expected in (("source_sha", args.source_sha), ("build_checkout_sha", args.build_sha),
                          ("run_id", args.run_id), ("artifact_producer", f"android-emulator-api{api}")):
        require(read_value(provenance, key) == expected, f"API {api} {key} mismatch")
    for key, expected in (("target_package", "app.serea.android.isolated"),
                          ("test_package", "app.serea.android.isolated.test")):
        require(read_value(provenance, key) == expected, f"API {api} {key} mismatch")
    for key in ("target_sha256", "test_sha256"):
        value = read_value(provenance, key)
        require(len(value) == 64 and all(c in "0123456789abcdef" for c in value), f"API {api} invalid {key}")
    producer_attempt = read_value(provenance, "producer_attempt")
    require(producer_attempt.isdigit() and int(producer_attempt) > 0, f"API {api} invalid producer_attempt")
    campaign = evidence / "build-provenance.txt"
    require(read_value(campaign, "source_checkout_sha") == args.build_sha, f"API {api} campaign checkout SHA mismatch")
    require(read_value(campaign, "campaign_run_id") == args.run_id, f"API {api} campaign Run ID mismatch")
    require(read_value(campaign, "campaign_attempt") == producer_attempt, f"API {api} campaign Attempt mismatch")
    for filename in ("instrumentation-status.txt", "process-death-status.txt", "data-isolation-status.txt", "google-demo-ui-status.txt"):
        require(read_status(evidence / filename) == "PASS", f"API {api} {filename} is not PASS")
    require((evidence / "process-death.log").is_file() and (evidence / "process-death.log").stat().st_size > 0,
            f"API {api} Process Death log missing")
    verify_junit(evidence / "junit")
    command = [sys.executable, str(args.junit_verifier), str(evidence / "junit"),
               "--source-root", str(args.source_root), "--minimum", "105", "--exact-count", "105",
               "--required-test-manifest", str(args.required_manifest),
               "--additional-test-manifest", str(args.additional_manifest),
               "--non-required-test-manifest", str(args.non_required_manifest),
               "--required-count", "103", "--additional-count", "2", "--api-level", str(api)]
    for test_class, count in (("app.serea.android.EncryptedHistoryTest", 17),
                              ("app.serea.android.ChatScreenTest", 16),
                              ("app.serea.android.ChatHttpE2ETest", 4),
                              ("app.serea.android.UsageLedgerDatabaseTest", 12),
                              ("app.serea.android.tasks.TasksDatabaseTest", 6),
                              ("app.serea.android.tasks.TaskScreenTest", 6),
                              ("app.serea.android.InstrumentationStorageIsolationTest", 1),
                              ("app.serea.android.ChatTasksDatabaseIntegrationTest", 1),
                              ("app.serea.android.ChatHistoryUxTest", 4)):
        command.extend(("--require-class", f"{test_class}={count}"))
    result = subprocess.run(command, check=False)
    require(result.returncode == 0, f"API {api} formal JUnit verifier exited {result.returncode}")


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--root", type=Path, required=True)
    p.add_argument("--source-sha", required=True)
    p.add_argument("--build-sha", required=True)
    p.add_argument("--run-id", required=True)
    p.add_argument("--api26-result", required=True)
    p.add_argument("--api35-result", required=True)
    p.add_argument("--build-result", required=True)
    p.add_argument("--output", type=Path)
    p.add_argument("--junit-verifier", type=Path, required=True)
    p.add_argument("--source-root", type=Path, required=True)
    p.add_argument("--required-manifest", type=Path, required=True)
    p.add_argument("--additional-manifest", type=Path, required=True)
    p.add_argument("--non-required-manifest", type=Path, required=True)
    args = p.parse_args()
    failures = []
    for api, result in ((26, args.api26_result), (35, args.api35_result)):
        try:
            verify_api(api, args.root, result, args)
            print(f"PASS: API {api} evidence validated")
        except (ValueError, OSError, ET.ParseError) as exc:
            failures.append(f"API {api}: {exc}")
            print(f"FAIL: API {api}: {exc}", file=sys.stderr)
    if args.build_result != "success":
        failures.append(f"Build job result is {args.build_result}")
    if failures:
        if args.output:
            with args.output.open("a", encoding="utf-8") as output:
                output.write("result=FAIL\n")
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        return 1
    if args.output:
        with args.output.open("a", encoding="utf-8") as output:
            output.write("result=PASS\n")
    print("PASS: both API campaigns and Build validated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
