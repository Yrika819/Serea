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


def collect_failure_details(directory: Path) -> list[str]:
    details: list[str] = []
    for file in sorted(directory.rglob("*.xml")):
        root = ET.parse(file).getroot()
        for case in root.iter():
            if case.tag.rsplit("}", 1)[-1] != "testcase":
                continue
            class_name, method_name = _case_identity(case, file)
            for result in (*case.findall("./failure"), *case.findall("./error")):
                detail = result.get("message") or " ".join("".join(result.itertext()).split()) or "no failure detail"
                details.append(f"{class_name}#{method_name}: {detail[:320]}")
    return details


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


def collect_source_test_identities(source_root: Path) -> set[tuple[str, str]]:
    identities: set[tuple[str, str]] = set()
    for file in sorted(source_root.rglob("*.kt")):
        try:
            source = file.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as error:
            raise ValueError(f"Could not read instrumentation test source: {file}: {error}") from error
        methods = _source_test_methods(source)
        if not methods:
            continue
        package_match = re.search(r"(?m)^\s*package\s+([A-Za-z_][\w.]*)\s*$", source)
        class_name = f"{package_match.group(1)}.{file.stem}" if package_match else file.stem
        identities.update((class_name, method) for method in methods)
    return identities


def read_test_manifest(path: Path) -> set[tuple[str, str]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeError) as error:
        raise ValueError(f"Could not read instrumentation test manifest: {path}: {error}") from error
    identities: set[tuple[str, str]] = set()
    for line_number, raw_line in enumerate(lines, start=1):
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        if line.count("#") != 1:
            raise ValueError(f"Invalid instrumentation test identity at {path}:{line_number}: {line!r}")
        class_name, method_name = line.split("#", 1)
        identity = (class_name.strip(), method_name.strip())
        if not all(identity) or identity in identities:
            raise ValueError(f"Invalid or duplicate instrumentation test identity at {path}:{line_number}: {line!r}")
        identities.add(identity)
    return identities


def verify_profile_verifier_evidence(path: Path, api_level: int) -> tuple[bool, str]:
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError as error:
        return False, f"could not read ProfileVerifier runtime evidence: {path}: {error}"
    pattern = re.compile(
        r"SereaProfileVerifierEvidence.*?\bapi=(\d+)\s+status=([A-Z_]+)\s+"
        r"resultCode=(\d+).*?\bembedded=(true|false)\s+marker=([a-z]+)\b"
    )
    matches = [pattern.search(line) for line in lines]
    evidence = [match.groups() for match in matches if match and int(match.group(1)) == api_level]
    if not evidence:
        return False, f"no SereaProfileVerifierEvidence runtime record found for API {api_level}"
    api_text, status, result_code_text, embedded_text, marker = evidence[-1]
    result_code = int(result_code_text)
    if api_level == 26:
        valid = status == "UNSUPPORTED" and result_code == (4 << 16) and marker == "absent"
        expected = "status=UNSUPPORTED resultCode=262144 marker=absent"
    elif api_level == 35:
        # AndroidX code 2 means the profile is installed and queued for background compilation.
        installed_profile_codes = {1, 2, 3}
        valid = (
            status == "COMPLETED"
            and result_code in installed_profile_codes
            and embedded_text == "true"
            and marker == "valid"
        )
        expected = "status=COMPLETED, resultCode in {1, 2, 3}, embedded=true, and marker=valid"
    else:
        return False, f"ProfileVerifier evidence validation is not defined for API {api_level}"
    if not valid:
        return False, (
            f"API {api_text} ProfileVerifier evidence was status={status} "
            f"resultCode={result_code} embedded={embedded_text} marker={marker}; expected {expected}"
        )
    return True, (
        f"ProfileVerifier runtime evidence: API {api_level} status={status} "
        f"resultCode={result_code} embedded={embedded_text} marker={marker}"
    )


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


