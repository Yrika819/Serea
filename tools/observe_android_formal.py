#!/usr/bin/env python3
"""Bounded, read-only observations during UTP; never determines the test result."""
import argparse
import datetime
import os
import re
import resource
import signal
import subprocess
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path

STOP = False
PACKAGES = (
    'app.serea.android.isolated',
    'app.serea.android.isolated.test',
    'app.serea.android.r25testime',
)

# One bounded device-side shell call collects process and memory fields. It does
# not dump all of ActivityManager or enumerate file descriptor targets.
DEVICE_SCRIPT = r'''for p in app.serea.android.isolated app.serea.android.isolated.test app.serea.android.r25testime; do
  echo "package=$p"
  ids=$(pidof "$p" 2>/dev/null); rc=$?; echo "pids=$ids"; echo "pidof_exit=$rc"
  for id in $ids; do
    case "$id" in *[!0-9]*|'') echo invalid_pid; continue;; esac
    echo "pid=$id"
    if [ -r "/proc/$id/status" ]; then
      status=$(grep -E '^(Name|State|Threads|VmRSS|VmSize):' "/proc/$id/status" 2>/dev/null); status_rc=$?
      if [ "$status_rc" -eq 0 ] && echo "$status" | grep -q '^Threads:' && echo "$status" | grep -q '^VmRSS:'; then
        echo proc_status_status=RECORDED; printf '%s\n' "$status"
      elif [ -d "/proc/$id" ]; then echo proc_status_status=READ_ERROR
      else echo proc_status_status=PID_GONE_DURING_SAMPLE; fi
    elif [ -d "/proc/$id" ]; then echo proc_status_status=PERMISSION_DENIED
    else echo proc_status_status=PID_GONE_DURING_SAMPLE; fi
    if [ -d "/proc/$id/fd" ]; then
      fds=$(ls -1 "/proc/$id/fd" 2>&1); fd_rc=$?
      if [ "$fd_rc" -eq 0 ]; then
        if [ -n "$fds" ]; then set -- $fds; count=$#; else count=0; fi
        echo "fd_status=RECORDED count=$count"
      else
        case "$fds" in
          *"Permission denied"*|*"Operation not permitted"*) echo fd_status=PERMISSION_DENIED;;
          *"No such file or directory"*) echo fd_status=PID_GONE_DURING_SAMPLE;;
          *) echo fd_status=READ_ERROR;;
        esac
      fi
    elif [ -d "/proc/$id" ]; then echo fd_status=PERMISSION_DENIED
    else echo fd_status=PID_GONE_DURING_SAMPLE; fi
  done
done
printf 'device_uptime='; cat /proc/uptime 2>/dev/null || echo unavailable
printf 'boot_id='; cat /proc/sys/kernel/random/boot_id 2>/dev/null || echo unavailable
if [ -r /proc/meminfo ]; then
  memory=$(grep -E '^(MemTotal|MemAvailable|MemFree):' /proc/meminfo 2>/dev/null)
  if echo "$memory" | grep -q '^MemTotal:' && echo "$memory" | grep -q '^MemAvailable:'; then
    echo device_memory_status=RECORDED; printf '%s\n' "$memory"
  else echo device_memory_status=READ_ERROR; fi
else echo device_memory_status=PERMISSION_DENIED; fi
if [ -r /proc/vmstat ]; then
  oom=$(grep -E '^oom_kill ' /proc/vmstat 2>/dev/null); oom_rc=$?
  if [ "$oom_rc" -eq 0 ] && echo "$oom" | grep -Eq '^oom_kill [0-9]+$'; then
    echo "device_oom_status=RECORDED $oom"
  elif [ "$oom_rc" -eq 1 ]; then echo device_oom_status=COUNTER_ABSENT
  else echo device_oom_status=READ_ERROR; fi
else echo device_oom_status=PERMISSION_DENIED; fi
'''


def stop(_sig, _frame):
    global STOP
    STOP = True


def stamp():
    return datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='milliseconds')


