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
        for name in (
            "assert_disposable_android_emulator.sh",
            "run_android_data_sentinel.sh",
            "prepare_isolated_instrumentation_target.sh",
            "run_android_process_death_harness.sh",
        ):
            subprocess.run(["bash", "-n", str(ROOT / "tools" / name)], check=True)
        self.assertIn("connectedIsolatedAndroidTest", script.read_text(encoding="utf-8"))
        self.assertNotIn("connectedDebugAndroidTest", script.read_text(encoding="utf-8"))
        self.assertRegex(
            workflow,
            r"android/\*\|[^\n]*tools/run_android_process_death_harness\.sh[^\n]*android_changed=true",
        )
        self.assertIn("tools/prepare_isolated_instrumentation_target.sh", workflow)
        for step_name in (
            "Run API 35 Pixel emulator and all instrumentation tests",
            "Run API 26 Pixel emulator and all instrumentation tests",
        ):
            self.assertIn("SEREA_DISPOSABLE_CI_EMULATOR: '1'", workflow.split(step_name, 1)[1].split("\n      - name:", 1)[0])


    def test_direct_instrumentation_requires_fresh_isolated_campaign_before_tests(self) -> None:
        runner = (ROOT / "android/app/src/androidTest/java/app/serea/android/SereaIsolatedAndroidJUnitRunner.kt").read_text(encoding="utf-8")
        self.assertIn('target.packageName == ISOLATED_PACKAGE', runner)
        self.assertIn('Build.VERSION.SDK_INT == 26 || Build.VERSION.SDK_INT == 35', runner)
        self.assertIn('hardware in setOf("ranchu", "goldfish")', runner)
        self.assertIn('campaignMarkerMatches', runner)
        self.assertIn('noBackupFiles == setOf(INSTRUMENTATION_MARKER)', runner)
        self.assertIn('testBuildType = "isolated"', (ROOT / "android/app/build.gradle.kts").read_text(encoding="utf-8"))
        self.assertIn("assembleIsolatedAndroidTest", (ROOT / ".github/workflows/android.yml").read_text(encoding="utf-8"))
        self.assertIn('marker.renameTo(claimedMarker)', runner)
        for diagnostic in (
            'preferencesEmpty=$preferencesEmpty',
            'filesEmpty=$filesEmpty',
            'databasesEmpty=$databasesEmpty',
            'markerMatches=$campaignMarkerMatches',
            'noBackupEntries=${noBackupFiles?.sorted()}',
        ):
            self.assertIn(diagnostic, runner)
        self.assertLess(runner.index('check(preferencesEmpty'), runner.index('super.onStart()'))
        self.assertLess(runner.index('marker.renameTo(claimedMarker)'), runner.index('super.onStart()'))
        self.assertIn('connectedIsolatedAndroidTest', (ROOT / "tools/run_android_instrumentation_campaign.sh").read_text(encoding="utf-8"))
        build = (ROOT / "android/app/build.gradle.kts").read_text(encoding="utf-8")
        self.assertIn('add("isolatedImplementation", "androidx.compose.ui:ui-test-manifest")', build)
        self.assertNotIn('androidTestImplementation("androidx.compose.ui:ui-test-manifest")', build)

    def test_process_death_reinstalls_only_isolated_target_after_utp_cleanup(self) -> None:
        campaign = (ROOT / "tools/run_android_instrumentation_campaign.sh").read_text(encoding="utf-8")
        harness = (ROOT / "tools/run_android_process_death_harness.sh").read_text(encoding="utf-8")
        reinstall = campaign.index('Instrumentation runner uninstalled the isolated target')
        reseed = campaign.index('prepare_isolated_instrumentation_target.sh', reinstall)
        readiness = campaign.index('process_death_target_ready=false', reseed)
        process_death = campaign.index('if [[ "$sdk_ok" == true && "$data_isolation_result" == PASS && "$process_death_target_ready" == true ]]')
        self.assertLess(reinstall, reseed)
        self.assertLess(reseed, readiness)
        self.assertLess(readiness, process_death)
        self.assertIn('adb install "$isolated_apk"', campaign[reinstall:reseed])
        self.assertNotIn('pm clear', campaign.lower())
        self.assertNotIn('pm clear', harness.lower())
        self.assertNotIn('pm path "${package}.test"', harness)

    def test_sentinel_rejects_stale_pass_and_current_failure_wins(self) -> None:
        script = (ROOT / "tools/run_android_data_sentinel.sh").read_text(encoding="utf-8")
        receiver = (ROOT / "android/app/src/debug/java/app/serea/android/ProcessDeathHarnessReceiver.kt").read_text(encoding="utf-8")
        self.assertIn('request_id="sentinel-${BASHPID}-$(date +%s%N)-${RANDOM}"', script)
        self.assertIn('--es requestId "$request_id"', script)
        self.assertIn('am broadcast -f 0x20 -n "$receiver"', script)
        self.assertIn('adb logcat -d -s SereaProcessDeath:V', script)
        fail_check = script.index('FAIL:$token:${phase}-sentinel:$request_id:')
        pass_check = script.index('PASS:$token:${phase}-sentinel:$request_id')
        self.assertLess(fail_check, pass_check)
        self.assertIn('PASS:$token:$phase:$requestId', receiver)
        self.assertIn('FAIL:$token:$phase:$requestId:', receiver)


    def test_sentinel_does_not_accept_stale_success_for_a_failed_current_request(self) -> None:
        script = ROOT / "tools/run_android_data_sentinel.sh"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            request_file = root / "request-id"
            adb = fake_bin / "adb"
            adb.write_text(
                "#!/usr/bin/env bash\n"
                "if [[ \"$1\" == devices ]]; then\n"
                "  printf 'List of devices attached\\nemulator-5554 device product:ci model:ci\\n'\n"
                "elif [[ \"$1 \" == 'shell ' && \"$2 \" == 'getprop ' ]]; then\n"
                "  if [[ \"$3\" == ro.build.version.sdk ]]; then printf '35\\n'; else printf '1\\n'; fi\n"
                "elif [[ \"$1 \" == 'shell ' && \"$2 \" == 'pm ' && \"$3\" == path ]]; then\n"
                "  printf 'package:/data/app/demo.apk\\n'\n"
                "elif [[ \"$1 \" == 'shell ' && \"$2 \" == 'am ' ]]; then\n"
                "  while (($#)); do if [[ \"$1\" == requestId ]]; then printf '%s' \"$2\" > \"$REQUEST_FILE\"; fi; shift; done\n"
                "elif [[ \"$1\" == logcat ]]; then\n"
                "  request=$(cat \"$REQUEST_FILE\")\n"
                "  printf 'I/SereaProcessDeath: PASS:sentinel-token:verify-sentinel:stale-request\\n'\n"
                "  printf 'E/SereaProcessDeath: FAIL:sentinel-token:verify-sentinel:%s:IllegalStateException\\n' \"$request\"\n"
                "fi\n",
                encoding="utf-8",
            )
            adb.chmod(0o755)
            environment = os.environ.copy()
            environment.update({
                "PATH": f"{fake_bin}{os.pathsep}{environment['PATH']}",
                "CI": "true",
                "GITHUB_ACTIONS": "true",
                "SEREA_DISPOSABLE_CI_EMULATOR": "1",
                "EXPECTED_API_LEVEL": "35",
                "REQUEST_FILE": str(request_file),
            })
            result = subprocess.run(
                ["bash", str(script), "sentinel-token", "verify"],
                env=environment,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertNotEqual(0, result.returncode)
            self.assertIn("FAIL:sentinel-token:verify-sentinel:", result.stderr)
            self.assertNotIn("sentinel verify verified", result.stdout)


    def test_process_death_phases_require_a_fresh_request_result(self) -> None:
        script = (ROOT / "tools/run_android_process_death_harness.sh").read_text(encoding="utf-8")
        helper = (ROOT / "tools/prepare_isolated_instrumentation_target.sh").read_text(encoding="utf-8")
        self.assertIn('request_id="process-${BASHPID}-$(date +%s%N)-${RANDOM}"', script)
        self.assertIn('--es requestId "$request_id"', script)
        self.assertIn('am broadcast -f 0x20 -n "$receiver"', script)
        self.assertIn('adb logcat -d -s SereaProcessDeath:V', script)
        self.assertIn('FAIL:$phase_token:$phase_name:$request_id:', script)
        self.assertIn('PASS:$phase_token:$phase_name:$request_id', script)
        self.assertLess(
            script.index('FAIL:$phase_token:$phase_name:$request_id:'),
            script.index('PASS:$phase_token:$phase_name:$request_id'),
        )
        self.assertIn('--es requestId "$request_id"', helper)
        self.assertIn('am broadcast -f 0x20 -n "$receiver"', helper)
        self.assertIn('adb logcat -d -s SereaProcessDeath:V', helper)
        self.assertIn('PASS:$token:campaign-seed:$request_id', helper)

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
        self.assertNotRegex(api35, r"(?m)^\s+if:")
        combine = workflow.split("name: Combine API-specific required evidence", 1)[1].split(
            "\n      - name:", 1
        )[0]
        self.assertIn(
            'if [ "$api35" = PASS ] && [ "$api26" = PASS ] '
            '&& [ "$API35_STEP_RESULT" = success ] && [ "$API26_STEP_RESULT" = success ]; then',
            combine,
        )
        for required in (
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
                "if [[ \"$1\" == devices ]]; then\n"
                "  printf 'List of devices attached\\nemulator-5554 device product:ci model:ci\\n'\n"
                "elif [[ \"$1\" == shell && \"$2 $3 $4\" == 'getprop ro.build.version.sdk ' ]]; then\n"
                "  printf '35\\n'\n"
                "elif [[ \"$1\" == shell && \"$2 $3 $4\" == 'getprop ro.kernel.qemu ' ]]; then\n"
                "  printf '1\\n'\n"
                "fi\n",
                encoding="utf-8",
            )
            adb.chmod(0o755)
            summary = root / "summary.md"
            environment = os.environ.copy()
            environment["PATH"] = f"{fake_bin}{os.pathsep}{environment['PATH']}"
            environment["CI"] = "true"
            environment["GITHUB_ACTIONS"] = "true"
            environment["SEREA_DISPOSABLE_CI_EMULATOR"] = "1"
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
            self.assertEqual("NOT_RUN\n", (evidence / "data-isolation-status.txt").read_text())
            self.assertEqual("FAIL\n", (evidence / "api-result.txt").read_text())
            self.assertIn("Expected API 26, observed '35'.", (evidence / "device-api-error.txt").read_text())
            self.assertFalse((evidence / "junit-verification.log").exists())
            self.assertFalse((evidence / "process-death.log").exists())
            self.assertFalse((evidence / "junit").exists())
            self.assertIn("instrumentation=NOT_RUN process-death=NOT_RUN data-isolation=NOT_RUN device-api=35", summary.read_text())

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

    def test_data_mutating_guard_rejects_a_physical_device(self) -> None:
        guard = ROOT / "tools/assert_disposable_android_emulator.sh"
        with tempfile.TemporaryDirectory() as temporary:
            fake_bin = Path(temporary) / "bin"
            fake_bin.mkdir()
            adb = fake_bin / "adb"
            adb.write_text(
                "#!/usr/bin/env bash\n"
                "if [[ \"$1\" == devices ]]; then\n"
                "  printf 'List of devices attached\\nPixel7a serial device product:lynx model:Pixel_7a\\n'\n"
                "else\n"
                "  touch \"$ADB_MUTATION_MARKER\"\n"
                "fi\n",
                encoding="utf-8",
            )
            adb.chmod(0o755)
            marker = Path(temporary) / "unexpected-adb-command"
            environment = os.environ.copy()
            environment["PATH"] = f"{fake_bin}{os.pathsep}{environment['PATH']}"
            environment.update({
                "CI": "true",
                "GITHUB_ACTIONS": "true",
                "SEREA_DISPOSABLE_CI_EMULATOR": "1",
                "EXPECTED_API_LEVEL": "35",
                "ADB_MUTATION_MARKER": str(marker),
            })
            result = subprocess.run(["bash", str(guard)], env=environment, capture_output=True, text=True, check=False)
            self.assertNotEqual(0, result.returncode)
            self.assertIn("Refusing non-emulator", result.stderr)
            self.assertFalse(marker.exists())

    def test_campaign_refuses_app_data_retained_by_uninstall_keep_data(self) -> None:
        script = ROOT / "tools/run_android_instrumentation_campaign.sh"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            android = root / "android"
            fake_bin = root / "bin"
            fake_bin.mkdir()
            android.mkdir()
            calls = root / "adb-calls"
            adb = fake_bin / "adb"
            adb.write_text(
                "#!/usr/bin/env bash\n"
                "printf '%s\\n' \"$*\" >> \"$ADB_CALLS\"\n"
                "if [[ \"$1\" == devices ]]; then\n"
                "  printf 'List of devices attached\\nemulator-5554 device product:ci model:ci\\n'\n"
                "elif [[ \"$1\" == shell && \"$2\" == getprop && \"$3\" == ro.build.version.sdk ]]; then\n"
                "  printf '%s\\n' \"$ANDROID_TEST_API\"\n"
                "elif [[ \"$1\" == shell && \"$2\" == getprop && \"$3\" == ro.kernel.qemu ]]; then\n"
                "  printf '1\\n'\n"
                "elif [[ \"$1 $2 $3 $4\" == 'shell pm list packages' && \"$5\" == -u ]]; then\n"
                "  printf '%s' \"$ADB_PACKAGE_INVENTORY\"\n"
                "  exit \"${ADB_PACKAGE_EXIT_STATUS:-0}\"\n"
                "fi\n",
                encoding="utf-8",
            )
            adb.chmod(0o755)
            summary = root / "summary.md"
            environment = os.environ.copy()
            environment.update({
                "PATH": f"{fake_bin}{os.pathsep}{environment['PATH']}",
                "CI": "true",
                "GITHUB_ACTIONS": "true",
                "SEREA_DISPOSABLE_CI_EMULATOR": "1",
                "EXPECTED_API_LEVEL": "35",
                "GITHUB_RUN_ID": "12345",
                "GITHUB_RUN_ATTEMPT": "1",
                "GITHUB_STEP_SUMMARY": str(summary),
                "ADB_CALLS": str(calls),
                "ANDROID_TEST_API": "35",
                "ADB_PACKAGE_INVENTORY": "package:app.serea.android.demo\n",
            })
            environment.pop("ADB_PACKAGE_EXIT_STATUS", None)
            result = subprocess.run(
                ["bash", str(script), "35"],
                cwd=android,
                env=environment,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertNotEqual(0, result.returncode)
            adb_calls = calls.read_text(encoding="utf-8")
            self.assertIn("shell pm list packages -u --user all", adb_calls)
            self.assertNotIn("install ", adb_calls)
            self.assertNotIn("shell pm path", adb_calls)
            evidence = android / "app/build/outputs/androidTest-results/api35"
            self.assertIn(
                "retained Serea app data",
                (evidence / "data-isolation-error.txt").read_text(encoding="utf-8"),
            )
            self.assertEqual("FAIL\n", (evidence / "data-isolation-status.txt").read_text(encoding="utf-8"))
            self.assertEqual("NOT_RUN\n", (evidence / "process-death-status.txt").read_text(encoding="utf-8"))

            calls.write_text("", encoding="utf-8")
            environment["ANDROID_TEST_API"] = "26"
            environment["EXPECTED_API_LEVEL"] = "26"
            api26_inventory = subprocess.run(
                ["bash", str(script), "26"],
                cwd=android,
                env=environment,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertNotEqual(0, api26_inventory.returncode)
            api26_calls = calls.read_text(encoding="utf-8")
            self.assertIn("shell pm list packages -u\n", api26_calls)
            self.assertNotIn("--user all", api26_calls)
            self.assertNotIn("install ", api26_calls)
            self.assertNotIn("shell pm path", api26_calls)
            evidence26 = android / "app/build/outputs/androidTest-results/api26"
            self.assertIn(
                "retained Serea app data",
                (evidence26 / "data-isolation-error.txt").read_text(encoding="utf-8"),
            )

            calls.write_text("", encoding="utf-8")
            environment["ANDROID_TEST_API"] = "35"
            environment["EXPECTED_API_LEVEL"] = "35"
            environment["ADB_PACKAGE_INVENTORY"] = ""
            empty_inventory = subprocess.run(
                ["bash", str(script), "35"],
                cwd=android,
                env=environment,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertNotEqual(0, empty_inventory.returncode)
            empty_calls = calls.read_text(encoding="utf-8")
            self.assertIn("shell pm list packages -u --user all", empty_calls)
            self.assertNotIn("install ", empty_calls)
            self.assertNotIn("shell pm path", empty_calls)
            self.assertIn(
                "empty or malformed",
                (evidence / "data-isolation-error.txt").read_text(encoding="utf-8"),
            )

            calls.write_text("", encoding="utf-8")
            environment["ADB_PACKAGE_INVENTORY"] = "Error: package service unavailable\n"
            malformed_inventory = subprocess.run(
                ["bash", str(script), "35"],
                cwd=android,
                env=environment,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertNotEqual(0, malformed_inventory.returncode)
            malformed_calls = calls.read_text(encoding="utf-8")
            self.assertIn("shell pm list packages -u --user all", malformed_calls)
            self.assertNotIn("install ", malformed_calls)
            self.assertNotIn("shell pm path", malformed_calls)
            self.assertIn(
                "empty or malformed",
                (evidence / "data-isolation-error.txt").read_text(encoding="utf-8"),
            )

            calls.write_text("", encoding="utf-8")
            environment["ADB_PACKAGE_INVENTORY"] = ""
            environment["ADB_PACKAGE_EXIT_STATUS"] = "1"
            failed_inventory = subprocess.run(
                ["bash", str(script), "35"],
                cwd=android,
                env=environment,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertNotEqual(0, failed_inventory.returncode)
            failed_calls = calls.read_text(encoding="utf-8")
            self.assertIn("shell pm list packages -u --user all", failed_calls)
            self.assertNotIn("install ", failed_calls)
            self.assertNotIn("shell pm path", failed_calls)
            self.assertIn(
                "Unable to prove",
                (evidence / "data-isolation-error.txt").read_text(encoding="utf-8"),
            )

    def test_sentinel_seeding_requires_a_fresh_complete_debug_data_namespace(self) -> None:
        receiver = (ROOT / "android/app/src/debug/java/app/serea/android/ProcessDeathHarnessReceiver.kt").read_text(
            encoding="utf-8",
        )
        seed_phase = receiver.split('if (phase == "seed-sentinel") {', 1)[1].split("}", 1)[0]
        verify_phase = receiver.split('if (phase == "verify-sentinel") {', 1)[1].split("}", 1)[0]
        fresh_namespace_check = receiver.split("private fun assertFreshDebugDataNamespace(", 1)[1].split(
            "private fun assertDebugSentinelAuthorization(", 1,
        )[0]
        verify_authorization = receiver.split("private fun assertDebugSentinelAuthorization(", 1)[1].split(
            "private fun isAbsentOrEmptyDirectory(", 1,
        )[0]
        seed = receiver.split("private suspend fun seedDebugSentinel(", 1)[1].split(
            "private suspend fun verifyDebugSentinel(", 1,
        )[0]
        runner = (ROOT / "android/app/src/androidTest/java/app/serea/android/SereaIsolatedAndroidJUnitRunner.kt").read_text(
            encoding="utf-8",
        )
        self.assertIn("assertFreshDebugDataNamespace(context)", seed_phase)
        self.assertLess(receiver.index('if (phase == "seed-sentinel")'), receiver.index("val database = SereaHistoryDatabase.open(context)"))
        self.assertIn("assertDebugSentinelAuthorization(context, token)", verify_phase)
        self.assertLess(receiver.index('if (phase == "verify-sentinel")'), receiver.index("val database = SereaHistoryDatabase.open(context)"))
        self.assertIn("context.databaseList().isEmpty()", fresh_namespace_check)
        self.assertIn('File(context.applicationInfo.dataDir, "databases")', fresh_namespace_check)
        self.assertIn('context.getDatabasePath("serea-history.db")', fresh_namespace_check)
        self.assertIn("isAbsentOrEmptyDirectory(databaseDirectory)", fresh_namespace_check)
        self.assertIn('File(context.applicationInfo.dataDir, "shared_prefs")', fresh_namespace_check)
        self.assertIn("context.filesDir", fresh_namespace_check)
        self.assertIn("context.noBackupFilesDir", fresh_namespace_check)
        self.assertIn("context.cacheDir", fresh_namespace_check)
        self.assertIn("context.codeCacheDir", fresh_namespace_check)
        self.assertIn("database.isFile && database.length() > 0L", verify_authorization)
        self.assertIn("marker.isFile && marker.readText() == token", verify_authorization)
        self.assertIn('selection.manualModelId == "sentinel/$token"', verify_authorization)
        self.assertIn("DEBUG_SENTINEL_AUTHORIZATION", seed)
        self.assertIn("directory.listFiles()?.isEmpty() == true", receiver)
        self.assertIn("isAbsentOrEmptyDirectory(preferences)", runner)
        self.assertIn("isAbsentOrEmptyDirectory(target.filesDir)", runner)
        self.assertIn("isAbsentOrEmptyDirectory(databaseDirectory)", runner)
        self.assertIn('target.getDatabasePath("serea-history.db").exists()', runner)
        self.assertNotIn("listFiles().isNullOrEmpty()", runner)
        self.assertLess(seed.index("preferences.all.isEmpty()"), seed.index("repository.list()"))


if __name__ == "__main__":
    unittest.main()
