from __future__ import annotations

import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "tools/generate_android_lockfile_sbom.py"
SPEC = importlib.util.spec_from_file_location("android_lockfile_sbom", SCRIPT)
assert SPEC and SPEC.loader
SBOM = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SBOM)


class AndroidLockfileSbomTests(unittest.TestCase):
    def test_checked_in_lockfile_generates_complete_deterministic_coordinate_inventory(self):
        raw = (ROOT / "android/app/gradle.lockfile").read_bytes()
        first = SBOM.create_bom(raw, "d74998f60c9062343ce9c62a326343e9a4d18e45")
        second = SBOM.create_bom(raw, "d74998f60c9062343ce9c62a326343e9a4d18e45")

        self.assertEqual(first, second)
        self.assertEqual("CycloneDX", first["bomFormat"])
        self.assertEqual("1.5", first["specVersion"])
        self.assertEqual(325, len(first["components"]))
        refs = [component["bom-ref"] for component in first["components"]]
        self.assertEqual(len(refs), len(set(refs)))
        self.assertTrue(all(ref.startswith("pkg:maven/") for ref in refs))
        properties = {value["name"]: value["value"] for value in first["metadata"]["properties"]}
        self.assertEqual(hashlib.sha256(raw).hexdigest(), properties["serea:sbom:lockfile-sha256"])
        self.assertEqual("not-represented-by-gradle-lockfile", properties["serea:sbom:dependency-graph"])
        self.assertEqual("outside-app-lockfile-scope", properties["serea:sbom:build-plugins-sdk-wrapper"])
        self.assertNotIn("dependencies", first)

    def test_empty_configuration_marker_is_not_emitted_as_a_library(self):
        components, empty = SBOM.parse_lockfile(
            b"com.example:runtime:1.2.3=releaseRuntimeClasspath\nempty=androidApis,debugCompileClasspath\n",
        )
        self.assertEqual(1, len(components))
        self.assertEqual(["androidApis", "debugCompileClasspath"], empty)

    def test_dynamic_versions_are_rejected(self):
        for version in ("1.+", "latest.release", "2.0-SNAPSHOT"):
            with self.subTest(version=version), self.assertRaises(SBOM.LockfileError):
                SBOM.parse_lockfile(f"com.example:runtime:{version}=releaseRuntimeClasspath\n".encode())

    def test_malformed_and_duplicate_lockfile_entries_are_rejected(self):
        malformed = b"com.example:runtime=releaseRuntimeClasspath\n"
        with self.assertRaises(SBOM.LockfileError):
            SBOM.parse_lockfile(malformed)
        duplicate = (
            b"com.example:runtime:1.0=releaseRuntimeClasspath\n"
            b"com.example:runtime:1.0=debugRuntimeClasspath\n"
        )
        with self.assertRaises(SBOM.LockfileError):
            SBOM.parse_lockfile(duplicate)

    def test_invalid_source_sha_is_rejected(self):
        with self.assertRaises(SBOM.LockfileError):
            SBOM.create_bom(b"com.example:runtime:1.0=releaseRuntimeClasspath\n", "not-a-sha")

    def test_cli_writes_json_bom_without_touching_repository_lockfile(self):
        original = (ROOT / "android/app/gradle.lockfile").read_bytes()
        with tempfile.TemporaryDirectory() as temp_dir:
            output = Path(temp_dir) / "evidence" / "android.cdx.json"
            status = SBOM.main([
                "--output", str(output),
                "--source-sha", "d74998f60c9062343ce9c62a326343e9a4d18e45",
            ])
            self.assertEqual(0, status)
            parsed = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(325, len(parsed["components"]))
        self.assertEqual(original, (ROOT / "android/app/gradle.lockfile").read_bytes())

    def test_cli_refuses_to_overwrite_source_lockfile(self):
        status = SBOM.main(["--lockfile", str(ROOT / "android/app/gradle.lockfile"),
                            "--output", str(ROOT / "android/app/gradle.lockfile")])
        self.assertEqual(2, status)


if __name__ == "__main__":
    unittest.main()
