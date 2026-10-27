#!/usr/bin/env python3
"""Validate that a G3 diagnostic event selects one API level unambiguously."""

import json
import os
from pathlib import Path
import re
import sys

API_MARKER_PATTERN = re.compile(r"\[g3-api=[^\]\r\n]*(?:\]|(?=$))")
SUPPORTED_API_MARKERS = {"[g3-api=26]": "26", "[g3-api=35]": "35"}


def select_api(event_name: str, message: str = "", dispatch_input: str | None = None) -> str:
    if event_name == "workflow_dispatch":
        if dispatch_input not in {"26", "35"}:
            raise ValueError("workflow_dispatch must select API 26 or 35")
        return dispatch_input

    if event_name != "push":
        raise ValueError(f"unsupported event for G3 API selection: {event_name}")
    if "[g3-poc-run]" not in message:
        raise ValueError("diagnostic push must include [g3-poc-run]")

    api_markers = API_MARKER_PATTERN.findall(message)
    unsupported = [marker for marker in api_markers if marker not in SUPPORTED_API_MARKERS]
    if unsupported:
        raise ValueError(f"unsupported or malformed G3 API marker(s): {unsupported!r}")
    if len(api_markers) != 1:
        raise ValueError("diagnostic push must contain exactly one API marker: [g3-api=26] or [g3-api=35]")
    return SUPPORTED_API_MARKERS[api_markers[0]]


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: api_selection.py <github-event.json> <workflow-selected-api>", file=sys.stderr)
        return 2
    event_path = Path(sys.argv[1])
    event = json.loads(event_path.read_text(encoding="utf-8"))
    event_name = os.environ.get("GITHUB_EVENT_NAME", "")
    if event_name == "workflow_dispatch":
        message = ""
        dispatch_input = event.get("inputs", {}).get("api_level")
    elif event_name == "push":
        message = event.get("head_commit", {}).get("message", "")
        dispatch_input = None
    else:
        message = ""
        dispatch_input = None
    selected = select_api(event_name, message, dispatch_input)
    if sys.argv[2] != selected:
        raise ValueError(f"workflow selected API {sys.argv[2]!r}, but event selects API {selected}")
    print(f"api_marker_validation=PASS selected_api={selected}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, json.JSONDecodeError, ValueError) as failure:
        print(f"api_marker_validation=FAIL reason={failure}", file=sys.stderr)
        raise SystemExit(1)
