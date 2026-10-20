from __future__ import annotations

import contextlib
import io
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from tools.verify_android_unit_junit import (
    collect_results,
    collect_source_class_counts,
    main,
    verify_source_discovery,
)


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

    def test_source_and_junit_discovery_counts_match_without_fixed_total(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "src" / "app" / "sample"
            results = root / "results"
            source.mkdir(parents=True)
            results.mkdir()
            (source / "FeatureTest.kt").write_text(
                "package app.sample\nimport org.junit.Test\nclass FeatureTest {\n"
                "@Test fun first() {}\n@Test fun second() {}\n}\n",
                encoding="utf-8",
            )
            java_source = root / "src" / "app" / "sample" / "JavaFeatureTest.java"
            java_source.write_text(
                "package app.sample;\nimport org.junit.Test;\npublic class JavaFeatureTest {\n"
                "@Test public void one() {}\n}\n",
                encoding="utf-8",
            )
            self.write_xml(results, "TEST-app.sample.FeatureTest.xml",
                '<testsuite tests="2" failures="0" errors="0" skipped="0">'
                '<testcase classname="app.sample.FeatureTest" name="first"/>'
                '<testcase classname="app.sample.FeatureTest" name="second"/>'
                '</testsuite>')
            self.write_xml(results, "TEST-app.sample.JavaFeatureTest.xml",
                '<testsuite tests="1" failures="0" errors="0" skipped="0">'
                '<testcase classname="app.sample.JavaFeatureTest" name="one"/>'
                '</testsuite>')

            self.assertEqual({"app.sample.FeatureTest": 2, "app.sample.JavaFeatureTest": 1}, collect_source_class_counts(root / "src"))
            verify_source_discovery(results, root / "src", ("app.sample.FeatureTest", "app.sample.JavaFeatureTest"))

    def test_source_junit_mismatch_and_missing_critical_class_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "src"
            results = root / "results"
            source.mkdir()
            results.mkdir()
            (source / "FeatureTest.kt").write_text(
                "package app.sample\nclass FeatureTest { @Test fun first() {} @Test fun second() {} }\n",
                encoding="utf-8",
            )
            self.write_xml(results, "TEST-app.sample.FeatureTest.xml",
                '<testsuite tests="1" failures="0" errors="0" skipped="0">'
                '<testcase classname="app.sample.FeatureTest" name="first"/>'
                '</testsuite>')

            with self.assertRaisesRegex(ValueError, "test count mismatch"):
                verify_source_discovery(results, source, ("app.sample.FeatureTest",))
            with self.assertRaisesRegex(ValueError, "Required Android unit test source class is missing"):
                verify_source_discovery(results, source, ("app.sample.SecurityTest",))


if __name__ == "__main__":
    unittest.main()
