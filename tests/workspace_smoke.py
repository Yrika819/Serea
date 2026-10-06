#!/usr/bin/env python3
"""P3B workspace shape and crate dependency direction.

Python 3.9-compatible standard library only; no Cargo invocation or network.
Checks ordinary, build, dev and target-specific dependencies, including aliases,
dotted keys, subtables, workspace inheritance and path package identities.
Engine's internal non-dev dependencies are limited to protocol/storage, with no
non-dev rusqlite edge. Storage depends only on protocol; Event Bus depends on
protocol/storage. Storage cannot depend on Event Bus or Engine. Protocol is a
leaf; testkit is dev-only.
The focused TOML subset supports ordinary tables, dotted/quoted keys, single-line
strings, booleans, decimal integers, arrays and inline tables. Unsupported syntax
(e.g. multiline strings, arrays of tables, floats/dates) fails inspection; no
unparsed section is skipped. This is not a replacement for Cargo validation.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EXPECTED_MEMBERS = [
    "crates/serea-event-bus",
    "crates/serea-protocol",
    "crates/serea-storage",
    "crates/serea-task-engine",
    "crates/serea-testkit",
]
PROTOCOL = "serea-protocol"
STORAGE = "serea-storage"
EVENT_BUS = "serea-event-bus"
ENGINE = "serea-task-engine"
TESTKIT = "serea-testkit"
DEPENDENCY_KINDS = ("dependencies", "build-dependencies", "dev-dependencies")


def table(value: object, label: str) -> dict:
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be a TOML table")
    return value


class _Table(dict):
    def __init__(self, kind="implicit"):
        super().__init__()
        self.kind = kind
        self.sealed = False


class ManifestParser:
    """Consume the entire supported TOML subset, or reject it with a line number."""

    def __init__(self, text: str):
        self.text = text.replace("\r\n", "\n")
        self.pos = 0

    def fail(self, message):
        line = self.text.count("\n", 0, self.pos) + 1
        raise ValueError(f"line {line}: {message}")

    def take(self, token):
        if self.text.startswith(token, self.pos):
            self.pos += len(token)
            return True
        return False

    def expect(self, token):
        if not self.take(token):
            self.fail(f"expected {token!r}; unsupported or malformed TOML")

    def space(self, multiline=False):
        while self.pos < len(self.text):
            char = self.text[self.pos]
            if char in (" \t\n" if multiline else " \t"):
                self.pos += 1
            elif multiline and char == "#":
                self.comment()
            else:
                break

    def comment(self):
        end = self.text.find("\n", self.pos)
        self.pos = len(self.text) if end == -1 else end

    def string(self):
        quote = self.text[self.pos]
        if self.text.startswith(quote * 3, self.pos):
            self.fail("unsupported multiline string")
        self.pos += 1
        chars = []
        escapes = {"b": "\b", "t": "\t", "n": "\n", "f": "\f", "r": "\r", '"': '"', "\\": "\\"}
        while self.pos < len(self.text):
            char = self.text[self.pos]
            self.pos += 1
            if char == quote:
                return "".join(chars)
            if (ord(char) < 32 and char != "\t") or ord(char) == 127:
                self.fail("unsupported control character or newline in string")
            if char == "\\" and quote == '"':
                if self.pos == len(self.text):
                    self.fail("unterminated string escape")
                escape = self.text[self.pos]
                self.pos += 1
                if escape in escapes:
                    char = escapes[escape]
                elif escape in ("u", "U"):
                    width = 4 if escape == "u" else 8
                    digits = self.text[self.pos:self.pos + width]
                    if len(digits) != width or not re.fullmatch(r"[0-9a-fA-F]+", digits):
                        self.fail("invalid Unicode escape")
                    codepoint = int(digits, 16)
                    if codepoint > 0x10FFFF or 0xD800 <= codepoint <= 0xDFFF:
                        self.fail("invalid Unicode scalar")
                    char = chr(codepoint)
                    self.pos += width
                else:
                    self.fail("unsupported string escape")
            chars.append(char)
        self.fail("unterminated string")

    def key(self):
        parts = []
        while True:
            self.space()
            if self.pos < len(self.text) and self.text[self.pos] in "\"'":
                parts.append(self.string())
            else:
                match = re.match(r"[A-Za-z0-9_-]+", self.text[self.pos:])
                if match is None:
                    self.fail("unsupported or malformed key")
                parts.append(match[0])
                self.pos += len(match[0])
            self.space()
            if not self.take("."):
                return parts

    def descend(self, root, keys, kind):
        current = root
        for key in keys:
            if current.sealed:
                self.fail("cannot extend a closed inline table")
            if key not in current:
                current[key] = _Table(kind)
            current = current[key]
            if not isinstance(current, _Table):
                self.fail("table conflicts with an existing value")
        return current

    def assign(self, root, keys, value):
        parent = self.descend(root, keys[:-1], "dotted")
        if parent.sealed or keys[-1] in parent:
            self.fail("duplicate key or extension of a closed inline table")
        parent[keys[-1]] = value

    def seal(self, value):
        if isinstance(value, _Table):
            value.sealed = True
            for child in value.values():
                self.seal(child)

    def value(self):
        self.space()
        if self.pos == len(self.text):
            self.fail("missing value")
        if self.text[self.pos] in "\"'":
            return self.string()
        if self.take("["):
            result = []
            self.space(multiline=True)
            if self.take("]"):
                return result
            while True:
                result.append(self.value())
                self.space(multiline=True)
                if self.take("]"):
                    return result
                self.expect(",")
                self.space(multiline=True)
                if self.take("]"):
                    return result
        if self.take("{"):
            result = _Table("inline")
            self.space()
            if not self.take("}"):
                while True:
                    keys = self.key()
                    self.expect("=")
                    self.assign(result, keys, self.value())
                    self.space()
                    if self.take("}"):
                        break
                    self.expect(",")
            self.seal(result)
            return result
        match = re.match(r"[^\s,\]}#]+", self.text[self.pos:])
        if match is None:
            self.fail("unsupported or malformed value")
        token = match[0]
        self.pos += len(token)
        if token in ("true", "false"):
            return token == "true"
        if re.fullmatch(r"[+-]?(?:0|[1-9](?:_?[0-9])*)", token):
            return int(token.replace("_", ""))
        self.fail(f"unsupported value syntax {token!r}")

    def parse(self):
        root = _Table()
        current = root
        while True:
            self.space(multiline=True)
            if self.pos == len(self.text):
                return root
            if self.take("["):
                if self.text.startswith("[", self.pos):
                    self.fail("unsupported array-of-tables header")
                keys = self.key()
                self.expect("]")
                current = self.descend(root, keys, "implicit")
                if current.sealed or current.kind != "implicit":
                    self.fail("duplicate or conflicting table header")
                current.kind = "header"
            else:
                keys = self.key()
                self.expect("=")
                self.assign(current, keys, self.value())
            self.space()
            if self.text.startswith("#", self.pos):
                self.comment()
            if self.pos != len(self.text):
                self.expect("\n")


def parse_manifest(text: str) -> dict:
    try:
        return ManifestParser(text).parse()
    except RecursionError as error:
        raise ValueError("unsupported TOML nesting depth") from error


def load_manifest(path: Path) -> dict:
    try:
        return parse_manifest(path.read_text(encoding="utf-8"))
    except ValueError as error:
        raise ValueError(f"{path}: {error}") from error


def package_name(manifest: dict, path: Path) -> str:
    name = table(manifest.get("package"), f"{path}: package").get("name")
    if not isinstance(name, str) or not name:
        raise ValueError(f"{path}: package.name must be a nonempty string")
    return name


def dependency_spec(value: object, label: str) -> dict:
    if isinstance(value, str):
        return {"version": value}
    spec = table(value, label)
    strings = {"version", "package", "path", "git", "branch", "tag", "rev", "registry"}
    booleans = {"workspace", "optional", "default-features"}
    for field, setting in spec.items():
        if field in strings:
            valid = isinstance(setting, str)
        elif field in booleans:
            valid = isinstance(setting, bool)
        elif field == "features":
            valid = isinstance(setting, list) and all(isinstance(item, str) for item in setting)
        else:
            raise ValueError(f"{label}: unsupported dependency field {field}")
        if not valid:
            raise ValueError(f"{label}: unsupported value for dependency field {field}")
    return spec


def dependency_tables(manifest: dict, shared: dict, manifest_path: Path):
    """Yield (resolved package name, internal, dev-only) for every dependency."""
    scopes = [manifest]
    targets = table(manifest.get("target", {}), f"{manifest_path}: target")
    for key, value in targets.items():
        scope = table(value, f"{manifest_path}: target.{key}")
        if any(field not in DEPENDENCY_KINDS for field in scope):
            raise ValueError(f"{manifest_path}: unsupported target dependency scope {key}")
        scopes.append(scope)
    for scope in scopes:
        if any(field in scope for field in ("build_dependencies", "dev_dependencies")):
            raise ValueError(f"{manifest_path}: unsupported legacy dependency table spelling")
        for kind in DEPENDENCY_KINDS:
            dependencies = table(scope.get(kind, {}), f"{manifest_path}: {kind}")
            for key, value in dependencies.items():
                spec = dependency_spec(value, f"{manifest_path}: dependency {key}")
                inherited = spec.get("workspace", False)
                if not isinstance(inherited, bool):
                    raise ValueError(f"{manifest_path}: {key}.workspace must be a boolean")
                if inherited:
                    if key not in shared:
                        raise ValueError(f"{manifest_path}: missing workspace dependency {key}")
                    if any(field in spec for field in ("path", "package", "version")):
                        raise ValueError(f"{manifest_path}: {key} overrides inherited package/path/version")
                    spec = {**dependency_spec(shared[key], f"workspace dependency {key}"), **spec}
                name = spec.get("package", key)
                if not isinstance(name, str) or not name:
                    raise ValueError(f"{manifest_path}: {key}.package must be a nonempty string")
                internal = name.startswith("serea-")
                if "path" in spec:
                    path = spec["path"]
                    if not isinstance(path, str):
                        raise ValueError(f"{manifest_path}: {key}.path must be a string")
                    base = ROOT if inherited else manifest_path.parent
                    dependency_path = (base / path).resolve()
                    crates = (ROOT / "crates").resolve()
                    internal |= dependency_path == crates or crates in dependency_path.parents
                    dependency_manifest = dependency_path / "Cargo.toml"
                    target_name = package_name(load_manifest(dependency_manifest), dependency_manifest)
                    if "package" in spec and name != target_name:
                        raise ValueError(f"{manifest_path}: {key}.package disagrees with path package {target_name}")
                    name = target_name
                    internal |= name.startswith("serea-")
                yield name, internal, kind == "dev-dependencies"


def main() -> int:
    failures = []
    root_manifest = ROOT / "Cargo.toml"
    if not root_manifest.is_file():
        return report(["root workspace manifest missing: Cargo.toml"])
    try:
        root = load_manifest(root_manifest)
        workspace = table(root.get("workspace"), "workspace")
        members = workspace.get("members", [])
        if not isinstance(members, list) or any(not isinstance(member, str) for member in members):
            raise ValueError("workspace.members must be an array of strings")
        if sorted(members) != EXPECTED_MEMBERS:
            failures.append(
                f"expected exactly P3B members {EXPECTED_MEMBERS}, got {members}"
            )
        for member in EXPECTED_MEMBERS:
            path = ROOT / member / "Cargo.toml"
            if not path.is_file():
                failures.append(f"required member has no manifest: {member}/Cargo.toml")
                continue
            if package_name(load_manifest(path), path) != path.parent.name:
                failures.append(f"required member has wrong package name: {member}/Cargo.toml")

        shared = table(workspace.get("dependencies", {}), "workspace.dependencies")
        for manifest_path in sorted((ROOT / "crates").rglob("Cargo.toml")):
            manifest = load_manifest(manifest_path)
            owner = package_name(manifest, manifest_path)
            for name, internal, is_dev in dependency_tables(manifest, shared, manifest_path):
                if owner == PROTOCOL and internal:
                    failures.append(f"{PROTOCOL} depends on internal {name}; protocol is a leaf")
                if owner == ENGINE and internal and not is_dev and name not in (PROTOCOL, STORAGE):
                    failures.append(
                        f"{ENGINE} has internal non-dev dependency {name}; only protocol/storage are allowed"
                    )
                if owner == ENGINE and name == "rusqlite" and not is_dev:
                    failures.append(f"{ENGINE} has non-dev dependency rusqlite; use storage instead")
                if owner == STORAGE and name == ENGINE:
                    failures.append(f"{STORAGE} depends on {ENGINE}; forbidden even in [dev-dependencies]")
                if owner == STORAGE and name == EVENT_BUS:
                    failures.append(f"{STORAGE} depends on {EVENT_BUS}; forbidden even in [dev-dependencies]")
                if owner == STORAGE and internal and not is_dev and name != PROTOCOL:
                    failures.append(
                        f"{STORAGE} has internal non-dev dependency {name}; only protocol is allowed"
                    )
                if owner == EVENT_BUS and internal and not is_dev and name not in (PROTOCOL, STORAGE):
                    failures.append(
                        f"{EVENT_BUS} has internal non-dev dependency {name}; only protocol/storage are allowed"
                    )
                if name == TESTKIT and not is_dev:
                    failures.append(f"{owner} names {TESTKIT} outside [dev-dependencies]")
    except (OSError, UnicodeError, ValueError) as error:
        failures.append(f"manifest inspection failed: {error}")
    return report(failures)


def report(failures: list[str]) -> int:
    if failures:
        for message in failures:
            print(f"FAIL: {message}", file=sys.stderr)
        print(f"\n{len(failures)} workspace invariant failure(s)", file=sys.stderr)
        return 1
    print("OK: exact P3B protocol/storage/event-bus/task-engine/testkit workspace; "
          "event-bus non-dev internal protocol/storage-only; storage protocol-only "
          "with no Event Bus or task-engine edge; testkit dev-only")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
