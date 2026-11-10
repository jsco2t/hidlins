#!/usr/bin/env python3
"""The real cache fixture must compile for the installed host on Mac and Linux."""
import unittest

import test_vendor_cache as fixtures


class HostFixtureTests(unittest.TestCase):
    def test_dispatch_compiles_and_reuses_the_actual_host(self):
        case = fixtures.CacheTests()
        case.setUp()
        self.addCleanup(case.doCleanups)
        command = case.dart_package()
        self.assertEqual(command[-1], fixtures.HOST,
                         'CACHE_FIXTURE_MUST_SELECT_RUST_HOST_NOT_LINUX_FLUTTER_ALIAS')
        case.run_ok(command)
        output = case.root / 'cargokit-cache' / fixtures.HOST / 'debug/cache-fixture'
        self.assertEqual(case.run_ok([str(output)]).stdout.strip(), 'A|A|A')
        case.patch()
        case.run_ok(command)
        self.assertEqual(case.run_ok([str(output)]).stdout.strip(), 'B|B|B')
        artifacts = output.parent / 'deps'
        stamps = {str(p): p.stat().st_mtime_ns for p in artifacts.glob('*.rlib')}
        self.assertTrue(stamps, 'REAL_HOST_DEPENDENCY_ARTIFACTS_REQUIRED')
        case.run_ok(command)
        self.assertEqual(stamps, {str(p): p.stat().st_mtime_ns for p in artifacts.glob('*.rlib')},
                         'UNCHANGED_HOST_FIXTURE_MUST_REMAIN_WARM')


if __name__ == '__main__':
    unittest.main(verbosity=2)
