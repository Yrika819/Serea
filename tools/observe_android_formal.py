#!/usr/bin/env python3
"""Bounded read-only observations while UTP runs; never determines the test result."""
import argparse
import datetime
import os
import signal
import resource
import re
import tempfile
import subprocess
import sys
import time
from pathlib import Path

STOP = False
PREVIOUS_TARGET_PIDS = None
DEVICE_SCRIPT = r'''for p in app.serea.android.isolated app.serea.android.isolated.test app.serea.android.r25testime; do
  echo "package=$p"
  ids=$(pidof "$p" 2>/dev/null); rc=$?; echo "pids=$ids"; echo "pidof_exit=$rc"
  for id in $ids; do
    case "$id" in *[!0-9]*|'') echo invalid_pid; continue;; esac
    echo "pid=$id"
    if [ -r "/proc/$id/status" ]; then
      grep -E '^(Name|State|Threads|VmRSS|VmSize):' "/proc/$id/status" || echo status_unavailable
    else echo status_unavailable; fi
    if [ -d "/proc/$id/fd" ]; then
      fds=$(ls -1 "/proc/$id/fd" 2>/dev/null); fd_rc=$?
      if [ "$fd_rc" -eq 0 ]; then printf '%s\n' "$fds" | wc -l; else echo fd_unavailable; fi
    else echo fd_unavailable; fi
  done
done
printf 'device_uptime='; cat /proc/uptime
printf 'boot_id='; cat /proc/sys/kernel/random/boot_id
printf 'device_memory='; grep -E '^(MemTotal|MemAvailable|MemFree):' /proc/meminfo || echo unavailable
printf 'device_oom='; grep -E '^oom_kill ' /proc/vmstat || echo unavailable
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
                p = subprocess.run(args, stdout=output, stderr=subprocess.STDOUT,
                                   timeout=timeout, check=False, input=stdin, preexec_fn=bound_child)
                code = p.returncode
            except subprocess.TimeoutExpired:
                code = 124
            output.seek(0)
            data = output.read(limit)
            truncated = len(data) >= limit
            return code, len(data), data.decode('utf-8', 'replace'), truncated
    except OSError as exc:
        return 127, 0, type(exc).__name__, False


def valid_device_observation(data):
    packages = ('app.serea.android.isolated', 'app.serea.android.isolated.test', 'app.serea.android.r25testime')
    for package in packages:
        match = re.search(r'^package=' + re.escape(package) + r'\npids=([^\n]*)\npidof_exit=([0-9]+)$', data, re.M)
        if not match:
            return False
        pids, code = match.groups()
        if not ((code == '0' and re.fullmatch(r'[0-9]+(?: [0-9]+)*', pids)) or (code == '1' and not pids)):
            return False
    return bool(re.search(r'^device_uptime=[0-9]+(?:[.][0-9]+)? [0-9]+(?:[.][0-9]+)?$', data, re.M) and
                re.search(r'^boot_id=[0-9a-fA-F-]{36}$', data, re.M) and
                re.search(r'^device_memory=MemTotal:\s+[0-9]+ kB$', data, re.M) and
                re.search(r'^MemAvailable:\s+[0-9]+ kB$', data, re.M) and
                re.search(r'^device_oom=oom_kill\s+[0-9]+$', data, re.M) and
                'fd_unavailable' not in data and 'status_unavailable' not in data)


def sample(out, serial, cycle):
    global PREVIOUS_TARGET_PIDS
    incomplete = False
    target_disappeared = False
    out.write(f'cycle_start={cycle} at={stamp()}\n')
    for name, cmd, stdin in [('adb_state', ['adb', '-s', serial, 'get-state'], None),
                             ('device_processes', ['adb', '-s', serial, 'shell', 'sh'], DEVICE_SCRIPT.encode())]:
        code, size, data, truncated = command(cmd, stdin=stdin)
        out.write(f'{name}: exit={code} bytes={size} truncated={str(truncated).lower()}\n{data}\n')
        if code or truncated or (name == 'adb_state' and data.strip() != 'device') or (name == 'device_processes' and not valid_device_observation(data)):
            incomplete = True
        if name == 'device_processes' and code == 0 and not truncated and valid_device_observation(data):
            match = re.search(r'^package=app[.]serea[.]android[.]isolated\npids=([^\n]*)\npidof_exit=', data, re.M)
            current = set(match.group(1).split())
            target_disappeared = PREVIOUS_TARGET_PIDS is not None and bool(PREVIOUS_TARGET_PIDS - current)
            if target_disappeared:
                out.write(f'target_pid_transition=disappeared previous={",".join(sorted(PREVIOUS_TARGET_PIDS))} current={",".join(sorted(current))}\n')
            PREVIOUS_TARGET_PIDS = current
    # Exit records are sampled every fourth cycle and remain observational: the
    # platform may remove them during UTP uninstall. No command mutates the guest.
    if cycle % 4 == 1 or target_disappeared:
        code, size, data, truncated = command(['adb', '-s', serial, 'shell', 'dumpsys', 'activity', 'exit-info', 'app.serea.android.isolated'])
        valid = code == 0 and 'ACTIVITY MANAGER PROCESS EXIT INFO' in data and not truncated
        out.write(f'exit_info: exit={code} bytes={size} valid={str(valid).lower()} truncated={str(truncated).lower()}\n{data}\n')
        incomplete |= not valid
    try:
        mem = Path('/proc/meminfo').read_text().splitlines()
        out.write('host_memory=' + ';'.join(line for line in mem if line.startswith(('MemTotal:', 'MemAvailable:', 'MemFree:'))) + '\n')
        group = Path('/proc/self/cgroup').read_text().strip()
        out.write('host_cgroup=' + group.replace('\n', ';') + '\n')
        unified = next((line.split(':', 2)[2] for line in group.splitlines() if line.startswith('0::')), '')
        if not unified.startswith('/') or '..' in unified.split('/'):
            raise ValueError('unavailable unified cgroup')
        for name in ('memory.current', 'memory.events'):
            path = Path('/sys/fs/cgroup') / unified.lstrip('/') / name
            out.write(f'host_{name}=' + path.read_text().strip().replace('\n', ';') + '\n')
    except (OSError, ValueError, IndexError) as exc:
        out.write(f'host_cgroup_or_memory=UNAVAILABLE:{type(exc).__name__}\n')
        incomplete = True
    out.write(f'cycle_end={cycle} incomplete={str(incomplete).lower()} at={stamp()}\n')
    out.flush()
    return incomplete


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--serial', required=True)
    p.add_argument('--output', required=True)
    p.add_argument('--interval', type=int, default=20)
    args = p.parse_args()
    if not args.serial.startswith('emulator-') or not args.serial[9:].isdigit() or args.interval < 15:
        p.error('expected explicit emulator serial and interval >= 15 seconds')
    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    cycles = 0
    incomplete = False
    with open(args.output, 'w', encoding='utf-8', buffering=1) as out:
        out.write(f'observer_start={stamp()} serial={args.serial} run={os.getenv("GITHUB_RUN_ID", "unknown")} attempt={os.getenv("GITHUB_RUN_ATTEMPT", "unknown")}\n')
        while not STOP:
            cycles += 1
            incomplete |= sample(out, args.serial, cycles)
            for _ in range(args.interval):
                if STOP: break
                time.sleep(1)
        out.write(f'observer_end={stamp()} cycles={cycles} observation={"INCOMPLETE" if incomplete or not cycles else "RECORDED"}\n')
    return 0


if __name__ == '__main__':
    sys.exit(main())
