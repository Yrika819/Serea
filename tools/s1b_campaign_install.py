"""Fixed S1B emulator-only install plus exactly one allowlisted instrumentation method."""
from __future__ import annotations

import argparse
import datetime as dt
import errno
import fcntl
import hashlib
import importlib
import json
import os
import re
import shutil
import shlex
import stat
import subprocess
import sys
from pathlib import Path
from typing import Callable, Sequence

from tools import s1b_campaign_safety as safety

ANDROID_ROOT = Path('/Volumes/Tahoe - Data/Users/yuta/DevelopmentStorage/Android')
INTERNAL_VOLUME = Path('/Volumes/Tahoe - Data')
LOCAL_GUARD_ROOT = Path.home() / '.local/share/serea-storage-monitor-v1'
APPROVED_TRUST_ANCHOR = Path.home() / '.local/share/serea-s1b-diagnostic-trust/s1b-diagnostic-trust-anchor.json'
S1B_SIGNING_GENERATION = 'S1B_DIAGNOSTIC_2026_11'
S1B_APPROVED_REVIEW = 'OWNER_APPROVED_S1B_DIAGNOSTIC_2026_11'
S1B_GRADLE_SHA256 = '672516dea970cf93b3dfa3bc78559ea6ef85c3e36a1b6dd941d48d5bb33f30ec'
EMULATOR_SERIAL = re.compile(r'emulator-[0-9]+\Z')


class InstrumentationNotStarted(safety.SafetyError):
    """The local instrumentation process could not be spawned; no test ran."""


def _current_parent_command() -> str:
    try:
        result = subprocess.run(['/bin/ps', '-p', str(os.getppid()), '-o', 'command='],
                                capture_output=True, text=True, timeout=5, check=False)
    except (OSError, subprocess.SubprocessError) as exc:
        raise safety.SafetyError('Could not verify android-common run parent') from exc
    if result.returncode != 0 or not result.stdout.strip():
        raise safety.SafetyError('Could not verify android-common run parent')
    return result.stdout.strip()


def checked_active_anchor(assessment: object, anchor_path: str | Path) -> Path:
    digest_fields=('candidate_sha256','approval_digest','source_manifest_sha256','target_apk_sha256',
                   'test_apk_sha256','build_command_sha256','anchor_sha256','certificate_sha256')
    if not isinstance(assessment, dict) or any((
        assessment.get('signing_review') != S1B_APPROVED_REVIEW,
        assessment.get('candidate_status') != S1B_APPROVED_REVIEW,
        assessment.get('signing_generation') != S1B_SIGNING_GENERATION,
        assessment.get('gradle_sha256') != S1B_GRADLE_SHA256,
        assessment.get('pin_activated') is not True,
        assessment.get('runtime_authorized') is not False,
        any(not isinstance(assessment.get(name),str) or not re.fullmatch(r'[0-9a-f]{64}',assessment[name]) for name in digest_fields),
    )):
        raise safety.SafetyError('Active S1B signer/pin is not Owner-approved for the current source, APK pair, and command')
    try:
        expected = APPROVED_TRUST_ANCHOR.resolve(strict=True)
        supplied = Path(anchor_path).resolve(strict=True)
    except OSError as exc:
        raise safety.SafetyError('Owner-approved S1B trust anchor is unavailable') from exc
    if supplied != expected:
        raise safety.SafetyError('Trust anchor must be the independently reviewed active S1B anchor')
    _, fingerprint = safety._load_trust_anchor(expected)
    if assessment.get('certificate_sha256') != fingerprint or assessment.get('anchor_sha256') != hashlib.sha256(expected.read_bytes()).hexdigest():
        raise safety.SafetyError('Active S1B review certificate or anchor digest does not match the approved trust anchor')
    import offline_execution_guard as guard
    if assessment.get('runtime_plan') != guard.S1B_REQUIRED_RUNTIME_PLAN:
        raise safety.SafetyError('Active S1B review runtime conditions changed')
    if (assessment.get('target_apk_path') != str(guard.S1B_APK_ROOT/'candidate-target-signed.apk')
            or assessment.get('test_apk_path') != str(guard.S1B_APK_ROOT/'candidate-test-signed.apk')):
        raise safety.SafetyError('Active S1B review APK preservation paths changed')
    return expected


S1B_METHOD_KEYS = {
    'ownership': 'app.serea.android.core.EmbeddedCoreAdapterTest#processOwnerSurvivesAdapterReplacement',
    'ownership-recovery': 'app.serea.android.core.EmbeddedCoreAdapterTest#processOwnerSurvivesAdapterReplacement',
    'ownership-recovery-r2': 'app.serea.android.core.EmbeddedCoreAdapterTest#processOwnerSurvivesAdapterReplacement',
    'chat-core': 'app.serea.android.EmbeddedCoreDiagnosticChatUiTest#diagnosticChatUsesCoreMockAcrossRecreationAndCleansUp',
    'chat-core-recovery': 'app.serea.android.EmbeddedCoreDiagnosticChatUiTest#diagnosticChatUsesCoreMockAcrossRecreationAndCleansUp',
}


