from __future__ import annotations

import contextlib
import io
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from tools.verify_android_unit_junit import collect_results, main


class AndroidUnitJunitResultTests(unittest.TestCase):
    def write_xml(self, directory: Path, name: str, xml: str) -> None:
        (directory / name).write_text(xml, encoding="utf-8")

    def test_sums_junit_counts_for_the_full_unit_task(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_xml(root, "TEST-a.xml", '<testsuite tests="3" failures="0" errors="0" skipped="0"/>')
            self.write_xml(root, "TEST-b.xml", '<testsuite tests="2" failures="0" errors="0" skipped="0"/>')
            results = collect_results(root)
            self.assertEqual((5, 0, 0, 0, 2), tuple(results.__dict__.values()))

    def test_rejects_missing_empty_and_malformed_junit_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaisesRegex(ValueError, "No Android unit JUnit XML"):
                collect_results(root)
            self.write_xml(root, "empty.xml", '<testsuite tests="0" failures="0" errors="0" skipped="0"/>')
            with self.assertRaisesRegex(ValueError, "zero tests"):
                collect_results(root)
            self.write_xml(root, "broken.xml", "<testsuite")
            with self.assertRaisesRegex(ValueError, "Invalid Android unit JUnit XML"):
                collect_results(root)

    def test_verifier_rejects_failure_error_or_skip(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_xml(root, "failure.xml", '<testsuite tests="1" failures="1" errors="0" skipped="0"/>')
            with patch("sys.argv", ["verify_android_unit_junit.py", str(root)]), contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(1, main())
            self.write_xml(root, "failure.xml", '<testsuite tests="1" failures="0" errors="0" skipped="1"/>')
            with patch("sys.argv", ["verify_android_unit_junit.py", str(root)]), contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(1, main())


if __name__ == "__main__":
    unittest.main()
