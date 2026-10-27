from __future__ import annotations

import copy
import hashlib
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools"))
from generate_android_lockfile_sbom import create_bom  # noqa: E402
from bind_android_apk_sbom import ArtifactSbomError, bind_artifact_to_bom  # noqa: E402


SOURCE_SHA = "113a81ea5ab8739740edf0d7c17deb2354adad78"
PACKAGE_ID = "app.serea.android.demo"
BADGING = f"package: name='{PACKAGE_ID}' versionCode='1' versionName='0.1.0-demo' platformBuildVersionName='15'\n"


class AndroidArtifactSbomTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.lockfile = self.directory / "gradle.lockfile"
        self.lockfile.write_bytes(b"com.example:runtime:1.2.3=debugRuntimeClasspath\n")
        self.apk = self.directory / "app-debug.apk"
        self.apk.write_bytes(b"synthetic Android package bytes")
        self.aapt = self.directory / "aapt"
        self.write_aapt(BADGING)
        self.bom = create_bom(self.lockfile.read_bytes(), SOURCE_SHA)

    def write_aapt(self, stdout: str, exit_code: int = 0):
        escaped = stdout.replace("'", "'\\''")
        self.aapt.write_text(
            "#!/bin/sh\n"
            f"printf '%s' '{escaped}'\n"
            f"exit {exit_code}\n",
            encoding="utf-8",
        )
        self.aapt.chmod(0o755)

    def bind(self, *, bom=None, expected_source=SOURCE_SHA, expected_apk=None, expected_package=PACKAGE_ID):
        return bind_artifact_to_bom(
            self.bom if bom is None else bom,
            lockfile_path=self.lockfile,
            apk_path=self.apk,
            aapt_path=self.aapt,
            expected_source_sha=expected_source,
            expected_apk_sha256=expected_apk or hashlib.sha256(self.apk.read_bytes()).hexdigest(),
            expected_package_id=expected_package,
        )

    def test_bom_subject_binds_apk_digest_and_inspected_package_version(self):
        original = copy.deepcopy(self.bom)
        bound = self.bind()
        subject = bound["metadata"]["component"]
        apk_sha = hashlib.sha256(self.apk.read_bytes()).hexdigest()
        self.assertEqual("Serea Android debug APK", subject["name"])
        self.assertEqual("0.1.0-demo", subject["version"])
        self.assertEqual([{"alg": "SHA-256", "content": apk_sha}], subject["hashes"])
        subject_properties = {item["name"]: item["value"] for item in subject["properties"]}
        self.assertEqual(PACKAGE_ID, subject_properties["serea:android:package-id"])
        self.assertEqual("1", subject_properties["serea:android:version-code"])
        properties = {item["name"]: item["value"] for item in bound["metadata"]["properties"]}
        self.assertEqual(apk_sha, properties["serea:sbom:artifact-sha256"])
        self.assertEqual(
            "APK subject SHA-256 present; locked library hashes not represented by Gradle lockfile",
            properties["serea:sbom:artifact-hashes"],
        )
        self.assertEqual("not-represented-by-gradle-lockfile", properties["serea:sbom:dependency-graph"])
        self.assertEqual("not-established-by-this-inventory", properties["serea:sbom:publisher-authenticity"])
        self.assertEqual(
            "lockfile inventory; packaged-component membership not independently established",
            properties["serea:sbom:dependency-association"],
        )
        self.assertEqual(
            "workflow source input; APK-to-source build provenance not independently attested",
            properties["serea:sbom:source-artifact-provenance"],
        )
        self.assertNotIn("dependencies", bound)
        self.assertEqual(original, self.bom)
        self.assertNotEqual(original["serialNumber"], bound["serialNumber"])
        self.assertEqual(bound, self.bind())

    def test_mismatched_apk_digest_fails_closed(self):
        with self.assertRaisesRegex(ArtifactSbomError, "APK SHA-256 does not match"):
            self.bind(expected_apk="0" * 64)

    def test_mismatched_source_sha_fails_closed(self):
        with self.assertRaisesRegex(ArtifactSbomError, "SBOM source SHA does not match"):
            self.bind(expected_source="a" * 40)

    def test_source_sha_must_be_a_full_git_sha(self):
        with self.assertRaisesRegex(ArtifactSbomError, "full hexadecimal commit SHA"):
            self.bind(expected_source="a" * 41)

    def test_mismatched_lockfile_digest_fails_closed(self):
        self.lockfile.write_bytes(b"com.example:runtime:9.9.9=debugRuntimeClasspath\n")
        with self.assertRaisesRegex(ArtifactSbomError, "lockfile digest does not match"):
            self.bind()

    def test_unexpected_package_id_fails_closed(self):
        with self.assertRaisesRegex(ArtifactSbomError, "package ID does not match"):
            self.bind(expected_package="app.serea.android.beta")

    def test_missing_aapt_identity_fails_closed(self):
        self.write_aapt("application-label:'Serea'\n")
        with self.assertRaisesRegex(ArtifactSbomError, "did not report an Android package identity"):
            self.bind()

    def test_aapt_failure_fails_closed(self):
        self.write_aapt("", exit_code=1)
        with self.assertRaisesRegex(ArtifactSbomError, "aapt failed"):
            self.bind()

    def test_lockfile_inventory_limitations_are_required(self):
        altered = copy.deepcopy(self.bom)
        for prop in altered["metadata"]["properties"]:
            if prop["name"] == "serea:sbom:dependency-graph":
                prop["value"] = "complete"
        with self.assertRaisesRegex(ArtifactSbomError, "preserve the lockfile dependency-graph limitation"):
            self.bind(bom=altered)

    def test_publisher_authenticity_and_build_scope_limitations_are_required(self):
        for property_name, message in (
            ("serea:sbom:publisher-authenticity", "preserve the publisher-authenticity limitation"),
            ("serea:sbom:build-plugins-sdk-wrapper", "preserve the plugin, wrapper, and SDK scope limitation"),
        ):
            with self.subTest(property_name=property_name):
                altered = copy.deepcopy(self.bom)
                for prop in altered["metadata"]["properties"]:
                    if prop["name"] == property_name:
                        prop["value"] = "covered"
                with self.assertRaisesRegex(ArtifactSbomError, message):
                    self.bind(bom=altered)


if __name__ == "__main__":
    unittest.main()
