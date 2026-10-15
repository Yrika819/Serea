#!/usr/bin/env python3
"""Exercise safe Python 3 launcher selection for versioned Git hooks."""
from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RUNNER = ROOT / "tools" / "run_python.sh"


class PythonLauncherTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="serea-python-launcher-")
        self.addCleanup(self.temp.cleanup)
        self.bin = Path(self.temp.name) / "python launchers"
        self.bin.mkdir()
        self.log = Path(self.temp.name) / "launcher.log"
        self.sh = shutil.which("sh")
        if not self.sh:
            self.skipTest("a POSIX shell is required to execute Git hooks")

    def launcher(self, name: str, body: str) -> None:
        path = self.bin / name
        path.write_text("#!/bin/sh\n" + body, encoding="utf-8")
        path.chmod(0o755)

    def run_runner(self):
        return subprocess.run(
            [self.sh, str(RUNNER), "-c", "print('hook ran')"],
            env={**os.environ, "PATH": str(self.bin), "LAUNCHER_LOG": str(self.log)},
            text=True,
            capture_output=True,
            check=False,
        )

    def test_python3_launcher_is_used_from_path_with_spaces(self):
        self.launcher(
            "python3",
            'case "$*" in *sys.version_info*) exit 0;; esac\n'
            'printf python3 >> "$LAUNCHER_LOG"\n',
        )
        self.launcher(
            "python",
            'case "$*" in *sys.version_info*) exit 0;; esac\n'
            'printf python >> "$LAUNCHER_LOG"\n',
        )
        result = self.run_runner()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.log.read_text(encoding="utf-8"), "python3")

    def test_python_launcher_fallback(self):
        self.launcher(
            "python",
            'case "$*" in *sys.version_info*) exit 0;; esac\n'
            'printf python >> "$LAUNCHER_LOG"\n',
        )
        result = self.run_runner()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.log.read_text(encoding="utf-8"), "python")

    def test_py_three_fallback(self):
        self.launcher(
            "py",
            'if [ "$1" != "-3" ]; then exit 1; fi\n'
            'case "$*" in *sys.version_info*) exit 0;; esac\n'
            'printf py-3 >> "$LAUNCHER_LOG"\n',
        )
        result = self.run_runner()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.log.read_text(encoding="utf-8"), "py-3")

    def test_missing_python3_fails_closed_without_running_payload(self):
        self.launcher("python3", 'exit 1\n')
        self.launcher("python", 'exit 1\n')
        result = self.run_runner()
        self.assertEqual(result.returncode, 127)
        self.assertIn("requires a working Python 3 launcher", result.stderr)
        self.assertFalse(self.log.exists())


if __name__ == "__main__":
    unittest.main()
