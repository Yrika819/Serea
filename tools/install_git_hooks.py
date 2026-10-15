#!/usr/bin/env python3
"""Enable this repository's versioned hooks using repository-local config."""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    if not (root / ".git").exists():
        print("hook installation requires a Git working tree", file=sys.stderr)
        return 2
    hooks = root / ".githooks"
    required = (hooks / "pre-commit", hooks / "pre-push")
    if any(not path.is_file() for path in required):
        print("required versioned hook is missing", file=sys.stderr)
        return 2
    result = subprocess.run(
        ["git", "config", "--local", "core.hooksPath", ".githooks"],
        cwd=root, check=False,
    )
    if result.returncode:
        print("could not enable repository-local hooks", file=sys.stderr)
        return 2
    verify = subprocess.run(
        ["git", "config", "--local", "--get", "core.hooksPath"],
        cwd=root, text=True, capture_output=True, check=False,
    )
    if verify.returncode or verify.stdout.strip() != ".githooks":
        print("hook path verification failed", file=sys.stderr)
        return 2
    print("repository-local pre-commit and pre-push hooks enabled")
    print("verify with: git config --show-origin --show-scope --get core.hooksPath")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
