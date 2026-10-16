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


if __name__ == "__main__":
    unittest.main()
