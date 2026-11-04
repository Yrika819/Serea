#!/usr/bin/env python3
"""Focused contract tests for the raw AndroidJUnitRunner diagnostic parser."""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

PARSER = Path(__file__).with_name("verify-instrumentation-stream.py")
NOOP = "app.serea.android.SereaNativeRunnerNoOpTest"
NATIVE = "app.serea.android.LocalModelNativeRuntimeE2ETest"
NOOP_METHOD = "androidJUnit4RunnerInstantiatesAndTargetsIsolatedApp"
NATIVE_METHOD = "pinnedModelRunsThroughLocalModelEgressUsageAndNativeBackend"


def status(class_name: str, method: str, code: int, extra: str = "") -> str:
    return (
        f"INSTRUMENTATION_STATUS: class={class_name}\n"
        f"INSTRUMENTATION_STATUS: test={method}\n"
        f"{extra}"
        f"INSTRUMENTATION_STATUS_CODE: {code}\n"
    )


class InstrumentationStreamParserTest(unittest.TestCase):
    def run_parser(self, stream: str) -> tuple[int, str, str]:
        with tempfile.TemporaryDirectory(prefix="serea-instrumentation-parser-") as temp_dir:
            root = Path(temp_dir)
            source = root / "instrumentation.txt"
            report = root / "junit.xml"
            summary = root / "summary.txt"
            source.write_text(stream, encoding="utf-8")
            result = subprocess.run(
                [sys.executable, str(PARSER), str(source), str(report), str(summary), NOOP, NATIVE],
                check=False,
                capture_output=True,
                text=True,
            )
            return result.returncode, summary.read_text(encoding="utf-8"), (
                report.read_text(encoding="utf-8") if report.exists() else ""
            )

    def test_noop_and_native_pass_with_exact_identities(self) -> None:
        stream = (
            status(NOOP, NOOP_METHOD, 1, "INSTRUMENTATION_STATUS: numtests=2\n")
            + status(NOOP, NOOP_METHOD, 0, "INSTRUMENTATION_STATUS: numtests=2\n")
            + status(NATIVE, NATIVE_METHOD, 1, "INSTRUMENTATION_STATUS: numtests=2\n")
            + status(NATIVE, NATIVE_METHOD, 0, "INSTRUMENTATION_STATUS: numtests=2\n")
            + "INSTRUMENTATION_CODE: -1\n"
        )
        code, summary, report = self.run_parser(stream)
        self.assertEqual(0, code)
        self.assertIn("tests=2\n", summary)
        self.assertIn("noop_result=PASS\n", summary)
        self.assertIn("native_result=PASS\n", summary)
        self.assertIn("reported_numtests=2\n", summary)
        self.assertIn(f'classname="{NOOP}" name="{NOOP_METHOD}"', report)
        self.assertIn(f'classname="{NATIVE}" name="{NATIVE_METHOD}"', report)

    def test_initialization_error_keeps_cause_chain(self) -> None:
        cause = "java.lang.IllegalStateException: runner failed\nCaused by: java.lang.NoSuchMethodException"
        stream = (
            status(NOOP, "initializationError", -2, "INSTRUMENTATION_STATUS: Error=Runner constructor failed\n")
            + status(
                NATIVE,
                "initializationError",
                -2,
                "INSTRUMENTATION_STATUS: Error=Failed to instantiate test runner\n"
                f"INSTRUMENTATION_STATUS: stack={cause}\n",
            )
            + "INSTRUMENTATION_CODE: -1\n"
        )
        code, summary, report = self.run_parser(stream)
        self.assertEqual(0, code)
        self.assertIn("tests=2\n", summary)
        self.assertIn(f"terminal_status={NOOP}#initializationError:-2\n", summary)
        self.assertIn(f"terminal_status={NATIVE}#initializationError:-2\n", summary)
        self.assertIn("native_initialization_error=PRESENT\n", summary)
        self.assertIn("native_initialization_stack_present=true\n", summary)
        self.assertIn("java.lang.IllegalStateException", report)
        self.assertIn("Raw failure message and stack were suppressed", report)
        self.assertIn(
            "native_initialization_exception_types=java.lang.IllegalStateException,java.lang.NoSuchMethodException\n",
            summary,
        )
        self.assertNotIn("NoSuchMethodException", report)
        self.assertNotIn("Caused by:", report)

    def test_noop_pass_with_native_initialization_error_discriminates_test_class(self) -> None:
        stream = (
            status(NOOP, NOOP_METHOD, 0)
            + status(
                NATIVE,
                "initializationError",
                -2,
                "INSTRUMENTATION_STATUS: Error=Native test class initialization failed\n",
            )
            + "INSTRUMENTATION_CODE: -1\n"
        )
        code, summary, _ = self.run_parser(stream)
        self.assertEqual(0, code)
        self.assertIn("tests=2\n", summary)
        self.assertIn("noop_result=PASS\n", summary)
        self.assertIn("native_initialization_error=PRESENT\n", summary)

    def test_skip_and_failure_are_counted_separately(self) -> None:
        stream = (
            status(NOOP, NOOP_METHOD, -3)
            + status(NATIVE, NATIVE_METHOD, -2, "INSTRUMENTATION_STATUS: Error=JNI load failed\n")
            + "INSTRUMENTATION_CODE: -1\n"
        )
        code, summary, report = self.run_parser(stream)
        self.assertEqual(0, code)
        self.assertIn("tests=2\n", summary)
        self.assertIn("failures=1\n", summary)
        self.assertIn("skipped=1\n", summary)
        self.assertIn("<failure", report)
        self.assertIn("<skipped", report)
        self.assertNotIn("JNI load failed", report)

    def test_runner_failure_without_testcase_is_incomplete(self) -> None:
        stream = (
            "INSTRUMENTATION_RESULT: shortMsg=Failed to instantiate AndroidJUnit4ClassRunner\n"
            "INSTRUMENTATION_CODE: 0\n"
        )
        code, summary, _ = self.run_parser(stream)
        self.assertEqual(1, code)
        self.assertIn("stream_outcome=NO_TEST_CASES\n", summary)
        self.assertIn("tests=0\n", summary)

    def test_unexpected_identity_is_rejected(self) -> None:
        stream = (
            status(NOOP, NOOP_METHOD, 0)
            + status(NATIVE, NATIVE_METHOD, 0)
            + status("app.serea.android.UnexpectedTest", "other", 0)
            + "INSTRUMENTATION_CODE: -1\n"
        )
        code, summary, _ = self.run_parser(stream)
        self.assertEqual(1, code)
        self.assertIn("unexpected_identity_count=1\n", summary)

    def test_passing_test_statuses_with_non_success_completion_code_are_rejected(self) -> None:
        stream = (
            status(NOOP, NOOP_METHOD, 0)
            + status(NATIVE, NATIVE_METHOD, 0)
            + "INSTRUMENTATION_CODE: 0\n"
        )
        code, summary, report = self.run_parser(stream)
        self.assertEqual(1, code)
        self.assertIn("instrumentation_code=0\n", summary)
        self.assertIn("noop_result=PASS\n", summary)
        self.assertIn("native_result=PASS\n", summary)
        self.assertIn("instrumentation completion code must be -1", summary)
        self.assertIn("<testsuite", report)

    def test_reported_numtests_must_match_expected_identity_pair(self) -> None:
        stream = (
            status(NOOP, NOOP_METHOD, 0, "INSTRUMENTATION_STATUS: numtests=3\n")
            + status(NATIVE, NATIVE_METHOD, 0, "INSTRUMENTATION_STATUS: numtests=3\n")
            + "INSTRUMENTATION_CODE: -1\n"
        )
        code, summary, _ = self.run_parser(stream)
        self.assertEqual(1, code)
        self.assertIn("reported_numtests=3\n", summary)
        self.assertIn("numtests does not match", summary)


if __name__ == "__main__":
    unittest.main(verbosity=2)
