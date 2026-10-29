#!/usr/bin/env python3
"""Emit only the bounded integer diagnostics from the Native instrumentation log."""

from __future__ import annotations

import re
import sys

MAX_INPUT_BYTES = 262_144
MAX_METRIC_VALUE = 1_000_000_000
EXPECTED = {
    "pss_measurement_status",
    "load_before_kb",
    "load_after_kb",
    "inference_1_before_kb",
    "inference_1_after_kb",
    "inference_2_before_kb",
    "inference_2_after_kb",
    "main_release_pss_kb",
    "cancelled_release_pss_kb",
    "cancel_to_release_ms",
}
PSS_METRICS = EXPECTED - {"pss_measurement_status", "cancel_to_release_ms"}
REQUIRED_PSS_FOR_PASS = {
    "load_before_kb",
    "load_after_kb",
    "inference_1_before_kb",
    "inference_1_after_kb",
    "inference_2_before_kb",
    "inference_2_after_kb",
    "main_release_pss_kb",
    "cancelled_release_pss_kb",
}


def main() -> int:
    raw = sys.stdin.buffer.read(MAX_INPUT_BYTES + 1)
    if len(raw) > MAX_INPUT_BYTES:
        print("Native metric input exceeded the bounded diagnostic window.", file=sys.stderr)
        return 2
    text = raw.decode("utf-8", errors="replace")
    summary_lines = [line.partition("metric_summary=")[2] for line in text.splitlines() if "metric_summary=" in line]
    if len(summary_lines) != 1:
        print("Expected exactly one Native metric summary.", file=sys.stderr)
        return 1

    metrics: dict[str, int | str] = {}
    for entry in summary_lines[0].split(","):
        key, separator, value = entry.partition("=")
        if not separator or key not in EXPECTED:
            print("Native metric summary was malformed.", file=sys.stderr)
            return 1
        if key == "pss_measurement_status":
            if value not in {"PASS", "NOT_AVAILABLE"}:
                print("Native PSS measurement status was invalid.", file=sys.stderr)
                return 1
            numeric: int | str = value
        elif value == "NOT_AVAILABLE" and key in PSS_METRICS:
            numeric = value
        elif re.fullmatch(r"[0-9]{1,10}", value):
            numeric = int(value)
            if numeric > MAX_METRIC_VALUE:
                print("Native metric value exceeded its bound.", file=sys.stderr)
                return 1
        else:
            print("Native metric summary was malformed.", file=sys.stderr)
            return 1
        if key in metrics:
            print("Native metric summary was out of bounds or duplicated.", file=sys.stderr)
            return 1
        metrics[key] = numeric

    missing = sorted(EXPECTED - metrics.keys())
    if missing:
        print("Native metric summary omitted required measurements.", file=sys.stderr)
        return 1
    observed_pss = {key for key in PSS_METRICS if isinstance(metrics[key], int)}
    expected_pss_status = "PASS" if REQUIRED_PSS_FOR_PASS <= observed_pss else "NOT_AVAILABLE"
    if metrics["pss_measurement_status"] != expected_pss_status:
        print("Native PSS measurement status did not match available samples.", file=sys.stderr)
        return 1
    if not isinstance(metrics["cancel_to_release_ms"], int):
        print("Native cancellation latency was not reported as an integer.", file=sys.stderr)
        return 1

    print("evidence_kind=native-runtime-observed-metrics")
    print("pss_semantics=observed_process_metric_not_memory_cap")
    for key in sorted(metrics):
        print(f"{key}={metrics[key]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
