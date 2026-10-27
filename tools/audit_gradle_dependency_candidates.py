#!/usr/bin/env python3
"""Build an evidence inventory from an inactive Gradle verification candidate.

The Gradle-generated hashes are candidate values.  Optional network checks fetch
published SHA-256 sidecars, and optional GPG checks verify AndroidX binary
signatures against the fingerprints published by Android Developers.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import csv
import os
import re
import subprocess
import sys
import tempfile
import urllib.error
import urllib.request
import xml.etree.ElementTree as ET
from collections import Counter, defaultdict
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CANDIDATE = ROOT / "docs/security/evidence/gradle-8.14.6-generated-candidate.xml"
LOCKFILE = ROOT / "android/app/gradle.lockfile"
DEFAULT_OUTPUT = ROOT / "docs/security/evidence/gradle-dependency-inventory.csv"
NS = {"v": "https://schema.gradle.org/dependency-verification"}
GOOGLE_MAVEN = "https://dl.google.com/dl/android/maven2/"
MAVEN_CENTRAL = "https://repo.maven.apache.org/maven2/"
PLUGIN_PORTAL = "https://plugins.gradle.org/m2/"
USER_AGENT = "Serea-SEC-A2-dependency-audit/1.0"

# These are the full fingerprints published on Android Developers' Jetpack
# signature verification page.  A signature key is trusted only when its full
# fingerprint matches one of these values.
ANDROIDX_TRUSTED_FINGERPRINTS = {
    "8461EFA0E74ABAE010DE66994EB27DB2A3B88B8B",
    "A5F483CD733A4EBAEA378B2AE88979FB9B30ACF2",
    "0F06FF86BEEAF4E71866EE5232EE5355A6BC6E42",
    "0E225917414670F4442C250DFD533C07C264648F",
}

DIRECT_PLUGIN_COORDINATES = {
    "com.android.application:com.android.application.gradle.plugin:8.12.3",
    "com.android.tools.build:gradle:8.12.3",
    "org.jetbrains.kotlin.android:org.jetbrains.kotlin.android.gradle.plugin:2.4.21",
    "org.jetbrains.kotlin:kotlin-gradle-plugin:2.4.21",
    "org.jetbrains.kotlin.plugin.compose:org.jetbrains.kotlin.plugin.compose.gradle.plugin:2.4.21",
    "org.jetbrains.kotlin:compose-compiler-gradle-plugin:2.4.21",
    "com.google.devtools.ksp:com.google.devtools.ksp.gradle.plugin:2.3.12",
    "com.google.devtools.ksp:symbol-processing-gradle-plugin:2.3.12",
    "com.github.jk1.dependency-license-report:com.github.jk1.dependency-license-report.gradle.plugin:3.1.4",
    "com.github.jk1:gradle-license-report:3.1.4",
}


def read_candidate() -> dict[str, dict[str, object]]:
    tree = ET.parse(CANDIDATE)
    result: dict[str, dict[str, object]] = {}
    for component in tree.findall(".//v:component", NS):
        group, name, version = (component.get(x, "") for x in ("group", "name", "version"))
        coordinate = f"{group}:{name}:{version}"
        artifacts = {}
        for artifact in component.findall("v:artifact", NS):
            checksum = artifact.find("v:sha256", NS)
            artifacts[artifact.get("name", "")] = checksum.get("value", "") if checksum is not None else ""
        result[coordinate] = {"group": group, "name": name, "version": version, "artifacts": artifacts}
    return result


def read_locks() -> dict[str, set[str]]:
    result: dict[str, set[str]] = {}
    for line in LOCKFILE.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#") or "=" not in line:
            continue
        coordinate, configuration_list = line.split("=", 1)
        if coordinate == "empty":
            continue
        result[coordinate] = set(configuration_list.split(","))
    return result


def scope_labels(coordinate: str, configurations: set[str]) -> list[str]:
    scopes: set[str] = set()
    if coordinate in DIRECT_PLUGIN_COORDINATES:
        scopes.add("plugin")
    if not configurations:
        scopes.add("plugin_buildscript")
        return sorted(scopes)

    if any(
        re.search(r"^(debug|release|isolated)(CompileClasspath|RuntimeClasspath)$", item)
        for item in configurations
    ):
        scopes.add("runtime")
    if any("UnitTestCompileClasspath" in item or "UnitTestRuntimeClasspath" in item for item in configurations):
        scopes.add("unit_test")
    if any(
        "AndroidTestCompileClasspath" in item
        or "AndroidTestRuntimeClasspath" in item
        or item.startswith("_internal-unified-test-platform")
        for item in configurations
    ):
        scopes.add("instrumentation")
    if any(
        "ksp" in item.lower()
        or "kotlinCompiler" in item
        or "composeMappingProducer" in item
        or item.startswith("_agp_internal")
        or item.startswith("_internal-unified-test-platform")
        for item in configurations
    ):
        scopes.add("plugin_buildscript")
    if not scopes:
        scopes.add("android_build_tooling")
    return sorted(scopes)


def artifact_url(coordinate: str, filename: str, prefix: str) -> str:
    group, name, version = coordinate.split(":")
    return f"{prefix}{group.replace('.', '/')}/{name}/{version}/{filename}"


def repository_candidates(group: str) -> list[tuple[str, str]]:
    google_group = group.startswith(
        ("androidx.", "com.android.", "com.google.android.", "com.google.firebase.", "com.google.testing.platform.")
    )
    repos = [("Google Maven", GOOGLE_MAVEN), ("Maven Central", MAVEN_CENTRAL), ("Gradle Plugin Portal", PLUGIN_PORTAL)]
    return repos if google_group else repos[1:] + repos[:1]


def fetch_bytes(url: str, timeout: int = 25) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return response.read()


def check_sidecar(coordinate: str, filename: str, candidate_sha256: str, enabled: bool) -> dict[str, str]:
    if not enabled:
        return {"status": "NOT_CHECKED", "sha256": "", "source": "", "url": ""}
    group = coordinate.split(":", 1)[0]
    errors: list[str] = []
    for source, prefix in repository_candidates(group):
        url = artifact_url(coordinate, filename + ".sha256", prefix)
        try:
            body = fetch_bytes(url).decode("ascii", "replace").lower()
            match = re.search(r"(?<![0-9a-f])[0-9a-f]{64}(?![0-9a-f])", body)
            if match:
                reported = match.group(0)
                return {
                    "status": "MATCH" if reported == candidate_sha256.lower() else "MISMATCH",
                    "sha256": reported,
                    "source": source,
                    "url": url,
                }
        except urllib.error.HTTPError as error:
            if error.code not in (404, 410):
                errors.append(f"{source}:{error.code}")
        except Exception as error:  # Network failures must not be reported as missing sidecars.
            errors.append(f"{source}:{type(error).__name__}")
    status = "CHECK_ERROR" if errors else "NO_SIDECAR"
    return {"status": status, "sha256": "", "source": ";".join(errors), "url": ""}


def prepare_androidx_keyring(directory: Path) -> set[str]:
    if not shutil_which("gpg"):
        raise RuntimeError("--verify-androidx-signatures requires gpg")
    env = dict(os.environ, GNUPGHOME=str(directory))
    for fingerprint in sorted(ANDROIDX_TRUSTED_FINGERPRINTS):
        key_url = f"https://keyserver.ubuntu.com/pks/lookup?op=get&search=0x{fingerprint}"
        key_data = fetch_bytes(key_url)
        key_file = directory / f"{fingerprint}.asc"
        key_file.write_bytes(key_data)
        subprocess.run(
            ["gpg", "--batch", "--homedir", str(directory), "--import", str(key_file)],
            env=env,
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    listing = subprocess.run(
        ["gpg", "--batch", "--homedir", str(directory), "--with-colons", "--fingerprint", "--list-keys"],
        env=env,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    imported = {
        line.split(":")[9].upper()
        for line in listing.splitlines()
        if line.startswith("fpr:") and len(line.split(":")) > 9
    }
    missing = ANDROIDX_TRUSTED_FINGERPRINTS - imported
    if missing:
        raise RuntimeError(f"Ubuntu keyserver response did not contain expected full fingerprints: {sorted(missing)}")
    return imported


def shutil_which(command: str) -> str | None:
    from shutil import which

    return which(command)


def verify_androidx_signature(
    coordinate: str, filename: str, enabled: bool, keyring: Path | None
) -> dict[str, str]:
    if not enabled or not coordinate.startswith("androidx.") or not filename.endswith((".jar", ".aar")):
        return {"status": "NOT_AUDITED", "fingerprint": ""}
    assert keyring is not None
    binary_url = artifact_url(coordinate, filename, GOOGLE_MAVEN)
    signature_url = binary_url + ".asc"
    try:
        binary = fetch_bytes(binary_url, timeout=45)
        signature = fetch_bytes(signature_url, timeout=25)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return {"status": "NO_SIGNATURE", "fingerprint": ""}
        return {"status": f"FETCH_ERROR_{error.code}", "fingerprint": ""}
    except Exception as error:
        return {"status": f"FETCH_ERROR_{type(error).__name__}", "fingerprint": ""}

    stem = re.sub(r"[^A-Za-z0-9_.-]", "_", f"{coordinate}-{filename}")
    binary_path = keyring / f"{stem}.artifact"
    signature_path = keyring / f"{stem}.asc"
    binary_path.write_bytes(binary)
    signature_path.write_bytes(signature)
    env = dict(os.environ, GNUPGHOME=str(keyring))
    result = subprocess.run(
        [
            "gpg", "--batch", "--no-options", "--no-auto-key-retrieve", "--homedir", str(keyring),
            "--status-fd", "1", "--verify", str(signature_path), str(binary_path),
        ],
        env=env,
        capture_output=True,
        text=True,
    )
    match = re.search(r"^\[GNUPG:\] VALIDSIG ([0-9A-F]+)", result.stdout, re.MULTILINE)
    fingerprint = match.group(1).upper() if match else ""
    if result.returncode == 0 and fingerprint in ANDROIDX_TRUSTED_FINGERPRINTS:
        return {"status": "VERIFIED_ANDROID_OFFICIAL_KEY", "fingerprint": fingerprint}
    if result.returncode == 0:
        return {"status": "VALID_SIGNATURE_OTHER_KEY", "fingerprint": fingerprint}
    return {"status": "INVALID_OR_UNTRUSTED_SIGNATURE", "fingerprint": fingerprint}


def component_status(artifacts: dict[str, dict[str, str]], sidecars_enabled: bool) -> str:
    if not sidecars_enabled:
        return "NOT_CHECKED"
    statuses = [item["status"] for item in artifacts.values()]
    if statuses and all(status == "MATCH" for status in statuses):
        return "ALL_REPOSITORY_SHA256_MATCH"
    if any(status == "MATCH" for status in statuses):
        return "PARTIAL_REPOSITORY_SHA256_MATCH"
    if any(status == "CHECK_ERROR" for status in statuses):
        return "CHECK_ERROR"
    return "NO_REPOSITORY_SHA256_MATCH"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check-official-sidecars", action="store_true", help="fetch published .sha256 sidecars")
    parser.add_argument(
        "--verify-androidx-signatures",
        action="store_true",
        help="verify AndroidX .jar/.aar PGP signatures using Android Developers' published fingerprints",
    )
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()

    candidate = read_candidate()
    locks = read_locks()
    all_coordinates = set(candidate) | set(locks)
    rows: list[dict[str, str]] = []
    sidecar_results: dict[str, dict[str, dict[str, str]]] = defaultdict(dict)
    signature_results: dict[tuple[str, str], dict[str, str]] = {}

    checks: list[tuple[str, str, str]] = []
    for coordinate, component in candidate.items():
        for filename, checksum in component["artifacts"].items():
            checks.append((coordinate, filename, checksum))
    if args.check_official_sidecars:
        with concurrent.futures.ThreadPoolExecutor(max_workers=32) as pool:
            futures = {
                pool.submit(check_sidecar, coordinate, filename, checksum, True): (coordinate, filename)
                for coordinate, filename, checksum in checks
            }
            for future in concurrent.futures.as_completed(futures):
                coordinate, filename = futures[future]
                sidecar_results[coordinate][filename] = future.result()
    else:
        for coordinate, filename, _checksum in checks:
            sidecar_results[coordinate][filename] = check_sidecar(coordinate, filename, "", False)

    if args.verify_androidx_signatures:
        for coordinate, filename, _checksum in checks:
            signature_results[(coordinate, filename)] = {"status": "NOT_AUDITED", "fingerprint": ""}
        with tempfile.TemporaryDirectory(prefix="serea-sec-a2-gpg-") as temporary:
            keyring = Path(temporary)
            prepare_androidx_keyring(keyring)
            signature_checks = [
                (coordinate, filename)
                for coordinate, filename, _checksum in checks
                if coordinate.startswith("androidx.") and filename.endswith((".jar", ".aar"))
            ]
            with concurrent.futures.ThreadPoolExecutor(max_workers=12) as pool:
                futures = {
                    pool.submit(verify_androidx_signature, coordinate, filename, True, keyring): (coordinate, filename)
                    for coordinate, filename in signature_checks
                }
                for future in concurrent.futures.as_completed(futures):
                    signature_results[futures[future]] = future.result()
    else:
        for coordinate, filename, _checksum in checks:
            signature_results[(coordinate, filename)] = {"status": "NOT_AUDITED", "fingerprint": ""}

    component_states: dict[str, str] = {}
    for coordinate, component in candidate.items():
        component_states[coordinate] = component_status(sidecar_results[coordinate], args.check_official_sidecars)

    fields = [
        "group", "artifact", "version", "scope", "lock_configurations", "artifact_file",
        "gradle_candidate_sha256", "official_sha256_status", "official_sha256",
        "official_sha256_source", "official_sha256_url", "publisher_signature_status",
        "publisher_key_fingerprint", "component_checksum_status",
    ]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w", newline="", encoding="utf-8") as output:
        writer = csv.DictWriter(output, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        for coordinate in sorted(all_coordinates):
            group, artifact, version = coordinate.split(":")
            configurations = locks.get(coordinate, set())
            scopes = ";".join(scope_labels(coordinate, configurations))
            component = candidate.get(coordinate)
            if component is None:
                writer.writerow({
                    "group": group, "artifact": artifact, "version": version,
                    "scope": scopes, "lock_configurations": ";".join(sorted(configurations)),
                    "artifact_file": "", "gradle_candidate_sha256": "",
                    "official_sha256_status": "NO_GRADLE_CANDIDATE",
                    "component_checksum_status": "NO_GRADLE_CANDIDATE",
                })
                continue
            state = component_states[coordinate]
            for filename, checksum in sorted(component["artifacts"].items()):
                sidecar = sidecar_results[coordinate][filename]
                signature = signature_results[(coordinate, filename)]
                writer.writerow({
                    "group": group,
                    "artifact": artifact,
                    "version": version,
                    "scope": scopes,
                    "lock_configurations": ";".join(sorted(configurations)),
                    "artifact_file": filename,
                    "gradle_candidate_sha256": checksum,
                    "official_sha256_status": sidecar["status"],
                    "official_sha256": sidecar["sha256"],
                    "official_sha256_source": sidecar["source"],
                    "official_sha256_url": sidecar["url"],
                    "publisher_signature_status": signature["status"],
                    "publisher_key_fingerprint": signature["fingerprint"],
                    "component_checksum_status": state,
                })

    scope_counts = Counter()
    for coordinate in all_coordinates:
        for scope in scope_labels(coordinate, locks.get(coordinate, set())):
            scope_counts[scope] += 1
    complete = sum(value == "ALL_REPOSITORY_SHA256_MATCH" for value in component_states.values())
    partial = sum(value == "PARTIAL_REPOSITORY_SHA256_MATCH" for value in component_states.values())
    none = sum(value == "NO_REPOSITORY_SHA256_MATCH" for value in component_states.values())
    artifact_statuses = Counter(
        sidecar_results[coordinate][filename]["status"]
        for coordinate, filename, _checksum in checks
    )
    verified_signatures = sum(
        item["status"] == "VERIFIED_ANDROID_OFFICIAL_KEY" for item in signature_results.values()
    )
    print(f"components_total={len(all_coordinates)}")
    print(f"components_with_gradle_candidate={len(candidate)}")
    print(f"lockfile_components_missing_candidate={len(set(locks) - set(candidate))}")
    print(f"scope_counts_overlap={dict(sorted(scope_counts.items()))}")
    print(f"candidate_artifact_files={len(checks)}")
    print(f"official_sha256_artifact_statuses={dict(sorted(artifact_statuses.items()))}")
    print(f"components_all_candidate_files_sha256_matched={complete}")
    print(f"components_partially_sha256_matched={partial}")
    print(f"components_without_sha256_sidecar_match={none}")
    print(f"androidx_binary_signatures_verified={verified_signatures}")
    print(f"inventory={args.output}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"audit failed: {error}", file=sys.stderr)
        raise SystemExit(2)
