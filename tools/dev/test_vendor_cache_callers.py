#!/usr/bin/env python3
"""Exercise canonical caller adoption and clean recovery with actual Cargo."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

import test_vendor_cache as fixtures


SPY = r'''
import json,os,pathlib,subprocess,sys
args=sys.argv[1:];root=pathlib.Path(os.environ['CALLER_ROOT'])
if args[0]=='metadata':os.execv(os.environ['CALLER_CARGO'],[os.environ['CALLER_CARGO'],*args])
subprocess.run([os.environ['CALLER_CARGO'],'build','--offline','--locked'],check=True)
metadata=json.loads(subprocess.check_output([os.environ['CALLER_CARGO'],'metadata','--no-deps',
 '--format-version','1','--offline','--locked'],text=True))
value=subprocess.check_output([str(pathlib.Path(metadata['target_directory'])/'debug/cache-fixture')],text=True).strip()
with (root/'values').open('a') as stream:stream.write(value+'\n')
if args[0]=='bench':
 for name in ['bench_search_5k','bench_search_5k_wildcard','bench_search_5k_fuzzy',
              'bench_fuzzy_matcher_5k','bench_search_5k_unicode_substring','bench_search_5k_unicode_wildcard']:
  for stat in ['median','p99']:print(name+'_'+stat+'_ms=1.0')
'''

SNAPSHOT_DRIVER = r'''
import os, pathlib, sys
root = pathlib.Path(__file__).resolve().parents[2]
if os.environ.get('SNAPSHOT_CREATE'):
 (root/'crates/hidlins-tui/tests/snapshots/new.txt').write_text('new golden\n')
print(os.environ.get('SNAPSHOT_OUTPUT', ''))
sys.exit(int(os.environ.get('SNAPSHOT_EXIT', '0')))
'''


class SnapshotReportingTests(unittest.TestCase):
    def run_report(self, *, check=False, output='', status=0, create=False, existing=False):
        with tempfile.TemporaryDirectory(prefix='hidlins snapshot reporting ') as directory:
            root = Path(directory)
            tools = root / 'tools/dev'; tools.mkdir(parents=True)
            script = tools / 'update-snapshots.sh'
            shutil.copyfile(fixtures.ROOT / 'tools/dev/update-snapshots.sh', script)
            (tools / 'cargo_artifacts.py').write_text(SNAPSHOT_DRIVER)
            snapshots = root / 'crates/hidlins-tui/tests/snapshots'
            snapshots.mkdir(parents=True)
            if existing:
                (snapshots / 'keep.txt').write_text('existing golden\n')
            environment = dict(os.environ, SNAPSHOT_OUTPUT=output, SNAPSHOT_EXIT=str(status))
            environment.pop('SNAPSHOT_CREATE', None)
            if create:
                environment['SNAPSHOT_CREATE'] = '1'
            # macOS /bin/bash is Bash 3.2. Exercise the native shell even when
            # a newer Homebrew Bash precedes it on PATH.
            result = subprocess.run(['/bin/bash', str(script), *(['--check'] if check else [])],
                                    cwd=root, env=environment, text=True, capture_output=True,
                                    timeout=10)
            if existing:
                self.assertEqual((snapshots / 'keep.txt').read_text(), 'existing golden\n')
            if create:
                self.assertEqual((snapshots / 'new.txt').read_text(), 'new golden\n')
            return result

    def test_empty_snapshot_reports_succeed_in_native_bash(self):
        for check, message in [(False, 'No changes'), (True, 'present and up to date')]:
            with self.subTest(check=check):
                result = self.run_report(check=check)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertIn(message, result.stdout)

    def test_new_and_existing_snapshot_reports(self):
        for existing in (False, True):
            with self.subTest(existing=existing):
                result = self.run_report(create=True, existing=existing)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertIn('Created (1):\n  + new.txt', result.stdout)
                self.assertEqual('Updated (1):\n  ~ keep.txt' in result.stdout, existing)

    def test_missing_and_stale_reports_preserve_failed_status(self):
        for output, expected in [
            ('missing snapshot golden `new` at fixture', ['Missing goldens (1):', 'new.txt']),
            ('--- snapshot mismatch: old ---', ['Stale goldens (1):', 'old.txt']),
            ('missing snapshot golden `new` at fixture\n--- snapshot mismatch: old ---',
             ['Missing goldens (1):', 'Stale goldens (1):']),
        ]:
            with self.subTest(output=output):
                result = self.run_report(check=True, output=output, status=101)
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                for message in expected:
                    self.assertIn(message, result.stdout)

    def test_unrecognized_cargo_failure_is_not_reported_as_clean(self):
        result = self.run_report(check=True, output='fixture compile failure', status=101)
        self.assertEqual(result.returncode, 1)
        self.assertIn('fixture compile failure', result.stderr)
        self.assertIn('unrecognized reason', result.stderr)


class CallerTests(unittest.TestCase):
    def fixture(self):
        case = fixtures.CacheTests()
        case.setUp(); self.addCleanup(case.doCleanups)
        return case

    def test_clean_recovers_with_missing_or_indirect_vendor_and_retains_lock(self):
        for broken in ('missing', 'symlink'):
            with self.subTest(input=broken):
                case = self.fixture(); case.make_build()
                target = Path(case.metadata()['target_directory'])
                lock = target.parent / ('.' + target.name + '.hidlins-vendor.lock')
                self.assertTrue(lock.is_file())
                inode = lock.stat().st_ino
                vendor = case.root / 'vendor'
                if broken == 'missing': shutil.rmtree(vendor)
                else:
                    old = case.root / 'saved-vendor'; vendor.rename(old)
                    vendor.symlink_to(old, target_is_directory=True)
                case.run_ok(['make', '--no-print-directory', 'clean'])
                self.assertFalse((target / 'debug/cache-fixture').exists())
                self.assertEqual(lock.stat().st_ino, inode, 'CLEAN_MUST_KEEP_THE_SHARED_LOCK_DOMAIN')

    def test_clean_waits_for_in_progress_compile_lock(self):
        import fcntl
        case = self.fixture(); case.make_build()
        target = Path(case.metadata()['target_directory'])
        lock = target.parent / ('.' + target.name + '.hidlins-vendor.lock')
        with lock.open('r+b') as stream:
            fcntl.lockf(stream, fcntl.LOCK_EX)
            process = subprocess.Popen(['make', '--no-print-directory', 'clean'], cwd=case.root,
                                       env=case.env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            try:
                time.sleep(.3)
                self.assertIsNone(process.poll(), 'CLEAN_MUST_WAIT_FOR_COMPILATION')
                self.assertTrue((target / 'debug/cache-fixture').exists())
            finally:
                fcntl.lockf(stream, fcntl.LOCK_UN)
                output, errors = process.communicate(timeout=30)
        self.assertEqual(process.returncode, 0, output + errors)
        self.assertFalse((target / 'debug/cache-fixture').exists())

    def test_snapshot_and_benchmark_public_callers(self):
        for target in ('snapshots-check', 'test-update-snapshots', 'bench-search-gate',
                       'bench-search-gate-ci'):
            for mode in ('environment', 'configuration'):
                with self.subTest(target=target, root=mode):
                    case = self.fixture()
                    if mode == 'environment':
                        case.env['CARGO_TARGET_DIR'] = str(case.root / 'environment outputs')
                        case.env['CARGO_BUILD_BUILD_DIR'] = str(case.root / 'environment intermediates')
                    else:
                        with case.config.open('a') as stream:
                            stream.write('[build]\ntarget-dir="configured outputs"\nbuild-dir="configured intermediates"\n')
                    for name in ('tools/dev/update-snapshots.sh', 'tools/bench/bench_search_gate.sh'):
                        path = case.root / name; path.parent.mkdir(parents=True, exist_ok=True)
                        shutil.copyfile(fixtures.ROOT / name, path); shutil.copymode(fixtures.ROOT / name, path)
                    (case.root / 'crates/hidlins-tui/tests/snapshots').mkdir(parents=True)
                    binary = case.root / 'bin'; binary.mkdir()
                    cargo = binary / 'cargo'; cargo.write_text('#!' + sys.executable + '\n' + SPY); cargo.chmod(0o755)
                    case.env.update(PATH=str(binary) + os.pathsep + case.env['PATH'],
                                    CALLER_CARGO=fixtures.CARGO, CALLER_ROOT=str(case.root))
                    case.run_ok(['make', '--no-print-directory', target])
                    self.assertEqual((case.root / 'values').read_text().splitlines(), ['A|A|A'])
                    case.patch()
                    case.run_ok(['make', '--no-print-directory', target])
                    self.assertEqual((case.root / 'values').read_text().splitlines(), ['A|A|A', 'B|B|B'],
                                     'CANONICAL_CALLER_MUST_INVALIDATE_VENDOR_BYTES')


if __name__ == '__main__':
    unittest.main(verbosity=2)
