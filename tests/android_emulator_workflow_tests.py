from __future__ import annotations

import re
import os
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class AndroidEmulatorWorkflowTests(unittest.TestCase):
    def test_api_scripts_are_single_process_commands(self) -> None:
        workflow = (ROOT / ".github/workflows/android.yml").read_text(encoding="utf-8")
        scripts = re.findall(r"^[ \t]+script: (.+)$", workflow, flags=re.MULTILINE)
        self.assertEqual(
            [
                "bash ../tools/run_android_instrumentation_campaign.sh 35",
                "bash ../tools/run_android_instrumentation_campaign.sh 26",
            ],
            scripts,
        )
        script = ROOT / "tools/run_android_instrumentation_campaign.sh"
        self.assertTrue(script.is_file())
        subprocess.run(["bash", "-n", str(script)], check=True)

    def test_wrong_emulator_api_level_fails_closed_as_not_run(self) -> None:
        script = ROOT / "tools/run_android_instrumentation_campaign.sh"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            android = root / "android"
            fake_bin = root / "bin"
            fake_bin.mkdir()
            android.mkdir()
            adb = fake_bin / "adb"
            adb.write_text(
                "#!/usr/bin/env bash\n"
                "if [[ \"$1 $2 $3 $4\" == \"shell getprop ro.build.version.sdk \" ]]; then\n"
                "  printf '35\\n'\n"
                "fi\n",
                encoding="utf-8",
            )
            adb.chmod(0o755)
            summary = root / "summary.md"
            environment = os.environ.copy()
            environment["PATH"] = f"{fake_bin}{os.pathsep}{environment['PATH']}"
            environment["GITHUB_STEP_SUMMARY"] = str(summary)
            result = subprocess.run(
                ["bash", str(script), "26"],
                cwd=android,
                env=environment,
                capture_output=True,
                text=True,
                check=False,
            )
            evidence = android / "app/build/outputs/androidTest-results/api26"
            self.assertNotEqual(0, result.returncode)
            self.assertEqual("35\n", (evidence / "device-api-level.txt").read_text())
            self.assertEqual("NOT_RUN\n", (evidence / "instrumentation-status.txt").read_text())
            self.assertEqual("NOT_RUN\n", (evidence / "process-death-status.txt").read_text())
            self.assertEqual("FAIL\n", (evidence / "api-result.txt").read_text())
            self.assertIn("Expected API 26, observed '35'.", (evidence / "device-api-error.txt").read_text())
            self.assertIn("instrumentation=NOT_RUN process-death=NOT_RUN device-api=35", summary.read_text())


if __name__ == "__main__":
    unittest.main()
