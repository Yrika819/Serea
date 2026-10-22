from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "tools" / "android_firebase_distribution.py"
sys.path.insert(0, str(ROOT / "tools"))

import android_firebase_distribution as pipeline  # noqa: E402


CERTIFICATE = "a" * 64
SOURCE_SHA = "b" * 40


class AndroidFirebaseDistributionTests(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.temp_dir.name)
        self.apk = self.root / "owner-signed.apk"
        self.apk.write_bytes(b"owner-signed-test-apk")
        self.notes = self.root / "release-notes.txt"
        self.notes.write_text("Serea Personal Beta test release\n", encoding="utf-8")
        self.result_file = self.root / "distribution-result.txt"
        self.marker_file = self.root / "firebase-invocations.txt"
        self.cli = self.root / "fake-firebase"

    def tearDown(self):
        self.temp_dir.cleanup()

    def write_fake_firebase(self, stdout: str, stderr: str, exit_code: int) -> None:
        script = "\n".join(
            [
                "#!/usr/bin/env python3",
                "import os, sys",
                "with open(os.environ['FAKE_FIREBASE_MARKER'], 'a', encoding='utf-8') as stream:",
                "    stream.write('called\\n')",
                f"sys.stdout.write({stdout!r})",
                f"sys.stderr.write({stderr!r})",
                f"raise SystemExit({exit_code})",
                "",
            ]
        )
        self.cli.write_text(script, encoding="utf-8")
        self.cli.chmod(0o700)

    def run_deploy(self, expected_sha: str | None = None) -> subprocess.CompletedProcess[str]:
        env = os.environ.copy()
        env.update(
            {
                "FIREBASE_CLI": str(self.cli),
                "SEREA_FIREBASE_APP_ID": "1:1234567890:android:abcdef012345",
                "SEREA_FIREBASE_APPROVED_TESTER_GROUP": "serea-personal-beta",
                "SEREA_FIREBASE_DISTRIBUTION_ENABLED": "true",
                "FAKE_FIREBASE_MARKER": str(self.marker_file),
            }
        )
        return subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                "distribute",
                "--apk",
                str(self.apk),
                "--expected-sha256",
                expected_sha or hashlib.sha256(self.apk.read_bytes()).hexdigest(),
                "--release-notes-file",
                str(self.notes),
                "--result-file",
                str(self.result_file),
            ],
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_firebase_distribution_success_suppresses_cli_and_tester_output(self):
        self.write_fake_firebase("uploaded for beta-owner@example.test\n", "token=private\n", 0)

        completed = self.run_deploy()

        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(self.result_file.read_text(encoding="utf-8").strip(), "SUCCESS")
        self.assertEqual(self.marker_file.read_text(encoding="utf-8").strip(), "called")
        self.assertNotIn("beta-owner@example.test", completed.stdout + completed.stderr)
        self.assertNotIn("private", completed.stdout + completed.stderr)

    def test_firebase_distribution_failure_is_recorded_without_raw_cli_output(self):
        self.write_fake_firebase("", "private Firebase request body\n", 1)

        completed = self.run_deploy()

        self.assertNotEqual(completed.returncode, 0)
        self.assertEqual(self.result_file.read_text(encoding="utf-8").strip(), "FAILED")
        self.assertNotIn("private Firebase request body", completed.stdout + completed.stderr)

    def test_firebase_authorization_denial_is_classified_without_tester_details(self):
        self.write_fake_firebase("", "403 permission denied for beta-owner@example.test\n", 1)

        completed = self.run_deploy()

        self.assertNotEqual(completed.returncode, 0)
        self.assertEqual(self.result_file.read_text(encoding="utf-8").strip(), "FAILED_AUTHORIZATION")
        self.assertIn("authorization was denied", completed.stderr)
        self.assertNotIn("beta-owner@example.test", completed.stdout + completed.stderr)

    def test_checksum_mismatch_blocks_firebase_before_cli_invocation(self):
        self.write_fake_firebase("", "should never run\n", 0)

        completed = self.run_deploy("0" * 64)

        self.assertNotEqual(completed.returncode, 0)
        self.assertEqual(
            self.result_file.read_text(encoding="utf-8").strip(), "FAILED_CHECKSUM_MISMATCH"
        )
        self.assertFalse(self.marker_file.exists())
        self.assertNotIn("should never run", completed.stdout + completed.stderr)

    def test_required_checks_must_pass_on_exact_source_sha(self):
        required = ["Linux fast checks", "Full validation"]
        checks = [
            {
                "id": 1,
                "name": "Linux fast checks",
                "head_sha": SOURCE_SHA,
                "status": "completed",
                "conclusion": "success",
                "started_at": "2026-01-01T00:00:00Z",
                "app": {"slug": "github-actions"},
            },
            {
                "id": 2,
                "name": "Full validation",
                "head_sha": SOURCE_SHA,
                "status": "completed",
                "conclusion": "failure",
                "started_at": "2026-01-01T00:01:00Z",
                "app": {"slug": "github-actions"},
            },
        ]

        failures = pipeline.required_check_failures(required, checks, SOURCE_SHA)

        self.assertEqual(failures, ["Full validation: latest result is not success"])

    def test_apk_verification_checks_package_version_and_owner_certificate(self):
        aapt = self.root / "fake-aapt"
        apksigner = self.root / "fake-apksigner"
        aapt.write_text(
            "#!/bin/sh\nprintf \"package: name='app.serea.android.demo' versionCode='17' versionName='0.1.0-demo'\\n\"\n",
            encoding="utf-8",
        )
        apksigner.write_text(
            "#!/bin/sh\nprintf \"Signer #1 certificate SHA-256 digest: "
            + CERTIFICATE
            + "\\n\"\n",
            encoding="utf-8",
        )
        aapt.chmod(0o700)
        apksigner.chmod(0o700)

        verified = pipeline.verify_apk(
            self.apk,
            "app.serea.android.demo",
            17,
            CERTIFICATE,
            str(aapt),
            str(apksigner),
        )

        self.assertEqual(verified["application_id"], "app.serea.android.demo")
        self.assertEqual(verified["version_code"], "17")
        self.assertEqual(verified["certificate_sha256"], CERTIFICATE.upper())
        self.assertEqual(verified["apk_sha256"], hashlib.sha256(self.apk.read_bytes()).hexdigest())

    def test_release_workflow_is_manual_and_never_uploads_an_apk_artifact(self):
        workflow = (ROOT / ".github/workflows/android-firebase-distribution.yml").read_text(
            encoding="utf-8"
        )
        lane_b_workflow = (ROOT / ".github/workflows/android-internal-beta.yml").read_text(
            encoding="utf-8"
        )

        self.assertIn("workflow_dispatch:", workflow)
        self.assertNotIn("pull_request:", workflow)
        self.assertNotIn("pull_request_target:", workflow)
        self.assertNotIn("actions/upload-artifact", workflow)
        self.assertIn("ref: ${{ github.sha }}", workflow)
        self.assertIn("if: github.ref == 'refs/heads/main'", workflow)
        self.assertNotIn("actions/upload-artifact", lane_b_workflow)
        self.assertNotIn("serea-android-internal-beta.apk", lane_b_workflow)

    def test_missing_wif_fails_closed_even_if_a_long_lived_key_is_present(self):
        env = {
            "SEREA_ANDROID_KEYSTORE_BASE64": "owner-key-bytes",
            "SEREA_ANDROID_KEYSTORE_PASSWORD": "owner-store-password",
            "SEREA_ANDROID_KEY_ALIAS": "owner-alias",
            "SEREA_ANDROID_KEY_PASSWORD": "owner-key-password",
            "SEREA_ANDROID_SIGNING_CERT_SHA256": CERTIFICATE,
            "SEREA_REQUIRED_CHECKS_JSON": json.dumps(["Linux fast checks"]),
            "SEREA_FIREBASE_DISTRIBUTION_ENABLED": "true",
            "SEREA_FIREBASE_APP_ID": "1:1234567890:android:abcdef012345",
            "SEREA_FIREBASE_APPROVED_TESTER_GROUP": "serea-personal-beta",
            "GOOGLE_APPLICATION_CREDENTIALS_JSON": "long-lived-key-must-not-be-used",
        }

        with patch.dict(os.environ, env, clear=True):
            with self.assertRaisesRegex(pipeline.PipelineError, "not configured"):
                pipeline.run_preflight(distribute_requested=True)

    def test_pull_request_contract_workflow_has_no_credentials_or_oidc_permission(self):
        workflow = (ROOT / ".github/workflows/android-firebase-distribution-tests.yml").read_text(
            encoding="utf-8"
        )

        self.assertIn("pull_request:", workflow)
        self.assertNotIn("secrets.", workflow)
        self.assertNotIn("id-token:", workflow)

    def test_firebase_cli_and_transitive_dependencies_are_locked(self):
        package = json.loads(
            (ROOT / "tools/firebase-distribution/package.json").read_text(encoding="utf-8")
        )
        lock = json.loads(
            (ROOT / "tools/firebase-distribution/package-lock.json").read_text(encoding="utf-8")
        )

        self.assertEqual(package["dependencies"]["firebase-tools"], "15.34.1")
        self.assertEqual(
            lock["packages"]["node_modules/firebase-tools"]["version"], "15.34.1"
        )


if __name__ == "__main__":
    unittest.main()
