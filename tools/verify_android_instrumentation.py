#!/usr/bin/env python3
"""Fail unless Gradle produced passing Android instrumentation JUnit XML."""

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
    files: int = 0


def _number(value: str | None) -> int:
    return int(value or 0)


def _suites(root: ET.Element) -> list[ET.Element] | None:
    tag = root.tag.rsplit("}", 1)[-1]
    if tag == "testsuite":
        return [root]
    elif tag == "testsuites":
        return [suite for suite in root.iter() if suite.tag.rsplit("}", 1)[-1] == "testsuite"]
    return None


def _case_identity(case: ET.Element, file: Path) -> tuple[str, str]:
    class_name = case.get("classname")
    method_name = case.get("name")
    if not class_name or not method_name:
        raise ValueError(f"JUnit testcase must include classname and name: {file}")
    return class_name, method_name


def _suite_counts(root: ET.Element, file: Path) -> tuple[int, int, int, int] | None:
    suites = _suites(root)
    if suites is None:
        return None
    totals = [0, 0, 0, 0]
    if not suites:
        raise ValueError(f"Instrumentation JUnit XML has no testsuites: {file}")
    for suite in suites:
        cases = list(suite.findall("./testcase"))
        declared_tests = _number(suite.get("tests")) if "tests" in suite.attrib else len(cases)
        if declared_tests == 0 or not cases:
            raise ValueError(f"Instrumentation JUnit suite recorded zero tests: {file}")
        if declared_tests != len(cases):
            raise ValueError(f"JUnit testcase count does not match tests attribute: {file}")
        totals[0] += declared_tests
        child_results = tuple(
            sum(bool(case.findall(f"./{result_tag}")) for case in cases)
            for result_tag in ("failure", "error", "skipped")
        )
        for attribute, child_count in zip(("failures", "errors", "skipped"), child_results):
            if attribute in suite.attrib and _number(suite.get(attribute)) != child_count:
                raise ValueError(f"JUnit {attribute} attribute does not match testcase results: {file}")
        totals[1:] = [total + child for total, child in zip(totals[1:], child_results)]
    return tuple(totals)


def collect_results(directory: Path) -> Results:
    files = sorted(directory.rglob("*.xml"))
    totals = [0, 0, 0, 0]
    used_files = 0
    seen: set[tuple[str, str]] = set()
    for file in files:
        try:
            parsed = _suite_counts(ET.parse(file).getroot(), file)
        except (ET.ParseError, OSError) as error:
            raise ValueError(f"Invalid instrumentation JUnit XML: {file}: {error}") from error
        if parsed is None:
            raise ValueError(f"Non-JUnit XML was found in the instrumentation result directory: {file}")
        root = ET.parse(file).getroot()
        for case in root.iter():
            if case.tag.rsplit("}", 1)[-1] != "testcase":
                continue
            identity = _case_identity(case, file)
            if identity in seen:
                raise ValueError(f"Duplicate instrumentation JUnit testcase: {identity[0]}#{identity[1]}")
            seen.add(identity)
        used_files += 1
        for index, count in enumerate(parsed):
            totals[index] += count
    return Results(*totals, files=used_files)


def collect_class_counts(directory: Path) -> dict[str, int]:
    counts: dict[str, int] = {}
    for file in sorted(directory.rglob("*.xml")):
        try:
            root = ET.parse(file).getroot()
        except (ET.ParseError, OSError) as error:
            raise ValueError(f"Invalid instrumentation JUnit XML: {file}: {error}") from error
        for case in root.iter():
            if case.tag.rsplit("}", 1)[-1] != "testcase":
                continue
            class_name, _ = _case_identity(case, file)
            counts[class_name] = counts.get(class_name, 0) + 1
    return counts


def _source_test_count(source: str) -> int:
    # Ignore comments and literals so examples do not look like executable tests.
    source = re.sub(r'""".*?"""', "", source, flags=re.DOTALL)
    source = re.sub(r'"(?:\\.|[^"\\])*"', '""', source)
    source = re.sub(r"'(?:\\.|[^'\\])*'", "''", source)
    source = re.sub(r"/\*.*?\*/", "", source, flags=re.DOTALL)
    source = re.sub(r"//[^\n]*", "", source)
    return len(re.findall(r"@(?:org\.junit\.)?Test\b", source))


def _source_test_methods(source: str) -> set[str]:
    source = re.sub(r'""".*?"""', "", source, flags=re.DOTALL)
    source = re.sub(r'"(?:\\.|[^"\\])*"', '""', source)
    source = re.sub(r"'(?:\\.|[^'\\])*'", "''", source)
    source = re.sub(r"/\*.*?\*/", "", source, flags=re.DOTALL)
    source = re.sub(r"//[^\n]*", "", source)
    pattern = re.compile(
        r"@(?:org\.junit\.)?Test\b(?:\s*\([^)]*\))?\s*"
        r"(?:(?:public|private|protected|internal|suspend|open|override|final)\s+)*"
        r"(?:fun\s+([A-Za-z_]\w*)|(?:[\w<>?.\[\]]+\s+)+([A-Za-z_]\w*)\s*\()"
    )
    return {match.group(1) or match.group(2) for match in pattern.finditer(source)}


