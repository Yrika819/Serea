#!/usr/bin/env python3
"""Tests that metric summarization is bounded and never copies raw log text."""

import subprocess
import sys
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("summarize-runtime-metrics.py")


class RuntimeMetricSummaryTest(unittest.TestCase):
    def run_summary(self, payload: str) -> tuple[int, str, str]:
        result = subprocess.run(
            [sys.executable, str(SCRIPT)],
            input=payload,
            check=False,
            capture_output=True,
            text=True,
        )
        return result.returncode, result.stdout, result.stderr

    def test_emits_only_whitelisted_integer_metrics(self) -> None:
        payload = (
            "I SereaLocalNativeRuntime: prompt_private_text=must-not-appear\n"
            "I SereaLocalNativeRuntime: metric_summary=pss_measurement_status=PASS,"
            "load_before_kb=100,load_after_kb=200,"
            "inference_1_before_kb=210,inference_1_after_kb=220,inference_2_before_kb=215,"
            "inference_2_after_kb=225,main_release_pss_kb=150,cancelled_release_pss_kb=160,"
            "cancel_to_release_ms=321\n"
        )
        code, output, _ = self.run_summary(payload)
        self.assertEqual(0, code)
        self.assertIn("load_after_kb=200\n", output)
        self.assertIn("cancel_to_release_ms=321\n", output)
        self.assertNotIn("prompt_private_text", output)
        self.assertNotIn("must-not-appear", output)

    def test_missing_pss_samples_are_explicitly_not_available(self) -> None:
        payload = (
            "metric_summary=pss_measurement_status=NOT_AVAILABLE,"
            "load_before_kb=NOT_AVAILABLE,load_after_kb=NOT_AVAILABLE,"
            "inference_1_before_kb=NOT_AVAILABLE,inference_1_after_kb=NOT_AVAILABLE,"
            "inference_2_before_kb=NOT_AVAILABLE,inference_2_after_kb=NOT_AVAILABLE,"
            "main_release_pss_kb=NOT_AVAILABLE,cancelled_release_pss_kb=NOT_AVAILABLE,"
            "cancel_to_release_ms=321\n"
        )
        code, output, _ = self.run_summary(payload)
        self.assertEqual(0, code)
        self.assertIn("pss_measurement_status=NOT_AVAILABLE\n", output)
        self.assertIn("load_after_kb=NOT_AVAILABLE\n", output)

    def test_missing_metric_fails_closed_without_copying_input(self) -> None:
        code, output, _ = self.run_summary("metric_summary=load_before_kb=4\nsecret=DO_NOT_PRINT\n")
        self.assertNotEqual(0, code)
        self.assertEqual("", output)

    def test_input_bound_is_enforced(self) -> None:
        code, output, _ = self.run_summary("x" * (262_144 + 1))
        self.assertEqual(2, code)
        self.assertEqual("", output)


if __name__ == "__main__":
    unittest.main(verbosity=2)
