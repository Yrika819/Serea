import hashlib
import json
import os
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock

from tools import serea_apk_signer as signer


class ArtifactValidationTests(unittest.TestCase):
    def make_apk(self, path: Path, entries=None):
        with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            for name, data in (entries or [("AndroidManifest.xml", b"manifest"), ("classes.dex", b"dex")]):
                archive.writestr(name, data)
        return path

    def test_accepts_well_formed_unsigned_apk_archive(self):
        with tempfile.TemporaryDirectory() as temp:
            apk = self.make_apk(Path(temp) / "candidate.apk")
            result = signer.inspect_unsigned_apk(apk)
            self.assertEqual(result.size_bytes, apk.stat().st_size)
            self.assertEqual(result.sha256, hashlib.sha256(apk.read_bytes()).hexdigest())

    def test_rejects_duplicate_zip_entries(self):
        with tempfile.TemporaryDirectory() as temp:
            apk = self.make_apk(Path(temp) / "candidate.apk", [
                ("AndroidManifest.xml", b"one"), ("AndroidManifest.xml", b"two")])
            with self.assertRaisesRegex(signer.ArtifactError, "duplicate"):
                signer.inspect_unsigned_apk(apk)

    def test_rejects_traversal_and_symlink_zip_entries(self):
        with tempfile.TemporaryDirectory() as temp:
            traversal = self.make_apk(Path(temp) / "traversal.apk", [("../manifest", b"bad")])
            with self.assertRaises(signer.ArtifactError):
                signer.inspect_unsigned_apk(traversal)

            symlink = Path(temp) / "symlink.apk"
            with zipfile.ZipFile(symlink, "w") as archive:
                info = zipfile.ZipInfo("AndroidManifest.xml")
                info.external_attr = (0o120777 << 16)
                archive.writestr(info, b"target")
            with self.assertRaisesRegex(signer.ArtifactError, "link"):
                signer.inspect_unsigned_apk(symlink)

    def test_rejects_signed_v1_signature_entries(self):
        with tempfile.TemporaryDirectory() as temp:
            apk = self.make_apk(Path(temp) / "signed.apk", [
                ("AndroidManifest.xml", b"manifest"), ("META-INF/CERT.RSA", b"signature")])
            with self.assertRaisesRegex(signer.ArtifactError, "already signed"):
                signer.inspect_unsigned_apk(apk)

    def test_provenance_hash_must_match_separate_owner_approved_digest(self):
        record = {
            "schema": 1,
            "lane": "public",
            "variant": "beta",
            "application_id": "app.serea.android.beta",
            "version_code": 42,
            "version_name": "0.1.0-demo",
            "source_commit": "a" * 40,
            "source_tree": "b" * 40,
            "gradle_wrapper_sha256": "c" * 64,
            "build_config_sha256": "d" * 64,
            "build_conditions": "offline",
            "test_result": "PASS",
            "apk_sha256": "e" * 64,
        }
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "provenance.json"
            path.write_text(json.dumps(record, sort_keys=True, separators=(",", ":")), encoding="utf-8")
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            validated = signer.validate_provenance(
                path, lane="public", application_id=record["application_id"], version_code=42,
                version_name="0.1.0-demo", apk_sha256="e" * 64, approved_apk_sha256="e" * 64,
                approved_provenance_sha256=digest, approved_source_commit="a" * 40,
                approved_source_tree="b" * 40)
            self.assertEqual(validated["source_commit"], "a" * 40)
            with self.assertRaisesRegex(signer.ArtifactError, "approved"):
                signer.validate_provenance(
                    path, lane="public", application_id=record["application_id"], version_code=42,
                    version_name="0.1.0-demo", apk_sha256="e" * 64, approved_apk_sha256="f" * 64,
                    approved_provenance_sha256=digest, approved_source_commit="a" * 40,
                    approved_source_tree="b" * 40)

    def test_android_tool_environment_strips_java_injection_variables(self):
        captured = {}

        def fake_run(command, **kwargs):
            captured.update(kwargs)
            return __import__("subprocess").CompletedProcess(command, 0, stdout=b"", stderr=b"")

        with mock.patch.dict(os.environ, {"JAVA_TOOL_OPTIONS": "-javaagent:untrusted.jar",
                                           "_JAVA_OPTIONS": "-javaagent:other.jar",
                                           "JDK_JAVA_OPTIONS": "-javaagent:third.jar"}), \
                mock.patch.object(signer.subprocess, "run", side_effect=fake_run):
            signer._tool_run(["aapt", "version"])

        environment = captured["env"]
        self.assertIsNotNone(environment)
        self.assertNotIn("JAVA_TOOL_OPTIONS", environment)
        self.assertNotIn("_JAVA_OPTIONS", environment)
        self.assertNotIn("JDK_JAVA_OPTIONS", environment)
        self.assertIn("PATH", environment)

    def test_imports_artifact_into_private_staging_without_mutating_source(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            source = self.make_apk(base / "candidate.apk")
            staging = base / "staging"
            staging.mkdir(mode=0o700)
            os.chmod(staging, 0o700)
            before = source.read_bytes()

            imported = signer.import_apk(source, staging)

            self.assertEqual(imported.read_bytes(), before)
            self.assertEqual(source.read_bytes(), before)
            self.assertEqual(imported.stat().st_mode & 0o777, 0o400)
            self.assertEqual(signer.inspect_unsigned_apk(imported).sha256,
                             hashlib.sha256(before).hexdigest())

    def test_import_refuses_existing_or_unsafe_staging_target(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            source = self.make_apk(base / "candidate.apk")
            staging = base / "staging"
            staging.mkdir(mode=0o700)
            os.chmod(staging, 0o700)
            target = staging / "candidate.apk"
            target.write_bytes(b"preserve")
            with self.assertRaises(signer.ArtifactError):
                signer.import_apk(source, staging)
            self.assertEqual(target.read_bytes(), b"preserve")

    def test_rejects_symlinked_input(self):
        with tempfile.TemporaryDirectory() as temp:
            original = self.make_apk(Path(temp) / "candidate.apk")
            link = Path(temp) / "link.apk"
            link.symlink_to(original)
            with self.assertRaises(signer.ArtifactError):
                signer.inspect_unsigned_apk(link)


if __name__ == "__main__":
    unittest.main()