class AdbEvidence:
    """Write redacted per-command ADB evidence in an Owner-private campaign directory."""
    def __init__(self,campaign_directory: str | Path,campaign_id: str,token: str):
        if (not re.fullmatch(r's1b[a-f0-9]{10}',campaign_id)
                or not isinstance(token,str) or not re.fullmatch(r'[0-9a-f]{32}',token)):
            raise safety.SafetyError('ADB evidence identity is malformed')
        root=Path(campaign_directory)
        info=root.stat(follow_symlinks=False)
        if not stat.S_ISDIR(info.st_mode) or info.st_uid!=os.getuid() or stat.S_IMODE(info.st_mode)!=0o700:
            raise safety.SafetyError('ADB evidence parent must be Owner-owned mode 0700')
        self.token=token
        self.campaign_id=campaign_id
        self.directory=root/'adb-evidence'
        try:self.directory.mkdir(mode=0o700)
        except FileExistsError:
            info=self.directory.stat(follow_symlinks=False)
            if not stat.S_ISDIR(info.st_mode) or info.st_uid!=os.getuid() or stat.S_IMODE(info.st_mode)!=0o700:
                raise safety.SafetyError('Existing ADB evidence directory is unsafe')
        self.counter=0

    def record(self,stage,argv,exit_code,stdout='',stderr='',error=None):
        if not re.fullmatch(r'[a-z0-9-]{1,48}',stage):raise safety.SafetyError('ADB evidence stage is malformed')
        def scrub(value):
            if isinstance(value,bytes):value=value.decode('utf-8','replace')
            return str(value or '').replace(self.token,'[REDACTED]')
        self.counter+=1
        row={'schema':1,'campaign_id':self.campaign_id,'stage':stage,'recorded_utc':dt.datetime.now(dt.timezone.utc).isoformat(),
             'exit_code':exit_code,'argv':[scrub(x) for x in argv],
             'stdout':scrub(stdout),'stderr':scrub(stderr),'error':scrub(error)}
        path=self.directory/(f'{self.counter:03d}-{stage}.json')
        payload=(json.dumps(row,sort_keys=True,ensure_ascii=False,separators=(',',':'))+'\n').encode()
        fd=os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL|getattr(os,'O_NOFOLLOW',0),0o600)
        with os.fdopen(fd,'wb') as stream:
            stream.write(payload);stream.flush();os.fsync(stream.fileno())
        dirfd=os.open(self.directory,os.O_RDONLY|os.O_DIRECTORY|getattr(os,'O_NOFOLLOW',0))
        try:os.fsync(dirfd)
        finally:os.close(dirfd)
        return path


def remote_shell_run_as(package: str,script: str) -> str:
    if not re.fullmatch(r'app\.serea\.android\.isolated\.s1b[a-f0-9]{10}',package):
        raise safety.SafetyError('run-as package is outside the unique S1B namespace')
    if not isinstance(script,str) or not script or '\x00' in script:
        raise safety.SafetyError('run-as shell script is malformed')
    return f'run-as {shlex.quote(package)} sh -c {shlex.quote(script)}'


def marker_check_remote_command(package: str) -> str:
    return remote_shell_run_as(package,
        'test ! -e no_backup/serea-instrumentation-token && test ! -e no_backup/serea-instrumentation-claimed')


def marker_setup_remote_command(package: str,token: str) -> str:
    if not isinstance(token,str) or not re.fullmatch(r'[0-9a-f]{32}',token):
        raise safety.SafetyError('Campaign token is malformed')
    script=('umask 077 && mkdir -p no_backup && '
            'test ! -e no_backup/serea-instrumentation-token && '
            'test ! -e no_backup/serea-instrumentation-claimed && '
            f'printf %s {shlex.quote(token)} > no_backup/serea-instrumentation-token')
    return remote_shell_run_as(package,script)


def marker_list_remote_command(package: str) -> str:
    return remote_shell_run_as(package,'ls -1 no_backup')


def checked_campaign_review(assessment: object, verified: safety.VerifiedCampaign, profile: str,
                            method_key: str | None = None) -> None:
    """Bind a freshly built campaign pair to the active signer generation, not build-only APKs."""
    if not isinstance(assessment,dict):
        raise safety.SafetyError('Active S1B review assessment is missing')
    plan=assessment.get('runtime_plan')
    if (not isinstance(plan,dict) or profile!=plan.get('profile')
            or plan.get('avd')!='serea_s1_01a3' or profile!='api35-4k'):
        raise safety.SafetyError('Runtime profile/AVD differs from the Owner-reviewed S1B condition')
    if verified.certificate_sha256!=assessment.get('certificate_sha256'):
        raise safety.SafetyError('Runtime APK signer differs from the Owner-approved signing generation')
    if method_key is not None:
        expected_method=S1B_METHOD_KEYS.get(method_key)
        plan_key=('recovery_method' if method_key in ('ownership-recovery','ownership-recovery-r2')
                  else ('first_method' if method_key=='ownership' else 'second_method'))
        if expected_method is None or plan.get(plan_key)!=expected_method:
            raise safety.SafetyError('Selected runtime method is outside the signed Owner-approved campaign scope')
    if (not re.fullmatch(r's1b[a-f0-9]{10}',verified.campaign_id)
            or verified.campaign_id==plan.get('build_only_package_suffix')):
        raise safety.SafetyError('No Owner-approved runtime campaign ID carries a unique issued campaign identity')


