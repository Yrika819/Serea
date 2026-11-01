#!/usr/bin/env python3
"""Fail closed if the disposable Real IME diagnostic privilege leaks out of the isolated target."""

from __future__ import annotations

import argparse
import sys
import xml.etree.ElementTree as ET
from pathlib import Path


ANDROID = "{http://schemas.android.com/apk/res/android}"
DIAGNOSTICS_PERMISSION = "app.serea.android.r25testime.permission.DIAGNOSTICS"
IME_PACKAGE = "app.serea.android.r25testime"
IME_SERVICE = "app.serea.android.diagnostics.RealImeService"
IME_PROVIDER = "app.serea.android.diagnostics.RealImeDiagnosticsProvider"


def read_manifest(path: Path) -> ET.Element:
    if not path.is_file():
        raise ValueError(f"Merged manifest is missing: {path}")
    try:
        return ET.parse(path).getroot()
    except ET.ParseError as error:
        raise ValueError(f"Invalid merged manifest {path}: {error}") from error


def permissions(root: ET.Element) -> set[str]:
    return {node.get(ANDROID + "name", "") for node in root.findall("uses-permission")}


def components(root: ET.Element, tag: str) -> list[ET.Element]:
    application = root.find("application")
    if application is None:
        return []
    return application.findall(tag)


def check_target(name: str, root: ET.Element, expected_package: str, allow_diagnostic: bool) -> None:
    actual_package = root.get("package", "")
    if actual_package != expected_package:
        raise ValueError(f"{name} package is {actual_package!r}; expected {expected_package!r}")

    requested = DIAGNOSTICS_PERMISSION in permissions(root)
    if requested != allow_diagnostic:
        expectation = "must request" if allow_diagnostic else "must not request"
        raise ValueError(f"{name} {expectation} {DIAGNOSTICS_PERMISSION}")

    leaked_components = [
        (tag, node.get(ANDROID + "name", ""))
        for tag in ("service", "provider")
        for node in components(root, tag)
        if node.get(ANDROID + "name", "").endswith(("RealImeService", "RealImeDiagnosticsProvider"))
    ]
    if leaked_components:
        raise ValueError(f"Test IME components leaked into {name}: {leaked_components}")


def check_ime(root: ET.Element) -> None:
    actual_package = root.get("package", "")
    if actual_package != IME_PACKAGE:
        raise ValueError(f"Test IME APK package is {actual_package!r}; expected {IME_PACKAGE!r}")

    declared = [
        node for node in root.findall("permission")
        if node.get(ANDROID + "name") == DIAGNOSTICS_PERMISSION
    ]
    if len(declared) != 1 or declared[0].get(ANDROID + "protectionLevel") != "signature":
        raise ValueError("Test IME must declare exactly one signature-protected diagnostics permission")

    service = [node for node in components(root, "service") if node.get(ANDROID + "name") == IME_SERVICE]
    if len(service) != 1 or service[0].get(ANDROID + "exported") != "true":
        raise ValueError("Test IME service must be present and explicitly exported for Android IME binding")
    if service[0].get(ANDROID + "permission") != "android.permission.BIND_INPUT_METHOD":
        raise ValueError("Test IME service must require android.permission.BIND_INPUT_METHOD")

    provider = [node for node in components(root, "provider") if node.get(ANDROID + "name") == IME_PROVIDER]
    if len(provider) != 1 or provider[0].get(ANDROID + "exported") != "true":
        raise ValueError("Test IME diagnostics provider must be present and explicitly exported")
    if provider[0].get(ANDROID + "permission") != DIAGNOSTICS_PERMISSION:
        raise ValueError("Test IME diagnostics provider must require the signature permission")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--debug", required=True, type=Path)
    parser.add_argument("--isolated", required=True, type=Path)
    parser.add_argument("--beta", required=True, type=Path)
    parser.add_argument("--ime", required=True, type=Path)
    arguments = parser.parse_args()

    try:
        check_target("debug target", read_manifest(arguments.debug), "app.serea.android.demo", False)
        check_target("isolated target", read_manifest(arguments.isolated), "app.serea.android.isolated", True)
        check_target("beta target", read_manifest(arguments.beta), "app.serea.android.beta", False)
        check_ime(read_manifest(arguments.ime))
    except ValueError as error:
        print(f"MANIFEST_SCOPE_FAIL: {error}", file=sys.stderr)
        return 1

    print("MANIFEST_SCOPE_PASS: diagnostics permission is confined to isolated target and signature-gated Test IME")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
