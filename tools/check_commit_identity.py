#!/usr/bin/env python3
"""Reject reachable commits without a public-safe GitHub noreply identity."""
from __future__ import annotations

import subprocess
import sys


def approved_email(email: str) -> bool:
    return email == "noreply@github.com" or (
        email.endswith("@users.noreply.github.com")
        and bool(email.removesuffix("@users.noreply.github.com"))
    )


def invalid_commits(records: str) -> list[str]:
    invalid: list[str] = []
    for record in records.splitlines():
        fields = record.split("\0")
        if len(fields) != 3:
            continue
        sha, author_email, committer_email = fields
        if not approved_email(author_email) or not approved_email(committer_email):
            invalid.append(sha)
    return invalid


def main() -> int:
    try:
        result = subprocess.run(
            ["git", "log", "--all", "--format=%H%x00%ae%x00%ce"],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        print("commit identity check could not read Git history", file=sys.stderr)
        return 2

    bad = invalid_commits(result.stdout)
    if bad:
        print("commit identity check failed; disallowed author/committer on:", file=sys.stderr)
        for sha in bad:
            print(sha, file=sys.stderr)
        return 1
    print("commit identity check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
