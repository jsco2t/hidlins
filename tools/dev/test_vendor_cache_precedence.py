#!/usr/bin/env python3
"""Explicit Cargo cache roots must beat inherited output-directory settings."""
import json
import os
from pathlib import Path
import sys
import unittest

import test_vendor_cache as fixtures


class PrecedenceTests(unittest.TestCase):
    def fixture(self):
        case = fixtures.CacheTests(); case.setUp()
        self.addCleanup(case.doCleanups)
        case.env['CARGO_TARGET_DIR'] = str(case.root / 'inherited outputs')
        return case

    def test_guarded_cargo_cli_target_beats_environment_and_configuration(self):
        for configuration in (False, True):
            for equals in (False, True):
                with self.subTest(config=configuration, equals=equals):
                    case = self.fixture()
                    if configuration:
                        with case.config.open('a') as stream:
                            stream.write('[build]\ntarget-dir="configured outputs"\nbuild-dir="configured intermediates"\n')
                    target = case.root / 'explicit outputs'
                    flag = ['--target-dir=' + str(target)] if equals else ['--target-dir', str(target)]
                    command = case.guarded_cargo('build', '--offline', '--locked', *flag)
                    case.run_ok(command)
                    binary = target / 'debug/cache-fixture'
                    self.assertEqual(case.run_ok([str(binary)]).stdout.strip(), 'A|A|A')
                    case.patch(); case.run_ok(command)
                    self.assertEqual(case.run_ok([str(binary)]).stdout.strip(), 'B|B|B',
                                     'EXPLICIT_TARGET_MUST_BE_INVALIDATED')

    def test_cargokit_descriptor_matches_real_explicit_cargo_root(self):
        case = self.fixture()
        target = case.root / 'explicit cargokit outputs'
        with case.config.open('a') as stream:
            stream.write('[build]\ntarget-dir="configured outputs"\nbuild-dir="{workspace-root}/configured intermediates"\n')
        # Query Cargo using the effective explicit root as the environment:
        # --target-dir on actual builds has that highest-priority meaning.
        environment = dict(case.env, CARGO_TARGET_DIR=str(target))
        expected = json.loads(case.run_ok([fixtures.CARGO, 'metadata', '--no-deps',
                            '--format-version', '1', '--offline', '--locked'], env=environment).stdout)
        result = case.run_ok([sys.executable, str(case.root / 'tools/dev/vendor_cache.py'),
                             'describe', '--workspace', str(case.root), '--target-dir', str(target),
                             '--', fixtures.CARGO])
        actual = json.loads(result.stdout)
        for key in ('target_directory', 'build_directory'):
            self.assertEqual(actual[key], expected[key], 'DESCRIPTOR_MUST_MATCH_EXPLICIT_BUILD_ROOTS')
        roots = {Path(expected[key]) for key in ('target_directory', 'build_directory')}
        self.assertEqual(set(actual['lock_paths']),
                         {str(root.parent / ('.' + root.name + '.hidlins-vendor.lock')) for root in roots})


if __name__ == '__main__':
    unittest.main(verbosity=2)
