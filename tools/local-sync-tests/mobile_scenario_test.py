#!/usr/bin/env python3
"""Unit tests for the mobile local-sync scenario harness."""

from __future__ import annotations

import importlib.util
import ipaddress
import pathlib
import re
import unittest
from unittest import mock


MODULE_PATH = pathlib.Path(__file__).with_name("mobile_scenario.py")
LAN_MODULE_PATH = MODULE_PATH.parents[1] / "android-native/lan_discovery.py"
ANDROID_SCENARIO_PATH = MODULE_PATH.parents[1] / "android-native/android_mobile_scenario.py"


def load_module():
    spec = importlib.util.spec_from_file_location("mobile_scenario", MODULE_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("could not load mobile scenario harness")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def load_lan_module():
    spec = importlib.util.spec_from_file_location("lan_discovery", LAN_MODULE_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("could not load Android LAN discovery harness")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def load_android_scenario_module():
    spec = importlib.util.spec_from_file_location(
        "android_mobile_scenario", ANDROID_SCENARIO_PATH
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("could not load Android mobile scenario harness")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class MobileScenarioHarnessTest(unittest.TestCase):
    def test_platform_routes_are_explicit_and_local_only(self) -> None:
        module = load_module()
        self.assertEqual(module.platform_route("ios"), "127.0.0.1")
        self.assertEqual(module.platform_route("android"), "10.0.2.2")
        with self.assertRaises(ValueError):
            module.platform_route("desktop")

    def test_redaction_removes_sas_password_and_private_paths(self) -> None:
        module = load_module()
        raw = (
            "Pairing SAS: ABC-123 integration-master-marker "
            "/private/tmp/hidlins-mobile-sync.secret/vaults.toml"
        )
        redacted = module.redact_text(raw, ["integration-master-marker"])
        self.assertNotIn("ABC-123", redacted)
        self.assertNotIn("integration-master-marker", redacted)
        self.assertNotIn("hidlins-mobile-sync.secret", redacted)
        self.assertIn("<redacted-sas>", redacted)

    def test_public_sync_endpoint_is_rejected(self) -> None:
        module = load_module()
        module.require_local_endpoint("127.0.0.1", 37371)
        module.require_local_endpoint("10.0.2.2", 37371)
        with self.assertRaises(ValueError):
            module.require_local_endpoint("8.8.8.8", 37371)
        with self.assertRaises(ValueError):
            module.require_local_endpoint("192.0.2.1", 37371)
        with self.assertRaises(ValueError):
            module.require_local_endpoint("example.com", 37371)

    def test_device_failure_cannot_be_masked_by_driver_success(self) -> None:
        module = load_module()
        self.assertTrue(module.flutter_output_passed("00:12 +3: All tests passed!"))
        self.assertFalse(
            module.flutter_output_passed(
                "00:00 +1 -1: Some tests failed.\nAll tests passed."
            )
        )
        self.assertFalse(module.flutter_output_passed("[E] device failure\nAll tests passed."))

    def test_evidence_name_uses_stable_device_identity(self) -> None:
        module = load_module()
        self.assertEqual(module.evidence_slug("iPhone 17 Pro Max"), "iPhone-17-Pro-Max")
        with self.assertRaises(ValueError):
            module.evidence_slug("../")

    def test_android_requires_modern_two_device_private_lan(self) -> None:
        module = load_module()
        topology = module.require_android_lan_topology(
            emulator_version="37.1.11",
            client_serial="emulator-5554",
            authority_serial="emulator-5556",
            client_address="10.0.2.16",
            authority_address="10.0.2.17",
            prefix_length=24,
        )
        self.assertEqual(topology.client, ipaddress.ip_address("10.0.2.16"))
        self.assertEqual(topology.authority, ipaddress.ip_address("10.0.2.17"))

        invalid = (
            {"emulator_version": "36.4.9"},
            {"authority_serial": "emulator-5554"},
            {"authority_address": "10.0.2.16"},
            {"authority_address": "10.0.3.17"},
            {"authority_address": "8.8.8.8"},
            {"client_address": "127.0.0.1"},
        )
        baseline = {
            "emulator_version": "37.1.11",
            "client_serial": "emulator-5554",
            "authority_serial": "emulator-5556",
            "client_address": "10.0.2.16",
            "authority_address": "10.0.2.17",
            "prefix_length": 24,
        }
        for replacement in invalid:
            with self.subTest(replacement=replacement), self.assertRaises(ValueError):
                module.require_android_lan_topology(**(baseline | replacement))

    def test_android_flutter_invocation_contains_no_sync_route(self) -> None:
        module = load_module()
        defines = module.scenario_defines(
            platform="android",
            phase="pair",
            sync_port=42873,
            control_port=42874,
            token="test-token",
        )
        joined = " ".join(defines)
        self.assertNotIn("HIDLINS_SCENARIO_SYNC_HOST", joined)
        self.assertNotIn("HIDLINS_SCENARIO_SYNC_PORT", joined)
        self.assertIn("HIDLINS_SCENARIO_CONTROL_HOST=10.0.2.2", joined)

    def test_android_discovery_markers_must_include_each_authority_phase(self) -> None:
        module = load_module()
        output = "\n".join(
            (
                "HIDLINS_DISCOVERY_CANDIDATE=10.0.2.17:37371:0",
                "HIDLINS_DISCOVERY_CANDIDATE=10.0.2.18:37371:0",
            )
        )
        self.assertEqual(
            module.assert_discovered_routes(output, "10.0.2.17", 37371),
            [
                {"address": "10.0.2.17", "port": 37371, "scope_id": 0},
                {"address": "10.0.2.18", "port": 37371, "scope_id": 0},
            ],
        )
        with self.assertRaises(ValueError):
            module.assert_discovered_routes(output, "10.0.2.19", 37371)
        with self.assertRaises(ValueError):
            module.assert_discovered_routes(
                output + "\nHIDLINS_DISCOVERY_CANDIDATE=8.8.8.8:37371:0",
                "10.0.2.17",
                37371,
            )
        with self.assertRaises(ValueError):
            module.assert_discovered_routes("All tests passed!", "10.0.2.17", 37371)

    def test_android_scenario_source_cannot_inject_candidates(self) -> None:
        source = (MODULE_PATH.parents[2] / "app/integration_test/mobile_local_sync_scenario_test.dart").read_text()
        self.assertNotIn("HIDLINS_SCENARIO_SYNC_HOST", source)
        self.assertNotIn("HIDLINS_SCENARIO_SYNC_PORT", source)
        self.assertNotIn("LocalEndpointDto(address: _syncHost", source)
        self.assertNotIn("candidates: [_endpoint]", source)
        self.assertNotIn("setDiscoveryCandidates", source)

        harness = (MODULE_PATH.parents[1] / "android-native/lan_discovery.py").read_text()
        self.assertNotIn('"forward"', harness)
        self.assertNotIn('"reverse"', harness)
        self.assertNotIn("redir", harness)
        self.assertNotIn("HIDLINS_SCENARIO_SYNC_HOST", harness)
        self.assertNotIn("HIDLINS_SCENARIO_SYNC_PORT", harness)

    def test_android_test_registrar_receives_no_network_route(self) -> None:
        module = load_lan_module()
        arguments = module.registrar_start_arguments(port=37371, kind="pairing")
        joined = " ".join(arguments)
        self.assertIn("ScenarioNsdRegistrarActivity", joined)
        self.assertIn("37371", joined)
        self.assertIn("pairing", joined)
        self.assertNotIn("address", joined.lower())
        self.assertNotRegex(joined, r"\b(?:\d{1,3}\.){3}\d{1,3}\b")
        with self.assertRaises(ValueError):
            module.registrar_start_arguments(port=0, kind="pairing")
        with self.assertRaises(ValueError):
            module.registrar_start_arguments(port=37371, kind="other")

    def test_android_peer_route_readiness_retries_until_live(self) -> None:
        module = load_android_scenario_module()
        failed = module.subprocess.CompletedProcess([], 1)
        connected = module.subprocess.CompletedProcess([], 0)
        with mock.patch.object(
            module.subprocess,
            "run",
            side_effect=(failed, connected),
        ) as run, mock.patch.object(module.time, "sleep") as sleep:
            module.require_peer_reachable("emulator-5554", "10.0.2.18")

        self.assertEqual(run.call_count, 2)
        sleep.assert_called_once_with(0.25)
        command = run.call_args.args[0]
        self.assertEqual(
            command[-6:],
            ["ping", "-c", "1", "-W", "1", "10.0.2.18"],
        )

    def test_android_registrar_is_confined_to_instrumentation_source_set(self) -> None:
        root = MODULE_PATH.parents[2]
        registrar = (
            root
            / "app/android/app/src/androidTest/java/app/hidlins/ScenarioNsdRegistrarActivity.java"
        )
        self.assertTrue(registrar.is_file())
        source = registrar.read_text(encoding="utf-8")
        self.assertIn("manager.registerService", source)
        self.assertIn("manager.unregisterService", source)
        self.assertIn("UUID.randomUUID()", source)
        self.assertNotIn("Process.myPid()", source)
        self.assertNotIn("service.setNetwork(", source)
        self.assertNotIn("bindProcessToNetwork(", source)
        self.assertNotIn("setHost(", source)
        self.assertNotIn("getActiveNetwork()", source)

        test_manifest = (root / "app/android/app/src/androidTest/AndroidManifest.xml").read_text()
        main_manifest = (root / "app/android/app/src/main/AndroidManifest.xml").read_text()
        self.assertIn("ScenarioNsdRegistrarActivity", test_manifest)
        self.assertNotIn("ScenarioNsdRegistrarActivity", main_manifest)

    def test_authority_cli_is_a_standalone_guest_process(self) -> None:
        module = load_lan_module()
        command = module.authority_command(
            "emulator-5556", "10.0.2.17"
        )
        joined = " ".join(command)
        self.assertIn("sync serve", joined)
        self.assertIn("10.0.2.17", joined)
        self.assertNotIn("run-as", joined)
        self.assertNotIn("app.hidlins/.MainActivity", joined)

        source = LAN_MODULE_PATH.read_text(encoding="utf-8")
        self.assertNotIn("activate_authority_uid", source)
        self.assertNotIn("startSyncServer", source)
        cleanup_armed = source.index("registrar_started = True")
        registrar_start = source.index("start_registrar(AUTHORITY_SERIAL")
        self.assertLess(cleanup_armed, registrar_start)

    def test_full_android_scenario_owns_two_authorities_and_nsd_lifecycle(self) -> None:
        self.assertTrue(ANDROID_SCENARIO_PATH.is_file())
        source = ANDROID_SCENARIO_PATH.read_text(encoding="utf-8")
        self.assertIn('switch_service("trusted")', source)
        self.assertIn("replace_authority", source)
        self.assertIn('selected_scenario_avds("replacement")', source)
        self.assertIn('"-wifi-server-port"', source)
        self.assertIn('"-wifi-client-port"', source)
        self.assertIn('"-network-user-mode-options"', source)
        self.assertIn("prepare_legacy_wifi_guest", source)
        initial_client_boot = source.index("client_process = lan.boot")
        initial_authority_boot = source.index("authority_process = lan.boot")
        self.assertLess(
            initial_authority_boot,
            initial_client_boot,
            "the explicit Wi-Fi server must be listening before its client boots",
        )
        topology_check = source.index("mobile.require_android_lan_topology")
        peer_route_check = source.index(
            "require_peer_reachable(\n                CLIENT_SERIAL",
            topology_check,
        )
        initial_harness = source.index("harness = AndroidAuthorityHarness")
        self.assertLess(topology_check, peer_route_check)
        self.assertLess(peer_route_check, initial_harness)
        self.assertRegex(
            source,
            re.compile(
                r"if legacy_wifi:.*?prepare_legacy_wifi_guest\(CLIENT_SERIAL\).*?"
                r"prepare_legacy_wifi_guest\(AUTHORITY_SERIAL\)",
                re.DOTALL,
            ),
        )
        self.assertIn("cleanup_guest(CLIENT_SERIAL)", source)
        self.assertIn("restarted_client_process = lan.boot", source)
        self.assertIn("prepare_client_permissions", source)
        self.assertRegex(
            source,
            re.compile(
                r"client_network_args\s*=.*?-wifi-client-port.*?"
                r"authority_network_args\s*=.*?-wifi-server-port.*?"
                r"replacement_network_args\s*=.*?-wifi-server-port",
                re.DOTALL,
            ),
        )
        self.assertNotIn("HIDLINS_SCENARIO_SYNC_HOST", source)
        self.assertNotIn("HIDLINS_SCENARIO_SYNC_PORT", source)
        self.assertNotIn('"forward"', source)
        self.assertNotIn('"reverse"', source)
        self.assertNotIn("redir", source)
        self.assertNotIn(
            'lan.adb(self.active_serial, "shell", "pidof", "hidlins")',
            source,
        )
        self.assertRegex(
            source,
            re.compile(
                r"def switch_service\(self, kind: str\).*?"
                r"if self\.registrar_active:\s+self\.stop_registrar\(\)",
                re.DOTALL,
            ),
        )
        self.assertLess(
            source.index("shutil.rmtree(scratch)"),
            source.index('evidence["cleanup"] = {'),
        )
        self.assertIn('"test_apk_sha256": mobile.file_sha256(test_apk)', source)
        self.assertIn('"resolved_routes": {', source)
        self.assertIn('"rust_accepted_endpoints":', source)
        self.assertIn('"scenario_boundary_check": "PASS"', source)

        dart = (
            MODULE_PATH.parents[2]
            / "app/integration_test/mobile_local_sync_scenario_test.dart"
        ).read_text(encoding="utf-8")
        self.assertIn("/service/trusted", dart)
        self.assertIn("HIDLINS_DISCOVERY_CANDIDATE", dart)
        self.assertIn("_waitForReachableDiscoveredCandidate", dart)
        self.assertIn("candidateDeadline", dart)
        self.assertNotIn(
            "await _discoverAndReport(sync, DiscoveryKind.trusted);\n"
            "        expect(await _unlockAndWaitForStartup",
            dart,
        )
        self.assertIn("session.startStartupSync().timeout", dart)
        self.assertNotIn("sync.startStartupSync().timeout", dart)
        self.assertIn(
            "await _discoverAndReport(sync, DiscoveryKind.trusted)", dart
        )
        self.assertRegex(
            dart,
            re.compile(
                r"await _control\('/authority-add'\);.*?"
                r"await _discoverAndReport\(sync, DiscoveryKind\.trusted\);\s+"
                r"await session\.syncNow\(\);",
                re.DOTALL,
            ),
        )
        self.assertRegex(
            dart,
            re.compile(
                r"await _control\('/assert-client-absent'\);\s+"
                r"await _discoverAndReport\(sync, DiscoveryKind\.trusted\);\s+"
                r"await session\.syncNow\(\);",
                re.DOTALL,
            ),
        )
        self.assertIn("await session.localDiscoveryStatus()", dart)
        self.assertNotIn("HIDLINS_TRUSTED_CLI_TCP_CONNECTED=true", dart)
        self.assertIn("Directory.systemTemp.parent.path}/files", dart)
        self.assertIn("getUrl(uri).timeout", dart)
        self.assertIn("await _restoreRestartSnapshot(state)", dart)

        suite = (
            MODULE_PATH.parents[1] / "android-native/run_emulator_suite.sh"
        ).read_text(encoding="utf-8")
        self.assertIn("android_mobile_scenario.py", suite)
        self.assertNotIn("mobile_scenario.py\" android", suite)


if __name__ == "__main__":
    unittest.main()
