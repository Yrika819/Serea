#!/usr/bin/env python3
"""Fail-closed source boundary checks for the explicitly synthetic Host.

Cargo dependency-tree checks remain in the Rust security guard. These static
checks are deliberately narrow and are paired with behavioral tests.
"""
from __future__ import annotations

import argparse
from pathlib import Path
import sys


def check(root: Path) -> list[str]:
    errors: list[str] = []

    def read(path: str) -> str:
        try:
            return (root / path).read_text(encoding="utf-8")
        except OSError as exc:
            errors.append(f"cannot read {path}: {exc}")
            return ""

    core_manifest = read("crates/serea-core/Cargo.toml")
    host_manifest = read("crates/serea-host/Cargo.toml")
    host = read("crates/serea-host/src/main.rs")
    model = read("crates/serea-host/src/synthetic_model.rs")

    if "serea-model-router" in core_manifest:
        errors.append("Core manifest must not depend on Model Router")
    if "default = []" not in host_manifest:
        errors.append("Host default features must remain empty")
    if 'synthetic = [' not in host_manifest or '"dep:serea-model-router"' not in host_manifest:
        errors.append("Model Router must be activated only by the synthetic feature")
    if 'synthetic-test-hooks = ["synthetic", "serea-scheduler/test-hooks"]' not in host_manifest:
        errors.append("process crash hooks must remain a separate opt-in feature")
    if "serea-model-router = { workspace = true, optional = true }" not in host_manifest:
        errors.append("Host Model Router dependency must be optional")

    if "ModelRouter" in host or "ModelProvider::generate" in host:
        errors.append("Host entrypoint must not directly construct or invoke Model Router")
    run = host.split("fn run_synthetic_host()", 1)
    run = run[1].split("#[cfg(feature = \"synthetic\")]\nstruct HostLock", 1)[0] if len(run) == 2 else ""
    if "synthetic_model::recover_unresolved" not in run:
        errors.append("synthetic startup must reconcile unresolved model intents")
    if "synthetic_model::execute_once" in run:
        errors.append("normal Host lifecycle must not dispatch models")

    if "PreparedModelCallV1::from_host" not in model or "router.execute(&call, &host)" not in model:
        errors.append("synthetic dispatch must use trusted Host preparation and existing Router")
    if "MockModelProvider" not in model:
        errors.append("synthetic provider must be an explicit in-process mock")
    for forbidden in ("reqwest", "std::net::", "CapabilityProvider::invoke", "OPENAI_API_KEY", "CredentialHandle"):
        if forbidden in model:
            errors.append(f"synthetic adapter contains forbidden path: {forbidden}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--static-only", action="store_true", help="skip Cargo graph checks")
    args = parser.parse_args()
    errors = check(args.root.resolve())
    if errors:
        print("\n".join(f"FAIL: {error}" for error in errors), file=sys.stderr)
        return 1
    print("synthetic Host static boundary: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