def require_active_signing_generation() -> tuple[Path, dict]:
    """Use the existing local signer review; never trust an anchor passed by the caller."""
    if not LOCAL_GUARD_ROOT.is_dir():
        raise safety.SafetyError('Existing Owner signer review is unavailable; installation is refused')
    guard_path = str(LOCAL_GUARD_ROOT)
    inserted = guard_path not in sys.path
    if inserted:
        sys.path.insert(0, guard_path)
    try:
        guard = importlib.import_module('offline_execution_guard')
        assessment = guard.production_review()
        return checked_active_anchor(assessment, guard.TRUST), assessment
    except safety.SafetyError:
        raise
    except Exception as exc:
        raise safety.SafetyError('Existing Owner signer review is missing, changed, or unapproved') from exc
    finally:
        if inserted:
            try:
                sys.path.remove(guard_path)
            except ValueError:
                pass


def require_android_common_lock(lock_path: str | Path | None = None) -> None:
    """Require this process to be a direct android-common child and its shared lock held."""
    try:
        parent = shlex.split(_current_parent_command())
    except ValueError as exc:
        raise safety.SafetyError('android-common run parent command is malformed') from exc
    wrapper = str(Path.home() / '.local/bin/android-common')
    if not any(parent[index:index + 2] == [wrapper, 'run'] for index in range(max(0, len(parent) - 1))):
        raise safety.SafetyError('campaign install must be a direct android-common run child')
    path = Path(lock_path) if lock_path is not None else ANDROID_ROOT / 'locks/common.lock'
    try:
        fd = os.open(path, os.O_RDWR | getattr(os, 'O_NOFOLLOW', 0))
    except OSError as exc:
        raise safety.SafetyError('android-common shared lock is unavailable') from exc
    try:
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return
        fcntl.flock(fd, fcntl.LOCK_UN)
        raise safety.SafetyError('campaign install must run under android-common run')
    finally:
        os.close(fd)


def parse_connected_emulator(output: str | bytes) -> str:
    if isinstance(output, bytes):
        try:
            output = output.decode('utf-8', 'strict')
        except UnicodeError as exc:
            raise safety.SafetyError('ADB device inventory is malformed') from exc
    if not isinstance(output, str):
        raise safety.SafetyError('ADB device inventory is malformed')
    rows = []
    for line in output.splitlines():
        line = line.strip()
        if not line or line == 'List of devices attached':
            continue
        fields = line.split()
        if len(fields) < 2:
            raise safety.SafetyError('ADB device inventory is malformed')
        rows.append(fields)
    if (len(rows) != 1 or len(rows[0]) < 2 or rows[0][1] != 'device'
            or not EMULATOR_SERIAL.fullmatch(rows[0][0])):
        raise safety.SafetyError('Exactly one connected emulator is required; physical or extra devices are refused')
    return rows[0][0]


def install_succeeded(output: str) -> bool:
    lines = [line.strip() for line in output.splitlines() if line.strip()]
    return bool(lines) and lines[-1] == 'Success' and not any(line.startswith('Failure') for line in lines)


def install_commands(adb: str | Path, serial: str, permit: safety.InstallAttemptPermit) -> list[list[str]]:
    if not EMULATOR_SERIAL.fullmatch(serial):
        raise safety.SafetyError('Install target is not an emulator serial')
    return [
        [str(adb), '-s', serial, 'install', '--no-streaming', str(permit.target_apk)],
        [str(adb), '-s', serial, 'install', '--no-streaming', str(permit.test_apk)],
    ]


def _run(argv: Sequence[str | Path], *, timeout: int = 30, runner: Callable = subprocess.run,
         stage: str = 'adb', evidence: AdbEvidence | None = None,
         campaign_token: str | None = None) -> str:
    args=[str(part) for part in argv]
    try:
        result=runner(args,capture_output=True,text=True,timeout=timeout,check=False)
    except subprocess.TimeoutExpired as exc:
        if evidence is not None:evidence.record(stage,args,None,exc.stdout,exc.stderr,str(exc))
        raise safety.SafetyError(f'ADB stage {stage} timed out; campaign attempt remains consumed') from exc
    except (OSError,subprocess.SubprocessError) as exc:
        if evidence is not None:evidence.record(stage,args,None,'','',str(exc))
        raise safety.SafetyError(f'ADB stage {stage} could not be executed; campaign attempt remains consumed') from exc
    raw_stdout=getattr(result,'stdout','');raw_stderr=getattr(result,'stderr','')
    stdout=raw_stdout.decode('utf-8','replace') if isinstance(raw_stdout,bytes) else (raw_stdout or '')
    stderr=raw_stderr.decode('utf-8','replace') if isinstance(raw_stderr,bytes) else (raw_stderr or '')
    if evidence is not None:evidence.record(stage,args,result.returncode,stdout,stderr,None)
    if result.returncode != 0:
        raise safety.SafetyError(f'ADB stage {stage} failed with exit code {result.returncode}; campaign attempt remains consumed')
    return stdout


