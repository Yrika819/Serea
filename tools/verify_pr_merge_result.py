#!/usr/bin/env python3
"""Fail closed unless the checkout is GitHub's current PR merge result."""
from __future__ import annotations

import os
import re
import subprocess
import sys
from dataclasses import dataclass

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
        command = " ".join(args[:2])
        raise VerificationError(f"git {command} failed") from exc
    return result.stdout.strip()


def commit_parents(commit: str, label: str) -> tuple[str, str]:
    parents = git("show", "-s", "--format=%P", commit).split()
    if len(parents) != 2:
        raise VerificationError(f"{label} has {len(parents)} parents; expected exactly two")
    return parents[0].lower(), parents[1].lower()


def commit_tree(commit: str, label: str) -> str:
    tree = git("rev-parse", "--verify", f"{commit}^{{tree}}")
    if not SHA.fullmatch(tree):
        raise VerificationError(f"{label} tree SHA is malformed")
    return tree.lower()


@dataclass(frozen=True)
class Verification:
    base: str
    head: str
    expected_merge: str
    latest_merge: str
    expected_parents: tuple[str, str]
    latest_parents: tuple[str, str]
    expected_tree: str
    latest_tree: str
    acceptance: str


def verify(env: dict[str, str] | None = None) -> Verification:
    values = os.environ if env is None else env
    if values.get("SEREA_EVENT_NAME") != "pull_request":
        raise VerificationError("merge-result verification requires pull_request event")

    base_ref = values.get("SEREA_PR_BASE_REF", "")
    if not base_ref:
        raise VerificationError("base branch ref is missing or invalid")
    git("check-ref-format", f"refs/heads/{base_ref}")
    pr_number = values.get("SEREA_PR_NUMBER", "")
    if not PR_NUMBER.fullmatch(pr_number):
        raise VerificationError("pull request number is missing or invalid")

    base = values.get("SEREA_PR_BASE_SHA", "")
    head = values.get("SEREA_PR_HEAD_SHA", "")
    expected_merge = values.get("SEREA_EXPECTED_MERGE_SHA", "")
    for label, sha in (("base", base), ("head", head), ("expected merge", expected_merge)):
        if not SHA.fullmatch(sha):
            raise VerificationError(f"{label} SHA is missing or invalid")
        git("cat-file", "-e", f"{sha}^{{commit}}")
    base, head, expected_merge = base.lower(), head.lower(), expected_merge.lower()

    checked_out = git("rev-parse", "--verify", "HEAD^{commit}").lower()
    if not SHA.fullmatch(checked_out) or checked_out != expected_merge:
        raise VerificationError(
            f"checked-out SHA {checked_out} does not match expected merge SHA {expected_merge}"
        )

    # Refresh all three GitHub refs immediately before each gate. Their state at
    # workflow dispatch may be stale by the time a queued job begins or ends.
    base_ref_name = f"refs/heads/{base_ref}"
    pr_head_ref = f"refs/pull/{pr_number}/head"
    pr_merge_ref = f"refs/pull/{pr_number}/merge"
    local_base_ref = "refs/ir/pr-base"
    local_head_ref = "refs/ir/pr-head"
    local_merge_ref = "refs/ir/pr-merge-result"
    for ref in (base_ref_name, pr_head_ref, pr_merge_ref,
                local_base_ref, local_head_ref, local_merge_ref):
        git("check-ref-format", ref)

    # Fetch these refs in one advertisement so the comparison does not combine
    # a Base/HEAD snapshot with a later, unrelated merge-ref snapshot.
    git(
        "fetch", "--no-tags", "origin",
        f"+{base_ref_name}:{local_base_ref}",
        f"+{pr_head_ref}:{local_head_ref}",
        f"+{pr_merge_ref}:{local_merge_ref}",
    )
    latest_base = git("rev-parse", "--verify", f"{local_base_ref}^{{commit}}").lower()
    if latest_base != base:
        raise VerificationError(
            f"base branch advanced: event base={base}, latest base={latest_base}"
        )

    latest_head = git("rev-parse", "--verify", f"{local_head_ref}^{{commit}}").lower()
    if latest_head != head:
        raise VerificationError(
            f"PR HEAD advanced: event head={head}, latest head={latest_head}"
        )

    latest_merge = git("rev-parse", "--verify", f"{local_merge_ref}^{{commit}}").lower()

    expected_parents = commit_parents(expected_merge, "checked-out merge commit")
    latest_parents = commit_parents(latest_merge, "latest GitHub merge commit")
    expected_pair = (base, head)
    if expected_parents != expected_pair:
        raise VerificationError(
            f"checked-out merge parents {expected_parents} do not match event Base/HEAD {expected_pair}"
        )
    if latest_parents != expected_pair:
        raise VerificationError(
            f"latest GitHub merge parents {latest_parents} do not match event Base/HEAD {expected_pair}"
        )

    expected_tree = commit_tree(expected_merge, "checked-out merge commit")
    latest_tree = commit_tree(latest_merge, "latest GitHub merge commit")
    if expected_tree != latest_tree:
        raise VerificationError(
            f"merge trees differ: checked-out tree={expected_tree}, latest tree={latest_tree}"
        )

    acceptance = "exact-merge-sha" if latest_merge == expected_merge else "same-parents-and-tree"
    return Verification(
        base=base,
        head=head,
        expected_merge=expected_merge,
        latest_merge=latest_merge,
        expected_parents=expected_parents,
        latest_parents=latest_parents,
        expected_tree=expected_tree,
        latest_tree=latest_tree,
        acceptance=acceptance,
    )


def main() -> int:
    try:
        result = verify()
    except VerificationError as exc:
        print(f"PR merge-result verification failed closed: {exc}", file=sys.stderr)
        return 2
    print(
        "PR merge-result verified: "
        f"expected_sha={result.expected_merge} latest_sha={result.latest_merge} "
        f"base={result.base} head={result.head} "
        f"expected_parents={result.expected_parents[0]},{result.expected_parents[1]} "
        f"latest_parents={result.latest_parents[0]},{result.latest_parents[1]} "
        f"expected_tree={result.expected_tree} latest_tree={result.latest_tree} "
        f"acceptance={result.acceptance}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
