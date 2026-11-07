import importlib.util
import io
from pathlib import Path
import sys
import time
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    'observe_android_formal', Path(__file__).resolve().parents[1] / 'observe_android_formal.py')
observer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(observer)
PACKAGES = observer.PACKAGES


def host_files(path, *args, **kwargs):
    if str(path) == '/proc/meminfo': return 'MemTotal: 10 kB\nMemAvailable: 5 kB\n'
    if str(path) == '/proc/self/cgroup': return '0::/runner\n'
    if str(path).endswith('memory.current'): return '100\n'
    if str(path).endswith('memory.events'): return 'oom 0\noom_kill 0\n'
    raise FileNotFoundError(path)


def device_sample(target_pids=('123',), instrumentation_pids=('456',),
                  fd='RECORDED count=8', oom='RECORDED oom_kill 0'):
    parts = []
    for index, package in enumerate(PACKAGES):
        pids = target_pids if index == 0 else instrumentation_pids if index == 1 else ()
        parts.extend((f'package={package}', f'pids={" ".join(pids)}', f'pidof_exit={0 if pids else 1}'))
        for pid in pids:
            parts.extend((f'pid={pid}', 'proc_status_status=RECORDED', 'Name: app.serea',
                          'State: S', 'Threads: 14', 'VmRSS: 1024 kB', f'fd_status={fd}'))
    parts.extend(('device_uptime=123.00 99.00', 'boot_id=12345678-1234-1234-1234-123456789abc',
                  'device_memory_status=RECORDED', 'MemTotal: 100 kB', 'MemAvailable: 50 kB',
                  f'device_oom_status={oom}'))
    return '\n'.join(parts) + '\n'


def command_for(data=None, state=(0, 7, 'device\n', False),
                exit_info=(0, 35, 'ACTIVITY MANAGER PROCESS EXIT INFO', False)):
    device = data if data is not None else device_sample()
    def run(cmd, timeout=5, stdin=None):
        if 'get-state' in cmd: return state
        if 'exit-info' in cmd: return exit_info
        assert stdin and b'pidof' in stdin
        return 0, len(device), device, False
    return run


