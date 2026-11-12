from __future__ import annotations

import hashlib
import re
import os
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path

from tools.summarize_android_test_lifecycle import summarize
from tools.classify_android_changes import classify


ROOT = Path(__file__).resolve().parents[1]


class AndroidEmulatorWorkflowTests(unittest.TestCase):
    def test_g6_exact_source_focused_workflow_keeps_main_merge_result_gate(self) -> None:
        focused = (ROOT / ".github/workflows/android-functional-focused.yml").read_text(encoding="utf-8")
        pull_request = focused.split("  pull_request:", 1)[1].split("  push:", 1)[0]
        self.assertIn("      - main\n", pull_request)
        self.assertIn("      - 'tools/assert_exact_source_checkout.sh'", pull_request)
        self.assertIn("ref: ${{ github.event_name == 'pull_request' && github.event.pull_request.head.sha || github.sha }}", focused)
        self.assertIn("EXPECTED_SOURCE_SHA: ${{ github.event_name == 'pull_request' && github.event.pull_request.head.sha || github.sha }}", focused)
        checkout_starts = list(re.finditer(r"(?m)^      - uses: actions/checkout@[^\n]*", focused))
        self.assertEqual(3, len(checkout_starts))
        for checkout in checkout_starts:
            next_step = re.search(r"(?m)^      - ", focused[checkout.end():])
            stop = checkout.end() + next_step.start() if next_step else len(focused)
            self.assertIn("persist-credentials: false", focused[checkout.start():stop])
        self.assertEqual((True, False), classify(["tools/assert_exact_source_checkout.sh"]))

        required = (ROOT / ".github/workflows/android.yml").read_text(encoding="utf-8")
        self.assertIn("ref: ${{ github.sha }}", required)
        self.assertNotIn("ref: ${{ github.event.pull_request.head.sha", required)
        self.assertEqual(2, required.count("python3 tools/verify_pr_merge_result.py"))
        self.assertEqual(2, required.count("SEREA_EXPECTED_MERGE_SHA: ${{ github.sha }}"))
        self.assertIn("SOURCE_SHA: ${{ github.event.pull_request.head.sha || github.sha }}", required)
        self.assertIn("BUILD_CHECKOUT_SHA: ${{ github.sha }}", required)
        self.assertIn("python3 tools/verify_android_build_provenance.py", required)
        self.assertIn('--source-sha "$SOURCE_SHA"', required)
        self.assertIn('--build-sha "$BUILD_CHECKOUT_SHA"', required)

    def test_android_scope_job_verifies_pr_merge_result_before_classifying(self) -> None:
        workflow = (ROOT / ".github/workflows/android.yml").read_text(encoding="utf-8")
        match = re.search(
            r"(?ms)^  android-changes:\n(?P<body>.*?)(?=^  [A-Za-z0-9_-]+:\n|\Z)",
            workflow,
        )
        self.assertIsNotNone(match)
        body = match.group("body")
        self.assertIn("name: Android Change Scope", body)
        self.assertIn("ref: ${{ github.sha }}", body)
        verifier = body.index("name: Verify PR merge-result before Android scope checks")
        classifier = body.index("name: Classify changed files")
        self.assertLess(verifier, classifier)
        verification = body[verifier:classifier]
        self.assertIn("if: ${{ github.event_name == 'pull_request' }}", verification)
        self.assertIn("SEREA_EXPECTED_MERGE_SHA: ${{ github.sha }}", verification)
        self.assertIn("run: python3 tools/verify_pr_merge_result.py", verification)

    def test_real_ime_diagnostic_checkout_is_read_only_and_targets_main(self) -> None:
        workflow = (ROOT / ".github/workflows/android-api35-real-ime-independent-bc.yml").read_text(
            encoding="utf-8"
        )
        pull_request = workflow.split("  pull_request:", 1)[1].split("\npermissions:", 1)[0]
        self.assertIn("      - main\n", pull_request)
        self.assertIn("      - codex/serea-g4-final-integration-candidate-20261027\n", pull_request)
        self.assertIn("  workflow_dispatch:\n", workflow)
        self.assertIn("permissions:\n  contents: read\n", workflow)
        self.assertNotIn("pull_request_target:", workflow)
        self.assertIn("DIAGNOSTIC_BASE_SHA: b06c1acd5f3ddb4f59f0cad6ae1a1d291b086671", workflow)
        self.assertIn("CURRENT_BASE_SHA: ${{ github.event.pull_request.base.sha || github.sha }}", workflow)
        self.assertIn('git merge-base --is-ancestor "$CURRENT_BASE_SHA" HEAD', workflow)
        self.assertIn('git diff --name-only --no-renames -z "$DIAGNOSTIC_BASE_SHA" HEAD', workflow)
        self.assertIn("while IFS= read -r -d '' path", workflow)
        self.assertIn('git diff --name-only --no-renames -z "$DIAGNOSTIC_BASE_SHA" HEAD > "$paths_file"', workflow)
        self.assertNotIn("done < <(git diff", workflow)
        self.assertIn('git --literal-pathspecs diff --quiet "$CURRENT_BASE_SHA" HEAD -- "$path"', workflow)

        checkout_starts = list(re.finditer(r"(?m)^      - uses: actions/checkout@[^\n]*", workflow))
        self.assertEqual(len(checkout_starts), 2)
        for checkout in checkout_starts:
            next_step = re.search(r"(?m)^      - ", workflow[checkout.end():])
            stop = checkout.end() + next_step.start() if next_step else len(workflow)
            self.assertIn("persist-credentials: false", workflow[checkout.start():stop])

    def test_real_ime_scope_check_detects_rename_source_hidden_by_allowlisted_destination(self) -> None:
        workflow = (ROOT / ".github/workflows/android-api35-real-ime-independent-bc.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn('git diff --name-only --no-renames -z "$DIAGNOSTIC_BASE_SHA" HEAD', workflow)

        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp)

            def git(*args: str) -> str:
                result = subprocess.run(
                    ["git", *args], cwd=repo, check=True, capture_output=True, text=True
                )
                return result.stdout.strip()

            git("init", "-q")
            git("config", "user.name", "Test")
            git("config", "user.email", "test@example.invalid")
            (repo / "out-of-scope.txt").write_text("rename payload from source\n", encoding="utf-8")
            destination = repo / ".github/workflows/android-api35-real-ime-independent-bc.yml"
            git("add", "out-of-scope.txt")
            git("commit", "-qm", "diagnostic baseline")
            diagnostic_base = git("rev-parse", "HEAD")

            destination.parent.mkdir(parents=True)
            destination.write_text("rename payload from source\n", encoding="utf-8")
            git("add", "out-of-scope.txt", ".github/workflows/android-api35-real-ime-independent-bc.yml")
            git("commit", "-qm", "current base updates destination")
            current_base = git("rev-parse", "HEAD")

            (repo / "out-of-scope.txt").unlink()
            git("add", "-A")
            git("commit", "-qm", "candidate deletes out-of-scope source")

            renamed = git("diff", "--name-only", "--find-renames", diagnostic_base, "HEAD").splitlines()
            self.assertNotIn("out-of-scope.txt", renamed)
            self.assertIn(".github/workflows/android-api35-real-ime-independent-bc.yml", renamed)

            paths = git("diff", "--name-only", "--no-renames", diagnostic_base, "HEAD").splitlines()
            self.assertIn("out-of-scope.txt", paths)
            changed_from_current_base = git(
                "diff", "--name-only", current_base, "HEAD", "--", "out-of-scope.txt"
            )
            self.assertEqual(changed_from_current_base, "out-of-scope.txt")

    def test_real_ime_scope_guard_rejects_newline_path_that_mimics_allowlisted_path(self) -> None:
        workflow = (ROOT / ".github/workflows/android-api35-real-ime-independent-bc.yml").read_text(
            encoding="utf-8"
        )
        marker = "      - name: Verify source scope is limited to this diagnostic harness\n        shell: bash\n        run: |\n"
        self.assertIn(marker, workflow)
        block = workflow.split(marker, 1)[1].split("\n      - name:", 1)[0]
        script = textwrap.dedent("\n".join(line[10:] for line in block.splitlines()))

        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp)
            subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
            subprocess.run(["git", "config", "user.name", "Test"], cwd=repo, check=True)
            subprocess.run(["git", "config", "user.email", "test@example.invalid"], cwd=repo, check=True)
            subprocess.run(["git", "commit", "--allow-empty", "-qm", "baseline"], cwd=repo, check=True)
            baseline = subprocess.run(
                ["git", "rev-parse", "HEAD"], cwd=repo, check=True, capture_output=True, text=True
            ).stdout.strip()
            malicious = repo / ".github/workflows/android-api35-real-ime-independent-bc.yml\n"
            malicious.parent.mkdir(parents=True)
            malicious.write_text("unexpected\n", encoding="utf-8")
            subprocess.run(["git", "add", "-A"], cwd=repo, check=True)
            subprocess.run(["git", "commit", "-qm", "newline path"], cwd=repo, check=True)
            head = subprocess.run(
                ["git", "rev-parse", "HEAD"], cwd=repo, check=True, capture_output=True, text=True
            ).stdout.strip()
            result = subprocess.run(
                ["bash", "-euo", "pipefail", "-c", script],
                cwd=repo,
                env={
                    **os.environ,
                    "EXPECTED_SOURCE_SHA": head,
                    "CURRENT_BASE_SHA": baseline,
                    "DIAGNOSTIC_BASE_SHA": baseline,
                },
                capture_output=True,
                text=True,
                check=False,
            )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Unexpected changes since diagnostic base", result.stderr)
        self.assertIn("\\n", result.stderr)

    def test_real_ime_scope_guard_fails_closed_when_git_diff_fails(self) -> None:
        workflow = (ROOT / ".github/workflows/android-api35-real-ime-independent-bc.yml").read_text(
            encoding="utf-8"
        )
        marker = "      - name: Verify source scope is limited to this diagnostic harness\n        shell: bash\n        run: |\n"
        block = workflow.split(marker, 1)[1].split("\n      - name:", 1)[0]
        script = textwrap.dedent("\n".join(line[10:] for line in block.splitlines()))

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            repo = root / "repo"
            fake_bin = root / "bin"
            repo.mkdir()
            fake_bin.mkdir()
            subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
            subprocess.run(["git", "config", "user.name", "Test"], cwd=repo, check=True)
            subprocess.run(["git", "config", "user.email", "test@example.invalid"], cwd=repo, check=True)
            subprocess.run(["git", "commit", "--allow-empty", "-qm", "baseline"], cwd=repo, check=True)
            baseline = subprocess.run(
                ["git", "rev-parse", "HEAD"], cwd=repo, check=True, capture_output=True, text=True
            ).stdout.strip()
            git_path = subprocess.check_output(["which", "git"], text=True).strip()
            fake_git = fake_bin / "git"
            fake_git.write_text(
                f"#!/bin/sh\nif [ \"${{1:-}}\" = diff ]; then exit 42; fi\nexec {git_path} \"$@\"\n",
                encoding="utf-8",
            )
            fake_git.chmod(0o755)
            result = subprocess.run(
                ["bash", "-euo", "pipefail", "-c", script],
                cwd=repo,
                env={
                    **os.environ,
                    "PATH": f"{fake_bin}:{os.environ['PATH']}",
                    "EXPECTED_SOURCE_SHA": baseline,
                    "CURRENT_BASE_SHA": baseline,
                    "DIAGNOSTIC_BASE_SHA": baseline,
                },
                capture_output=True,
                text=True,
                check=False,
            )

        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("Unexpected changes since diagnostic base", result.stderr)

    def test_real_ime_scope_guard_treats_pathspec_magic_as_literal_filename(self) -> None:
        workflow = (ROOT / ".github/workflows/android-api35-real-ime-independent-bc.yml").read_text(
            encoding="utf-8"
        )
        marker = "      - name: Verify source scope is limited to this diagnostic harness\n        shell: bash\n        run: |\n"
        block = workflow.split(marker, 1)[1].split("\n      - name:", 1)[0]
        script = textwrap.dedent("\n".join(line[10:] for line in block.splitlines()))

        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp)
            subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
            subprocess.run(["git", "config", "user.name", "Test"], cwd=repo, check=True)
            subprocess.run(["git", "config", "user.email", "test@example.invalid"], cwd=repo, check=True)
            subprocess.run(["git", "commit", "--allow-empty", "-qm", "baseline"], cwd=repo, check=True)
            baseline = subprocess.run(
                ["git", "rev-parse", "HEAD"], cwd=repo, check=True, capture_output=True, text=True
            ).stdout.strip()
            (repo / ":(exclude)*").write_text("unexpected\n", encoding="utf-8")
            subprocess.run(["git", "add", "-A"], cwd=repo, check=True)
            subprocess.run(["git", "commit", "-qm", "pathspec magic path"], cwd=repo, check=True)
            head = subprocess.run(
                ["git", "rev-parse", "HEAD"], cwd=repo, check=True, capture_output=True, text=True
            ).stdout.strip()
            result = subprocess.run(
                ["bash", "-euo", "pipefail", "-c", script],
                cwd=repo,
                env={
                    **os.environ,
                    "EXPECTED_SOURCE_SHA": head,
                    "CURRENT_BASE_SHA": baseline,
                    "DIAGNOSTIC_BASE_SHA": baseline,
                },
                capture_output=True,
                text=True,
                check=False,
            )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Unexpected changes since diagnostic base", result.stderr)

    def test_personal_beta_identity_preflight_waits_for_stable_disposable_emulator(self) -> None:
        script = (ROOT / "tools/test_personal_beta_identity.sh").read_text(encoding="utf-8")
        self.assertIn('discovery_deadline=$((SECONDS + 60))', script)
        self.assertIn('device_listing=$(timeout "${timeout_seconds}s" adb devices -l 2>&1 || true)', script)
        self.assertIn('bounded_adb "$deadline" 8 -s "$serial" wait-for-device', script)
        self.assertIn('local deadline=$((SECONDS + 120))', script)
        self.assertIn('while (( SECONDS < deadline )); do', script)
        self.assertIn('timeout "${cap}s" adb "$@"', script)
        self.assertIn('source "$(dirname "${BASH_SOURCE[0]}")/android_emulator_lifecycle.sh"', script)
        self.assertIn('serea_retry_adb_root "$serial"', script)
        self.assertIn('adb -s "$serial" get-state', script)
        self.assertIn('"$state" == device && "$sdk" == 35 && "$qemu" == 1 && "$boot_completed" == 1', script)
        self.assertIn('[[ "$stable_samples" -ge 3 ]]', script)
        self.assertIn('print_emulator_diagnostics "Emulator did not remain online', script)
        reboot = script.split("reboot_emulator() {", 1)[1].split("\n}", 1)[0]
        reboot_stability = reboot.index("wait_for_stable_emulator 'Android system reboot before root restart'")
        root_restart = reboot.index('serea_retry_adb_root "$serial"')
        root_stability = reboot.index("wait_for_stable_emulator 'adb root restart after Android system reboot'")
        self.assertLess(reboot_stability, root_restart)
        self.assertLess(root_restart, root_stability)
        self.assertIn('shell id -u', reboot)
        self.assertIn('wait_for_package_manager', reboot)
        self.assertIn("set -Eeuo pipefail", script)
        self.assertIn("trap 'on_error \"$LINENO\" \"$BASH_COMMAND\"' ERR", script)
        self.assertIn('command=%q', script)
        self.assertIn("set_phase 'restart adb as root and wait for stable transport'", script)
        self.assertIn('if [[ "$beta_path_status" -ne 0 ]]; then', script)
        self.assertIn('adb -s "$serial" shell pm list packages "$beta_package"', script)
        self.assertIn('"$beta_device_state" != device', script)
        self.assertIn("'Error: package not found'|\"Error: package not found: $beta_package\")", script)
        self.assertIn('Beta package query failed (exit=%s): %s', script)
        subprocess.run(["bash", "-n", str(ROOT / "tools/test_personal_beta_identity.sh")], check=True)

    def test_adb_root_retries_only_transient_transport_failures(self) -> None:
        helper = ROOT / "tools/android_emulator_lifecycle.sh"
        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary)
            bindir = temporary_path / "bin"
            bindir.mkdir()
            attempts = temporary_path / "root-attempts"
            reconnect = temporary_path / "reconnect-called"
            adb = bindir / "adb"
            adb.write_text(
                "#!/usr/bin/env bash\n"
                "if [[ \"$*\" == *'reconnect offline'* ]]; then touch \"$ADB_RECONNECT_MARKER\"; exit 0; fi\n"
                "if [[ \"$*\" == *' root' ]]; then\n"
                "  count=$(cat \"$ADB_ROOT_ATTEMPTS\" 2>/dev/null || echo 0)\n"
                "  count=$((count + 1)); printf '%s' \"$count\" > \"$ADB_ROOT_ATTEMPTS\"\n"
                "  if [[ \"$count\" == 1 ]]; then echo 'adb: unable to connect for root: closed' >&2; exit 1; fi\n"
                "  echo 'restarting adbd as root'; exit 0\n"
                "fi\n"
                "exit 0\n",
                encoding="utf-8",
            )
            adb.chmod(0o755)
            sleep = bindir / "sleep"
            sleep.write_text("#!/usr/bin/env bash\nexit 0\n", encoding="utf-8")
            sleep.chmod(0o755)
            env = os.environ.copy()
            env.update({
                "PATH": f"{bindir}{os.pathsep}{env['PATH']}",
                "ADB_ROOT_ATTEMPTS": str(attempts),
                "ADB_RECONNECT_MARKER": str(reconnect),
            })
            result = subprocess.run(
                ["bash", "-c", 'source "$1"; serea_retry_adb_root emulator-5554', "test", str(helper)],
                check=False,
                capture_output=True,
                text=True,
                env=env,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(attempts.read_text(encoding="utf-8"), "2")
            self.assertTrue(reconnect.exists())
            self.assertIn("restarting adbd as root", result.stdout)
            self.assertIn("RETRY: transient ADB transport failure", result.stderr)

    def test_adb_root_fails_closed_on_non_transient_error(self) -> None:
        helper = ROOT / "tools/android_emulator_lifecycle.sh"
        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary)
            bindir = temporary_path / "bin"
            bindir.mkdir()
            attempts = temporary_path / "root-attempts"
            reconnect = temporary_path / "reconnect-called"
            adb = bindir / "adb"
            adb.write_text(
                "#!/usr/bin/env bash\n"
                "if [[ \"$*\" == *'reconnect offline'* ]]; then touch \"$ADB_RECONNECT_MARKER\"; exit 0; fi\n"
                "if [[ \"$*\" == *' root' ]]; then\n"
                "  count=$(cat \"$ADB_ROOT_ATTEMPTS\" 2>/dev/null || echo 0)\n"
                "  count=$((count + 1)); printf '%s' \"$count\" > \"$ADB_ROOT_ATTEMPTS\"\n"
                "  echo 'adbd cannot run as root in production builds' >&2; exit 1\n"
                "fi\n"
                "exit 0\n",
                encoding="utf-8",
            )
            adb.chmod(0o755)
            env = os.environ.copy()
            env.update({
                "PATH": f"{bindir}{os.pathsep}{env['PATH']}",
                "ADB_ROOT_ATTEMPTS": str(attempts),
                "ADB_RECONNECT_MARKER": str(reconnect),
            })
            result = subprocess.run(
                [
                    "bash",
                    "-c",
                    'source "$1"; if serea_retry_adb_root emulator-5554; then exit 1; else exit 0; fi',
                    "test",
                    str(helper),
                ],
                check=False,
                capture_output=True,
                text=True,
                env=env,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(attempts.read_text(encoding="utf-8"), "1")
            self.assertFalse(reconnect.exists())
            self.assertIn("non-transient error", result.stderr)

    def test_personal_beta_identity_does_not_mask_package_probe_transport_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            mock_adb = Path(temporary) / "adb"
            mock_adb.write_text(
                "#!/usr/bin/env bash\n"
                "case \"$*\" in\n"
                "  *'devices -l'*) printf 'List of devices attached\\nemulator-5554 device\\n' ;;\n"
                "  *'wait-for-device'*) exit 0 ;;\n"
                "  *'get-state'*) echo device ;;\n"
                "  *'getprop ro.build.version.sdk'*) echo 35 ;;\n"
                "  *'getprop ro.kernel.qemu'*) echo 1 ;;\n"
                "  *'getprop sys.boot_completed'*) echo 1 ;;\n"
                "  *' root'*) exit 0 ;;\n"
                "  *'shell id -u'*) echo 0 ;;\n"
                "  *'shell pm list packages -3'*) echo package:android ;;\n"
                "  *'shell pm path app.serea.android.beta'*) echo 'adb: device offline'; exit 7 ;;\n"
                "  *) exit 0 ;;\n"
                "esac\n",
                encoding="utf-8",
            )
            mock_adb.chmod(0o755)
            result_path = ROOT / "android/app/build/outputs/androidTest-results/personal-beta-identity.txt"
            previous_result = result_path.read_bytes() if result_path.exists() else None
            environment = os.environ.copy()
            environment["PATH"] = f"{temporary}{os.pathsep}{environment['PATH']}"
            environment["ANDROID_HOME"] = temporary
            environment["ANDROID_SDK_ROOT"] = temporary
            environment["SEREA_PERSONAL_BETA_DISPOSABLE_EMULATOR"] = "1"
            try:
                result = subprocess.run(
                    ["bash", "tools/test_personal_beta_identity.sh"],
                    cwd=ROOT,
                    env=environment,
                    text=True,
                    capture_output=True,
                    timeout=30,
                )
                self.assertEqual(result.returncode, 7, result.stderr)
                self.assertIn(
                    "Personal Beta harness phase: refuse preinstalled Beta package on the disposable emulator",
                    result.stdout,
                )
                self.assertIn("Beta package query failed (exit=7): adb: device offline", result.stderr)
                self.assertIn("ADB state after an unexpected Beta package query response:", result.stderr)
                self.assertIn("List of devices attached", result.stderr)
                self.assertEqual(result_path.read_text(encoding="utf-8"), "NOT_RUN\n")
            finally:
                if previous_result is None:
                    result_path.unlink(missing_ok=True)
                else:
                    result_path.write_bytes(previous_result)

    def test_personal_beta_identity_accepts_only_package_manager_not_found(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            mock_adb = Path(temporary) / "adb"
            mock_adb.write_text(
                "#!/usr/bin/env bash\n"
                "case \"$*\" in\n"
                "  *'devices -l'*) printf 'List of devices attached\\nemulator-5554 device\\n' ;;\n"
                "  *'wait-for-device'*) exit 0 ;;\n"
                "  *'get-state'*) echo device ;;\n"
                "  *'getprop ro.build.version.sdk'*) echo 35 ;;\n"
                "  *'getprop ro.kernel.qemu'*) echo 1 ;;\n"
                "  *'getprop sys.boot_completed'*) echo 1 ;;\n"
                "  *' root'*) exit 0 ;;\n"
                "  *'shell id -u'*) echo 0 ;;\n"
                "  *'shell pm list packages -3'*) echo package:android ;;\n"
                "  *'shell pm path app.serea.android.beta'*) exit 1 ;;\n"
                "  *'shell pm list packages app.serea.android.beta'*) echo package:app.serea.android.demo ;;\n"
                "  *) exit 0 ;;\n"
                "esac\n",
                encoding="utf-8",
            )
            mock_adb.chmod(0o755)
            fake_java = Path(temporary) / "bin/java"
            fake_java.parent.mkdir()
            fake_java.write_text("#!/usr/bin/env bash\nexit 19\n", encoding="utf-8")
            fake_java.chmod(0o755)
            result_path = ROOT / "android/app/build/outputs/androidTest-results/personal-beta-identity.txt"
            previous_result = result_path.read_bytes() if result_path.exists() else None
            environment = os.environ.copy()
            environment["PATH"] = f"{temporary}{os.pathsep}{environment['PATH']}"
            environment["JAVA_HOME"] = temporary
            environment["ANDROID_HOME"] = temporary
            environment["ANDROID_SDK_ROOT"] = temporary
            environment["SEREA_PERSONAL_BETA_DISPOSABLE_EMULATOR"] = "1"
            try:
                result = subprocess.run(
                    ["bash", "tools/test_personal_beta_identity.sh"],
                    cwd=ROOT,
                    env=environment,
                    text=True,
                    capture_output=True,
                    timeout=30,
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(
                    "Personal Beta harness phase: build and capture existing Demo identity/data",
                    result.stdout,
                )
                self.assertNotIn("Beta package query failed", result.stderr)
                self.assertIn("phase=build\\ and\\ capture", result.stderr)
                self.assertEqual(result_path.read_text(encoding="utf-8"), "NOT_RUN\n")
            finally:
                if previous_result is None:
                    result_path.unlink(missing_ok=True)
                else:
                    result_path.write_bytes(previous_result)

    def test_personal_beta_identity_fails_on_package_list_transport_error(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            mock_adb = Path(temporary) / "adb"
            mock_adb.write_text(
                "#!/usr/bin/env bash\n"
                "case \"$*\" in\n"
                "  *'devices -l'*) printf 'List of devices attached\\nemulator-5554 device\\n' ;;\n"
                "  *'wait-for-device'*) exit 0 ;;\n"
                "  *'get-state'*) echo device ;;\n"
                "  *'getprop ro.build.version.sdk'*) echo 35 ;;\n"
                "  *'getprop ro.kernel.qemu'*) echo 1 ;;\n"
                "  *'getprop sys.boot_completed'*) echo 1 ;;\n"
                "  *' root'*) exit 0 ;;\n"
                "  *'shell id -u'*) echo 0 ;;\n"
                "  *'shell pm list packages -3'*) echo package:android ;;\n"
                "  *'shell pm path app.serea.android.beta'*) exit 1 ;;\n"
                "  *'shell pm list packages app.serea.android.beta'*) echo 'adb: device offline'; exit 7 ;;\n"
                "  *) exit 0 ;;\n"
                "esac\n",
                encoding="utf-8",
            )
            mock_adb.chmod(0o755)
            result_path = ROOT / "android/app/build/outputs/androidTest-results/personal-beta-identity.txt"
            previous_result = result_path.read_bytes() if result_path.exists() else None
            environment = os.environ.copy()
            environment["PATH"] = f"{temporary}{os.pathsep}{environment['PATH']}"
            environment["ANDROID_HOME"] = temporary
            environment["ANDROID_SDK_ROOT"] = temporary
            environment["SEREA_PERSONAL_BETA_DISPOSABLE_EMULATOR"] = "1"
            try:
                result = subprocess.run(
                    ["bash", "tools/test_personal_beta_identity.sh"],
                    cwd=ROOT,
                    env=environment,
                    text=True,
                    capture_output=True,
                    timeout=30,
                )
                self.assertEqual(result.returncode, 7, result.stderr)
                self.assertIn("Beta package-list verification failed (exit=7): adb: device offline", result.stderr)
                self.assertIn("ADB state after the Beta package-list verification failed:", result.stderr)
                self.assertEqual(result_path.read_text(encoding="utf-8"), "NOT_RUN\n")
            finally:
                if previous_result is None:
                    result_path.unlink(missing_ok=True)
                else:
                    result_path.write_bytes(previous_result)

    def test_personal_beta_identity_refuses_exact_package_list_match(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            mock_adb = Path(temporary) / "adb"
            mock_adb.write_text(
                "#!/usr/bin/env bash\n"
                "case \"$*\" in\n"
                "  *'devices -l'*) printf 'List of devices attached\\nemulator-5554 device\\n' ;;\n"
                "  *'wait-for-device'*) exit 0 ;;\n"
                "  *'get-state'*) echo device ;;\n"
                "  *'getprop ro.build.version.sdk'*) echo 35 ;;\n"
                "  *'getprop ro.kernel.qemu'*) echo 1 ;;\n"
                "  *'getprop sys.boot_completed'*) echo 1 ;;\n"
                "  *' root'*) exit 0 ;;\n"
                "  *'shell id -u'*) echo 0 ;;\n"
                "  *'shell pm list packages -3'*) echo package:android ;;\n"
                "  *'shell pm path app.serea.android.beta'*) exit 1 ;;\n"
                "  *'shell pm list packages app.serea.android.beta'*) echo package:app.serea.android.beta ;;\n"
                "  *) exit 0 ;;\n"
                "esac\n",
                encoding="utf-8",
            )
            mock_adb.chmod(0o755)
            result_path = ROOT / "android/app/build/outputs/androidTest-results/personal-beta-identity.txt"
            previous_result = result_path.read_bytes() if result_path.exists() else None
            environment = os.environ.copy()
            environment["PATH"] = f"{temporary}{os.pathsep}{environment['PATH']}"
            environment["ANDROID_HOME"] = temporary
            environment["ANDROID_SDK_ROOT"] = temporary
            environment["SEREA_PERSONAL_BETA_DISPOSABLE_EMULATOR"] = "1"
            try:
                result = subprocess.run(
                    ["bash", "tools/test_personal_beta_identity.sh"],
                    cwd=ROOT,
                    env=environment,
                    text=True,
                    capture_output=True,
                    timeout=30,
                )
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertIn("Refusing an emulator that already contains app.serea.android.beta", result.stderr)
                self.assertNotIn("Beta package query failed", result.stderr)
                self.assertEqual(result_path.read_text(encoding="utf-8"), "NOT_RUN\n")
            finally:
                if previous_result is None:
                    result_path.unlink(missing_ok=True)
                else:
                    result_path.write_bytes(previous_result)

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
        self.assertIn(
            'if [[ "$gradle_status" -eq 0 && "$verify_exit_code" -eq 0 && "$install_sha_status" -eq 0 ]]',
            stage_body,
        )
        self.assertIn(
            'if [[ "$gradle_status" -ne 0 || "$verify_status" -ne 0 || "$install_sha_status" -ne 0 ]]',
            stage_body,
        )
        self.assertIn('while kill -0 "$gradle_pid" 2>/dev/null; do', stage_body)
        self.assertIn('pm path app.serea.android.isolated.test', stage_body)
        self.assertIn('test_expected_apk_sha256=%s', stage_body)

    def test_android_default_ime_readiness_waits_for_value_without_mutating_settings(self) -> None:
        helper = ROOT / "tools/wait_for_android_default_ime.sh"
        helper_text = helper.read_text(encoding="utf-8")
        self.assertIn('local max_attempts="${1:-30}"', helper_text)
        self.assertIn('adb shell settings get secure default_input_method', helper_text)
        self.assertIn('SEREA_ANDROID_DEFAULT_IME_ATTEMPTS="$attempt"', helper_text)
        self.assertNotRegex(helper_text, r"\bime\s+(set|enable|disable)\b")
        for script_path in (
            ROOT / "tools/run_android_instrumentation_campaign.sh",
            ROOT / "tools/run_android_functional_focused.sh",
        ):
            script = script_path.read_text(encoding="utf-8")
            self.assertIn("wait_for_android_default_ime.sh", script)
            self.assertIn("wait_for_android_default_ime", script)
            subprocess.run(["bash", "-n", str(script_path)], check=True)

        with tempfile.TemporaryDirectory() as temporary:
            mock_adb = Path(temporary) / "adb"
            mock_adb.write_text(
                "#!/usr/bin/env bash\n"
                "count_file=\"$MOCK_ADB_COUNT_FILE\"\n"
                "count=0; [[ ! -f \"$count_file\" ]] || count=$(cat \"$count_file\")\n"
                "count=$((count + 1)); printf '%s' \"$count\" > \"$count_file\"\n"
                "[[ \"$*\" == 'shell settings get secure default_input_method' ]] || exit 9\n"
                "if [[ \"${MOCK_ADB_ALWAYS_NULL:-0}\" == 1 ]] || ((count < 3)); then echo null; "
                "else echo 'com.example.ime/.Keyboard'; fi\n",
                encoding="utf-8",
            )
            mock_adb.chmod(0o755)
            environment = os.environ.copy()
            environment["PATH"] = f"{temporary}{os.pathsep}{environment['PATH']}"
            environment["MOCK_ADB_COUNT_FILE"] = str(Path(temporary) / "count")
            script = (
                '. "$1"; wait_for_android_default_ime 4; '
                'printf "%s\\n%s\\n" "$SEREA_ANDROID_DEFAULT_IME" '
                '"$SEREA_ANDROID_DEFAULT_IME_ATTEMPTS"'
            )
            result = subprocess.run(
                ["bash", "-c", script, "test", str(helper)],
                check=True,
                capture_output=True,
                text=True,
                env=environment,
            )
            self.assertEqual("com.example.ime/.Keyboard\n3\n", result.stdout)

            environment["MOCK_ADB_ALWAYS_NULL"] = "1"
            failure_script = (
                '. "$1"; if wait_for_android_default_ime 2; then exit 7; fi; '
                'printf "%s\\n%s\\n" "$SEREA_ANDROID_DEFAULT_IME" '
                '"$SEREA_ANDROID_DEFAULT_IME_ATTEMPTS"'
            )
            failure = subprocess.run(
                ["bash", "-c", failure_script, "test", str(helper)],
                check=True,
                capture_output=True,
                text=True,
                env=environment,
            )
            self.assertEqual("null\n2\n", failure.stdout)

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
                "bash ../tools/run_android_instrumentation_campaign.sh 26",
                "bash ../tools/run_android_api35_campaign.sh",
                "bash tools/test_personal_beta_identity.sh",
            ],
            scripts,
        )
        self.assertIn('build_checkout_commit=$(git rev-parse HEAD)', workflow)
        self.assertIn('echo \"build_checkout_commit=$build_checkout_sha\"', workflow)
        self.assertIn('base_commit=$BASE_SHA', workflow)
        self.assertIn('github_run_attempt=$GITHUB_RUN_ATTEMPT', workflow)
        self.assertIn(
            'Source review: locate the matching change from `provenance.txt`; this packet does not authorize merge.',
            workflow,
        )
        self.assertIn('Physical Pixel 7a reacceptance: NOT_RUN by CI.', workflow)
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
            "test_personal_beta_identity.sh",
            "run_android_api35_campaign.sh",
        ):
            subprocess.run(["bash", "-n", str(ROOT / "tools" / name)], check=True)
        self.assertIn("connectedIsolatedAndroidTest", script.read_text(encoding="utf-8"))
        self.assertNotIn("connectedDebugAndroidTest", script.read_text(encoding="utf-8"))
        self.assertIn("python3 tools/classify_android_changes.py", workflow)
        self.assertIn('git diff --no-renames --name-only -z "$PR_BASE_SHA...$PR_HEAD_SHA"', workflow)
        self.assertEqual(workflow.count("uses: actions/checkout@"), workflow.count("name: Verify exact checkout commit"))
        self.assertEqual(2, workflow.count("python3 tools/verify_pr_merge_result.py"))
        self.assertEqual(2, workflow.count("SEREA_EXPECTED_MERGE_SHA: ${{ github.sha }}"))
        self.assertNotIn("ref: ${{ github.event.pull_request.head.sha", workflow)
        self.assertIn("SOURCE_SHA: ${{ github.event.pull_request.head.sha || github.sha }}", workflow)
        self.assertEqual(1, workflow.count("SOURCE_SHA: ${{ github.sha }}"))
        self.assertEqual(2, workflow.count("BUILD_CHECKOUT_SHA: ${{ github.sha }}"))
        self.assertIn("python3 tools/verify_android_build_provenance.py", workflow)
        self.assertIn('--build-sha "$BUILD_CHECKOUT_SHA"', workflow)
        scope_helper = (ROOT / "tools/classify_android_changes.py").read_text(encoding="utf-8")
        self.assertIn('"tools/"', scope_helper)
        self.assertIn('".github/"', scope_helper)
        self.assertIn("Test Android update emulator guard with Mock ADB", workflow)
        self.assertIn("tools/test_android_update_compatibility_guard.sh", workflow)
        api35_campaign = (ROOT / "tools/run_android_api35_campaign.sh").read_text(encoding="utf-8")
        self.assertIn("run_android_instrumentation_campaign.sh\" 35", api35_campaign)
        self.assertIn("test_android_update_compatibility.sh", api35_campaign)
        for api in ("26", "35"):
            self.assertIn(f"android-emulator-api{api}", workflow)
            self.assertIn(f"serea-android-formal-api{api}-provisional", workflow)
            self.assertIn(f"api{api}-build-provenance.txt", workflow)
        self.assertIn("Verify API evidence provenance, JUnit, and isolation results", workflow)
        self.assertIn("serea-android-instrumentation-api26", workflow)
        self.assertIn("serea-android-instrumentation-api35", workflow)
        self.assertIn("Save API 26 campaign logcat diagnostics", workflow)
        self.assertIn("Save API 35 campaign logcat diagnostics", workflow)

    def test_build_provenance_validator_is_shared_by_build_formal_and_reacceptance(self) -> None:
        workflow = (ROOT / ".github/workflows/android.yml").read_text(encoding="utf-8")
        self.assertEqual(4, workflow.count("python3 tools/verify_android_build_provenance.py"))
        self.assertIn("Validate exact Build APK provenance and target/test bindings", workflow)
        self.assertIn("--provenance \"$provenance\" --source-sha \"$EXPECTED_SOURCE_SHA\"", workflow)
        self.assertIn("--provenance \"$provenance_path\"", workflow)
        self.assertEqual(2, workflow.count("apksigner\" verify --verbose \"$target\""))
        verifier = (ROOT / "tools/verify_android_build_provenance.py").read_text(encoding="utf-8")
        for key in ("source_commit", "build_checkout_commit", "github_run_id", "github_run_attempt",
                    "isolated_apk_path", "isolated_apk_package", "isolated_apk_sha256"):
            self.assertIn(key, verifier)


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
        self.assertIn('calendar-class', campaign)
        self.assertIn('app.serea.android.SereaAppShellTest 900 19', campaign)
        self.assertIn('chat-screen-class', campaign)
        self.assertIn('app.serea.android.ChatScreenTest 900 16', campaign)
        self.assertNotIn('runner-smoke', campaign)
        self.assertNotIn('chat-http-e2e', campaign)
        self.assertNotIn('activity-recreation', campaign)
        self.assertNotIn('calendar-target', campaign)
        focused_stage = campaign.index('run_diagnostic_stage cursor-position-focused')
        isolated_consent_stage = campaign.index('run_diagnostic_stage consent-relay-lifecycle-api26')
        consent_companion_stage = campaign.index('run_diagnostic_stage consent-relay-companion-api26')
        api26_focused_stage = campaign.index('run_diagnostic_stage cursor-position-focused', isolated_consent_stage)
        companion_block = campaign.split("consent_companion_tests=$(cat <<'TESTS'", 1)[1].split("\nTESTS\n)", 1)[0]
        companion_identities = {line.strip() for line in companion_block.splitlines() if line.strip()}
        class_stage = campaign.index('run_diagnostic_stage calendar-class')
        chat_class_stage = campaign.index('run_diagnostic_stage chat-screen-class')
        full_suite = campaign.index('timeout --signal=INT --kill-after=120s 40m ./gradlew')
        self.assertEqual(4, len(companion_identities))
        self.assertNotIn(
            "app.serea.android.googleauth.PrivateGoogleConsentRelayInstrumentationTest#relayRecreationDoesNotRelaunchOrLoseTheInFlightIdentity",
            companion_identities,
        )
        self.assertIn('consent_companion_filter="$(printf', campaign)
        self.assertRegex(
            campaign,
            r'run_diagnostic_stage consent-relay-companion-api26 \\\s+"\$consent_companion_filter" \\\s+1200 4',
        )
        self.assertLess(isolated_consent_stage, consent_companion_stage)
        self.assertLess(consent_companion_stage, api26_focused_stage)
        self.assertLess(focused_stage, class_stage)
        self.assertLess(class_stage, chat_class_stage)
        self.assertLess(chat_class_stage, full_suite)
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

    def test_g8_fake_google_and_gmail_ui_runs_as_exact_non_required_api35_stage(self) -> None:
        campaign = (ROOT / "tools/run_android_instrumentation_campaign.sh").read_text(encoding="utf-8")
        manifest = (ROOT / "android/app/src/androidTest/assets/serea-non-required-test-identities.txt").read_text(encoding="utf-8")
        required = (ROOT / "tools/android-instrumentation-required-tests.txt").read_text(encoding="utf-8")
        additional = (ROOT / "tools/android-instrumentation-additional-tests.txt").read_text(encoding="utf-8")
        expected = {
            "app.serea.android.GoogleAuthorizationDemoUiTest#selectedSyntheticAccountIsBoundToTheConsentPreviewAndRevokedOnSwitch",
            "app.serea.android.GoogleAuthorizationDemoUiTest#consentCanBeGrantedDeniedAndCancelledWithoutRealGoogleConnection",
            "app.serea.android.GoogleAuthorizationDemoUiTest#accountSwitchExpiryAndSharedRevocationInvalidateDemoAuthorization",
            "app.serea.android.GoogleAuthorizationDemoUiTest#inAppDisconnectDoesNotRevokeTheSharedDemoGrant",
            "app.serea.android.GoogleCalendarDemoLoopbackUiTest#consentUiOpensLoopbackAgendaAndAccountSwitchDisconnectsIt",
            "app.serea.android.GoogleAuthorizationAppIntegrationTest#settingsConsentOpensCalendarSessionAndActivityRecreationReturnsToConsentPanel",
            "app.serea.android.GoogleCalendarHandoffRevocationUiTest#confirmedCalendarHandoffCannotSendAfterSyntheticAccountSwitch",
            "app.serea.android.GoogleAuthorizationDemoUiTest#expiredCalendarReconnectShowsFreshPreviewAndWaitsForConfirm",
            "app.serea.android.GmailHtmlInertUiSecurityTest#fakeGmailRendersAdversarialHtmlAsInertTextWithoutRemoteRequests",
            "app.serea.android.GoogleTrustedHandoffUiTest#fakeGmailPreviewCanBeCancelledOrConfirmedForLocalChat",
            "app.serea.android.GoogleTrustedHandoffUiTest#fakeCalendarPreviewCanBeCancelledOrConfirmedForLocalChat",
            "app.serea.android.GoogleTrustedHandoffUiTest#gmailOutOfOrderPreviewCallbackCannotReplaceNewestSelection",
            "app.serea.android.GoogleTrustedHandoffUiTest#gmailProviderCancellationFailsClosedAndDialogRemainsCancellable",
            "app.serea.android.GoogleTrustedHandoffShellTest#gmailHandoffReachesChatComposerAndUsesLocalDemoOnlyAfterSend",
            "app.serea.android.GoogleTrustedHandoffShellTest#calendarHandoffReachesChatComposerAndUsesLocalDemoOnlyAfterSend",
            "app.serea.android.GoogleTrustedHandoffShellTest#activityRecreationDropsUnconfirmedPreview",
            "app.serea.android.googleauth.PrivateGoogleConsentRelayInstrumentationTest#nestedActivityResultReturnsLaunchIdentityAndOriginalPayload",
            "app.serea.android.googleauth.PrivateGoogleConsentRelayInstrumentationTest#relayRecreationDoesNotRelaunchOrLoseTheInFlightIdentity",
            "app.serea.android.googleauth.PrivateGoogleConsentRelayInstrumentationTest#missingRelayIdentityFailsClosed",
            "app.serea.android.googleauth.PrivateGoogleConsentRelayInstrumentationTest#accountPickerContractCarriesPerLaunchTokenAcrossSuccessAndCancellation",
            "app.serea.android.googleauth.PrivateGoogleIsolatedManifestTest#isolatedManifestDoesNotRegisterPrivateProductRelayActivities",
        }
        focused = campaign.split("google_demo_ui_tests=$(cat <<'TESTS'", 1)[1].split("\nTESTS\n)", 1)[0]
        actual = {line.strip() for line in focused.splitlines() if line.strip()}
        self.assertEqual(expected, actual)
        self.assertEqual(21, len(actual))
        self.assertTrue(expected.issubset(set(manifest.splitlines())))
        self.assertFalse(expected.intersection(required.splitlines()))
        self.assertFalse(expected.intersection(additional.splitlines()))
        self.assertIn('if [[ ( "$api_level" == 26 || "$api_level" == 35 ) && "$junit_result" == PASS', campaign)
        self.assertIn('run_diagnostic_stage google-calendar-and-gmail-ui "$google_demo_ui_filter" 1200 21', campaign)
        helper = 'bash "$tool_dir/run_android_fake_google_process_death.sh"'
        self.assertIn("GoogleCalendarHandoffRevocationUiTest", campaign.split("google_demo_ui_filter=", 1)[1].split("\n", 1)[0])
        self.assertIn('SEREA_ISOLATED_INSTRUMENTATION_FINISHED=1', campaign)
        self.assertIn('fake_google_process_death_status" != PASS', campaign)
        self.assertEqual(1, campaign.count(helper))
        stage_start = campaign.index('run_diagnostic_stage google-calendar-and-gmail-ui')
        process_death = campaign.index(helper)
        verified_target_install = campaign.index('bash "$tool_dir/ensure_verified_isolated_target.sh"')
        post_stage_cleanup = campaign.index('# This campaign owns only the isolated CI target and test packages on a verified disposable')
        self.assertLess(stage_start, process_death)
        self.assertLess(verified_target_install, process_death)
        self.assertLess(process_death, post_stage_cleanup)
        self.assertIn('"$isolated_apk" "$fake_google_process_death_evidence/target-install.txt"', campaign)
        self.assertIn("Could not safely query the isolated target package; refusing installation.",
                      (ROOT / "tools/ensure_verified_isolated_target.sh").read_text(encoding="utf-8"))
        process_death_guard = campaign.rfind('if [[ "$google_demo_ui_stage_result" -eq 0 ]]; then', stage_start, process_death)
        self.assertNotEqual(-1, process_death_guard)
        self.assertEqual(1, campaign.count('verify_args+=(--require-test "$identity")'))
        self.assertLess(
            campaign.rfind('printf \'%s\\n\' "$death_result"'),
            campaign.index('run_diagnostic_stage google-calendar-and-gmail-ui'),
        )
        self.assertIn('google-demo-ui-status.txt', campaign)
        self.assertIn('google-demo-ui-cleanup-status.txt', campaign)
        self.assertIn('google_demo_ui_result" == PASS', campaign)
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

    def test_android_package_query_transport_failure_cannot_mean_absent(self) -> None:
        helper = ROOT / "tools/android_package_path.sh"
        campaign = (ROOT / "tools/run_android_instrumentation_campaign.sh").read_text(encoding="utf-8")
        self.assertIn('source "$tool_dir/android_package_path.sh"', campaign)
        self.assertIn('if ! existing_target_paths="$(android_package_path app.serea.android.isolated)"', campaign)
        self.assertIn('FAIL: ADB package query failed during focused UI cleanup verification', campaign)
        self.assertIn('if [[ "$google_demo_ui_stage_result" -eq 0 && "$google_demo_ui_cleanup_result" -eq 0 ]]; then', campaign)
        with tempfile.TemporaryDirectory() as temp:
            bindir = Path(temp)
            fake_adb = bindir / "adb"
            fake_adb.write_text(
                "#!/usr/bin/env bash\n"
                "case \"${FAKE_ADB_MODE:-}\" in\n"
                "  path) printf 'package:/data/app/example/base.apk\r\n'; exit 0 ;;\n"
                "  absent) if [[ \"$*\" == *'pm path '* ]]; then exit 1; fi; exit 0 ;;\n"
                "  listed) if [[ \"$*\" == *'pm path '* ]]; then exit 1; fi; printf 'package:app.serea.android.isolated\n'; exit 0 ;;\n"
                "  offline) echo 'adb: device offline' >&2; exit 73 ;;\n"
                "  list_offline) if [[ \"$*\" == *'pm path '* ]]; then exit 1; fi; echo 'adb: device offline' >&2; exit 73 ;;\n"
                "  malformed) echo 'Error: package lookup failed'; exit 0 ;;\n"
                "esac\nexit 0\n",
                encoding="utf-8",
            )
            fake_adb.chmod(0o755)
            command = 'source "$1"; set +e; output=$(android_package_path app.serea.android.isolated 2>"$TMPERR"); result=$?; printf "status=%s output=%s\n" "$result" "$output"'
            common = {**os.environ, "PATH": f"{bindir}:{os.environ['PATH']}", "TMPERR": str(bindir / "stderr")}
            for mode, expected, stderr in (
                ("path", "status=0 output=package:/data/app/example/base.apk", ""),
                ("absent", "status=0 output=", ""),
                ("listed", "status=2 output=", "lists app.serea.android.isolated"),
                ("offline", "status=2 output=", "adb: device offline"),
                ("list_offline", "status=2 output=", "Could not independently verify"),
                ("malformed", "status=2 output=", "unexpected output"),
            ):
                env = {**common, "FAKE_ADB_MODE": mode}
                result = subprocess.run(["bash", "-c", command, "bash", str(helper)], cwd=ROOT, env=env, capture_output=True, text=True, check=True)
                self.assertEqual(expected, result.stdout.strip(), mode)
                diagnostic = Path(env["TMPERR"]).read_text(encoding="utf-8")
                if stderr:
                    self.assertIn(stderr, diagnostic, mode)
                else:
                    self.assertEqual("", diagnostic, mode)

    def test_fake_google_process_death_reinstalls_only_the_verified_isolated_apk(self) -> None:
        helper = ROOT / "tools/ensure_verified_isolated_target.sh"
        apk_bytes = b"synthetic isolated APK bytes"
        expected_sha = hashlib.sha256(apk_bytes).hexdigest()

        def run_helper(*, installed: bool, installed_sha: str, path_mode: str = "normal"):
            with tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                bindir = root / "bin"
                bindir.mkdir()
                apk = root / "app-isolated.apk"
                apk.write_bytes(apk_bytes)
                installed_marker = root / "installed"
                trace = root / "adb.trace"
                if installed:
                    installed_marker.touch()
                adb = bindir / "adb"
                adb.write_text(
                    "#!/usr/bin/env bash\n"
                    "if [[ \"$1\" == -s ]]; then shift 2; fi\n"
                    "printf '%s\\n' \"$*\" >> \"$ADB_TRACE\"\n"
                    "if [[ \"$1\" == devices ]]; then\n"
                    "  printf 'List of devices attached\\nemulator-5554 device product:ci model:ci\\n'\n"
                    "elif [[ \"$1 $2 $3\" == 'shell getprop ro.build.version.sdk' ]]; then\n"
                    "  printf '%s\\n' \"$ANDROID_TEST_API\"\n"
                    "elif [[ \"$1 $2 $3\" == 'shell getprop ro.kernel.qemu' ]]; then\n"
                    "  printf '1\\n'\n"
                    "elif [[ \"$1 $2 $3\" == 'shell pm path' ]]; then\n"
                    "  if [[ \"$FAKE_PATH_MODE\" == transport_failure ]]; then echo 'adb: device offline' >&2; exit 73; fi\n"
                    "  if [[ -e \"$INSTALLED_MARKER\" ]]; then printf 'package:/data/app/isolated/base.apk\\n'; else exit 1; fi\n"
                    "elif [[ \"$1 $2 $3 $4\" == 'shell pm list packages' ]]; then\n"
                    "  if [[ -e \"$INSTALLED_MARKER\" ]]; then printf 'package:app.serea.android.isolated\\n'; fi\n"
                    "elif [[ \"$1\" == install ]]; then\n"
                    "  [[ \"$2\" == \"$EXPECTED_APK\" ]] || exit 74\n"
                    "  touch \"$INSTALLED_MARKER\"\n"
                    "elif [[ \"$1 $2\" == 'shell sha256sum' ]]; then\n"
                    "  printf '%s  %s\\n' \"$FAKE_INSTALLED_SHA\" \"$4\"\n"
                    "fi\n",
                    encoding="utf-8",
                )
                adb.chmod(0o755)
                evidence = root / "evidence" / "target-install.txt"
                environment = os.environ.copy()
                environment.update({
                    "PATH": f"{bindir}{os.pathsep}{environment['PATH']}",
                    "CI": "true",
                    "GITHUB_ACTIONS": "true",
                    "GITHUB_RUN_ID": "12345",
                    "SEREA_DISPOSABLE_CI_EMULATOR": "1",
                    "EXPECTED_API_LEVEL": "26",
                    "ANDROID_TEST_API": "26",
                    "ADB_TRACE": str(trace),
                    "INSTALLED_MARKER": str(installed_marker),
                    "EXPECTED_APK": str(apk),
                    "FAKE_INSTALLED_SHA": installed_sha,
                    "FAKE_PATH_MODE": path_mode,
                })
                result = subprocess.run(
                    ["bash", str(helper), str(apk), str(evidence)],
                    cwd=ROOT,
                    env=environment,
                    capture_output=True,
                    text=True,
                    check=False,
                )
                calls = trace.read_text(encoding="utf-8") if trace.exists() else ""
                evidence_text = evidence.read_text(encoding="utf-8") if evidence.exists() else ""
                return result, calls, evidence_text

        result, calls, evidence = run_helper(installed=False, installed_sha=expected_sha)
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertIn("install /", calls)
        self.assertIn("installation=installed-exact-candidate-apk", evidence)
        self.assertIn(f"candidate_apk_sha256={expected_sha}", evidence)
        self.assertIn(f"installed_apk_sha256={expected_sha}", evidence)

        wrong_sha = "0" * 64
        result, calls, evidence = run_helper(installed=True, installed_sha=wrong_sha)
        self.assertNotEqual(0, result.returncode)
        self.assertIn("does not match the candidate APK", result.stderr)
        self.assertNotIn("install /", calls)
        self.assertEqual("", evidence)

        result, calls, evidence = run_helper(
            installed=False, installed_sha=expected_sha, path_mode="transport_failure"
        )
        self.assertNotEqual(0, result.returncode)
        self.assertIn("Could not safely query", result.stderr)
        self.assertNotIn("install /", calls)
        self.assertEqual("", evidence)

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

    def test_fake_google_process_death_harness_is_host_driven_and_isolated(self) -> None:
        harness = (ROOT / "tools/run_android_fake_google_process_death.sh").read_text(encoding="utf-8")
        activity = (ROOT / "android/app/src/isolated/java/app/serea/android/FakeGoogleProcessDeathHarnessActivity.kt").read_text(
            encoding="utf-8"
        )
        manifest = (ROOT / "android/app/src/isolated/AndroidManifest.xml").read_text(encoding="utf-8")
        app_gradle = (ROOT / "android/app/build.gradle.kts").read_text(encoding="utf-8")

        self.assertIn("SEREA_ISOLATED_INSTRUMENTATION_FINISHED", harness)
        self.assertIn('test_package="app.serea.android.isolated.test"', harness)
        self.assertIn('package="app.serea.android.isolated"', harness)
        self.assertIn('activity_class="app.serea.android.FakeGoogleProcessDeathHarnessActivity"', harness)
        self.assertIn('activity="$package/$activity_class"', harness)
        self.assertIn('namespace = "app.serea.android"', app_gradle)
        self.assertIn("grep -Fxq 'Status: ok'", harness)
        self.assertIn('grep -Fq "Activity: $activity"', harness)
        self.assertIn('query_package_pid "$test_package"', harness)
        self.assertIn('shell ps -A', harness)
        self.assertNotIn('shell sh -c "$pid_query_script"', harness)
        self.assertIn('shell am force-stop "$package"', harness)
        self.assertNotIn("pm clear", harness.lower())
        self.assertNotIn("app.serea.android.demo", harness)
        self.assertIn("wait_target_stopped", harness)
        self.assertIn("PASS:$token:$phase:pid=", harness)
        self.assertIn('android:name=".FakeGoogleProcessDeathHarnessActivity"', manifest)
        self.assertIn('android:permission="android.permission.DUMP"', manifest)
        self.assertIn("ViewModelProvider(this@FakeGoogleProcessDeathHarnessActivity)", activity)
        self.assertIn("requestConsent(GoogleFeature.CALENDAR)", activity)
        self.assertIn("grantPendingConsent(preview)", activity)
        self.assertIn("CalendarProviderKind.FAKE", activity)
        self.assertIn("CalendarAuthorizationState.MOCK_ONLY", activity)
        self.assertIn("GoogleDemoFeatureState.DISCONNECTED", activity)
        self.assertNotIn("OkHttpClient", activity)
        self.assertNotIn("MockWebServer", activity)
        self.assertNotIn("|| true", harness)
        self.assertIn('awk -v target="$target_package"', harness)
        self.assertIn('if [[ -z "$matching_pids" ]]', harness)
        self.assertIn('if ! observed="$(query_package_pid "$package")"; then', harness)
        subprocess.run(["bash", "-n", str(ROOT / "tools/run_android_fake_google_process_death.sh")], check=True)

    def test_inflight_consent_completion_targets_original_androidtest_sender_once(self) -> None:
        script = (ROOT / "tools/run_android_inflight_consent_process_death.sh").read_text(encoding="utf-8")
        sender = (ROOT / "android/app/src/androidTest/java/app/serea/android/googleauth/SyntheticConsentSenderActivity.java").read_text(encoding="utf-8")
        receiver = (ROOT / "android/app/src/androidTest/java/app/serea/android/googleauth/SyntheticConsentSenderCompletionReceiver.java").read_text(encoding="utf-8")
        test_manifest = (ROOT / "android/app/src/androidTest/AndroidManifest.xml").read_text(encoding="utf-8")
        main_manifest = (ROOT / "android/app/src/main/AndroidManifest.xml").read_text(encoding="utf-8")
        isolated_manifest = (ROOT / "android/app/src/isolated/AndroidManifest.xml").read_text(encoding="utf-8")
        private_manifest = (ROOT / "android/app/src/privateBeta/AndroidManifest.xml").read_text(encoding="utf-8")

        self.assertIn("SyntheticConsentSenderCompletionReceiver", script)
        self.assertIn("am broadcast --receiver-foreground", script)
        self.assertNotIn('am start -W -f 0x04000000 -n "$sender_component"', script)
        self.assertIn("generation=1:session=session-A", script)
        self.assertNotIn("generation=41:session=session-A", script)
        self.assertIn('sender_created_count" == 1', script)
        self.assertIn('sender_completion_count" == 1', script)
        self.assertIn('sender_instance_nonce', script)
        self.assertIn("SENDER_CREATED:", sender)
        self.assertIn("SENDER_RESUMED:", sender)
        self.assertNotIn("kotlin.", sender)
        self.assertIn("SyntheticConsentSenderRegistry.current() != this", sender)
        self.assertIn("!token.equals(registeredToken) || !nonce.equals(registeredNonce)", sender)
        self.assertIn("if (completed ||", sender)
        self.assertNotIn("kotlin.", receiver)
        self.assertIn("ACTION_COMPLETE.equals(intent.getAction())", receiver)
        self.assertIn("DELIVERED_REJECTED", script)
        self.assertIn("NOT_DELIVERED", script)
        self.assertIn("DELIVERY_UNKNOWN", script)
        self.assertIn("NOT_RUN", script)
        self.assertIn("printf 'FAIL\\n' > \"$evidence/activity-result-delivery.txt\"", script)
        self.assertIn("status=CONSENT_PENDING", script)
        self.assertIn(
            'B_READY:$SEREA_CONSENT_DEATH_RUN_ID:request=request-B:generation=1:session=session-B:status=CONSENT_PENDING:',
            script,
        )
        self.assertNotIn(
            'B_READY:$SEREA_CONSENT_DEATH_RUN_ID:request=request-B:generation=1:session=session-B:pid=',
            script,
        )
        self.assertIn("synthetic-consent-result-A", script)
        self.assertIn("duplicate-sender-completion.txt", script)
        self.assertIn("SENDER_COMPLETION_REJECTED", script)
        self.assertIn("sender_process_after_completion=process-exited-after-original-Activity-finished", script)
        self.assertIn("SIGKILL_PRECHECK", script)
        self.assertIn('NR > 1 && $1 == pid', script)
        self.assertIn("STALE_REPLAY_REJECTED", script)
        self.assertIn("complete-B", script)
        self.assertIn("B_VALID_RESULT_DELIVERED", script)
        self.assertIn("android:permission=\"android.permission.DUMP\"", test_manifest)
        self.assertIn("SyntheticConsentSenderCompletionReceiver", test_manifest)
        self.assertIn("SyntheticConsentBResultActivity", isolated_manifest)
        self.assertNotIn("SyntheticConsentSenderCompletionReceiver", main_manifest)
        self.assertNotIn("SyntheticConsentSenderCompletionReceiver", isolated_manifest)
        self.assertNotIn("SyntheticConsentBResultActivity", main_manifest)
        self.assertNotIn("SyntheticConsentBResultActivity", private_manifest)
        subprocess.run(["bash", "-n", str(ROOT / "tools/run_android_inflight_consent_process_death.sh")], check=True)

    def test_fake_google_process_death_pid_queries_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            bindir = root / "bin"
            bindir.mkdir()
            fake_adb = bindir / "adb"
            fake_adb.write_text(
                "#!/usr/bin/env bash\n"
                "set -euo pipefail\n"
                "printf '%s\\n' \"$*\" >> \"$ADB_CALLS\"\n"
                "if [[ \"${1:-}\" == devices ]]; then\n"
                "  printf 'List of devices attached\\nemulator-5554 device product:ci model:ci\\n'\n"
                "  exit 0\n"
                "fi\n"
                "if [[ \"${1:-}\" == -s ]]; then shift 2; fi\n"
                "command=\"$1\"; shift\n"
                "if [[ \"$command\" == logcat ]]; then\n"
                "  phase=\"$(cat \"$ADB_CURRENT_PHASE\")\"\n"
                "  token=\"$(cat \"$ADB_CURRENT_TOKEN\")\"\n"
                "  printf 'I/SereaFakeGoogleDeath: PASS:%s:%s:pid=2101\\n' \"$token\" \"$phase\"\n"
                "  exit 0\n"
                "fi\n"
                "[[ \"$command\" == shell ]] || exit 90\n"
                "operation=\"$1\"; shift\n"
                "case \"$operation\" in\n"
                "  getprop)\n"
                "    case \"$1\" in\n"
                "      ro.build.version.sdk) printf '35\\n' ;;\n"
                "      ro.kernel.qemu) printf '1\\n' ;;\n"
                "      *) exit 91 ;;\n"
                "    esac\n"
                "    ;;\n"
                "  pm)\n"
                "    [[ \"$1\" == path ]] || exit 92\n"
                "    printf 'package:/data/app/app.serea.android.isolated/base.apk\\n'\n"
                "    ;;\n"
                "  ps)\n"
                "    [[ \"$1\" == -A && \"$2\" == -o && \"$3\" == PID,NAME ]] || exit 93\n"
                "    stopped=\"$(cat \"$ADB_FORCE_STOPPED\")\"\n"
                "    case \"${PID_BAD_PROCESS_TABLE:-}\" in\n"
                "      missing-header) printf 'incomplete process table\\n'; exit 0 ;;\n"
                "      header-only) printf 'PID NAME\\n'; exit 0 ;;\n"
                "      truncated-row) printf 'PID NAME\\n2101\\n'; exit 0 ;;\n"
                "    esac\n"
                "    if [[ -n \"${PID_FAIL_PACKAGE:-}\" && ( -z \"${PID_FAIL_AFTER_FORCE_STOP:-}\" || \"$stopped\" == 1 ) ]]; then\n"
                "      exit 41\n"
                "    fi\n"
                "    printf 'PID NAME\\n'\n"
                "    printf '100 system_server\\n'\n"
                "    if [[ \"$stopped\" != 1 ]]; then\n"
                "      case \"$(cat \"$ADB_CURRENT_PHASE\")\" in\n"
                "        seed) pid=2101 ;;\n"
                "        verify) pid=2102 ;;\n"
                "        *) pid=2100 ;;\n"
                "      esac\n"
                "      printf '%s app.serea.android.isolated\\n' \"$pid\"\n"
                "    fi\n"
                "    ;;\n"
                "  am)\n"
                "    action=\"$1\"; shift\n"
                "    if [[ \"$action\" == force-stop ]]; then\n"
                "      count=\"$(cat \"$ADB_FORCE_STOP_COUNT\")\"\n"
                "      printf '%s\\n' \"$((count + 1))\" > \"$ADB_FORCE_STOP_COUNT\"\n"
                "      printf '1\\n' > \"$ADB_FORCE_STOPPED\"\n"
                "      exit 0\n"
                "    fi\n"
                "    [[ \"$action\" == start ]] || exit 94\n"
                "    requested_activity=\"\"\n"
                "    while [[ $# -gt 0 ]]; do\n"
                "      if [[ \"$1\" == --es && \"${2:-}\" == phase ]]; then printf '%s\\n' \"$3\" > \"$ADB_CURRENT_PHASE\"; printf '%s\\n' \"$3\" >> \"$ADB_PHASES\"; shift 3;\n"
                "      elif [[ \"$1\" == --es && \"${2:-}\" == token ]]; then printf '%s\\n' \"$3\" > \"$ADB_CURRENT_TOKEN\"; shift 3;\n"
                "      elif [[ \"$1\" == -n ]]; then requested_activity=\"$2\"; shift 2;\n"
                "      else shift; fi\n"
                "    done\n"
                "    printf '0\\n' > \"$ADB_FORCE_STOPPED\"\n"
                "    if [[ \"${ADB_START_STATUS:-ok}\" == ok ]]; then printf 'Status: ok\\nActivity: %s\\n' \"$requested_activity\"; else printf 'Status: error\\nError: Activity class does not exist\\n'; fi\n"
                "    ;;\n"
                "  logcat)\n"
                "    phase=\"$(cat \"$ADB_CURRENT_PHASE\")\"\n"
                "    token=\"$(cat \"$ADB_CURRENT_TOKEN\")\"\n"
                "    printf 'I/SereaFakeGoogleDeath: PASS:%s:%s:pid=2101\\n' \"$token\" \"$phase\"\n"
                "    ;;\n"
                "  *) exit 95 ;;\n"
                "esac\n",
                encoding="utf-8",
            )
            fake_adb.chmod(0o755)

            helper = ROOT / "tools/run_android_fake_google_process_death.sh"
            target_package = "app.serea.android.isolated"
            test_package = "app.serea.android.isolated.test"

            def run_case(case_name: str, *, fail_package: str = "", fail_after_force_stop: bool = False,
                         bad_process_table: str = "", am_start_status: str = "ok"):
                case_dir = root / case_name
                case_dir.mkdir()
                environment = os.environ.copy()
                environment.update({
                    "PATH": f"{bindir}{os.pathsep}{environment['PATH']}",
                    "CI": "true",
                    "GITHUB_ACTIONS": "true",
                    "SEREA_DISPOSABLE_CI_EMULATOR": "1",
                    "GITHUB_RUN_ID": "54321",
                    "EXPECTED_API_LEVEL": "35",
                    "SEREA_ISOLATED_INSTRUMENTATION_FINISHED": "1",
                    "SEREA_FAKE_GOOGLE_DEATH_TOKEN": f"{case_name}-token",
                    "SEREA_FAKE_GOOGLE_DEATH_EVIDENCE_DIR": str(case_dir / "evidence"),
                    "ADB_PID_QUERIES": str(case_dir / "pid-queries"),
                    "ADB_CALLS": str(case_dir / "adb-calls"),
                    "ADB_FORCE_STOP_COUNT": str(case_dir / "force-stop-count"),
                    "ADB_FORCE_STOPPED": str(case_dir / "force-stopped"),
                    "ADB_CURRENT_PHASE": str(case_dir / "phase"),
                    "ADB_PHASES": str(case_dir / "phases"),
                    "ADB_CURRENT_TOKEN": str(case_dir / "token"),
                    "PID_FAIL_PACKAGE": fail_package,
                    "PID_FAIL_AFTER_FORCE_STOP": "1" if fail_after_force_stop else "",
                    "PID_BAD_PROCESS_TABLE": bad_process_table,
                    "ADB_START_STATUS": am_start_status,
                })
                (case_dir / "pid-queries").write_text("", encoding="utf-8")
                (case_dir / "force-stop-count").write_text("0\n", encoding="utf-8")
                initial_stopped = "0\n" if fail_after_force_stop else "1\n"
                (case_dir / "force-stopped").write_text(initial_stopped, encoding="utf-8")
                (case_dir / "phase").write_text("none\n", encoding="utf-8")
                (case_dir / "token").write_text("none\n", encoding="utf-8")
                return subprocess.run(
                    ["bash", str(helper)], env=environment, capture_output=True, text=True, check=False,
                ), case_dir

            instrumentation_failure, instrumentation_case = run_case(
                "instrumentation-query-failure", fail_package=test_package,
            )
            self.assertNotEqual(0, instrumentation_failure.returncode)
            self.assertEqual("0\n", (instrumentation_case / "force-stop-count").read_text(encoding="utf-8"))
            self.assertEqual("FAIL\n", (instrumentation_case / "evidence/status.txt").read_text(encoding="utf-8"))
            self.assertNotIn("PASS:", instrumentation_failure.stdout + instrumentation_failure.stderr)

            target_failure, target_case = run_case("target-query-failure", fail_package=target_package)
            self.assertNotEqual(0, target_failure.returncode)
            self.assertEqual("0\n", (target_case / "force-stop-count").read_text(encoding="utf-8"))
            self.assertEqual("FAIL\n", (target_case / "evidence/status.txt").read_text(encoding="utf-8"))
            self.assertNotIn("PASS:", target_failure.stdout + target_failure.stderr)

            for malformed_kind in ("missing-header", "header-only", "truncated-row"):
                with self.subTest(malformed_kind=malformed_kind):
                    malformed_table, malformed_case = run_case(
                        f"malformed-process-table-{malformed_kind}", bad_process_table=malformed_kind,
                    )
                    self.assertNotEqual(0, malformed_table.returncode)
                    self.assertEqual("0\n", (malformed_case / "force-stop-count").read_text(encoding="utf-8"))
                    self.assertEqual("FAIL\n", (malformed_case / "evidence/status.txt").read_text(encoding="utf-8"))
                    self.assertNotIn("PASS:", malformed_table.stdout + malformed_table.stderr)

            force_stop_query_failure, stop_failure_case = run_case(
                "force-stop-query-failure", fail_package=target_package, fail_after_force_stop=True,
            )
            self.assertNotEqual(0, force_stop_query_failure.returncode)
            self.assertEqual("1\n", (stop_failure_case / "force-stop-count").read_text(encoding="utf-8"))
            self.assertEqual("FAIL\n", (stop_failure_case / "evidence/status.txt").read_text(encoding="utf-8"))
            self.assertNotIn("PASS:", force_stop_query_failure.stdout + force_stop_query_failure.stderr)

            successful_absence, success_case = run_case("successful-absence")
            self.assertEqual(
                0,
                successful_absence.returncode,
                successful_absence.stderr + "\nADB calls:\n" + (success_case / "adb-calls").read_text(encoding="utf-8"),
            )
            self.assertEqual("PASS\n", (success_case / "evidence/status.txt").read_text(encoding="utf-8"))
            self.assertEqual("2\n", (success_case / "force-stop-count").read_text(encoding="utf-8"))
            self.assertEqual("seed\nverify\n", (success_case / "phases").read_text(encoding="utf-8"))
            self.assertEqual("absent\n", (success_case / "evidence/post-force-stop-pid.txt").read_text(encoding="utf-8"))
            self.assertIn("PASS: Fake Google consent/session OS process-death boundary", successful_absence.stdout)

            activity_launch_failure, activity_failure_case = run_case(
                "activity-launch-failure", am_start_status="error",
            )
            self.assertNotEqual(0, activity_launch_failure.returncode)
            self.assertEqual("FAIL\n", (activity_failure_case / "evidence/status.txt").read_text(encoding="utf-8"))
            self.assertNotIn("logcat", (activity_failure_case / "adb-calls").read_text(encoding="utf-8"))
            self.assertIn("Status: error", activity_launch_failure.stderr)
            self.assertIn("Activity class does not exist", activity_launch_failure.stderr)
            self.assertNotIn("Timed out waiting for phase seed", activity_launch_failure.stderr)

    def test_personal_beta_campaign_seeds_and_claims_marker_before_single_use_consume(self) -> None:
        script = (ROOT / "tools/test_personal_beta_identity.sh").read_text(encoding="utf-8")
        receiver = (ROOT / "android/app/src/debug/java/app/serea/android/ProcessDeathHarnessReceiver.kt").read_text(
            encoding="utf-8"
        )
        phases = receiver.split('if (phase == "campaign-seed-and-claim") {', 1)[1].split("}", 1)[0]
        personal_beta_allowlist = receiver.split("private val PERSONAL_BETA_PHASES = setOf(", 1)[1].split(")", 1)[0]
        script_seed = script.index("phase campaign-seed-and-claim")
        script_consume = script.index("phase campaign-consume", script_seed)
        self.assertLess(phases.index("seedInstrumentationMarker(context, token)"),
                        phases.index("claimInstrumentationMarker(context, token)"))
        self.assertIn('"campaign-seed-and-claim"', personal_beta_allowlist)
        self.assertIn('"campaign-consume"', personal_beta_allowlist)
        target_package_mapping = receiver.split("val expectedPackage = when (phase) {", 1)[1].split("}", 1)[0]
        self.assertIn('"campaign-seed-and-claim" -> PERSONAL_BETA_PACKAGE', target_package_mapping)
        self.assertLess(script_seed, script_consume)
        self.assertIn('check(!claimedMarker.exists())', receiver)
        self.assertIn('check(!authorization.exists())', receiver)

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

    def test_formal_api_jobs_are_independent_and_aggregate_fail_closed(self) -> None:
        import yaml

        workflow = yaml.safe_load((ROOT / ".github/workflows/android.yml").read_text(encoding="utf-8"))
        jobs = workflow["jobs"]
        probe_workflow = (ROOT / ".github/workflows/android-host-probe.yml").read_text(encoding="utf-8")
        self.assertIn("  workflow_call:", probe_workflow)
        self.assertIn("  workflow_dispatch:", probe_workflow)
        self.assertIn("serea-android-host-probe-attempt-${{ github.run_attempt }}", probe_workflow)
        self.assertIn("record-kvm --api host-probe --phase-detail before", probe_workflow)
        self.assertIn("record-kvm --api host-probe --phase-detail after", probe_workflow)
        self.assertNotIn("system-images;android-", probe_workflow)
        probe_yaml = yaml.safe_load(probe_workflow)
        probe_steps = probe_yaml["jobs"]["probe"]["steps"]
        probe_upload = next(step for step in probe_steps if "upload-artifact" in step.get("uses", ""))
        self.assertEqual(probe_upload["if"], "always()")
        self.assertEqual(probe_upload["with"]["if-no-files-found"], "error")
        self.assertGreaterEqual(
            next(i for i, step in enumerate(probe_steps) if "phase-detail before" in step.get("run", "")),
            0,
        )
        self.assertGreater(
            next(i for i, step in enumerate(probe_steps) if "phase-detail after" in step.get("run", "")),
            next(i for i, step in enumerate(probe_steps) if "phase-detail before" in step.get("run", "")),
        )
        probe_job = jobs["android-host-probe"]
        self.assertEqual(probe_job["needs"], "android-changes")
        self.assertEqual(probe_job["uses"], "./.github/workflows/android-host-probe.yml")
        for api in ("26", "35"):
            job = jobs[f"android-emulator-api{api}"]
            self.assertEqual(job["needs"], ["android-changes", "android-build", "android-host-probe"])
            self.assertIn("needs.android-host-probe.result == 'success'", job["if"])
            self.assertEqual(job["outputs"]["artifact_attempt"], "${{ github.run_attempt }}")
            self.assertTrue(any("android-emulator-runner" in step.get("uses", "") for step in job["steps"]))
            artifacts = [
                step
                for step in job["steps"]
                if "upload-artifact" in step.get("uses", "")
            ]
            self.assertEqual(
                [step.get("with", {}).get("name") for step in artifacts],
                [
                    f"serea-android-formal-api{api}-provisional-attempt-${{{{ github.run_attempt }}}}",
                    f"serea-android-host-api{api}-attempt-${{{{ github.run_attempt }}}}",
                ],
            )
            self.assertEqual(artifacts[0]["if"], "always()")
            self.assertIn("androidTest-results/api", artifacts[0]["with"]["path"])
            self.assertIn("${{ github.run_attempt }}", artifacts[0]["with"]["name"])
            self.assertEqual(artifacts[1]["if"], "always()")
            self.assertIn("${{ runner.temp }}/serea-android-host-api", artifacts[1]["with"]["path"])
            self.assertEqual(artifacts[1]["with"]["if-no-files-found"], "error")
            provenance = next(step for step in job["steps"] if step.get("name") == f"Prepare API {api} artifact with producer provenance")
            self.assertIn("producer_attempt=$GITHUB_RUN_ATTEMPT", provenance["run"])

        aggregate = jobs["android-storage-emulator"]
        self.assertEqual(
            aggregate["needs"],
            ["android-changes", "android-build", "android-emulator-api26", "android-emulator-api35"],
        )
        self.assertFalse(any("android-emulator-runner" in step.get("uses", "") for step in aggregate["steps"]))
        self.assertEqual(aggregate["outputs"]["instrumentation"], "${{ steps.instrumentation.outputs.result }}")
        api_downloads = [step for step in aggregate["steps"] if step.get("name") in ("Download API 26 evidence", "Download API 35 evidence")]
        self.assertEqual(
            [step["with"]["name"] for step in api_downloads],
            [
                "serea-android-formal-api26-provisional-attempt-${{ needs.android-emulator-api26.outputs.artifact_attempt }}",
                "serea-android-formal-api35-provisional-attempt-${{ needs.android-emulator-api35.outputs.artifact_attempt }}",
            ],
        )
        verification = next(step for step in aggregate["steps"] if step.get("id") == "instrumentation")["run"]
        self.assertIn("verify_android_formal_aggregate.py", verification)
        module = (ROOT / "tools/verify_android_formal_aggregate.py").read_text(encoding="utf-8")
        for required in ("api26_result", "api35_result", "105", "skipped", "process-death-status.txt", "data-isolation-status.txt", "source_sha", "run_id"):
            self.assertIn(required, module)
        self.assertIn("result=PASS", module)
        self.assertIn("result=FAIL", module)
        gate = jobs["android-emulator-required-gate"]
        self.assertIn("android-host-probe", gate["needs"])
        self.assertEqual(gate["steps"][2]["env"]["HOST_PROBE_RESULT"], "${{ needs.android-host-probe.result }}")
        self.assertIn('"host-probe:$HOST_PROBE_RESULT"', gate["steps"][2]["run"])

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
                "elif [[ \"$1 $2 $3 $4 $5\" == 'shell settings get secure default_input_method' ]]; then\n"
                "  printf 'com.google.android.inputmethod.latin/.LatinIME\\n'\n"
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
