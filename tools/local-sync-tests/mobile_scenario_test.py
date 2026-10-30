#!/usr/bin/env python3
"""Unit tests for the mobile local-sync scenario harness."""

from __future__ import annotations

import importlib.util
import contextlib
import io
import ipaddress
import pathlib
import re
import tempfile
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
    def test_scenario_boot_wakes_and_dismisses_keyguard(self) -> None:
        module = load_lan_module()
        process = mock.Mock()
        process.poll.return_value = None
        booted = module.subprocess.CompletedProcess([], 0, stdout='1\n')
        with mock.patch.object(module.subprocess, 'Popen', return_value=process), \
                mock.patch.object(module.subprocess, 'run', return_value=booted), \
                mock.patch.object(module, 'adb') as adb:
            self.assertIs(module.boot('fixture', 'emulator-fixture', 5554, io.StringIO()), process)
        adb.assert_has_calls([
            mock.call('emulator-fixture', 'shell', 'input', 'keyevent', 'KEYCODE_WAKEUP'),
            mock.call('emulator-fixture', 'shell', 'wm', 'dismiss-keyguard'),
        ])

    def test_ios_scenario_selects_the_reporter_its_result_parser_requires(self) -> None:
        module = load_module()

        def flutter(command, **kwargs):
            # Flutter's GitHub reporter is selected by the CI environment;
            # unlike expanded output, its successful summary has no "All".
            summary = ('00:01 +1: All tests passed!' if '--reporter=expanded' in command
                       else '🎉 1 test passed.')
            return module.subprocess.CompletedProcess(command, 0, stdout=summary, stderr='')

        with mock.patch.object(module.subprocess, 'run', side_effect=flutter):
            output = module.run_flutter(MODULE_PATH.parents[2], 'ios', 'fixture',
                                        'pair', 37373, 37374, 'test-token')
        self.assertTrue(module.flutter_output_passed(output))

    def test_ios_scenario_reports_redacted_authority_failure(self) -> None:
        module = load_module()
        with tempfile.TemporaryDirectory(prefix='hidlins-ios-diagnostic-') as scratch:
            root = pathlib.Path(scratch)
            artifact = root / 'artifact'
            artifact.write_bytes(b'fixture')
            args = module.argparse.Namespace(
                platform='ios', cli=str(artifact), device_name='iPhone fixture',
                device='fixture', runtime='fixture', results_dir=str(root), artifact=str(artifact))
            harness = mock.Mock(service_stderr=[])
            token = 'test-control-token-canary'
            control = mock.Mock(token=token, server_address=('127.0.0.1', 37374),
                                failures=module.queue.Queue())
            output = io.StringIO()
            with mock.patch.object(module, 'AuthorityHarness', return_value=harness), \
                    mock.patch.object(module, 'git_revision', return_value='fixture'), \
                    mock.patch.object(module, 'reserve_port', return_value=37373), \
                    mock.patch.object(module, 'ControlServer', return_value=control), \
                    mock.patch.object(module.secrets, 'token_hex', return_value=token), \
                    mock.patch.object(module.threading, 'Thread'), \
                    mock.patch.object(module, 'run_flutter', side_effect=RuntimeError(
                        'fixture authority failure ' + module.MASTER_PASSWORD + ' ' + token)), \
                    contextlib.redirect_stdout(output), contextlib.redirect_stderr(output):
                status = module.run_scenario(args)
            self.assertEqual(status, 1)
            self.assertIn('fixture authority failure', output.getvalue())
            self.assertNotIn(module.MASTER_PASSWORD, output.getvalue())
            self.assertNotIn(token, output.getvalue())
            harness.stop.assert_called_once_with(force=True)

    def test_android_scenario_reports_redacted_boot_failure_before_cleanup(self) -> None:
        module = load_android_scenario_module()
        with tempfile.TemporaryDirectory(prefix='hidlins-scenario-diagnostic-') as scratch:
            root = pathlib.Path(scratch)
            artifact = root / 'artifact'
            artifact.write_bytes(b'fixture')
            output = io.StringIO()

            def failed_boot(avd, serial, port, log, extra_args=()):
                diagnostic = '\nfixture engine initialization failed'
                # Force a canary across the output-tail boundary: redact the
                # complete value before truncation, including partial tails.
                log.write(module.mobile.MASTER_PASSWORD +
                          'x' * (2000 - len(diagnostic) - 5) + diagnostic)
                log.flush()
                raise RuntimeError('fixture boot failure ' + module.mobile.MASTER_PASSWORD)

            row = ('hidlins-fixture', '29', 'x86_64', 'phone', 'fixture-image')
            with mock.patch.object(module.lan, 'boot', side_effect=failed_boot), \
                    mock.patch.object(module, 'cleanup_guest'), \
                    mock.patch.object(module.mobile, 'reserve_port', return_value=37373), \
                    mock.patch.object(module.mobile, 'git_revision', return_value='fixture'), \
                    contextlib.redirect_stdout(output), contextlib.redirect_stderr(output):
                passed = module.run_entry(
                    row, authority_row=row, replacement_row=row, binary=artifact,
                    test_apk=artifact, shipping_apk=artifact, emulator_version='37.2.13',
                    results_dir=root)
            self.assertFalse(passed)
            self.assertIn('fixture boot failure', output.getvalue())
            self.assertIn('fixture engine initialization failed', output.getvalue())
            self.assertNotIn(module.mobile.MASTER_PASSWORD, output.getvalue())
            evidence = module.json.loads((root / 'android-hidlins-fixture.json').read_text())
            self.assertNotIn(module.mobile.MASTER_PASSWORD[-5:],
                             evidence['boot_diagnostics']['authority.log'])

    def test_shared_lan_bootstrap_uses_three_distinct_dhcp_managed_guests(self) -> None:
        module = load_android_scenario_module()
        with tempfile.TemporaryDirectory(prefix='hidlins-wifi-bootstrap-') as scratch:
            root = pathlib.Path(scratch)
            artifact = root / 'artifact'
            artifact.write_bytes(b'fixture')
            starts = []
            processes = []

            def boot(avd, serial, port, log, extra_args=()):
                starts.append((avd, serial, extra_args))
                process = mock.Mock()
                processes.append(process)
                return process

            row = ('hidlins-client', '30', 'x86_64', 'phone', 'fixture-image')
            authority = ('hidlins-authority', *row[1:])
            replacement = ('hidlins-replacement', *row[1:])
            with mock.patch.object(module.lan, 'boot', side_effect=boot), \
                    mock.patch.object(module, 'runtime_identity', side_effect=RuntimeError('stop before OS checks')), \
                    mock.patch.object(module, 'cleanup_guest') as cleanup, \
                    mock.patch.object(module.mobile, 'reserve_port', return_value=37373), \
                    mock.patch.object(module.mobile, 'git_revision', return_value='fixture'), \
                    contextlib.redirect_stdout(io.StringIO()), \
                    contextlib.redirect_stderr(io.StringIO()):
                module.run_entry(
                    row, authority_row=authority, replacement_row=replacement, binary=artifact,
                    test_apk=artifact, shipping_apk=artifact, emulator_version='37.2.13',
                    results_dir=root)
            # API30 ships both drivers but defaults to isolated legacy Wi-Fi.
            # Select the shared backend; DHCP and routing remain Android-owned.
            shared_wifi = ('-feature', 'VirtioWifi,WiFiPacketStream')
            self.assertEqual(starts, [
                ('hidlins-authority', module.AUTHORITY_SERIAL, shared_wifi),
                ('hidlins-client', module.CLIENT_SERIAL, shared_wifi),
                ('hidlins-replacement', module.REPLACEMENT_SERIAL, shared_wifi),
            ])
            for process in processes:
                process.wait.assert_called_once_with(timeout=15)
            cleanup.assert_has_calls([
                mock.call(module.CLIENT_SERIAL), mock.call(module.AUTHORITY_SERIAL),
                mock.call(module.REPLACEMENT_SERIAL),
            ])

    def test_android_full_scenario_selects_the_explicit_lan_matrix(self) -> None:
        module = load_android_scenario_module()

        def matrix(command, **kwargs):
            apis = (30, 36) if command[-1] == 'lan' else (29, 36)
            output = ''.join(f'fixture-{api}\t{api}\tx86_64\tphone\timage\n' for api in apis)
            return module.subprocess.CompletedProcess(command, 0, stdout=output)

        with mock.patch.object(module.subprocess, 'run', side_effect=matrix):
            self.assertEqual([row[1] for row in module.selected_matrix()], ['30', '36'])

    def test_emulator_version_probe_works_without_desktop_audio_libraries(self) -> None:
        module = load_lan_module()
        with tempfile.TemporaryDirectory(prefix='hidlins-headless-emulator-') as scratch:
            emulator = pathlib.Path(scratch) / 'emulator'
            emulator.write_text('''#!/bin/sh
case "$*" in
  '-no-window -no-audio -version') echo 'Android emulator version 37.2.13.0 (fixture)' ;;
  *) echo 'libpulse.so.0: cannot open shared object file' >&2; exit 127 ;;
esac
''')
            emulator.chmod(0o755)
            with mock.patch.object(module, 'EMULATOR', emulator):
                self.assertEqual(module.emulator_version(), '37.2.13')
                emulator.write_text('#!/bin/sh\necho unknown-version\n')
                with self.assertRaisesRegex(RuntimeError, 'could not be determined'):
                    module.emulator_version()

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
            "/private/tmp/hidlins-mobile-sync.secret/vaults.toml "
            "/tmp/hidlins-android-mobile-sync.secret/vaults.toml"
        )
        redacted = module.redact_text(raw, ["integration-master-marker"])
        self.assertNotIn("ABC-123", redacted)
        self.assertNotIn("integration-master-marker", redacted)
        self.assertNotIn("hidlins-mobile-sync.secret", redacted)
        self.assertNotIn("hidlins-android-mobile-sync", redacted)
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

    def test_android_peer_route_readiness_uses_wifi_without_requiring_icmp(self) -> None:
        module = load_android_scenario_module()
        failed = module.subprocess.CompletedProcess([], 2, stdout='', stderr='unreachable')
        connected = module.subprocess.CompletedProcess(
            [], 0, stdout='10.0.2.18 dev wlan0 table 1005 src 10.0.2.16 uid 2000\n    cache\n')
        with mock.patch.object(
            module.subprocess, "run",
            side_effect=(module.subprocess.TimeoutExpired("adb", 5), failed, connected),
        ) as run, mock.patch.object(module.time, "sleep") as sleep:
            module.require_wifi_route("emulator-5554", "10.0.2.18", "10.0.2.16")

        self.assertEqual(run.call_count, 3)
        self.assertEqual(sleep.call_args_list, [mock.call(0.25), mock.call(0.25)])
        self.assertEqual(run.call_args.args[0][-5:],
                         ["ip", "-4", "route", "get", "10.0.2.18"])
        self.assertIs(run.call_args.kwargs['capture_output'], True)

    def test_android_peer_route_rejects_cellular_and_malformed_routes(self) -> None:
        module = load_android_scenario_module()
        for route in (
            '10.0.2.18 dev rmnet_data0 src 10.0.2.15',
            '10.0.2.18 dev wlan01 src 10.0.2.16',
            'unreachable 10.0.2.18 dev wlan0',
            '10.0.2.19 dev wlan0 src 10.0.2.16',
            '10.0.2.18 dev wlan0',
            '10.0.2.18 dev wlan0 src 10.0.2.15',
            '',
        ):
            with self.subTest(route=route), \
                    mock.patch.object(module.subprocess, 'run', return_value=
                                      module.subprocess.CompletedProcess([], 0, stdout=route)), \
                    mock.patch.object(module.time, 'monotonic', side_effect=(0, 0, 31)), \
                    mock.patch.object(module.time, 'sleep'), \
                    self.assertRaisesRegex(RuntimeError, 'Wi-Fi route'):
                module.require_wifi_route('emulator-5554', '10.0.2.18', '10.0.2.16')

    def test_android_route_failure_reports_only_bounded_network_fields(self) -> None:
        module = load_android_scenario_module()
        for output, expected in (
            ('local 10.0.2.16 dev lo src 10.0.2.16 uid SECRET-CANARY',
             'kind=local, interface=lo, source=10.0.2.16'),
            ('10.0.2.16 dev SECRET-CANARY src SECRET-CANARY',
             'kind=unicast, interface=other, source=unknown'),
        ):
            response = module.subprocess.CompletedProcess([], 0, stdout=output)
            with self.subTest(output=output), \
                    mock.patch.object(module.subprocess, 'run', return_value=response), \
                    mock.patch.object(module.time, 'monotonic', side_effect=(0, 0, 31)), \
                    mock.patch.object(module.time, 'sleep'), \
                    self.assertRaises(RuntimeError) as failed:
                module.require_wifi_route('emulator-5556', '10.0.2.16', '10.0.2.17')
            self.assertIn(expected, str(failed.exception))
            self.assertNotIn('SECRET-CANARY', str(failed.exception))

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
        initial_client_boot = source.index("client_process = lan.boot")
        initial_authority_boot = source.index("authority_process = lan.boot")
        initial_replacement_boot = source.index("replacement_process = lan.boot")
        self.assertLess(initial_authority_boot, initial_client_boot)
        self.assertLess(initial_client_boot, initial_replacement_boot)
        topology_check = source.index("mobile.require_android_lan_topology")
        self.assertLess(topology_check, source.index("require_wifi_route(CLIENT_SERIAL", topology_check))
        self.assertLess(source.index("require_wifi_route(CLIENT_SERIAL", topology_check),
                        source.index("harness = AndroidAuthorityHarness"))
        self.assertIn("prepare_client_permissions", source)
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
