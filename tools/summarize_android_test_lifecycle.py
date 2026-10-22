#!/usr/bin/env python3
"""Summarize test, UI-state, and Process Death evidence from a disposable emulator."""

from __future__ import annotations

import re
import sys
from datetime import datetime, timezone
from pathlib import Path


EVENT = re.compile(
    r"^.*(?:I/SereaTestLifecycle\([^)]*\):|I SereaTestLifecycle:)\s+"
    r"(START|FINISH|FAIL|RUN_FINISH)\|([0-9]+)\|(.+)$"
)
EXPECTED_TESTS = 103
CALENDAR_STAGE = re.compile(
    r"^.*(?:I/SereaCalendarStage\([^)]*\):|I SereaCalendarStage:)\s+"
    r"(START|FINISH)\|([a-z0-9-]+)\|(.+)$"
)
LOGCAT_TAG = re.compile(
    r"^(?P<clock>[0-9]{2}-[0-9]{2} [0-9:.]+).*?"
    r"(?:[VDIWE]/(?P<tag_slash>[A-Za-z0-9_.-]+)\([^)]*\):|"
    r"[VDIWE]\s+(?P<tag_space>[A-Za-z0-9_.-]+):)"
)
PROCESS_DEATH = re.compile(
    r"^(?:.*?)(?:[IWE]/SereaProcessDeath\([^)]*\):|[IWE]\s+SereaProcessDeath:)\s+"
    r"(PASS|FAIL):[^:\s]+:([^:\s]+):[^:\s]+(?::([^:\s]+))?"
)
UI_STATE_TAGS = {
    "chat-demo-label",
    "chat-current-delete-confirmation",
    "chat-current-delete-status",
    "chat-delete-all-confirmation",
    "chat-delete-all-status",
    "chat-restored-conversation",
    "chat-draft-gmail-search-timeout",
    "font-scale-gmail-heading",
    "gmail-search-result-timeout",
    "gmail-repeat-search-result-timeout",
    "task-proposal-font-scale",
}


def utc_timestamp(epoch_ms: int) -> str:
    return datetime.fromtimestamp(epoch_ms / 1000, tz=timezone.utc).isoformat(timespec="milliseconds")


def _read_fields(path: Path) -> dict[str, str]:
    fields: dict[str, str] = {}
    try:
        rows = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return fields
    for row in rows:
        key, separator, value = row.partition("=")
        if separator:
            fields[key] = value
    return fields


def _epoch_ms(value: str | None) -> int | None:
    if not value:
        return None
    try:
        return int(datetime.fromisoformat(value.replace("Z", "+00:00")).timestamp() * 1000)
    except ValueError:
        return None


def _window_for(timestamp: int, windows: list[tuple[str, int | None, int | None]]) -> str:
    for name, started, finished in windows:
        if started is not None and timestamp >= started and (finished is None or timestamp <= finished):
            return name
    return "OUTSIDE_RECORDED_TEST_WINDOW"


