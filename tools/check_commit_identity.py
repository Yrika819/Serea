#!/usr/bin/env python3
"""Validate identities on the commits explicitly introduced by one CI event."""
from __future__ import annotations

import os
import re
import subprocess
import sys
import argparse

_SHA = re.compile(r"[0-9a-fA-F]{40}")
_ZERO_SHA = "0" * 40


def approved_email(email: str) -> bool:
    return email == "noreply@github.com" or bool(
        re.fullmatch(r"[^@/\s]+@users\.noreply\.github\.com", email)
    )


def valid_name(name: str) -> bool:
    """Accept ordinary Unicode names while rejecting empty/option-like values."""
    if not name or name != name.strip() or name.startswith("--"):
        return False
    return not any(ord(char) < 32 or ord(char) == 127 for char in name)


def invalid_commits(records: str) -> list[str]:
    """Check NUL-separated five-field identity records (retained for callers/tests)."""
    invalid: list[str] = []
    fields_all = records.split("\0")
    for offset in range(0, len(fields_all), 5):
        fields = fields_all[offset : offset + 5]
        if len(fields) != 5 or not fields[0]:
            continue
        sha, author_name, author_email, committer_name, committer_email = fields
        if (
            not valid_name(author_name)
            or not valid_name(committer_name)
            or not approved_email(author_email)
            or not approved_email(committer_email)
        ):
            invalid.append(sha)
    return invalid


class ScopeError(RuntimeError):
    """The event did not provide a verifiable commit range."""


def git(args: list[str]) -> str:
    try:
        result = subprocess.run(
            ["git", *args], check=True, capture_output=True, text=True,
            encoding="utf-8", errors="strict",
        )
    except (OSError, UnicodeError, subprocess.CalledProcessError) as exc:
        raise ScopeError(f"git {args[0]} failed") from exc
    return result.stdout


def require_commit(sha: str, label: str) -> None:
    if not _SHA.fullmatch(sha):
        raise ScopeError(f"{label} SHA is missing or invalid")
    git(["cat-file", "-e", f"{sha}^{{commit}}"])


def introduced_commits(
    base: str, head: str, *, require_base_ancestor: bool = True
) -> list[str]:
    """Return commits reachable from head but not base, rejecting unrelated scopes."""
    require_commit(base, "base")
    require_commit(head, "head")
    merge_base = git(["merge-base", base, head]).strip()
    if not _SHA.fullmatch(merge_base):
        raise ScopeError("merge-base is missing or malformed")
    if require_base_ancestor and merge_base.lower() != base.lower():
        raise ScopeError("base is not an ancestor of head; refusing non-fast-forward scope")
    raw = git(["rev-list", "--reverse", f"{base}..{head}"])
    commits = raw.splitlines()
    if any(not _SHA.fullmatch(sha) for sha in commits):
        raise ScopeError("commit range contains a malformed object id")
    return commits


def commit_record(sha: str) -> str:
    record = git(["show", "-s", "--format=%H%x00%an%x00%ae%x00%cn%x00%ce", sha])
    return record[:-1] if record.endswith("\n") else record


def event_scope(environ: dict[str, str] | None = None) -> tuple[str, str, str]:
    env = os.environ if environ is None else environ
    event = env.get("SEREA_EVENT_NAME", "")
    if event == "pull_request":
        base, head = env.get("SEREA_PR_BASE_SHA", ""), env.get("SEREA_PR_HEAD_SHA", "")
    elif event == "push":
        base, head = env.get("SEREA_PUSH_BEFORE_SHA", ""), env.get("SEREA_PUSH_AFTER_SHA", "")
        if head == _ZERO_SHA:
            if env.get("SEREA_PUSH_DELETED", "").lower() == "true" and _SHA.fullmatch(base):
                raise ScopeError("branch deletion is explicitly unsupported by the identity guard")
            raise ScopeError("push after SHA is missing or zero")
        if base == _ZERO_SHA:
            if env.get("SEREA_PUSH_CREATED", "").lower() != "true":
                raise ScopeError("zero before SHA is accepted only for a confirmed branch creation")
            base = git(["rev-parse", "refs/remotes/origin/main"]).strip()
    elif event == "workflow_dispatch":
        base, head = env.get("SEREA_DISPATCH_BASE_SHA", ""), env.get("SEREA_DISPATCH_HEAD_SHA", "")
        selected_ref = env.get("SEREA_DISPATCH_REF_SHA", "")
        checkout_head = git(["rev-parse", "HEAD"]).strip()
        if (
            not _SHA.fullmatch(head)
            or not _SHA.fullmatch(selected_ref)
            or head.lower() != selected_ref.lower()
            or checkout_head.lower() != head.lower()
        ):
            raise ScopeError("dispatch head SHA must match both selected workflow ref and checked-out HEAD")
    else:
        raise ScopeError("unsupported or missing GitHub event")
    if event in {"pull_request", "push"}:
        checkout_head = git(["rev-parse", "HEAD"]).strip()
        if not _SHA.fullmatch(head) or checkout_head.lower() != head.lower():
            raise ScopeError(f"{event} head SHA must exactly match checked-out HEAD")
    return event, base, head


