from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from argparse import Namespace
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools"))

import android_beta_release as release  # noqa: E402


class AndroidBetaReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.apk = self.root / "unsigned.apk"
        self.apk.write_bytes(b"synthetic-unsigned-beta-version-17")
        self.record = self.root / "release.json"
        self.aapt = self.root / "aapt"
        self.apksigner = self.root / "apksigner"
        self.aapt.write_text(
            "#!/usr/bin/env python3\n"
            "import sys\n"
            "blob=open(sys.argv[-1], 'rb').read()\n"
            "package='app.serea.android.beta' if b'demo' not in blob else 'app.serea.android.demo'\n"
            "version=16 if b'version-16' in blob else (17 if b'version-17' in blob else 18)\n"
            "print(f\"package: name='{package}' versionCode='{version}' versionName='test'\")\n",
            encoding="utf-8",
        )
        self.apksigner.write_text(
            "#!/usr/bin/env python3\n"
            "import sys\n"
            "if sys.argv[1] == 'verify' and b'signature-valid' in open(sys.argv[-1], 'rb').read():\n"
            " print('Signer #1 certificate SHA-256 digest: " + "a" * 64 + "'); raise SystemExit(0)\n"
            "print('DOES NOT VERIFY'); raise SystemExit(1)\n",
            encoding="utf-8",
        )
        self.aapt.chmod(0o700)
        self.apksigner.chmod(0o700)
        self.common = {
            "apk": self.apk,
            "record": self.record,
            "source_sha": "b" * 40,
            "tree_sha": "c" * 40,
            "run_id": "123",
            "version_code": 17,
            "aapt": str(self.aapt),
            "apksigner": str(self.apksigner),
        }

    def tearDown(self):
        self.temp.cleanup()

    def test_unsigned_record_binds_exact_run_source_package_version_and_digest(self):
        release.create_record(Namespace(**self.common))
        saved = json.loads(self.record.read_text(encoding="utf-8"))
        self.assertEqual(saved["signature_state"], "unsigned")
        self.assertEqual(saved["source_sha"], "b" * 40)
        self.assertEqual(saved["source_tree_sha"], "c" * 40)
        self.assertEqual(saved["workflow_run_id"], "123")
        self.assertEqual(saved["apk_sha256"], release.sha256_file(self.apk))
        release.verify_record(Namespace(
            **{**self.common, "expected_sha256": release.sha256_file(self.apk)},
        ))

        with self.assertRaisesRegex(release.ReleaseError, "metadata"):
            release.verify_record(Namespace(
                **{**self.common, "source_sha": "d" * 40, "expected_sha256": release.sha256_file(self.apk)},
            ))
        self.apk.write_bytes(b"changed unsigned APK")
        with self.assertRaisesRegex(release.ReleaseError, "SHA-256"):
            release.verify_record(Namespace(
                **{**self.common, "expected_sha256": saved["apk_sha256"]},
            ))

    def test_build_recorder_rejects_signed_demo_and_version_rollback_artifacts(self):
        signed = self.root / "signed.apk"
        signed.write_bytes(b"synthetic signature-valid APK version-17")
        with self.assertRaisesRegex(release.ReleaseError, "unsigned"):
            release.create_record(Namespace(**{**self.common, "apk": signed}))

        demo = self.root / "demo.apk"
        demo.write_bytes(b"demo version-17")
        with self.assertRaisesRegex(release.ReleaseError, "Application ID"):
            release.create_record(Namespace(**{**self.common, "apk": demo}))

        wrong_version = self.root / "wrong-version.apk"
        wrong_version.write_bytes(b"beta version-16")
        with self.assertRaisesRegex(release.ReleaseError, "versionCode"):
            release.create_record(Namespace(**{**self.common, "apk": wrong_version, "version_code": 17}))

    def test_scanner_copy_rejects_symlinks_without_copying_host_targets(self):
        source = self.root / "android-source"
        destination = self.root / "scanner-source"
        outside = self.root / "host-only.txt"
        source.mkdir()
        outside.write_text("must not enter the scanner context", encoding="utf-8")
        (source / "safe.gradle").write_text("plugins {}", encoding="utf-8")
        (source / "host-link").symlink_to(outside)

        with self.assertRaisesRegex(release.ReleaseError, "symbolic links"):
            release.copy_android_scan_tree(source, destination)
        self.assertFalse(destination.exists())

        (source / "host-link").unlink()
        release.copy_android_scan_tree(source, destination)
        self.assertEqual((destination / "safe.gradle").read_text(encoding="utf-8"), "plugins {}")
        self.assertFalse((destination / "host-only.txt").exists())

    def test_signed_apk_requires_exact_hash_package_version_and_certificate(self):
        signed = self.root / "signed.apk"
        signed.write_bytes(b"signature-valid beta version-17")
        expected_sha = release.sha256_file(signed)
        result = release.verify_signed(signed, expected_sha, 17, "a" * 64, str(self.aapt), str(self.apksigner))
        self.assertEqual(result["certificate_sha256"], "A" * 64)
        with self.assertRaisesRegex(release.ReleaseError, "SHA-256"):
            release.verify_signed(signed, "0" * 64, 17, "a" * 64, str(self.aapt), str(self.apksigner))
        with self.assertRaisesRegex(release.ReleaseError, "certificate"):
            release.verify_signed(signed, expected_sha, 17, "b" * 64, str(self.aapt), str(self.apksigner))

    def test_inline_attestation_policy_rejects_wrong_repo_workflow_commit_run_or_subject(self):
        workflow = (ROOT / ".github/workflows/android-personal-beta.yml").read_text(encoding="utf-8")
        step = workflow.split("- name: Verify Build provenance before the Owner key is materialized", 1)[1]
        python_block = step.split('python3 - "$RUNNER_TEMP/serea-beta-provenance.json" <<\'PY\'\n', 1)[1]
        python_block = python_block.split("\n          PY", 1)[0]
        script = textwrap.dedent(python_block)
        source = "d" * 40
        tree = "e" * 40
        apk_digest = "f" * 64
        certificate = {
            "issuer": "https://token.actions.githubusercontent.com",
            "sourceRepositoryURI": "https://github.com/Yrika819/Serea",
            "sourceRepositoryDigest": f"sha1:{source}",
            "sourceRepositoryRef": "refs/heads/main",
            "buildSignerURI": "https://github.com/Yrika819/Serea/.github/workflows/android-personal-beta.yml@refs/heads/main",
            "runInvocationURI": "https://github.com/Yrika819/Serea/actions/runs/123456789/attempts/1",
            "runnerEnvironment": "github-hosted",
        }
        valid = [{"verificationResult": {
            "statement": {
                "predicateType": "https://slsa.dev/provenance/v1",
                "subject": [{"digest": {"sha256": apk_digest}}],
            },
            "signature": {"certificate": certificate},
        }}]

        def run_policy(document):
            provenance = self.root / "attestation.json"
            provenance.write_text(json.dumps(document), encoding="utf-8")
            env = os.environ.copy()
            env.update({
                "EXPECTED_APK_SHA256": apk_digest,
                "EXPECTED_SOURCE_SHA": source,
                "EXPECTED_TREE_SHA": tree,
                "GITHUB_RUN_ID": "123456789",
                "GITHUB_RUN_ATTEMPT": "1",
            })
            return subprocess.run(
                [sys.executable, "-c", script, str(provenance)], env=env,
                capture_output=True, text=True, check=False,
            )

        self.assertEqual(run_policy(valid).returncode, 0)
        mutations = (
            lambda doc: doc[0]["verificationResult"]["signature"]["certificate"].update(
                sourceRepositoryURI="https://github.com/attacker/other"),
            lambda doc: doc[0]["verificationResult"]["signature"]["certificate"].update(
                buildSignerURI="https://github.com/Yrika819/Serea/.github/workflows/other.yml@refs/heads/main"),
            lambda doc: doc[0]["verificationResult"]["signature"]["certificate"].update(
                sourceRepositoryDigest="sha1:" + "a" * 40),
            lambda doc: doc[0]["verificationResult"]["signature"]["certificate"].update(
                runInvocationURI="https://github.com/Yrika819/Serea/actions/runs/987654321/attempts/1"),
            lambda doc: doc[0]["verificationResult"]["statement"]["subject"][0]["digest"].update(
                sha256="0" * 64),
        )
        for mutate in mutations:
            with self.subTest(mutation=mutate):
                candidate = json.loads(json.dumps(valid))
                mutate(candidate)
                self.assertNotEqual(run_policy(candidate).returncode, 0)

    def test_gcm_handoff_authenticates_manifest_randomizes_and_rejects_tampering_or_replay(self):
        env_password = "SEREA_BETA_TRANSPORT_KEY_PASSWORD"
        password = "disposable-transport-only-test-password"
        original_password = os.environ.get(env_password)
        os.environ[env_password] = password
        try:
            key_plain = self.root / "transport-private-plain.pem"
            key = self.root / "transport-private.pem"
            certificate = self.root / "transport-cert.pem"
            subprocess.run(
                ["openssl", "req", "-new", "-x509", "-newkey", "rsa:2048", "-nodes",
                 "-keyout", str(key_plain), "-out", str(certificate), "-subj", "/CN=Serea throwaway transport",
                 "-days", "1"],
                check=True, capture_output=True, text=True,
            )
            subprocess.run(
                ["openssl", "pkcs8", "-topk8", "-in", str(key_plain), "-out", str(key),
                 "-passout", f"env:{env_password}"],
                check=True, capture_output=True, text=True,
            )
            key_plain.unlink()
            fingerprint = release.certificate_sha256(certificate, "openssl")
            cipher = self.root / "handoff.cms"
            randomized = self.root / "handoff-second.cms"
            manifest_args = {
                "apk": self.apk,
                "certificate": certificate,
                "openssl": "openssl",
                "expected_cert": fingerprint,
                "source_sha": "1" * 40,
                "tree_sha": "2" * 40,
                "run_id": "123456789",
                "run_attempt": "1",
                "version_code": 17,
                "signing_certificate_sha256": "a" * 64,
            }
            release.encrypt_apk(Namespace(**{**manifest_args, "output": cipher}))
            release.encrypt_apk(Namespace(**{**manifest_args, "output": randomized}))
            self.assertNotEqual(cipher.read_bytes(), randomized.read_bytes(), "CMS GCM must use fresh encryption randomness")

            decrypt_args = {
                "ciphertext": cipher,
                "certificate": certificate,
                "private_key": key,
                "openssl": "openssl",
                "expected_cert": fingerprint,
                "expected_sha256": release.sha256_file(self.apk),
                "expected_signing_certificate_sha256": "a" * 64,
                "source_sha": "1" * 40,
                "tree_sha": "2" * 40,
                "run_id": "123456789",
                "run_attempt": "1",
                "version_code": 17,
            }
            decrypted = self.root / "decrypted.apk"
            release.decrypt_apk(Namespace(**{**decrypt_args, "output": decrypted}))
            self.assertEqual(decrypted.read_bytes(), self.apk.read_bytes())

            tampered = self.root / "tampered.cms"
            data = bytearray(cipher.read_bytes())
            data[-1] ^= 1
            tampered.write_bytes(data)
            with self.assertRaisesRegex(release.ReleaseError, "authenticated, decrypted, or parsed"):
                release.decrypt_apk(Namespace(**{**decrypt_args, "ciphertext": tampered,
                                                  "output": self.root / "tampered.apk"}))

            with self.assertRaisesRegex(release.ReleaseError, "another run"):
                release.decrypt_apk(Namespace(**{**decrypt_args, "run_id": "987654321",
                                                  "output": self.root / "replayed.apk"}))
            self.assertFalse((self.root / "replayed.apk").exists())
        finally:
            if original_password is None:
                os.environ.pop(env_password, None)
            else:
                os.environ[env_password] = original_password

    def _fake_docker(self, bin_dir: Path):
        docker = bin_dir / "docker"
        docker.write_text(
            "#!/usr/bin/env python3\n"
            "import json, os, pathlib, sys\n"
            "args=sys.argv[1:]\n"
            "if args[0] == 'build': raise SystemExit(0)\n"
            "mode=os.environ.get('FAKE_DOCKER_MODE', 'success')\n"
            "if mode == 'fail': raise SystemExit(23)\n"
            "if mode == 'missing': raise SystemExit(0)\n"
            "mount=next(x for x in args if x.startswith('type=bind,src=') and ',dst=/out' in x)\n"
            "out=mount.split('src=', 1)[1].split(',', 1)[0]\n"
            "pathlib.Path(out, 'osv-android-dependencies.json').write_text(os.environ.get('FAKE_OSV_JSON', '{}'))\n"
            "raise SystemExit(0)\n",
            encoding="utf-8",
        )
        docker.chmod(0o700)
        return docker

    def _run_osv_wrapper(self, mode: str, json_body: str = "{}", wrong_digest: bool = False):
        bin_dir = self.root / "bin"
        bin_dir.mkdir(exist_ok=True)
        self._fake_docker(bin_dir)
        env = os.environ.copy()
        env.update({
            "PATH": f"{bin_dir}:{env['PATH']}",
            "RUNNER_TEMP": str(self.root / "runner-temp"),
            "GITHUB_RUN_ID": "123",
            "FAKE_DOCKER_MODE": mode,
            "FAKE_OSV_JSON": json_body,
        })
        if wrong_digest:
            env["SEREA_OSV_SCANNER_IMAGE"] = "ghcr.io/google/osv-scanner-action:v2.3.3"
        return subprocess.run(
            ["bash", str(ROOT / "tools/run_pinned_osv_scanner.sh")],
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_osv_wrapper_rejects_mutable_and_unexpected_image_references(self):
        script = (ROOT / "tools/run_pinned_osv_scanner.sh").read_text(encoding="utf-8")
        self.assertIn("fc633b975f339ddfde55c747f5bf4dde71605ccda1073fd535760ba38e4e7065", script)
        self.assertIn("--cap-drop ALL", script)
        self.assertIn("--read-only", script)
        self.assertIn("--user 65532:65532", script)
        self.assertIn("SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt", script)
        self.assertIn("ca_bundle", script.lower())
        self.assertIn("dst=/scan,readonly", script)
        self.assertIn("copy-android-scan-tree", script)
        rejected = self._run_osv_wrapper("success", wrong_digest=True)
        self.assertNotEqual(rejected.returncode, 0)
        self.assertIn("immutable digest", rejected.stderr)

    def test_osv_scanner_failure_missing_output_and_invalid_json_never_pass(self):
        self.assertNotEqual(self._run_osv_wrapper("fail").returncode, 0)
        self.assertNotEqual(self._run_osv_wrapper("missing").returncode, 0)
        self.assertNotEqual(self._run_osv_wrapper("success", "not-json").returncode, 0)
        passed = self._run_osv_wrapper("success", '{"results": []}')
        self.assertEqual(passed.returncode, 0, passed.stderr)

    def test_gradle_and_github_action_workflow_boundaries_are_explicit(self):
        workflow = (ROOT / ".github/workflows/android-personal-beta.yml").read_text(encoding="utf-8")
        build = workflow.split("\n  build-unsigned:\n", 1)[1].split("\n  sign:\n", 1)[0]
        sign = workflow.split("\n  sign:\n", 1)[1].split("\n  distribute:\n", 1)[0]
        distribution = workflow.split("\n  distribute:\n", 1)[1]
        gradle = (ROOT / "android/app/build.gradle.kts").read_text(encoding="utf-8")
        self.assertNotIn("secrets.", build)
        self.assertNotIn("SEREA_FIREBASE", build)
        self.assertNotIn("SEREA_GCP", build)
        self.assertNotIn("secrets.SEREA_ANDROID_PERSONAL_BETA", build)
        self.assertIn("env -u SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PATH", build)
        self.assertIn("-u SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PASSWORD", build)
        self.assertNotIn("actions/checkout", sign)
        self.assertNotIn("gradlew", sign)
        self.assertNotIn("tools/android_", sign)
        self.assertNotIn("id-token: write", sign)
        self.assertNotIn("SEREA_FIREBASE", sign)
        self.assertIn("deployment-branch-policies", sign)
        self.assertIn('policies[0].get("name") == "main"', sign)
        self.assertNotIn("SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PASSWORD", distribution)
        self.assertNotIn("SEREA_ANDROID_PERSONAL_BETA_KEYSTORE", distribution)
        self.assertNotIn("SEREA_BETA_SIGNING_KEY_PASSWORD", distribution)
        self.assertNotIn("actions/checkout", distribution)
        self.assertNotIn("gradlew", distribution)
        self.assertNotIn("tools/android_", distribution)
        self.assertIn("id-token: write", distribution)
        self.assertIn("deployment-branch-policies", distribution)
        self.assertIn("package-lock.json", distribution)
        encrypted_upload = sign.split("name: Upload encrypted handoff only", 1)[1].split(
            "name: Remove local keys", 1
        )[0]
        self.assertIn("serea-personal-beta-signed.apk.cms", encrypted_upload)
        self.assertNotRegex(encrypted_upload, r"serea-personal-beta-signed\.apk(?:['\"\s]|$)")
        self.assertNotIn("SEREA_ANDROID_PERSONAL_BETA_KEYSTORE", gradle)
        self.assertNotIn("validatePersonalBetaSigning", gradle)
        self.assertIn("assembleBeta", build)
        self.assertIn("--source-digest", sign)
        self.assertIn("serea-personal-beta-ciphertext-", sign)
        self.assertNotIn("signBetaBundle", gradle)


if __name__ == "__main__":
    unittest.main()
