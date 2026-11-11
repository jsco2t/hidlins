#!/usr/bin/env python3
"""Real cache contracts through the iOS runner; SDK work is isolated, not claimed."""
import json
import os
from pathlib import Path
import shutil
import sys
import unittest

import test_vendor_cache as fixtures


CARGO_DRIVER = r'''
import json, os, pathlib, subprocess, sys
args = sys.argv[1:]
real = os.environ['IOS_CACHE_REAL_CARGO']
if args[0] == 'metadata':
    os.execv(real, [real, *args])
assert args[:6] == ['check', '--offline', '--locked', '-p', 'hidlins-api', '--no-default-features'], args
assert args[6] == '--target' and len(args) == 8, args
assert args[7] in ('aarch64-apple-ios', 'aarch64-apple-ios-sim'), args
root = pathlib.Path(os.environ['IOS_CACHE_ROOT'])
with (root / 'cargo-commands').open('a') as stream:
    stream.write(json.dumps(args) + '\n')
if os.environ.get('IOS_CACHE_FAIL_TARGET') == args[7]:
    sys.exit(19)
# Compile a real first-party fixture for this host, recording the iOS request.
# This isolates runner cache policy without pretending the fixture is an iOS app.
build = subprocess.run([real, 'build', '--offline', '--locked', '--message-format=json'],
                       capture_output=True, text=True)
sys.stderr.write(build.stderr)
if build.returncode:
    sys.exit(build.returncode)
messages = [json.loads(line) for line in build.stdout.splitlines() if line.startswith('{')]
deps = [item for item in messages if item.get('reason') == 'compiler-artifact'
        and any(name in item['package_id'] for name in ('fixture-value', 'fixture-macro'))]
assert deps, 'real dependency artifacts missing'
metadata = json.loads(subprocess.check_output([real, 'metadata', '--no-deps',
                      '--format-version', '1', '--offline', '--locked'], text=True))
output = pathlib.Path(metadata['target_directory']) / 'debug/cache-fixture'
value = subprocess.check_output([str(output)], text=True).strip()
with (root / 'results').open('a') as stream:
    stream.write(json.dumps({'value': value, 'fresh': all(item['fresh'] for item in deps),
                             'target': metadata['target_directory'],
                             'build': metadata['build_directory']}) + '\n')
'''

SDK_DRIVER = r'''
import json, os, pathlib, sys
with (pathlib.Path(os.environ['IOS_CACHE_ROOT']) / 'sdk-commands').open('a') as stream:
    stream.write(json.dumps([pathlib.Path(sys.argv[0]).name, *sys.argv[1:]]) + '\n')
sys.exit(int(os.environ.get('IOS_CACHE_XCODE_STATUS', '0')) if pathlib.Path(sys.argv[0]).name == 'xcodebuild' else 0)
'''


class IosRunnerCacheTests(unittest.TestCase):
    def fixture(self, mode):
        case = fixtures.CacheTests()
        case.setUp()
        self.addCleanup(case.doCleanups)
        runner = case.root / 'tools/ios-native/run.sh'
        runner.parent.mkdir(parents=True)
        shutil.copyfile(fixtures.ROOT / 'tools/ios-native/run.sh', runner)
        (case.root / 'app/ios').mkdir(parents=True)
        binary = case.root / 'bin'
        binary.mkdir()
        for name, source in [('cargo', CARGO_DRIVER), ('flutter', SDK_DRIVER), ('xcodebuild', SDK_DRIVER)]:
            file = binary / name
            file.write_text('#!' + sys.executable + '\n' + source)
            file.chmod(0o755)
        if mode == 'environment':
            case.env['CARGO_TARGET_DIR'] = str(case.root / 'environment outputs')
            case.env['CARGO_BUILD_BUILD_DIR'] = str(case.root / 'environment intermediates')
        else:
            with case.config.open('a') as stream:
                stream.write('[build]\ntarget-dir="configured outputs"\nbuild-dir="configured intermediates"\n')
        case.env.update(PATH=str(binary) + os.pathsep + case.env['PATH'],
                        IOS_CACHE_REAL_CARGO=fixtures.CARGO, IOS_CACHE_ROOT=str(case.root),
                        IOS_SIMULATOR_DESTINATION='platform=iOS Simulator,id=isolated-fixture')
        return case, ['bash', str(runner), 'check']

    def assert_cleaned(self, case, expected_status):
        commands = [json.loads(line) for line in (case.root / 'sdk-commands').read_text().splitlines()]
        calls = [args for args in commands if args[0] == 'xcodebuild']
        self.assertTrue(calls)
        for call in calls:
            derived = Path(call[call.index('-derivedDataPath') + 1])
            self.assertFalse(derived.exists(), 'RUNNER_MUST_CLEAN_OWNED_DERIVED_DATA')
            self.assertIn('CODE_SIGNING_ALLOWED=NO', call)
            self.assertIn('platform=iOS Simulator,id=isolated-fixture', call)
        self.assertIn(expected_status, (0, 65))

    def test_runner_invalidates_vendor_and_keeps_unchanged_roots_warm(self):
        for mode in ('environment', 'configuration'):
            with self.subTest(root=mode):
                case, command = self.fixture(mode)
                case.run_ok(command)
                case.run_ok(command)
                case.patch()
                case.run_ok(command)
                case.run_ok(command)
                records = [json.loads(line) for line in (case.root / 'results').read_text().splitlines()]
                self.assertEqual([item['value'] for item in records], ['A|A|A'] * 4 + ['B|B|B'] * 4,
                                 'IOS_RUNNER_MUST_COMPILE_CURRENT_VENDOR_BYTES')
                self.assertEqual([item['fresh'] for item in records],
                                 [False, True, True, True, False, True, True, True],
                                 'UNCHANGED_IOS_CHECKS_MUST_REUSE_WARM_DEPENDENCIES')
                prefix = 'environment' if mode == 'environment' else 'configured'
                self.assertEqual([item['target'] for item in records],
                                 [str(case.root / (prefix + ' outputs'))] * 8)
                self.assertEqual([item['build'] for item in records],
                                 [str(case.root / (prefix + ' intermediates'))] * 8)
                commands = [json.loads(line) for line in (case.root / 'cargo-commands').read_text().splitlines()]
                self.assertEqual([args[-1] for args in commands],
                                 ['aarch64-apple-ios', 'aarch64-apple-ios-sim'] * 4)
                self.assert_cleaned(case, 0)

    def test_cargo_failure_stops_before_sdk_and_xcode_failure_cleans_up(self):
        case, command = self.fixture('environment')
        case.env['IOS_CACHE_FAIL_TARGET'] = 'aarch64-apple-ios'
        result = case.execute(command)
        self.assertEqual(result.returncode, 19, result.stdout + result.stderr)
        self.assertFalse((case.root / 'sdk-commands').exists(), 'FAILED_CHECK_MUST_NOT_LAUNCH_XCODE')
        case.env.pop('IOS_CACHE_FAIL_TARGET')
        case.env['IOS_CACHE_XCODE_STATUS'] = '65'
        result = case.execute(command)
        self.assertEqual(result.returncode, 65, result.stdout + result.stderr)
        self.assert_cleaned(case, 65)


if __name__ == '__main__':
    unittest.main(verbosity=2)
