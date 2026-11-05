import importlib.util
import pathlib
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
    if args == ('rev-parse', 'HEAD'):
        return HEAD
    if args == ('rev-parse', f'{HEAD}:android'):
        return TREE
    if args == ('rev-parse', f'{module.BASELINE}:android'):
        return TREE
    if args[:3] == ('diff', '--exit-code', module.HARNESS): return ''
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
            if args[0] == 'diff': raise module.subprocess.CalledProcessError(1, args)
            return fake_git(*args)
        with patch.object(module, 'git', side_effect=changed), self.assertRaises(module.subprocess.CalledProcessError):
            module.verify(self.env)

    def test_bad_or_wrong_checkout_rejected(self):
        for value in ('main', 'A' * 40, OTHER):
            with self.subTest(value=value), self.assertRaises(ValueError):
                module.verify({**self.env, 'DIAG_SOURCE_SHA': value})

    def test_pull_request_requires_same_repo(self):
        self.env.update(DIAG_EVENT='pull_request', DIAG_BASE_SHA=module.BASELINE, DIAG_REPOSITORY='owner/repo', DIAG_HEAD_REPOSITORY='other/repo')
        with self.assertRaises(ValueError): module.verify(self.env)
        self.env['DIAG_HEAD_REPOSITORY'] = 'owner/repo'
        self.assertEqual(module.verify(self.env)['mode'], 'baseline')


if __name__ == '__main__':
    unittest.main()
