#!/usr/bin/env python3
"""Extract focus-owner evidence from raw Android dumpsys lines captured in logcat."""

from __future__ import annotations

import re
import sys
from pathlib import Path


WINDOW_KEYS = re.compile(
    r"mCurrentFocus|mFocusedApp|mFocusedWindow|mInputMethodTarget|"
    r"mImeInputTarget|mImeLayeringTarget|mInputMethodControlTarget|"
    r"mObscuringWindow|mDisplayImePolicy"
)
IME_KEYS = re.compile(
    r"mServedView|mCurRootView|mServedInputConnection|mShowRequested|"
    r"mInputShown|mInputMethodShown|mIsInputViewShown|mShowExplicitlyRequested|"
    r"mImeWindowVis|mFullscreenMode|mBindState"
)
ACTIVITY_KEYS = re.compile(r"mResumedActivity|topResumedActivity|ResumedActivity")


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: summarize_android_root_cause.py LOGCAT_RAW OUTPUT", file=sys.stderr)
        return 2
    source = Path(sys.argv[1])
    destination = Path(sys.argv[2])
    destination.parent.mkdir(parents=True, exist_ok=True)
    active: tuple[str, str] | None = None
    evidence: list[str] = []
    for raw_line in source.read_text(encoding="utf-8", errors="replace").splitlines():
        if raw_line.startswith("BEGIN|"):
            fields = raw_line.split("|", 2)
            stage = fields[1].removeprefix("stage=") if len(fields) > 1 else "unknown"
            service = fields[2].removeprefix("service=") if len(fields) > 2 else "unknown"
            active = (stage, service)
            evidence.append(f"BEGIN stage={stage} service={service}")
            continue
        if raw_line.startswith("END|"):
            fields = raw_line.split("|", 2)
            stage = fields[1].removeprefix("stage=") if len(fields) > 1 else "unknown"
            service = fields[2].removeprefix("service=") if len(fields) > 2 else "unknown"
            evidence.append(f"END stage={stage} service={service}")
            active = None
            continue
        if not raw_line.startswith("RAW|"):
            continue
        fields = raw_line.split("|", 3)
        if len(fields) < 4:
            continue
        stage = fields[1].removeprefix("stage=")
        service = fields[2].removeprefix("service=")
        body = fields[3]
        keys = WINDOW_KEYS if service == "window" else IME_KEYS if service == "input_method" else ACTIVITY_KEYS
        if keys.search(body):
            evidence.append(f"EXTRACT stage={stage} service={service} {body}")
    destination.write_text("\n".join(evidence) + ("\n" if evidence else ""), encoding="utf-8")
    print(f"host_extracted_focus_lines={len(evidence)} output={destination}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
