#!/usr/bin/env python3
"""Fail unless Gradle produced passing Android instrumentation JUnit XML."""

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
    files: int = 0


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
        if "failures" in suite.attrib:
            totals[1] += _number(suite.get("failures"))
        else:
            totals[1] += sum(len(case.findall("./failure")) for case in cases)
        if "errors" in suite.attrib:
            totals[2] += _number(suite.get("errors"))
        else:
            totals[2] += sum(len(case.findall("./error")) for case in cases)
        if "skipped" in suite.attrib:
            totals[3] += _number(suite.get("skipped"))
        else:
            totals[3] += sum(len(case.findall("./skipped")) for case in cases)
    return tuple(totals)


def collect_results(directory: Path) -> Results:
    files = sorted(directory.rglob("*.xml"))
    totals = [0, 0, 0, 0]
    used_files = 0
    for file in files:
        try:
            parsed = _suite_counts(ET.parse(file).getroot())
        except (ET.ParseError, OSError) as error:
            raise ValueError(f"Invalid instrumentation JUnit XML: {file}: {error}") from error
        if parsed is None:
            continue
        used_files += 1
        for index, count in enumerate(parsed):
            totals[index] += count
    return Results(*totals, files=used_files)


def collect_class_counts(directory: Path) -> dict[str, int]:
    counts: dict[str, int] = {}
    for file in sorted(directory.rglob("*.xml")):
        try:
            root = ET.parse(file).getroot()
        except (ET.ParseError, OSError):
            continue  # collect_results reports malformed JUnit XML with the path.
        for case in root.iter():
            if case.tag.rsplit("}", 1)[-1] != "testcase":
                continue
            class_name = case.get("classname")
            if class_name:
                counts[class_name] = counts.get(class_name, 0) + 1
    return counts


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("results", type=Path, help="Gradle connected Android test results directory")
    parser.add_argument("--minimum", type=int, default=9, help="minimum number of executed tests (default: 9)")
    parser.add_argument(
        "--require-class", action="append", default=[], metavar="CLASS=COUNT",
        help="require an exact executed testcase count for this fully qualified test class",
    )
    args = parser.parse_args()
    if args.minimum < 1:
        parser.error("--minimum must be at least 1")
    try:
        results = collect_results(args.results)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    print(
        "Instrumentation JUnit: "
        f"tests={results.tests} failures={results.failures} "
        f"errors={results.errors} skipped={results.skipped} "
        f"suites={results.files}"
    )
    if results.files == 0:
        print("FAIL: no instrumentation JUnit XML found; tests are NOT_RUN.", file=sys.stderr)
        return 1
    if results.tests < args.minimum:
        print(
            f"FAIL: only {results.tests} tests were recorded; expected at least {args.minimum}.",
            file=sys.stderr,
        )
        return 1
    if results.failures or results.errors or results.skipped:
        print("FAIL: every instrumentation test must pass without failures, errors, or skips.", file=sys.stderr)
        return 1
    class_counts = collect_class_counts(args.results)
    for requirement in args.require_class:
        try:
            class_name, count_text = requirement.rsplit("=", 1)
            expected = int(count_text)
            if not class_name or expected < 1:
                raise ValueError
        except ValueError:
            parser.error(f"invalid --require-class value: {requirement!r}")
        actual = class_counts.get(class_name, 0)
        if actual != expected:
            print(
                f"FAIL: {class_name} recorded {actual} tests; expected exactly {expected}.",
                file=sys.stderr,
            )
            return 1
    print("PASS: instrumentation JUnit evidence meets the required threshold.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
