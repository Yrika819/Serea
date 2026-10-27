#!/usr/bin/env python3
"""Host regression coverage for mutually exclusive G3 push API markers."""

from pathlib import Path
import sys


SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))
from api_selection import select_api  # noqa: E402


def must_reject(message: str) -> None:
    try:
        select_api("push", message)
    except ValueError:
        return
    raise SystemExit(f"expected API marker input to be rejected: {message!r}")


assert select_api("push", "G3 run [g3-poc-run] [g3-api=26]") == "26"
assert select_api("push", "G3 run [g3-poc-run] [g3-api=35]") == "35"
must_reject("G3 run [g3-poc-run]")
must_reject("G3 run [g3-poc-run] [g3-api=26] [g3-api=35]")
must_reject("G3 run [g3-poc-run] [g3-api=26] [g3-api=26]")
must_reject("G3 run [g3-api=26]")

assert select_api("workflow_dispatch", dispatch_input="26") == "26"
assert select_api("workflow_dispatch", dispatch_input="35") == "35"
for invalid_dispatch in (None, "", "34"):
    try:
        select_api("workflow_dispatch", dispatch_input=invalid_dispatch)
    except ValueError:
        continue
    raise SystemExit(f"expected unsupported dispatch API to be rejected: {invalid_dispatch!r}")

print("g3_api_selection_host_regression=PASS valid_push=2 both_markers_rejected=true duplicate_marker_rejected=true dispatch=2")