def _device_observation(adb: Path, profile: str, runner: Callable,
                        evidence: AdbEvidence | None = None) -> dict:
    serial = parse_connected_emulator(_run([adb, 'devices', '-l'], runner=runner,
                                           stage='device-inventory',evidence=evidence))

    def query(*args: str) -> str:
        label=re.sub(r'[^a-z0-9-]+','-', '-'.join(args).lower()).strip('-')
        return _run([adb, '-s', serial, *args], runner=runner,
                    stage='device-'+label,evidence=evidence).strip()

    if query('get-state') != 'device' or query('shell', 'getprop', 'sys.boot_completed') != '1':
        raise safety.SafetyError('Allowlisted emulator is not fully booted')
    avd_lines = [line.strip() for line in query('emu', 'avd', 'name').splitlines()
                 if line.strip() and line.strip() != 'OK']
    if len(avd_lines) != 1:
        raise safety.SafetyError('Emulator AVD identity is malformed')
    api_text = query('shell', 'getprop', 'ro.build.version.sdk')
    abi = query('shell', 'getprop', 'ro.product.cpu.abi')
    page_text = query('shell', 'getconf', 'PAGE_SIZE')
    if not api_text.isdecimal() or not page_text.isdecimal():
        raise safety.SafetyError('Emulator API or page-size observation is malformed')
    observation = {
        'serial': serial, 'avd': avd_lines[0], 'api': int(api_text), 'abi': abi,
        'page_size': int(page_text), 'host_free_bytes': shutil.disk_usage(INTERNAL_VOLUME).free,
    }
    safety.validate_device_observation(profile, observation)
    return observation


def _same_device(left: dict, right: dict) -> bool:
    keys = ('serial', 'avd', 'api', 'abi', 'page_size')
    return all(left.get(key) == right.get(key) for key in keys)


def _run_instrumentation_once(command: Sequence[str | Path], *, campaign_id: str,
                              campaign_token: str, method: str, ledger_directory: str | Path,
                              evidence: AdbEvidence, runner: Callable) -> tuple[str,Path,str]:
    try:
        result=runner([str(part) for part in command],capture_output=True,text=True,timeout=600,check=False)
    except subprocess.TimeoutExpired as exc:
        stdout=exc.stdout.decode('utf-8','replace') if isinstance(exc.stdout,bytes) else (exc.stdout or '')
        stderr=exc.stderr.decode('utf-8','replace') if isinstance(exc.stderr,bytes) else (exc.stderr or '')
        evidence.record('instrumentation',command,None,stdout,stderr,'timeout')
        safety.save_runtime_evidence(ledger_directory=ledger_directory,campaign_id=campaign_id,
                                     output=stdout+'\n[timeout]\n'+stderr,campaign_token=campaign_token)
        raise safety.SafetyError(
            'Instrumentation timed out; outcome is unknown and the one-shot attempt remains consumed'
        ) from exc
    except OSError as exc:
        evidence.record('instrumentation',command,None,'','',str(exc))
        spawn_errnos={errno.E2BIG,errno.EACCES,errno.ELOOP,errno.ENOENT,errno.ENOEXEC,errno.ENOTDIR}
        if exc.filename is not None and exc.errno in spawn_errnos:
            raise InstrumentationNotStarted(
                'Local ADB executable could not be spawned; recorded as not executed'
            ) from exc
        raise safety.SafetyError(
            'Instrumentation start/result is ambiguous after an operating-system error; campaign remains consumed'
        ) from exc
    except subprocess.SubprocessError as exc:
        evidence.record('instrumentation',command,None,'','',str(exc))
        raise safety.SafetyError(
            'Instrumentation process start/result is ambiguous; campaign remains consumed'
        ) from exc
    stdout=result.stdout.decode('utf-8','replace') if isinstance(result.stdout,bytes) else (result.stdout or '')
    stderr=result.stderr.decode('utf-8','replace') if isinstance(result.stderr,bytes) else (result.stderr or '')
    evidence.record('instrumentation',command,result.returncode,stdout,stderr,None)
    evidence_output=stdout if not stderr else stdout+'\n[stderr]\n'+stderr
    evidence_path,digest=safety.save_runtime_evidence(ledger_directory=ledger_directory,campaign_id=campaign_id,
                                                       output=evidence_output,campaign_token=campaign_token)
    if result.returncode!=0 or not instrumentation_succeeded(stdout,method):
        raise safety.SafetyError('Specified instrumentation method did not produce one unambiguous PASS; campaign remains consumed')
    return stdout,evidence_path,digest


