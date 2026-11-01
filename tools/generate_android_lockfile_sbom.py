#!/usr/bin/env python3
"""Generate a deterministic CycloneDX component inventory from Gradle's app lockfile.

This deliberately does not claim to be an artifact-complete or dependency-graph SBOM:
Gradle lockfiles carry selected coordinates/configurations, not artifact hashes, publisher
provenance, plugin-build dependencies, Android SDK components, or dependency edges.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import uuid
from pathlib import Path
from urllib.parse import quote


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_LOCKFILE = ROOT / "android/app/gradle.lockfile"
NAMESPACE = uuid.UUID("b87222c4-95a4-518b-9af4-23fcdfef39ed")
SOURCE_SHA_RE = re.compile(r"[0-9a-fA-F]{40,64}\Z")
DYNAMIC_VERSION_RE = re.compile(r"(?:\+|\*|latest(?:\.|$)|SNAPSHOT)", re.IGNORECASE)


class LockfileError(ValueError):
    pass


def _purl(group: str, name: str, version: str) -> str:
    namespace = quote(group, safe="-._~")
    return f"pkg:maven/{namespace}/{quote(name, safe='-._~')}@{quote(version, safe='-._~')}"


def parse_lockfile(raw: bytes) -> tuple[list[dict[str, object]], list[str]]:
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise LockfileError("Gradle lockfile must be UTF-8.") from error

    components: list[dict[str, object]] = []
    empty_configurations: list[str] = []
    seen_refs: set[str] = set()
    for line_number, raw_line in enumerate(text.splitlines(), 1):
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            raise LockfileError(f"Malformed lockfile entry at line {line_number}.")
        coordinate, raw_configurations = line.split("=", 1)
        configurations = raw_configurations.split(",") if raw_configurations else []
        if not configurations or any(not value for value in configurations):
            raise LockfileError(f"Missing configuration list at line {line_number}.")
        if coordinate == "empty":
            empty_configurations.extend(configurations)
            continue

        parts = coordinate.split(":")
        if len(parts) != 3 or any(not part for part in parts):
            raise LockfileError(f"Malformed Maven coordinate at line {line_number}.")
        group, name, version = parts
        if DYNAMIC_VERSION_RE.search(version):
            raise LockfileError(f"Dynamic dependency version at line {line_number}.")
        reference = _purl(group, name, version)
        if reference in seen_refs:
            raise LockfileError(f"Duplicate locked coordinate at line {line_number}.")
        seen_refs.add(reference)
        components.append({
            "type": "library",
            "bom-ref": reference,
            "group": group,
            "name": name,
            "version": version,
            "purl": reference,
            "properties": [
                {"name": "serea:gradle:configurations", "value": ",".join(sorted(configurations))},
            ],
        })

    if not components:
        raise LockfileError("Gradle lockfile contains no locked components.")
    components.sort(key=lambda component: str(component["bom-ref"]))
    return components, sorted(set(empty_configurations))


def create_bom(raw_lockfile: bytes, source_sha: str | None = None) -> dict[str, object]:
    if source_sha is not None and not SOURCE_SHA_RE.fullmatch(source_sha):
        raise LockfileError("Source SHA must be a full 40- or 64-character hexadecimal value.")
    lockfile_sha = hashlib.sha256(raw_lockfile).hexdigest()
    components, empty_configurations = parse_lockfile(raw_lockfile)
    properties = [
        {"name": "serea:sbom:scope", "value": "android/app/gradle.lockfile component inventory"},
        {"name": "serea:sbom:lockfile-sha256", "value": lockfile_sha},
        {"name": "serea:sbom:dependency-graph", "value": "not-represented-by-gradle-lockfile"},
        {"name": "serea:sbom:artifact-hashes", "value": "not-represented-by-gradle-lockfile"},
        {"name": "serea:sbom:publisher-authenticity", "value": "not-established-by-this-inventory"},
        {"name": "serea:sbom:build-plugins-sdk-wrapper", "value": "outside-app-lockfile-scope"},
        {"name": "serea:gradle:empty-configurations", "value": ",".join(empty_configurations) or "none"},
    ]
    if source_sha is not None:
        properties.append({"name": "serea:source:commit", "value": source_sha.lower()})

    serial_name = f"serea-android-lockfile:{lockfile_sha}:{source_sha or 'unspecified'}"
    return {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "serialNumber": f"urn:uuid:{uuid.uuid5(NAMESPACE, serial_name)}",
        "version": 1,
        "metadata": {
            "tools": {"components": [{
                "type": "application",
                "name": "serea-android-lockfile-sbom-generator",
                "version": "1.0.0",
            }]},
            "component": {
                "type": "application",
                "group": "app.serea",
                "name": "Serea Android app locked dependency inventory",
                "version": "0.1.0",
                "purl": "pkg:generic/app.serea/serea-android-locked-dependency-inventory@0.1.0",
            },
            "properties": properties,
        },
        "components": components,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lockfile", type=Path, default=DEFAULT_LOCKFILE)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source-sha")
    args = parser.parse_args(argv)
    try:
        if args.output.resolve() == args.lockfile.resolve():
            raise LockfileError("SBOM output must not overwrite the Gradle lockfile.")
        raw = args.lockfile.read_bytes()
        bom = create_bom(raw, args.source_sha)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(bom, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    except (OSError, LockfileError) as error:
        print(f"SBOM generation failed: {error}", file=sys.stderr)
        return 2
    print(f"Generated {len(bom['components'])} locked components at {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
