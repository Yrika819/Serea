import importlib.util
import pathlib
import os
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

path = pathlib.Path(__file__).resolve().parents[1] / 'verify_api35_diagnostic_source.py'
spec = importlib.util.spec_from_file_location('diagnostic_source', path)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
HEAD = 'a' * 40
TREE = 'b' * 40
OTHER = 'c' * 40


def fake_git(*args):
    if args[:2] == ('merge-base', '--is-ancestor'):
        return ''
    if args == ('rev-parse', 'HEAD'):
        return HEAD
    if args == ('rev-parse', f'{HEAD}:android'):
        return TREE
    if args in (('rev-parse', f'{module.BASELINE}:android'), ('rev-parse', f'{OTHER}:android')):
        return TREE
    if len(args) == 2 and args[0] == 'rev-parse' and args[1].startswith(f'{HEAD}:'):
        return module.TRUSTED[args[1].split(':', 1)[1]]
    raise AssertionError(args)


class SourceIdentityTest(unittest.TestCase):
    def setUp(self):
        self.env = {'DIAG_EVENT': 'workflow_dispatch', 'DIAG_MODE': 'baseline', 'DIAG_SOURCE_SHA': HEAD}
        self.mock = patch.object(module, 'git', side_effect=fake_git)
        self.mock.start()
        self.addCleanup(self.mock.stop)

    def test_baseline_records_exact_tree(self):
        result = module.verify(self.env)
        self.assertEqual(result['source_sha'], HEAD)
        self.assertEqual(result['android_tree'], TREE)
        self.assertEqual(result['mode'], 'baseline')

    def test_baseline_rejects_changed_tree(self):
        def different(*args):
            return OTHER if args == ('rev-parse', f'{module.BASELINE}:android') else fake_git(*args)
        with patch.object(module, 'git', side_effect=different), self.assertRaises(ValueError):
            module.verify(self.env)

    def test_candidate_requires_exact_selected_sha(self):
        self.env.update(DIAG_MODE='candidate', DIAG_REQUEST_SHA=OTHER)
        with self.assertRaises(ValueError): module.verify(self.env)
        self.env['DIAG_REQUEST_SHA'] = HEAD
        self.assertEqual(module.verify(self.env)['mode'], 'candidate')

    def test_candidate_rejects_changed_trusted_harness(self):
        self.env.update(DIAG_MODE='candidate', DIAG_REQUEST_SHA=HEAD)
        def changed(*args):
            if len(args) == 2 and args[1].endswith('tools/android_package_path.sh'): return OTHER
            return fake_git(*args)
        with patch.object(module, 'git', side_effect=changed), self.assertRaises(ValueError):
            module.verify(self.env)

    def test_bad_or_wrong_checkout_rejected(self):
        for value in ('main', 'A' * 40, OTHER):
            with self.subTest(value=value), self.assertRaises(ValueError):
                module.verify({**self.env, 'DIAG_SOURCE_SHA': value})

    def test_pr_accepts_changed_base_with_identical_android_tree(self):
        self.env.update(DIAG_EVENT='pull_request', DIAG_BASE_SHA=OTHER,
                        DIAG_REPOSITORY='owner/repo', DIAG_HEAD_REPOSITORY='owner/repo')
        self.assertEqual(module.verify(self.env)['pr_base_android_tree'], TREE)
        def base_changed(*args):
            return OTHER if args == ('rev-parse', f'{OTHER}:android') else fake_git(*args)
        with patch.object(module, 'git', side_effect=base_changed), self.assertRaises(ValueError):
            module.verify(self.env)

    def test_pr_rejects_unrelated_base(self):
        self.env.update(DIAG_EVENT='pull_request', DIAG_BASE_SHA=OTHER,
                        DIAG_REPOSITORY='owner/repo', DIAG_HEAD_REPOSITORY='owner/repo')
        def unrelated(*args):
            if args[0] == 'merge-base': raise module.subprocess.CalledProcessError(1, args)
            return fake_git(*args)
        with patch.object(module, 'git', side_effect=unrelated), self.assertRaises(module.subprocess.CalledProcessError):
            module.verify(self.env)

    def test_pr_rejects_invalid_base_identity(self):
        self.env.update(DIAG_EVENT='pull_request', DIAG_BASE_SHA='main',
                        DIAG_REPOSITORY='owner/repo', DIAG_HEAD_REPOSITORY='owner/repo')
        with self.assertRaises(ValueError): module.verify(self.env)

    def test_pull_request_requires_same_repo(self):
        self.env.update(DIAG_EVENT='pull_request', DIAG_BASE_SHA=module.BASELINE, DIAG_REPOSITORY='owner/repo', DIAG_HEAD_REPOSITORY='other/repo')
        with self.assertRaises(ValueError): module.verify(self.env)
        self.env['DIAG_HEAD_REPOSITORY'] = 'owner/repo'
        self.assertEqual(module.verify(self.env)['mode'], 'baseline')


class DiagnosticGuardTest(unittest.TestCase):
    def test_guard_failure_preserves_incomplete_evidence(self):
        source_root = pathlib.Path(__file__).resolve().parents[2]
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            (root / 'tools').mkdir()
            (root / 'android').mkdir()
            for name in ('run_android_api35_diagnostic.sh', 'assert_disposable_android_emulator.sh'):
                shutil.copy(source_root / 'tools' / name, root / 'tools' / name)
            env = {**os.environ, 'CI': 'false', 'GITHUB_ACTIONS': 'false'}
            run = subprocess.run(['bash', str(root / 'tools/run_android_api35_diagnostic.sh')],
                                 env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10)
            self.assertNotEqual(run.returncode, 0)
            evidence = root / 'android/app/build/outputs/androidTest-results/api35-isolation'
            self.assertEqual((evidence / 'test-result.txt').read_text().strip(), 'NOT_RUN')
            self.assertEqual((evidence / 'diagnostic-result.txt').read_text().strip(), 'INCOMPLETE')


if __name__ == '__main__':
    unittest.main()