def command(args, timeout=5, stdin=None):
    # Child output goes to a size-limited temporary file, not an unbounded pipe.
    limit = 16384

    def bound_child():
        resource.setrlimit(resource.RLIMIT_FSIZE, (limit, limit))

    try:
        with tempfile.TemporaryFile() as output:
            try:
                process = subprocess.run(args, stdout=output, stderr=subprocess.STDOUT,
                                         timeout=timeout, check=False, input=stdin,
                                         preexec_fn=bound_child)
                code = process.returncode
            except subprocess.TimeoutExpired:
                code = 124
            output.seek(0)
            data = output.read(limit)
            return code, len(data), data.decode('utf-8', 'replace'), len(data) >= limit
    except OSError as exc:
        return 127, 0, type(exc).__name__, False


def command_failure(code, data):
    if code == 124:
        return 'TIMEOUT'
    if code == 127:
        return 'EXEC_ERROR'
    lowered = data.lower()
    if 'permission denied' in lowered or 'operation not permitted' in lowered:
        return 'PERMISSION_DENIED'
    if 'unknown command' in lowered or 'not found' in lowered:
        return 'UNSUPPORTED_COMMAND'
    return 'COMMAND_ERROR'


def package_pids(data, package):
    match = re.search(
        r'^package=' + re.escape(package) + r'\npids=([^\n]*)\npidof_exit=([0-9]+)$',
        data, re.M)
    if not match:
        return None
    values, code = match.groups()
    if code == '0' and re.fullmatch(r'[0-9]+(?: [0-9]+)*', values):
        return set(values.split())
    if code == '1' and not values:
        return set()
    return None


def device_gaps(data):
    """Return missing fields while preserving individually useful PID samples."""
    gaps = set()
    for package in PACKAGES:
        pids = package_pids(data, package)
        if pids is None:
            gaps.add(f'pid_list:{package}')
            continue
        for pid in pids:
            section = re.search(
                r'^pid=' + re.escape(pid) + r'\n(.*?)(?=^pid=|^package=|\Z)', data, re.M | re.S)
            block = section.group(1) if section else ''
            for field_name, status_name in (('proc_status', 'proc_status_status'),
                                            ('fd', 'fd_status')):
                match = re.search(r'^' + status_name + r'=([^\n]+)$', block, re.M)
                status = match.group(1) if match else 'MISSING'
                if status == 'RECORDED' or status.startswith('RECORDED '):
                    continue
                gaps.add(f'{field_name}:{status}')
    for field_name in ('device_memory_status', 'device_oom_status'):
        match = re.search(r'^' + field_name + r'=([^\n]+)$', data, re.M)
        status = match.group(1).split()[0] if match else 'MISSING'
        if status != 'RECORDED':
            gaps.add(f'{field_name.removesuffix("_status")}:{status}')
    if not re.search(r'^device_uptime=[0-9]+(?:[.][0-9]+)? [0-9]+(?:[.][0-9]+)?$', data, re.M):
        gaps.add('device_uptime:MISSING_OR_INVALID')
    if not re.search(r'^boot_id=[0-9a-fA-F-]{36}$', data, re.M):
        gaps.add('boot_id:MISSING_OR_INVALID')
    return gaps


@dataclass
class Summary:
    cycles: int = 0
    pid_samples: int = 0
    target_pid_seen: bool = False
    instrumentation_pid_seen: bool = False
    pid_disappearances: int = 0
    previous_pids: dict = field(default_factory=dict)
    packages_seen: set = field(default_factory=set)
    adb_failures: int = 0
    device_process_failures: int = 0
    exit_info_statuses: list = field(default_factory=list)
    gaps: set = field(default_factory=set)

    def pid_status(self):
        return 'RECORDED' if self.cycles and self.pid_samples == self.cycles else 'INCOMPLETE'