def summarize(source: Path, evidence_root: Path | None = None) -> str:
    events: list[tuple[str, int, list[str]]] = []
    calendar_stages: list[tuple[str, str, str]] = []
    ui_events: list[tuple[str, str, str]] = []
    process_death_events: list[tuple[str, str, str, str]] = []
    active_test: str | None = None

    for line in source.read_text(encoding="utf-8", errors="replace").splitlines():
        match = EVENT.match(line)
        if match:
            kind, timestamp_text, payload_text = match.groups()
            timestamp = int(timestamp_text)
            payload = payload_text.split("|")
            events.append((kind, timestamp, payload))
            if kind == "START":
                active_test = payload[0]
            elif kind == "FINISH":
                active_test = None

        stage_match = CALENDAR_STAGE.match(line)
        if stage_match:
            calendar_stages.append(stage_match.groups())

        tag_match = LOGCAT_TAG.match(line)
        if tag_match:
            tag = tag_match.group("tag_slash") or tag_match.group("tag_space")
            if tag in UI_STATE_TAGS:
                ui_events.append((tag_match.group("clock"), tag, active_test or "NO_ACTIVE_TEST_RECORDED"))

        death_match = PROCESS_DEATH.match(line)
        if death_match:
            outcome, phase, failure_type = death_match.groups()
            logcat_match = LOGCAT_TAG.match(line)
            clock = logcat_match.group("clock") if logcat_match else "NOT_RECORDED"
            process_death_events.append((clock, outcome, phase, failure_type or "NONE"))

    starts = [event for event in events if event[0] == "START"]
    finishes = [event for event in events if event[0] == "FINISH"]
    failures = [event for event in events if event[0] == "FAIL"]
    run_finish = next((event for event in reversed(events) if event[0] == "RUN_FINISH"), None)

    host = _read_fields(evidence_root / "test-lifecycle-host.txt") if evidence_root else {}
    windows: list[tuple[str, int | None, int | None]] = []
    full_start = _epoch_ms(host.get("instrumentation_started_at_utc"))
    full_finish = _epoch_ms(host.get("instrumentation_finished_at_utc"))
    stage_windows: list[tuple[str, int | None, int | None, dict[str, str]]] = []
    if evidence_root:
        for stage_file in sorted((evidence_root / "stages").glob("*/host-lifecycle.txt")):
            fields = _read_fields(stage_file)
            stage_name = fields.get("stage", stage_file.parent.name)
            started = _epoch_ms(fields.get("stage_started_at_utc"))
            finished = _epoch_ms(fields.get("stage_finished_at_utc"))
            stage_windows.append((stage_name, started, finished, fields))
            windows.append((f"diagnostic:{stage_name}", started, finished))
    if full_start is not None:
        windows.append(("full_instrumentation", full_start, full_finish))

    lines = [
        f"expected_test_cases={EXPECTED_TESTS}",
        f"test_start_events={len(starts)}",
        f"test_finish_events={len(finishes)}",
        f"test_failure_events={len(failures)}",
        f"tests_not_started={max(0, EXPECTED_TESTS - len(starts))}",
        f"tests_started_without_finish={max(0, len(starts) - len(finishes))}",
        f"test_lifecycle_log={source.name}",
        "junit_results=junit/",
        "full_gradle_log=instrumentation-gradle.log",
        "process_death_log=process-death.log",
    ]
    if host:
        for key in ("api_level", "campaign_token", "campaign_started_at_utc", "instrumentation_started_at_utc", "instrumentation_finished_at_utc"):
            if key in host:
                field_name = key if key.startswith("campaign_") else f"campaign_{key}"
                lines.append(f"{field_name}={host[key]}")
    if starts:
        _, timestamp, payload = starts[-1]
        test_name = payload[0]
        lines.extend((f"last_test_started={test_name}", f"last_test_started_at_utc={utc_timestamp(timestamp)}"))
    else:
        lines.append("last_test_started=NOT_RECORDED")
    if finishes:
        _, timestamp, payload = finishes[-1]
        test_name = payload[0]
        lines.extend((f"last_test_finished={test_name}", f"last_test_finished_at_utc={utc_timestamp(timestamp)}"))
    else:
        lines.append("last_test_finished=NOT_RECORDED")
    if starts and (not finishes or starts[-1][1] > finishes[-1][1]):
        lines.append(f"unfinished_test={starts[-1][2][0]}")
    else:
        lines.append("unfinished_test=NONE_RECORDED")
    if run_finish:
        _, timestamp, result = run_finish
        lines.append(f"test_run_finished_at_utc={utc_timestamp(timestamp)}")
        if len(result) >= 3:
            lines.extend((
                f"runner_test_count={result[0]}",
                f"runner_failure_count={result[1]}",
                f"runner_skipped_count={result[2]}",
            ))
        else:
            lines.append("runner_result_fields=INCOMPLETE")
    else:
        lines.append("test_run_finished=NOT_RECORDED")

    lines.append(f"test_event_count={len(events)}")
    for index, (kind, timestamp, payload) in enumerate(events, 1):
        lines.append(f"test_event.{index}.kind={kind}")
        lines.append(f"test_event.{index}.at_utc={utc_timestamp(timestamp)}")
        if payload:
            lines.append(f"test_event.{index}.test={payload[0]}")
            lines.append(f"test_event.{index}.window={_window_for(timestamp, windows)}")
        if kind == "FAIL" and len(payload) > 1:
            lines.append(f"test_event.{index}.failure_type={payload[1]}")

    lines.append(f"test_failure_count={len(failures)}")
    for index, (_, timestamp, payload) in enumerate(failures, 1):
        lines.extend((
            f"test_failure.{index}={payload[0]}",
            f"test_failure.{index}.at_utc={utc_timestamp(timestamp)}",
            f"test_failure.{index}.window={_window_for(timestamp, windows)}",
            f"test_failure.{index}.junit_directory=junit/",
        ))
        if len(payload) > 1:
            lines.append(f"test_failure.{index}.type={payload[1]}")

    lines.append(f"ui_state_event_count={len(ui_events)}")
    for index, (clock, tag, test_name) in enumerate(ui_events, 1):
        lines.extend((
            f"ui_state_event.{index}.logcat_time={clock}",
            f"ui_state_event.{index}.tag={tag}",
            f"ui_state_event.{index}.test={test_name}",
            f"ui_state_event.{index}.source=logcat-live.txt",
        ))

    lines.append(f"process_death_event_count={len(process_death_events)}")
    for index, (clock, outcome, phase, failure_type) in enumerate(process_death_events, 1):
        lines.extend((
            f"process_death_event.{index}.logcat_time={clock}",
            f"process_death_event.{index}.outcome={outcome}",
            f"process_death_event.{index}.phase={phase}",
            f"process_death_event.{index}.failure_type={failure_type}",
            f"process_death_event.{index}.source=process-death.log and logcat-live.txt",
        ))

    if evidence_root:
        for source_file, field in (
            ("instrumentation-gradle-exit-code.txt", "instrumentation_gradle_exit_code"),
            ("instrumentation-verification-exit-code.txt", "instrumentation_verification_exit_code"),
            ("instrumentation-status.txt", "instrumentation_status"),
            ("diagnostic-status.txt", "diagnostic_status"),
            ("process-death-status.txt", "process_death_status"),
            ("process-death-host-lifecycle.txt", "process_death_host_lifecycle"),
        ):
            path = evidence_root / source_file
            value = path.read_text(encoding="utf-8", errors="replace").strip() if path.is_file() else "NOT_RECORDED"
            lines.append(f"{field}={value.replace(chr(10), ',')}")
        lines.append(f"diagnostic_stage_count={len(stage_windows)}")
        for index, (name, _, _, fields) in enumerate(stage_windows, 1):
            stage_dir = evidence_root / "stages" / name
            lines.extend((
                f"diagnostic_stage.{index}.name={name}",
                f"diagnostic_stage.{index}.test_filter={fields.get('test_filter', 'NOT_RECORDED')}",
                f"diagnostic_stage.{index}.started_at_utc={fields.get('stage_started_at_utc', 'NOT_RECORDED')}",
                f"diagnostic_stage.{index}.finished_at_utc={fields.get('stage_finished_at_utc', 'NOT_RECORDED')}",
                f"diagnostic_stage.{index}.gradle_exit_code={_read_scalar(stage_dir / 'gradle-exit-code.txt')}",
                f"diagnostic_stage.{index}.verification_exit_code={_read_scalar(stage_dir / 'verification-exit-code.txt')}",
                f"diagnostic_stage.{index}.status={_read_scalar(stage_dir / 'status.txt')}",
                f"diagnostic_stage.{index}.gradle_log=stages/{name}/gradle.log",
                f"diagnostic_stage.{index}.junit_directory=stages/{name}/junit/",
            ))

    stage_starts = [stage for stage in calendar_stages if stage[0] == "START"]
    stage_finishes = [stage for stage in calendar_stages if stage[0] == "FINISH"]
    lines.extend((
        f"calendar_stage_start_events={len(stage_starts)}",
        f"calendar_stage_finish_events={len(stage_finishes)}",
        f"calendar_last_stage_started={stage_starts[-1][1] if stage_starts else 'NOT_RECORDED'}",
        f"calendar_last_stage_finished={stage_finishes[-1][1] if stage_finishes else 'NOT_RECORDED'}",
    ))
    if stage_starts and (not stage_finishes or stage_starts[-1][1] != stage_finishes[-1][1]):
        lines.append(f"calendar_stage_incomplete={stage_starts[-1][1]}")
    else:
        lines.append("calendar_stage_incomplete=NONE_RECORDED")
    return "\n".join(lines) + "\n"


def _read_scalar(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8", errors="replace").strip() or "NOT_RECORDED"
    except OSError:
        return "NOT_RECORDED"


def main() -> int:
    if len(sys.argv) not in (3, 4):
        print("Usage: summarize_android_test_lifecycle.py LOGCAT OUTPUT [EVIDENCE_ROOT]", file=sys.stderr)
        return 2
    source = Path(sys.argv[1])
    destination = Path(sys.argv[2])
    evidence_root = Path(sys.argv[3]) if len(sys.argv) == 4 else None
    destination.write_text(summarize(source, evidence_root), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