def _installed_package_path(adb_path: Path, serial: str, package: str, runner: Callable,
                            evidence: AdbEvidence | None = None) -> str:
    output=_run([adb_path,'-s',serial,'shell','pm','path',package],runner=runner,
                stage='installed-apk-path',evidence=evidence)
    lines=[line.strip() for line in output.splitlines() if line.strip()]
    if len(lines)!=1 or not lines[0].startswith('package:'):
        raise safety.SafetyError('Installed campaign package path is missing or ambiguous')
    remote=lines[0][len('package:'):]
    if not re.fullmatch(r'/data/app/[A-Za-z0-9_./~=+-]+/base\.apk',remote) or '..' in remote.split('/'):
        raise safety.SafetyError('Installed campaign APK path is outside the package-manager data namespace')
    return remote


def _verify_installed_apk_pair(*, adb_path: Path, serial: str, permit: object,
                               verified: safety.VerifiedCampaign, campaign_directory: Path,
                               evidence: AdbEvidence, runner: Callable) -> None:
    check_dir=campaign_directory/'installed-apk-check'
    try:check_dir.mkdir(mode=0o700)
    except FileExistsError:
        info=check_dir.stat(follow_symlinks=False)
        if not stat.S_ISDIR(info.st_mode) or info.st_uid!=os.getuid() or stat.S_IMODE(info.st_mode)!=0o700:
            raise safety.SafetyError('Existing installed-APK evidence directory is unsafe')
        if any(entry.name not in ('target.apk','test.apk') for entry in check_dir.iterdir()):
            raise safety.SafetyError('Installed-APK evidence directory contains unapproved files')
    for role,package,expected in (('target',permit.target_package,permit.target_sha256),
                                  ('test',permit.test_package,permit.test_sha256)):
        local=check_dir/(role+'.apk')
        if not local.exists():
            remote=_installed_package_path(adb_path,serial,package,runner,evidence)
            _run([adb_path,'-s',serial,'pull',remote,str(local)],timeout=90,runner=runner,
                 stage=f'pull-{role}-apk',evidence=evidence)
        try:
            info=local.stat(follow_symlinks=False)
            if not stat.S_ISREG(info.st_mode) or info.st_uid!=os.getuid() or info.st_nlink!=1:
                raise safety.SafetyError('Pulled campaign APK evidence is unsafe')
            os.chmod(local,0o600,follow_symlinks=False)
            digest=hashlib.sha256(local.read_bytes()).hexdigest()
        except OSError as exc:raise safety.SafetyError('Installed campaign APK bytes could not be verified') from exc
        if digest!=expected:raise safety.SafetyError('Installed APK digest differs from the ledger-bound campaign build')
    if verified.target.sha256!=permit.target_sha256 or verified.test.sha256!=permit.test_sha256:
        raise safety.SafetyError('Installed APK verification permit changed unexpectedly')


