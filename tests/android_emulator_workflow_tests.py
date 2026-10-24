from __future__ import annotations

import re
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

from tools.summarize_android_test_lifecycle import summarize


ROOT = Path(__file__).resolve().parents[1]


class AndroidEmulatorWorkflowTests(unittest.TestCase):
    def test_lifecycle_summary_links_failures_ui_state_and_process_death(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            evidence = Path(temporary) / "api35"
            stages = evidence / "stages" / "calendar-target"
            stages.mkdir(parents=True)
            log = evidence / "logcat-live.txt"
            evidence.joinpath("test-lifecycle-host.txt").write_text(
                "api_level=35\n"
                "campaign_token=serea-isolated-api35-run-attempt\n"
                "instrumentation_started_at_utc=2026-10-22T11:00:00.000Z\n"
                "instrumentation_finished_at_utc=2026-10-22T11:00:00.300Z\n",
                encoding="utf-8",
            )
            evidence.joinpath("instrumentation-gradle-exit-code.txt").write_text("0\n", encoding="utf-8")
            evidence.joinpath("instrumentation-verification-exit-code.txt").write_text("1\n", encoding="utf-8")
            evidence.joinpath("instrumentation-status.txt").write_text("FAIL\n", encoding="utf-8")
            evidence.joinpath("diagnostic-status.txt").write_text("PASS\n", encoding="utf-8")
            evidence.joinpath("process-death-status.txt").write_text("PASS\n", encoding="utf-8")
            evidence.joinpath("process-death-host-lifecycle.txt").write_text(
                "process_death_started_at_utc=2026-10-22T11:00:01Z\n"
                "process_death_finished_at_utc=2026-10-22T11:00:02Z\n"
                "process_death_exit_code=0\n",
                encoding="utf-8",
            )
            stages.joinpath("host-lifecycle.txt").write_text(
                "stage=calendar-target\n"
                "test_filter=app.serea.android.SereaAppShellTest#calendarDetailAndSelectedCalendarSurviveAnotherDestination\n"
                "stage_started_at_utc=2026-10-22T10:59:59Z\n"
                "stage_finished_at_utc=2026-10-22T11:00:00.500Z\n",
                encoding="utf-8",
            )
            stages.joinpath("gradle-exit-code.txt").write_text("1\n", encoding="utf-8")
            stages.joinpath("verification-exit-code.txt").write_text("1\n", encoding="utf-8")
            stages.joinpath("status.txt").write_text("FAIL\n", encoding="utf-8")
            log.write_text(
                "10-22 11:00:00.000  42  42 I/SereaTestLifecycle(  42): "
                "START|1792666800000|app.serea.android.ChatScreenTest#deleteCurrent\n"
                "10-22 11:00:00.100  42  42 I/chat-current-delete-confirmation(  42): "
                "Delete this conversation?\n"
                "10-22 11:00:00.200  42  42 I/SereaTestLifecycle(  42): "
                "FAIL|1792666800200|app.serea.android.ChatScreenTest#deleteCurrent|AssertionError\n"
                "10-22 11:00:00.300  42  42 I/SereaTestLifecycle(  42): "
                "FINISH|1792666800300|app.serea.android.ChatScreenTest#deleteCurrent\n"
                "10-22 11:00:01.000  42  42 I/SereaProcessDeath(  42): "
                "PASS:campaign-token:prepare:request-1\n",
                encoding="utf-8",
            )
            report = summarize(log, evidence)
            self.assertIn("campaign_api_level=35", report)
            self.assertIn("campaign_token=serea-isolated-api35-run-attempt", report)
            self.assertIn("test_event.1.window=diagnostic:calendar-target", report)
            self.assertIn("test_failure.1=app.serea.android.ChatScreenTest#deleteCurrent", report)
            self.assertIn("test_failure.1.type=AssertionError", report)
            self.assertIn("ui_state_event.1.test=app.serea.android.ChatScreenTest#deleteCurrent", report)
            self.assertIn("ui_state_event.1.tag=chat-current-delete-confirmation", report)
            self.assertIn("process_death_event.1.outcome=PASS", report)
            self.assertIn("process_death_event.1.phase=prepare", report)
            self.assertIn("process_death_event.1.logcat_time=10-22 11:00:01.000", report)
            self.assertIn("instrumentation_verification_exit_code=1", report)
            self.assertIn("process_death_status=PASS", report)
            self.assertIn("diagnostic_stage.1.gradle_exit_code=1", report)
            self.assertIn("diagnostic_stage.1.verification_exit_code=1", report)

    def test_instrumentation_stage_results_are_independent_and_stale_evidence_is_removed(self) -> None:
        campaign = (ROOT / "tools/run_android_instrumentation_campaign.sh").read_text(encoding="utf-8")
        self.assertIn('"$evidence/stages"', campaign)
        self.assertRegex(campaign, r"rm -rf[^\n]*\$evidence/stages")
        self.assertIn('verification-exit-code.txt', campaign)
        self.assertIn('instrumentation-verification-exit-code.txt', campaign)
        self.assertIn('stage_dir/gradle-exit-code.txt', campaign)
        self.assertIn('stage_dir/verification-exit-code.txt', campaign)
        self.assertIn('stage_dir/gradle.log', campaign)
        stage_body = campaign.split("run_diagnostic_stage() {", 1)[1].split("\n}\n\ncleanup_failed()", 1)[0]
        self.assertIn('if [[ -d "$stage_dir/junit" ]]; then', stage_body)
        self.assertIn('if [[ "$gradle_status" -eq 0 && "$verify_exit_code" -eq 0 ]]', stage_body)
        self.assertIn('if [[ "$gradle_status" -ne 0 || "$verify_status" -ne 0 ]]', stage_body)

    def test_pr82_dependency_seed_and_process_death_gates_remain_fail_closed(self) -> None:
        campaign = (ROOT / "tools/run_android_instrumentation_campaign.sh").read_text(encoding="utf-8")
        self.assertNotRegex(campaign, r"(^|\s)rg(\s|$)")
        self.assertIn('sentinel_seed_result" -eq 0 && -z "$(adb shell pm path "$isolated_package"', campaign)
        self.assertIn('[[ "$sdk_ok" == true && "$data_isolation_result" == PASS && "$process_death_target_ready" == true ]]', campaign)
        self.assertIn('"$data_isolation_result" == PASS', campaign)
        self.assertIn('--minimum 105', campaign)
        self.assertIn('timeout --signal=INT --kill-after=120s 40m ./gradlew', campaign)

    def test_api_instrumentation_evidence_has_distinct_artifact_names(self) -> None:
        workflow = (ROOT / ".github/workflows/android.yml").read_text(encoding="utf-8")
        for api in ("26", "35"):
            self.assertIn(f"name: serea-android-instrumentation-api{api}", workflow)
            self.assertRegex(
                workflow,
                rf"name: serea-android-instrumentation-api{api}[\s\S]{{0,500}}"
                rf"path:[^\n]*androidTest-results/api{api}/",
            )
            self.assertIn(f"api{api}_evidence_artifact=serea-android-instrumentation-api{api}", workflow)

    def test_api_artifact_split_preserves_combined_evidence_layout(self) -> None:
        workflow = (ROOT / ".github/workflows/android.yml").read_text(encoding="utf-8")
        self.assertIn('cp -R android/app/build/outputs/androidTest-results/api35 "$bundle/tests/api35"', workflow)
        self.assertIn('cp -R android/app/build/outputs/androidTest-results/api26 "$bundle/tests/api26"', workflow)
        self.assertIn('name: serea-android-emulator-evidence', workflow)

    def test_api_scripts_are_single_process_commands(self) -> None:
        workflow = (ROOT / ".github/workflows/android.yml").read_text(encoding="utf-8")
        scripts = re.findall(r"^[ \t]+script: (.+)$", workflow, flags=re.MULTILINE)
        self.assertEqual(
            [
                "bash ../tools/run_android_api35_campaign.sh",
                "bash ../tools/run_android_instrumentation_campaign.sh 26",
                "bash tools/test_personal_beta_identity.sh",
            ],
            scripts,
        )
        self.assertIn('build_checkout_commit=$(git rev-parse HEAD)', workflow)
        self.assertIn('build_checkout_commit=$GITHUB_SHA', workflow)
        self.assertIn('base_commit=$BASE_SHA', workflow)
        self.assertIn('github_run_attempt=$GITHUB_RUN_ATTEMPT', workflow)
        self.assertIn('based on `codex/pr71-beta-acceptance-final-20261021-v2`', workflow)
        script = ROOT / "tools/run_android_instrumentation_campaign.sh"
        self.assertTrue(script.is_file())
        subprocess.run(["bash", "-n", str(script)], check=True)
        for name in (
            "assert_disposable_android_emulator.sh",
            "run_android_data_sentinel.sh",
            "prepare_isolated_instrumentation_target.sh",
            "run_android_process_death_harness.sh",
            "test_android_update_compatibility.sh",
            "test_android_update_compatibility_guard.sh",
            "run_android_api35_campaign.sh",
        ):
            subprocess.run(["bash", "-n", str(ROOT / "tools" / name)], check=True)
        self.assertIn("connectedIsolatedAndroidTest", script.read_text(encoding="utf-8"))
        self.assertNotIn("connectedDebugAndroidTest", script.read_text(encoding="utf-8"))
        self.assertRegex(
            workflow,
            r"android/\*\|[^\n]*tools/run_android_process_death_harness\.sh[^\n]*android_changed=true",
        )
        self.assertIn("tools/prepare_isolated_instrumentation_target.sh", workflow)
        self.assertIn("Test Android update emulator guard with Mock ADB", workflow)
        self.assertIn("tools/test_android_update_compatibility_guard.sh", workflow)
        api35_campaign = (ROOT / "tools/run_android_api35_campaign.sh").read_text(encoding="utf-8")
        self.assertIn("run_android_instrumentation_campaign.sh\" 35", api35_campaign)
        self.assertIn("test_android_update_compatibility.sh", api35_campaign)
        self.assertIn("Save API 35 failure semantics and safety logcat", workflow)
        self.assertIn("Save API 26 failure semantics and safety logcat", workflow)
        self.assertEqual(2, workflow.count("live='android/app/build/outputs/androidTest-results/api"))
        self.assertEqual(2, workflow.count("' \"$live\" >> \"$output\""))
        self.assertIn("Emulator detached after test step; captured instrumentation logcat is preserved above.", workflow)
        self.assertIn("font-scale-gmail-heading", workflow)
        self.assertIn("chat-draft-gmail-search-timeout", workflow)
        self.assertLess(
            workflow.index("Save API 35 failure semantics and safety logcat"),
            workflow.index("Clear API 35 emulator state before API 26 campaign"),
        )
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
        profile_marker = (ROOT / "android/app/src/debug/java/app/serea/android/ProfileVerifierCacheMarker.kt").read_text(encoding="utf-8")
        workflow = (ROOT / ".github/workflows/android.yml").read_text(encoding="utf-8")
        self.assertIn('FILE_NAME = "profileInstalled"', profile_marker)
        self.assertIn('private const val SCHEMA = 1', profile_marker)
        self.assertIn('private const val BYTE_COUNT = 24', profile_marker)
        self.assertIn('storedPackageUpdateTime == packageLastUpdateTime', profile_marker)
        self.assertIn('private val installedProfileResultCodes = setOf(1, 2, 3)', profile_marker)
        self.assertIn('resultCode in installedProfileResultCodes', profile_marker)
        self.assertIn('Files.isSymbolicLink(markerPath)', profile_marker)
        self.assertIn('Files.isRegularFile(markerPath, LinkOption.NOFOLLOW_LINKS)', profile_marker)
        self.assertIn('channel.size() != BYTE_COUNT.toLong()', profile_marker)
        self.assertIn('Files.newByteChannel(path, setOf(StandardOpenOption.READ, LinkOption.NOFOLLOW_LINKS))', profile_marker)
        self.assertIn('channel.read(ByteBuffer.allocate(1)) != -1', profile_marker)
        self.assertIn('schema == SCHEMA', profile_marker)
        self.assertIn('resultCode in installedProfileResultCodes', profile_marker)
        self.assertIn('storedPackageUpdateTime == packageLastUpdateTime', profile_marker)
        self.assertIn('installedCurrentProfileSize >= 0L', profile_marker)
        self.assertIn('fun isValidMarker(filesDirectory: File, packageLastUpdateTime: Long)', profile_marker)
        self.assertIn('entries.size == 1 && entries.single().name == FILE_NAME', profile_marker)
        self.assertIn('ProfileVerifierCacheMarker.isOnlyValidMarker(filesDirectory, packageLastUpdateTime)', runner)
        self.assertIn('Build.VERSION.SDK_INT >= 28 && Build.VERSION.SDK_INT != 30', runner)
        self.assertIn('filesDirEntries=${filesDirectoryEntries?.sorted()}', runner)
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
        campaign = (ROOT / "tools/run_android_instrumentation_campaign.sh").read_text(encoding="utf-8")
        target_helper = (ROOT / "tools/prepare_isolated_instrumentation_target.sh").read_text(encoding="utf-8")
        self.assertIn('run-as "$package"', target_helper)
        self.assertIn('profileVerifierMarkerBeforeUtp=ABSENT', target_helper)
        self.assertIn('if [[ "$mode" == process-death ]]', target_helper)
        self.assertIn('am broadcast -f 0x20 -n "$receiver"', target_helper)
        self.assertIn('calendar-target', campaign)
        self.assertIn('calendar-class', campaign)
        self.assertIn('app.serea.android.SereaAppShellTest#calendarDetailAndSelectedCalendarSurviveAnotherDestination', campaign)
        self.assertIn('app.serea.android.SereaAppShellTest 900 19', campaign)
        self.assertIn('--require-test', campaign)
        self.assertIn('--exact-count', campaign)
        self.assertIn('SereaCalendarStage', campaign)
        self.assertIn('SereaProfileVerifier:I', campaign)
        self.assertIn('--api-level "$api_level" --profile-verifier-log "$evidence/logcat-live.txt"', campaign)
        self.assertIn('PROFILE_MARKER_CHECK', runner)
        self.assertIn('SereaInstrumentationLifecycleListener', campaign)
        self.assertIn('logcat-live.txt', campaign)
        self.assertIn('timeout --signal=INT --kill-after=120s 40m ./gradlew', campaign)
        self.assertIn('test-device-state.txt', campaign)
        verifier_test = (ROOT / "android/app/src/androidTest/java/app/serea/android/InstrumentationStorageIsolationTest.kt").read_text(encoding="utf-8")
        self.assertIn("ProfileVerifier.writeProfileVerification(target)", verifier_test)
        self.assertIn("RESULT_CODE_ERROR_UNSUPPORTED_API_VERSION", verifier_test)
        self.assertIn("SereaProfileVerifier", verifier_test)
        self.assertIn("marker=valid", verifier_test)
        self.assertIn("SereaProfileVerifier AndroidRuntime", workflow)
        build = (ROOT / "android/app/build.gradle.kts").read_text(encoding="utf-8")
        self.assertIn('add("isolatedImplementation", "androidx.compose.ui:ui-test-manifest")', build)
        self.assertNotIn('androidTestImplementation("androidx.compose.ui:ui-test-manifest")', build)
        manifest_lock = next(
            line for line in (ROOT / "android/app/gradle.lockfile").read_text(encoding="utf-8").splitlines()
            if line.startswith("androidx.compose.ui:ui-test-manifest:")
        )
        self.assertEqual(
            "androidx.compose.ui:ui-test-manifest:1.7.8=isolatedAndroidTestCompileClasspath,isolatedCompileClasspath,isolatedImplementationDependenciesMetadata,isolatedRuntimeClasspath,isolatedUnitTestCompileClasspath,isolatedUnitTestRuntimeClasspath",
            manifest_lock,
        )

    def test_host_seed_uses_stdin_and_fails_closed_on_uid_or_package_path(self) -> None:
        helper = ROOT / "tools/prepare_isolated_instrumentation_target.sh"
        source = helper.read_text(encoding="utf-8")
        self.assertNotIn("run-as \"$package\" sh -c", source)
        self.assertIn('adb shell run-as "$package" sh', source)
        self.assertIn('app_data_dir="$(adb shell run-as "$package" pwd', source)
        self.assertIn('app_uid="$(adb shell run-as "$package" id -u', source)
        self.assertIn('"$app_data_dir" != "/data/user/0/$package"', source)
        self.assertIn('"$app_data_dir" != "/data/data/$package"', source)
        self.assertIn('[ -w no_backup ] && [ -x no_backup ]', source)
        self.assertIn('[ -L "$directory" ]', source)
        self.assertIn('files/profileInstalled', source)
        self.assertIn('no_backup/serea-instrumentation-token', source)
        self.assertNotIn("app.serea.android.demo", source)

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            trace = root / "adb-trace"
            adb = fake_bin / "adb"
            adb.write_text(
                "#!/usr/bin/env bash\n"
                "if [[ \"$1\" == -s ]]; then shift 2; fi\n"
                "printf '%q ' \"$@\" >> \"$ADB_TRACE\"\n"
                "if [[ \"$1\" == devices ]]; then\n"
                "  printf 'List of devices attached\\nemulator-5554 device product:ci model:ci\\n'\n"
                "elif [[ \"$1\" == shell && \"$2\" == getprop && \"$3\" == ro.build.version.sdk ]]; then\n"
                "  printf '35\\n'\n"
                "elif [[ \"$1\" == shell && \"$2\" == getprop && \"$3\" == ro.kernel.qemu ]]; then\n"
                "  printf '1\\n'\n"
                "elif [[ \"$1\" == shell && \"$2\" == pm && \"$3\" == path ]]; then\n"
                "  printf 'package:/data/app/isolated.apk\\n'\n"
                "elif [[ \"$1\" == shell && \"$2\" == run-as && \"$4\" == id ]]; then\n"
                "  printf '%s\\n' \"${ADB_UID:-10142}\"\n"
                "elif [[ \"$1\" == shell && \"$2\" == run-as && \"$4\" == pwd ]]; then\n"
                "  printf '%s\\n' \"${ADB_DATA_DIR:-/data/user/0/app.serea.android.isolated}\"\n"
                "elif [[ \"$1\" == shell && \"$2\" == run-as && \"$4\" == sh ]]; then\n"
                "  cat > \"$SEED_STDIN\"\n"
                "fi\n",
                encoding="utf-8",
            )
            adb.chmod(0o755)
            environment = os.environ.copy()
            environment.update({
                "PATH": f"{fake_bin}{os.pathsep}{environment['PATH']}",
                "CI": "true",
                "GITHUB_ACTIONS": "true",
                "GITHUB_RUN_ID": "12345",
                "SEREA_DISPOSABLE_CI_EMULATOR": "1",
                "EXPECTED_API_LEVEL": "35",
                "ADB_TRACE": str(trace),
                "SEED_STDIN": str(root / "seed-stdin"),
            })

            invalid = subprocess.run(
                ["bash", str(helper), "bad token"], env=environment,
                capture_output=True, text=True, check=False,
            )
            self.assertNotEqual(0, invalid.returncode)
            self.assertFalse(trace.exists())

            wrong_path_env = environment | {"ADB_DATA_DIR": "/data/user/0/app.serea.android.demo"}
            wrong_path = subprocess.run(
                ["bash", str(helper), "campaign-token"], env=wrong_path_env,
                capture_output=True, text=True, check=False,
            )
            self.assertNotEqual(0, wrong_path.returncode)
            self.assertIn("not the target package data directory", wrong_path.stderr)
            self.assertFalse((root / "seed-stdin").exists())

            wrong_uid_env = environment | {"ADB_UID": "2000"}
            wrong_uid = subprocess.run(
                ["bash", str(helper), "campaign-token"], env=wrong_uid_env,
                capture_output=True, text=True, check=False,
            )
            self.assertNotEqual(0, wrong_uid.returncode)
            self.assertIn("unexpected UID", wrong_uid.stderr)
            self.assertFalse((root / "seed-stdin").exists())

            seeded = subprocess.run(
                ["bash", str(helper), "campaign-token"], env=environment,
                capture_output=True, text=True, check=False,
            )
            self.assertEqual(0, seeded.returncode, seeded.stderr)
            seed_program = (root / "seed-stdin").read_text(encoding="utf-8")
            self.assertIn("token='campaign-token'", seed_program)
            self.assertIn("expected_dir='/data/user/0/app.serea.android.isolated'", seed_program)
            self.assertIn("[ -L \"$directory\" ]", seed_program)
            self.assertIn("serea-instrumentation-token", seed_program)

    def test_process_death_reinstalls_only_isolated_target_after_utp_cleanup(self) -> None:
        campaign = (ROOT / "tools/run_android_instrumentation_campaign.sh").read_text(encoding="utf-8")
        harness = (ROOT / "tools/run_android_process_death_harness.sh").read_text(encoding="utf-8")
        receiver = (ROOT / "android/app/src/debug/java/app/serea/android/ProcessDeathHarnessReceiver.kt").read_text(encoding="utf-8")
        target_helper = (ROOT / "tools/prepare_isolated_instrumentation_target.sh").read_text(encoding="utf-8")
        readiness_initial = campaign.index('process_death_target_ready=false')
        fresh_target = campaign.index('Always create a new isolated namespace for Process Death')
        uninstall = campaign.index('adb uninstall "$isolated_package"', fresh_target)
        reseed = campaign.index('prepare_isolated_instrumentation_target.sh', fresh_target)
        readiness = campaign.index('process_death_target_ready=true', reseed)
        process_death = campaign.index('if [[ "$sdk_ok" == true && "$data_isolation_result" == PASS && "$process_death_target_ready" == true ]]')
        self.assertLess(readiness_initial, fresh_target)
        self.assertLess(fresh_target, uninstall)
        self.assertLess(reseed, readiness)
        self.assertLess(readiness, process_death)
        self.assertIn('adb install "$isolated_apk"', campaign[uninstall:reseed])
        self.assertIn('process-death-readiness-status.txt', campaign)
        self.assertIn("pm list packages -u", campaign)
        self.assertIn('"$reseed_result" -eq 0', campaign[reseed:readiness])
        self.assertNotIn('pm clear', campaign.lower())
        self.assertNotIn('pm clear', harness.lower())
        self.assertNotIn('pm path "${package}.test"', harness)
        self.assertIn(' process-death \\', campaign[reseed:readiness])
        self.assertIn('"$mode" == process-death', target_helper)
        self.assertIn('phase == "process-death-seed"', receiver)
        process_death_seed = receiver.split('if (phase == "process-death-seed") {', 1)[1].split("}", 1)[0]
        claim = receiver.split('private fun claimInstrumentationMarker(', 1)[1].split('private fun verifyProcessDeathAuthorization(', 1)[0]
        promote = receiver.split('private fun promoteInstrumentationMarker(', 1)[1].split('private fun claimInstrumentationMarker(', 1)[0]
        self.assertLess(process_death_seed.index('seedInstrumentationMarker(context, token)'),
                        process_death_seed.index('claimInstrumentationMarker(context, token)'))
        self.assertIn('check(!claimedMarker.exists())', claim)
        self.assertIn('marker.renameTo(claimedMarker)', claim)
        self.assertIn('CLAIMED_INSTRUMENTATION_MARKER', promote)
        self.assertIn('marker.renameTo(authorization)', promote)
        self.assertIn("phase='process-death-seed'", target_helper)
        self.assertIn('Missing or mismatched fresh Process Death marker', receiver)

    def test_device_diagnostics_use_posix_grep_and_keep_gradle_result_separate(self) -> None:
        campaign = (ROOT / "tools/run_android_instrumentation_campaign.sh").read_text(encoding="utf-8")
        self.assertNotIn("rg ", campaign)
        self.assertIn("grep -E", campaign)
        self.assertIn('capture_device_state "$evidence/test-device-state.txt" "$evidence/test-device-state-status.txt"', campaign)
        self.assertIn('instrumentation-gradle-exit-code.txt', campaign)
        self.assertIn('stage_dir/gradle-exit-code.txt', campaign)
        self.assertIn('# Device diagnostics are observational; their status never replaces Gradle\'s exit code.', campaign)

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
                "if [[ \"$1\" == -s ]]; then shift 2; fi\n"
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
                "GITHUB_RUN_ID": "12345",
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
        self.assertIn("phase='campaign-seed'", helper)
        self.assertIn('PASS:$token:$phase:$request_id', helper)

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
                "if [[ \"$1\" == -s ]]; then shift 2; fi\n"
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
            environment["GITHUB_RUN_ID"] = "12345"
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
            self.assertEqual(
                "35\n",
                (evidence / "device-api-level.txt").read_text(),
                result.stdout + result.stderr,
            )
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
                "if [[ \"$1\" == -s ]]; then shift 2; fi\n"
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
                "if [[ \"$1\" == -s ]]; then shift 2; fi\n"
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
                "GITHUB_RUN_ID": "12345",
                "SEREA_DISPOSABLE_CI_EMULATOR": "1",
                "EXPECTED_API_LEVEL": "35",
                "ADB_MUTATION_MARKER": str(marker),
            })
            result = subprocess.run(
                ["bash", "-c", 'source "$1"', "mock-guard", str(guard)],
                env=environment,
                capture_output=True,
                text=True,
                check=False,
            )
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
                "if [[ \"$1\" == -s ]]; then shift 2; fi\n"
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
        self.assertIn("isAbsentOrEmptyDirectory(filesDirectory)", runner)
        self.assertIn("filesSafe", runner)
        self.assertIn("isAbsentOrEmptyDirectory(databaseDirectory)", runner)
        self.assertIn('target.getDatabasePath("serea-history.db").exists()', runner)
        self.assertNotIn("listFiles().isNullOrEmpty()", runner)
        self.assertLess(seed.index("preferences.all.isEmpty()"), seed.index("repository.list()"))

    def test_lifecycle_summary_keeps_last_started_and_finished_tests_distinct(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            log = Path(temporary) / "logcat.txt"
            summary = Path(temporary) / "summary.txt"
            log.write_text(
                "10-21 16:00:00.000 123 456 I SereaTestLifecycle: START|1792598400000|a.First#done\n"
                "10-21 16:00:01.000 123 456 I SereaTestLifecycle: FINISH|1792598401000|a.First#done\n"
                "10-21 16:00:02.000 123 456 I SereaTestLifecycle: START|1792598402000|a.Second#blocked\n"
                "10-21 16:00:02.100 123 456 I SereaCalendarStage: START|scroll-event-50|testThread=Instr:1|uiThread=main:2|onUiThread=false\n"
                "10-21 16:00:03.000 123 456 I SereaTestLifecycle: RUN_FINISH|1792598403000|2|0|0\n",
                encoding="utf-8",
            )
            subprocess.run(
                ["python3", str(ROOT / "tools/summarize_android_test_lifecycle.py"), str(log), str(summary)],
                check=True,
            )
            contents = summary.read_text(encoding="utf-8")
            self.assertIn("last_test_started=a.Second#blocked", contents)
            self.assertIn("last_test_finished=a.First#done", contents)
            self.assertIn("unfinished_test=a.Second#blocked", contents)
            self.assertIn("expected_test_cases=105", contents)
            self.assertIn("original_required_test_cases=103", contents)
            self.assertIn("additional_test_cases=2", contents)
            self.assertIn("original_required_started=0/103", contents)
            self.assertIn("additional_started=0/2", contents)
            self.assertIn("tests_not_started=103", contents)
            self.assertIn("tests_started_without_finish=1", contents)
            self.assertIn("runner_test_count=2", contents)
            self.assertIn("runner_failure_count=0", contents)
            self.assertIn("calendar_last_stage_started=scroll-event-50", contents)
            self.assertIn("calendar_stage_incomplete=scroll-event-50", contents)


if __name__ == "__main__":
    unittest.main()
