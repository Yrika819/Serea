#!/usr/bin/env python3
"""Convert adb am-instrument output into a small JUnit artifact, including runner crashes."""
from __future__ import annotations

import sys
import xml.etree.ElementTree as ET
from pathlib import Path


def main() -> int:
    if len(sys.argv) != 4:
        raise SystemExit("usage: instrumentation_to_junit.py INPUT OUTPUT SUITE")
    source, destination, suite_name = map(Path, sys.argv[1:])
    raw = source.read_text(errors="replace") if source.exists() else ""
    outcomes: dict[str, bool] = {}
    current: str | None = None
    for line in raw.splitlines():
        if line.startswith("INSTRUMENTATION_STATUS: test="):
            current = line.partition("=")[2]
        elif line.startswith("INSTRUMENTATION_STATUS_CODE:") and current:
            code = line.partition(":")[2].strip()
            if code == "0":
                outcomes[current] = True
            elif code == "-2":
                outcomes[current] = False
    tests_run = 0
    for line in raw.splitlines():
        if line.startswith("Tests run:"):
            try:
                tests_run = int(line.partition(":")[2].split(",", 1)[0].strip())
            except ValueError:
                pass
    method_names = list(outcomes)
    failures = "FAILURES!!!" in raw
    crashed = "Process crashed" in raw or "INSTRUMENTATION_CODE: 0" in raw and tests_run == 0
    suite = ET.Element("testsuite", name=str(suite_name), tests=str(max(tests_run, len(method_names))), failures="0", errors="0", skipped="0")
    if not method_names and crashed:
        case = ET.SubElement(suite, "testcase", classname=str(suite_name), name="instrumentation_runner")
        ET.SubElement(case, "error", message="Instrumentation process crashed before running test methods").text = raw[-4000:]
        suite.set("tests", "1")
        suite.set("errors", "1")
    elif method_names:
        for name in method_names:
            case = ET.SubElement(suite, "testcase", classname=str(suite_name), name=name)
            if not outcomes[name] or failures:
                ET.SubElement(case, "failure", message="Instrumentation run reported a failure").text = raw[-4000:]
                suite.set("failures", str(int(suite.get("failures", "0")) + 1))
    else:
        case = ET.SubElement(suite, "testcase", classname=str(suite_name), name="instrumentation_runner")
        ET.SubElement(case, "skipped", message="No instrumentation method result was emitted")
        suite.set("tests", "1")
        suite.set("skipped", "1")
    ET.ElementTree(suite).write(destination, encoding="utf-8", xml_declaration=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
