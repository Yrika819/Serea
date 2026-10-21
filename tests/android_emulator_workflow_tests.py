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

    def test_api26_evidence_and_kvm_setup_run_after_api35_image_failure(self) -> None:
        workflow = (ROOT / ".github/workflows/android.yml").read_text(encoding="utf-8")
        steps = {
            match.group(1): match.group(0)
            for match in re.finditer(
                r"^      - name: ([^\n]+)\n(?:(?!^      - name: ).*\n)*",
                workflow,
                flags=re.MULTILINE,
            )
        }
        always = "if: always() && !cancelled()"
        for name in (
            "Diagnose KVM before permission changes",
            "Apply official KVM udev permissions",
            "Initialize API-specific evidence as NOT_RUN",
            "Install API 26 emulator image",
            "Run API 26 Pixel emulator and all instrumentation tests",
        ):
            self.assertIn(name, steps)
            self.assertIn(always, steps[name], name)
        self.assertLess(
            workflow.index("- name: Apply official KVM udev permissions"),
            workflow.index("- name: Run API 26 Pixel emulator and all instrumentation tests"),
        )
        api35 = steps["Run API 35 Pixel emulator and all instrumentation tests"]
        self.assertNotIn(always, api35)
        combine = workflow.split("name: Combine API-specific required evidence", 1)[1].split(
            "\n      - name:", 1
        )[0]
        for required in (
            '[ "$api35" = PASS ]',
            '[ "$api26" = PASS ]',
            '[ "$API35_STEP_RESULT" = success ]',
            '[ "$API26_STEP_RESULT" = success ]',
            'echo "FAIL: API 35=$api35',
            "exit 1",
        ):
            self.assertIn(required, combine)

    def test_wrong_emulator_api_level_fails_closed_as_not_run(self) -> None:
        script = ROOT / "tools/run_android_instrumentation_campaign.sh"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            android = root / "android"
            fake_bin = root / "bin"
            fake_bin.mkdir()
            android.mkdir()
            evidence = android / "app/build/outputs/androidTest-results/api26"
            evidence.mkdir(parents=True)
            for name, contents in {
                "device-api-error.txt": "old error\n",
                "junit-verification.log": "old JUnit result\n",
                "process-death.log": "old process death result\n",
                "instrumentation-status.txt": "PASS\n",
                "process-death-status.txt": "PASS\n",
                "api-result.txt": "PASS\n",
            }.items():
                (evidence / name).write_text(contents, encoding="utf-8")
            (evidence / "junit").mkdir()
            (evidence / "junit/old.xml").write_text("old result", encoding="utf-8")
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
            self.assertNotEqual(0, result.returncode)
            self.assertEqual("35\n", (evidence / "device-api-level.txt").read_text())
            self.assertEqual("NOT_RUN\n", (evidence / "instrumentation-status.txt").read_text())
            self.assertEqual("NOT_RUN\n", (evidence / "process-death-status.txt").read_text())
            self.assertEqual("FAIL\n", (evidence / "api-result.txt").read_text())
            self.assertIn("Expected API 26, observed '35'.", (evidence / "device-api-error.txt").read_text())
            self.assertFalse((evidence / "junit-verification.log").exists())
            self.assertFalse((evidence / "process-death.log").exists())
            self.assertFalse((evidence / "junit").exists())
            self.assertIn("instrumentation=NOT_RUN process-death=NOT_RUN device-api=35", summary.read_text())

    def test_stale_junit_cleanup_failure_stops_campaign(self) -> None:
        script = ROOT / "tools/run_android_instrumentation_campaign.sh"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            android = root / "android"
            fake_bin = root / "bin"
            fake_bin.mkdir()
            android.mkdir()
            evidence = android / "app/build/outputs/androidTest-results/api26"
            evidence.mkdir(parents=True)
            (evidence / "instrumentation-status.txt").write_text("PASS\n", encoding="utf-8")
            (evidence / "process-death-status.txt").write_text("PASS\n", encoding="utf-8")
            (evidence / "device-api-level.txt").write_text("26\n", encoding="utf-8")
            (evidence / "api-result.txt").write_text("PASS\n", encoding="utf-8")
            (evidence / "junit").mkdir()
            (evidence / "junit/old.xml").write_text("old passing result", encoding="utf-8")
            rm = fake_bin / "rm"
            rm.write_text("#!/usr/bin/env bash\nexit 73\n", encoding="utf-8")
            rm.chmod(0o755)
            adb_marker = root / "adb-was-called"
            adb = fake_bin / "adb"
            adb.write_text(
                "#!/usr/bin/env bash\n"
                "touch \"$ADB_MARKER\"\n"
                "printf '26\\n'\n",
                encoding="utf-8",
            )
            adb.chmod(0o755)
            summary = root / "summary.md"
            environment = os.environ.copy()
            environment["PATH"] = f"{fake_bin}{os.pathsep}{environment['PATH']}"
            environment["GITHUB_STEP_SUMMARY"] = str(summary)
            environment["ADB_MARKER"] = str(adb_marker)
            result = subprocess.run(
                ["bash", str(script), "26"],
                cwd=android,
                env=environment,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertNotEqual(0, result.returncode)
            self.assertFalse(adb_marker.exists())
            self.assertEqual("NOT_RUN\n", (evidence / "instrumentation-status.txt").read_text())
            self.assertEqual("NOT_RUN\n", (evidence / "process-death-status.txt").read_text())
            self.assertEqual("NOT_RUN\n", (evidence / "device-api-level.txt").read_text())
            self.assertEqual("FAIL\n", (evidence / "api-result.txt").read_text())
            self.assertEqual("old passing result", (evidence / "junit/old.xml").read_text())
            self.assertIn("Failed to clear stale API 26 instrumentation evidence", (evidence / "device-api-error.txt").read_text())
            self.assertIn("instrumentation=NOT_RUN process-death=NOT_RUN device-api=NOT_RUN cleanup=FAIL", summary.read_text())


if __name__ == "__main__":
    unittest.main()