def pre_push_commits(input_text: str, remote: str) -> list[str]:
    """Validate every pre-push ref row and return commits introduced by each ref."""
    rows = [line.split() for line in input_text.splitlines() if line.strip()]
    if not rows:
        raise ScopeError("pre-push supplied no explicit target refs")
    head = git(["rev-parse", "HEAD"]).strip()
    commits: list[str] = []
    for row in rows:
        if len(row) != 4:
            raise ScopeError("pre-push ref input is malformed")
        local_ref, local_sha, remote_ref, remote_sha = row
        if local_sha == _ZERO_SHA:
            raise ScopeError("pre-push branch deletion has an all zeroes local SHA")
        if local_ref != "HEAD" and not local_ref.startswith("refs/heads/"):
            raise ScopeError("pre-push accepts only an explicit branch or HEAD local ref")
        if not remote_ref.startswith("refs/heads/"):
            raise ScopeError("pre-push accepts only explicit branch refs")
        for ref in (local_ref, remote_ref):
            if ref == "HEAD":
                continue
            git(["check-ref-format", ref])
        if not _SHA.fullmatch(local_sha):
            raise ScopeError("pre-push local SHA is missing or malformed")
        try:
            local_tip = head if local_ref == "HEAD" else git(["rev-parse", local_ref]).strip()
        except ScopeError as exc:
            raise ScopeError("pre-push local ref is unavailable") from exc
        if local_tip.lower() != local_sha.lower():
            raise ScopeError("pre-push local ref does not match its supplied SHA")
        if remote_sha == _ZERO_SHA:
            # git_preflight also requires new branches to descend from current origin/main.
            tracking_main = f"refs/remotes/{remote}/main"
            git(["check-ref-format", tracking_main])
            base = git(["rev-parse", tracking_main]).strip()
        else:
            tracking_ref = f"refs/remotes/{remote}/" + remote_ref.removeprefix("refs/heads/")
            git(["check-ref-format", tracking_ref])
            tracking_sha = git(["rev-parse", tracking_ref]).strip()
            if tracking_sha.lower() != remote_sha.lower():
                raise ScopeError("remote target changed since last fetch; refusing stale push")
            base = remote_sha
        try:
            commits.extend(introduced_commits(base, local_sha))
        except ScopeError as exc:
            if "not an ancestor" in str(exc):
                raise ScopeError("push would not be a fast-forward; refusing non-fast-forward push") from exc
            raise
    return list(dict.fromkeys(commits))


def main() -> int:
    try:
        parser = argparse.ArgumentParser(description=__doc__)
        parser.add_argument("--pre-push", action="store_true")
        parser.add_argument("--remote", default="origin")
        args = parser.parse_args()
        if args.pre_push:
            event = "pre-push"
            commits = pre_push_commits(sys.stdin.read(), args.remote)
        else:
            event, base, head = event_scope()
            commits = introduced_commits(
                base, head, require_base_ancestor=event != "pull_request"
            )
        bad: list[str] = []
        for sha in commits:
            record = commit_record(sha)
            fields = record.split("\0")
            if len(fields) != 5 or fields[0].lower() != sha.lower():
                raise ScopeError("commit identity record is malformed")
            if invalid_commits(record):
                bad.append(sha)
    except ScopeError as exc:
        print(f"commit identity check could not establish a safe scope: {exc}", file=sys.stderr)
        return 2
    if bad:
        print(f"commit identity check failed for {event}; malformed author/committer on:", file=sys.stderr)
        for sha in bad:
            print(sha, file=sys.stderr)
        return 1
    print(f"commit identity check passed ({event}: {len(commits)} introduced commit(s))")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
