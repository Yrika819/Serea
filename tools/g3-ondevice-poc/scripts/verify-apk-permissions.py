#!/usr/bin/env python3
"""Fail closed unless generated APK permission requests match the PoC allowlists."""

from __future__ import annotations

import re
import sys
from pathlib import Path


APP_PACKAGE = "app.serea.g3poc"
TEST_PACKAGE = "app.serea.g3poc.test"
APP_ALLOWED = frozenset()
TEST_ALLOWED = frozenset({"android.permission.REORDER_TASKS"})
PACKAGE_LINE = re.compile(r"^package:\s*(\S+)\s*$", re.MULTILINE)
REQUEST_LINE = re.compile(
    r"^uses-permission(?:-sdk-23)?:\s*name=['\"]([^'\"]+)['\"](?:\s+.*)?$",
    re.MULTILINE,
)
REQUEST_PREFIX = re.compile(r"^uses-permission(?:-sdk-23)?:", re.MULTILINE)


def parse_dump(text: str, expected_package: str) -> frozenset[str]:
    packages = PACKAGE_LINE.findall(text)
    if packages != [expected_package]:
        raise ValueError(f"expected exactly package {expected_package}, got {packages}")
    request_lines = REQUEST_PREFIX.findall(text)
    parsed = REQUEST_LINE.findall(text)
    if len(request_lines) != len(parsed):
        raise ValueError(f"could not parse every permission request in {expected_package}")
    return frozenset(parsed)


def require_allowlist(actual: frozenset[str], expected: frozenset[str], label: str) -> None:
    if actual != expected:
        added = sorted(actual - expected)
        missing = sorted(expected - actual)
        raise ValueError(f"{label} permission allowlist mismatch: added={added} missing={missing}")


def main() -> int:
    if len(sys.argv) != 3:
        raise SystemExit("usage: verify-apk-permissions.py APP_AAPT_DUMP ANDROIDTEST_AAPT_DUMP")
    app_dump = Path(sys.argv[1]).read_text(encoding="utf-8")
    test_dump = Path(sys.argv[2]).read_text(encoding="utf-8")
    app_permissions = parse_dump(app_dump, APP_PACKAGE)
    test_permissions = parse_dump(test_dump, TEST_PACKAGE)
    require_allowlist(app_permissions, APP_ALLOWED, "target APK")
    require_allowlist(test_permissions, TEST_ALLOWED, "AndroidTest APK")
    print(
        "apk_permission_allowlists=PASS target=[] "
        "androidtest=['android.permission.REORDER_TASKS']"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
