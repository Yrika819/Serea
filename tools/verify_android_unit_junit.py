#!/usr/bin/env python3
"""Fail unless the complete Android JVM unit-test task produced passing JUnit XML."""

from __future__ import annotations

import argparse
import re
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


CORE_REQUIRED_CLASSES = (
    "app.serea.android.ChatViewModelTest",
    "app.serea.android.calendar.CalendarSecurityTest",
    "app.serea.android.calendar.transport.GoogleCalendarHttpReadPortTest",
    "app.serea.android.gmail.GmailSecurityPortsTest",
    "app.serea.android.googleauth.GoogleAuthorizationTest",
    "app.serea.android.assistant.taskproposal.TaskProposalTest",
    "app.serea.android.tasks.TaskRepositoryTest",
    "app.serea.android.tasks.TaskViewModelTest",
)


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


def _source_test_count(source: str) -> int:
    source = re.sub(r'""".*?"""', "", source, flags=re.DOTALL)
    source = re.sub(r'"(?:\\.|[^"\\])*"', '""', source)
    source = re.sub(r"'(?:\\.|[^'\\])*'", "''", source)
    source = re.sub(r"/\*.*?\*/", "", source, flags=re.DOTALL)
    source = re.sub(r"//[^\n]*", "", source)
    return len(re.findall(r"@(?:org\.junit\.)?Test\b", source))


def collect_source_class_counts(source_root: Path) -> dict[str, int]:
    if not source_root.is_dir():
        raise ValueError(f"Android unit test source directory is missing: {source_root}")
    counts: dict[str, int] = {}
    source_files = sorted((*source_root.rglob("*.kt"), *source_root.rglob("*.java")))
    for file in source_files:
        try:
            source = file.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as error:
            raise ValueError(f"Could not read Android unit test source: {file}: {error}") from error
        package = re.search(r"^\s*package\s+([A-Za-z_][\w.]*)", source, flags=re.MULTILINE)
        classes = list(re.finditer(r"^(?:(?:public|internal|private|open|abstract|final|static)\s+)*class\s+([A-Za-z_]\w*)", source, flags=re.MULTILINE))
        if package is None:
            raise ValueError(f"Test source must declare a package and class: {file}")
        for index, declaration in enumerate(classes):
            end = classes[index + 1].start() if index + 1 < len(classes) else len(source)
            tests = _source_test_count(source[declaration.start():end])
            if tests == 0:
                continue
            class_name = f"{package.group(1)}.{declaration.group(1)}"
            if class_name in counts:
                raise ValueError(f"Duplicate Android unit test class source: {class_name}")
            counts[class_name] = tests
    if not counts:
        raise ValueError(f"No @Test methods found under Android unit source directory: {source_root}")
    return counts


def collect_junit_class_counts(directory: Path) -> dict[str, int]:
    counts: dict[str, int] = {}
    for file in sorted(directory.rglob("*.xml")):
        try:
            root = ET.parse(file).getroot()
        except (ET.ParseError, OSError) as error:
            raise ValueError(f"Invalid Android unit JUnit XML: {file}: {error}") from error
        if root.tag.rsplit("}", 1)[-1] not in {"testsuite", "testsuites"}:
            raise ValueError(f"Non-JUnit XML was found in the Android unit result directory: {file}")
        for case in root.iter():
            if case.tag.rsplit("}", 1)[-1] != "testcase":
                continue
            class_name = case.get("classname")
            if not class_name:
                raise ValueError(f"JUnit testcase has no classname: {file}")
            counts[class_name] = counts.get(class_name, 0) + 1
    return counts


def verify_source_discovery(directory: Path, source_root: Path, required_classes: tuple[str, ...]) -> None:
    expected = collect_source_class_counts(source_root)
    actual = collect_junit_class_counts(directory)
    missing_required = sorted(set(required_classes) - set(expected))
    if missing_required:
        raise ValueError("Required Android unit test source class is missing: " + ", ".join(missing_required))
    missing = sorted(set(expected) - set(actual))
    unexpected = sorted(set(actual) - set(expected))
    count_mismatches = sorted(
        (name, expected[name], actual[name])
        for name in set(expected) & set(actual)
        if expected[name] != actual[name]
    )
    if missing or unexpected or count_mismatches:
        details = []
        if missing:
            details.append("missing JUnit classes: " + ", ".join(missing))
        if unexpected:
            details.append("unexpected JUnit classes: " + ", ".join(unexpected))
        if count_mismatches:
            details.append("test count mismatch: " + ", ".join(f"{name} source={source} junit={junit}" for name, source, junit in count_mismatches))
        raise ValueError("Android unit source/JUnit discovery mismatch: " + "; ".join(details))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("results", type=Path, help="Gradle testDebugUnitTest JUnit result directory")
    parser.add_argument("--minimum", type=int, default=1, help="minimum number of executed tests")
    parser.add_argument("--source-root", type=Path, help="Kotlin unit-test source root for source/JUnit discovery comparison")
    parser.add_argument("--require-class", action="append", default=None, help="critical source test class that must remain present (repeatable)")
    args = parser.parse_args()
    if args.minimum < 1:
        parser.error("--minimum must be at least 1")
    try:
        results = collect_results(args.results)
        if args.source_root:
            required_classes = tuple(dict.fromkeys((*CORE_REQUIRED_CLASSES, *(args.require_class or ()))))
            verify_source_discovery(args.results, args.source_root, required_classes)
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
