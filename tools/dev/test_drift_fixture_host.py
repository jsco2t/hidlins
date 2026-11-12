"""Check shell completion drift fixtures with native host Make."""
import unittest
import test_drift_hashes as fixture

class DriftFixtureHostTests(unittest.TestCase):
    def test_unchanged_recipe_control_remains_valid(self):
        result = fixture.DriftRecipeTests().exercise('completions-check', 'noop')
        self.assertEqual(result.returncode, 0, result.stdout)

if __name__ == '__main__':
    unittest.main(verbosity=2)
