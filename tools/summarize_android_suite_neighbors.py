#!/usr/bin/env python3
"""Summarize test order, adjacent tests, and aggregate JUnit counts."""
from __future__ import annotations

import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

EVENT = re.compile(r"^(START|FINISH|FAIL)\|(\d+)\|([^|]+)(?:\|([^|]+))?$")
TARGETS = {
    "app.serea.android.ChatScreenTest": {
        "demoLabelAndConversationTitleAreVisible",
        "completedDemoConversationSurvivesActivityRecreation",
        "currentConversationDeletionRequiresConfirmationAndCancelKeepsMessages",
        "confirmedCurrentConversationDeletionRemovesSelectedConversation",
        "deleteAllConfirmationCancelKeepsConversationAndConfirmClearsHistory",
    },
    "app.serea.android.SereaAppShellTest": {
        "chatDraftSurvivesGmailAndCalendarDestinationChanges",
    },
}


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: summarize_android_suite_neighbors.py LOGCAT JUNIT_DIR", file=sys.stderr)
        return 2
    logcat = Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace").splitlines()
    events: list[tuple[str, int, str, str]] = []
    for line in logcat:
        match = EVENT.match(line.strip())
        if match:
            event, millis, test_name, detail = match.groups()
            events.append((event, int(millis), test_name, detail or ""))

    starts = [(i, event[2]) for i, event in enumerate(events) if event[0] == "START"]
    output: list[str] = []
    output.append(f"lifecycle_events={len(events)}")
    output.append(f"tests_started={len(starts)}")
    output.append("test_start_order:")
    output.extend(f"  {index:03d} {name}" for index, (_, name) in enumerate(starts, 1))
    output.append("failure_neighbors:")
    failures = [event for event in events if event[0] == "FAIL"]
    if not failures:
        output.append("  none in lifecycle log")
    for failure in failures:
        test_name = failure[2]
        if "#" not in test_name:
            continue
        class_name, method = test_name.split("#", 1)
        if method not in TARGETS.get(class_name, set()):
            continue
        start_index = next((i for i, e in enumerate(events) if e[0] == "START" and e[2] == test_name), None)
        if start_index is None:
            output.append(f"  FAIL {test_name}; matching START missing")
            continue
        finish_index = next((i for i in range(start_index + 1, len(events)) if events[i][0] == "FINISH" and events[i][2] == test_name), start_index)
        previous = next((events[i][2] for i in range(start_index - 1, -1, -1) if events[i][0] == "START"), "<suite-start>")
        following = next((events[i][2] for i in range(finish_index + 1, len(events)) if events[i][0] == "START"), "<suite-end>")
        output.append(
            f"  FAIL {test_name} failure_type={failure[3]} at_ms={failure[1]}\n"
            f"    previous_started_test={previous}\n"
            f"    next_started_test={following}"
        )

    junit_root = Path(sys.argv[2])
    totals = {"tests": 0, "failures": 0, "errors": 0, "skipped": 0, "files": 0}
    for path in sorted(junit_root.rglob("*.xml")) if junit_root.exists() else []:
        try:
            root = ET.parse(path).getroot()
        except ET.ParseError:
            continue
        suites = [root] if root.tag == "testsuite" else list(root.iter("testsuite"))
        for suite in suites:
            totals["tests"] += int(suite.attrib.get("tests", 0))
            totals["failures"] += int(suite.attrib.get("failures", 0))
            totals["errors"] += int(suite.attrib.get("errors", 0))
            totals["skipped"] += int(suite.attrib.get("skipped", 0))
            totals["files"] += 1
    output.append(
        "junit_totals=" + " ".join(f"{key}={value}" for key, value in totals.items())
    )
    Path(sys.argv[1] + ".neighbors.txt").write_text("\n".join(output) + "\n", encoding="utf-8")
    print("\n".join(output))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
