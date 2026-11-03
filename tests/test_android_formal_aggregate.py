from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MODULE = ROOT / "tools/verify_android_formal_aggregate.py"


class FormalAggregateFailureInjectionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.output = self.root / "github-output.txt"
        self.verifier = self.root / "fake_verifier.py"
        self.verifier.write_text(
            "import os, sys; from pathlib import Path; "
            "api=os.getenv('VERIFY_FAIL_API'); "
            "log=sys.argv[sys.argv.index('--profile-verifier-log') + 1]; "
            "assert Path(log).is_file(); "
            "raise SystemExit(7 if api and any(api in arg for arg in sys.argv) else 0)\n"
        )
        for api in (26, 35):
            self.make_api(api)

    def tearDown(self) -> None:
        self.temp.cleanup()

    def make_api(self, api: int) -> None:
        evidence = self.root / f"api{api}"
        (evidence / "junit").mkdir(parents=True)
        (evidence / "api-result.txt").write_text("PASS\n")
        (evidence / f"api{api}-build-provenance.txt").write_text(
            f"source_sha=source\nbuild_checkout_sha=merge\nrun_id=77\n"
            f"artifact_producer=android-emulator-api{api}\ntarget_sha256={'a'*64}\ntest_sha256={'b'*64}\n"
            "target_package=app.serea.android.isolated\ntest_package=app.serea.android.isolated.test\n"
            "build_artifact_attempt=1\nproducer_attempt=2\n"
        )
        (evidence / "build-provenance.txt").write_text(
            "source_checkout_sha=merge\ncampaign_run_id=77\ncampaign_attempt=2\n"
        )
        for name in ("instrumentation-status.txt", "process-death-status.txt", "data-isolation-status.txt", "google-demo-ui-status.txt"):
            (evidence / name).write_text("PASS\n")
        (evidence / "process-death.log").write_text("PASS: process death observed\n")
        profile = (
            "api=26 status=UNSUPPORTED resultCode=262144 embedded=false marker=absent\n"
            if api == 26 else
            "api=35 status=COMPLETED resultCode=2 embedded=true marker=valid\n"
        )
        (evidence / "logcat-live.txt").write_text(f"I SereaProfileVerifier: {profile}")
        cases = "".join(f'<testcase classname="C{i}" name="t{i}"/>' for i in range(105))
        (evidence / "junit/results.xml").write_text(f"<testsuite tests=\"105\">{cases}</testsuite>")

    def invoke(self, api26="success", api35="success", verify_api="", build="success"):
        env = dict(os.environ, VERIFY_FAIL_API=verify_api)
        return subprocess.run(
            [sys.executable, str(MODULE), "--root", str(self.root), "--source-sha", "source",
             "--build-sha", "merge", "--run-id", "77", "--api26-result", api26,
             "--api35-result", api35, "--build-result", build, "--output", str(self.output),
             "--junit-verifier", str(self.verifier), "--source-root", str(self.root),
             "--required-manifest", str(self.root), "--additional-manifest", str(self.root),
             "--non-required-manifest", str(self.root)], capture_output=True, text=True, env=env)

    def assert_fail_closed(self, **kwargs):
        self.output.unlink(missing_ok=True)
        result = self.invoke(**kwargs)
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertNotIn("result=PASS", self.output.read_text() if self.output.exists() else "")

    def test_valid_pair_passes(self):
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.output.read_text(), "result=PASS\n")

    def test_api26_and_api35_junit_verifier_failures(self):
        self.assert_fail_closed(verify_api="api26")
        self.assert_fail_closed(verify_api="api35")

    def test_api_result_fail_and_not_run(self):
        for value in ("FAIL", "NOT_RUN"):
            (self.root / "api26/api-result.txt").write_text(value + "\n")
            self.assert_fail_closed()
            (self.root / "api26/api-result.txt").write_text("PASS\n")

    def test_each_required_status_and_consent_evidence(self):
        for api in (26, 35):
            for filename, value in (("process-death-status.txt", "FAIL"), ("data-isolation-status.txt", "FAIL"), ("google-demo-ui-status.txt", None)):
                path = self.root / f"api{api}" / filename
                original = path.read_text() if path.exists() else None
                if value is None:
                    path.unlink(missing_ok=True)
                else:
                    path.write_text(value + "\n")
                self.assert_fail_closed()
                if original is None:
                    path.write_text("PASS\n")
                else:
                    path.write_text(original)

    def test_source_sha_run_id_and_provenance_mismatch_or_missing(self):
        p = self.root / "api26/api26-build-provenance.txt"
        original = p.read_text()
        for bad in (original.replace("source_sha=source", "source_sha=other"), original.replace("run_id=77", "run_id=78"), ""):
            p.write_text(bad)
            self.assert_fail_closed()
        p.write_text(original)
        p.unlink()
        self.assert_fail_closed()

    def test_duplicate_or_empty_provenance_keys_fail_for_both_apis(self):
        api_keys = (
            "source_sha", "build_checkout_sha", "run_id", "artifact_producer", "producer_attempt",
            "build_artifact_attempt", "target_package", "test_package", "target_sha256", "test_sha256",
        )
        campaign_keys = ("source_checkout_sha", "campaign_run_id", "campaign_attempt")
        for api in (26, 35):
            provenance = self.root / f"api{api}/api{api}-build-provenance.txt"
            campaign = self.root / f"api{api}/build-provenance.txt"
            for path, keys in ((provenance, api_keys), (campaign, campaign_keys)):
                original = path.read_text()
                for key in keys:
                    line = next(line for line in original.splitlines() if line.startswith(key + "="))
                    for duplicate in (line, f"{key}=conflicting-value"):
                        with self.subTest(api=api, key=key, duplicate=duplicate):
                            path.write_text(original + duplicate + "\n")
                            result = self.invoke()
                            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                            self.assertNotIn("result=PASS", self.output.read_text() if self.output.exists() else "")
                            self.assertIn(f"API {api}", result.stderr)
                            self.assertIn(key, result.stderr)
                    with self.subTest(api=api, key=key, empty=True):
                        path.write_text("\n".join(item for item in original.splitlines()
                                                      if not item.startswith(key + "=")) + f"\n{key}=\n")
                        result = self.invoke()
                        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                        self.assertNotIn("result=PASS", self.output.read_text() if self.output.exists() else "")
                    with self.subTest(api=api, key=key, missing=True):
                        path.write_text("\n".join(item for item in original.splitlines()
                                                      if not item.startswith(key + "=")) + "\n")
                        result = self.invoke()
                        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                        self.assertNotIn("result=PASS", self.output.read_text() if self.output.exists() else "")
                path.write_text(original)
        # Even when one API's artifact is rejected, the other API's independent result is reported.
        provenance = self.root / "api26/api26-build-provenance.txt"
        original = provenance.read_text()
        provenance.write_text(original + "source_sha=conflicting-value\n")
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("PASS: API 35 evidence validated", result.stdout)
        self.assertNotIn("result=PASS", self.output.read_text() if self.output.exists() else "")
        provenance.write_text(original)

    def test_junit_under_count_and_skipped_are_rejected(self):
        xml = self.root / "api35/junit/results.xml"
        xml.write_text('<testsuite tests="104">' + ''.join(f'<testcase classname="C{i}" name="t{i}"/>' for i in range(104)) + '</testsuite>')
        self.assert_fail_closed()
        skipped = ''.join(f'<testcase classname="C{i}" name="t{i}">{"<skipped/>" if i == 0 else ""}</testcase>' for i in range(105))
        xml.write_text(f'<testsuite tests="105">{skipped}</testsuite>')
        self.assert_fail_closed()

    def test_each_single_api_job_failure_is_rejected(self):
        self.assert_fail_closed(api26="failure")
        self.assert_fail_closed(api35="failure")
        self.assert_fail_closed(api26="failure", api35="failure")
        self.assert_fail_closed(build="failure")


if __name__ == "__main__":
    unittest.main()