class ObserverTest(unittest.TestCase):
    def setUp(self):
        self.summary = observer.Summary()
        self.host_patch = patch.object(Path, 'read_text', autospec=True, side_effect=host_files)
        self.host_patch.start()
        self.addCleanup(self.host_patch.stop)

    def sample(self, output, cycle=1, api=35, command=None):
        return observer.sample(output, 'emulator-5554', cycle, api, self.summary, command or command_for())

    def test_complete_sample_records_pid_and_resources(self):
        output = io.StringIO()
        self.assertFalse(self.sample(output))
        self.assertIn('target_pid_tracking=RECORDED count=1', output.getvalue())
        self.assertIn('device_resource_observation=RECORDED', output.getvalue())

    def test_pid_disappearance_is_independent_of_fd_oom_and_exit_info_gaps(self):
        outputs = [io.StringIO(), io.StringIO()]
        responses = [device_sample(), device_sample((), fd='PERMISSION_DENIED', oom='PERMISSION_DENIED')]
        def injected(cmd, timeout=5, stdin=None):
            if 'get-state' in cmd: return 0, 7, 'device\n', False
            if 'exit-info' in cmd: return 0, 15, 'Unknown command', False
            data = responses.pop(0)
            return 0, len(data), data, False
        self.sample(outputs[0], 1, 35, injected)
        self.assertTrue(self.sample(outputs[1], 2, 35, injected))
        combined = ''.join(item.getvalue() for item in outputs)
        self.assertIn('process_pid_transition=DISAPPEARED package=app.serea.android.isolated ', combined)
        self.assertIn('target_pid_tracking=RECORDED count=0', combined)
        self.assertIn('fd:PERMISSION_DENIED', combined)
        self.assertIn('device_oom:PERMISSION_DENIED', combined)
        self.assertIn('exit_info_status=UNSUPPORTED_COMMAND', combined)
        self.assertEqual(self.summary.pid_disappearances, 1)

    def test_api26_exit_info_is_unsupported_without_command(self):
        seen = []
        base = command_for()
        def command(cmd, timeout=5, stdin=None):
            seen.append(cmd)
            return base(cmd, timeout, stdin)
        output = io.StringIO()
        self.sample(output, api=26, command=command)
        self.assertIn('exit_info_status=UNSUPPORTED_API api_level=26', output.getvalue())
        self.assertFalse(any('exit-info' in cmd for cmd in seen))
        self.assertTrue(self.summary.gaps)

    def test_adb_timeout_is_gap_without_fabricated_pid_disappearance(self):
        self.summary.previous_pids[PACKAGES[0]] = {'123'}
        output = io.StringIO()
        self.assertTrue(self.sample(output, command=command_for(state=(124, 0, '', False))))
        self.assertIn('adb_state_status=TIMEOUT', output.getvalue())
        self.assertNotIn('process_pid_transition=DISAPPEARED', output.getvalue())
        self.assertEqual(self.summary.previous_pids[PACKAGES[0]], {'123'})

    def test_oom_counter_missing_is_distinct_and_incomplete(self):
        output = io.StringIO()
        self.assertTrue(self.sample(output, command=command_for(data=device_sample(oom='COUNTER_ABSENT'))))
        self.assertIn('device_oom:COUNTER_ABSENT', output.getvalue())
        self.assertIn('target_pid_tracking=RECORDED count=1', output.getvalue())

    def test_device_process_timeout_marks_pid_tracking_missing(self):
        def timeout(cmd, timeout=5, stdin=None):
            if 'get-state' in cmd: return 0, 7, 'device\n', False
            if stdin: return 124, 0, '', False
            return command_for()(cmd, timeout, stdin)
        output = io.StringIO()
        self.assertTrue(self.sample(output, command=timeout))
        self.assertIn('target_pid_tracking=INCOMPLETE reason=TIMEOUT', output.getvalue())
        self.assertNotIn('process_pid_transition=DISAPPEARED', output.getvalue())

    def test_pidof_failure_is_not_recorded_as_process_disappearance(self):
        data = device_sample().replace(
            f'package={PACKAGES[0]}\npids=123\npidof_exit=0',
            f'package={PACKAGES[0]}\npids=\npidof_exit=127')
        output = io.StringIO()
        self.assertTrue(self.sample(output, command=command_for(data=data)))
        self.assertIn('target_pid_tracking=INCOMPLETE', output.getvalue())
        self.assertIn('pid_list:app.serea.android.isolated', output.getvalue())
        self.assertNotIn('process_pid_transition=DISAPPEARED', output.getvalue())
        self.assertNotIn('target_pid_tracking=RECORDED count=0', output.getvalue())

    def test_adb_disconnect_does_not_clear_last_known_pids(self):
        self.summary.previous_pids[PACKAGES[0]] = {'123'}

        def disconnected(cmd, timeout=5, stdin=None):
            if 'get-state' in cmd:
                return 1, 7, 'offline\n', False
            return 1, 8, 'device offline', False

        output = io.StringIO()
        self.assertTrue(self.sample(output, command=disconnected))
        self.assertIn('adb_state_status=COMMAND_ERROR', output.getvalue())
        self.assertIn('target_pid_tracking=INCOMPLETE', output.getvalue())
        self.assertNotIn('process_pid_transition=DISAPPEARED', output.getvalue())
        self.assertEqual(self.summary.previous_pids[PACKAGES[0]], {'123'})

    def test_truncated_device_output_is_incomplete_not_a_zero_pid_sample(self):
        def truncated(cmd, timeout=5, stdin=None):
            if 'get-state' in cmd:
                return 0, 7, 'device\n', False
            return 0, 16384, 'x' * 16384, True

        output = io.StringIO()
        self.assertTrue(self.sample(output, command=truncated))
        self.assertIn('target_pid_tracking=INCOMPLETE reason=OUTPUT_TRUNCATED', output.getvalue())
        self.assertNotIn('process_pid_transition=DISAPPEARED', output.getvalue())

    def test_command_timeout_terminates_child_and_returns_bounded_status(self):
        started = time.monotonic()
        code, size, _, truncated = observer.command(
            [sys.executable, '-c', 'import time; time.sleep(10)'], timeout=0.1)
        self.assertEqual(code, 124)
        self.assertEqual(size, 0)
        self.assertFalse(truncated)
        self.assertLess(time.monotonic() - started, 2)

    def test_command_output_is_capped(self):
        code, size, _, truncated = observer.command(
            [sys.executable, '-c', "import sys; sys.stdout.write('x' * 100000)"], timeout=5)
        self.assertNotEqual(code, 0)
        self.assertEqual(size, 16384)
        self.assertTrue(truncated)

    def test_instrumentation_pid_disappearance_collects_its_exit_info(self):
        responses = [device_sample(), device_sample(instrumentation_pids=())]
        seen = []
        def injected(cmd, timeout=5, stdin=None):
            seen.append(cmd)
            if 'get-state' in cmd: return 0, 7, 'device\n', False
            if 'exit-info' in cmd: return 0, 35, 'ACTIVITY MANAGER PROCESS EXIT INFO', False
            data = responses.pop(0)
            return 0, len(data), data, False
        self.sample(io.StringIO(), 1, 35, injected)
        output = io.StringIO()
        self.assertFalse(self.sample(output, 2, 35, injected))
        self.assertIn('process_pid_transition=DISAPPEARED package=app.serea.android.isolated.test ',
                      output.getvalue())
        self.assertTrue(any(cmd[-1] == PACKAGES[1] for cmd in seen if 'exit-info' in cmd))

    def test_permission_denied_fd_preserves_pid_sample(self):
        output = io.StringIO()
        self.assertTrue(self.sample(output, command=command_for(data=device_sample(fd='PERMISSION_DENIED'))))
        self.assertIn('target_pid_tracking=RECORDED count=1', output.getvalue())
        self.assertIn('fd:PERMISSION_DENIED', output.getvalue())

    def test_exit_info_timeout_is_explicit_gap(self):
        output = io.StringIO()
        self.sample(output, command=command_for(exit_info=(124, 0, '', False)))
        self.assertIn('exit_info_status=TIMEOUT', output.getvalue())
        self.assertIn('cycle_end=1 incomplete=true', output.getvalue())


if __name__ == '__main__': unittest.main()
