from __future__ import annotations

import subprocess
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
            self.write_xml(root, "room.xml", '<testsuite tests="2" failures="0" errors="0" skipped="0"><testcase classname="a.Room" name="first"/><testcase classname="a.Room" name="second"/></testsuite>')
            self.write_xml(root, "compose.xml", '<testsuite tests="1" failures="0" errors="0" skipped="1"><testcase classname="a.Compose" name="one"><skipped/></testcase></testsuite>')
            self.assertEqual((3, 0, 0, 1, 2), tuple(collect_results(root).__dict__.values()))

    def test_reads_nested_testsuites(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_xml(
                root,
                "results.xml",
                '<testsuites><testsuite tests="1" failures="1" errors="0" skipped="0"><testcase classname="a.Test" name="first"><failure/></testcase></testsuite></testsuites>',
            )
            result = collect_results(root)
            self.assertEqual(1, result.tests)
            self.assertEqual(1, result.failures)

    def test_rejects_non_junit_xml(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_xml(root, "metadata.xml", "<metadata/>")
            with self.assertRaisesRegex(ValueError, "Non-JUnit XML"):
                collect_results(root)

    def test_rejects_empty_suites_duplicate_cases_and_missing_case_identity(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_xml(root, "empty.xml", '<testsuite tests="0"/>')
            with self.assertRaisesRegex(ValueError, "zero tests"):
                collect_results(root)
            self.write_xml(root, "empty.xml", '<testsuite tests="2"><testcase classname="a.C" name="one"/></testsuite>')
            with self.assertRaisesRegex(ValueError, "count does not match"):
                collect_results(root)
            self.write_xml(root, "empty.xml", '<testsuite tests="1"><testcase classname="a.C"/></testsuite>')
            with self.assertRaisesRegex(ValueError, "classname and name"):
                collect_results(root)
            self.write_xml(root, "empty.xml", '<testsuite tests="1"><testcase classname="a.C" name="one"/></testsuite>')
            self.write_xml(root, "duplicate.xml", '<testsuite tests="1"><testcase classname="a.C" name="one"/></testsuite>')
            with self.assertRaisesRegex(ValueError, "Duplicate instrumentation JUnit testcase"):
                collect_results(root)

    def test_rejects_malformed_junit_xml(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_xml(root, "broken.xml", "<testsuite")
            with self.assertRaises(ValueError):
                collect_results(root)

    def test_rejects_suite_attributes_that_hide_testcase_failures(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for attribute, child in (("failures", "failure"), ("errors", "error"), ("skipped", "skipped")):
                self.write_xml(root, "contradictory.xml",
                    f'<testsuite tests="1" failures="0" errors="0" skipped="0">'
                    f'<testcase classname="a.C" name="bad"><{child}/></testcase></testsuite>')
                with self.assertRaisesRegex(ValueError, f"JUnit {attribute} attribute"):
                    collect_results(root)

    def test_counts_required_test_classes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_xml(root, "results.xml", '<testsuite tests="2"><testcase classname="a.Storage" name="first"/><testcase classname="a.Chat" name="second"/></testsuite>')
            self.assertEqual({"a.Storage": 1, "a.Chat": 1}, collect_class_counts(root))
            verifier = Path(__file__).resolve().parents[1] / "tools/verify_android_instrumentation.py"
            exact_case = subprocess.run(
                ["python3", str(verifier), str(root), "--minimum", "2", "--exact-count", "2",
                 "--require-test", "a.Storage#first", "--require-test", "a.Chat#second"],
                capture_output=True,
                text=True,
            )
            self.assertEqual(0, exact_case.returncode, exact_case.stderr)
            wrong_case = subprocess.run(
                ["python3", str(verifier), str(root), "--minimum", "2", "--exact-count", "2",
                 "--require-test", "a.Storage#first", "--require-test", "a.Chat#missing"],
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(0, wrong_case.returncode)

    def test_partial_103_case_junit_cannot_pass_the_complete_campaign_gate(self) -> None:
        required = {
            "app.serea.android.EncryptedHistoryTest": 17,
            "app.serea.android.ChatScreenTest": 16,
            "app.serea.android.ChatHttpE2ETest": 4,
            "app.serea.android.UsageLedgerDatabaseTest": 12,
            "app.serea.android.tasks.TasksDatabaseTest": 6,
            "app.serea.android.tasks.TaskScreenTest": 6,
            "app.serea.android.InstrumentationStorageIsolationTest": 1,
            "app.serea.android.ChatTasksDatabaseIntegrationTest": 1,
            "app.serea.android.ChatHistoryUxTest": 4,
            "app.serea.android.tasks.TaskDateTimePickerTest": 6,
            "app.serea.android.assistant.taskproposal.TaskProposalReviewScreenTest": 2,
            "app.serea.android.SereaAppShellTest": 19,
            "app.serea.android.assistant.taskproposal.TaskProposalDemoScreenTest": 3,
            "app.serea.android.SereaThemeTest": 2,
            "app.serea.android.MockUiStateIntegrationTest": 4,
        }
        self.assertEqual(103, sum(required.values()))
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            results = root / "junit"
            source_root = root / "androidTest"
            results.mkdir()
            cases: list[str] = []
            for class_name, count in required.items():
                source = source_root.joinpath(*class_name.split(".")).with_suffix(".kt")
                source.parent.mkdir(parents=True, exist_ok=True)
                methods = [f"test_{index}" for index in range(count)]
                source.write_text(
                    "package " + class_name.rpartition(".")[0] + "\nclass " + class_name.rpartition(".")[2] + " {\n"
                    + "\n".join(f"@Test fun {method}() {{}}" for method in methods)
                    + "\n}\n",
                    encoding="utf-8",
                )
                cases.extend(f'<testcase classname="{class_name}" name="{method}"/>' for method in methods)

            suite = results / "TEST-campaign.xml"
            suite.write_text(
                f'<testsuite tests="103" failures="0" errors="0" skipped="0">'
                + "".join(cases)
                + "</testsuite>",
                encoding="utf-8",
            )
            verifier = Path(__file__).resolve().parents[1] / "tools/verify_android_instrumentation.py"
            args = [
                "python3", str(verifier), str(results), "--minimum", "103", "--source-root", str(source_root),
            ]
            for class_name, count in required.items():
                args.extend(("--require-class", f"{class_name}={count}"))
            complete = subprocess.run(args, capture_output=True, text=True)
            self.assertEqual(0, complete.returncode, complete.stderr)

            suite.write_text(
                f'<testsuite tests="102" failures="0" errors="0" skipped="0">'
                + "".join(cases[:-1])
                + "</testsuite>",
                encoding="utf-8",
            )
            partial = subprocess.run(args, capture_output=True, text=True)
            self.assertNotEqual(0, partial.returncode)
            self.assertIn("only 102 tests were recorded", partial.stderr)

    def test_source_counts_ignore_comments_and_string_literals_and_reject_unlisted_tests(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "a").mkdir()
            (root / "a" / "Storage.kt").write_text(
                '''package a\nclass Storage {\n  // @Test fun comment() {}\n  val sample = "@Test"\n  @Test fun first() {}\n  @org.junit.Test fun second() {}\n}\n''',
                encoding="utf-8",
            )
            counts, total, methods = collect_source_test_counts(root, {"a.Storage": 2})
            self.assertEqual({"a.Storage": 2}, counts)
            self.assertEqual(2, total)
            self.assertEqual({"first", "second"}, methods["a.Storage"])
            (root / "a" / "Other.kt").write_text("package a\nclass Other { @Test fun added() {} }\n", encoding="utf-8")
            _, total, _ = collect_source_test_counts(root, {"a.Storage": 2})
            self.assertEqual(3, total)


if __name__ == "__main__":
    unittest.main()
