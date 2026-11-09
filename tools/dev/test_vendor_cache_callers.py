#!/usr/bin/env python3
"""Exercise canonical caller adoption and clean recovery with actual Cargo."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
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

    def test_snapshot_benchmark_and_frozen_startup_public_callers(self):
        for target in ('snapshots-check', 'test-update-snapshots', 'bench-search-gate',
                       'bench-search-gate-ci', 'app-test-startup-discovery'):
            for mode in ('environment', 'configuration'):
                with self.subTest(target=target, root=mode):
                    case = self.fixture()
                    if mode == 'environment':
                        case.env['CARGO_TARGET_DIR'] = str(case.root / ('environment-outputs' if target == 'app-test-startup-discovery' else 'environment outputs'))
                        case.env['CARGO_BUILD_BUILD_DIR'] = str(case.root / 'environment intermediates')
                    else:
                        with case.config.open('a') as stream:
                            stream.write('[build]\ntarget-dir="' + ('configured-outputs' if target == 'app-test-startup-discovery' else 'configured outputs') + '"\nbuild-dir="configured intermediates"\n')
                    for name in ('tools/dev/update-snapshots.sh', 'tools/bench/bench_search_gate.sh',
                                 'tools/android-native/startup-discovery-tests.mk'):
                        path = case.root / name; path.parent.mkdir(parents=True, exist_ok=True)
                        shutil.copyfile(fixtures.ROOT / name, path); shutil.copymode(fixtures.ROOT / name, path)
                    (case.root / 'crates/hidlins-tui/tests/snapshots').mkdir(parents=True)
                    (case.root / 'app').mkdir()
                    for name in ('test_startup_discovery.py', 'startup_discovery.py'):
                        (case.root / 'tools/android-native' / name).write_text('print("CALLER_STUB_ONLY")\n')
                    # Isolate Pub/native effects; the actual copied Make target
                    # and frozen host recipe still dispatch compilation normally.
                    with (case.root / 'Makefile').open('a') as stream:
                        stream.write('\napp-deps:\n\t@true\nandroid-gradle-prepare:\n\t@true\n')
                    binary = case.root / 'bin'; binary.mkdir()
                    cargo = binary / 'cargo'; cargo.write_text('#!' + sys.executable + '\n' + SPY); cargo.chmod(0o755)
                    flutter = binary / 'flutter'; flutter.write_text('#!/bin/sh\nexit 0\n'); flutter.chmod(0o755)
                    # F6's separately approved library-path fixture correction
                    # is applied during earlier revalidation; isolate cache
                    # adoption here without depending on that pending path edit.
                    case.env['HIDLINS_API_LIB'] = str(case.root / 'libfixture.so')
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
