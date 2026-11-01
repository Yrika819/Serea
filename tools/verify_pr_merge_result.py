#!/usr/bin/env python3
"""Fail closed unless the checkout is GitHub's current PR merge result."""
from __future__ import annotations

import os
import re
import subprocess
import sys

SHA = re.compile(r"[0-9a-fA-F]{40}\Z")
PR_NUMBER = re.compile(r"[1-9][0-9]*\Z")


class VerificationError(RuntimeError):
    pass


def git(*args: str) -> str:
    try:
        result = subprocess.run(
            ["git", *args], check=True, capture_output=True, text=True,
            encoding="utf-8", errors="strict",
        )
    except (OSError, UnicodeError, subprocess.CalledProcessError) as exc:
        raise VerificationError(f"git {args[0]} failed") from exc
    return result.stdout.strip()


def verify(env: dict[str, str] | None = None) -> tuple[str, str, str]:
    values = os.environ if env is None else env
    if values.get("SEREA_EVENT_NAME") != "pull_request":
        raise VerificationError("merge-result verification requires pull_request event")

    base_ref = values.get("SEREA_PR_BASE_REF", "")
    if not base_ref:
        raise VerificationError("base branch ref is missing or invalid")
    git("check-ref-format", f"refs/remotes/origin/{base_ref}")
    pr_number = values.get("SEREA_PR_NUMBER", "")
    if not PR_NUMBER.fullmatch(pr_number):
        raise VerificationError("pull request number is missing or invalid")
    merge_ref = f"refs/pull/{pr_number}/merge"
    git("check-ref-format", merge_ref)
    base = values.get("SEREA_PR_BASE_SHA", "")
    head = values.get("SEREA_PR_HEAD_SHA", "")
    expected_merge = values.get("SEREA_EXPECTED_MERGE_SHA", "")
    actual = git("rev-parse", "--verify", "HEAD^{commit}")
    for label, sha in (("base", base), ("head", head), ("expected merge", expected_merge)):
        if not SHA.fullmatch(sha):
            raise VerificationError(f"{label} SHA is missing or invalid")
        git("cat-file", "-e", f"{sha}^{{commit}}")
    if not SHA.fullmatch(actual) or actual.lower() != expected_merge.lower():
        raise VerificationError("checked-out SHA does not match expected merge SHA")

    tracking_ref = f"refs/remotes/origin/{base_ref}"
    # Re-fetch immediately before the test gate: checkout's initial ref snapshot
    # can become stale while the job is queued or running.
    git("fetch", "--no-tags", "origin", f"refs/heads/{base_ref}:{tracking_ref}")
    tracking_base = git("rev-parse", "--verify", f"{tracking_ref}^{{commit}}")
    if tracking_base.lower() != base.lower():
        raise VerificationError("base branch advanced; merge-result is stale")

    local_merge_ref = "refs/ir/pr-merge-result"
    git("check-ref-format", local_merge_ref)
    git("fetch", "--no-tags", "origin", f"+{merge_ref}:{local_merge_ref}")
    latest_merge = git("rev-parse", "--verify", f"{local_merge_ref}^{{commit}}")
    if latest_merge.lower() != expected_merge.lower():
        raise VerificationError("PR merge ref changed; refusing a stale merge-result")

    parents = git("show", "-s", "--format=%P", expected_merge).split()
    if len(parents) != 2 or [p.lower() for p in parents] != [base.lower(), head.lower()]:
        raise VerificationError("merge-result parents do not match the event Base and HEAD")
    return base.lower(), head.lower(), expected_merge.lower()


def main() -> int:
    try:
        base, head, merge = verify()
    except VerificationError as exc:
        print(f"PR merge-result verification failed closed: {exc}", file=sys.stderr)
        return 2
    print(f"PR merge-result verified: base={base} head={head} merge={merge}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
