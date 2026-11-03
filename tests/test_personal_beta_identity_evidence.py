from __future__ import annotations
import subprocess
import sys
import tempfile
import unittest
import yaml
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VERIFY = ROOT / "tools/verify_personal_beta_identity_evidence.py"
GOOD = """demo_and_beta_parallel_install=PASS
demo_package_data_and_signer_unchanged=PASS
beta_encrypted_room_data_after_emulator_reboot=PASS
same_test_key_version_100_to_101_update=PASS
different_test_key_version_102_update=INSTALL_FAILED_UPDATE_INCOMPATIBLE
owner_signing_key_used=no
demo_uninstall_attempted=no
"""

class IdentityEvidenceTests(unittest.TestCase):
    def run_verifier(self, contents: str | None):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "identity.txt"
            if contents is not None:
                path.write_text(contents)
            return subprocess.run([sys.executable, str(VERIFY), str(path)], capture_output=True, text=True)

    def test_complete_evidence_passes(self):
        self.assertEqual(self.run_verifier(GOOD).returncode, 0)

    def test_missing_artifact_and_each_required_field_fail(self):
        self.assertNotEqual(self.run_verifier(None).returncode, 0)
        for line in GOOD.splitlines():
            evidence = "\n".join(item for item in GOOD.splitlines() if item != line) + "\n"
            self.assertNotEqual(self.run_verifier(evidence).returncode, 0, line)

    def test_failed_identity_or_security_boundary_fails(self):
        for old, new in (("demo_and_beta_parallel_install=PASS", "demo_and_beta_parallel_install=FAIL"),
                         ("owner_signing_key_used=no", "owner_signing_key_used=yes"),
                         ("demo_uninstall_attempted=no", "demo_uninstall_attempted=yes"),
                         ("different_test_key_version_102_update=INSTALL_FAILED_UPDATE_INCOMPATIBLE", "different_test_key_version_102_update=PASS")):
            self.assertNotEqual(self.run_verifier(GOOD.replace(old, new)).returncode, 0)

    def test_required_gate_fails_when_identity_job_fails(self):
        workflow = yaml.safe_load((ROOT / ".github/workflows/android.yml").read_text())
        gate = workflow["jobs"]["android-emulator-required-gate"]
        self.assertIn("android-personal-beta-identity", gate["needs"])
        step = next(x for x in gate["steps"] if x.get("name") == "Require Android checks and executed instrumentation results")
        env = dict(__import__("os").environ)
        env.update({"CHANGE_RESULT":"success", "ANDROID_CHANGED":"true", "DOCS_CHANGED":"false",
                    "BUILD_RESULT":"success", "UNIT_RESULT":"success", "INTEGRATION_RESULT":"success",
                    "SECURITY_RESULT":"success", "EMULATOR_RESULT":"success", "BETA_IDENTITY_RESULT":"failure",
                    "DOCS_RESULT":"success", "AUDIT_RESULT":"success", "BUNDLE_RESULT":"success",
                    "INSTRUMENTATION_RESULT":"PASS"})
        result = subprocess.run(["bash", "-c", step["run"]], env=env, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("beta-identity:failure", result.stdout)

    def test_duplicate_evidence_field_fails(self):
        self.assertNotEqual(self.run_verifier(GOOD + "demo_and_beta_parallel_install=PASS\n").returncode, 0)

if __name__ == "__main__":
    unittest.main()