def parse_test_requirements(requirements: list[str], parser: argparse.ArgumentParser) -> set[tuple[str, str]]:
    parsed: set[tuple[str, str]] = set()
    for requirement in requirements:
        try:
            class_name, method_name = requirement.rsplit("#", 1)
            if not class_name or not method_name or (class_name, method_name) in parsed:
                raise ValueError
        except ValueError:
            parser.error(f"invalid --require-test value: {requirement!r}")
        parsed.add((class_name, method_name))
    return parsed


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("results", type=Path, help="Gradle connected Android test results directory")
    parser.add_argument("--minimum", type=int, default=9, help="minimum number of executed tests (default: 9)")
    parser.add_argument("--exact-count", type=int, help="require exactly this many recorded testcases")
    parser.add_argument(
        "--require-class", action="append", default=[], metavar="CLASS=COUNT",
        help="require an exact executed testcase count for this fully qualified test class",
    )
    parser.add_argument(
        "--require-test", action="append", default=[], metavar="CLASS#METHOD",
        help="require an exact passing testcase identity",
    )
    parser.add_argument(
        "--required-test-manifest", type=Path,
        help="exact Original Required Set identities, one CLASS#METHOD per line",
    )
    parser.add_argument(
        "--additional-test-manifest", type=Path,
        help="exact additional regression-test identities, one CLASS#METHOD per line",
    )
    parser.add_argument("--required-count", type=int, help="require this exact Original Required Set size")
    parser.add_argument("--additional-count", type=int, help="require this exact Additional Tests size")
    parser.add_argument("--api-level", type=int, choices=(26, 35), help="emulator API for ProfileVerifier evidence")
    parser.add_argument(
        "--profile-verifier-log", type=Path,
        help="logcat file containing positive SereaProfileVerifierEvidence runtime output",
    )
    parser.add_argument(
        "--source-root", type=Path,
        help="also require the exact @Test method count in every required Kotlin source and across this directory",
    )
    args = parser.parse_args()
    if args.minimum < 1:
        parser.error("--minimum must be at least 1")
    if args.exact_count is not None and args.exact_count < 1:
        parser.error("--exact-count must be at least 1")
    required_classes = parse_class_requirements(args.require_class, parser)
    required_tests = parse_test_requirements(args.require_test, parser)
    if (args.required_test_manifest is None) != (args.additional_test_manifest is None):
        parser.error("--required-test-manifest and --additional-test-manifest must be used together")
    manifest_required: set[tuple[str, str]] = set()
    manifest_additional: set[tuple[str, str]] = set()
    if args.required_test_manifest is not None and args.additional_test_manifest is not None:
        try:
            manifest_required = read_test_manifest(args.required_test_manifest)
            manifest_additional = read_test_manifest(args.additional_test_manifest)
        except ValueError as error:
            print(error, file=sys.stderr)
            return 2
        if not manifest_required or not manifest_additional:
            print("Original Required Set and Additional Tests manifests must both be non-empty.", file=sys.stderr)
            return 2
        if args.api_level is None or args.profile_verifier_log is None:
            parser.error("manifest-based Required Gate verification also requires --api-level and --profile-verifier-log")
        overlap = manifest_required & manifest_additional
        if overlap:
            print(f"Original Required Set and Additional Tests overlap: {sorted(overlap)}", file=sys.stderr)
            return 2
        if args.required_count is not None and len(manifest_required) != args.required_count:
            print(
                f"Original Required Set contains {len(manifest_required)} identities; expected {args.required_count}.",
                file=sys.stderr,
            )
            return 1
        if args.additional_count is not None and len(manifest_additional) != args.additional_count:
            print(
                f"Additional Tests contains {len(manifest_additional)} identities; expected {args.additional_count}.",
                file=sys.stderr,
            )
            return 1
        manifest_required_classes = {class_name for class_name, _ in manifest_required}
        for class_name in required_classes:
            if class_name not in manifest_required_classes:
                print(f"Required class is absent from Original Required Set: {class_name}", file=sys.stderr)
                return 1
        required_class_counts = {
            class_name: sum(identity[0] == class_name for identity in manifest_required)
            for class_name in required_classes
        }
        if required_classes and required_class_counts != required_classes:
            print(
                "Original Required Set class counts do not match --require-class: "
                f"manifest={required_class_counts} expected={required_classes}.",
                file=sys.stderr,
            )
            return 1
        if required_tests & (manifest_required | manifest_additional):
            print("--require-test identities must not duplicate either manifest.", file=sys.stderr)
            return 1
    all_required_tests = manifest_required | manifest_additional | required_tests
    source_total: int | None = None
    source_identities: set[tuple[str, str]] | None = None
    if args.source_root is not None:
        try:
            source_counts, source_total, source_methods = collect_source_test_counts(args.source_root, required_classes)
            if args.required_test_manifest is not None:
                source_identities = collect_source_test_identities(args.source_root)
        except ValueError as error:
            print(error, file=sys.stderr)
            return 2
        for class_name, expected in required_classes.items():
            actual = source_counts[class_name]
            additional_class_count = sum(identity[0] == class_name for identity in manifest_additional)
            expected_source_count = expected + additional_class_count
            if actual != expected_source_count:
                print(
                    f"FAIL: {class_name} source contains {actual} @Test methods; expected Original Required Set "
                    f"{expected} plus Additional Tests {additional_class_count} = {expected_source_count}.",
                    file=sys.stderr,
                )
                return 1
        if args.required_test_manifest is not None:
            assert source_identities is not None
            expected_source_identities = manifest_required | manifest_additional
            if source_identities != expected_source_identities:
                print(
                    "FAIL: source instrumentation identities do not match the two manifests; "
                    f"missing={sorted(expected_source_identities - source_identities)}, "
                    f"unexpected={sorted(source_identities - expected_source_identities)}.",
                    file=sys.stderr,
                )
                return 1
            if source_total != len(expected_source_identities):
                print(
                    f"FAIL: instrumentation source counted {source_total} tests but the manifests contain "
                    f"{len(expected_source_identities)} identities.", file=sys.stderr,
                )
                return 1
            print(
                f"Instrumentation source: Original Required Set={len(manifest_required)} "
                f"Additional Tests={len(manifest_additional)} total={source_total}"
            )
        else:
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
    if args.exact_count is not None and results.tests != args.exact_count:
        print(
            f"FAIL: {results.tests} tests were recorded; expected exactly {args.exact_count}.",
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
        for detail in collect_failure_details(args.results):
            print(f"JUnit failure detail: {detail}", file=sys.stderr)
        print("FAIL: every instrumentation test must pass without failures, errors, or skips.", file=sys.stderr)
        return 1
    class_counts = collect_class_counts(args.results)
    junit_methods = collect_junit_methods(args.results)
    actual_tests = {
        (class_name, method_name)
        for class_name, methods in junit_methods.items()
        for method_name in methods
    }
    if all_required_tests and (results.tests != len(all_required_tests) or actual_tests != all_required_tests):
        print(
            f"FAIL: JUnit testcase identities do not match the required set; "
            f"missing={sorted(all_required_tests - actual_tests)}, "
            f"unexpected={sorted(actual_tests - all_required_tests)}.",
            file=sys.stderr,
        )
        return 1
    if args.required_test_manifest is not None:
        print(
            f"Original Required Set: {sum(identity in actual_tests for identity in manifest_required)}/"
            f"{len(manifest_required)} PASS; Additional Tests: "
            f"{sum(identity in actual_tests for identity in manifest_additional)}/"
            f"{len(manifest_additional)} PASS; total={results.tests}"
        )
    for class_name, expected in required_classes.items():
        actual = class_counts.get(class_name, 0)
        additional_class_count = sum(identity[0] == class_name for identity in manifest_additional)
        expected_junit_count = expected + additional_class_count
        if actual != expected_junit_count:
            print(
                f"FAIL: {class_name} recorded {actual} tests; expected Original Required Set "
                f"{expected} plus Additional Tests {additional_class_count} = {expected_junit_count}.",
                file=sys.stderr,
            )
            return 1
        expected_class_methods = {
            method for class_name_value, method in all_required_tests if class_name_value == class_name
        }
        if source_total is not None and source_methods[class_name] != expected_class_methods:
            missing = sorted(expected_class_methods - source_methods[class_name])
            unexpected = sorted(source_methods[class_name] - expected_class_methods)
            print(
                f"FAIL: {class_name} manifest/source method mismatch; missing={missing}, unexpected={unexpected}.",
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
    if args.required_test_manifest is not None:
        assert args.api_level is not None and args.profile_verifier_log is not None
        profile_ok, profile_message = verify_profile_verifier_evidence(
            args.profile_verifier_log, args.api_level,
        )
        if not profile_ok:
            print(f"FAIL: {profile_message}", file=sys.stderr)
            return 1
        print(profile_message)
    print("PASS: instrumentation JUnit evidence meets the required threshold.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
