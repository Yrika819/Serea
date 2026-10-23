#!/usr/bin/env python3
"""Extract actual focus ownership from raw dumpsys files exported by the host."""
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
        print("usage: summarize_android_root_cause_files.py APP_FILES_DIR OUTPUT", file=sys.stderr)
        return 2
    source = Path(sys.argv[1])
    destination = Path(sys.argv[2])
    destination.parent.mkdir(parents=True, exist_ok=True)
    evidence: list[str] = []
    for path in sorted(source.rglob("*-dumpsys-*.txt")) if source.exists() else []:
        name = path.name
        if "dumpsys-window" in name:
            keys = WINDOW_KEYS
            service = "window"
        elif "dumpsys-input_method" in name:
            keys = IME_KEYS
            service = "input_method"
        elif "dumpsys-activity-activities" in name:
            keys = ACTIVITY_KEYS
            service = "activity activities"
        else:
            continue
        stage = name.removesuffix(f"-dumpsys-{service.replace(' ', '-')}.txt")
        evidence.append(f"BEGIN stage={stage} service={service} source={path.relative_to(source)}")
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            if keys.search(line):
                evidence.append(f"EXTRACT stage={stage} service={service} {line}")
        evidence.append(f"END stage={stage} service={service}")
    destination.write_text("\n".join(evidence) + ("\n" if evidence else ""), encoding="utf-8")
    print(f"host_extracted_focus_lines={len(evidence)} output={destination}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
