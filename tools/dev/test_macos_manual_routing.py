#!/usr/bin/env python3
"""Verify real Make dispatch separates human-triggered observations from CI."""
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
MANUAL = {'sleep_transition_locks_helper', 'screen_lock_locks_helper'}
PROBE = '''import json, os, sys
with open(os.environ['HIDLINS_MANUAL_ROUTING_LOG'], 'a') as output:
    output.write(json.dumps(sys.argv[1:]) + '\\n')
'''


class MacManualRoutingTests(unittest.TestCase):
    def dispatch(self, target, host):
        with tempfile.TemporaryDirectory(prefix='hidlins manual routing ') as directory:
            directory = Path(directory)
            probe = directory / 'command probe.py'
            log = directory / 'calls.jsonl'
            probe.write_text(PROBE)
            environment = os.environ.copy()
            for key in ('MAKEFLAGS', 'MFLAGS'):
                environment.pop(key, None)
            environment['HIDLINS_MANUAL_ROUTING_LOG'] = str(log)
            command = shlex.join([sys.executable, str(probe)])
            result = subprocess.run(
                ['make', '--no-print-directory', '-f', str(ROOT / 'Makefile'),
                 target, 'UNAME_S=' + host, 'CARGO_COMPILE=' + command,
                 'MAKE=' + command], cwd=ROOT, env=environment,
                stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT, text=True, timeout=15)
            self.assertEqual(result.returncode, 0, result.stdout)
            return [json.loads(line) for line in log.read_text().splitlines()]

    def test_darwin_headless_sweep_excludes_only_interactive_cases(self):
        calls = self.dispatch('test-ignored', 'Darwin')
        ordinary = next(args for args in calls if args[0] == 'test')
        skipped = {ordinary[i + 1] for i, arg in enumerate(ordinary[:-1]) if arg == '--skip'}
        self.assertEqual(skipped, MANUAL | {
            'framework_memory_regressions::render_workload_performance_baseline'})
        self.assertIn('--ignored', ordinary)
        self.assertIn('--test-threads=1', ordinary)
        self.assertIn(['bench-tui-render'], calls)

    def test_linux_retains_all_existing_ignored_cases(self):
        calls = self.dispatch('test-ignored', 'Linux')
        ordinary = next(args for args in calls if args[0] == 'test')
        skipped = {ordinary[i + 1] for i, arg in enumerate(ordinary[:-1]) if arg == '--skip'}
        self.assertEqual(skipped, {
            'framework_memory_regressions::render_workload_performance_baseline'})
        self.assertIn('--test-threads=1', ordinary)

    def test_explicit_darwin_command_retains_real_observers_and_manual_file(self):
        calls = self.dispatch('test-os-events', 'Darwin')
        self.assertEqual(len(calls), 2)
        self.assertIn('os_events::macos', calls[0])
        self.assertIn('--lib', calls[0])
        self.assertIn('us_052_post_mvp_iokit', calls[1])
        self.assertIn('test-binaries,iokit', calls[1])
        for args in calls:
            self.assertIn('--ignored', args)
            self.assertIn('--test-threads=1', args)
            self.assertNotIn('--skip', args)

    def test_manual_names_match_exact_existing_test_functions(self):
        import re
        source = (ROOT / 'crates/hidlins-security/tests/us_052_post_mvp_iokit.rs').read_text()
        ignored = set(re.findall(r'#\[ignore\s*=\s*"manual:[^\n]+\]\s*fn\s+(\w+)\(', source))
        self.assertEqual(ignored, MANUAL)


if __name__ == '__main__':
    unittest.main(verbosity=2)
