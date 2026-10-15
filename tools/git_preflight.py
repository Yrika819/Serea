#!/usr/bin/env python3
"""Fail-closed preflight for commits and explicit branch pushes.

Diagnostics intentionally redact all identity emails and remote URL values.
"""
from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from pathlib import Path
from urllib.parse import urlparse

EXPECTED_REPOSITORY = "Yrika819/Serea"
EXPECTED_NAME = "Codex"


class PreflightError(RuntimeError):
    pass


def git(args: list[str], *, input_text: str | None = None) -> str:
    try:
        result = subprocess.run(
            ["git", *args], input=input_text, text=True, encoding="utf-8",
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False,
        )
    except OSError as exc:
        raise PreflightError(f"Git could not be executed ({exc.__class__.__name__}).") from None
    if result.returncode:
        raise PreflightError(f"Git command failed: git {args[0]} (exit {result.returncode}).")
    return result.stdout.strip()


def _origin_label(value: str) -> str:
    if value.startswith("file:"):
        path = value[5:]
        return Path(path).name or "config"
    return "included-config"


def config_sources(key: str) -> list[str]:
    try:
        raw = subprocess.run(
            ["git", "config", "--show-origin", "--show-scope", "--get-all", key],
            text=True, encoding="utf-8", stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL, check=False,
        ).stdout
    except OSError:
        return []
    labels = []
    for line in raw.splitlines():
        parts = line.split("\t", 2)
        if len(parts) >= 2:
            labels.append(f"{parts[0]}:{_origin_label(parts[1])}")
    return labels


def has_local_source(key: str) -> bool:
    return any(label.startswith(("local:", "worktree:")) for label in config_sources(key))


def canonical_repo(url: str) -> str | None:
    """Return owner/repo only for credential-free GitHub URLs."""
    if re.fullmatch(r"git@github\.com:[^?#]+", url):
        path = url.split(":", 1)[1]
    else:
        parsed = urlparse(url)
        if (
            parsed.scheme not in {"https", "ssh"}
            or parsed.hostname != "github.com"
            or parsed.username not in {None, "git"}
            or parsed.password is not None
            or parsed.port is not None
            or parsed.query
            or parsed.fragment
        ):
            return None
        path = parsed.path.lstrip("/")
    path = path.removesuffix(".git").strip("/")
    if path.count("/") != 1:
        return None
    return path


def approved_email(email: str) -> bool:
    return email == "noreply@github.com" or bool(
        re.fullmatch(r"[^@/\s]+@users\.noreply\.github\.com", email)
    )


def valid_name(name: str) -> bool:
    return bool(name) and name == name.strip() and not name.startswith("--") and not any(
        ord(char) < 32 or ord(char) == 127 for char in name
    )


def parse_ident(ident: str) -> tuple[str, str]:
    match = re.fullmatch(r"(.+?) <([^<>\s]+)> [0-9]+ [+-][0-9]{4}", ident)
    if not match:
        raise PreflightError("Git produced an empty or malformed effective identity.")
    return match.group(1), match.group(2)


