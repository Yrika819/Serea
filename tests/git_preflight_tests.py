#!/usr/bin/env python3
"""Isolated Git-config and policy regression fixtures; no network pushes."""
from __future__ import annotations

import contextlib
import io
import os
import shutil
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
        project = Path(__file__).resolve().parents[1]
        (self.root / "tools").mkdir()
        for filename in ("git_preflight.py", "check_commit_identity.py", "run_python.sh"):
            shutil.copy2(project / "tools" / filename, self.root / "tools" / filename)
        shutil.copytree(project / ".githooks", self.root / ".githooks")
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
        self.run_git("config", "serea.identityProfile", "codex")
        self.run_git("config", "push.default", "nothing")
        self.run_git("config", "commit.gpgsign", "false")
        (self.root / "seed.txt").write_text("seed\n", encoding="utf-8")
        self.run_git("add", "seed.txt", "tools", ".githooks")
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

    def hook_env(self):
        real_git = shutil.which("git")
        if os.name == "nt":
            real_git = Path(real_git).as_posix()
        shim_dir = Path(self.temp.name) / "shim"
        shim_dir.mkdir(exist_ok=True)
        shim = shim_dir / "git"
        shim.write_text(
            "#!/usr/bin/env python\n"
            "import os, subprocess, sys\n"
            "real = os.environ['REAL_GIT']\n"
            "args = sys.argv[1:]\n"
            "if args and args[0] == 'ls-remote':\n"
            "    args = ['ls-remote', *args[1:]]\n"
            "    args = [os.environ['FIXTURE_BARE'] if x == 'origin' else x for x in args]\n"
            "result = subprocess.run([real, *args], check=False)\n"
            "raise SystemExit(result.returncode)\n",
            encoding="utf-8",
        )
        shim.chmod(0o755)
        return {
            **self.env,
            "PATH": f"{shim_dir}{os.pathsep}{os.environ['PATH']}",
            "REAL_GIT": real_git,
        }

    def make_hook_fixture(self):
        bare = Path(self.temp.name) / "remote.git"
        subprocess.run([shutil.which("git"), "init", "--bare", str(bare)], check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.run_git("remote", "set-url", "origin", "https://github.com/Yrika819/Serea.git")
        self.run_git("push", str(bare), "HEAD:refs/heads/main")
        self.run_git("update-ref", "refs/remotes/origin/main", self.head)
        self.run_git("switch", "-c", "topic")
        (self.root / "topic.txt").write_text("topic\n", encoding="utf-8")
        self.run_git("add", "topic.txt")
        self.run_git("commit", "-m", "topic")
        head = self.run_git("rev-parse", "HEAD")
        env = self.hook_env()
        env["FIXTURE_BARE"] = str(bare)
        return bare, head, env

    def run_prepush_hook(self, env, rows):
        return subprocess.run(
            ["sh", str(Path(__file__).resolve().parents[1] / ".githooks" / "pre-push"),
             "origin", "https://github.com/Yrika819/Serea.git"],
            cwd=self.root, env=env, input=rows, text=True, capture_output=True,
        )

    @staticmethod
    def push_row(head, target="topic", remote_sha="0" * 40):
        return f"refs/heads/{target} {head} refs/heads/{target} {remote_sha}\n"

    def test_normal_codex_identity_and_japanese_history(self):
        self.check_identity()
        rows = "\0".join(("sha", "ユウタ ゴトウ", "noreply@github.com", "Codex", "noreply@github.com"))
        self.assertEqual(identity.invalid_commits(rows), [])

    def test_explicit_personal_profile_accepts_matching_noreply_identity(self):
        self.run_git("config", "serea.identityProfile", "personal")
        self.run_git("config", "serea.approvedName", "山田 花子")
        self.run_git("config", "user.name", "山田 花子")
        self.run_git("config", "user.email", "123456+hanako@users.noreply.github.com")
        self.env.update({
            "GIT_AUTHOR_NAME": "山田 花子",
            "GIT_COMMITTER_NAME": "山田 花子",
            "GIT_AUTHOR_EMAIL": "123456+hanako@users.noreply.github.com",
            "GIT_COMMITTER_EMAIL": "123456+hanako@users.noreply.github.com",
        })
        self.check_identity()

    def test_personal_profile_is_explicit_and_exact(self):
        self.run_git("config", "serea.identityProfile", "personal")
        self.run_git("config", "serea.approvedName", "山田 花子")
        self.run_git("config", "user.name", "Another User")
        self.env["GIT_AUTHOR_NAME"] = "Another User"
        self.env["GIT_COMMITTER_NAME"] = "Another User"
        with self.assertRaisesRegex(preflight.PreflightError, "explicitly approved personal name"):
            self.check_identity()

    def test_codex_profile_remains_strict_and_personal_email_is_rejected(self):
        self.run_git("config", "user.name", "Local Developer")
        with self.assertRaisesRegex(preflight.PreflightError, "explicit codex profile"):
            self.check_identity()

        self.run_git("config", "user.name", "Codex")
        self.run_git("config", "user.email", "developer@example.com")
        self.env["GIT_AUTHOR_NAME"] = "Codex"
        self.env["GIT_COMMITTER_NAME"] = "Codex"
        self.env["GIT_AUTHOR_EMAIL"] = "developer@example.com"
        self.env["GIT_COMMITTER_EMAIL"] = "developer@example.com"
        with self.assertRaisesRegex(preflight.PreflightError, "noreply"):
            self.check_identity()

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

    def test_actual_precommit_hook_accepts_clean_codex_fixture(self):
        hook = Path(__file__).resolve().parents[1] / ".githooks" / "pre-commit"
        result = subprocess.run(["sh", str(hook)], cwd=self.root, env=self.hook_env(),
                                text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_actual_prepush_hook_accepts_new_branch_and_fast_forward(self):
        bare, head, env = self.make_hook_fixture()
        new_branch = self.run_prepush_hook(env, self.push_row(head))
        self.assertEqual(new_branch.returncode, 0, new_branch.stdout + new_branch.stderr)

        base = self.head
        subprocess.run([shutil.which("git"), "--git-dir", str(bare), "update-ref",
                        "refs/heads/topic", base], check=True)
        self.run_git("update-ref", "refs/remotes/origin/topic", base)
        self.run_git("branch", "--set-upstream-to=origin/topic", "topic")
        fast_forward = self.run_prepush_hook(env, self.push_row(head, remote_sha=base))
        self.assertEqual(fast_forward.returncode, 0, fast_forward.stdout + fast_forward.stderr)

    def test_actual_prepush_hook_rejects_all_zero_deletion_sha(self):
        _, head, env = self.make_hook_fixture()
        self.run_git("update-ref", "refs/remotes/origin/topic", self.head)
        deletion = f"(delete) {'0' * 40} refs/heads/topic {head}\n"
        result = self.run_prepush_hook(env, deletion)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("all zeroes", result.stderr)

    def test_actual_prepush_hook_rejects_non_fast_forward(self):
        bare, head, env = self.make_hook_fixture()
        diverged = self.run_git("commit-tree", self.run_git("rev-parse", "HEAD^{tree}"),
                                "-p", self.head, "-m", "divergent")
        subprocess.run([shutil.which("git"), "-C", str(self.root), "push", str(bare),
                        f"{diverged}:refs/heads/topic"], check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        subprocess.run([shutil.which("git"), "--git-dir", str(bare), "update-ref",
                        "refs/heads/topic", diverged], check=True)
        self.run_git("update-ref", "refs/remotes/origin/topic", diverged)
        self.run_git("branch", "--set-upstream-to=origin/topic", "topic")
        result = self.run_prepush_hook(env, self.push_row(head, remote_sha=diverged))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not be a fast-forward", result.stderr)

    def test_actual_prepush_hook_rejects_stale_remote_head(self):
        bare, head, env = self.make_hook_fixture()
        old = self.head
        divergent = self.run_git("commit-tree", self.run_git("rev-parse", "HEAD^{tree}"),
                                "-p", old, "-m", "remote move")
        subprocess.run([shutil.which("git"), "-C", str(self.root), "push", str(bare),
                        f"{divergent}:refs/heads/topic"], check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        subprocess.run([shutil.which("git"), "--git-dir", str(bare), "update-ref",
                        "refs/heads/topic", divergent], check=True)
        self.run_git("update-ref", "refs/remotes/origin/topic", old)
        self.run_git("branch", "--set-upstream-to=origin/topic", "topic")
        result = self.run_prepush_hook(env, self.push_row(head, remote_sha=divergent))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("changed since last fetch", result.stderr)

    def test_actual_prepush_hook_rejects_multiple_refs_including_main(self):
        _, head, env = self.make_hook_fixture()
        rows = self.push_row(head, "main") + self.push_row(head, "topic")
        result = self.run_prepush_hook(env, rows)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("non-main branch", result.stderr)

    def test_actual_prepush_hook_rejects_multiple_branch_targets(self):
        _, head, env = self.make_hook_fixture()
        rows = self.push_row(head, "topic") + self.push_row(head, "other")
        result = self.run_prepush_hook(env, rows)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("does not match the current branch", result.stderr)

    def test_actual_prepush_hook_rejects_main_push(self):
        _, head, env = self.make_hook_fixture()
        result = self.run_prepush_hook(env, self.push_row(head, "main"))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("non-main branch", result.stderr)

    def test_actual_prepush_hook_rejects_commit_author_override_before_remote_check(self):
        self.make_hook_fixture()
        subprocess.run(
            [shutil.which("git"), "commit", "--allow-empty", "--author=--get <noreply@github.com>",
             "-m", "bad author"], cwd=self.root, env=self.hook_env(), check=True,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        result = self.run_prepush_hook(self.hook_env(), self.push_row(self.run_git("rev-parse", "HEAD")))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("commit identity check failed", result.stderr)


if __name__ == "__main__":
    unittest.main()
