#!/usr/bin/env python3
"""Validate mandatory Personal Beta disposable identity isolation evidence."""
from __future__ import annotations
import argparse
import sys
from pathlib import Path

EXPECTED = {
    "demo_and_beta_parallel_install": "PASS",
    "demo_package_data_and_signer_unchanged": "PASS",
    "beta_encrypted_room_data_after_emulator_reboot": "PASS",
    "same_test_key_version_100_to_101_update": "PASS",
    "different_test_key_version_102_update": "INSTALL_FAILED_UPDATE_INCOMPATIBLE",
    "owner_signing_key_used": "no",
    "demo_uninstall_attempted": "no",
}


def verify(path: Path) -> None:
    if not path.is_file():
        raise ValueError(f"Identity evidence is missing: {path}")
    fields: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if "=" in line:
            key, value = line.split("=", 1)
            if key in EXPECTED:
                if key in fields:
                    raise ValueError(f"duplicate evidence field: {key}")
                fields[key] = value
    missing = sorted(set(EXPECTED) - set(fields))
    if missing:
        raise ValueError(f"missing Identity evidence fields: {', '.join(missing)}")
    invalid = [f"{key}={fields[key]} (expected {value})" for key, value in EXPECTED.items() if fields[key] != value]
    if invalid:
        raise ValueError("Identity evidence failed: " + "; ".join(invalid))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("evidence", type=Path)
    args = parser.parse_args()
    try:
        verify(args.evidence)
    except (OSError, ValueError) as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 1
    print("PASS: Personal Beta Identity Isolation evidence validated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
