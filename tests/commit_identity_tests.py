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
        output = (
            "good\0noreply@github.com\0dev@users.noreply.github.com\n"
            "bad-author\0secret@example.com\0noreply@github.com\n"
            "bad-committer\0noreply@github.com\0secret@example.com\n"
        )
        self.assertEqual(guard.invalid_commits(output), ["bad-author", "bad-committer"])
        self.assertNotIn("secret@example.com", "\n".join(guard.invalid_commits(output)))


if __name__ == "__main__":
    unittest.main()
