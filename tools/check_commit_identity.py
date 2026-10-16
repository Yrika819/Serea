#!/usr/bin/env python3
"""Reject reachable commits with malformed names or unsafe email addresses."""
from __future__ import annotations

import re
import subprocess
import sys


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


def main() -> int:
    try:
        result = subprocess.run(
            ["git", "log", "--all", "-z", "--format=%H%x00%an%x00%ae%x00%cn%x00%ce"],
            check=True,
            capture_output=True,
            text=True,
            encoding="utf-8",
        )
    except (OSError, subprocess.CalledProcessError):
        print("commit identity check could not read Git history", file=sys.stderr)
        return 2

    bad = invalid_commits(result.stdout)
    if bad:
        print("commit identity check failed; malformed author/committer on:", file=sys.stderr)
        for sha in bad:
            print(sha, file=sys.stderr)
        return 1
    print("commit identity check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
