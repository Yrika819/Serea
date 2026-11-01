#!/usr/bin/env python3
"""Offline fail-closed tests for PR merge-result verification."""
from __future__ import annotations

import os
import re
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
import verify_pr_merge_result as verifier


class MergeResultTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.repo = Path(self.temp.name)
        self.git("init", "-q")
        self.git("config", "user.name", "Test Author")
        self.git("config", "user.email", "noreply@github.com")
        self.git("remote", "add", "origin", ".")
        self.base = self.commit("base")
        self.git("branch", "base")
        self.git("branch", "main", self.base)
        self.git("checkout", "-q", "--detach", self.base)
        self.head = self.commit("head")
        self.git("checkout", "-q", "--detach", self.base)
        tree = self.git("rev-parse", f"{self.head}^{{tree}}")
        self.merge = self.git("commit-tree", tree, "-p", self.base, "-p", self.head, "-m", "synthetic merge")
        self.git("update-ref", "refs/remotes/origin/main", self.base)
        self.git("update-ref", "refs/pull/1/merge", self.merge)
        self.env = {
            "SEREA_EVENT_NAME": "pull_request",
            "SEREA_PR_BASE_REF": "main",
            "SEREA_PR_NUMBER": "1",
            "SEREA_PR_BASE_SHA": self.base,
            "SEREA_PR_HEAD_SHA": self.head,
            "SEREA_EXPECTED_MERGE_SHA": self.merge,
        }
        self.git("checkout", "-q", "--detach", self.merge)
        self.original_cwd = Path.cwd()
        os.chdir(self.repo)

    def tearDown(self):
        os.chdir(self.original_cwd)
        self.temp.cleanup()

    def git(self, *args):
        import subprocess
        return subprocess.run(["git", *args], cwd=self.repo, check=True,
                              capture_output=True, text=True).stdout.strip()

    def commit(self, message):
        self.git("commit", "--allow-empty", "-m", message)
        return self.git("rev-parse", "HEAD")

    def test_accepts_exact_merge_sha_parent_order_and_current_base(self):
        self.assertEqual(verifier.verify(self.env), (self.base, self.head, self.merge))

    def test_rejects_wrong_checkout_sha(self):
        self.git("checkout", "-q", "--detach", self.head)
        with self.assertRaises(verifier.VerificationError):
            verifier.verify(self.env)

    def test_rejects_advanced_tracking_base(self):
        # Model a test run that passed its start gate, then observe Base moving
        # before the final gate.
        self.assertEqual(verifier.verify(self.env), (self.base, self.head, self.merge))
        self.git("checkout", "-q", "--detach", self.base)
        newer_base = self.commit("new base")
        self.git("update-ref", "refs/heads/main", newer_base)
        self.git("checkout", "-q", "--detach", self.merge)
        with self.assertRaisesRegex(verifier.VerificationError, "advanced"):
            verifier.verify(self.env)

    def test_rejects_merge_ref_updated_after_checkout(self):
        # The first gate sees the original merge result; the final gate must
        # reject a merge-ref that changed while tests were running.
        self.assertEqual(verifier.verify(self.env), (self.base, self.head, self.merge))
        self.git("update-ref", "refs/pull/1/merge", self.head)
        with self.assertRaisesRegex(verifier.VerificationError, "merge ref changed"):
            verifier.verify(self.env)

    def test_rejects_base_fetch_failure_at_final_gate(self):
        self.assertEqual(verifier.verify(self.env), (self.base, self.head, self.merge))
        self.git("remote", "set-url", "origin", str(self.repo / "missing-origin"))
        with self.assertRaisesRegex(verifier.VerificationError, "git fetch failed"):
            verifier.verify(self.env)

    def test_rejects_merge_ref_fetch_failure_at_final_gate(self):
        self.assertEqual(verifier.verify(self.env), (self.base, self.head, self.merge))
        self.git("update-ref", "-d", "refs/pull/1/merge")
        with self.assertRaisesRegex(verifier.VerificationError, "git fetch failed"):
            verifier.verify(self.env)

    def test_rejects_missing_malformed_or_unavailable_shas(self):
        for key, value in (("SEREA_PR_BASE_SHA", ""),
                           ("SEREA_PR_HEAD_SHA", "z" * 40),
                           ("SEREA_EXPECTED_MERGE_SHA", "1" * 40)):
            with self.subTest(key=key):
                env = dict(self.env)
                env[key] = value
                with self.assertRaises(verifier.VerificationError):
                    verifier.verify(env)

    def test_rejects_wrong_event_invalid_ref_and_parent_mismatch(self):
        for key, value in (("SEREA_EVENT_NAME", "push"),
                           ("SEREA_PR_BASE_REF", "../main"),
                           ("SEREA_PR_NUMBER", "../1"),
                           ("SEREA_PR_HEAD_SHA", self.base)):
            with self.subTest(key=key):
                env = dict(self.env)
                env[key] = value
                with self.assertRaises(verifier.VerificationError):
                    verifier.verify(env)

    def test_full_linux_stable_checks_the_merge_result_it_tests(self):
        root = Path(__file__).resolve().parents[1]
        source = (root / ".github" / "workflows" / "full-ci.yml").read_text()
        match = re.search(
            r"(?ms)^  linux-stable:\n(?P<body>.*?)(?=^  [A-Za-z0-9_-]+:\n|\Z)",
            source,
        )
        self.assertIsNotNone(match)
        body = match.group("body")
        expected = "${{ github.event_name == 'pull_request' && github.sha || github.event_name == 'workflow_dispatch' && inputs.head_sha || github.sha }}"
        checkout = body.split("      - name: Checkout exact validation target", 1)[1].split("      - name:", 1)[0]
        self.assertIn(f"ref: {expected}", checkout)
        self.assertIn(f"EXPECTED_SOURCE_SHA: {expected}", body)
        self.assertIn("name: Verify current PR merge-result", body)
        self.assertIn("SEREA_EXPECTED_MERGE_SHA: ${{ github.sha }}", body)

    def test_fast_and_full_workflows_end_with_success_gated_pr_recheck(self):
        root = Path(__file__).resolve().parents[1]
        for workflow, job, last_check in (
            ("ci.yml", "fast", "Check workspace"),
            ("full-ci.yml", "linux-stable", "Prove release fault-seam exclusion"),
        ):
            with self.subTest(workflow=workflow):
                source = (root / ".github" / "workflows" / workflow).read_text()
                match = re.search(
                    rf"(?ms)^  {re.escape(job)}:\n(?P<body>.*?)(?=^  [A-Za-z0-9_-]+:\n|\Z)",
                    source,
                )
                self.assertIsNotNone(match, f"missing {job} job")
                body = match.group("body")
                names = re.findall(r"^      - name: (.+)$", body, re.M)
                self.assertEqual(names[-1], "Final PR merge-result freshness verification")
                self.assertEqual(names[-2], last_check)
                final_step = body[body.rfind("      - name: Final PR merge-result freshness verification"):]
                self.assertIn("if: ${{ github.event_name == 'pull_request' }}", final_step)
                self.assertIn("run: python3 tools/verify_pr_merge_result.py", final_step)
                self.assertNotIn("always()", final_step)
                self.assertNotIn("continue-on-error", final_step)
                # GitHub applies success() implicitly to a step-level if.
                # Without continue-on-error, an earlier failed required step
                # keeps the job failed and skips this final verification.
                self.assertNotIn("continue-on-error:", body)


if __name__ == "__main__":
    unittest.main()
