#!/usr/bin/env python3
"""Offline tests for the public commit identity guard."""
from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
import check_commit_identity as guard


class CommitIdentityTests(unittest.TestCase):
    def test_approved_github_noreply_forms(self):
        self.assertTrue(guard.approved_email("noreply@github.com"))
        self.assertTrue(guard.approved_email("143304522+Yrika819@users.noreply.github.com"))

    def test_rejects_other_and_empty_local_part(self):
        self.assertFalse(guard.approved_email("user@example.com"))
        self.assertFalse(guard.approved_email("@users.noreply.github.com"))

    def test_reports_only_commit_sha_for_disallowed_author_or_committer(self):
        output = "\0".join((
            "good", "Codex", "noreply@github.com", "日本語 作者",
            "143304522+Yrika819@users.noreply.github.com",
            "bad-author", "Codex", "secret@example.com", "Codex", "noreply@github.com",
            "bad-committer", "Codex", "noreply@github.com", "Codex", "secret@example.com",
            "bad-name", "--get", "noreply@github.com", "Codex", "noreply@github.com",
        ))
        self.assertEqual(
            guard.invalid_commits(output),
            ["bad-author", "bad-committer", "bad-name"],
        )
        self.assertNotIn("secret@example.com", "\n".join(guard.invalid_commits(output)))

    def test_accepts_japanese_and_github_noreply_names(self):
        self.assertTrue(guard.valid_name("ユウタ ゴトウ"))
        self.assertTrue(guard.valid_name("Codex"))
        self.assertTrue(guard.approved_email("noreply@github.com"))

    def test_rejects_empty_option_like_or_control_names(self):
        for name in ("", "--get", "name\nforged", " name"):
            with self.subTest(name=name):
                self.assertFalse(guard.valid_name(name))

    def test_author_only_and_committer_only_corruption_are_separately_detected(self):
        bad_author = "\0".join(("author-only", "--get", "noreply@github.com", "Codex", "noreply@github.com"))
        bad_committer = "\0".join(("committer-only", "Codex", "noreply@github.com", "", "noreply@github.com"))
        self.assertEqual(guard.invalid_commits(bad_author), ["author-only"])
        self.assertEqual(guard.invalid_commits(bad_committer), ["committer-only"])


