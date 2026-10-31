#!/usr/bin/env python3
"""Offline fail-closed tests for PR merge-result verification."""
from __future__ import annotations

import os
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
        self.git("checkout", "-q", "--detach", self.base)
        newer_base = self.commit("new base")
        self.git("update-ref", "refs/heads/main", newer_base)
        self.git("checkout", "-q", "--detach", self.merge)
        with self.assertRaisesRegex(verifier.VerificationError, "advanced"):
            verifier.verify(self.env)

    def test_rejects_merge_ref_updated_after_checkout(self):
        self.git("update-ref", "refs/pull/1/merge", self.head)
        with self.assertRaisesRegex(verifier.VerificationError, "merge ref changed"):
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


if __name__ == "__main__":
    unittest.main()
