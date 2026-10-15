#!/usr/bin/env python3
"""Isolated Git-config and policy regression fixtures; no network pushes."""
from __future__ import annotations

import contextlib
import io
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
import git_preflight as preflight
import check_commit_identity as identity


class GitFixture(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="serea-git-preflight-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "repo"
        self.root.mkdir()
        self.env = {
            **os.environ,
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_AUTHOR_NAME": "Codex",
            "GIT_AUTHOR_EMAIL": "noreply@github.com",
            "GIT_COMMITTER_NAME": "Codex",
            "GIT_COMMITTER_EMAIL": "noreply@github.com",
        }
        for name in ("GIT_CONFIG_COUNT", "GIT_CONFIG_PARAMETERS", "EMAIL"):
            self.env.pop(name, None)
        for name in ("GIT_AUTHOR_IDENT", "GIT_COMMITTER_IDENT"):
            self.env.pop(name, None)
        self.run_git("init", "-b", "main")
        self.run_git("config", "user.name", "Codex")
        self.run_git("config", "user.email", "noreply@github.com")
        self.run_git("config", "user.useConfigOnly", "true")
        self.run_git("config", "push.default", "nothing")
        self.run_git("config", "commit.gpgsign", "false")
        (self.root / "seed.txt").write_text("seed\n", encoding="utf-8")
        self.run_git("add", "seed.txt")
        self.run_git("commit", "-m", "seed")
        self.run_git("remote", "add", "origin", "https://github.com/Yrika819/Serea.git")
        self.head = self.run_git("rev-parse", "HEAD")
        self.run_git("update-ref", "refs/remotes/origin/main", self.head)

    def run_git(self, *args):
        return subprocess.run(
            ["git", *args], cwd=self.root, env=self.env, check=True,
            text=True, capture_output=True,
        ).stdout.strip()

    @contextlib.contextmanager
    def cwd(self):
        old = Path.cwd()
        os.chdir(self.root)
        try:
            yield
        finally:
            os.chdir(old)

    def check_identity(self):
        with self.cwd(), mock.patch.dict(os.environ, self.env, clear=True), contextlib.redirect_stdout(io.StringIO()):
            preflight.check_identity()

    def test_normal_codex_identity_and_japanese_history(self):
        self.check_identity()
        rows = "\0".join(("sha", "ユウタ ゴトウ", "noreply@github.com", "Codex", "noreply@github.com"))
        self.assertEqual(identity.invalid_commits(rows), [])

    def test_name_option_value_rejected(self):
        self.run_git("config", "user.name", "--get")
        with self.assertRaisesRegex(preflight.PreflightError, "user.name"):
            self.check_identity()

    def test_email_option_value_rejected(self):
        self.run_git("config", "user.email", "--get")
        with self.assertRaisesRegex(preflight.PreflightError, "user.email"):
            self.check_identity()

    def test_empty_identity_rejected(self):
        self.run_git("config", "user.email", "")
        with self.assertRaises(preflight.PreflightError):
            self.check_identity()

    def test_author_environment_override_rejected_without_echoing_value(self):
        self.env["GIT_AUTHOR_EMAIL"] = "private@example.invalid"
        with self.assertRaisesRegex(preflight.PreflightError, "GIT_AUTHOR_EMAIL") as caught:
            self.check_identity()
        self.assertNotIn("private@example.invalid", str(caught.exception))

    def test_committer_environment_override_rejected(self):
        self.env["GIT_COMMITTER_NAME"] = "--get"
        with self.assertRaisesRegex(preflight.PreflightError, "GIT_COMMITTER_NAME"):
            self.check_identity()

    def test_config_environment_override_has_precedence_and_is_rejected(self):
        self.env.update({"GIT_CONFIG_COUNT": "1", "GIT_CONFIG_KEY_0": "user.name", "GIT_CONFIG_VALUE_0": "--get"})
        with self.assertRaisesRegex(preflight.PreflightError, "config environment"):
            self.check_identity()

    def test_local_config_wins_over_global_but_scope_is_visible(self):
        global_file = Path(self.temp.name) / "global-config"
        global_file.write_text("[user]\nname = --get\nemail = bad@example.invalid\n", encoding="utf-8")
        self.env["GIT_CONFIG_GLOBAL"] = str(global_file)
        self.check_identity()
        with self.cwd(), mock.patch.dict(os.environ, self.env, clear=True):
            self.assertIn("local", " ".join(preflight.config_sources("user.name")))

    def test_bad_remote_identification_rejected(self):
        self.assertIsNone(preflight.canonical_repo("https://attacker.invalid/Yrika819/Serea.git"))
        self.assertIsNone(preflight.canonical_repo("https://token@github.com/Yrika819/Serea.git"))

    def test_main_push_rejected_before_any_network_access(self):
        with self.cwd(), mock.patch.dict(os.environ, self.env, clear=True):
            with self.assertRaisesRegex(preflight.PreflightError, "non-main"):
                preflight.check_push("origin", "main", self.head, "main", "main", False)

    def test_wrong_push_branch_rejected_before_network_access(self):
        with self.assertRaisesRegex(preflight.PreflightError, "current branch"):
            preflight.check_push("origin", "other", self.head, "topic", "main", False)

    def test_actual_remote_rewrite_rejected(self):
        self.run_git("remote", "set-url", "origin", "https://evil.example/repo.git")
        with self.cwd(), mock.patch.dict(os.environ, self.env, clear=True):
            with self.assertRaisesRegex(preflight.PreflightError, "does not identify"):
                preflight.check_repository("origin", preflight.EXPECTED_REPOSITORY)

    def test_stale_remote_base_is_rejected(self):
        self.run_git("switch", "-c", "chore/git-safety-hardening")
        with self.cwd(), mock.patch.dict(os.environ, self.env, clear=True):
            with mock.patch.object(preflight, "ls_remote", side_effect=[None, "f" * 40]):
                with self.assertRaisesRegex(preflight.PreflightError, "stale"):
                    preflight.check_push("origin", "chore/git-safety-hardening", self.head,
                                         "chore/git-safety-hardening", "main", False)

    def test_stale_remote_target_is_rejected(self):
        self.run_git("switch", "-c", "topic")
        self.run_git("update-ref", "refs/remotes/origin/topic", self.head)
        self.run_git("branch", "--set-upstream-to=origin/topic")
        with self.cwd(), mock.patch.dict(os.environ, self.env, clear=True):
            with mock.patch.object(preflight, "ls_remote", return_value="f" * 40):
                with self.assertRaisesRegex(preflight.PreflightError, "changed since last fetch"):
                    preflight.check_push("origin", "topic", self.head, "topic", "main", False)

    def test_force_push_rejected(self):
        with self.assertRaisesRegex(preflight.PreflightError, "Force push"):
            preflight.check_push("origin", "topic", self.head, "topic", "main", True)

    def test_old_malformed_commit_is_rejected(self):
        records = "\0".join(("oldsha", "Codex", "noreply@github.com", "--get", "noreply@github.com"))
        self.assertEqual(identity.invalid_commits(records), ["oldsha"])


if __name__ == "__main__":
    unittest.main()
