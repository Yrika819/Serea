#!/usr/bin/env python3
"""Workspace shape smoke test for the Serea Rust workspace.

Standalone: Python standard library only. It never invokes Cargo, never reads
credentials, and never touches the network, so it is deterministic on a clean
checkout.

Checks the mechanically-checkable half of the layering rule in
`docs/architecture/03-crate-map.md` §1 and §5.3:

1. the root workspace declares `crates/serea-protocol` as a member;
2. that member's `Cargo.toml` exists;
3. `serea-protocol` has no internal (path/workspace) dependency of any kind;
4. nothing in the workspace names `serea-testkit` outside a `[dev-dependencies]`
   table, so no runtime crate can reach a test double.

Exits non-zero with a diagnostic on the first failed check.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ROOT_MANIFEST = ROOT / "Cargo.toml"
PROTOCOL_MEMBER = "crates/serea-protocol"
TESTKIT = "serea-testkit"

failures: list[str] = []


def fail(message: str) -> None:
    failures.append(message)


def workspace_members(text: str) -> list[str]:
    """Return the quoted entries of `[workspace] members`."""
    section = re.search(r"^\[workspace\]\s*$(.*?)(?=^\[|\Z)", text, re.M | re.S)
    if section is None:
        return []
    entries = re.search(r"^members\s*=\s*\[(.*?)\]", section.group(1), re.M | re.S)
    if entries is None:
        return []
    return re.findall(r'"([^"]+)"', entries.group(1))


# Any dependency table, including a target-specific or build one. Missing a
# table form here is how a guard gets silently evaded.
DEPENDENCY_TABLE = re.compile(
    r"^\[(?:target\.(?P<target>[^]]+?)\.)?(?P<kind>dev-|build-)?dependencies\]\s*$"
    r"(?P<body>.*?)(?=^\[|\Z)",
    re.M | re.S,
)


def dependency_tables(text: str) -> list[tuple[str, bool]]:
    """Return every dependency table as `(package name, is_dev)`."""
    # Three spellings name a workspace package, and all three must be caught:
    #   serea-protocol = { path = ... }        (plain key)
    #   serea-testkit.workspace = true         (dotted key)
    #   tk = { package = "serea-testkit", ... } (renamed key)
    entry = re.compile(
        r"^\s*(?:(?P<key>[A-Za-z0-9_-]+)(?:\.workspace|\.path|\.version)?\s*=)"
        r"|(?:package\s*=\s*\"(?P<package>[^\"]+)\")",
        re.M,
    )
    tables = []
    for match in DEPENDENCY_TABLE.finditer(text):
        is_dev = (match.group("kind") or "") == "dev-"
        for name in entry.finditer(match.group("body")):
            named = name.group("package") or name.group("key") or ""
            if named.startswith("serea-"):
                tables.append((named, is_dev))
    return tables


def main() -> int:
    if not ROOT_MANIFEST.exists():
        fail(f"root workspace manifest missing: {ROOT_MANIFEST.relative_to(ROOT)}")
        return report()

    root_text = ROOT_MANIFEST.read_text(encoding="utf-8")
    members = workspace_members(root_text)

    if PROTOCOL_MEMBER not in members:
        fail(
            f"workspace members {members} do not include {PROTOCOL_MEMBER!r}; "
            "PROTO-* types have no owner crate"
        )

    protocol_manifest = ROOT / PROTOCOL_MEMBER / "Cargo.toml"
    if not protocol_manifest.is_file():
        fail(f"declared member has no manifest: {protocol_manifest.relative_to(ROOT)}")
        return report()

    protocol_text = protocol_manifest.read_text(encoding="utf-8")
    internal = [name for name, _ in dependency_tables(protocol_text)]
    if internal:
        fail(
            f"{PROTOCOL_MEMBER} declares internal dependencies on {sorted(internal)}; "
            "the contract layer depends on nothing internal"
        )

    # Crate Map 5.3: the test double is unreachable from any runtime crate.
    # Checked across every crate directory, so a `providers/` crate is covered,
    # and across every dependency table form, so a target-specific or build
    # table is covered.
    for manifest in sorted(ROOT.glob("crates/*/Cargo.toml")) + sorted(
        ROOT.glob("crates/*/*/Cargo.toml")
    ):
        if manifest.parent.name == TESTKIT:
            continue
        text = manifest.read_text(encoding="utf-8")
        offenders = [
            f"{name} ({'dev' if is_dev else 'non-dev'})"
            for name, is_dev in dependency_tables(text)
            if name == TESTKIT and not is_dev
        ]
        if offenders:
            fail(
                f"{manifest.parent.name} names {TESTKIT} as {offenders}; a test double "
                "must be reachable only from [dev-dependencies]"
            )

    return report()


def report() -> int:
    if failures:
        for message in failures:
            print(f"FAIL: {message}", file=sys.stderr)
        print(f"\n{len(failures)} workspace invariant failure(s)", file=sys.stderr)
        return 1
    print("OK: workspace shape satisfies the crate-map layering rule")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
