#!/usr/bin/env python3
"""Executable fail-closed tests for PR merge-result freshness verification."""
from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path
from unittest.mock import patch

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
        self.git("branch", "main", self.base)
        self.git("checkout", "-q", "--detach", self.base)
        self.head = self.commit("head")
        self.tree = self.git("rev-parse", f"{self.head}^{{tree}}")
        self.merge = self.commit_tree(self.tree, [self.base, self.head], "synthetic merge")
        self.git("update-ref", "refs/pull/1/head", self.head)
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
        return subprocess.run(["git", *args], cwd=self.repo, check=True,
                              capture_output=True, text=True).stdout.strip()

    def commit(self, message):
        self.git("commit", "--allow-empty", "-m", message)
        return self.git("rev-parse", "HEAD")

    def commit_tree(self, tree, parents, message):
        args = ["commit-tree", tree]
        for parent in parents:
            args.extend(("-p", parent))
        return self.git(*args, "-m", message)

    def new_tree(self, filename="tree-marker.txt", content="changed tree\n"):
        self.git("checkout", "-q", "--detach", self.base)
        (self.repo / filename).write_text(content, encoding="utf-8")
        self.git("add", filename)
        self.git("commit", "-q", "-m", "tree change")
        tree = self.git("rev-parse", "HEAD^{tree}")
        self.git("checkout", "-q", "--detach", self.merge)
        return tree

    def test_accepts_exact_merge_sha_parent_order_and_current_refs(self):
        result = verifier.verify(self.env)
        self.assertEqual(result.base, self.base)
        self.assertEqual(result.head, self.head)
        self.assertEqual(result.expected_merge, self.merge)
        self.assertEqual(result.latest_merge, self.merge)
        self.assertEqual(result.expected_parents, (self.base, self.head))
        self.assertEqual(result.latest_parents, (self.base, self.head))
        self.assertEqual(result.expected_tree, self.tree)
        self.assertEqual(result.latest_tree, self.tree)
        self.assertEqual(result.acceptance, "exact-merge-sha")

    def test_accepts_regenerated_merge_sha_with_identical_parents_and_tree(self):
        regenerated = self.commit_tree(self.tree, [self.base, self.head], "regenerated merge")
        self.assertNotEqual(regenerated, self.merge)
        self.git("update-ref", "refs/pull/1/merge", regenerated)
        result = verifier.verify(self.env)
        self.assertEqual(result.expected_merge, self.merge)
        self.assertEqual(result.latest_merge, regenerated)
        self.assertEqual(result.acceptance, "same-parents-and-tree")

    def test_regenerated_sha_acceptance_log_records_comparison_and_reason(self):
        regenerated = self.commit_tree(self.tree, [self.base, self.head], "regenerated merge")
        self.git("update-ref", "refs/pull/1/merge", regenerated)
        output = StringIO()
        with patch.dict(os.environ, self.env, clear=False), redirect_stdout(output):
            self.assertEqual(verifier.main(), 0)
        text = output.getvalue()
        for value in (self.merge, regenerated, self.base, self.head, self.tree, "same-parents-and-tree"):
            self.assertIn(value, text)

    def test_rejects_advanced_base_ref(self):
        self.git("checkout", "-q", "--detach", self.base)
        newer_base = self.commit("new base")
        self.git("update-ref", "refs/heads/main", newer_base)
        self.git("checkout", "-q", "--detach", self.merge)
        with self.assertRaisesRegex(verifier.VerificationError, "base branch advanced"):
            verifier.verify(self.env)

    def test_rejects_advanced_pr_head_ref(self):
        self.git("checkout", "-q", "--detach", self.head)
        newer_head = self.commit("new PR head")
        self.git("update-ref", "refs/pull/1/head", newer_head)
        self.git("checkout", "-q", "--detach", self.merge)
        with self.assertRaisesRegex(verifier.VerificationError, "PR HEAD advanced"):
            verifier.verify(self.env)

    def test_rejects_latest_merge_with_same_parents_but_different_tree(self):
        changed_tree = self.new_tree()
        latest_merge = self.commit_tree(changed_tree, [self.base, self.head], "different tree")
        self.git("update-ref", "refs/pull/1/merge", latest_merge)
        with self.assertRaisesRegex(verifier.VerificationError, "merge trees differ"):
            verifier.verify(self.env)

    def test_rejects_latest_merge_with_wrong_parent_order(self):
        wrong_order = self.commit_tree(self.tree, [self.head, self.base], "wrong parent order")
        self.git("update-ref", "refs/pull/1/merge", wrong_order)
        with self.assertRaisesRegex(verifier.VerificationError, "latest GitHub merge parents"):
            verifier.verify(self.env)

    def test_rejects_latest_merge_with_one_or_three_parents(self):
        self.git("checkout", "-q", "--detach", self.base)
        third_parent = self.commit("third parent")
        self.git("checkout", "-q", "--detach", self.merge)
        one_parent = self.commit_tree(self.tree, [self.base], "one parent")
        three_parent = self.commit_tree(self.tree, [self.base, self.head, third_parent], "three parents")
        self.assertEqual(len(self.git("show", "-s", "--format=%P", one_parent).split()), 1)
        self.assertEqual(len(self.git("show", "-s", "--format=%P", three_parent).split()), 3)
        for merge, expected_count in ((one_parent, 1), (three_parent, 3)):
            with self.subTest(expected_count=expected_count):
                self.git("update-ref", "refs/pull/1/merge", merge)
                with self.assertRaisesRegex(verifier.VerificationError, "expected exactly two"):
                    verifier.verify(self.env)

    def test_rejects_checked_out_sha_mismatch(self):
        self.git("checkout", "-q", "--detach", self.head)
        with self.assertRaisesRegex(verifier.VerificationError, "checked-out SHA"):
            verifier.verify(self.env)

    def test_rejects_expected_merge_with_parent_mismatch(self):
        wrong = self.commit_tree(self.tree, [self.head, self.base], "wrong expected parents")
        self.git("checkout", "-q", "--detach", wrong)
        env = dict(self.env, SEREA_EXPECTED_MERGE_SHA=wrong)
        with self.assertRaisesRegex(verifier.VerificationError, "checked-out merge parents"):
            verifier.verify(env)

    def test_rejects_base_fetch_failure(self):
        self.git("remote", "set-url", "origin", str(self.repo / "missing-origin"))
        with self.assertRaisesRegex(verifier.VerificationError, "git fetch"):
            verifier.verify(self.env)

    def test_rejects_missing_pr_head_ref(self):
        self.git("update-ref", "-d", "refs/pull/1/head")
        with self.assertRaisesRegex(verifier.VerificationError, "git fetch"):
            verifier.verify(self.env)

    def test_rejects_missing_pr_merge_ref(self):
        self.git("update-ref", "-d", "refs/pull/1/merge")
        with self.assertRaisesRegex(verifier.VerificationError, "git fetch"):
            verifier.verify(self.env)

    def test_rejects_wrong_event_invalid_ref_pr_number_and_shas(self):
        cases = (
            ("SEREA_EVENT_NAME", "push"),
            ("SEREA_PR_BASE_REF", "../main"),
            ("SEREA_PR_NUMBER", "../1"),
            ("SEREA_PR_NUMBER", "0"),
            ("SEREA_PR_BASE_SHA", ""),
            ("SEREA_PR_BASE_SHA", "z" * 40),
            ("SEREA_PR_HEAD_SHA", "x"),
            ("SEREA_EXPECTED_MERGE_SHA", "1" * 40),
        )
        for key, value in cases:
            with self.subTest(key=key, value=value):
                env = dict(self.env, **{key: value})
                with self.assertRaises(verifier.VerificationError):
                    verifier.verify(env)

    def test_workflows_run_freshness_verifier_at_start_and_final_gate(self):
        root = Path(__file__).resolve().parents[1]
        for workflow in ("ci.yml", "full-ci.yml", "android.yml"):
            with self.subTest(workflow=workflow):
                source = (root / ".github" / "workflows" / workflow).read_text()
                self.assertIn("tools/verify_pr_merge_result.py", source)
                self.assertIn("SEREA_EXPECTED_MERGE_SHA: ${{ github.sha }}", source)

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
                self.assertIsNotNone(match)
                body = match.group("body")
                names = re.findall(r"^      - name: (.+)$", body, re.M)
                self.assertEqual(names[-1], "Final PR merge-result freshness verification")
                self.assertEqual(names[-2], last_check)
                final_step = body[body.rfind("      - name: Final PR merge-result freshness verification"):]
                self.assertIn("if: ${{ github.event_name == 'pull_request' }}", final_step)
                self.assertIn("run: python3 tools/verify_pr_merge_result.py", final_step)
                self.assertNotIn("continue-on-error", final_step)
                self.assertNotIn("continue-on-error:", body)


if __name__ == "__main__":
    unittest.main()
