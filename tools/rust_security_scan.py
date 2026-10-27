#!/usr/bin/env python3
"""Fail-closed RustSec lockfile scan with portable evidence output."""

from __future__ import annotations

import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "security-results"
LOCKFILES = [
    "Cargo.lock",
    "vendor/serde_json/Cargo.lock",
    "vendor/jsonschema-value/Cargo.lock",
    "vendor/jsonschema-value/regression-tests/Cargo.lock",
]
VENDORS = {
    "vendor/serde_json": {
        "version": "1.0.151",
        "upstream_revision": "de8500740cdcabffb9734f503e4889def823cf10",
        "modified_rust": ["src/number.rs", "src/raw.rs", "src/value/de.rs"],
        "source_tree_sha256": "c747734a4921784666fc0b41a3cd68cd6719b2c5217bcbdd1d8f5e696a77005f",
    },
    "vendor/jsonschema-value": {
        "version": "0.58.3",
        "upstream_revision": "a171a01f52f20142bc4f1cfb07c6e56e195e8da6",
        "modified_rust": ["src/numeric.rs", "src/types.rs"],
        "source_tree_sha256": "7f54a47ca7755e119619c878e839401cce716634baae42db61484700a0c8d57a",
    },
}


def run(command: list[str]) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(command, cwd=ROOT, text=True, capture_output=True, check=False)
    except OSError as exc:
        return subprocess.CompletedProcess(command, 127, "", str(exc))


def vendor_inventory() -> tuple[list[dict], list[str]]:
    inventory: list[dict] = []
    errors: list[str] = []
    for rel, expected in VENDORS.items():
        base = ROOT / rel
        manifest = base / "Cargo.toml"
        vcs = base / ".cargo_vcs_info.json"
        if not manifest.is_file() or not vcs.is_file():
            errors.append(f"vendor provenance missing: {rel}")
            continue
        text = manifest.read_text(encoding="utf-8")
        metadata = json.loads(vcs.read_text(encoding="utf-8"))
        package = metadata.get("git", {}).get("sha1")
        if f'version = "{expected["version"]}"' not in text:
            errors.append(f"unexpected package version in {rel}/Cargo.toml")
        if package != expected["upstream_revision"]:
            errors.append(f"unexpected upstream revision in {rel}/.cargo_vcs_info.json")
        rust_files = sorted((base / "src").rglob("*.rs"))
        tree_hash = hashlib.sha256()
        modified = []
        for source in rust_files:
            content = source.read_bytes()
            rel_source = source.relative_to(base).as_posix()
            rel_bytes = rel_source.encode("utf-8")
            tree_hash.update(len(rel_bytes).to_bytes(4, "big"))
            tree_hash.update(rel_bytes)
            tree_hash.update(len(content).to_bytes(8, "big"))
            tree_hash.update(content)
            if any(rel_source == allowed for allowed in expected["modified_rust"]):
                modified.append(rel_source)
            inventory.append({
                "package": rel,
                "path": rel_source,
                "sha256": hashlib.sha256(content).hexdigest(),
                "bytes": len(content),
            })
        if modified != sorted(expected["modified_rust"]):
            errors.append(f"vendored Rust source change set differs from reviewed inventory: {rel}")
        if tree_hash.hexdigest() != expected["source_tree_sha256"]:
            errors.append(f"vendored Rust source tree differs from reviewed baseline: {rel}")
    return inventory, errors


def db_metadata() -> dict:
    cargo_home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
    db = cargo_home / "advisory-db"
    rev = run(["git", "-C", str(db), "rev-parse", "HEAD"])
    stamp = run(["git", "-C", str(db), "log", "-1", "--format=%cI"])
    if rev.returncode or stamp.returncode:
        return {"commit": None, "last_updated": None}
    return {"commit": rev.stdout.strip(), "last_updated": stamp.stdout.strip()}


