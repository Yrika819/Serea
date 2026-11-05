import importlib.util
import io
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('observe_android_formal', Path(__file__).resolve().parents[1] / 'observe_android_formal.py')
observer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(observer)


def files(path, *args, **kwargs):
    if str(path) == '/proc/meminfo': return 'MemTotal: 10 kB\nMemAvailable: 5 kB\n'
    if str(path) == '/proc/self/cgroup': return '0::/runner\n'
    if str(path).endswith('memory.current'): return '100\n'
    if str(path).endswith('memory.events'): return 'oom 0\noom_kill 0\n'
    raise FileNotFoundError(path)


def normal(cmd, timeout=5, stdin=None):
    if 'get-state' in cmd: return 0, 7, 'device\n', False
    if 'exit-info' in cmd: return 0, 35, 'ACTIVITY MANAGER PROCESS EXIT INFO', False
    assert stdin and b'pidof' in stdin
    data = ''.join(f'package={p}\npids=\npidof_exit=1\n' for p in (
        'app.serea.android.isolated', 'app.serea.android.isolated.test', 'app.serea.android.r25testime'))
    data += 'device_uptime=123.00 99.00\nboot_id=12345678-1234-1234-1234-123456789abc\ndevice_memory=MemTotal: 100 kB\nMemAvailable: 50 kB\ndevice_oom=oom_kill 0\n'
    return 0, len(data), data, False


class ObserverTest(unittest.TestCase):
    def test_normal_sample_records_completion(self):
        out = io.StringIO()
        with patch.object(observer, 'command', side_effect=normal), patch.object(Path, 'read_text', autospec=True, side_effect=files):
            self.assertFalse(observer.sample(out, 'emulator-5554', 1))
        self.assertIn('cycle_end=1 incomplete=false', out.getvalue())
        self.assertIn('host_memory.events=oom 0;oom_kill 0', out.getvalue())

    def test_device_offline_and_command_timeout_mark_incomplete(self):
        for injected in ((0, 8, 'offline\n', False), (124, 0, '', False), (1, 0, '', False)):
            def fail(cmd, timeout=5, stdin=None):
                return injected if 'get-state' in cmd else normal(cmd, timeout, stdin)
            out = io.StringIO()
            with self.subTest(injected=injected), patch.object(observer, 'command', side_effect=fail), patch.object(Path, 'read_text', autospec=True, side_effect=files):
                self.assertTrue(observer.sample(out, 'emulator-5554', 1))
                self.assertIn('cycle_end=1 incomplete=true', out.getvalue())

    def test_unavailable_fd_and_exit_info_are_not_success(self):
        def bad(cmd, timeout=5, stdin=None):
            if stdin: return 0, 24, 'boot_id=abc\nfd_unavailable\n', False
            if 'exit-info' in cmd: return 0, 15, 'Unknown command', False
            return normal(cmd, timeout, stdin)
        out = io.StringIO()
        with patch.object(observer, 'command', side_effect=bad), patch.object(Path, 'read_text', autospec=True, side_effect=files):
            self.assertTrue(observer.sample(out, 'emulator-5554', 1))
            self.assertIn('valid=false', out.getvalue())

    def test_missing_device_oom_and_memory_are_incomplete(self):
        def missing(cmd, timeout=5, stdin=None):
            result = normal(cmd, timeout, stdin)
            if stdin: return 0, 1, result[2].replace('device_oom=oom_kill 0', ''), False
            return result
        out = io.StringIO()
        with patch.object(observer, 'command', side_effect=missing), patch.object(Path, 'read_text', autospec=True, side_effect=files):
            self.assertTrue(observer.sample(out, 'emulator-5554', 1))

    def test_pidof_error_is_not_absence(self):
        def bad_pid(cmd, timeout=5, stdin=None):
            result = normal(cmd, timeout, stdin)
            if stdin: return 0, result[1], result[2].replace('pidof_exit=1', 'pidof_exit=2', 1), False
            return result
        with patch.object(observer, 'command', side_effect=bad_pid), patch.object(Path, 'read_text', autospec=True, side_effect=files):
            self.assertTrue(observer.sample(io.StringIO(), 'emulator-5554', 1))

    def test_missing_host_cgroup_marks_incomplete(self):
        def missing(path, *args, **kwargs):
            if str(path).endswith('memory.events'): raise PermissionError('denied')
            return files(path, *args, **kwargs)
        with patch.object(observer, 'command', side_effect=normal), patch.object(Path, 'read_text', autospec=True, side_effect=missing):
            self.assertTrue(observer.sample(io.StringIO(), 'emulator-5554', 1))

    def test_actual_subprocess_timeout_is_bounded(self):
        status, _, _, _ = observer.command(['python3', '-c', 'import time; time.sleep(2)'], timeout=0.1)
        self.assertEqual(status, 124)

    def test_large_output_is_limited_before_collection(self):
        status, size, data, truncated = observer.command(['python3', '-c', 'import os; os.write(1, b"x" * 200000)'])
        self.assertEqual((size, len(data), truncated), (16384, 16384, True))


if __name__ == '__main__': unittest.main()