def check_identity() -> None:
    try:
        profile = git(["config", "--get", "serea.identityProfile"])
    except PreflightError:
        raise PreflightError("Repository-local serea.identityProfile must explicitly select codex or personal.") from None
    if not has_local_source("serea.identityProfile") or profile not in {"codex", "personal"}:
        raise PreflightError("Repository-local serea.identityProfile must explicitly select codex or personal.")
    expected_name = EXPECTED_NAME
    if profile == "personal":
        try:
            expected_name = git(["config", "--get", "serea.approvedName"])
        except PreflightError:
            raise PreflightError("Personal profile requires repository-local serea.approvedName.") from None
        if not has_local_source("serea.approvedName") or not valid_name(expected_name):
            raise PreflightError("Personal profile requires a valid repository-local serea.approvedName.")
    for var in ("GIT_AUTHOR_NAME", "GIT_COMMITTER_NAME"):
        value = os.environ.get(var)
        if value is not None and value != expected_name:
            raise PreflightError(f"{var} overrides the explicitly approved {profile} name.")
    for var in ("GIT_AUTHOR_EMAIL", "GIT_COMMITTER_EMAIL", "EMAIL"):
        value = os.environ.get(var)
        if value is not None and not approved_email(value):
            raise PreflightError(f"{var} overrides the approved noreply email.")
    if os.environ.get("GIT_CONFIG_PARAMETERS") or os.environ.get("GIT_CONFIG_COUNT", "0") != "0":
        raise PreflightError("Git config environment overrides are present; inspect GIT_CONFIG_* safely.")
    config_env = sorted(key for key in os.environ if key.startswith("GIT_CONFIG_"))
    for key in config_env:
        if key in {"GIT_CONFIG_GLOBAL", "GIT_CONFIG_SYSTEM", "GIT_CONFIG_NOSYSTEM"}:
            print(f"INFO {key}=<set; value redacted>")
        elif key not in {"GIT_CONFIG_PARAMETERS", "GIT_CONFIG_COUNT"}:
            raise PreflightError(f"Unsupported {key} environment override is present.")

    for key in ("user.name", "user.email", "user.useConfigOnly"):
        try:
            value = git(["config", "--get", key])
        except PreflightError:
            raise PreflightError(f"Required Git setting {key} is missing or empty.") from None
        if value in {"--get", "--add", "--unset", "--replace-all"}:
            raise PreflightError(f"Git setting {key} contains an option-like invalid value.")
        if not has_local_source(key):
            raise PreflightError(f"Git setting {key} must be set in this clone's repository-local config.")
        if key == "user.name" and (not valid_name(value) or value != expected_name):
            raise PreflightError(f"Effective user.name does not match the explicit {profile} profile.")
        if key == "user.email" and not approved_email(value):
            raise PreflightError("Effective user.email is not an approved GitHub noreply address.")
        if key == "user.useConfigOnly" and value.lower() != "true":
            raise PreflightError("user.useConfigOnly must be true in this dedicated development clone.")

    for var in ("GIT_AUTHOR_IDENT", "GIT_COMMITTER_IDENT"):
        name, email = parse_ident(git(["var", var]))
        if name != expected_name or not approved_email(email):
            raise PreflightError(f"Effective {var} does not match the approved identity.")
        print(f"PASS {var}: profile={profile}, name={name}, email=approved-noreply")
    for key in ("user.name", "user.email", "user.useConfigOnly"):
        sources = ",".join(config_sources(key)) or "config source unavailable"
        print(f"PASS {key}: scope/source={sources}; effective-value=validated")


def check_repository(remote: str, expected_repository: str) -> tuple[str, str, str]:
    top = Path(git(["rev-parse", "--show-toplevel"])).resolve()
    if not (top / ".git").exists() and not (top / ".git").is_file():
        raise PreflightError("Current directory is not a Git worktree root.")
    branch = git(["branch", "--show-current"])
    if not branch:
        raise PreflightError("Detached HEAD is not allowed for write preflight.")
    head = git(["rev-parse", "HEAD"])
    fetch_url = git(["remote", "get-url", remote])
    push_url = git(["remote", "get-url", "--push", remote])
    if canonical_repo(fetch_url) != expected_repository or canonical_repo(push_url) != expected_repository:
        raise PreflightError(f"Remote {remote!r} does not identify the expected repository.")
    print(f"PASS repository: {expected_repository}; root={top}")
    print(f"PASS remote {remote}: fetch/push repository match; URLs redacted")
    print(f"PASS HEAD={head}; branch={branch}; detached=false")
    return str(top), branch, head


def check_worktree(mode: str) -> None:
    status = git(["status", "--porcelain=v1", "--untracked-files=all"])
    lines = status.splitlines()
    unstaged_or_untracked = [line for line in lines if line.startswith("??") or (len(line) > 1 and line[1] != " ")]
    if unstaged_or_untracked:
        raise PreflightError("Worktree has unstaged or untracked changes; review them before writing.")
    if mode == "push" and lines:
        raise PreflightError("Push preflight requires a completely clean worktree.")
    print(f"PASS worktree: {'clean' if not lines else 'staged changes only'}")


def ls_remote(remote: str, branch: str) -> str | None:
    try:
        result = subprocess.run(
            ["git", "ls-remote", "--heads", remote, f"refs/heads/{branch}"],
            text=True, encoding="utf-8", stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL, check=False,
        )
    except OSError:
        raise PreflightError("Could not run remote HEAD freshness check.") from None
    if result.returncode:
        raise PreflightError("Remote HEAD freshness check failed; refusing to push.")
    if not result.stdout.strip():
        return None
    fields = result.stdout.split()
    if len(fields) != 2 or fields[1] != f"refs/heads/{branch}":
        raise PreflightError("Remote HEAD response was malformed.")
    return fields[0]


