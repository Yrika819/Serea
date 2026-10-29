from __future__ import annotations

import subprocess
import json
import tempfile
import unittest
import zipfile
from pathlib import Path

from tools.verify_android_instrumentation import (
    _source_test_count,
    _source_test_methods,
    collect_class_counts,
    collect_failure_details,
    collect_results,
    collect_source_test_identities,
    collect_source_test_counts,
    read_test_manifest,
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
        required_tests = read_test_manifest(required_manifest)
        additional_tests = read_test_manifest(additional_manifest)
        non_required_tests = read_test_manifest(non_required_manifest)
        self.assertEqual(103, len(required_tests))
        self.assertEqual(2, len(additional_tests))
        self.assertFalse(required_tests & additional_tests)
        self.assertEqual(105, len(required_tests | additional_tests))
        self.assertEqual(17, len(non_required_tests))
        self.assertFalse(non_required_tests & (required_tests | additional_tests))
        expected_source_identities = required_tests | additional_tests | non_required_tests
        actual_source_root = repo / "android/app/src/androidTest/java"
        actual_source_identities = collect_source_test_identities(actual_source_root)
        self.assertEqual(expected_source_identities, actual_source_identities)
        actual_source_test_count = sum(
            _source_test_count(source.read_text(encoding="utf-8"))
            for source in actual_source_root.rglob("*.kt")
        )
        self.assertEqual(122, actual_source_test_count)
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

    def test_required_chat_http_identities_are_split_without_changing_the_105_gate(self) -> None:
        repo = Path(__file__).resolve().parents[1]
        helper = repo / "tools/android_instrumentation_variant_filters.py"
        required = repo / "tools/android-instrumentation-required-tests.txt"
        additional = repo / "tools/android-instrumentation-additional-tests.txt"
        http_tests = repo / "tools/android-instrumentation-http-loopback-tests.txt"
        non_required = repo / "android/app/src/androidTest/assets/serea-non-required-test-identities.txt"
        source_root = repo / "android/app/src/androidTest/java"
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "filters.json"
            result = subprocess.run(
                [
                    "python3", str(helper), "--required", str(required), "--additional", str(additional),
                    "--non-required", str(non_required), "--http-tests", str(http_tests),
                    "--source-root", str(source_root), "--output", str(output),
                ],
                cwd=repo,
                capture_output=True,
                text=True,
            )
            self.assertEqual(0, result.returncode, result.stdout + result.stderr)
            filters = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual((103, 2, 105, 101, 4), (
                filters["required_count"], filters["additional_count"], filters["formal_total"],
                filters["offline_count"], filters["http_count"],
            ))
            self.assertTrue(filters["all_unique"])
            self.assertEqual(4, len(set(filters["http_identities"])))
            self.assertTrue(all(identity.startswith("app.serea.android.ChatHttpE2ETest#")
                                for identity in filters["http_identities"]))
            self.assertEqual(105, len(set(filters["offline_identities"] + filters["http_identities"])))
            self.assertFalse(set(filters["offline_identities"]) & set(filters["http_identities"]))

            reduced = Path(temporary) / "reduced-required.txt"
            lines = required.read_text(encoding="utf-8").splitlines()
            reduced.write_text("\n".join(lines[:-1]) + "\n", encoding="utf-8")
            rejected = subprocess.run(
                [
                    "python3", str(helper), "--required", str(reduced), "--additional", str(additional),
                    "--non-required", str(non_required), "--http-tests", str(http_tests), "--source-root", str(source_root),
                    "--output", str(Path(temporary) / "rejected.json"),
                ],
                cwd=repo,
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(0, rejected.returncode)
            self.assertIn("Required gate changed", rejected.stderr)

            reduced_http = Path(temporary) / "reduced-http.txt"
            http_lines = http_tests.read_text(encoding="utf-8").splitlines()
            reduced_http.write_text("\n".join(http_lines[:-1]) + "\n", encoding="utf-8")
            rejected_http = subprocess.run(
                [
                    "python3", str(helper), "--required", str(required), "--additional", str(additional),
                    "--non-required", str(non_required), "--http-tests", str(reduced_http),
                    "--source-root", str(source_root), "--output", str(Path(temporary) / "rejected-http.json"),
                ],
                cwd=repo,
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(0, rejected_http.returncode)
            self.assertIn("HTTP loopback manifest differs", rejected_http.stderr)

    def test_http_test_apk_permission_verifier_keeps_native_variant_offline(self) -> None:
        repo = Path(__file__).resolve().parents[1]
        verifier = repo / "tools/verify_android_http_test_apk_permissions.py"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            aapt = root / "aapt"
            aapt.write_text(
                "#!/usr/bin/env python3\n"
                "import pathlib, sys\n"
                "apk = pathlib.Path(sys.argv[-1]).name\n"
                "package = 'app.serea.android.isolated.test' if 'test' in apk else 'app.serea.android.isolated'\n"
                "print('package: ' + package)\n"
                "if apk.startswith('http-'):\n"
                "    print(\"uses-permission: name='android.permission.INTERNET'\")\n",
                encoding="utf-8",
            )
            aapt.chmod(0o755)
            apks = {}
            for name in ("offline-target.apk", "offline-test.apk", "http-target.apk", "http-test.apk"):
                apk = root / name
                with zipfile.ZipFile(apk, "w") as archive:
                    archive.writestr("classes.dex", b"fixture")
                apks[name] = apk
            evidence = root / "permissions.txt"
            result = subprocess.run(
                [
                    "python3", str(verifier), "--aapt", str(aapt),
                    "--offline-target", str(apks["offline-target.apk"]),
                    "--offline-test", str(apks["offline-test.apk"]),
                    "--http-target", str(apks["http-target.apk"]),
                    "--http-test", str(apks["http-test.apk"]), "--evidence", str(evidence),
                ],
                cwd=repo,
                capture_output=True,
                text=True,
            )
            self.assertEqual(0, result.returncode, result.stdout + result.stderr)
            report = evidence.read_text(encoding="utf-8")
            self.assertIn("offline_target_internet=DENIED", report)
            self.assertIn("offline_test_internet=DENIED", report)
            self.assertIn("http_target_internet=GRANTED", report)
            self.assertIn("http_test_internet=GRANTED", report)
            self.assertIn("http_test_native_jni=ABSENT", report)

            aapt.write_text(
                aapt.read_text(encoding="utf-8").replace("if apk.startswith('http-'):", "if not apk.startswith('http-'):"),
                encoding="utf-8",
            )
            denied_fixture = subprocess.run(
                [
                    "python3", str(verifier), "--aapt", str(aapt),
                    "--offline-target", str(apks["offline-target.apk"]),
                    "--offline-test", str(apks["offline-test.apk"]),
                    "--http-target", str(apks["http-target.apk"]),
                    "--http-test", str(apks["http-test.apk"]), "--evidence", str(evidence),
                ],
                cwd=repo,
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(0, denied_fixture.returncode)
            self.assertIn("Unexpected INTERNET permission", denied_fixture.stderr)

    def test_partition_result_verifier_binds_exact_source_api_and_method_sets(self) -> None:
        repo = Path(__file__).resolve().parents[1]
        filter_helper = repo / "tools/android_instrumentation_variant_filters.py"
        verifier = repo / "tools/verify_android_instrumentation_partition_results.py"
        required = repo / "tools/android-instrumentation-required-tests.txt"
        additional = repo / "tools/android-instrumentation-additional-tests.txt"
        http_tests = repo / "tools/android-instrumentation-http-loopback-tests.txt"
        non_required = repo / "android/app/src/androidTest/assets/serea-non-required-test-identities.txt"
        source_root = repo / "android/app/src/androidTest/java"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            filters = root / "filters.json"
            generated = subprocess.run(
                [
                    "python3", str(filter_helper), "--required", str(required), "--additional", str(additional),
                    "--non-required", str(non_required), "--http-tests", str(http_tests),
                    "--source-root", str(source_root), "--output", str(filters),
                ],
                cwd=repo,
                capture_output=True,
                text=True,
            )
            self.assertEqual(0, generated.returncode, generated.stdout + generated.stderr)
            payload = json.loads(filters.read_text(encoding="utf-8"))
            offline = root / "offline"
            http = root / "http"
            offline.mkdir()
            http.mkdir()

            def write_suite(directory: Path, identities: list[str]) -> None:
                cases = []
                for identity in identities:
                    class_name, method_name = identity.split("#", 1)
                    cases.append(f'<testcase classname="{class_name}" name="{method_name}"/>')
                xml = f'<testsuite tests="{len(cases)}" failures="0" errors="0" skipped="0">' + "".join(cases) + "</testsuite>"
                (directory / "results.xml").write_text(xml, encoding="utf-8")

            write_suite(offline, payload["offline_identities"])
            write_suite(http, payload["http_identities"])
            evidence = root / "api-evidence"
            evidence.mkdir()
            (evidence / "device-api-level.txt").write_text("26\n", encoding="utf-8")
            (evidence / "build-provenance.txt").write_text(
                f"source_checkout_sha={payload['source_sha']}\napi_level=26\n", encoding="utf-8"
            )
            command = [
                "python3", str(verifier), "--filters", str(filters), "--offline-junit", str(offline),
                "--http-junit", str(http), "--evidence-dir", str(evidence), "--api-level", "26",
            ]
            result = subprocess.run(command, cwd=repo, capture_output=True, text=True)
            self.assertEqual(0, result.returncode, result.stdout + result.stderr)
            self.assertIn("offline=101 http=4 total=105", result.stdout)

            (evidence / "device-api-level.txt").write_text("35\n", encoding="utf-8")
            wrong_api = subprocess.run(command, cwd=repo, capture_output=True, text=True)
            self.assertNotEqual(0, wrong_api.returncode)
            self.assertIn("JUnit API binding mismatch", wrong_api.stderr)


if __name__ == "__main__":
    unittest.main()