class CommitIdentityScopeTests(unittest.TestCase):
    def setUp(self):
        import tempfile
        self.temp = tempfile.TemporaryDirectory()
        self.repo = Path(self.temp.name)
        self.git("init", "-q")
        self.git("config", "user.name", "Valid Author")
        self.git("config", "user.email", "noreply@github.com")
        self.base = self.commit()

    def tearDown(self):
        self.temp.cleanup()

    def git(self, *args, env=None):
        import os
        import subprocess
        merged = os.environ.copy()
        if env:
            merged.update(env)
        return subprocess.run(["git", *args], cwd=self.repo, env=merged, check=True,
                              capture_output=True, text=True).stdout.strip()

    def commit(self, *, author_name="Valid Author", author_email="noreply@github.com",
               committer_name="Valid Committer", committer_email="noreply@github.com"):
        import os
        env = os.environ.copy()
        env.update({"GIT_AUTHOR_NAME": author_name, "GIT_AUTHOR_EMAIL": author_email,
                    "GIT_COMMITTER_NAME": committer_name, "GIT_COMMITTER_EMAIL": committer_email})
        self.git("commit", "--allow-empty", "-m", "fixture", env=env)
        return self.git("rev-parse", "HEAD")

    def check(self, event, **values):
        import os
        import subprocess
        env = os.environ.copy()
        env["SEREA_EVENT_NAME"] = event
        env.update(values)
        return subprocess.run([sys.executable, str(Path(__file__).resolve().parents[1] / "tools/check_commit_identity.py")],
                              cwd=self.repo, env=env, capture_output=True, text=True)

    def pr(self, base, head):
        return self.check("pull_request", SEREA_PR_BASE_SHA=base, SEREA_PR_HEAD_SHA=head)

    def test_pr_author_only_bad_is_red(self):
        head = self.commit(author_name="Bad", author_email="author@example.com")
        self.assertEqual(self.pr(self.base, head).returncode, 1)

    def test_pr_committer_only_bad_is_red(self):
        head = self.commit(committer_name="Bad", committer_email="committer@example.com")
        self.assertEqual(self.pr(self.base, head).returncode, 1)

    def test_good_final_head_does_not_hide_bad_intermediate_commit(self):
        bad = self.commit(author_email="bad@example.com")
        good = self.commit()
        result = self.pr(self.base, good)
        self.assertEqual(result.returncode, 1)
        self.assertIn(bad, result.stderr)

    def test_unrelated_bad_remote_branch_does_not_fail_pr(self):
        good = self.commit()
        bad = self.commit(author_email="unrelated@example.com")
        self.git("update-ref", "refs/remotes/origin/pr-38", bad)
        self.assertIn(bad, self.git("rev-list", "--all").splitlines())
        self.git("checkout", "-q", "-B", "pr", good)
        result = self.pr(self.base, good)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_pr_introducing_bad_commit_is_red(self):
        bad = self.commit(committer_email="bad@example.com")
        self.assertEqual(self.pr(self.base, bad).returncode, 1)

    def test_missing_or_unavailable_pr_sha_is_red(self):
        for base, head in (("", self.base), (self.base, ""), ("a" * 40, self.base)):
            with self.subTest(base=base, head=head):
                self.assertEqual(self.pr(base, head).returncode, 2)

    def test_stack_pr_checks_only_commits_after_selected_parent_tip(self):
        parent_tip = self.commit(author_email="parent-branch@example.com")
        head = self.commit()
        result = self.pr(parent_tip, head)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("1 introduced commit", result.stdout)

    def test_non_fast_forward_pr_fails_closed(self):
        head = self.commit()
        self.git("checkout", "-q", "--detach", self.base)
        alternate = self.commit(author_email="alternate@example.com")
        result = self.pr(alternate, head)
        self.assertEqual(result.returncode, 2, result.stderr)

    def test_push_uses_before_after_and_rejects_missing_range(self):
        good = self.commit()
        green = self.check("push", SEREA_PUSH_BEFORE_SHA=self.base, SEREA_PUSH_AFTER_SHA=good)
        self.assertEqual(green.returncode, 0, green.stderr)
        bad = self.commit(author_email="bad@example.com")
        result = self.check("push", SEREA_PUSH_BEFORE_SHA=self.base, SEREA_PUSH_AFTER_SHA=bad)
        self.assertEqual(result.returncode, 1)
        missing = self.check("push", SEREA_PUSH_BEFORE_SHA="0" * 40, SEREA_PUSH_AFTER_SHA=self.base)
        self.assertEqual(missing.returncode, 2)

    def test_workflow_dispatch_requires_explicit_matching_scope(self):
        head = self.commit()
        ok = self.check("workflow_dispatch", SEREA_DISPATCH_BASE_SHA=self.base,
                        SEREA_DISPATCH_HEAD_SHA=head, SEREA_DISPATCH_REF_SHA=head)
        self.assertEqual(ok.returncode, 0, ok.stderr)
        bad = self.check("workflow_dispatch", SEREA_DISPATCH_BASE_SHA=self.base,
                         SEREA_DISPATCH_HEAD_SHA=self.base, SEREA_DISPATCH_REF_SHA=head)
        self.assertEqual(bad.returncode, 2)
        missing_ref = self.check("workflow_dispatch", SEREA_DISPATCH_BASE_SHA=self.base,
                                 SEREA_DISPATCH_HEAD_SHA=head)
        self.assertEqual(missing_ref.returncode, 2)

    def test_unknown_event_fails_closed(self):
        self.assertEqual(self.check("schedule").returncode, 2)


if __name__ == "__main__":
    unittest.main()