def _execute_runtime(*, verified: safety.VerifiedCampaign, permit: object, method_key: str,
                     profile: str, ledger_directory: str | Path, adb_path: Path,
                     final: dict, observe: Callable[[],dict], campaign_directory: Path,
                     evidence: AdbEvidence, runner: Callable) -> dict:
    if method_key not in S1B_METHOD_KEYS or permit.method!=method_key:
        raise safety.SafetyError('Campaign ledger method differs from the requested one-time runtime method')
    _verify_installed_apk_pair(adb_path=adb_path,serial=final['serial'],permit=permit,
                               verified=verified,campaign_directory=campaign_directory,
                               evidence=evidence,runner=runner)
    token=evidence.token
    if not re.fullmatch(r'[0-9a-f]{32}',token):
        raise safety.SafetyError('Campaign token changed before runtime preparation')
    method=S1B_METHOD_KEYS[method_key]
    try:
        current=observe()
        if not _same_device(final,current):
            raise safety.SafetyError('Emulator identity changed before instrumentation preparation')
        marker_check=_run([adb_path,'-s',final['serial'],'shell',marker_check_remote_command(permit.target_package)],
                          runner=runner,stage='marker-precheck',evidence=evidence,campaign_token=token)
        if marker_check.strip():
            raise safety.SafetyError('Fresh instrumentation marker precondition returned unexpected output')
        _run([adb_path,'-s',final['serial'],'shell',marker_setup_remote_command(permit.target_package,token)],
             runner=runner,stage='marker-create',evidence=evidence,campaign_token=token)
        marker_names=_run([adb_path,'-s',final['serial'],'shell',marker_list_remote_command(permit.target_package)],
                          runner=runner,stage='marker-verify',evidence=evidence,campaign_token=token).splitlines()
        if marker_names!=['serea-instrumentation-token']:
            raise safety.SafetyError('Fresh target namespace contains unexpected no-backup files')
        runner_name=verified.test.instrumentation_runner
        if not runner_name or not re.fullmatch(r'[A-Za-z_$][A-Za-z0-9_.$]*',runner_name):
            raise safety.SafetyError('Verified test APK has no safe instrumentation runner identity')
        command=[adb_path,'-s',final['serial'],'shell','am','instrument','-w','-r',
                 '-e','class',method,'-e','sereaS1bCampaign',permit.campaign_id,
                 '-e','sereaCampaignToken',token,f'{permit.test_package}/{runner_name}']
    except safety.SafetyError:
        safety.mark_runtime_not_started(ledger_directory=ledger_directory,verified=verified,
            expected_method=S1B_METHOD_KEYS[method_key],reason='pre_instrumentation_setup_failed')
        raise
    token=safety.mark_runtime_attempt(ledger_directory=ledger_directory,verified=verified,
                                      expected_method=S1B_METHOD_KEYS[method_key])
    if token!=evidence.token:
        raise safety.SafetyError('Campaign token changed between evidence setup and one-shot consumption')
    try:
        output,evidence_path,result_digest=_run_instrumentation_once(
            command,campaign_id=permit.campaign_id,campaign_token=token,method=method,
            ledger_directory=ledger_directory,evidence=evidence,runner=runner,
        )
    except InstrumentationNotStarted:
        safety.mark_runtime_not_started(ledger_directory=ledger_directory,verified=verified,
            expected_method=S1B_METHOD_KEYS[method_key],reason='adb_process_spawn_failed')
        raise
    current=observe()
    if not _same_device(final,current):
        raise safety.SafetyError('Emulator identity changed during instrumentation; campaign result is not accepted')
    claimed=_run([adb_path,'-s',final['serial'],'shell',marker_list_remote_command(permit.target_package)],
                 runner=runner,stage='marker-consumed-verify',evidence=evidence,campaign_token=token).splitlines()
    if not marker_consumed(claimed):
        raise safety.SafetyError('Instrumentation did not atomically claim the one-shot campaign marker')
    for package in (permit.test_package,permit.target_package):
        removed=_run([adb_path,'-s',final['serial'],'uninstall',package],timeout=90,runner=runner,
                     stage='cleanup-package',evidence=evidence,campaign_token=token)
        if removed.strip()!='Success':
            raise safety.SafetyError('Campaign-owned package cleanup did not return exact Success; no other package was touched')
    remaining=safety.parse_package_inventory(
        _run([adb_path,'-s',final['serial'],'shell','pm','list','packages'],runner=runner,
             stage='package-inventory-after-cleanup',evidence=evidence,campaign_token=token))
    if permit.target_package in remaining or permit.test_package in remaining:
        raise safety.SafetyError('Campaign-owned package cleanup was not confirmed')
    safety.mark_runtime_pass(ledger_directory=ledger_directory,verified=verified,
                             expected_method=method,result_sha256=result_digest,cleanup_status='CLEANUP_PASS')
    return {
        'schema':1,'status':'RUNTIME_PASS','campaign_id':permit.campaign_id,
        'serial':final['serial'],'avd':final['avd'],'profile':profile,
        'method':method,'target_package':permit.target_package,'test_package':permit.test_package,
        'target_apk_sha256':permit.target_sha256,'test_apk_sha256':permit.test_sha256,
        'certificate_sha256':verified.certificate_sha256,'runtime_result_sha256':result_digest,
        'runtime_evidence_path':str(evidence_path),'adb_evidence_directory':str(evidence.directory),
        'installed_target_digest_verified':True,
        'installed_test_digest_verified':True,'cleanup':'CLEANUP_PASS',
        'ledger_state':'RUNTIME_PASS','runtime':'PASS',
    }


