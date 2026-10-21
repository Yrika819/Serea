#!/usr/bin/env python3
"""Summarize allowlisted JUnit lifecycle markers collected from a disposable emulator."""

from __future__ import annotations

import re
import sys
from datetime import datetime, timezone
from pathlib import Path


EVENT = re.compile(
    r"^.*I/SereaTestLifecycle\([^)]*\): "
    r"(START|FINISH|FAIL|RUN_FINISH)\|([0-9]+)\|([^|]+)(?:\|([^|]+))?$"
)
EXPECTED_TESTS = 103


def utc_timestamp(epoch_ms: int) -> str:
    return datetime.fromtimestamp(epoch_ms / 1000, tz=timezone.utc).isoformat(timespec="milliseconds")


def summarize(source: Path) -> str:
    events: list[tuple[str, int, str, str | None]] = []
    for line in source.read_text(encoding="utf-8", errors="replace").splitlines():
        match = EVENT.match(line)
        if match:
            kind, timestamp, test_name, detail = match.groups()
            events.append((kind, int(timestamp), test_name, detail))

    starts = [event for event in events if event[0] == "START"]
    finishes = [event for event in events if event[0] == "FINISH"]
    failures = [event for event in events if event[0] == "FAIL"]
    run_finish = next((event for event in reversed(events) if event[0] == "RUN_FINISH"), None)
    lines = [
        f"expected_test_cases={EXPECTED_TESTS}",
        f"test_start_events={len(starts)}",
        f"test_finish_events={len(finishes)}",
        f"test_failure_events={len(failures)}",
        f"tests_not_started={max(0, EXPECTED_TESTS - len(starts))}",
        f"tests_started_without_finish={max(0, len(starts) - len(finishes))}",
    ]
    if starts:
        _, timestamp, test_name, _ = starts[-1]
        lines.extend((f"last_test_started={test_name}", f"last_test_started_at_utc={utc_timestamp(timestamp)}"))
    else:
        lines.append("last_test_started=NOT_RECORDED")
    if finishes:
        _, timestamp, test_name, _ = finishes[-1]
        lines.extend((f"last_test_finished={test_name}", f"last_test_finished_at_utc={utc_timestamp(timestamp)}"))
    else:
        lines.append("last_test_finished=NOT_RECORDED")
    if starts and (not finishes or starts[-1][1] > finishes[-1][1]):
        lines.append(f"unfinished_test={starts[-1][2]}")
    else:
        lines.append("unfinished_test=NONE_RECORDED")
    if run_finish:
        _, timestamp, totals, failure_count = run_finish
        lines.extend((f"test_run_finished_at_utc={utc_timestamp(timestamp)}", f"runner_totals={totals}", f"runner_failure_count={failure_count or 'UNKNOWN'}"))
    else:
        lines.append("test_run_finished=NOT_RECORDED")
    return "\n".join(lines) + "\n"


def main() -> int:
    if len(sys.argv) != 3:
        print("Usage: summarize_android_test_lifecycle.py LOGCAT OUTPUT", file=sys.stderr)
        return 2
    source, destination = map(Path, sys.argv[1:])
    destination.write_text(summarize(source), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
