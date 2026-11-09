#!/usr/bin/env python3
"""RustBuilder metadata and compilation must honor the same Cargo configuration."""
import json
import os
from pathlib import Path
import shutil
import sys
import unittest

import test_vendor_cache as fixtures


class CargokitConfigTests(unittest.TestCase):
    def test_metadata_uses_the_existing_home_rustup_resolver_without_path_entry(self):
        case = fixtures.CacheTests(); case.setUp()
        self.addCleanup(case.doCleanups)
        command = case.dart_package()
        binary = case.root / 'minimal bin'; binary.mkdir()
        (binary / 'python3').symlink_to(sys.executable)
        paths = [p for p in case.env['PATH'].split(os.pathsep)
                 if p and not (Path(p) / 'rustup').is_file()]
        environment = dict(case.env, PATH=os.pathsep.join([str(binary), *paths]))
        self.assertIsNone(shutil.which('rustup', path=environment['PATH']))
        self.assertTrue((Path(environment['HOME']) / '.cargo/bin/rustup').is_file(),
                        'INSTALLED_HOME_TOOLCHAIN_REQUIRED')
        case.run_ok(command, env=environment)
        output = case.root / 'cargokit-cache' / fixtures.HOST / 'debug/cache-fixture'
        self.assertEqual(case.run_ok([str(output)]).stdout.strip(), 'A|A|A')

    def test_inline_and_file_configs_guard_the_real_intermediate_root(self):
        for form in ('inline', 'equals', 'file'):
            with self.subTest(form=form):
                case = fixtures.CacheTests(); case.setUp()
                self.addCleanup(case.doCleanups)
                command = case.dart_package()
                root = case.root / 'configured intermediate files'
                value = 'build.build-dir=' + json.dumps(str(root))
                flags = ['--config', value] if form == 'inline' else ['--config=' + value]
                if form == 'file':
                    config = case.root / 'extra cargo config.toml'
                    config.write_text('[build]\nbuild-dir=' + json.dumps(str(root)) + '\n')
                    flags = ['--config', str(config)]
                driver = case.root / 'driver.dart'
                original = driver.read_text()
                self.assertIn('CargokitCrateOptions()', original)
                driver.write_text(original.replace('CargokitCrateOptions()',
                    'CargokitCrateOptions(cargo: {BuildConfiguration.debug: CargoBuildOptions('
                    'toolchain: Toolchain.stable, flags: ' + json.dumps(flags) + ')})'))
                output = case.root / 'cargokit-cache' / fixtures.HOST / 'debug/cache-fixture'
                case.run_ok(command)
                self.assertEqual(case.run_ok([str(output)]).stdout.strip(), 'A|A|A')
                self.assertTrue(any(root.rglob('*.rlib')), 'REAL_INTERMEDIATE_ROOT_MISSING')
                case.patch(); case.run_ok(command)
                self.assertEqual(case.run_ok([str(output)]).stdout.strip(), 'B|B|B',
                                 'CARGOKIT_CONFIGURED_INTERMEDIATES_MUST_FOLLOW_VENDOR_BYTES')
                self.assertTrue((root / '.hidlins-vendor-generation').is_file())
                stamps = {str(p.relative_to(root)): p.stat().st_mtime_ns for p in root.rglob('*.rlib')}
                case.run_ok(command)
                self.assertEqual(stamps, {str(p.relative_to(root)): p.stat().st_mtime_ns for p in root.rglob('*.rlib')},
                                 'UNCHANGED_CONFIGURED_DEPENDENCIES_MUST_STAY_WARM')


if __name__ == '__main__':
    unittest.main(verbosity=2)
