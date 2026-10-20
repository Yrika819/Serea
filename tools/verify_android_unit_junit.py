#!/usr/bin/env python3
"""Fail unless the complete Android JVM unit-test task produced passing JUnit XML."""

from __future__ import annotations

import argparse
import sys
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Results:
    tests: int = 0
    failures: int = 0
    errors: int = 0
    skipped: int = 0
    suites: int = 0


def _number(value: str | None) -> int:
    return int(value or 0)


def _suite_counts(root: ET.Element) -> tuple[int, int, int, int] | None:
    tag = root.tag.rsplit("}", 1)[-1]
    if tag == "testsuite":
        suites = [root]
    elif tag == "testsuites":
        suites = list(root.findall("./testsuite"))
        if not suites and "tests" in root.attrib:
            suites = [root]
    else:
        return None

    totals = [0, 0, 0, 0]
    for suite in suites:
        cases = list(suite.findall("./testcase"))
        totals[0] += _number(suite.get("tests")) if "tests" in suite.attrib else len(cases)
        totals[1] += _number(suite.get("failures")) if "failures" in suite.attrib else sum(
            len(case.findall("./failure")) for case in cases
        )
        totals[2] += _number(suite.get("errors")) if "errors" in suite.attrib else sum(
            len(case.findall("./error")) for case in cases
        )
        totals[3] += _number(suite.get("skipped")) if "skipped" in suite.attrib else sum(
            len(case.findall("./skipped")) for case in cases
        )
    return tuple(totals)


def collect_results(directory: Path) -> Results:
    if not directory.is_dir():
        raise ValueError(f"Android unit JUnit result directory is missing: {directory}")
    files = sorted(directory.rglob("*.xml"))
    if not files:
        raise ValueError(f"No Android unit JUnit XML found; tests are NOT_RUN: {directory}")

    totals = [0, 0, 0, 0]
    used_suites = 0
    for file in files:
        try:
            root = ET.parse(file).getroot()
        except (ET.ParseError, OSError) as error:
            raise ValueError(f"Invalid Android unit JUnit XML: {file}: {error}") from error
        parsed = _suite_counts(root)
        if parsed is None:
            raise ValueError(f"Non-JUnit XML was found in the Android unit result directory: {file}")
        used_suites += 1
        if parsed[0] == 0:
            raise ValueError(f"Android unit JUnit suite recorded zero tests: {file}")
        for index, count in enumerate(parsed):
            totals[index] += count
    if used_suites == 0 or totals[0] == 0:
        raise ValueError("No Android JVM unit tests were recorded; tests are NOT_RUN.")
    return Results(*totals, suites=used_suites)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("results", type=Path, help="Gradle testDebugUnitTest JUnit result directory")
    parser.add_argument("--minimum", type=int, default=1, help="minimum number of executed tests")
    args = parser.parse_args()
    if args.minimum < 1:
        parser.error("--minimum must be at least 1")
    try:
        results = collect_results(args.results)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    print(
        "Android unit JUnit: "
        f"tests={results.tests} failures={results.failures} errors={results.errors} "
        f"skipped={results.skipped} suites={results.suites}"
    )
    if results.tests < args.minimum:
        print(f"FAIL: only {results.tests} Android unit tests were recorded; expected at least {args.minimum}.", file=sys.stderr)
        return 1
    if results.failures or results.errors or results.skipped:
        print("FAIL: every Android unit test must pass without failures, errors, or skips.", file=sys.stderr)
        return 1
    print("PASS: Android unit JUnit evidence is present and contains no failures, errors, or skips.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