def main() -> int:
    OUT.mkdir(exist_ok=True)
    result = {
        "schema_version": 1,
        "status": "scan_failed",
        "commit_sha": os.environ.get("GITHUB_SHA") or run(["git", "rev-parse", "HEAD"]).stdout.strip(),
        "tree_sha": run(["git", "rev-parse", "HEAD^{tree}"]).stdout.strip(),
        "scanner": {"name": "cargo-audit", "version": os.environ.get("CARGO_AUDIT_VERSION")},
        "toolchain": {
            "rustc": run(["rustc", "--version"]).stdout.strip() or None,
            "cargo": run(["cargo", "--version"]).stdout.strip() or None,
        },
        "database": {"commit": None, "last_updated": None},
        "finding_counts": {"critical": 0, "high": 0, "moderate": 0, "low": 0, "unknown": 0, "total": 0},
        "lockfiles": [],
        "vendor_source_inventory": [],
        "errors": [],
        "generated_at": dt.datetime.now(dt.timezone.utc).isoformat(),
    }
    if not result["scanner"]["version"]:
        result["errors"].append("scanner version was not provided")
    scan_lockfiles = list(LOCKFILES)
    if os.environ.get("GITHUB_EVENT_NAME") == "pull_request" and os.environ.get("FULL_SCAN", "false").lower() != "true":
        scan_lockfiles = ["Cargo.lock"]
        base_sha = os.environ.get("BASE_SHA")
        if not base_sha:
            result["errors"].append("pull request base SHA missing; cannot select changed vendor lockfiles")
        else:
            changed = run(["git", "diff", "--name-only", f"{base_sha}...HEAD"])
            if changed.returncode:
                result["errors"].append("could not identify changed vendor files for lightweight pull request scan")
            else:
                paths = set(changed.stdout.splitlines())
                for rel in LOCKFILES[1:]:
                    vendor_root = rel.split("/Cargo.lock", 1)[0]
                    if any(path == rel or path.startswith(vendor_root + "/") for path in paths):
                        scan_lockfiles.append(rel)
    for rel in scan_lockfiles:
        if not (ROOT / rel).is_file():
            result["errors"].append(f"required lockfile missing: {rel}")
            continue
        proc = run(["cargo", "audit", "--json", "--file", rel])
        entry = {"path": rel, "exit_code": proc.returncode, "findings": [], "status": "scan_failed"}
        try:
            parsed = json.loads(proc.stdout)
            vulns = parsed.get("vulnerabilities", {}).get("list", [])
            entry["findings"] = vulns
            entry["status"] = "findings" if vulns else "clean"
            if not parsed.get("database", {}).get("last-updated"):
                result["errors"].append(f"advisory database timestamp missing for {rel}")
            for item in vulns:
                advisory = item.get("advisory", {})
                cvss = advisory.get("cvss")
                try:
                    score = float(cvss)
                except (TypeError, ValueError):
                    score = None
                bucket = ("critical" if score is not None and score >= 9 else
                          "high" if score is not None and score >= 7 else
                          "moderate" if score is not None and score >= 4 else
                          "low" if score is not None else "unknown")
                result["finding_counts"][bucket] += 1
                result["finding_counts"]["total"] += 1
        except (json.JSONDecodeError, AttributeError, TypeError) as exc:
            entry["stderr"] = proc.stderr[-4000:]
            entry["parse_error"] = str(exc)
        if proc.returncode and not entry["findings"]:
            entry["stderr"] = proc.stderr[-4000:]
        if proc.returncode not in (0, 1):
            result["errors"].append(f"cargo-audit execution failed for {rel} (exit {proc.returncode})")
        if proc.returncode == 1 and entry["status"] == "clean":
            result["errors"].append(f"cargo-audit failed without reporting findings for {rel}")
        result["lockfiles"].append(entry)

    inventory, vendor_errors = vendor_inventory()
    result["vendor_source_inventory"] = inventory
    result["errors"].extend(vendor_errors)
    result["database"] = db_metadata()
    if not result["database"]["last_updated"]:
        result["errors"].append("RustSec advisory database revision/timestamp unavailable")
    if len(result["lockfiles"]) != len(scan_lockfiles):
        result["errors"].append("one or more required lockfile scan results are missing")
    result["scan_scope"] = "full" if len(scan_lockfiles) == len(LOCKFILES) else "pull_request_changed_lockfiles"

    # Any unexcepted advisory fails closed. Exceptions require an explicit
    # time-limited record; an owner approval cannot waive scan/result failures.
    exceptions_path = ROOT / "security" / "rust-audit-exceptions.json"
    exceptions = json.loads(exceptions_path.read_text(encoding="utf-8")) if exceptions_path.exists() else []
    today = dt.date.today()
    active = set()
    for e in exceptions:
        required = {"advisory_id", "package", "rationale", "expires", "owner_approval"}
        if not required.issubset(e) or not e.get("rationale"):
            result["errors"].append("malformed vulnerability exception")
            continue
        approval = e["owner_approval"]
        if not approval.get("approved_by") or not approval.get("approved_at") or not approval.get("approval_ref"):
            result["errors"].append(f"exception lacks verifiable Owner approval: {e['advisory_id']}")
            continue
        try:
            expiry = dt.date.fromisoformat(e["expires"])
        except (TypeError, ValueError):
            result["errors"].append(f"exception has invalid expiry: {e['advisory_id']}")
            continue
        if expiry < today:
            result["errors"].append(f"exception expired: {e['advisory_id']}")
            continue
        active.add((e["advisory_id"], e["package"]))
    unexcepted = []
    for lock in result["lockfiles"]:
        for item in lock["findings"]:
            adv = item.get("advisory", {})
            pkg = item.get("package", {})
            key = (adv.get("id"), pkg.get("name"))
            if key not in active:
                unexcepted.append({"lockfile": lock["path"], "advisory_id": key[0], "package": key[1]})
    result["unexcepted_findings"] = unexcepted
    result["status"] = "scan_failed" if result["errors"] else ("findings" if unexcepted else "passed")
    (OUT / "rust-security.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    if result["status"] != "passed":
        print(f"Rust security gate: {result['status']}; findings={result['finding_counts']['total']}; errors={len(result['errors'])}", file=sys.stderr)
        return 1
    print(f"Rust security gate passed; findings={result['finding_counts']['total']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