def collect_source_test_counts(
    source_root: Path, requirements: dict[str, int]
) -> tuple[dict[str, int], int, dict[str, set[str]]]:
    if not source_root.is_dir():
        raise ValueError(f"Instrumentation Kotlin source directory is missing: {source_root}")
    total = 0
    for file in sorted(source_root.rglob("*.kt")):
        try:
            total += _source_test_count(file.read_text(encoding="utf-8"))
        except (OSError, UnicodeError) as error:
            raise ValueError(f"Could not read instrumentation source: {file}: {error}") from error

    class_counts: dict[str, int] = {}
    source_methods: dict[str, set[str]] = {}
    for class_name in requirements:
        file = source_root.joinpath(*class_name.split(".")).with_suffix(".kt")
        try:
            source = file.read_text(encoding="utf-8")
            class_counts[class_name] = _source_test_count(source)
            source_methods[class_name] = _source_test_methods(source)
        except (OSError, UnicodeError) as error:
            raise ValueError(f"Required instrumentation test source is missing or unreadable: {file}: {error}") from error
    return class_counts, total, source_methods


def collect_junit_methods(directory: Path) -> dict[str, set[str]]:
    methods: dict[str, set[str]] = {}
    for file in sorted(directory.rglob("*.xml")):
        try:
            root = ET.parse(file).getroot()
        except (ET.ParseError, OSError) as error:
            raise ValueError(f"Invalid instrumentation JUnit XML: {file}: {error}") from error
        if _suites(root) is None:
            raise ValueError(f"Non-JUnit XML was found in the instrumentation result directory: {file}")
        for case in root.iter():
            if case.tag.rsplit("}", 1)[-1] != "testcase":
                continue
            class_name, method_name = _case_identity(case, file)
            methods.setdefault(class_name, set()).add(method_name)
    return methods


def parse_class_requirements(requirements: list[str], parser: argparse.ArgumentParser) -> dict[str, int]:
    parsed: dict[str, int] = {}
    for requirement in requirements:
        try:
            class_name, count_text = requirement.rsplit("=", 1)
            expected = int(count_text)
            if not class_name or expected < 1 or class_name in parsed:
                raise ValueError
        except ValueError:
            parser.error(f"invalid --require-class value: {requirement!r}")
        parsed[class_name] = expected
    return parsed


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("results", type=Path, help="Gradle connected Android test results directory")
    parser.add_argument("--minimum", type=int, default=9, help="minimum number of executed tests (default: 9)")
    parser.add_argument(
        "--require-class", action="append", default=[], metavar="CLASS=COUNT",
        help="require an exact executed testcase count for this fully qualified test class",
    )
    parser.add_argument(
        "--source-root", type=Path,
        help="also require the exact @Test method count in every required Kotlin source and across this directory",
    )
    args = parser.parse_args()
    if args.minimum < 1:
        parser.error("--minimum must be at least 1")
    required_classes = parse_class_requirements(args.require_class, parser)
    source_total: int | None = None
    if args.source_root is not None:
        try:
            source_counts, source_total, source_methods = collect_source_test_counts(args.source_root, required_classes)
        except ValueError as error:
            print(error, file=sys.stderr)
            return 2
        for class_name, expected in required_classes.items():
            actual = source_counts[class_name]
            if actual != expected:
                print(
                    f"FAIL: {class_name} source contains {actual} @Test methods; expected exactly {expected}.",
                    file=sys.stderr,
                )
                return 1
        expected_total = sum(required_classes.values())
        print(f"Instrumentation source: tests={source_total} required_class_tests={expected_total}")
        if source_total != expected_total:
            print(
                f"FAIL: instrumentation source contains {source_total} @Test methods across the source set; "
                f"all {expected_total} must belong to the required class list.",
                file=sys.stderr,
            )
            return 1
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
    if source_total is not None and results.tests != source_total:
        print(
            f"FAIL: JUnit recorded {results.tests} tests but Kotlin source contains {source_total} @Test methods.",
            file=sys.stderr,
        )
        return 1
    if results.failures or results.errors or results.skipped:
        print("FAIL: every instrumentation test must pass without failures, errors, or skips.", file=sys.stderr)
        return 1
    class_counts = collect_class_counts(args.results)
    junit_methods = collect_junit_methods(args.results)
    for class_name, expected in required_classes.items():
        actual = class_counts.get(class_name, 0)
        if actual != expected:
            print(
                f"FAIL: {class_name} recorded {actual} tests; expected exactly {expected}.",
                file=sys.stderr,
            )
            return 1
        if source_total is not None and source_methods[class_name] != junit_methods.get(class_name, set()):
            missing = sorted(source_methods[class_name] - junit_methods.get(class_name, set()))
            unexpected = sorted(junit_methods.get(class_name, set()) - source_methods[class_name])
            print(
                f"FAIL: {class_name} source/JUnit method mismatch; missing={missing}, unexpected={unexpected}.",
                file=sys.stderr,
            )
            return 1
    print("PASS: instrumentation JUnit evidence meets the required threshold.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