def check_push(remote: str, target: str, head: str, branch: str, base_branch: str, force: bool) -> None:
    if force:
        raise PreflightError("Force push is prohibited.")
    if target.startswith("refs/heads/"):
        target = target.removeprefix("refs/heads/")
    if not target or target.startswith(("-", "refs/")) or target == "main":
        raise PreflightError("Push target must be an explicit non-main branch name.")
    if subprocess.run(["git", "check-ref-format", "--branch", target], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode:
        raise PreflightError("Push target is not a valid Git branch name.")
    if subprocess.run(["git", "check-ref-format", "--branch", base_branch], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode:
        raise PreflightError("Base branch is not a valid Git branch name.")
    if target != branch:
        raise PreflightError("Push target does not match the current branch.")
    live_target = ls_remote(remote, target)
    upstream_result = subprocess.run(
        ["git", "rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"],
        text=True, encoding="utf-8", stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL, check=False,
    )
    upstream = upstream_result.stdout.strip() if upstream_result.returncode == 0 else ""
    if live_target is not None:
        if upstream != f"{remote}/{target}":
            raise PreflightError("Remote target exists but is not the configured upstream; refusing push.")
        tracking = git(["rev-parse", f"refs/remotes/{remote}/{target}"])
        if live_target != tracking:
            raise PreflightError("Remote target HEAD changed since last fetch; fetch and review before pushing.")
        if subprocess.run(["git", "merge-base", "--is-ancestor", live_target, head], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode:
            raise PreflightError("Push would not be a fast-forward; force push is prohibited.")
    else:
        live_base = ls_remote(remote, base_branch)
        try:
            tracking_base = git(["rev-parse", f"refs/remotes/{remote}/{base_branch}"])
        except PreflightError:
            raise PreflightError("Base branch is not fetched locally; fetch before pushing.") from None
        if live_base != tracking_base:
            raise PreflightError("Base branch HEAD is stale; fetch and rebase/merge by normal workflow.")
        if subprocess.run(["git", "merge-base", "--is-ancestor", tracking_base, head], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode:
            raise PreflightError("Current branch is not based on the current remote base branch.")
    print(f"PASS push target: {remote}/refs/heads/{target}; remote HEAD freshly checked")
    print("PASS push history: fast-forward or new branch from current base")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("commit", "push"), required=True)
    parser.add_argument("--remote", default="origin")
    parser.add_argument("--target", help="explicit target branch for push preflight")
    parser.add_argument("--base-branch", default="main")
    parser.add_argument("--expected-repository", default=EXPECTED_REPOSITORY)
    parser.add_argument("--force", action="store_true", help="rejected; documents forbidden push mode")
    parser.add_argument("--force-with-lease", action="store_true", help="rejected; force push is prohibited")
    parser.add_argument("--hook-url", help=argparse.SUPPRESS)
    parser.add_argument("--hook", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    try:
        if args.expected_repository != EXPECTED_REPOSITORY:
            raise PreflightError("This checker is bound to Yrika819/Serea.")
        _, branch, head = check_repository(args.remote, args.expected_repository)
        if args.hook_url is not None and canonical_repo(args.hook_url) != args.expected_repository:
            raise PreflightError("pre-push hook URL does not match the configured repository.")
        check_identity()
        check_worktree(args.mode)
        default = git(["config", "--get", "push.default"])
        if default != "nothing":
            raise PreflightError("push.default must be nothing in the dedicated development clone.")
        if not has_local_source("push.default"):
            raise PreflightError("push.default must be set in this clone's repository-local config.")
        print(f"PASS push.default={default}")
        if args.mode == "push":
            if args.force_with_lease:
                raise PreflightError("Force push is prohibited.")
            if args.hook:
                rows = [line.split() for line in sys.stdin if line.strip()]
                if not rows:
                    raise PreflightError("pre-push supplied no explicit target refs.")
                for row in rows:
                    if len(row) != 4:
                        raise PreflightError("pre-push ref input is malformed.")
                    _, local_sha, remote_ref, _ = row
                    if re.fullmatch(r"0+", local_sha):
                        raise PreflightError("Branch deletion pushes are prohibited (local_sha is all zeroes).")
                    if not remote_ref.startswith("refs/heads/"):
                        raise PreflightError("Only explicit branch pushes are allowed.")
                    target = remote_ref.removeprefix("refs/heads/")
                    if local_sha != head:
                        raise PreflightError("Pushed commit differs from the inspected HEAD.")
                    check_push(args.remote, target, head, branch, args.base_branch, args.force)
            else:
                if not args.target:
                    raise PreflightError("Push preflight requires --target BRANCH.")
                check_push(args.remote, args.target, head, branch, args.base_branch, args.force)
        else:
            print("PASS commit target: current branch; no remote mutation requested")
        return 0
    except PreflightError as exc:
        print(f"FAIL preflight: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
