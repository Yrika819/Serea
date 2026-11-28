from pathlib import Path
from types import SimpleNamespace
import errno
import fcntl
import hashlib
import json
import os
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from tools import s1b_campaign_install as installer
from tools import s1b_campaign_safety as safety
if str(installer.LOCAL_GUARD_ROOT) not in sys.path:
    sys.path.insert(0, str(installer.LOCAL_GUARD_ROOT))
import offline_execution_guard as guard


class CampaignInstallRunnerTest(unittest.TestCase):
    def test_marker_remote_shell_quoting_creates_fresh_directory_and_consumes_once(self):
        package='app.serea.android.isolated.s1b0123456789'
        token='0123456789abcdef0123456789abcdef'
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);bin_dir=root/'bin';bin_dir.mkdir()
            run_as=bin_dir/'run-as'
            run_as.write_text('#!/bin/sh\n[ "$1" = '+package+' ] || exit 81\nshift\nexec "$@"\n',encoding='utf-8')
            run_as.chmod(0o700)
            command=installer.marker_setup_remote_command(package,token)
            env=dict(os.environ);env['PATH']=str(bin_dir)+os.pathsep+env.get('PATH','')
            first=subprocess.run(['/bin/sh','-c',command],cwd=root,env=env,capture_output=True,text=True,check=False)
            marker=root/'no_backup/serea-instrumentation-token'
            self.assertEqual(0,first.returncode,first.stderr)
            self.assertEqual(token,marker.read_text())
            self.assertEqual(0o700,stat.S_IMODE((root/'no_backup').stat().st_mode))
            self.assertEqual(0o600,stat.S_IMODE(marker.stat().st_mode))
            second=subprocess.run(['/bin/sh','-c',command],cwd=root,env=env,capture_output=True,text=True,check=False)
            self.assertNotEqual(0,second.returncode)
            self.assertEqual(token,marker.read_text())
            self.assertNotIn(token,first.stdout+first.stderr+second.stdout+second.stderr)

    def test_failed_adb_stage_is_redacted_and_saved_with_exit_code(self):
        token='0123456789abcdef0123456789abcdef'
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);root.chmod(0o700)
            evidence=installer.AdbEvidence(root,'s1b0123456789',token)
            def failed(argv,**kwargs):
                return SimpleNamespace(returncode=47,stdout='out '+token,stderr='err '+token)
            with self.assertRaisesRegex(safety.SafetyError,'marker-create.*47'):
                installer._run(['adb','shell','opaque-'+token],stage='marker-create',evidence=evidence,
                               campaign_token=token,runner=failed)
            path=next((root/'adb-evidence').glob('*.json'));record=json.loads(path.read_text())
            self.assertEqual('marker-create',record['stage'])
            self.assertEqual(47,record['exit_code'])
            self.assertNotIn(token,path.read_text())
            self.assertEqual(0o600,stat.S_IMODE(path.stat().st_mode))
    def test_selects_only_one_connected_emulator_and_never_a_physical_device(self):
        self.assertEqual(
            installer.parse_connected_emulator(
                'List of devices attached\nemulator-5554 device product:sdk_gphone64_x86_64\n'),
            'emulator-5554',
        )
        for listing in (
            'List of devices attached\n0123456789ABCDEF device usb:1-1\n',
            'List of devices attached\nemulator-5554 device product:sdk\nemulator-5556 device product:sdk\n',
            'List of devices attached\nemulator-5554 offline\n',
            'List of devices attached\n',
        ):
            with self.subTest(listing=listing), self.assertRaises(safety.SafetyError):
                installer.parse_connected_emulator(listing)

    def test_install_requires_independent_active_pin_and_has_no_caller_anchor_option(self):
        with tempfile.TemporaryDirectory() as directory:
            active = Path(directory) / 'owner-approved.json'
            candidate = Path(directory) / 'candidate.json'
            active.write_text(json.dumps({
                'schema': 1,
                'purpose': 'S1B_SYNTHETIC_DIAGNOSTIC',
                'certificate_sha256': 'a' * 64,
            }), encoding='utf-8')
            active.chmod(0o600)
            candidate.write_text('{}', encoding='utf-8')
            candidate.chmod(0o600)
            approved = {
                'signing_review': installer.S1B_APPROVED_REVIEW,
                'candidate_status': installer.S1B_APPROVED_REVIEW,
                'signing_generation': installer.S1B_SIGNING_GENERATION,
                'gradle_sha256': installer.S1B_GRADLE_SHA256,
                'candidate_sha256': '1' * 64,
                'approval_digest': '2' * 64,
                'source_manifest_sha256': '3' * 64,
                'target_apk_sha256': '4' * 64,
                'test_apk_sha256': '5' * 64,
                'target_apk_path': str(guard.S1B_APK_ROOT/'candidate-target-signed.apk'),
                'test_apk_path': str(guard.S1B_APK_ROOT/'candidate-test-signed.apk'),
                'build_command_sha256': '6' * 64,
                'anchor_sha256': hashlib.sha256(active.read_bytes()).hexdigest(),
                'certificate_sha256': 'a' * 64,
                'pin_activated': True,
                'runtime_authorized': False,
                'runtime_plan': guard.S1B_REQUIRED_RUNTIME_PLAN,
            }
            with patch.object(installer, 'APPROVED_TRUST_ANCHOR', active):
                self.assertEqual(installer.checked_active_anchor(approved, active), active.resolve())
                approved_but_unactivated = {**approved, 'pin_activated': False}
                with self.assertRaises(safety.SafetyError):
                    installer.checked_active_anchor(approved_but_unactivated, active)
                with self.assertRaises(safety.SafetyError):
                    installer.checked_active_anchor(approved, candidate)
                with self.assertRaises(safety.SafetyError):
                    installer.checked_active_anchor({'signing_review': 'UNCHANGED_FROM_4C2_BASELINE'}, active)
                stale_digest = {**approved, 'gradle_sha256': '0' * 64}
                with self.assertRaises(safety.SafetyError):
                    installer.checked_active_anchor(stale_digest, active)
                wrong_certificate = {**approved, 'certificate_sha256': 'b' * 64}
                with self.assertRaises(safety.SafetyError):
                    installer.checked_active_anchor(wrong_certificate, active)
        options = {option for action in installer._parser()._actions for option in action.option_strings}
        self.assertNotIn('--trust-anchor', options)

    def test_unapproved_pin_blocks_before_tools_or_apk_snapshotting(self):
        with patch.object(installer, 'require_android_common_lock'), \
             patch.object(installer, 'require_active_signing_generation',
                          side_effect=safety.SafetyError('pin review required')), \
             patch.object(safety, 'resolve_android_tools') as resolve_tools, \
             patch.object(safety, 'verify_campaign_apks') as verify:
            with self.assertRaises(safety.SafetyError):
                installer.run_campaign_install(
                    target_apk=Path('/not-read/target.apk'), test_apk=Path('/not-read/test.apk'),
                    campaign_id='s1b0123456789', profile='api35-4k',
                    staging_root=Path('/not-created'), ledger_directory=Path('/not-created-ledger'),
                )
        resolve_tools.assert_not_called()
        verify.assert_not_called()

    def test_internal_install_entry_enforces_lock_and_active_pin_itself(self):
        verified = SimpleNamespace(trust_anchor_path=Path('/candidate/anchor.json'))
        with patch.object(installer, 'require_android_common_lock'), \
             patch.object(installer, 'require_active_signing_generation',
                          side_effect=safety.SafetyError('active pin review required')), \
             patch.object(safety, '_tool_path') as resolve_adb:
            with self.assertRaises(safety.SafetyError):
                installer._install_verified_campaign(
                    verified=verified, profile='api35-4k', ledger_directory=Path('/ledger'),
                    adb=Path('/adb'), aapt=Path('/aapt'), apksigner=Path('/apksigner'),
                )
        resolve_adb.assert_not_called()

    def test_candidate_apks_require_distinct_owner_issued_runtime_campaign(self):
        plan=dict(guard.S1B_REQUIRED_RUNTIME_PLAN)
        assessment={'runtime_plan':plan,'target_apk_sha256':'a'*64,
                    'test_apk_sha256':'b'*64,'certificate_sha256':'c'*64}
        verified=SimpleNamespace(
            campaign_id=plan['build_only_package_suffix'],
            target=SimpleNamespace(sha256='a'*64),test=SimpleNamespace(sha256='b'*64),
            certificate_sha256='c'*64,
        )
        with self.assertRaisesRegex(safety.SafetyError,'No Owner-approved runtime campaign ID'):
            installer.checked_campaign_review(assessment,verified,'api35-4k')
        verified.campaign_id='s1b0123456789'
        installer.checked_campaign_review(assessment,verified,'api35-4k')
        verified.target=SimpleNamespace(sha256='d'*64)
        verified.test=SimpleNamespace(sha256='e'*64)
        installer.checked_campaign_review(assessment,verified,'api35-4k')
        verified.certificate_sha256='d'*64
        with self.assertRaisesRegex(safety.SafetyError,'signer differs'):
            installer.checked_campaign_review(assessment,verified,'api35-4k')

    def test_cloud_core_campaign_is_bound_to_one_exact_runtime_method(self):
        method='app.serea.android.EmbeddedCoreCloudChatIntegrationTest#fixedCloudProbeCrossesRealCoreBeforeLoopbackRequestAndSettlesUsage'
        self.assertEqual(method,installer.S1B_METHOD_KEYS['cloud-core'])
        plan=dict(guard.S1B_REQUIRED_RUNTIME_PLAN)
        self.assertEqual(method,plan['cloud_core_method'])
        assessment={'runtime_plan':plan,'target_apk_sha256':'a'*64,
                    'test_apk_sha256':'b'*64,'certificate_sha256':'c'*64}
        verified=SimpleNamespace(campaign_id='s1b0123456789',
            target=SimpleNamespace(sha256='a'*64),test=SimpleNamespace(sha256='b'*64),
            certificate_sha256='c'*64)
        installer.checked_campaign_review(assessment,verified,'api35-4k','cloud-core')
        plan['cloud_core_method']='app.serea.android.EmbeddedCoreCloudChatIntegrationTest#otherMethod'
        with self.assertRaisesRegex(safety.SafetyError,'outside the signed Owner-approved campaign scope'):
            installer.checked_campaign_review({**assessment,'runtime_plan':plan},verified,'api35-4k','cloud-core')

    def test_installed_package_path_accepts_only_package_manager_base_apk(self):
        remote='/data/app/~~abc123==/app.serea.android.isolated.s1b0123456789-xyz==/base.apk'
        result=SimpleNamespace(returncode=0,stdout='package:'+remote+'\n',stderr='')
        observed=installer._installed_package_path(Path('/sdk/platform-tools/adb'),'emulator-5554',
            'app.serea.android.isolated.s1b0123456789',lambda *a,**k:result)
        self.assertEqual(remote,observed)
        for invalid in ('/data/local/tmp/base.apk','/data/app/../base.apk','/data/app/a/base.apk\npackage:/data/app/b/base.apk'):
            bad=SimpleNamespace(returncode=0,stdout='package:'+invalid+'\n',stderr='')
            with self.assertRaises(safety.SafetyError):
                installer._installed_package_path(Path('/sdk/platform-tools/adb'),'emulator-5554',
                    'app.serea.android.isolated.s1b0123456789',lambda *a,**k:bad)

    def test_consumed_marker_allows_only_claimed_marker_and_expected_core_directory(self):
        self.assertTrue(installer.marker_consumed(["serea-instrumentation-claimed"]))
        self.assertTrue(installer.marker_consumed(["serea-core", "serea-instrumentation-claimed"]))
        self.assertFalse(installer.marker_consumed(["serea-instrumentation-token"]))
        self.assertFalse(installer.marker_consumed(["serea-core", "other-marker", "serea-instrumentation-claimed"]))
        self.assertFalse(installer.marker_consumed(["serea-core", "serea-instrumentation-token"]))

    def test_instrumentation_result_requires_exact_single_method_pass(self):
        method='app.serea.android.core.EmbeddedCoreAdapterTest#processOwnerSurvivesAdapterReplacement'
        passed=('INSTRUMENTATION_STATUS: class=app.serea.android.core.EmbeddedCoreAdapterTest\n'
                'INSTRUMENTATION_STATUS: current=1\nINSTRUMENTATION_STATUS: numtests=1\n'
                'INSTRUMENTATION_STATUS: test=processOwnerSurvivesAdapterReplacement\n'
                'INSTRUMENTATION_STATUS_CODE: 1\n'
                'INSTRUMENTATION_STATUS: class=app.serea.android.core.EmbeddedCoreAdapterTest\n'
                'INSTRUMENTATION_STATUS: current=1\nINSTRUMENTATION_STATUS: numtests=1\n'
                'INSTRUMENTATION_STATUS: test=processOwnerSurvivesAdapterReplacement\n'
                'INSTRUMENTATION_STATUS_CODE: 0\n'
                'INSTRUMENTATION_RESULT: stream=\nOK (1 test)\nINSTRUMENTATION_CODE: -1\n')
        self.assertTrue(installer.instrumentation_succeeded(passed,method))
        self.assertFalse(installer.instrumentation_succeeded(passed.replace('OK (1 test)','OK (2 tests)'),method))
        self.assertFalse(installer.instrumentation_succeeded(passed.replace('processOwnerSurvivesAdapterReplacement','otherTest'),method))
        self.assertFalse(installer.instrumentation_succeeded(passed.replace('numtests=1','numtests=2'),method))
        self.assertFalse(installer.instrumentation_succeeded(passed.replace('INSTRUMENTATION_CODE: -1','INSTRUMENTATION_CODE: 0'),method))

    def test_install_success_accepts_progress_but_not_failed_or_ambiguous_output(self):
        self.assertTrue(installer.install_succeeded('Performing Push Install\nSuccess\n'))
        self.assertFalse(installer.install_succeeded('Failure [INSTALL_FAILED_UPDATE_INCOMPATIBLE]\n'))
        self.assertFalse(installer.install_succeeded('Success\nFailure [later failure]\n'))
        self.assertFalse(installer.install_succeeded(''))

    def test_instrumentation_timeout_is_recorded_as_outcome_unknown_with_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);root.chmod(0o700)
            (root/'.reservation.lock').touch(mode=0o600)
            os.chmod(root/'.reservation.lock',0o600)
            evidence=installer.AdbEvidence(root,'s1b0123456789','0123456789abcdef0123456789abcdef')
            def uncertain(argv,**kwargs):
                raise subprocess.TimeoutExpired(argv,600,output='partial synthetic output',stderr='transport timeout')
            with self.assertRaisesRegex(safety.SafetyError,'outcome is unknown|outcome-unknown'):
                installer._run_instrumentation_once(
                    ['/adb','shell','am','instrument'],campaign_id='s1b0123456789',
                    campaign_token='0123456789abcdef0123456789abcdef',
                    method='app.serea.android.core.EmbeddedCoreAdapterTest#processOwnerSurvivesAdapterReplacement',
                    ledger_directory=root,evidence=evidence,runner=uncertain)
            stage=json.loads(next((root/'adb-evidence').glob('*.json')).read_text())
            self.assertEqual('instrumentation',stage['stage'])
            self.assertEqual('timeout',stage['error'])
            runtime_file=root/'runtime-evidence'/'s1b0123456789.instrumentation.txt'
            self.assertIn('[timeout]',runtime_file.read_text())
            self.assertIn('partial synthetic output',runtime_file.read_text())

    def test_instrumentation_process_spawn_error_is_proven_not_started(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);root.chmod(0o700)
            evidence=installer.AdbEvidence(root,'s1b0123456789','0123456789abcdef0123456789abcdef')
            def cannot_spawn(argv,**kwargs):
                raise FileNotFoundError(errno.ENOENT,'synthetic missing adb executable','/missing/adb')
            with self.assertRaises(installer.InstrumentationNotStarted):
                installer._run_instrumentation_once(
                    ['/missing/adb','shell','am','instrument'],campaign_id='s1b0123456789',
                    campaign_token='0123456789abcdef0123456789abcdef',
                    method='app.serea.android.core.EmbeddedCoreAdapterTest#processOwnerSurvivesAdapterReplacement',
                    ledger_directory=root,evidence=evidence,runner=cannot_spawn)
            stage=json.loads(next((root/'adb-evidence').glob('*.json')).read_text())
            self.assertEqual('instrumentation',stage['stage'])
            self.assertIsNone(stage['exit_code'])
            self.assertFalse((root/'runtime-evidence').exists())
            self.assertNotIn('0123456789abcdef0123456789abcdef',json.dumps(stage))

    def test_unclassified_os_error_remains_ambiguous_and_consumed(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);root.chmod(0o700)
            evidence=installer.AdbEvidence(root,'s1b0123456789','0123456789abcdef0123456789abcdef')
            def uncertain(argv,**kwargs):
                raise OSError(errno.EIO,'synthetic uncertain instrumentation failure','/adb')
            with self.assertRaisesRegex(safety.SafetyError,'ambiguous'):
                installer._run_instrumentation_once(
                    ['/adb','shell','am','instrument'],campaign_id='s1b0123456789',
                    campaign_token='0123456789abcdef0123456789abcdef',
                    method='app.serea.android.core.EmbeddedCoreAdapterTest#processOwnerSurvivesAdapterReplacement',
                    ledger_directory=root,evidence=evidence,runner=uncertain)
            stage=json.loads(next((root/'adb-evidence').glob('*.json')).read_text())
            self.assertEqual('instrumentation',stage['stage'])
            self.assertFalse((root/'runtime-evidence').exists())

    def test_requires_existing_android_common_lock_for_full_operation(self):
        with tempfile.TemporaryDirectory() as directory:
            lock_path = Path(directory) / 'common.lock'
            lock_path.touch()
            with lock_path.open('r+') as lock:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                wrapper = '/usr/bin/python3 /Users/yuta/.local/bin/android-common run /usr/bin/python3 -m tools.s1b_campaign_install'
                with patch.object(installer, '_current_parent_command', return_value=wrapper, create=True):
                    installer.require_android_common_lock(lock_path)
                with patch.object(installer, '_current_parent_command', return_value='python test_runner.py', create=True):
                    with self.assertRaises(safety.SafetyError):
                        installer.require_android_common_lock(lock_path)
                fcntl.flock(lock, fcntl.LOCK_UN)
            with patch.object(installer, '_current_parent_command', return_value=wrapper, create=True):
                with self.assertRaises(safety.SafetyError):
                    installer.require_android_common_lock(lock_path)

    def test_campaign_observation_is_collected_only_after_emulator_inventory(self):
        calls = []
        def runner(argv, **kwargs):
            calls.append(argv)
            return SimpleNamespace(returncode=0,
                                   stdout='List of devices attached\n0123456789ABCDEF device usb:1-1\n')
        with self.assertRaises(safety.SafetyError):
            installer._device_observation(Path('/sdk/adb'), 'api26', runner)
        self.assertEqual(calls, [['/sdk/adb', 'devices', '-l']])

    def test_observes_exact_allowlisted_profile_before_claim(self):
        responses = {
            ('/sdk/adb', 'devices', '-l'): 'List of devices attached\nemulator-5554 device product:sdk_gphone_x86\n',
            ('/sdk/adb', '-s', 'emulator-5554', 'get-state'): 'device\n',
            ('/sdk/adb', '-s', 'emulator-5554', 'shell', 'getprop', 'sys.boot_completed'): '1\n',
            ('/sdk/adb', '-s', 'emulator-5554', 'emu', 'avd', 'name'): 'serea_s1_api26\nOK\n',
            ('/sdk/adb', '-s', 'emulator-5554', 'shell', 'getprop', 'ro.build.version.sdk'): '26\n',
            ('/sdk/adb', '-s', 'emulator-5554', 'shell', 'getprop', 'ro.product.cpu.abi'): 'x86_64\n',
            ('/sdk/adb', '-s', 'emulator-5554', 'shell', 'getconf', 'PAGE_SIZE'): '4096\n',
        }
        calls = []
        def runner(argv, **kwargs):
            calls.append(tuple(argv))
            return SimpleNamespace(returncode=0, stdout=responses[tuple(argv)])
        with patch.object(installer.shutil, 'disk_usage', return_value=SimpleNamespace(free=40 * 1024**3)):
            result = installer._device_observation(Path('/sdk/adb'), 'api26', runner)
        self.assertEqual(result, {'serial': 'emulator-5554', 'avd': 'serea_s1_api26', 'api': 26,
                                  'abi': 'x86_64', 'page_size': 4096, 'host_free_bytes': 40 * 1024**3})
        self.assertEqual(len(calls), len(responses))

    def test_install_commands_use_verified_snapshots_without_update_or_downgrade(self):
        permit = SimpleNamespace(
            target_apk=Path('/private/campaign/target/base.apk'),
            test_apk=Path('/private/campaign/test/test.apk'),
        )
        commands = installer.install_commands(Path('/sdk/platform-tools/adb'), 'emulator-5554', permit)
        self.assertEqual(commands, [
            ['/sdk/platform-tools/adb', '-s', 'emulator-5554', 'install', '--no-streaming',
             '/private/campaign/target/base.apk'],
            ['/sdk/platform-tools/adb', '-s', 'emulator-5554', 'install', '--no-streaming',
             '/private/campaign/test/test.apk'],
        ])
        self.assertTrue(all('-r' not in command and '-d' not in command for command in commands))


if __name__ == '__main__':
    unittest.main()
