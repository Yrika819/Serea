#!/usr/bin/env python3
"""Bind the lockfile component inventory to one inspected Android APK.

The resulting CycloneDX BOM identifies the APK as its subject and records its
SHA-256 and aapt-inspected package/version. The Maven component list still comes
from Gradle's app lockfile; this does not infer dependency edges, prove which
locked artifacts were packaged, establish publisher authenticity, or include
Gradle plugins, the wrapper, or Android SDK components.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import re
import subprocess
import sys
import uuid
from pathlib import Path
from typing import Any


SOURCE_SHA_RE = re.compile(r"(?:[0-9a-fA-F]{40}|[0-9a-fA-F]{64})\Z")
SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")
PACKAGE_ID_RE = re.compile(r"[A-Za-z][A-Za-z0-9_]*(?:\.[A-Za-z][A-Za-z0-9_]*)+\Z")
PACKAGE_LINE_RE = re.compile(
    r"^package:\s+name='(?P<package>[^']+)'\s+"
    r"versionCode='(?P<version_code>[0-9]+)'\s+"
    r"versionName='(?P<version_name>[^']+)'(?:\s.*)?$"
)
NAMESPACE = uuid.UUID("a4e83010-2cc4-5b4e-91b7-36d8c1e96a1f")


class ArtifactSbomError(ValueError):
    """Raised when APK/SBOM provenance cannot be safely bound."""


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def parse_aapt_badging(output: str) -> tuple[str, str, str]:
    for line in output.splitlines():
        match = PACKAGE_LINE_RE.fullmatch(line.strip())
        if not match:
            continue
        package_id = match.group("package")
        version_code = match.group("version_code")
        version_name = match.group("version_name")
        if not PACKAGE_ID_RE.fullmatch(package_id):
            raise ArtifactSbomError("aapt returned an invalid Android package ID.")
        if not version_code.isdecimal() or int(version_code) <= 0:
            raise ArtifactSbomError("aapt returned an invalid Android version code.")
        if not version_name or len(version_name) > 128:
            raise ArtifactSbomError("aapt returned a missing or invalid Android version name.")
        return package_id, version_code, version_name
    raise ArtifactSbomError("aapt did not report an Android package identity.")


def inspect_apk(apk_path: Path, aapt_path: Path) -> tuple[str, str, str]:
    if not apk_path.is_file() or apk_path.is_symlink() or apk_path.stat().st_size <= 0:
        raise ArtifactSbomError("APK must be a nonempty regular file.")
    if not aapt_path.is_file() or not aapt_path.stat().st_mode & 0o111:
        raise ArtifactSbomError("aapt must be an executable file.")
    try:
        result = subprocess.run(
            [str(aapt_path), "dump", "badging", str(apk_path)],
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ArtifactSbomError("Unable to inspect APK metadata with aapt.") from error
    if result.returncode != 0:
        raise ArtifactSbomError("aapt failed to inspect the APK.")
    return parse_aapt_badging(result.stdout)


def _properties(value: Any) -> dict[str, str]:
    if not isinstance(value, list):
        raise ArtifactSbomError("CycloneDX metadata properties are malformed.")
    properties: dict[str, str] = {}
    for item in value:
        if not isinstance(item, dict) or not isinstance(item.get("name"), str) or not isinstance(item.get("value"), str):
            raise ArtifactSbomError("CycloneDX metadata properties are malformed.")
        if item["name"] in properties:
            raise ArtifactSbomError("CycloneDX metadata contains duplicate property names.")
        properties[item["name"]] = item["value"]
    return properties


def bind_artifact_to_bom(
    bom: dict[str, Any],
    *,
    lockfile_path: Path,
    apk_path: Path,
    aapt_path: Path,
    expected_source_sha: str,
    expected_apk_sha256: str,
    expected_package_id: str,
) -> dict[str, Any]:
    """Return a copied CycloneDX BOM with a verified APK subject component."""
    if not SOURCE_SHA_RE.fullmatch(expected_source_sha):
        raise ArtifactSbomError("Source SHA must be a full hexadecimal commit SHA.")
    if not SHA256_RE.fullmatch(expected_apk_sha256):
        raise ArtifactSbomError("Expected APK SHA-256 must be lowercase hexadecimal.")
    if not PACKAGE_ID_RE.fullmatch(expected_package_id):
        raise ArtifactSbomError("Expected Android package ID is malformed.")
    if not lockfile_path.is_file() or lockfile_path.is_symlink():
        raise ArtifactSbomError("Gradle lockfile must be a regular file.")

    if bom.get("bomFormat") != "CycloneDX" or bom.get("specVersion") != "1.5":
        raise ArtifactSbomError("Input must be a CycloneDX 1.5 BOM.")
    metadata = bom.get("metadata")
    if not isinstance(metadata, dict):
        raise ArtifactSbomError("CycloneDX metadata is missing.")
    properties = _properties(metadata.get("properties"))
    if properties.get("serea:source:commit", "").lower() != expected_source_sha.lower():
        raise ArtifactSbomError("SBOM source SHA does not match the requested build source.")
    actual_lockfile_sha = sha256_file(lockfile_path)
    if properties.get("serea:sbom:lockfile-sha256") != actual_lockfile_sha:
        raise ArtifactSbomError("SBOM lockfile digest does not match the checked-in Gradle lockfile.")
    if properties.get("serea:sbom:dependency-graph") != "not-represented-by-gradle-lockfile":
        raise ArtifactSbomError("Input SBOM must preserve the lockfile dependency-graph limitation.")
    if properties.get("serea:sbom:publisher-authenticity") != "not-established-by-this-inventory":
        raise ArtifactSbomError("Input SBOM must preserve the publisher-authenticity limitation.")
    if properties.get("serea:sbom:build-plugins-sdk-wrapper") != "outside-app-lockfile-scope":
        raise ArtifactSbomError("Input SBOM must preserve the plugin, wrapper, and SDK scope limitation.")

    package_id, version_code, version_name = inspect_apk(apk_path, aapt_path)
    if package_id != expected_package_id:
        raise ArtifactSbomError("Inspected APK package ID does not match the expected package.")
    actual_apk_sha = sha256_file(apk_path)
    if actual_apk_sha != expected_apk_sha256:
        raise ArtifactSbomError("APK SHA-256 does not match the independently recorded digest.")

    output = copy.deepcopy(bom)
    output_metadata = output["metadata"]
    output_metadata["component"] = {
        "type": "application",
        "name": "Serea Android debug APK",
        "version": version_name,
        "hashes": [{"alg": "SHA-256", "content": actual_apk_sha}],
        "properties": [
            {"name": "serea:android:package-id", "value": package_id},
            {"name": "serea:android:version-code", "value": version_code},
            {"name": "serea:artifact:sha256", "value": actual_apk_sha},
            {"name": "serea:artifact:purpose", "value": "CI-only debug APK; not a Beta package"},
        ],
    }
    output_properties = _properties(output_metadata.get("properties"))
    output_properties.update({
        "serea:sbom:scope": "debug APK subject plus android/app/gradle.lockfile coordinate inventory",
        "serea:sbom:artifact-binding": "metadata.component SHA-256 and aapt-inspected package/version",
        "serea:sbom:artifact-hashes": "APK subject SHA-256 present; locked library hashes not represented by Gradle lockfile",
        "serea:sbom:artifact-sha256": actual_apk_sha,
        "serea:sbom:artifact-package-id": package_id,
        "serea:sbom:artifact-version-code": version_code,
        "serea:sbom:artifact-version-name": version_name,
        "serea:sbom:artifact-purpose": "CI-only debug APK; not a Beta package",
        "serea:sbom:dependency-association": "lockfile inventory; packaged-component membership not independently established",
        "serea:sbom:source-artifact-provenance": "workflow source input; APK-to-source build provenance not independently attested",
    })
    output_metadata["properties"] = [
        {"name": key, "value": value} for key, value in sorted(output_properties.items())
    ]
    serial_name = f"serea-android-artifact-sbom:{actual_lockfile_sha}:{expected_source_sha.lower()}:{actual_apk_sha}"
    output["serialNumber"] = f"urn:uuid:{uuid.uuid5(NAMESPACE, serial_name)}"
    return output


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sbom", type=Path, required=True)
    parser.add_argument("--lockfile", type=Path, required=True)
    parser.add_argument("--apk", type=Path, required=True)
    parser.add_argument("--aapt", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--expected-apk-sha256", required=True)
    parser.add_argument("--expected-package-id", required=True)
    args = parser.parse_args(argv)
    try:
        if args.output.resolve() in {
            args.sbom.resolve(), args.lockfile.resolve(), args.apk.resolve(),
        }:
            raise ArtifactSbomError("Output must not overwrite the input SBOM, lockfile, or APK.")
        bom = json.loads(args.sbom.read_text(encoding="utf-8"))
        if not isinstance(bom, dict):
            raise ArtifactSbomError("Input CycloneDX BOM must be a JSON object.")
        bound_bom = bind_artifact_to_bom(
            bom,
            lockfile_path=args.lockfile,
            apk_path=args.apk,
            aapt_path=args.aapt,
            expected_source_sha=args.source_sha,
            expected_apk_sha256=args.expected_apk_sha256,
            expected_package_id=args.expected_package_id,
        )
        args.output.parent.mkdir(parents=True, exist_ok=True)
        temporary_output = args.output.with_suffix(args.output.suffix + ".tmp")
        temporary_output.write_text(json.dumps(bound_bom, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        temporary_output.replace(args.output)
    except (OSError, json.JSONDecodeError, ArtifactSbomError) as error:
        print(f"Artifact SBOM binding failed: {error}", file=sys.stderr)
        return 2
    component = bound_bom["metadata"]["component"]
    digest = component["hashes"][0]["content"]
    print(f"Bound CycloneDX {bound_bom['specVersion']} BOM to {component['name']} sha256={digest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