def exit_info_sample(out, serial, api_level, summary, package, run_command=command):
    if api_level < 30:
        status = 'UNSUPPORTED_API'
        out.write(f'exit_info_status={status} api_level={api_level}\n')
    else:
        code, size, data, truncated = run_command(
            ['adb', '-s', serial, 'shell', 'dumpsys', 'activity', 'exit-info', package])
        if truncated:
            status = 'OUTPUT_TRUNCATED'
        elif code:
            status = command_failure(code, data)
        elif 'unknown command' in data.lower():
            status = 'UNSUPPORTED_COMMAND'
        elif 'ACTIVITY MANAGER PROCESS EXIT INFO' in data:
            status = 'RECORDED'
        elif 'permission denial' in data.lower() or 'permission denied' in data.lower():
            status = 'PERMISSION_DENIED'
        else:
            status = 'INVALID_OUTPUT'
        out.write(f'exit_info_status={status} exit={code} bytes={size} truncated={str(truncated).lower()}\n')
        if status == 'RECORDED':
            out.write(data + '\n')
    summary.exit_info_statuses.append(status)
    if status != 'RECORDED':
        summary.gaps.add(f'exit_info:{status}')


def sample(out, serial, cycle, api_level, summary, run_command=command):
    summary.cycles += 1
    out.write(f'cycle_start={cycle} at={stamp()}\n')
    state_code, state_size, state_data, state_truncated = run_command(
        ['adb', '-s', serial, 'get-state'])
    if state_code == 0 and state_data.strip() == 'device' and not state_truncated:
        out.write(f'adb_state_status=RECORDED bytes={state_size}\n')
    else:
        status = 'OUTPUT_TRUNCATED' if state_truncated else command_failure(state_code, state_data)
        if state_code == 0 and state_data.strip() != 'device':
            status = 'DEVICE_NOT_READY'
        summary.adb_failures += 1
        summary.gaps.add(f'adb_state:{status}')
        out.write(f'adb_state_status={status} exit={state_code} bytes={state_size}\n')

    process_code, process_size, process_data, process_truncated = run_command(
        ['adb', '-s', serial, 'shell', 'sh'], stdin=DEVICE_SCRIPT.encode())
    out.write(f'device_processes: exit={process_code} bytes={process_size} truncated={str(process_truncated).lower()}\n')
    out.write(process_data + '\n')
    disappeared_packages = []
    parsed = {package: package_pids(process_data, package) for package in PACKAGES}
    if process_code == 0 and not process_truncated and all(value is not None for value in parsed.values()):
        summary.pid_samples += 1
        for package in PACKAGES:
            current = parsed[package]
            if current:
                summary.packages_seen.add(package)
            if package == PACKAGES[0]:
                summary.target_pid_seen |= bool(current)
            elif package == PACKAGES[1]:
                summary.instrumentation_pid_seen |= bool(current)
            previous = summary.previous_pids.get(package)
            if previous is not None:
                lost = previous - current
            else:
                lost = set()
            if lost:
                disappeared_packages.append(package)
                summary.pid_disappearances += len(lost)
                out.write(f'process_pid_transition=DISAPPEARED package={package} count={len(lost)} current_count={len(current)} at={stamp()}\n')
            summary.previous_pids[package] = current
        out.write(f'target_pid_tracking=RECORDED count={len(parsed[PACKAGES[0]])}\n')
        out.write(f'instrumentation_pid_tracking=RECORDED count={len(parsed[PACKAGES[1]])}\n')
        out.write(f'test_ime_pid_tracking=RECORDED count={len(parsed[PACKAGES[2]])}\n')
        missing = device_gaps(process_data)
        if missing:
            summary.gaps.update(missing)
            out.write('device_resource_observation=INCOMPLETE missing=' + ','.join(sorted(missing)) + '\n')
        else:
            out.write('device_resource_observation=RECORDED\n')
    else:
        summary.device_process_failures += 1
        status = 'OUTPUT_TRUNCATED' if process_truncated else command_failure(process_code, process_data)
        summary.gaps.add(f'pid_tracking:{status}')
        out.write(f'target_pid_tracking=INCOMPLETE reason={status}\n')
        out.write(f'device_resource_observation=INCOMPLETE reason={status}\n')

    # API 26 predates ActivityManager's historical process-exit API. On newer
    # APIs, sample exit info periodically and immediately for any tracked PID
    # disappearance. PID tracking is independent of this read.
    if disappeared_packages:
        for package in disappeared_packages:
            exit_info_sample(out, serial, api_level, summary, package, run_command)
    elif cycle % 4 == 1:
        exit_info_sample(out, serial, api_level, summary, PACKAGES[0], run_command)

    try:
        mem = Path('/proc/meminfo').read_text().splitlines()
        memory_fields = [line for line in mem if line.startswith(('MemTotal:', 'MemAvailable:', 'MemFree:'))]
        if not any(line.startswith('MemTotal:') for line in memory_fields) or not any(line.startswith('MemAvailable:') for line in memory_fields):
            raise ValueError('host_memory_fields_missing')
        out.write('host_memory_status=RECORDED\n')
        out.write('host_memory=' + ';'.join(memory_fields) + '\n')
        group = Path('/proc/self/cgroup').read_text().strip()
        unified = next((line.split(':', 2)[2] for line in group.splitlines() if line.startswith('0::')), '')
        if not unified.startswith('/') or '..' in unified.split('/'):
            raise ValueError('unavailable_unified_cgroup')
        metrics = {}
        for name in ('memory.current', 'memory.events'):
            path = Path('/sys/fs/cgroup') / unified.lstrip('/') / name
            metrics[name] = path.read_text().strip().replace('\n', ';')
        out.write('host_cgroup_status=RECORDED\n')
        out.write('host_cgroup=' + group.replace('\n', ';') + '\n')
        for name, value in metrics.items():
            out.write(f'host_{name}={value}\n')
    except (OSError, ValueError, IndexError) as exc:
        reason = type(exc).__name__
        out.write(f'host_resource_status=UNAVAILABLE_{reason}\n')
        summary.gaps.add(f'host_resources:{reason}')

    out.write(f'cycle_end={cycle} incomplete={str(bool(summary.gaps)).lower()} at={stamp()}\n')
    out.flush()
    return bool(summary.gaps)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--serial', required=True)
    parser.add_argument('--api-level', required=True, type=int, choices=(26, 35))
    parser.add_argument('--output', required=True)
    parser.add_argument('--interval', type=int, default=20)
    args = parser.parse_args()
    if not args.serial.startswith('emulator-') or not args.serial[9:].isdigit() or args.interval < 15:
        parser.error('expected explicit emulator serial and interval >= 15 seconds')
    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    summary = Summary()
    with open(args.output, 'w', encoding='utf-8', buffering=1) as out:
        out.write(f'observer_start={stamp()} serial={args.serial} api_level={args.api_level} '
                  f'run={os.getenv("GITHUB_RUN_ID", "unknown")} '
                  f'attempt={os.getenv("GITHUB_RUN_ATTEMPT", "unknown")}\n')
        while not STOP:
            sample(out, args.serial, summary.cycles + 1, args.api_level, summary)
            for _ in range(args.interval):
                if STOP:
                    break
                time.sleep(1)
        if not summary.target_pid_seen:
            summary.gaps.add('target_pid:NEVER_SEEN')
        if not summary.instrumentation_pid_seen:
            summary.gaps.add('instrumentation_pid:NEVER_SEEN')
        observation = 'INCOMPLETE' if summary.gaps or not summary.cycles else 'RECORDED'
        if args.api_level < 30:
            exit_status = 'UNSUPPORTED_API'
        elif not summary.exit_info_statuses:
            exit_status = 'NOT_SAMPLED'
        elif all(value == 'RECORDED' for value in summary.exit_info_statuses):
            exit_status = 'RECORDED'
        else:
            exit_status = 'INCOMPLETE'
        gaps = ','.join(sorted(summary.gaps)) if summary.gaps else 'none'
        out.write(f'observer_end={stamp()} cycles={summary.cycles} observation={observation} '
                  f'pid_tracking={summary.pid_status()} target_pid_seen={str(summary.target_pid_seen).lower()} '
                  f'instrumentation_pid_seen={str(summary.instrumentation_pid_seen).lower()} '
                  f'packages_seen={",".join(sorted(summary.packages_seen)) or "none"} '
                  f'pid_disappearances={summary.pid_disappearances} adb_failures={summary.adb_failures} '
                  f'exit_info={exit_status} missing={gaps}\n')
    # Deliberately return zero for completed observation even when evidence is
    # incomplete. The artifact carries the gap; no test/gate result is masked.
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
