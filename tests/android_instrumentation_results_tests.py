from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from tools.verify_android_instrumentation import collect_class_counts, collect_results, collect_source_test_counts


class InstrumentationResultTests(unittest.TestCase):
    def write_xml(self, directory: Path, name: str, xml: str) -> None:
        (directory / name).write_text(xml, encoding="utf-8")

    def test_sums_test_suites_and_skips(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_xml(root, "room.xml", '<testsuite tests="6" failures="0" errors="0" skipped="0"/>')
            self.write_xml(root, "compose.xml", '<testsuite tests="3" failures="0" errors="0" skipped="1"/>')
            self.assertEqual((9, 0, 0, 1, 2), tuple(collect_results(root).__dict__.values()))

    def test_reads_nested_testsuites(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_xml(
                root,
                "results.xml",
                '<testsuites><testsuite tests="4" failures="1" errors="0" skipped="0"/></testsuites>',
            )
            result = collect_results(root)
            self.assertEqual(4, result.tests)
            self.assertEqual(1, result.failures)

    def test_ignores_non_junit_xml(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_xml(root, "metadata.xml", "<metadata/>")
            self.assertEqual(0, collect_results(root).files)

    def test_rejects_malformed_junit_xml(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_xml(root, "broken.xml", "<testsuite")
            with self.assertRaises(ValueError):
                collect_results(root)

    def test_counts_required_test_classes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_xml(root, "results.xml", '<testsuite tests="2"><testcase classname="a.Storage"/><testcase classname="a.Chat"/></testsuite>')
            self.assertEqual({"a.Storage": 1, "a.Chat": 1}, collect_class_counts(root))

    def test_source_counts_ignore_comments_and_string_literals_and_reject_unlisted_tests(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "a").mkdir()
            (root / "a" / "Storage.kt").write_text(
                '''package a\nclass Storage {\n  // @Test fun comment() {}\n  val sample = "@Test"\n  @Test fun first() {}\n  @org.junit.Test fun second() {}\n}\n''',
                encoding="utf-8",
            )
            counts, total = collect_source_test_counts(root, {"a.Storage": 2})
            self.assertEqual({"a.Storage": 2}, counts)
            self.assertEqual(2, total)
            (root / "a" / "Other.kt").write_text("package a\nclass Other { @Test fun added() {} }\n", encoding="utf-8")
            _, total = collect_source_test_counts(root, {"a.Storage": 2})
            self.assertEqual(3, total)


if __name__ == "__main__":
    unittest.main()