def _install_verified_campaign(*, verified: safety.VerifiedCampaign, profile: str,
                               ledger_directory: str | Path, adb: str | Path,
                               aapt: str | Path, apksigner: str | Path,
                               method_key: str | None = None,
                               runner: Callable = subprocess.run) -> dict:
    """Install only a fresh pair, release the reservation lock, then consume one runtime attempt."""
    require_android_common_lock()
    active_anchor,assessment=require_active_signing_generation()
    checked_campaign_review(assessment,verified,profile,method_key)
    try:
        if Path(verified.trust_anchor_path).resolve(strict=True)!=active_anchor.resolve(strict=True):
            raise safety.SafetyError('Verified APK signer anchor is not the active Owner-approved generation')
    except OSError as exc:raise safety.SafetyError('Active S1B trust anchor disappeared before installation') from exc
    adb_path=safety._tool_path(adb,'adb')
    campaign_directory=Path(verified.snapshot_directory).parent
    evidence=None
    if method_key is not None:
        expected_method=S1B_METHOD_KEYS.get(method_key)
        if not expected_method:raise safety.SafetyError('Runtime method key is not Owner-approved')
        evidence_token=safety.campaign_token_for_execution(ledger_directory=ledger_directory,
            verified=verified,expected_method=expected_method)
        evidence=AdbEvidence(campaign_directory,verified.campaign_id,evidence_token)
    initial={}
    def observe():
        device=_device_observation(adb_path,profile,runner,evidence)
        if initial and not _same_device(initial,device):raise safety.SafetyError('Emulator identity changed during campaign install')
        if not initial:initial.update(device)
        return device
    def inventory():
        device=observe()
        return _run([adb_path,'-s',device['serial'],'shell','pm','list','packages'],runner=runner,
                    stage='package-inventory',evidence=evidence)
    if method_key is None:
        with safety.claim_observed_install_attempt(ledger_directory=ledger_directory,verified=verified,
                device_observation=observe,installed_packages=inventory,aapt=aapt,apksigner=apksigner) as permit:
            for install_index,command in enumerate(install_commands(adb_path,initial['serial'],permit)):
                if not _same_device(initial,observe()):raise safety.SafetyError('Emulator identity changed after INSTALL_ATTEMPTED')
                stage='install-target' if install_index==0 else 'install-test'
                if not install_succeeded(_run(command,timeout=300,runner=runner,stage=stage,evidence=evidence)):
                    raise safety.SafetyError('ADB install did not return Success; campaign remains consumed')
            final=observe()
            installed=safety.parse_package_inventory(_run([adb_path,'-s',final['serial'],'shell','pm','list','packages'],runner=runner,stage='package-inventory-after-install',evidence=evidence))
            if permit.target_package not in installed or permit.test_package not in installed:
                raise safety.SafetyError('Installed package inventory does not contain the exact campaign pair')
            return {'schema':1,'status':'INSTALLED_NOT_RUN','campaign_id':permit.campaign_id,
                    'serial':final['serial'],'avd':final['avd'],'profile':profile,
                    'target_package':permit.target_package,'test_package':permit.test_package,
                    'ledger_state':'INSTALL_ATTEMPTED','runtime':'NOT_RUN'}
    expected_method=S1B_METHOD_KEYS.get(method_key)
    if not expected_method:raise safety.SafetyError('Runtime method key is not Owner-approved')
    try:
        observed_state=safety.read_install_attempt(ledger_directory=ledger_directory,verified=verified,
                                                   expected_method=expected_method)
    except safety.SafetyError as state_error:
        if 'Existing install attempt' not in str(state_error):raise
        with safety.claim_observed_install_attempt(ledger_directory=ledger_directory,verified=verified,
                device_observation=observe,installed_packages=inventory,aapt=aapt,apksigner=apksigner,
                expected_method=expected_method) as permit:
            for install_index,command in enumerate(install_commands(adb_path,initial['serial'],permit)):
                if not _same_device(initial,observe()):raise safety.SafetyError('Emulator identity changed after INSTALL_ATTEMPTED')
                stage='install-target' if install_index==0 else 'install-test'
                if not install_succeeded(_run(command,timeout=300,runner=runner,stage=stage,evidence=evidence)):
                    raise safety.SafetyError('ADB install did not return Success; campaign remains consumed')
            final=observe()
            installed=safety.parse_package_inventory(_run([adb_path,'-s',final['serial'],'shell','pm','list','packages'],runner=runner,stage='package-inventory-after-install',evidence=evidence))
            if permit.target_package not in installed or permit.test_package not in installed:
                raise safety.SafetyError('Installed package inventory does not contain the exact campaign pair')
    else:
        final=observe()
        if (final['avd']!='serea_s1_01a3' or observed_state['profile']!=profile
                or observed_state['target_package']!=verified.target.application_id
                or observed_state['test_package']!=verified.test.application_id):
            raise safety.SafetyError('Consumed install record/device does not match this fresh campaign pair')
        installed=safety.parse_package_inventory(_run([adb_path,'-s',final['serial'],'shell','pm','list','packages'],runner=runner,stage='package-inventory-resume',evidence=evidence))
        if verified.target.application_id not in installed or verified.test.application_id not in installed:
            raise safety.SafetyError('Previously attempted campaign packages are not both installed; refusing reinstall')
        permit=type('CampaignRuntimePermit',(),{})()
        permit.campaign_id=verified.campaign_id;permit.target_package=verified.target.application_id
        permit.test_package=verified.test.application_id;permit.target_sha256=verified.target.sha256
        permit.test_sha256=verified.test.sha256;permit.method=method_key
    final=observe()
    if permit.method!=method_key:
        raise safety.SafetyError('Campaign install method does not match the authorized runtime method')
    return _execute_runtime(verified=verified,permit=permit,method_key=method_key,profile=profile,
        ledger_directory=ledger_directory,adb_path=adb_path,final=final,observe=observe,
        campaign_directory=campaign_directory,evidence=evidence,runner=runner)


def marker_consumed(marker_names: list[str]) -> bool:
    return marker_names in (['serea-instrumentation-claimed'],
                            ['serea-core','serea-instrumentation-claimed'])


