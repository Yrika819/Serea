#!/usr/bin/env python3
"""Compile an external consumer and require the removed fixture API diagnostic."""

from __future__ import annotations

import os
from pathlib import Path
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parent.parent
EXPECTED = "no method named `install_synthetic_registry_generation`"


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="serea-core-api-boundary-") as directory:
        consumer = Path(directory)
        (consumer / "src").mkdir()
        manifest = """[package]
name = "serea-core-api-boundary-probe"
version = "0.1.0"
edition = "2021"

[dependencies]
"""
        manifest += f'serea-core = {{ path = "{ROOT / "crates/serea-core"}" }}\n'
        manifest += f'serea-protocol = {{ path = "{ROOT / "crates/serea-protocol"}" }}\n'
        (consumer / "Cargo.toml").write_text(manifest)
        (consumer / "src/main.rs").write_text(
            """use serea_core::Core;
use serea_protocol::EpochMillis;

fn probe(core: &Core) {
    core.install_synthetic_registry_generation(EpochMillis::new(1).unwrap()).unwrap();
}

fn main() { let _ = probe; }
"""
        )
        environment = os.environ.copy()
        result = subprocess.run(
            ["cargo", "check", "--manifest-path", str(consumer / "Cargo.toml")],
            cwd=ROOT,
            env=environment,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
        )
        if result.returncode == 0:
            print("FAIL: external consumer can call the removed Core fixture API")
            return 1
        if EXPECTED not in result.stdout:
            print("FAIL: compile failed for an unrelated reason")
            print(result.stdout)
            return 1
        print(f"OK: external consumer rejected the removed Core fixture API ({EXPECTED})")
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
