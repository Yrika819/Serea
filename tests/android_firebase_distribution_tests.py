from __future__ import annotations

import hashlib
import fnmatch
import json
import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "tools" / "android_firebase_distribution.py"
WORKFLOW = ROOT / ".github/workflows/android-personal-beta.yml"
sys.path.insert(0, str(ROOT / "tools"))

import android_firebase_distribution as pipeline  # noqa: E402


CERTIFICATE = "a" * 64
SOURCE_SHA = "b" * 40
FIREBASE_APP_ID = "1:1234567890:android:abcdef012345"


class AndroidPersonalBetaDistributionTests(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.temp_dir.name)
        self.apk = self.root / "personal-beta-signed-test.apk"
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

    def run_deploy(
        self, expected_sha: str | None = None, distribution_enabled: str = "true"
    ) -> subprocess.CompletedProcess[str]:
        env = os.environ.copy()
        env.update(
            {
                "FIREBASE_CLI": str(self.cli),
                "SEREA_FIREBASE_APP_ID": FIREBASE_APP_ID,
                "SEREA_FIREBASE_APPROVED_TESTER_GROUP": "serea-personal-beta",
                "SEREA_FIREBASE_DISTRIBUTION_ENABLED": distribution_enabled,
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
        self.assertEqual(self.result_file.read_text(encoding="utf-8").strip(), "FAILED_CHECKSUM_MISMATCH")
        self.assertFalse(self.marker_file.exists())
        self.assertNotIn("should never run", completed.stdout + completed.stderr)

    def test_owner_disabled_distribution_blocks_firebase_before_cli_invocation(self):
        self.write_fake_firebase("should never run\n", "", 0)
        completed = self.run_deploy(distribution_enabled="false")
        self.assertNotEqual(completed.returncode, 0)
        self.assertEqual(self.result_file.read_text(encoding="utf-8").strip(), "FAILED_PREFLIGHT")
        self.assertFalse(self.marker_file.exists())

    def test_required_checks_must_pass_on_exact_source_sha(self):
        required = ["Linux fast checks", "Full validation"]
        checks = [
            {
                "id": 1,
                "name": "Linux fast checks",
                "head_sha": SOURCE_SHA,
                "status": "completed",
                "conclusion": "success",
                "started_at": "2026-10-22T00:00:00Z",
                "app": {"slug": "github-actions"},
            },
            {
                "id": 2,
                "name": "Full validation",
                "head_sha": SOURCE_SHA,
                "status": "completed",
                "conclusion": "failure",
                "started_at": "2026-10-22T00:01:00Z",
                "app": {"slug": "github-actions"},
            },
        ]
        self.assertEqual(
            pipeline.required_check_failures(required, checks, SOURCE_SHA),
            ["Full validation: latest result is not success"],
        )
        self.assertIn(
            "Full validation: missing on source SHA",
            pipeline.required_check_failures(required, checks[:1], "c" * 40),
        )

    def _apk_tools(self, application_id: str, certificate: str):
        aapt = self.root / "fake-aapt"
        apksigner = self.root / "fake-apksigner"
        aapt.write_text(
            "#!/bin/sh\nprintf \"package: name='" + application_id + "' versionCode='17' versionName='0.1.0-demo'\\n\"\n",
            encoding="utf-8",
        )
        apksigner.write_text(
            "#!/bin/sh\nprintf 'Signer #1 certificate SHA-256 digest: " + certificate + "\\n'\n",
            encoding="utf-8",
        )
        aapt.chmod(0o700)
        apksigner.chmod(0o700)
        return str(aapt), str(apksigner)

    def test_apk_verification_checks_beta_package_version_certificate_and_checksum(self):
        aapt, apksigner = self._apk_tools("app.serea.android.beta", CERTIFICATE)
        verified = pipeline.verify_apk(
            self.apk, "app.serea.android.beta", 17, CERTIFICATE, aapt, apksigner
        )
        self.assertEqual(verified["application_id"], "app.serea.android.beta")
        self.assertEqual(verified["version_code"], "17")
        self.assertEqual(verified["certificate_sha256"], CERTIFICATE.upper())
        self.assertEqual(verified["apk_sha256"], hashlib.sha256(self.apk.read_bytes()).hexdigest())

    def test_apk_identity_certificate_and_version_mismatches_are_rejected(self):
        aapt, apksigner = self._apk_tools("app.serea.android.demo", CERTIFICATE)
        with self.assertRaisesRegex(pipeline.PipelineError, "Application ID"):
            pipeline.verify_apk(self.apk, "app.serea.android.beta", 17, CERTIFICATE, aapt, apksigner)

        aapt, apksigner = self._apk_tools("app.serea.android.beta", "c" * 64)
        with self.assertRaisesRegex(pipeline.PipelineError, "certificate"):
            pipeline.verify_apk(self.apk, "app.serea.android.beta", 17, CERTIFICATE, aapt, apksigner)

        aapt, apksigner = self._apk_tools("app.serea.android.beta", CERTIFICATE)
        for invalid_version in (1, 0, 2_147_483_648):
            with self.assertRaisesRegex(pipeline.PipelineError, "versionCode"):
                pipeline.verify_apk(self.apk, "app.serea.android.beta", invalid_version, CERTIFICATE, aapt, apksigner)

    def test_source_sha_mismatch_blocks_a_non_exact_checkout(self):
        workflow_ref = "Yrika819/Serea/.github/workflows/android-personal-beta.yml@refs/heads/main"
        with self.assertRaisesRegex(pipeline.PipelineError, "(?i)checked-out source SHA"):
            pipeline.validate_source("Yrika819/Serea", "refs/heads/main", workflow_ref, SOURCE_SHA, "c" * 40)
        with self.assertRaisesRegex(pipeline.PipelineError, "protected main"):
            pipeline.validate_source(
                "Yrika819/Serea", "refs/heads/feature", workflow_ref, SOURCE_SHA, SOURCE_SHA
            )

    def test_release_source_must_still_be_the_current_protected_main_tip(self):
        pipeline.validate_main_tip(SOURCE_SHA, SOURCE_SHA.upper())
        with self.assertRaisesRegex(pipeline.PipelineError, "current protected-main tip"):
            pipeline.validate_main_tip(SOURCE_SHA, "c" * 40)
        for missing_or_invalid in ("", "not-a-commit"):
            with self.assertRaisesRegex(pipeline.PipelineError, "missing or malformed"):
                pipeline.validate_main_tip(SOURCE_SHA, missing_or_invalid)

    def test_missing_environment_reviewer_or_protected_branch_policy_is_rejected(self):
        approved_environment = {
            "name": "serea-personal-beta",
            "protection_rules": [{"type": "required_reviewers", "reviewers": [
                {"type": "User", "reviewer": {"login": "owner", "id": 1}}
            ]}],
            "deployment_branch_policy": {"protected_branches": True, "custom_branch_policies": False},
        }
        pipeline.validate_environment_protection(approved_environment, "serea-personal-beta")

        no_reviewers = {**approved_environment, "protection_rules": []}
        with self.assertRaisesRegex(pipeline.PipelineError, "no required reviewers"):
            pipeline.validate_environment_protection(no_reviewers, "serea-personal-beta")

        malformed_reviewers = {
            **approved_environment,
            "protection_rules": [{"type": "required_reviewers", "reviewers": "owner"}],
        }
        with self.assertRaisesRegex(pipeline.PipelineError, "no required reviewers"):
            pipeline.validate_environment_protection(malformed_reviewers, "serea-personal-beta")
        invalid_reviewer = {
            **approved_environment,
            "protection_rules": [{"type": "required_reviewers", "reviewers": [
                {"type": "User", "reviewer": {"login": "owner", "id": 0}}
            ]}],
        }
        with self.assertRaisesRegex(pipeline.PipelineError, "no required reviewers"):
            pipeline.validate_environment_protection(invalid_reviewer, "serea-personal-beta")
        unrestricted = {
            **approved_environment,
            "deployment_branch_policy": {"protected_branches": False, "custom_branch_policies": True},
        }
        with self.assertRaisesRegex(pipeline.PipelineError, "protected branches"):
            pipeline.validate_environment_protection(unrestricted, "serea-personal-beta")

    def test_missing_or_unprotected_main_is_rejected(self):
        valid_protection = {
            "required_status_checks": {"strict": True, "contexts": ["Linux fast checks"]},
            "required_pull_request_reviews": {"required_approving_review_count": 1},
            "enforce_admins": {"enabled": True},
        }
        pipeline.validate_main_branch_protection(valid_protection, ["Linux fast checks"])
        for unprotected in (
            {},
            {**valid_protection, "required_status_checks": None},
            {**valid_protection, "required_status_checks": {"contexts": []}},
            {**valid_protection, "required_pull_request_reviews": None},
            {**valid_protection, "enforce_admins": {"enabled": False}},
        ):
            with self.assertRaises(pipeline.PipelineError):
                pipeline.validate_main_branch_protection(unprotected, ["Linux fast checks"])

    def test_version_code_increases_per_new_run_and_blocks_same_run_reruns(self):
        self.assertEqual(pipeline.version_code_for_run("1", "1", "100"), 101)
        self.assertEqual(pipeline.version_code_for_run("2", "1", "100"), 102)
        with self.assertRaisesRegex(pipeline.PipelineError, "reruns reuse"):
            pipeline.version_code_for_run("2", "2", "100")
        with self.assertRaisesRegex(pipeline.PipelineError, "outside Android limits"):
            pipeline.version_code_for_run("2", "1", "2147483646")
        with self.assertRaisesRegex(pipeline.PipelineError, "at least 1"):
            pipeline.version_code_for_run("1", "1", "0")

    def test_firebase_app_id_must_resolve_to_the_beta_package(self):
        apps = {"result": [{"appId": FIREBASE_APP_ID, "packageName": "app.serea.android.beta"}]}
        pipeline.verify_firebase_app(apps, FIREBASE_APP_ID, "app.serea.android.beta")
        apps["result"][0]["packageName"] = "app.serea.android.demo"
        with self.assertRaisesRegex(pipeline.PipelineError, "not bound"):
            pipeline.verify_firebase_app(apps, FIREBASE_APP_ID, "app.serea.android.beta")

    def test_missing_wif_fails_closed_even_if_a_long_lived_key_is_present(self):
        env = {
            "SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_BASE64": "temporary-mock-key-bytes",
            "SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PASSWORD": "temporary-mock-store-password",
            "SEREA_ANDROID_PERSONAL_BETA_KEY_ALIAS": "personal-beta-test",
            "SEREA_ANDROID_PERSONAL_BETA_KEY_PASSWORD": "temporary-mock-key-password",
            "SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256": CERTIFICATE,
            "SEREA_PERSONAL_BETA_VERSION_CODE_BASELINE": "100",
            "SEREA_REQUIRED_CHECKS_JSON": json.dumps(["Linux fast checks"]),
            "SEREA_FIREBASE_DISTRIBUTION_ENABLED": "true",
            "SEREA_FIREBASE_PROJECT_ID": "serea-personal-beta-test",
            "SEREA_FIREBASE_APP_ID": FIREBASE_APP_ID,
            "SEREA_FIREBASE_APPROVED_TESTER_GROUP": "serea-personal-beta",
            "GOOGLE_APPLICATION_CREDENTIALS_JSON": "must-not-be-used-or-fallback",
        }
        with patch.dict(os.environ, env, clear=True):
            with self.assertRaisesRegex(pipeline.PipelineError, "not configured"):
                pipeline.run_preflight(distribute_requested=True)

    def test_malformed_release_configuration_fails_closed(self):
        valid = {
            "SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_BASE64": "temporary-mock-key-bytes",
            "SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PASSWORD": "temporary-mock-store-password",
            "SEREA_ANDROID_PERSONAL_BETA_KEY_ALIAS": "personal-beta-test",
            "SEREA_ANDROID_PERSONAL_BETA_KEY_PASSWORD": "temporary-mock-key-password",
            "SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256": CERTIFICATE,
            "SEREA_PERSONAL_BETA_VERSION_CODE_BASELINE": "100",
            "SEREA_REQUIRED_CHECKS_JSON": json.dumps(["Linux fast checks"]),
            "SEREA_FIREBASE_DISTRIBUTION_ENABLED": "true",
            "SEREA_GCP_WIF_PROVIDER": "projects/123456/locations/global/workloadIdentityPools/serea/providers/github",
            "SEREA_GCP_SERVICE_ACCOUNT": "serea-beta@serea-project.iam.gserviceaccount.com",
            "SEREA_FIREBASE_PROJECT_ID": "serea-beta-project",
            "SEREA_FIREBASE_APP_ID": FIREBASE_APP_ID,
            "SEREA_FIREBASE_APPROVED_TESTER_GROUP": "serea-personal-beta",
        }
        invalid_values = (
            ("SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256", "not-a-fingerprint"),
            ("SEREA_PERSONAL_BETA_VERSION_CODE_BASELINE", "0"),
            ("SEREA_REQUIRED_CHECKS_JSON", "[]"),
            ("SEREA_FIREBASE_DISTRIBUTION_ENABLED", "false"),
            ("SEREA_GCP_WIF_PROVIDER", "projects/123456/locations/global/workloadIdentityPools"),
            ("SEREA_GCP_SERVICE_ACCOUNT", "long-lived-key.json"),
            ("SEREA_FIREBASE_PROJECT_ID", "invalid project id"),
            ("SEREA_FIREBASE_APP_ID", "not-an-app-id"),
            ("SEREA_FIREBASE_APPROVED_TESTER_GROUP", "unreviewed group"),
        )
        for key, value in invalid_values:
            with self.subTest(key=key, value=value), patch.dict(os.environ, {**valid, key: value}, clear=True):
                with self.assertRaises(pipeline.PipelineError):
                    pipeline.run_preflight(distribute_requested=True)

    def test_manual_workflow_is_main_only_protected_and_has_no_signed_apk_artifact(self):
        workflow = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("workflow_dispatch:", workflow)
        self.assertNotIn("pull_request:", workflow)
        self.assertNotIn("pull_request_target:", workflow)
        self.assertNotIn("actions/upload-artifact", workflow)
        self.assertIn("ref: ${{ github.sha }}", workflow)
        self.assertIn("if: github.ref == 'refs/heads/main'", workflow)
        self.assertIn("validate-main-tip", workflow)
        self.assertNotIn("merge-base --is-ancestor", workflow)
        self.assertGreaterEqual(workflow.count("git fetch --no-tags origin refs/heads/main"), 4)
        self.assertIn("name: serea-personal-beta", workflow)
        self.assertIn("branches/main/protection", workflow)
        self.assertIn("verify-main-protection", workflow)
        self.assertIn("actions/attest-build-provenance", workflow)
        self.assertIn("google-github-actions/auth", workflow)
        self.assertIn("workload_identity_provider:", workflow)
        self.assertNotIn("credentials_json:", workflow)
        self.assertNotIn("GOOGLE_APPLICATION_CREDENTIALS_JSON", workflow)
        self.assertNotIn("app.serea.android.demo", workflow)

        artifact_blocks = []
        for path in (ROOT / ".github/workflows").glob("*.yml"):
            text = path.read_text(encoding="utf-8")
            for start in (match.start() for match in re.finditer(r"uses:\s*actions/upload-artifact@", text)):
                next_step = text.find("\n      - name:", start + 1)
                artifact_blocks.append(text[start : next_step if next_step >= 0 else len(text)])

        beta_apk = "android/app/build/outputs/apk/beta/app-beta.apk"

        def artifact_patterns(block: str) -> list[str]:
            lines = block.splitlines()
            patterns: list[str] = []
            for index, line in enumerate(lines):
                match = re.match(r"^(\s*)path:\s*(.*)$", line)
                if not match:
                    continue
                indent = len(match.group(1))
                scalar = match.group(2).strip()
                if scalar and scalar not in {"|", ">"}:
                    patterns.append(scalar)
                    continue
                for child in lines[index + 1 :]:
                    if child.strip() and len(child) - len(child.lstrip()) <= indent:
                        break
                    value = child.strip()
                    if value and not value.startswith("#"):
                        patterns.append(value)
            return patterns

        def artifact_includes_beta(patterns: list[str]) -> bool:
            included = False
            for pattern in patterns:
                exclude = pattern.startswith("!")
                candidate = pattern[1:] if exclude else pattern
                if fnmatch.fnmatchcase(beta_apk, candidate):
                    included = not exclude
            return included

        self.assertTrue(artifact_includes_beta(["android/app/build/**"]))
        self.assertTrue(artifact_includes_beta(["android/app/build/outputs/apk/**/*.apk"]))
        self.assertTrue(artifact_includes_beta(["**/*.apk"]))
        self.assertFalse(artifact_includes_beta(["android/app/build/**", "!android/app/build/outputs/apk/beta/**"]))
        for block in artifact_blocks:
            with self.subTest(artifact_block=block[:120]):
                self.assertFalse(artifact_includes_beta(artifact_patterns(block)))

        ci_workflow = (ROOT / ".github/workflows/android.yml").read_text(encoding="utf-8")
        self.assertIn("assembleDebug assembleRelease assembleIsolated", ci_workflow)
        self.assertIn(":app:compileBetaKotlin", ci_workflow)
        self.assertNotIn("assembleBeta", ci_workflow)
        self.assertNotIn("outputs/apk/beta", ci_workflow)

    def test_owner_version_baseline_is_required_for_beta_builds(self):
        required = {
            "SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_BASE64": "temporary-mock-key-bytes",
            "SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PASSWORD": "temporary-mock-store-password",
            "SEREA_ANDROID_PERSONAL_BETA_KEY_ALIAS": "personal-beta-test",
            "SEREA_ANDROID_PERSONAL_BETA_KEY_PASSWORD": "temporary-mock-key-password",
            "SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256": CERTIFICATE,
            "SEREA_REQUIRED_CHECKS_JSON": json.dumps(["Linux fast checks"]),
        }
        with patch.dict(os.environ, required, clear=True):
            with self.assertRaisesRegex(pipeline.PipelineError, "configuration is missing"):
                pipeline.run_preflight(distribute_requested=False)

    def test_environment_review_gate_is_present_and_documented_for_owner_configuration(self):
        workflow = WORKFLOW.read_text(encoding="utf-8")
        runbook = (ROOT / "docs/plans/2026-ANDROID-PERSONAL-BETA-SECURE-DISTRIBUTION.md").read_text(
            encoding="utf-8"
        )
        self.assertIn("environment:\n      name: serea-personal-beta", workflow)
        self.assertIn("required reviewers", runbook.lower())
        self.assertIn("protected branches", runbook.lower())
        self.assertIn("at least one pull request approval", runbook.lower())

    def test_contract_workflow_has_no_credentials_or_oidc_permission(self):
        workflow = (ROOT / ".github/workflows/android-personal-beta-contract-tests.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("pull_request:", workflow)
        self.assertNotIn("secrets.", workflow)
        self.assertNotIn("id-token:", workflow)

    def test_firebase_cli_and_transitive_dependencies_are_locked(self):
        package = json.loads((ROOT / "tools/firebase-distribution/package.json").read_text(encoding="utf-8"))
        lock = json.loads((ROOT / "tools/firebase-distribution/package-lock.json").read_text(encoding="utf-8"))
        self.assertEqual(package["dependencies"]["firebase-tools"], "15.34.1")
        self.assertEqual(lock["packages"]["node_modules/firebase-tools"]["version"], "15.34.1")


if __name__ == "__main__":
    unittest.main()
