#!/usr/bin/env python3
"""Conservatively classify changed paths that require Android validation."""

from __future__ import annotations

import sys


DOC_ONLY_PREFIXES = ("docs/",)
ANDROID_AREAS = (
    "android/",
    ".github/",
    "tools/",
    "tests/",
    "scripts/",
    "gradle/",
)
ANDROID_DOCS = {"android/README.md"}
ROOT_GRADLE_FILES = {
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "settings.gradle.kts",
    "gradle.properties",
    "gradle.lockfile",
}


def classify(paths: list[str]) -> tuple[bool, bool]:
    normalized: list[str] = []
    for path in paths:
        value = path.strip()
        if not value:
            continue
        if value.startswith("/") or "\\" in value or any(part == ".." for part in value.split("/")):
            raise ValueError(f"invalid changed path: {path!r}")
        normalized.append(value)

    docs_changed = any(path.startswith(DOC_ONLY_PREFIXES) or path in ANDROID_DOCS for path in normalized)
    android_changed = False
    for path in normalized:
        if path.startswith(DOC_ONLY_PREFIXES) or path in ANDROID_DOCS:
            continue
        if path.startswith(ANDROID_AREAS) or path in ROOT_GRADLE_FILES:
            android_changed = True
            break
        if path.endswith((".gradle", ".gradle.kts")):
            android_changed = True
            break
    return android_changed, docs_changed


def main() -> int:
    try:
        raw = sys.stdin.buffer.read()
        if raw and not raw.endswith(b"\0"):
            raise ValueError("changed-path stream is not NUL terminated")
        paths = [item.decode("utf-8", errors="strict") for item in raw.split(b"\0")[:-1]]
        android_changed, docs_changed = classify(paths)
    except (OSError, UnicodeError, ValueError) as exc:
        print(f"Android change classification failed closed: {exc}", file=sys.stderr)
        return 2
    print(f"android={'true' if android_changed else 'false'}")
    print(f"docs={'true' if docs_changed else 'false'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
