#!/usr/bin/env python3
"""Bind API 35 diagnostics to an immutable source and label baseline vs candidate."""
import os
import re
import subprocess
import sys

BASELINE = '334da4bf8855e1b7fefc019a3b397aff506c9913'
HARNESS = '2c4327399e5adfec8e106df50a5c5bc6e11c50b8'
TRUSTED = ('tools/run_android_api35_diagnostic.sh', 'tools/assert_disposable_android_emulator.sh', 'tools/prepare_isolated_instrumentation_target.sh', 'tools/android_package_path.sh', 'tools/wait_for_android_default_ime.sh', 'tools/verify_android_instrumentation.py')
SHA = re.compile(r'^[0-9a-f]{40}$')


def git(*args):
    return subprocess.check_output(['git', *args], text=True, stderr=subprocess.PIPE).strip()


def verify(env):
    event = env.get('DIAG_EVENT', '')
    mode = 'baseline' if event == 'pull_request' else env.get('DIAG_MODE', '')
    expected = env.get('DIAG_SOURCE_SHA', '')
    if event not in ('pull_request', 'workflow_dispatch') or mode not in ('baseline', 'candidate'):
        raise ValueError('unsupported diagnostic event or mode')
    if not SHA.fullmatch(expected):
        raise ValueError('source must be an explicit full lowercase commit SHA')
    head = git('rev-parse', 'HEAD')
    if head != expected:
        raise ValueError('checkout does not equal requested source SHA')
    if event == 'workflow_dispatch' and mode == 'candidate' and env.get('DIAG_REQUEST_SHA', '') != head:
        raise ValueError('candidate input SHA must exactly match the selected workflow ref')
    if event == 'pull_request' and env.get('DIAG_BASE_SHA', '') != BASELINE:
        raise ValueError('historical PR baseline does not match')
    if event == 'pull_request' and env.get('DIAG_REPOSITORY', '') != env.get('DIAG_HEAD_REPOSITORY', ''):
        raise ValueError('candidate must be from the same repository')
    tree = git('rev-parse', f'{head}:android')
    baseline_tree = git('rev-parse', f'{BASELINE}:android')
    if mode == 'candidate':
        git('diff', '--exit-code', HARNESS, head, '--', *TRUSTED)
    if mode == 'baseline' and tree != baseline_tree:
        raise ValueError('baseline mode requires the historical Android tree')
    return {'mode': mode, 'source_sha': head, 'android_tree': tree, 'baseline_sha': BASELINE,
            'baseline_android_tree': baseline_tree, 'harness_reference_sha': HARNESS,
            'harness_match': 'PASS' if mode == 'candidate' else 'NOT_CHECKED', 'event_sha': env.get('DIAG_EVENT_SHA', ''),
            'run_id': env.get('GITHUB_RUN_ID', ''), 'run_attempt': env.get('GITHUB_RUN_ATTEMPT', '')}


def main():
    try:
        for key, value in verify(os.environ).items():
            print(f'{key}={value}')
    except (ValueError, subprocess.CalledProcessError, OSError) as exc:
        print(f'FAIL: diagnostic source identity: {exc}', file=sys.stderr)
        return 1
    return 0

if __name__ == '__main__':
    sys.exit(main())
