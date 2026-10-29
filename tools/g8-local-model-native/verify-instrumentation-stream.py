#!/usr/bin/env python3
"""Validate raw AndroidJUnitRunner status output and write a JUnit XML diagnostic."""

from __future__ import annotations

import sys
import xml.etree.ElementTree as ET
from pathlib import Path


def parse_stream(text: str) -> tuple[list[dict[str, str]], dict[str, str]]:
    terminal_codes = {"0", "-2", "-3", "-4"}
    fields: dict[str, str] = {}
    cases: list[dict[str, str]] = []
    result: dict[str, str] = {}
    for raw_line in text.splitlines():
        line = raw_line.strip("\r")
        if line.startswith("INSTRUMENTATION_STATUS: "):
            key_value = line[len("INSTRUMENTATION_STATUS: "):]
            key, separator, value = key_value.partition("=")
            if separator:
                fields[key] = value
        elif line.startswith("INSTRUMENTATION_RESULT: "):
            key_value = line[len("INSTRUMENTATION_RESULT: "):]
            key, separator, value = key_value.partition("=")
            if separator:
                result[key] = value
        elif line.startswith("INSTRUMENTATION_CODE: "):
            result["instrumentation_code"] = line[len("INSTRUMENTATION_CODE: "):].strip()
        elif "stack" in fields and line and not line.startswith("INSTRUMENTATION_"):
            # AndroidJUnitRunner may emit the first stack line as a status value and
            # continue the cause chain on unprefixed lines before the status code.
            fields["stack"] += "\n" + line
        elif line.startswith("INSTRUMENTATION_STATUS_CODE: "):
            code = line[len("INSTRUMENTATION_STATUS_CODE: "):].strip()
            if code in terminal_codes:
                case = dict(fields)
                case["status_code"] = code
                cases.append(case)
                fields = {}
    return cases, result


def main() -> int:
    if len(sys.argv) != 6:
        print(
            "usage: verify-instrumentation-stream.py INPUT OUTPUT SUMMARY NOOP_CLASS NATIVE_CLASS",
            file=sys.stderr,
        )
        return 2
    input_path, xml_path, summary_path = map(Path, sys.argv[1:4])
    noop_class, native_class = sys.argv[4:6]
    stream = input_path.read_text(encoding="utf-8", errors="replace")
    cases, result = parse_stream(stream)

    identities: dict[str, dict[str, str]] = {}
    invalid: list[str] = []
    for case in cases:
        class_name = case.get("class", "").strip()
        method = case.get("test", "").strip()
        status_code = case["status_code"]
        if not class_name or not method:
            invalid.append(f"status {status_code} lacked class/test identity")
            continue
        key = f"{class_name}#{method}"
        if status_code == "1":
            continue
        if key in identities:
            invalid.append("duplicate terminal status for " + key)
        identities[key] = case

    expected_noop = f"{noop_class}#androidJUnit4RunnerInstantiatesAndTargetsIsolatedApp"
    expected_native = f"{native_class}#pinnedModelRunsThroughLocalModelEgressUsageAndNativeBackend"
    noop_result = identities.get(expected_noop)
    native_result = identities.get(expected_native)
    native_initialization = identities.get(f"{native_class}#initializationError")
    noop_initialization = identities.get(f"{noop_class}#initializationError")
    allowed_identities = {
        expected_noop,
        expected_native,
        f"{noop_class}#initializationError",
        f"{native_class}#initializationError",
    }
    unexpected_identities = sorted(set(identities) - allowed_identities)
    if unexpected_identities:
        invalid.append("unexpected identities: " + ", ".join(unexpected_identities))
    expected_pairs = {
        frozenset((expected_noop, expected_native)),
        frozenset((expected_noop, f"{native_class}#initializationError")),
        frozenset((f"{noop_class}#initializationError", expected_native)),
        frozenset((f"{noop_class}#initializationError", f"{native_class}#initializationError")),
    }
    if frozenset(identities) not in expected_pairs:
        invalid.append("terminal test identities do not match one no-op/native outcome pair")
    noop_status_present = noop_result is not None or noop_initialization is not None
    native_status_present = native_result is not None or native_initialization is not None
    if not noop_status_present:
        invalid.append("no-op identity or initializationError was not reported")
    if not native_status_present:
        invalid.append("native identity or initializationError was not reported")

    failures = sum(case["status_code"] == "-2" for case in identities.values())
    skipped = sum(case["status_code"] in {"-3", "-4"} for case in identities.values())
    errors = len(invalid)
    test_count = len(identities)

    suites = ET.Element(
        "testsuite",
        {
            "name": "SereaNativeRuntimeInstrumentationDiagnostic",
            "tests": str(test_count),
            "failures": str(failures),
            "errors": str(errors),
            "skipped": str(skipped),
        },
    )
    for key, case in sorted(identities.items()):
        class_name, method = key.rsplit("#", 1)
        testcase = ET.SubElement(suites, "testcase", {"classname": class_name, "name": method})
        if case["status_code"] == "-2":
            trace = case.get("stack", case.get("Error", "AndroidJUnitRunner reported a failure."))
            ET.SubElement(
                testcase,
                "failure",
                {"message": case.get("Error", "AndroidJUnitRunner failure"), "type": "InstrumentationFailure"},
            ).text = trace
        elif case["status_code"] in {"-3", "-4"}:
            ET.SubElement(testcase, "skipped", {"message": "AndroidJUnitRunner ignored/assumption-failed test"})
    ET.ElementTree(suites).write(xml_path, encoding="utf-8", xml_declaration=True)

    code = result.get("instrumentation_code", "NOT_REPORTED")
    summary = [
        "evidence_kind=raw_android_instrumentation_status",
        f"stream_outcome={'NO_TEST_CASES' if not cases else 'TEST_STATUSES_PRESENT'}",
        f"instrumentation_code={code}",
        f"tests={test_count}",
        f"failures={failures}",
        f"errors={errors}",
        f"skipped={skipped}",
        f"noop_identity={expected_noop}",
        f"noop_result={'PASS' if noop_result and noop_result['status_code'] == '0' else 'FAIL_OR_NOT_REACHED'}",
        f"native_identity={expected_native}",
        f"native_result={'PASS' if native_result and native_result['status_code'] == '0' else 'FAIL_OR_NOT_REACHED'}",
        f"native_initialization_error={'PRESENT' if native_initialization else 'ABSENT'}",
        f"noop_initialization_error={'PRESENT' if noop_initialization else 'ABSENT'}",
        "unexpected_identity_count=" + str(len(unexpected_identities)),
    ]
    summary.extend(
        f"terminal_status={identity}:{case['status_code']}"
        for identity, case in sorted(identities.items())
    )
    if native_initialization:
        summary.append("native_initialization_status_code=" + native_initialization["status_code"])
        summary.append("native_initialization_error_text=" + native_initialization.get("Error", "NOT_REPORTED"))
        summary.append("native_initialization_stack_present=" + str(bool(native_initialization.get("stack"))).lower())
    summary.extend(f"parse_error={error}" for error in invalid)
    summary_path.write_text("\n".join(summary) + "\n", encoding="utf-8")

    # Keep a parsed but failing diagnostic artifact. The caller decides whether a test pass is
    # required; this tool only rejects malformed/incomplete instrumentation output.
    if not cases:
        print("No terminal AndroidJUnitRunner test status was emitted.", file=sys.stderr)
        return 1
    if code == "NOT_REPORTED":
        print("Android instrumentation completion code was not emitted.", file=sys.stderr)
        return 1
    if invalid:
        print("One or more terminal status records lacked a test identity.", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
