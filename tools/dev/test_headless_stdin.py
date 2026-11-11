#!/usr/bin/env python3
"""Require a real empty pipe for canonical noninteractive Rust test commands."""
import errno
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
PROBE = '''import errno, json, os, stat, sys, termios
args = sys.argv[1:]
record = {'args': args, 'fifo': stat.S_ISFIFO(os.fstat(0).st_mode)}
if args[0] == 'test':
    try:
        termios.tcgetattr(0)
        record['errno'] = None
    except termios.error as error:
        record['errno'] = error.args[0]
    record['empty'] = sys.stdin.buffer.read(1) == b''
with open(os.environ['HIDLINS_HEADLESS_STDIN_LOG'], 'a') as output:
    output.write(json.dumps(record) + '\\n')
if args[0] == 'test':
    if not record['fifo'] or record['errno'] != errno.ENOTTY or not record['empty']:
        print('ordinary test input must be an empty pipe with ENOTTY')
        raise SystemExit(74)
    raise SystemExit(int(os.environ.get('HIDLINS_HEADLESS_STDIN_EXIT', '0')))
'''


class HeadlessStdinTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='hidlins headless stdin ')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.probe = self.root / 'cargo probe.py'
        self.probe.write_text(PROBE)
        self.log = self.root / 'calls.jsonl'

    def environment(self, failure=False):
        environment = os.environ.copy()
        for key in ('MAKEFLAGS', 'MFLAGS', 'MAKELEVEL', 'MAKEFILES',
                    'HIDLINS_HEADLESS_STDIN_EXIT', 'CARGO_TEST_FLAGS'):
            environment.pop(key, None)
        environment['HIDLINS_HEADLESS_STDIN_LOG'] = str(self.log)
        if failure:
            environment['HIDLINS_HEADLESS_STDIN_EXIT'] = '79'
        return environment

    def dispatch(self, target, failure=False):
        self.log.unlink(missing_ok=True)
        command = shlex.join([sys.executable, str(self.probe)])
        result = subprocess.run(
            ['make', '--no-print-directory', '-f', str(ROOT / 'Makefile'), target,
             'CARGO_COMPILE=' + command, 'MAKE=' + command,
             'CARGO_FLAGS=--workspace --offline --locked'],
            cwd=ROOT, env=self.environment(failure), stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
            timeout=15)
        calls = [json.loads(line) for line in self.log.read_text().splitlines()]
        return result, calls

    def test_probe_positive_control_accepts_a_real_empty_pipe(self):
        result = subprocess.run(
            [sys.executable, str(self.probe), 'test'], input='', text=True,
            env=self.environment(), capture_output=True, timeout=5)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_default_tests_receive_empty_pipe_and_preserve_flags(self):
        result, calls = self.dispatch('test')
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0]['args'],
                         ['test', '--features', 'hidlins-security/test-binaries',
                          '--workspace', '--offline', '--locked'])
        self.assertTrue(calls[0]['fifo'])
        self.assertEqual(calls[0]['errno'], errno.ENOTTY)
        self.assertTrue(calls[0]['empty'])

    def test_ignored_tests_receive_pipe_and_keep_build_and_render(self):
        result, calls = self.dispatch('test-ignored')
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual([call['args'][0] for call in calls],
                         ['build', 'test', 'bench-tui-render'])
        self.assertFalse(calls[0]['fifo'], 'build input is unchanged')
        ordinary = calls[1]
        self.assertTrue(ordinary['fifo'])
        self.assertTrue(ordinary['empty'])
        self.assertIn('--ignored', ordinary['args'])
        self.assertIn('--test-threads=1', ordinary['args'])
        self.assertIn('test-binaries', ordinary['args'])

    def test_cargo_failure_propagates_and_stops_before_render(self):
        for target in ('test', 'test-ignored'):
            with self.subTest(target=target):
                result, calls = self.dispatch(target, failure=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('Error 79', result.stdout)
                self.assertFalse(any(call['args'][0] == 'bench-tui-render'
                                     for call in calls))


if __name__ == '__main__':
    unittest.main(verbosity=2)
