from __future__ import annotations

import contextlib
import fcntl
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock

from tools import serea_apk_signer
from tools import s1b_campaign_safety as safety
from tools import s1b_campaign_install as campaign_install


PROJECT = Path(__file__).resolve().parents[1]
SDK = Path(os.environ.get("ANDROID_SDK_ROOT") or os.environ.get("ANDROID_HOME") or
           (Path.home() / "Library/Android/sdk"))
BUILD_TOOLS = SDK / "build-tools/35.0.0"
ANDROID_JAR = SDK / "platforms/android-35/android.jar"
JDK_HOME = Path.home() / "Library/Java/JavaVirtualMachines/temurin-17.jdk/Contents/Home"
AAPT = BUILD_TOOLS / "aapt"
APKSIGNER = BUILD_TOOLS / "apksigner"
KEYTOOL = JDK_HOME / "bin/keytool"


@unittest.skipUnless(all(path.is_file() for path in (AAPT, APKSIGNER, ANDROID_JAR, KEYTOOL)),
                     "Android Build Tools 35/JDK 17 unavailable; synthetic APK integration NOT_RUN")
class CampaignApkFixture(unittest.TestCase):
    campaign_id = "s1b0123456789"
    target_package = f"app.serea.android.isolated.{campaign_id}"
    runner_name = "app.serea.android.SereaIsolatedAndroidJUnitRunner"

    @classmethod
    def setUpClass(cls):
        cls.key_temp = tempfile.TemporaryDirectory(prefix="s1b-test-key-")
        cls.key_root = Path(cls.key_temp.name)
        os.chmod(cls.key_root, 0o700)
        cls.password = "test-only-synthetic-passphrase"
        cls.alias = "s1b-synthetic-test-only"
        cls.keystore = cls.key_root / "synthetic-test-only.p12"
        generated = serea_apk_signer._tool_run([
            KEYTOOL, "-genkeypair", "-noprompt", "-storetype", "PKCS12", "-keystore", cls.keystore,
            "-storepass", cls.password, "-keypass", cls.password, "-alias", cls.alias,
            "-dname", "CN=S1B synthetic test only", "-validity", "2", "-keyalg", "RSA",
            "-keysize", "2048",
        ])
        if generated.returncode:
            raise RuntimeError("Could not generate ephemeral test-only certificate")
        exported = serea_apk_signer._tool_run([
            KEYTOOL, "-exportcert", "-keystore", cls.keystore, "-storetype", "PKCS12",
            "-storepass", cls.password, "-alias", cls.alias,
        ])
        if exported.returncode or not exported.stdout:
            raise RuntimeError("Could not export ephemeral test certificate")
        cls.test_fingerprint = hashlib.sha256(exported.stdout).hexdigest()
        builder = cls("runTest")
        builder.root = cls.key_root
        builder._fixture_counter = 0
        cls.baseline_target = builder.build_signed_apk("target")
        cls.baseline_test = builder.build_signed_apk("test")

    @classmethod
    def tearDownClass(cls):
        cls.key_temp.cleanup()

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="s1b-host-safety-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        os.chmod(self.root, 0o700)
        self.stage = self.root / "stage"
        self.stage.mkdir(mode=0o700)
        self.trust_anchor = self.root / "trusted-diagnostic-cert.json"
        self.write_trust_anchor(self.test_fingerprint)
        self._fixture_counter = 0
        self.ledger = self.root / "ledger"
        self.ledger.mkdir(mode=0o700)
        (self.ledger / ".reservation.lock").touch(mode=0o600)
        os.chmod(self.ledger / ".reservation.lock", 0o600)

    def write_trust_anchor(self, fingerprint: str):
        path = self.root / "trusted-diagnostic-cert.json"
        path.write_text(json.dumps({"schema": 1, "purpose": "S1B_SYNTHETIC_DIAGNOSTIC",
                                   "certificate_sha256": fingerprint}), encoding="utf-8")
        os.chmod(path, 0o600)
        return path

    def build_signed_apk(self, role: str, *, package: str | None = None,
                         target_package: str | None = None, runner: str | None = None,
                         debuggable: bool = True, component: str | None = None,
                         internet: bool = False, shared_user_id: bool = False,
                         min_sdk: int = 26) -> Path:
        package = package or (self.target_package if role == "target" else self.target_package + ".test")
        self._fixture_counter += 1
        stem = f"{role}-{self._fixture_counter}"
        manifest = self.root / "AndroidManifest.xml"
        uses_internet = '<uses-permission android:name="android.permission.INTERNET" />' if internet else ""
        shared_user = ' android:sharedUserId="app.serea.shared"' if shared_user_id else ""
        component_xml = component if component is not None else (
            '<activity android:name="app.serea.android.MainActivity" android:exported="false" />'
            if role == "target" else
            '<activity android:name="androidx.test.core.app.InstrumentationActivityInvoker$EmptyActivity" '
            'android:exported="true" />'
        )
        instrumentation = ""
        if role == "test":
            instrumentation = (
                f'<instrumentation android:name="{runner or self.runner_name}" '
                f'android:targetPackage="{target_package or self.target_package}" />'
            )
        manifest.write_text(
            '<manifest xmlns:android="http://schemas.android.com/apk/res/android" '
            f'package="{package}"{shared_user}>{uses_internet}'
            f'<uses-sdk android:minSdkVersion="{min_sdk}" android:targetSdkVersion="35" />'
            f'<application android:debuggable="{str(debuggable).lower()}" android:allowBackup="false">'
            f'{component_xml}</application>{instrumentation}</manifest>',
            encoding="utf-8",
        )
        unsigned = self.root / f"{stem}-unsigned.apk"
        packaged = serea_apk_signer._tool_run([
            AAPT, "package", "-f", "-M", manifest, "-I", ANDROID_JAR, "-F", unsigned,
        ])
        if packaged.returncode:
            raise RuntimeError("aapt could not package synthetic fixture manifest: " +
                               packaged.stderr.decode("utf-8", "replace"))
        signed = self.root / f"{stem}.apk"
        signed_result = serea_apk_signer._tool_run([
            APKSIGNER, "sign", "--ks", self.keystore, "--ks-type", "PKCS12",
            "--ks-pass", f"pass:{self.password}", "--key-pass", f"pass:{self.password}",
            "--ks-key-alias", self.alias, "--out", signed, unsigned,
        ])
        if signed_result.returncode:
            raise RuntimeError("apksigner could not sign synthetic fixture APK")
        return signed

    def copy_baseline(self, role: str) -> Path:
        self._fixture_counter += 1
        result = self.root / f"{role}-baseline-{self._fixture_counter}.apk"
        shutil.copyfile(self.baseline_target if role == "target" else self.baseline_test, result)
        return result

    def valid_pair(self):
        return self.copy_baseline("target"), self.copy_baseline("test")

    def verify(self, target: Path, test: Path, *, campaign_id: str | None = None,
               trust_anchor: Path | None | object = ..., aapt: Path = AAPT, apksigner: Path = APKSIGNER):
        anchor = self.trust_anchor if trust_anchor is ... else trust_anchor
        return safety.verify_campaign_apks(
            target_apk=target, test_apk=test, campaign_id=campaign_id or self.campaign_id,
            staging_root=self.stage, trust_anchor=anchor, aapt=aapt, apksigner=apksigner,
        )

    def write_campaign_record(self, *, state: str = "PREPARED_ONLY_NOT_BUILT_NOT_INSTALLED",
                              campaign_id: str | None = None):
        campaign_id = campaign_id or self.campaign_id
        record = {
            "campaign_id": campaign_id,
            "profile": "api35-4k",
            "avd": "serea_s1_01a3",
            "target_package": f"app.serea.android.isolated.{campaign_id}",
            "test_package": f"app.serea.android.isolated.{campaign_id}.test",
            "campaign_token": "a" * 32,
            "created_utc": "2026-01-01T00:00:00+00:00",
            "state": state,
            "minimum_free_gib_at_prepare": 8,
            "constraints": "No existing AVD/app/db/logs may be cleared or overwritten; inspect package identity before install.",
        }
        path = self.ledger / f"{campaign_id}.json"
        path.write_text(json.dumps(record), encoding="utf-8")
        os.chmod(path, 0o600)
        return path

    def test_full_runtime_runner_with_simulated_adb_reaches_pass_without_mocking_runtime_functions(self):
        active_target = os.environ.get("S1B_RUNTIME_INTEGRATION_TARGET_APK")
        active_test = os.environ.get("S1B_RUNTIME_INTEGRATION_TEST_APK")
        active_mode = bool(active_target and active_test and
                           os.environ.get("S1B_RUNTIME_INTEGRATION_ACTIVE_SIGNER") == "1")
        campaign_id = os.environ.get("S1B_RUNTIME_INTEGRATION_CAMPAIGN_ID", self.campaign_id)
        target, test = ((Path(active_target), Path(active_test)) if active_mode else self.valid_pair())
        record_path = self.write_campaign_record(campaign_id=campaign_id)
        record = json.loads(record_path.read_text(encoding="utf-8"))
        record["owner_approved_scope"] = safety.S1B_OWNER_SCOPE
        record["method"] = "ownership-recovery-r2"
        record_path.write_text(json.dumps(record), encoding="utf-8")
        os.chmod(record_path, 0o600)
        campaign_stage = self.stage / campaign_id
        campaign_stage.mkdir(mode=0o700)
        adb = SDK / "platform-tools/adb"
        serial = "emulator-5554"
        packages = {"android", "com.android.settings"}
        markers = set()
        calls = []
        installed_bytes = {
            record["target_package"]: target.read_bytes(),
            record["test_package"]: test.read_bytes(),
        }
        expected_method = safety.S1B_OWNER_METHODS["ownership-recovery-r2"]

        def simulated_adb(argv, **kwargs):
            command = [str(part) for part in argv]
            calls.append(command)
            tail = command[1:]
            if tail == ["devices", "-l"]:
                stdout = "List of devices attached\n" + serial + " device product:sdk_gphone64_x86_64\n"
            elif tail == ["-s", serial, "get-state"]:
                stdout = "device\n"
            elif tail == ["-s", serial, "emu", "avd", "name"]:
                stdout = "serea_s1_01a3\nOK\n"
            elif tail == ["-s", serial, "shell", "getprop", "sys.boot_completed"]:
                stdout = "1\n"
            elif tail == ["-s", serial, "shell", "getprop", "ro.build.version.sdk"]:
                stdout = "35\n"
            elif tail == ["-s", serial, "shell", "getprop", "ro.product.cpu.abi"]:
                stdout = "x86_64\n"
            elif tail == ["-s", serial, "shell", "getconf", "PAGE_SIZE"]:
                stdout = "4096\n"
            elif tail == ["-s", serial, "shell", "pm", "list", "packages"]:
                stdout = "".join(f"package:{item}\n" for item in sorted(packages))
            elif len(tail) >= 5 and tail[0:2] == ["-s", serial] and tail[2] == "install":
                apk = Path(tail[-1])
                digest = hashlib.sha256(apk.read_bytes()).hexdigest()
                package_name = next(package for package, contents in installed_bytes.items()
                                    if hashlib.sha256(contents).hexdigest() == digest)
                packages.add(package_name)
                stdout = "Performing Push Install\nSuccess\n"
            elif tail[:4] == ["-s", serial, "shell", "pm"] and tail[4] == "path":
                package_name = tail[5]
                stdout = f"package:/data/app/{package_name}/base.apk\n"
            elif tail[:3] == ["-s", serial, "pull"]:
                remote, destination = tail[3], Path(tail[4])
                package_name = remote.split("/")[-2]
                destination.write_bytes(installed_bytes[package_name])
                stdout = "1 file pulled\n"
            elif tail[:3] == ["-s", serial, "shell"] and tail[-1].startswith("run-as "):
                shell = tail[-1]
                if "printf %s" in shell:
                    markers.add("serea-instrumentation-token")
                    stdout = ""
                elif "test ! -e no_backup/serea-instrumentation-token" in shell:
                    stdout = "\n" if markers else ""
                elif "ls -1 no_backup" in shell:
                    stdout = "\n".join(sorted(markers)) + "\n"
                else:
                    raise AssertionError("unexpected run-as command: " + shell)
            elif tail[:5] == ["-s", serial, "shell", "am", "instrument"]:
                markers.discard("serea-instrumentation-token")
                markers.add("serea-instrumentation-claimed")
                markers.add("serea-core")
                class_name, test_name = expected_method.split("#", 1)
                stdout = (
                    f"INSTRUMENTATION_STATUS: class={class_name}\n"
                    "INSTRUMENTATION_STATUS: current=1\nINSTRUMENTATION_STATUS: numtests=1\n"
                    f"INSTRUMENTATION_STATUS: test={test_name}\n"
                    "INSTRUMENTATION_STATUS_CODE: 1\n"
                    f"INSTRUMENTATION_STATUS: class={class_name}\n"
                    "INSTRUMENTATION_STATUS: current=1\nINSTRUMENTATION_STATUS: numtests=1\n"
                    f"INSTRUMENTATION_STATUS: test={test_name}\n"
                    "INSTRUMENTATION_STATUS_CODE: 0\n"
                    "INSTRUMENTATION_RESULT: stream=\nOK (1 test)\n"
                    "INSTRUMENTATION_CODE: -1\n"
                )
            elif tail[:3] == ["-s", serial, "uninstall"]:
                packages.discard(tail[3])
                stdout = "Success\n"
            else:
                raise AssertionError("unexpected simulated ADB argv: " + repr(command))
            return subprocess.CompletedProcess(command, 0, stdout=stdout, stderr="")

        assessment = {
            "runtime_plan": {
                "profile": "api35-4k", "avd": "serea_s1_01a3",
                "first_method": expected_method, "recovery_method": expected_method,
                "second_method": "app.serea.android.EmbeddedCoreDiagnosticChatUiTest#diagnosticChatUsesCoreMockAcrossRecreationAndCleansUp",
                "build_only_package_suffix": "s1b0000000000",
            },
            "certificate_sha256": self.test_fingerprint,
        }
        with contextlib.ExitStack() as patches:
            if not active_mode:
                patches.enter_context(mock.patch.object(campaign_install, "require_android_common_lock"))
                patches.enter_context(mock.patch.object(
                    campaign_install, "require_active_signing_generation",
                    return_value=(self.trust_anchor, assessment)))
            result = campaign_install.run_campaign_install(
                target_apk=target, test_apk=test, campaign_id=campaign_id,
                profile="api35-4k", method_key="ownership-recovery-r2",
                staging_root=campaign_stage, ledger_directory=self.ledger,
                sdk_root=SDK, runner=simulated_adb,
            )
        self.assertEqual("RUNTIME_PASS", result["status"])
        self.assertEqual("RUNTIME_PASS", json.loads(record_path.read_text())["state"])
        self.assertEqual("CLEANUP_PASS", json.loads(record_path.read_text())["runtime_cleanup_status"])
        self.assertFalse(packages.intersection({record["target_package"], record["test_package"]}))
        self.assertEqual(1, sum("am" in command and "instrument" in command for command in calls))
        self.assertTrue(any("marker-precheck" in path.name for path in Path(result["adb_evidence_directory"]).glob("*.json")))
        self.assertEqual(campaign_stage / "adb-evidence", Path(result["adb_evidence_directory"]))
        self.assertFalse((self.stage / "adb-evidence").exists())
        self.assertEqual(["serea-core", "serea-instrumentation-claimed"], sorted(markers))

    def test_runtime_campaign_pair_is_not_pinned_to_build_only_apk_digests(self):
        target, test = self.valid_pair()
        verified = self.verify(target, test)
        assessment = {
            'runtime_plan': {
                'profile': 'api35-4k', 'avd': 'serea_s1_01a3',
                'first_method': 'app.serea.android.core.EmbeddedCoreAdapterTest#processOwnerSurvivesAdapterReplacement',
                'recovery_method': 'app.serea.android.core.EmbeddedCoreAdapterTest#processOwnerSurvivesAdapterReplacement',
                'second_method': 'app.serea.android.EmbeddedCoreDiagnosticChatUiTest#diagnosticChatUsesCoreMockAcrossRecreationAndCleansUp',
                'campaign_ids': 'ISSUE_SEPARATELY_FROM_PRIVATE_HOST_LEDGER_AFTER_PIN',
                'build_only_package_suffix': 's1b1c35bb0dae',
                'build_only_apks_are_runtime_targets': False,
                'candidate_transferable': False, 'max_attempts_per_campaign': 1,
                'second_campaign_requires_first_runtime_pass': True, 'runtime_authorized': False,
            },
            'target_apk_sha256': '0' * 64,
            'test_apk_sha256': '1' * 64,
            'certificate_sha256': verified.certificate_sha256,
        }
        self.assertNotEqual(verified.target.sha256, assessment['target_apk_sha256'])
        self.assertNotEqual(verified.test.sha256, assessment['test_apk_sha256'])
        campaign_install.checked_campaign_review(assessment, verified, 'api35-4k')

    def test_install_runner_claims_verified_pair_and_uses_no_replace_flags(self):
        target, test = self.valid_pair()
        verified = self.verify(target, test)
        record_path = self.write_campaign_record()
        fake_adb = self.root / 'adb'
        fake_adb.write_text('#!/bin/sh\nexit 0\n', encoding='utf-8')
        os.chmod(fake_adb, 0o700)
        android_root = self.root / 'Android'
        locks = android_root / 'locks'
        locks.mkdir(mode=0o700, parents=True)
        lock_path = locks / 'common.lock'
        lock_path.touch(mode=0o600)
        os.chmod(lock_path, 0o600)
        calls = []
        package_queries = 0
        serial = 'emulator-5554'

        def fake_run(argv, **kwargs):
            nonlocal package_queries
            command = [str(part) for part in argv]
            calls.append(command)
            if command == [str(fake_adb), 'devices', '-l']:
                output = 'List of devices attached\n' + serial + ' device product:sdk_gphone64_x86_64\n'
            elif command == [str(fake_adb), '-s', serial, 'get-state']:
                output = 'device\n'
            elif command == [str(fake_adb), '-s', serial, 'emu', 'avd', 'name']:
                output = 'serea_s1_01a3\nOK\n'
            elif command == [str(fake_adb), '-s', serial, 'shell', 'getprop', 'sys.boot_completed']:
                output = '1\n'
            elif command == [str(fake_adb), '-s', serial, 'shell', 'getprop', 'ro.build.version.sdk']:
                output = '35\n'
            elif command == [str(fake_adb), '-s', serial, 'shell', 'getprop', 'ro.product.cpu.abi']:
                output = 'x86_64\n'
            elif command == [str(fake_adb), '-s', serial, 'shell', 'getconf', 'PAGE_SIZE']:
                output = '4096\n'
            elif command == [str(fake_adb), '-s', serial, 'shell', 'pm', 'list', 'packages']:
                package_queries += 1
                output = 'package:android\n'
                if package_queries > 1:
                    output += 'package:' + verified.target.application_id + '\n'
                    output += 'package:' + verified.test.application_id + '\n'
            elif command[3:5] == ['install', '--no-streaming']:
                output = 'Performing Push Install\nSuccess\n'
            else:
                raise AssertionError('unexpected ADB argv: ' + repr(command))
            return subprocess.CompletedProcess(command, 0, stdout=output, stderr='')

        parent = '/usr/bin/python3 /Users/yuta/.local/bin/android-common run /usr/bin/python3 -m tools.s1b_campaign_install'
        assessment = {
            'runtime_plan': {
                'profile': 'api35-4k',
                'avd': 'serea_s1_01a3',
                'campaign_ids': 'ISSUE_SEPARATELY_FROM_PRIVATE_HOST_LEDGER_AFTER_PIN',
                'build_only_package_suffix': 's1b0000000000',
                'build_only_apks_are_runtime_targets': False,
                'max_attempts_per_campaign': 1,
                'second_campaign_requires_first_runtime_pass': True,
            },
            'target_apk_sha256': verified.target.sha256,
            'test_apk_sha256': verified.test.sha256,
            'certificate_sha256': verified.certificate_sha256,
        }
        with lock_path.open('r+') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with mock.patch.object(campaign_install, 'ANDROID_ROOT', android_root), \
                 mock.patch.object(campaign_install, '_current_parent_command', return_value=parent), \
                 mock.patch.object(campaign_install, 'require_active_signing_generation',
                                   return_value=(verified.trust_anchor_path, assessment)), \
                 mock.patch.object(campaign_install.shutil, 'disk_usage', return_value=mock.Mock(free=40 * 1024**3)):
                result = campaign_install.install_verified_campaign(
                    verified=verified, profile='api35-4k', ledger_directory=self.ledger,
                    adb=fake_adb, aapt=AAPT, apksigner=APKSIGNER, runner=fake_run,
                )
            fcntl.flock(lock, fcntl.LOCK_UN)

        self.assertEqual(result['status'], 'INSTALLED_NOT_RUN')
        self.assertEqual(result['ledger_state'], 'INSTALL_ATTEMPTED')
        self.assertEqual(json.loads(record_path.read_text())['state'], 'INSTALL_ATTEMPTED')
        installs = [command for command in calls if command[3:5] == ['install', '--no-streaming']]
        self.assertEqual(installs, [
            [str(fake_adb), '-s', serial, 'install', '--no-streaming', str(verified.target.snapshot_path)],
            [str(fake_adb), '-s', serial, 'install', '--no-streaming', str(verified.test.snapshot_path)],
        ])
        self.assertTrue(all('-r' not in command and '-d' not in command for command in installs))

    def test_real_synthetic_apks_verify_hash_signature_identity_and_manifest(self):
        target, test = self.valid_pair()
        source_hashes = (hashlib.sha256(target.read_bytes()).hexdigest(),
                         hashlib.sha256(test.read_bytes()).hexdigest())
        verified = self.verify(target, test)
        self.assertEqual(verified.target.application_id, self.target_package)
        self.assertEqual(verified.test.application_id, self.target_package + ".test")
        self.assertEqual(verified.test.instrumentation_runner, self.runner_name)
        self.assertEqual(verified.test.instrumentation_target, self.target_package)
        self.assertEqual(verified.certificate_sha256, self.test_fingerprint)
        self.assertTrue(verified.target.debuggable and verified.test.debuggable)
        self.assertEqual((verified.target.sha256, verified.test.sha256), source_hashes)
        self.assertEqual(0o400, verified.target.snapshot_path.stat().st_mode & 0o777)
        self.assertEqual(0o700, verified.snapshot_directory.stat().st_mode & 0o777)
        evidence = json.loads(verified.evidence_path.read_text(encoding="utf-8"))
        self.assertNotIn("campaign_token", json.dumps(evidence))
        self.assertTrue(evidence["cryptographic_signature_valid"])
        self.assertTrue(evidence["supplied_trust_anchor_matched"])

    def test_cli_emits_only_nonsecret_verification_summary(self):
        target, test = self.valid_pair()
        result = subprocess.run([
            sys.executable, str(PROJECT / "tools/s1b_campaign_safety.py"), "verify",
            "--campaign-id", self.campaign_id, "--target-apk", str(target),
            "--test-apk", str(test), "--staging-root", str(self.stage),
            "--trust-anchor", str(self.trust_anchor), "--sdk-root", str(SDK),
        ], cwd=PROJECT, capture_output=True, text=True, timeout=120)
        self.assertEqual(0, result.returncode, result.stderr)
        report = json.loads(result.stdout)
        self.assertTrue(report["cryptographic_signature_valid"])
        self.assertTrue(report["supplied_trust_anchor_matched"])
        self.assertNotIn("campaign_token", result.stdout)

    def test_target_and_test_apks_signed_by_different_certificates_are_rejected(self):
        target = self.copy_baseline("target")
        alternative_keystore = self.root / "alternative-synthetic-test-only.p12"
        alternative_password = "alternate-test-only-synthetic-passphrase"
        alternative_alias = "alternative-s1b-synthetic-test-only"
        generated = serea_apk_signer._tool_run([
            KEYTOOL, "-genkeypair", "-noprompt", "-storetype", "PKCS12", "-keystore",
            alternative_keystore, "-storepass", alternative_password, "-keypass", alternative_password,
            "-alias", alternative_alias, "-dname", "CN=Alternate S1B test only", "-validity", "2",
            "-keyalg", "RSA", "-keysize", "2048",
        ])
        self.assertEqual(0, generated.returncode, "temporary alternate fixture key generation failed")
        old_keystore, old_password, old_alias = self.keystore, self.password, self.alias
        try:
            self.keystore, self.password, self.alias = alternative_keystore, alternative_password, alternative_alias
            alternative_test = self.build_signed_apk("test")
        finally:
            self.keystore, self.password, self.alias = old_keystore, old_password, old_alias
        with self.assertRaisesRegex(safety.SafetyError, "certificate|trust anchor"):
            self.verify(target, alternative_test)

    def test_valid_signature_with_wrong_pin_is_rejected_as_untrusted(self):
        target, test = self.valid_pair()
        wrong_anchor = self.write_trust_anchor("0" * 64)
        with self.assertRaisesRegex(safety.SafetyError, "trust anchor|certificate"):
            self.verify(target, test, trust_anchor=wrong_anchor)

    def test_missing_or_unsafe_trust_anchor_fails_closed(self):
        target, test = self.valid_pair()
        with self.assertRaisesRegex(safety.SafetyError, "trust anchor|approved certificate"):
            self.verify(target, test, trust_anchor=None)
        os.chmod(self.trust_anchor, 0o644)
        with self.assertRaises(safety.SafetyError):
            self.verify(target, test)
        os.chmod(self.trust_anchor, 0o600)
        link = self.root / "trust-link.json"
        link.symlink_to(self.trust_anchor)
        with self.assertRaises(safety.SafetyError):
            self.verify(target, test, trust_anchor=link)

    def test_package_inventory_parser_rejects_incomplete_output(self):
        self.assertEqual({"android", "com.android.settings"},
                         safety.parse_package_inventory(b"package:android\npackage:com.android.settings\n"))
        for malformed in (b"", b"package:com.android.settings\nWarning: partial", b"package:../escape\n"):
            with self.subTest(malformed=malformed):
                with self.assertRaises(safety.SafetyError):
                    safety.parse_package_inventory(malformed)

    def test_oversized_package_inventory_is_rejected(self):
        inventory = b"package:android\n" * (safety.MAX_PACKAGE_INVENTORY_BYTES // 16 + 1)
        with self.assertRaisesRegex(safety.SafetyError, "size limit"):
            safety.parse_package_inventory(inventory)

    def test_invalid_or_ambiguous_campaign_identity_is_rejected(self):
        target, test = self.valid_pair()
        for campaign_id in ("s1bbad", "s1b012345678g", "s1b0123456789.test"):
            with self.subTest(campaign_id=campaign_id):
                with self.assertRaises(safety.SafetyError):
                    self.verify(target, test, campaign_id=campaign_id)

    def test_target_application_id_must_match_campaign_id(self):
        target = self.build_signed_apk("target", package="app.serea.android.isolated.s1b0000000000")
        test = self.copy_baseline("test")
        with self.assertRaisesRegex(safety.SafetyError, "target.*package|application ID"):
            self.verify(target, test)

    def test_test_package_and_instrumentation_target_must_match(self):
        target = self.copy_baseline("target")
        wrong_package = self.build_signed_apk("test", package=self.target_package + ".other")
        with self.assertRaisesRegex(safety.SafetyError, "test.*package|application ID"):
            self.verify(target, wrong_package)
        wrong_target = self.build_signed_apk("test", target_package="app.serea.android.other")
        with self.assertRaisesRegex(safety.SafetyError, "targetPackage|instrumentation"):
            self.verify(target, wrong_target)
        wrong_runner = self.build_signed_apk("test", runner="app.serea.android.OtherRunner")
        with self.assertRaisesRegex(safety.SafetyError, "runner|instrumentation"):
            self.verify(target, wrong_runner)

    def test_test_apk_allows_only_the_required_androidx_empty_activity(self):
        target = self.copy_baseline("target")
        empty = self.build_signed_apk(
            "test",
            component=(
                '<activity android:name="androidx.test.core.app.InstrumentationActivityInvoker$EmptyActivity" '
                'android:exported="true" />'
            ),
        )
        verified = self.verify(target, empty)
        self.assertEqual(
            {"activity": 1, "provider": 0, "receiver": 0, "service": 0},
            verified.test.component_counts,
        )
        for forbidden in (
            '<activity android:name="androidx.test.core.app.InstrumentationActivityInvoker$BootstrapActivity" android:exported="true" />',
            '<activity android:name="androidx.test.core.app.InstrumentationActivityInvoker$EmptyFloatingActivity" android:exported="true" />',
            '<activity android:name="injected.ExtraActivity" android:exported="true" />',
            '<activity android:name="androidx.test.core.app.InstrumentationActivityInvoker$EmptyActivity" android:exported="false" />',
            '<provider android:name="injected.Provider" android:authorities="injected.authority" android:exported="true" />',
        ):
            with self.subTest(component=forbidden), self.assertRaisesRegex(safety.SafetyError, "component"):
                rejected = self.build_signed_apk("test", component=forbidden)
                self.verify(target, rejected)

    def test_components_internet_and_non_debuggable_manifests_are_rejected(self):
        unsafe_component = '<receiver android:name="Injected" android:exported="true" />'
        unsafe_target = self.build_signed_apk("target", component=unsafe_component)
        test = self.copy_baseline("test")
        with self.assertRaisesRegex(safety.SafetyError, "component"):
            self.verify(unsafe_target, test)
        exported_main = self.build_signed_apk(
            "target", component='<activity android:name="app.serea.android.MainActivity" android:exported="true" />',
        )
        with self.assertRaisesRegex(safety.SafetyError, "non-exported MainActivity"):
            self.verify(exported_main, test)
        internet_target = self.build_signed_apk("target", internet=True)
        with self.assertRaisesRegex(safety.SafetyError, "INTERNET"):
            self.verify(internet_target, test)
        non_debuggable = self.build_signed_apk("target", debuggable=False)
        with self.assertRaisesRegex(safety.SafetyError, "debuggable|diagnostic"):
            self.verify(non_debuggable, test)
        shared_uid = self.build_signed_apk("target", shared_user_id=True)
        with self.assertRaisesRegex(safety.SafetyError, "sharedUserId|UID separation"):
            self.verify(shared_uid, test)
        wrong_min_sdk = self.build_signed_apk("target", min_sdk=25)
        with self.assertRaisesRegex(safety.SafetyError, "minSdkVersion"):
            self.verify(wrong_min_sdk, test)

    def test_tampered_signature_zip_and_missing_tools_fail_closed(self):
        target, test = self.valid_pair()
        tampered = self.root / "tampered.apk"
        data = bytearray(target.read_bytes())
        with zipfile.ZipFile(target) as archive:
            entry = archive.getinfo("AndroidManifest.xml")
        name_len = int.from_bytes(data[entry.header_offset + 26:entry.header_offset + 28], "little")
        extra_len = int.from_bytes(data[entry.header_offset + 28:entry.header_offset + 30], "little")
        data[entry.header_offset + 30 + name_len + extra_len] ^= 1
        tampered.write_bytes(data)
        with self.assertRaises(safety.SafetyError):
            self.verify(tampered, test)
        with self.assertRaises(safety.SafetyError):
            self.verify(target, test, aapt=self.root / "missing-aapt")
        with self.assertRaises(safety.SafetyError):
            self.verify(target, test, apksigner=self.root / "missing-apksigner")

    def test_manifest_and_zip_path_ambiguities_are_rejected(self):
        target, test = self.valid_pair()
        traversal = self.root / "traversal.apk"
        shutil.copyfile(target, traversal)
        with zipfile.ZipFile(traversal, "a") as archive:
            archive.writestr("../escape", b"outside")
        with self.assertRaisesRegex(safety.SafetyError, "traversal"):
            self.verify(traversal, test)
        duplicate = self.root / "duplicate-manifest.apk"
        shutil.copyfile(target, duplicate)
        with zipfile.ZipFile(duplicate, "a") as archive:
            archive.writestr("AndroidManifest.xml", b"duplicate")
        with self.assertRaisesRegex(safety.SafetyError, "duplicate"):
            self.verify(duplicate, test)

    def test_snapshot_mutated_during_tool_verification_is_rejected(self):
        target, test = self.valid_pair()
        original = safety._run_tool
        changed = []

        def mutate_snapshot(command, *, label):
            result = original(command, label=label)
            if label == "aapt manifest" and not changed:
                snapshot = Path(command[3])
                if snapshot.parent.name == "target":
                    os.chmod(snapshot, 0o600)
                    with snapshot.open("ab") as stream:
                        stream.write(b"changed")
                    os.chmod(snapshot, 0o400)
                    changed.append(True)
            return result

        with mock.patch.object(safety, "_run_tool", side_effect=mutate_snapshot):
            with self.assertRaisesRegex(safety.SafetyError, "changed"):
                self.verify(target, test)
        self.assertTrue(changed)

    def test_symlink_hardlink_and_missing_apk_are_rejected(self):
        target, test = self.valid_pair()
        link = self.root / "target-link.apk"
        link.symlink_to(target)
        with self.assertRaises(safety.SafetyError):
            self.verify(link, test)
        hardlink = self.root / "target-hardlink.apk"
        os.link(target, hardlink)
        with self.assertRaises(safety.SafetyError):
            self.verify(hardlink, test)
        with self.assertRaises(safety.SafetyError):
            self.verify(self.root / "missing.apk", test)

    def test_snapshot_change_after_validation_is_refused_before_ledger_transition(self):
        target, test = self.valid_pair()
        verified = self.verify(target, test)
        record_path = self.write_campaign_record()
        os.chmod(verified.target.snapshot_path, 0o600)
        verified.target.snapshot_path.write_bytes(b"replaced after verification")
        with self.assertRaises(safety.SafetyError):
            with safety.claim_install_attempt(
                ledger_directory=self.ledger, verified=verified,
                installed_packages=self.no_campaign_packages, aapt=AAPT, apksigner=APKSIGNER,
            ):
                self.fail("modified snapshot must never yield install authorization")
        self.assertEqual("PREPARED_ONLY_NOT_BUILT_NOT_INSTALLED",
                         json.loads(record_path.read_text())["state"])

    @staticmethod
    def no_campaign_packages():
        return b"package:android\npackage:com.android.settings\n"

    def test_anchor_generation_change_after_verification_blocks_install_without_consuming_campaign(self):
        target, test = self.valid_pair()
        verified = self.verify(target, test)
        record_path = self.write_campaign_record()
        self.write_trust_anchor("0" * 64)
        with self.assertRaisesRegex(safety.SafetyError, "trust anchor|certificate"):
            with safety.claim_install_attempt(
                ledger_directory=self.ledger, verified=verified,
                installed_packages=self.no_campaign_packages, aapt=AAPT, apksigner=APKSIGNER,
            ):
                self.fail("a different anchor generation must not yield install authorization")
        self.assertEqual("PREPARED_ONLY_NOT_BUILT_NOT_INSTALLED",
                         json.loads(record_path.read_text())["state"])

    def test_device_observation_is_checked_under_lock_before_consuming_campaign(self):
        target, test = self.valid_pair()
        verified = self.verify(target, test)
        record_path = self.write_campaign_record()
        calls = []

        def observation():
            with self.assertRaises(safety.SafetyError):
                with safety.reservation_lock(self.ledger, blocking=False):
                    self.fail("reservation must already be held")
            calls.append("observation")
            return {"serial": "emulator-5554", "avd": "serea_s1_16k", "api": 35,
                    "abi": "x86_64", "page_size": 16384, "host_free_bytes": 40 * 1024**3}

        with self.assertRaises(safety.SafetyError):
            with safety.claim_observed_install_attempt(
                ledger_directory=self.ledger, verified=verified, device_observation=observation,
                installed_packages=self.no_campaign_packages, aapt=AAPT, apksigner=APKSIGNER,
            ):
                self.fail("mismatched AVD must not yield")
        self.assertEqual(["observation"], calls)
        self.assertEqual("PREPARED_ONLY_NOT_BUILT_NOT_INSTALLED", json.loads(record_path.read_text())["state"])

    def test_valid_device_observation_precedes_inventory_and_durable_attempt(self):
        target, test = self.valid_pair()
        verified = self.verify(target, test)
        record_path = self.write_campaign_record()
        calls = []

        def observation():
            calls.append("observation")
            return {"serial": "emulator-5554", "avd": "serea_s1_01a3", "api": 35,
                    "abi": "x86_64", "page_size": 4096, "host_free_bytes": 40 * 1024**3}

        def inventory():
            calls.append("inventory")
            return self.no_campaign_packages()

        with safety.claim_observed_install_attempt(
            ledger_directory=self.ledger, verified=verified, device_observation=observation,
            installed_packages=inventory, aapt=AAPT, apksigner=APKSIGNER,
        ) as permit:
            self.assertEqual(["observation", "inventory"], calls)
            self.assertEqual(verified.target.snapshot_path, permit.target_apk)
            self.assertEqual("INSTALL_ATTEMPTED", json.loads(record_path.read_text())["state"])

    def test_original_apk_change_does_not_change_verified_install_snapshot(self):
        target, test = self.valid_pair()
        verified = self.verify(target, test)
        target.write_bytes(b"different source file after snapshot")
        self.write_campaign_record()
        with safety.claim_install_attempt(
            ledger_directory=self.ledger, verified=verified,
            installed_packages=self.no_campaign_packages, aapt=AAPT, apksigner=APKSIGNER,
        ) as permit:
            self.assertEqual(verified.target.snapshot_path, permit.target_apk)
            self.assertEqual(verified.target.sha256, hashlib.sha256(permit.target_apk.read_bytes()).hexdigest())

    def test_valid_pair_is_revalidated_and_consumed_under_shared_reservation_lock(self):
        target, test = self.valid_pair()
        verified = self.verify(target, test)
        record_path = self.write_campaign_record()
        callback_observed_lock = []

        def inventory():
            callback_observed_lock.append(True)
            return self.no_campaign_packages()

        with safety.claim_install_attempt(
            ledger_directory=self.ledger, verified=verified,
            installed_packages=inventory, aapt=AAPT, apksigner=APKSIGNER,
        ) as permit:
            self.assertEqual(self.target_package, permit.target_package)
            self.assertEqual(self.target_package + ".test", permit.test_package)
            self.assertTrue(callback_observed_lock)
            self.assertEqual("INSTALL_ATTEMPTED", json.loads(record_path.read_text())["state"])
            self.assertNotIn("campaign_token", permit.to_public_dict())
        with self.assertRaisesRegex(safety.SafetyError, "consumed|INSTALL_ATTEMPTED|state"):
            with safety.claim_install_attempt(
                ledger_directory=self.ledger, verified=verified,
                installed_packages=self.no_campaign_packages, aapt=AAPT, apksigner=APKSIGNER,
            ):
                self.fail("attempted campaign must not be reused")

    def test_owner_recovery_method_is_bound_to_one_campaign_and_runtime_pass_is_durable(self):
        target,test=self.valid_pair()
        verified=self.verify(target,test)
        record_path=self.write_campaign_record()
        record=json.loads(record_path.read_text())
        record['owner_approved_scope']=safety.S1B_OWNER_SCOPE
        record['method']='ownership'
        record_path.write_text(json.dumps(record))
        record_path.chmod(0o600)
        with self.assertRaisesRegex(safety.SafetyError,'method does not match'):
            with safety.claim_install_attempt(
                ledger_directory=self.ledger,verified=verified,installed_packages=self.no_campaign_packages,
                aapt=AAPT,apksigner=APKSIGNER,expected_method=safety.S1B_OWNER_METHODS['chat-core']):
                self.fail('wrong method must not claim an issued campaign')
        with safety.claim_install_attempt(
            ledger_directory=self.ledger,verified=verified,installed_packages=self.no_campaign_packages,
            aapt=AAPT,apksigner=APKSIGNER,expected_method=safety.S1B_OWNER_METHODS['ownership']) as permit:
            self.assertEqual('ownership',permit.method)
            self.assertEqual(record['campaign_token'],permit.campaign_token)
            self.assertNotIn('campaign_token',permit.to_public_dict())
        install_record=safety.read_install_attempt(ledger_directory=self.ledger,verified=verified,
                                                   expected_method=safety.S1B_OWNER_METHODS['ownership'])
        self.assertEqual('INSTALL_ATTEMPTED',install_record['state'])
        self.assertNotIn('campaign_token',install_record)
        token=safety.mark_runtime_attempt(ledger_directory=self.ledger,verified=verified,
                                          expected_method=safety.S1B_OWNER_METHODS['ownership'])
        self.assertEqual(record['campaign_token'],token)
        safety.mark_runtime_pass(ledger_directory=self.ledger,verified=verified,
            expected_method=safety.S1B_OWNER_METHODS['ownership'],result_sha256='a'*64)
        updated=json.loads(record_path.read_text())
        self.assertEqual('RUNTIME_PASS',updated['state'])
        self.assertEqual(safety.S1B_OWNER_METHODS['ownership'],updated['runtime_method'])
        with self.assertRaisesRegex(safety.SafetyError,'already been attempted|not bound|state'):
            safety.mark_runtime_attempt(ledger_directory=self.ledger,verified=verified,
                                        expected_method=safety.S1B_OWNER_METHODS['ownership'])

    def test_preinstrumentation_install_failure_is_recorded_as_not_executed(self):
        target,test=self.valid_pair()
        verified=self.verify(target,test)
        record_path=self.write_campaign_record()
        record=json.loads(record_path.read_text())
        record['owner_approved_scope']=safety.S1B_OWNER_SCOPE
        record['method']='ownership'
        record_path.write_text(json.dumps(record))
        record_path.chmod(0o600)
        with safety.claim_install_attempt(ledger_directory=self.ledger,verified=verified,
            installed_packages=self.no_campaign_packages,aapt=AAPT,apksigner=APKSIGNER,
            expected_method=safety.S1B_OWNER_METHODS['ownership']):
            pass
        with mock.patch.object(campaign_install,'_install_verified_campaign',
                               side_effect=safety.SafetyError('synthetic install preparation failure')):
            with self.assertRaisesRegex(safety.SafetyError,'synthetic install preparation failure'):
                campaign_install._install_with_prestart_classification(
                    verified=verified,profile='api35-4k',ledger_directory=self.ledger,
                    adb=SDK/'platform-tools/adb',aapt=AAPT,apksigner=APKSIGNER,
                    method_key='ownership',runner=subprocess.run)
        updated=json.loads(record_path.read_text())
        self.assertEqual('PREPARATION_ABORTED_NOT_EXECUTED',updated['state'])
        self.assertEqual('pre_instrumentation_setup_failed',updated['runtime_not_started_reason'])
        self.assertNotIn('runtime_result_sha256',updated)

    def test_proven_not_started_runtime_is_recorded_without_reusing_campaign_id(self):
        target,test=self.valid_pair()
        verified=self.verify(target,test)
        record_path=self.write_campaign_record()
        record=json.loads(record_path.read_text())
        record['owner_approved_scope']=safety.S1B_OWNER_SCOPE
        record['method']='ownership'
        record_path.write_text(json.dumps(record))
        record_path.chmod(0o600)
        with safety.claim_install_attempt(
            ledger_directory=self.ledger,verified=verified,installed_packages=self.no_campaign_packages,
            aapt=AAPT,apksigner=APKSIGNER,expected_method=safety.S1B_OWNER_METHODS['ownership']):
            pass
        safety.mark_runtime_attempt(ledger_directory=self.ledger,verified=verified,
                                    expected_method=safety.S1B_OWNER_METHODS['ownership'])
        safety.mark_runtime_not_started(ledger_directory=self.ledger,verified=verified,
            expected_method=safety.S1B_OWNER_METHODS['ownership'],reason='adb_process_spawn_failed')
        updated=json.loads(record_path.read_text())
        self.assertEqual('PREPARATION_ABORTED_NOT_EXECUTED',updated['state'])
        self.assertEqual('adb_process_spawn_failed',updated['runtime_not_started_reason'])
        self.assertNotIn('runtime_result_sha256',updated)
        with self.assertRaisesRegex(safety.SafetyError,'not bound|state|attempt'):
            safety.mark_runtime_attempt(ledger_directory=self.ledger,verified=verified,
                                        expected_method=safety.S1B_OWNER_METHODS['ownership'])

    def test_offline_preflight_rejects_system_temp_root_as_ledger_or_snapshot_directory(self):
        for ledger, staging in ((Path(tempfile.gettempdir()), self.stage),
                                (self.ledger, Path(tempfile.gettempdir()))):
            with self.subTest(ledger=ledger, staging=staging):
                with self.assertRaisesRegex(safety.SafetyError, "temp root|temporary"):
                    safety.offline_synthetic_preflight(
                        target_apk=self.root / "not-read.apk", test_apk=self.root / "not-read-test.apk",
                        campaign_id=self.campaign_id, staging_root=staging,
                        trust_anchor=self.trust_anchor, ledger_directory=ledger,
                        installed_packages=b"package:android\\n", aapt=AAPT, apksigner=APKSIGNER,
                    )

    def test_offline_synthetic_preflight_consumes_only_temporary_ledger_without_device_operations(self):
        target, test = self.valid_pair()
        record_path = self.write_campaign_record()
        commands = []
        original_run_tool = safety._run_tool

        def observe_tool(command, *, label):
            commands.append(Path(command[0]).name)
            return original_run_tool(command, label=label)

        with mock.patch.object(safety, "_run_tool", side_effect=observe_tool):
            result = safety.offline_synthetic_preflight(
                target_apk=target, test_apk=test, campaign_id=self.campaign_id,
                staging_root=self.stage, trust_anchor=self.trust_anchor,
                ledger_directory=self.ledger, installed_packages=self.no_campaign_packages(),
                aapt=AAPT, apksigner=APKSIGNER,
            )

        self.assertEqual("OFFLINE_SYNTHETIC_PREFLIGHT_PASS", result["status"])
        self.assertEqual("NOT_RUN", result["device_operation"])
        self.assertFalse(result["runtime_authorized"])
        self.assertEqual("INSTALL_ATTEMPTED", result["ledger_state"])
        self.assertNotIn("campaign_token", json.dumps(result))
        self.assertTrue(commands)
        self.assertTrue(set(commands).issubset({"aapt", "apksigner"}), commands)
        self.assertEqual("INSTALL_ATTEMPTED", json.loads(record_path.read_text())["state"])

    def test_existing_target_or_test_package_blocks_before_state_change(self):
        target, test = self.valid_pair()
        verified = self.verify(target, test)
        record_path = self.write_campaign_record()
        for existing in (self.target_package, self.target_package + ".test"):
            with self.subTest(existing=existing):
                with self.assertRaisesRegex(safety.SafetyError, "already installed|package exists"):
                    with safety.claim_install_attempt(
                        ledger_directory=self.ledger, verified=verified,
                        installed_packages=lambda: f"package:{existing}\n", aapt=AAPT, apksigner=APKSIGNER,
                    ):
                        self.fail("existing package must block")
                self.assertEqual("PREPARED_ONLY_NOT_BUILT_NOT_INSTALLED",
                                 json.loads(record_path.read_text())["state"])

    def test_install_failure_never_rolls_campaign_state_back(self):
        target, test = self.valid_pair()
        verified = self.verify(target, test)
        record_path = self.write_campaign_record()
        with self.assertRaisesRegex(RuntimeError, "simulated install failure"):
            with safety.claim_install_attempt(
                ledger_directory=self.ledger, verified=verified,
                installed_packages=self.no_campaign_packages, aapt=AAPT, apksigner=APKSIGNER,
            ):
                raise RuntimeError("simulated install failure")
        self.assertEqual("INSTALL_ATTEMPTED", json.loads(record_path.read_text())["state"])

    def test_ledger_fsync_failure_does_not_yield_and_does_not_revert_consumed_state(self):
        target, test = self.valid_pair()
        verified = self.verify(target, test)
        record_path = self.write_campaign_record()
        with mock.patch.object(safety, "_fsync_directory", side_effect=OSError("simulated fsync failure")):
            with self.assertRaises(safety.SafetyError):
                with safety.claim_install_attempt(
                    ledger_directory=self.ledger, verified=verified,
                    installed_packages=self.no_campaign_packages, aapt=AAPT, apksigner=APKSIGNER,
                ):
                    self.fail("fsync failure must not yield")
        self.assertEqual("INSTALL_ATTEMPTED", json.loads(record_path.read_text())["state"])

    def test_missing_or_symlinked_reservation_lock_fails_closed(self):
        with safety.reservation_lock(self.ledger):
            pass
        lock = self.ledger / ".reservation.lock"
        lock.unlink()
        with self.assertRaises(safety.SafetyError):
            with safety.reservation_lock(self.ledger):
                self.fail("missing shared lock must not be recreated")
        target = self.root / "other-lock"
        target.touch(mode=0o600)
        lock.symlink_to(target)
        with self.assertRaises(safety.SafetyError):
            with safety.reservation_lock(self.ledger):
                self.fail("symlink lock must be rejected")

    def test_concurrent_reservation_lock_is_nonblocking_and_fail_closed(self):
        self.write_campaign_record()
        child_code = (
            "from pathlib import Path; from tools import s1b_campaign_safety as s; "
            "\ntry:\n with s.reservation_lock(Path(__import__('sys').argv[1]), blocking=False): print('ACQUIRED')"
            "\nexcept s.SafetyError: print('BUSY')\n"
        )
        env = dict(os.environ)
        env["PYTHONPATH"] = str(PROJECT) + os.pathsep + env.get("PYTHONPATH", "")
        with safety.reservation_lock(self.ledger):
            result = subprocess.run([sys.executable, "-c", child_code, str(self.ledger)],
                                    cwd=PROJECT, env=env, capture_output=True, text=True, timeout=20)
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertEqual("BUSY", result.stdout.strip())

    def test_invalid_ledger_and_package_inventory_fail_without_install_permission(self):
        target, test = self.valid_pair()
        verified = self.verify(target, test)
        record_path = self.write_campaign_record()
        record = json.loads(record_path.read_text())
        record["campaign_token"] = "bad"
        record_path.write_text(json.dumps(record))
        os.chmod(record_path, 0o600)
        with self.assertRaises(safety.SafetyError):
            with safety.claim_install_attempt(
                ledger_directory=self.ledger, verified=verified,
                installed_packages=self.no_campaign_packages, aapt=AAPT, apksigner=APKSIGNER,
            ):
                self.fail("malformed ledger must block")
        record["campaign_token"] = "a" * 32
        record_path.write_text(json.dumps(record))
        os.chmod(record_path, 0o600)
        with self.assertRaises(safety.SafetyError):
            with safety.claim_install_attempt(
                ledger_directory=self.ledger, verified=verified,
                installed_packages=lambda: b"", aapt=AAPT, apksigner=APKSIGNER,
            ):
                self.fail("empty package inventory must fail closed")
        with self.assertRaises(safety.SafetyError):
            with safety.claim_install_attempt(
                ledger_directory=self.ledger, verified=verified,
                installed_packages=lambda: (_ for _ in ()).throw(RuntimeError("inventory unavailable")),
                aapt=AAPT, apksigner=APKSIGNER,
            ):
                self.fail("missing inventory must block")
        self.assertEqual("PREPARED_ONLY_NOT_BUILT_NOT_INSTALLED",
                         json.loads(record_path.read_text())["state"])

    def test_shell_metacharacters_are_never_interpreted_as_commands(self):
        target, test = self.valid_pair()
        marker = self.root / "shell-injection-marker"
        malicious = self.root / f"aapt; touch {marker}"
        with self.assertRaises(safety.SafetyError):
            self.verify(target, test, aapt=malicious)
        self.assertFalse(marker.exists())


if __name__ == "__main__":
    unittest.main()
