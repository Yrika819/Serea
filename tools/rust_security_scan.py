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
EXPECTED_CARGO_AUDIT_VERSION = "0.22.2"
EXPECTED_CARGO_AUDIT_OUTPUT = f"cargo-audit-audit {EXPECTED_CARGO_AUDIT_VERSION}"


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
        try:
            text = manifest.read_text(encoding="utf-8")
            metadata = json.loads(vcs.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, json.JSONDecodeError) as exc:
            errors.append(f"vendor provenance is unreadable or invalid: {rel}: {exc}")
            continue
        if not isinstance(metadata, dict):
            errors.append(f"vendor provenance must be a JSON object: {rel}")
            continue
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


def checked_scanner_version() -> tuple[str | None, str | None]:
    """Return the executable's verified version and an error, if any."""
    declared = os.environ.get("CARGO_AUDIT_VERSION")
    if declared != EXPECTED_CARGO_AUDIT_VERSION:
        return None, f"expected cargo-audit version must be {EXPECTED_CARGO_AUDIT_VERSION}"
    proc = run(["cargo", "audit", "--version"])
    if proc.returncode:
        return None, f"cargo-audit version check failed (exit {proc.returncode})"
    output = (proc.stdout + "\n" + proc.stderr).strip()
    expected = EXPECTED_CARGO_AUDIT_OUTPUT
    if output != expected:
        return output or None, f"unexpected cargo-audit version output: {output or '<empty>'}"
    return EXPECTED_CARGO_AUDIT_VERSION, None


def checked_checkout_sha() -> tuple[str | None, str | None]:
    """Ensure the revision GitHub reports is the revision being scanned."""
    proc = run(["git", "rev-parse", "--verify", "HEAD^{commit}"])
    if proc.returncode:
        return None, "could not resolve checked-out commit"
    actual = proc.stdout.strip()
    expected = os.environ.get("GITHUB_SHA")
    if not expected or len(expected) != 40 or any(c not in "0123456789abcdef" for c in expected.lower()):
        return actual, "GITHUB_SHA is missing or malformed"
    if actual.lower() != expected.lower():
        return actual, f"checked-out commit {actual} does not match GITHUB_SHA {expected}"
    dirty = run(["git", "diff", "--quiet", "HEAD", "--"])
    if dirty.returncode:
        return actual, "tracked checkout differs from HEAD; refusing to scan a different tree"
    return actual, None


def checked_tree_sha() -> tuple[str | None, str | None]:
    proc = run(["git", "rev-parse", "--verify", "HEAD^{tree}"])
    if proc.returncode:
        return None, "could not resolve checked-out tree"
    tree = proc.stdout.strip()
    if len(tree) != 40 or any(c not in "0123456789abcdef" for c in tree.lower()):
        return tree or None, "checked-out tree SHA is missing or malformed"
    return tree.lower(), None


def load_exceptions(path: Path) -> tuple[list, list[str]]:
    """Exceptions stay disabled until an externally verifiable approval exists."""
    try:
        parsed = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return [], []
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        return [], [f"exception file is unreadable or invalid: {exc}"]
    if not isinstance(parsed, list):
        return [], ["exception file must contain a JSON array"]
    if parsed:
        return [], ["RustSec exceptions are disabled until Owner approval is externally verifiable"]
    return [], []


def main() -> int:
    OUT.mkdir(exist_ok=True)
    checkout_sha, checkout_error = checked_checkout_sha()
    tree_sha, tree_error = checked_tree_sha()
    scanner_version, scanner_error = checked_scanner_version()
    result = {
        "schema_version": 1,
        "status": "scan_failed",
        "commit_sha": checkout_sha,
        "tree_sha": tree_sha,
        "scanner": {"name": "cargo-audit", "version": scanner_version},
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
    if checkout_error:
        result["errors"].append(checkout_error)
    if tree_error:
        result["errors"].append(tree_error)
    if scanner_error:
        result["errors"].append(scanner_error)
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
        if checkout_error or tree_error or scanner_error:
            result["lockfiles"].append({"path": rel, "exit_code": None, "findings": [], "status": "scan_failed"})
            continue
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
            result["errors"].append(f"cargo-audit JSON could not be parsed for {rel}")
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
    exceptions, exception_errors = load_exceptions(exceptions_path)
    result["errors"].extend(exception_errors)
    active = set()
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
