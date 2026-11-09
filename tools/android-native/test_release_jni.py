#!/usr/bin/env python3
"""Negative controls for the shipping JNI inspection and runtime receipts."""
import importlib.util
import contextlib
import io
from pathlib import Path
import tempfile
import unittest
import zipfile
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("release_jni", Path(__file__).with_name("release_jni.py"))
PROBE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROBE)


class ReleaseClipboardControls(unittest.TestCase):
    def test_release_runtime_rejects_missing_or_empty_flutter_payload(self):
        with tempfile.TemporaryDirectory() as temporary:
            apk = Path(temporary) / "release.apk"
            required = ("lib/arm64-v8a/libapp.so", "lib/x86_64/libapp.so",
                        "assets/flutter_assets/AssetManifest.bin")
            for missing in (None,) + required:
                for empty in (False, True):
                    with zipfile.ZipFile(apk, "w") as archive:
                        for name in required:
                            if name != missing or empty:
                                archive.writestr(name, b"" if name == missing else b"payload")
                    if missing is None:
                        PROBE.require_release_payload(apk)
                    else:
                        with self.subTest(missing=missing, empty=empty), self.assertRaises(RuntimeError):
                            PROBE.require_release_payload(apk)

    def test_canonical_offline_build_policy_is_owned_and_cleaned_on_failure(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(PROBE, "ROOT", Path(temporary)):
            policy = Path(temporary) / ".gradle/android/init.d/hidlins-release-jni-offline.gradle"
            with patch.object(PROBE.subprocess, "Popen", side_effect=RuntimeError("build failed")):
                with self.assertRaises(RuntimeError):
                    PROBE.build_shipping({})
            self.assertFalse(policy.exists())
            policy.write_text("existing unrelated policy")
            with patch.object(PROBE.subprocess, "Popen") as build, self.assertRaises(FileExistsError):
                PROBE.build_shipping({})
            build.assert_not_called()
            self.assertEqual(policy.read_text(), "existing unrelated policy")

    def test_offline_inventory_accepts_grouped_permissions_and_rejects_missing_required(self):
        grouped = ("Dangerous Permissions:\n\ngroup:android.permission-group.NEARBY_DEVICES\n"
                   "  permission:android.permission.NEARBY_WIFI_DEVICES\n"
                   "  permission: android.permission.ACCESS_LOCAL_NETWORK\n")
        self.assertEqual(PROBE.offline_permissions(grouped, 36), list(PROBE.OFFLINE_PERMISSIONS))
        self.assertEqual(PROBE.offline_permissions("Dangerous Permissions:\n", 29), [])
        for api in (33, 36):
            with self.subTest(api=api), self.assertRaises(RuntimeError):
                PROBE.offline_permissions("Dangerous Permissions:\n", api)

    def test_offline_denial_requires_denied_permission_and_both_user_flags(self):
        denied = ("  android.permission.NEARBY_WIFI_DEVICES: granted=false, "
                  "flags=[ USER_SET|USER_FIXED|USER_SENSITIVE_WHEN_DENIED]\n")
        permission = [PROBE.OFFLINE_PERMISSIONS[0]]
        PROBE.verify_offline_denial(denied, permission)
        for bad in (denied.replace("false", "true"), denied.replace("USER_FIXED|", ""),
                    denied.replace("USER_SET|", ""), denied.replace("NEARBY_WIFI_DEVICES", "OTHER"), ""):
            with self.subTest(bad=bad), self.assertRaises(RuntimeError):
                PROBE.verify_offline_denial(bad, permission)

    def test_failed_application_output_is_not_logged(self):
        completed = PROBE.subprocess.CompletedProcess(["adb"], 1, "DO_NOT_LOG_APPLICATION_CONTENT")
        with patch.object(PROBE.subprocess, "run", return_value=completed):
            with self.assertRaises(RuntimeError) as failure:
                PROBE.checked(["adb", "shell", "am", "instrument"], {}, diagnostic_output=False)
        self.assertNotIn("DO_NOT_LOG_APPLICATION_CONTENT", str(failure.exception))
        self.assertIn("output suppressed", str(failure.exception))

    def test_canonical_check_cannot_select_controls_or_reduce_coverage(self):
        self.assertEqual(PROBE.parse_arguments(["test"]).mode, "full")
        for flags in (["--variant", "debug"], ["--api", "29"], ["--mode", "abi"],
                      ["--apk", "/tmp/control.apk"], ["--mapping", "/tmp/control.txt"]):
            with self.subTest(flags=flags), contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                PROBE.parse_arguments(["test"] + flags)

    def test_mapping_uses_current_owner_and_preserves_missing_ttl_red(self):
        mapping = """app.hidlins.MainActivity -> app.hidlins.MainActivity:
    app.hidlins.HidlinsPlatformServices platformServices -> g
app.hidlins.HidlinsPlatformServices -> c9:
    app.hidlins.ClipboardController clipboard -> c
Other -> a:
    int clipboard -> wrong
"""
        self.assertEqual(PROBE.mapping_arguments("release", mapping),
                         {"servicesField": "g", "clipboardField": "c"})
        self.assertEqual(PROBE.mapping_arguments("debug", "")["ttlField"], "pendingTtlSeconds")
        with self.assertRaises(RuntimeError):
            PROBE.mapping_arguments("release", mapping.replace("clipboard -> c", "wrong -> c"))

    def test_runtime_receipt_needs_both_count_and_android_completion(self):
        good = PROBE.SUCCESS + "\nINSTRUMENTATION_CODE: -1\n"
        self.assertTrue(PROBE.parse_result(good)["passed"])
        for bad in (PROBE.SUCCESS, good + good, good.replace("-1", "0"),
                    "INSTRUMENTATION_CODE: -1\n", "Process crashed.\n"):
            with self.subTest(bad=bad), self.assertRaises(RuntimeError):
                PROBE.parse_result(bad)

    def test_behavioral_failure_is_distinct_from_infrastructure_failure(self):
        failure = ("HIDLINS_RELEASE_CLIPBOARD 0 passed, 1 failed "
                   "stage=callback-abi code=ABI_MISSING\nINSTRUMENTATION_CODE: 0\n")
        self.assertEqual(PROBE.parse_result(failure),
                         {"passed": False, "stage": "callback-abi", "code": "ABI_MISSING"})
        self.assertEqual(PROBE.parse_result(failure.replace("ABI_MISSING", "INVALID_UTF8_ACCEPTED"))["code"],
                         "INVALID_UTF8_ACCEPTED")
        with self.assertRaises(RuntimeError):
            PROBE.parse_result(failure + "\n" + PROBE.SUCCESS)

    def test_dex_requires_real_public_virtual_byte_array_boolean_definition(self):
        definition = """Class #12 -
  Class descriptor  : 'Lc0;'
  Direct methods    -
  Virtual methods   -
    #0              : (in Lc0;)
      name          : 'receive'
      type          : '([B)Z'
      access        : 0x0001 (PUBLIC)
"""
        self.assertTrue(PROBE.has_callback_definition(definition, "Lc0;"))
        for bad in (definition.replace("'receive'", "'a'"),
                    definition.replace("([B)Z", "([B)V"),
                    definition.replace("0x0001", "0x0009"),
                    definition.replace("0x0001", "0x0002"),
                    definition.replace("Virtual methods", "Direct methods"),
                    definition.replace("'Lc0;'", "'Lother;'"),
                    "string_ids: receive ([B)Z Lc0;"):
            with self.subTest(bad=bad):
                self.assertFalse(PROBE.has_callback_definition(bad, "Lc0;"))

    def test_signature_changes_cannot_hide_modified_dex_resources_or_native_code(self):
        with tempfile.TemporaryDirectory() as temporary:
            first, second = [Path(temporary) / name for name in ("a.apk", "b.apk")]
            def write(path, code):
                with zipfile.ZipFile(path, "w") as archive:
                    archive.writestr("classes.dex", code)
                    archive.writestr("resources.arsc", b"resources")
                    archive.writestr("lib/x86_64/libhidlins_api.so", b"native")
                    archive.writestr("META-INF/CERT.RSA", path.name)
            write(first, b"same")
            write(second, b"same")
            self.assertEqual(PROBE.apk_payload(first), PROBE.apk_payload(second))
            write(second, b"changed")
            self.assertNotEqual(PROBE.apk_payload(first), PROBE.apk_payload(second))


if __name__ == "__main__":
    unittest.main(verbosity=2)
