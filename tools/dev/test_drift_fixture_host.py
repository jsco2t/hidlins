#!/usr/bin/env python3
"""Run the existing isolated drift fixture through the native host Make."""
import unittest

import test_drift_hashes as fixture


class DriftFixtureHostTests(unittest.TestCase):
    def test_pub_noop_reaches_the_real_comparison_without_gnu_tools(self):
        result = fixture.DriftRecipeTests().exercise('pub-vendor-check', 'noop')
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn('vendored pub caches match the lockfile', result.stdout)

    def test_pub_drift_fails_at_the_intended_integrity_stage(self):
        result = fixture.DriftRecipeTests().exercise('pub-vendor-check', 'change')
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn('pub vendor cache drifted', result.stdout)

    def test_other_unchanged_recipe_controls_remain_valid(self):
        for target in ('api-gen-check', 'completions-check'):
            with self.subTest(target=target):
                result = fixture.DriftRecipeTests().exercise(target, 'noop')
                self.assertEqual(result.returncode, 0, result.stdout)


if __name__ == '__main__':
    unittest.main(verbosity=2)