def instrumentation_succeeded(output: str, method: str) -> bool:
    if not isinstance(output,str) or not output or 'FAILURES!!!' in output or 'INSTRUMENTATION_CODE: 0' in output:
        return False
    class_name,test_name=method.split('#',1)
    statuses=re.findall(r'^INSTRUMENTATION_STATUS: class=(.+)$',output,re.MULTILINE)
    tests=re.findall(r'^INSTRUMENTATION_STATUS: test=(.+)$',output,re.MULTILINE)
    codes=re.findall(r'^INSTRUMENTATION_STATUS_CODE: (-?\d+)$',output,re.MULTILINE)
    numtests=re.findall(r'^INSTRUMENTATION_STATUS: numtests=(\d+)$',output,re.MULTILINE)
    single_test=(1<=len(statuses)<=2 and len(statuses)==len(tests)
                 and statuses==[class_name]*len(statuses) and tests==[test_name]*len(tests)
                 and (not numtests or numtests==['1']*len(statuses)))
    return (single_test and codes==['1','0']
            and re.search(r'^INSTRUMENTATION_RESULT: stream=.*OK \(1 test\)',output,re.MULTILINE|re.DOTALL) is not None
            and output.rstrip().endswith('INSTRUMENTATION_CODE: -1'))


def _install_with_prestart_classification(*, verified: safety.VerifiedCampaign, profile: str,
                                           ledger_directory: str | Path, adb: str | Path,
                                           aapt: str | Path, apksigner: str | Path,
                                           method_key: str | None,
                                           runner: Callable) -> dict:
    try:
        return _install_verified_campaign(
            verified=verified,profile=profile,ledger_directory=ledger_directory,
            adb=adb,aapt=aapt,apksigner=apksigner,method_key=method_key,runner=runner)
    except safety.SafetyError:
        expected=S1B_METHOD_KEYS.get(method_key) if method_key is not None else None
        if expected is not None:
            try:
                safety.read_install_attempt(ledger_directory=ledger_directory,verified=verified,
                                            expected_method=expected)
            except safety.SafetyError:
                pass
            else:
                safety.mark_runtime_not_started(ledger_directory=ledger_directory,verified=verified,
                    expected_method=expected,reason='pre_instrumentation_setup_failed')
        raise


def install_verified_campaign(*, verified: safety.VerifiedCampaign, profile: str,
                              ledger_directory: str | Path, adb: str | Path,
                              aapt: str | Path, apksigner: str | Path,
                              runner: Callable = subprocess.run) -> dict:
    return _install_with_prestart_classification(
        verified=verified, profile=profile, ledger_directory=ledger_directory,
        adb=adb, aapt=aapt, apksigner=apksigner, method_key=None, runner=runner,
    )


def run_campaign_install(*, target_apk: str | Path, test_apk: str | Path,
                         campaign_id: str, profile: str, method_key: str | None = None,
                         staging_root: str | Path, ledger_directory: str | Path,
                         sdk_root: str | Path | None = None,
                         runner: Callable = subprocess.run) -> dict:
    require_android_common_lock()
    active_anchor, assessment = require_active_signing_generation()
    aapt, apksigner = safety.resolve_android_tools(sdk_root)
    root = Path(sdk_root or os.environ.get('ANDROID_SDK_ROOT') or os.environ.get('ANDROID_HOME', ''))
    if not str(root) or not root.is_dir():
        raise safety.SafetyError('Android SDK root is unavailable')
    adb = safety._tool_path(root / 'platform-tools' / 'adb', 'adb')
    verified = safety.verify_campaign_apks(
        target_apk=target_apk, test_apk=test_apk, campaign_id=campaign_id,
        staging_root=staging_root, trust_anchor=active_anchor, aapt=aapt, apksigner=apksigner,
    )
    checked_campaign_review(assessment, verified, profile, method_key)
    return _install_with_prestart_classification(
        verified=verified, profile=profile, ledger_directory=ledger_directory,
        adb=adb, aapt=aapt, apksigner=apksigner, method_key=method_key, runner=runner,
    )


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description='Install one verified S1B campaign pair and run one exact Owner-approved instrumentation method.')
    parser.add_argument('--campaign-id', required=True)
    parser.add_argument('--profile', required=True, choices=('api26', 'api35-4k', 'api35-16k'))
    parser.add_argument('--method', required=True, choices=('ownership','ownership-recovery','ownership-recovery-r2','chat-core','chat-core-recovery'))
    parser.add_argument('--target-apk', required=True, type=Path)
    parser.add_argument('--test-apk', required=True, type=Path)
    parser.add_argument('--staging-root', required=True, type=Path)
    parser.add_argument('--ledger-directory', required=True, type=Path)
    parser.add_argument('--sdk-root', type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        result = run_campaign_install(
            target_apk=args.target_apk, test_apk=args.test_apk, campaign_id=args.campaign_id,
            profile=args.profile, method_key=args.method,
            staging_root=args.staging_root, ledger_directory=args.ledger_directory,
            sdk_root=args.sdk_root,
        )
        print(json.dumps(result, sort_keys=True))
        return 0
    except safety.SafetyError as exc:
        print(json.dumps({'status': 'REJECTED', 'error': str(exc)}, sort_keys=True), file=sys.stderr)
        return 2
    except OSError:
        print(json.dumps({'status': 'REJECTED', 'error': 'Host file operation failed'}, sort_keys=True), file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
