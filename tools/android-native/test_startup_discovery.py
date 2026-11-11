"""Fresh-permission fixture controls; no guest, SDK process or network calls."""
import types
import unittest
import release_jni
import startup_discovery

NEARBY = release_jni.OFFLINE_PERMISSIONS[0]
SDK_PERMISSIONS = f'group: android.permission-group.NEARBY_DEVICES\n  permission: {NEARBY}\n'

def package(granted='false', flags=''):
    return f'    {NEARBY}: granted={granted}, flags=[{flags}]\n'

class FreshPermissionFixtureTests(unittest.TestCase):
    def policy(self):
        policy = startup_discovery.FreshPermissionPolicy(release_jni.offline_permissions)
        self.assertEqual(policy.declared(SDK_PERMISSIONS, 36), [])
        self.assertIn(NEARBY, policy.permissions)
        return policy

    def test_sdk_declaration_is_validated_without_pre_setting_denial(self):
        self.policy()

    def test_missing_sdk_nearby_permission_is_not_silently_ignored(self):
        policy = startup_discovery.FreshPermissionPolicy(release_jni.offline_permissions)
        with self.assertRaises(RuntimeError):
            policy.declared('group: empty\n', 36)

    def test_fresh_not_granted_and_not_user_decided_is_accepted(self):
        policy = self.policy()
        policy.verify(package(flags='USER_SENSITIVE_WHEN_GRANTED|USER_SENSITIVE_WHEN_DENIED'))
        self.assertTrue(policy.verified)

    def test_pregranted_permission_cannot_mask_startup_prompt(self):
        with self.assertRaises(RuntimeError):
            self.policy().verify(package(granted='true'))

    def test_prior_user_denial_cannot_mask_startup_prompt(self):
        for flags in ['USER_SET', 'USER_FIXED', 'USER_SET|USER_FIXED']:
            with self.subTest(flags=flags), self.assertRaises(RuntimeError):
                self.policy().verify(package(flags=flags))

    def test_missing_app_permission_state_is_rejected(self):
        with self.assertRaises(RuntimeError):
            self.policy().verify('no runtime permission block\n')

    def test_api29_does_not_require_unsupported_runtime_permissions(self):
        policy = startup_discovery.FreshPermissionPolicy(release_jni.offline_permissions)
        self.assertEqual(policy.declared('', 29), [])
        policy.verify('')
        self.assertTrue(policy.verified)

    def test_package_state_cannot_be_claimed_before_sdk_enumeration(self):
        policy = startup_discovery.FreshPermissionPolicy(release_jni.offline_permissions)
        with self.assertRaises(RuntimeError):
            policy.verify(package())

    def test_fixture_scope_restores_original_driver_even_on_failure(self):
        original = release_jni.offline_permissions
        driver = types.SimpleNamespace(offline_permissions=original)
        with self.assertRaisesRegex(ValueError, 'fixture-stop'):
            with startup_discovery.fresh_permissions(driver) as policy:
                self.assertEqual(driver.offline_permissions(SDK_PERMISSIONS, 36), [])
                self.assertIn(NEARBY, policy.permissions)
                raise ValueError('fixture-stop')
        self.assertIs(driver.offline_permissions, original)

if __name__ == '__main__':
    unittest.main(verbosity=2)
