from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path

from tools.verify_android_instrumentation import (
    _source_test_count,
    _source_test_methods,
    collect_class_counts,
    collect_failure_details,
    collect_results,
    collect_source_test_counts,
    verify_profile_verifier_evidence,
)


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

    def test_failure_details_identify_failing_instrumentation_case(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_xml(
                root,
                "failed.xml",
                '<testsuite tests="1" failures="1" errors="0" skipped="0">'
                '<testcase classname="a.CalendarTest" name="scrollsToTarget">'
                '<failure message="visible text bounds were clipped">stack trace</failure>'
                '</testcase></testsuite>',
            )
            self.assertEqual(
                ["a.CalendarTest#scrollsToTarget: visible text bounds were clipped"],
                collect_failure_details(root),
            )

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

    def test_103_required_plus_two_additional_testcases_are_verified_by_identity(self) -> None:
        repo = Path(__file__).resolve().parents[1]
        required_manifest = repo / "tools/android-instrumentation-required-tests.txt"
        additional_manifest = repo / "tools/android-instrumentation-additional-tests.txt"
        non_required_manifest = repo / "android/app/src/androidTest/assets/serea-non-required-test-identities.txt"
        required_tests = {
            tuple(line.split("#", 1))
            for line in required_manifest.read_text(encoding="utf-8").splitlines()
            if line.strip()
        }
        additional_tests = {
            tuple(line.split("#", 1))
            for line in additional_manifest.read_text(encoding="utf-8").splitlines()
            if line.strip()
        }
        non_required_tests = {
            tuple(line.split("#", 1))
            for line in non_required_manifest.read_text(encoding="utf-8").splitlines()
            if line.strip() and not line.startswith("#")
        }
        self.assertEqual(103, len(required_tests))
        self.assertEqual(2, len(additional_tests))
        self.assertFalse(required_tests & additional_tests)
        # Explicit non-required diagnostics and focused UI cases stay outside the 105 formal tests.
        self.assertEqual(22, len(non_required_tests))
        self.assertFalse(non_required_tests & (required_tests | additional_tests))
        required: dict[str, int] = {}
        for class_name, _ in required_tests:
            required[class_name] = required.get(class_name, 0) + 1
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            results = root / "junit"
            source_root = root / "androidTest"
            profile_log = root / "profile-verifier-logcat.txt"
            profile_log.write_text(
                "I/SereaProfileVerifier( 1234): api=35 status=COMPLETED "
                "resultCode=2 compiled=false enqueued=true embedded=true marker=valid\n",
                encoding="utf-8",
            )
            results.mkdir()
            cases: list[str] = []
            all_tests = required_tests | additional_tests
            by_class: dict[str, set[str]] = {}
            for class_name, method in all_tests:
                by_class.setdefault(class_name, set()).add(method)
            for class_name, methods in by_class.items():
                source = source_root.joinpath(*class_name.split(".")).with_suffix(".kt")
                source.parent.mkdir(parents=True, exist_ok=True)
                source.write_text(
                    "package " + class_name.rpartition(".")[0] + "\nclass " + class_name.rpartition(".")[2] + " {\n"
                    + "\n".join(f"@Test fun {method}() {{}}" for method in sorted(methods))
                    + "\n}\n",
                    encoding="utf-8",
                )
                cases.extend(f'<testcase classname="{class_name}" name="{method}"/>' for method in sorted(methods))

            for class_name, methods in {
                class_name: {method for candidate, method in non_required_tests if candidate == class_name}
                for class_name, _ in non_required_tests
            }.items():
                source = source_root.joinpath(*class_name.split(".")).with_suffix(".kt")
                source.parent.mkdir(parents=True, exist_ok=True)
                source.write_text(
                    "package " + class_name.rpartition(".")[0] + "\nclass " + class_name.rpartition(".")[2] + " {\n"
                    + "\n".join(f"@Test fun {method}() {{}}" for method in sorted(methods))
                    + "\n}\n",
                    encoding="utf-8",
                )

            suite = results / "TEST-campaign.xml"
            suite.write_text(
                f'<testsuite tests="105" failures="0" errors="0" skipped="0">'
                + "".join(cases)
                + "</testsuite>",
                encoding="utf-8",
            )
            verifier = repo / "tools/verify_android_instrumentation.py"
            args = [
                "python3", str(verifier), str(results), "--minimum", "105", "--exact-count", "105",
                "--source-root", str(source_root), "--required-test-manifest", str(required_manifest),
                "--additional-test-manifest", str(additional_manifest),
                "--non-required-test-manifest", str(non_required_manifest), "--required-count", "103",
                "--additional-count", "2", "--api-level", "35", "--profile-verifier-log", str(profile_log),
            ]
            for class_name, count in required.items():
                args.extend(("--require-class", f"{class_name}={count}"))
            complete = subprocess.run(args, capture_output=True, text=True)
            self.assertEqual(0, complete.returncode, complete.stderr)
            self.assertIn("Original Required Set: 103/103 PASS", complete.stdout)
            self.assertIn("Additional Tests: 2/2 PASS; total=105", complete.stdout)
            self.assertIn("ProfileVerifier runtime evidence: API 35", complete.stdout)

            unexpected_source = source_root / "a" / "UnlistedDiagnostic.kt"
            unexpected_source.parent.mkdir(parents=True, exist_ok=True)
            unexpected_source.write_text("package a\nclass UnlistedDiagnostic { @Test fun unlisted() {} }\n", encoding="utf-8")
            unlisted = subprocess.run(args, capture_output=True, text=True)
            self.assertNotEqual(0, unlisted.returncode)
            self.assertIn("unexpected=", unlisted.stderr)
            unexpected_source.unlink()

            suite.write_text(
                f'<testsuite tests="104" failures="0" errors="0" skipped="0">'
                + "".join(cases[:-1])
                + "</testsuite>",
                encoding="utf-8",
            )
            partial = subprocess.run(args, capture_output=True, text=True)
            self.assertNotEqual(0, partial.returncode)
        self.assertIn("only 104 tests were recorded; expected at least 105", partial.stderr)

    def test_profile_verifier_runtime_evidence_is_api_specific_and_positive(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            log = Path(temporary) / "logcat.txt"
            log.write_text(
                "I/SereaProfileVerifier( 10): api=35 status=COMPLETED "
                "resultCode=2 compiled=false enqueued=true embedded=true marker=valid\n",
                encoding="utf-8",
            )
            self.assertTrue(verify_profile_verifier_evidence(log, 35)[0])
            self.assertFalse(verify_profile_verifier_evidence(log, 26)[0])
            log.write_text(
                "I/SereaProfileVerifier( 10): api=35 status=COMPLETED "
                "resultCode=0 compiled=false enqueued=false embedded=false marker=valid\n",
                encoding="utf-8",
            )
            self.assertFalse(verify_profile_verifier_evidence(log, 35)[0])
            log.write_text(
                "I/SereaProfileVerifier( 10): api=35 status=COMPLETED "
                "resultCode=262144 compiled=false enqueued=false embedded=false marker=absent\n",
                encoding="utf-8",
            )
            valid, message = verify_profile_verifier_evidence(log, 35)
            self.assertFalse(valid)
            self.assertIn("resultCode in {1, 2, 3}", message)
            log.write_text(
                f"I/SereaProfileVerifier( 10): api=35 status=COMPLETED "
                f"resultCode={5 << 16} compiled=false enqueued=false embedded=false marker=absent\n",
                encoding="utf-8",
            )
            self.assertFalse(verify_profile_verifier_evidence(log, 35)[0])
            log.write_text(
                "I/SereaProfileVerifier( 10): api=26 status=UNSUPPORTED "
                "resultCode=262144 embedded=false marker=absent\n",
                encoding="utf-8",
            )
            self.assertTrue(verify_profile_verifier_evidence(log, 26)[0])

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

    def test_kotlin_source_scanner_keeps_multiline_comments_and_literals_separate(self) -> None:
        source = r'''package a
val raw = """@Test fun rawStringFake() {} // /* '"""
val escaped = "escaped quote: \" and @Test fun escapedStringFake() {}"
// Calendar owner's note has an unmatched apostrophe; it must not swallow later declarations.
@Test fun swallowedByOldCrossLineQuoteRegex() {}
val apostrophe = '\''
/* A nested block comment with @Test fun blockFake() {} and a ' quote.
   /* nested with \" and another ' quote */
*/
@Test fun firstRealTest() {}
@org.junit.Test fun secondRealTest() {}
'''
        self.assertEqual(3, _source_test_count(source))
        self.assertEqual(
            {"swallowedByOldCrossLineQuoteRegex", "firstRealTest", "secondRealTest"},
            _source_test_methods(source),
        )

    def test_real_serea_app_shell_source_has_all_nineteen_instrumentation_tests(self) -> None:
        source = (
            Path(__file__).resolve().parents[1]
            / "android/app/src/androidTest/java/app/serea/android/SereaAppShellTest.kt"
        ).read_text(encoding="utf-8")
        methods = _source_test_methods(source)
        self.assertEqual(19, _source_test_count(source))
        self.assertEqual(19, len(methods))
        self.assertIn("calendarDetailAndSelectedCalendarSurviveAnotherDestination", methods)


if __name__ == "__main__":
    unittest.main()
