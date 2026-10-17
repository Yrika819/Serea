#!/usr/bin/env python3
"""Compile positive and adversarial external Cargo consumers against Serea."""

from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[2]
SOURCE = Path(__file__).resolve().parent / "src" / "lib.rs"
MANIFEST = Path(__file__).resolve().parent / "Cargo.toml"
LOCK = Path(__file__).resolve().parent / "Cargo.lock"
TARGET = Path(tempfile.gettempdir()) / "serea-p8b-authority-cargo-target"


def run(source: str, expected: str | None) -> tuple[int, str]:
    with tempfile.TemporaryDirectory(prefix="serea-p8b-external-") as temp:
        project = Path(temp) / "consumer"
        (project / "src").mkdir(parents=True)
        manifest = MANIFEST.read_text()
        manifest = manifest.replace("../../crates", str(ROOT / "crates"))
        manifest = manifest.replace("../../vendor", str(ROOT / "vendor"))
        (project / "Cargo.toml").write_text(manifest)
        shutil.copy(LOCK, project / "Cargo.lock")
        (project / "src" / "lib.rs").write_text(source)
        env = os.environ.copy()
        env["CARGO_TARGET_DIR"] = str(TARGET)
        command = ["cargo", "check", "--manifest-path", str(project / "Cargo.toml"), "--locked", "--offline"]
        result = subprocess.run(command, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, env=env, check=False)
        output = result.stdout
        if expected is None:
            ok = result.returncode == 0
        else:
            ok = result.returncode != 0 and expected in output
        print(f"{'PASS' if ok else 'FAIL'}: {expected or 'external public API composition compiles'}")
        print(f"  exit={result.returncode}; diagnostic_match={expected in output if expected else 'n/a'}")
        if expected is not None:
            for line in output.splitlines():
                if expected in line or "error[E" in line:
                    print(f"  {line.strip()}")
        if not ok:
            for line in output.splitlines()[-16:]:
                print(f"  {line.strip()}")
        return (0 if ok else 1), output


def main() -> int:
    fetch = subprocess.run(
        ["cargo", "fetch", "--manifest-path", str(MANIFEST), "--locked"],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    if fetch.returncode != 0:
        print("FAIL: fetch locked external-consumer dependencies")
        for line in fetch.stdout.splitlines()[-16:]:
            print(f"  {line.strip()}")
        return 1

    cases = [
        (SOURCE.read_text(), None),
        (
            "use serea_protocol::{GrantId, StepId, TaskId};\n"
            "use serea_storage::{LeaseGuard, Tx};\n"
            "pub fn read_only(tx: &Tx<'_>, lease: &LeaseGuard, grant: &GrantId, task: &TaskId, step: &StepId) {\n"
            "    let _ = tx.observe_lease_facts(lease);\n"
            "    let _ = tx.observe_grant_facts(grant, task, step);\n"
            "}\n",
            None,
        ),
        (
            "use serea_task_engine::L1FactObservation;\n"
            "pub fn forge() { let _ = L1FactObservation { task: todo!() }; }\n",
            "private fields",
        ),
        (
            "use serea_storage::Tx;\n"
            "pub fn bypass(tx: &Tx<'_>) { let _ = &tx.inner; }\n",
            "E0616",
        ),
        (
            "use serea_storage::SyntheticDispatchPermit;\n"
            "pub fn forge() {\n"
            "    let permit = SyntheticDispatchPermit::new();\n"
            "    let cloned = permit.clone();\n"
            "    let copied = permit;\n"
            "    let _ = (cloned, copied);\n"
            "    let _ = serde_json::to_string(&permit);\n"
            "    let _: SyntheticDispatchPermit = serde_json::from_str(\"{}\").unwrap();\n"
            "}\n",
            "E0432",
        ),
        (
            "use serea_storage::CommitProof;\n"
            "pub fn forge() { let _ = core::mem::size_of::<CommitProof>(); }\n",
            "E0432",
        ),
        (
            "use serea_storage::migration_0005_tests::probe_grant_full_set;\n"
            "pub fn call_probe() { let _ = probe_grant_full_set; }\n",
            "E0432",
        ),
        (
            "use serea_storage::Tx;\n"
            "pub fn bypass(tx: &mut Tx<'_>) { let _ = tx.execute_batch(\"DELETE FROM tasks\"); }\n",
            "E0599",
        ),
        (
            "use serea_policy::{ApprovalAuthenticationError, ApprovalAuthenticationVerifier, ApprovalResponseCandidateV1};\n"
            "pub struct HostVerifier;\n"
            "impl ApprovalAuthenticationVerifier for HostVerifier {\n"
            "    fn verify(&self, candidate: &ApprovalResponseCandidateV1) -> Result<(), ApprovalAuthenticationError> {\n"
            "        let _session = candidate.session_id();\n"
            "        Ok(())\n"
            "    }\n"
            "}\n",
            "no method named `session_id`",
        ),
    ]
    failures = 0
    for source, expected in cases:
        code, _ = run(source, expected)
        failures += code != 0
    return int(failures != 0)


if __name__ == "__main__":
    raise SystemExit(main())
