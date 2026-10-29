#!/usr/bin/env python3
"""Verify that each Android variant produced its exact source-bound test partition."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

from verify_android_instrumentation import collect_results


def identities(directory: Path) -> set[str]:
    found: set[str] = set()
    for file in sorted(directory.rglob("*.xml")):
        root = ET.parse(file).getroot()
        for case in root.iter():
            if case.tag.rsplit("}", 1)[-1] != "testcase":
                continue
            class_name = case.get("classname")
            method_name = case.get("name")
            if not class_name or not method_name:
                raise ValueError(f"JUnit testcase is missing identity: {file}")
            identity = f"{class_name}#{method_name}"
            if identity in found:
                raise ValueError(f"Duplicate identity in partition: {identity}")
            found.add(identity)
    return found


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--filters", type=Path, required=True)
    parser.add_argument("--offline-junit", type=Path, required=True)
    parser.add_argument("--http-junit", type=Path, required=True)
    parser.add_argument("--evidence-dir", type=Path, required=True)
    parser.add_argument("--api-level", choices=("26", "35"), required=True)
    args = parser.parse_args()

    try:
        filters = json.loads(args.filters.read_text(encoding="utf-8"))
        source_sha = subprocess.run(
            ["git", "rev-parse", "HEAD"], check=True, capture_output=True, text=True
        ).stdout.strip()
        tree_sha = subprocess.run(
            ["git", "rev-parse", "HEAD^{tree}"], check=True, capture_output=True, text=True
        ).stdout.strip()
        if filters.get("source_sha") != source_sha or filters.get("tree_sha") != tree_sha:
            raise ValueError("Variant filters are not bound to the current exact source SHA/tree.")
        device_api = (args.evidence_dir / "device-api-level.txt").read_text(encoding="utf-8").strip()
        if device_api != args.api_level:
            raise ValueError(f"JUnit API binding mismatch: expected {args.api_level}, observed {device_api}.")
        provenance = (args.evidence_dir / "build-provenance.txt").read_text(encoding="utf-8")
        if f"source_checkout_sha={source_sha}\n" not in provenance or f"api_level={args.api_level}\n" not in provenance:
            raise ValueError("Build provenance does not bind the current source and API level.")

        offline_expected = set(filters["offline_identities"])
        http_expected = set(filters["http_identities"])
        if len(offline_expected) != 101 or len(http_expected) != 4 or offline_expected & http_expected:
            raise ValueError("Filter partition must contain 101 unique offline and 4 unique HTTP identities.")
        offline_results = collect_results(args.offline_junit)
        http_results = collect_results(args.http_junit)
        offline_actual = identities(args.offline_junit)
        http_actual = identities(args.http_junit)
        if offline_results.tests != 101 or offline_results.failures or offline_results.errors or offline_results.skipped:
            raise ValueError(f"Offline partition result is not 101 clean tests: {offline_results}")
        if http_results.tests != 4 or http_results.failures or http_results.errors or http_results.skipped:
            raise ValueError(f"HTTP partition result is not 4 clean tests: {http_results}")
        if offline_actual != offline_expected:
            raise ValueError(
                f"Offline partition identities differ: missing={sorted(offline_expected-offline_actual)} "
                f"unexpected={sorted(offline_actual-offline_expected)}"
            )
        if http_actual != http_expected:
            raise ValueError(
                f"HTTP partition identities differ: missing={sorted(http_expected-http_actual)} "
                f"unexpected={sorted(http_actual-http_expected)}"
            )
        if len(offline_actual | http_actual) != 105 or offline_actual & http_actual:
            raise ValueError("Partition JUnit union is not 105 unique identities.")
        print(
            f"android_instrumentation_partition_results=PASS source={source_sha} tree={tree_sha} "
            f"api={args.api_level} offline={len(offline_actual)} http={len(http_actual)} total=105"
        )
        return 0
    except (OSError, ValueError, KeyError, json.JSONDecodeError, ET.ParseError, subprocess.CalledProcessError) as error:
        print(f"android_instrumentation_partition_results=FAIL reason={error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
