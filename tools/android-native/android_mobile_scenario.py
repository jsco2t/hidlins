#!/usr/bin/env python3
"""Run full shipping-client sync against discovered CLI authority emulators."""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import selectors
import secrets
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "local-sync-tests"))
sys.path.insert(0, str(ROOT / "tools" / "android-native"))

import lan_discovery as lan  # noqa: E402
import mobile_scenario as mobile  # noqa: E402

CLIENT_SERIAL = "emulator-5554"
AUTHORITY_SERIAL = "emulator-5556"
REPLACEMENT_SERIAL = "emulator-5558"
REMOTE_ROOT = lan.REMOTE_ROOT
PORT = lan.PORT


class AndroidAuthorityHarness:
    """Control the real standalone CLI and test-only registrar over ADB."""

    def __init__(
        self,
        *,
        binary: pathlib.Path,
        shipping_apk: pathlib.Path,
        test_apk: pathlib.Path,
        primary_serial: str,
        primary_address: str,
        replacement_serial: str,
        replacement_address: str | None,
        scratch: pathlib.Path,
    ) -> None:
        self.binary = binary
        self.shipping_apk = shipping_apk
        self.test_apk = test_apk
        self.primary_serial = primary_serial
        self.primary_address = primary_address
        self.replacement_serial = replacement_serial
        self.replacement_address = replacement_address
        self.active_serial = primary_serial
        self.active_address = primary_address
        self.scratch = scratch
        self.service: subprocess.Popen[str] | None = None
        self.service_stderr: list[str] = []
        self._stderr_thread: threading.Thread | None = None
        self.registrar_active = False
        self.registrar_kind: str | None = None
        self.registrations: list[dict[str, object]] = []
        self.authority_state_hashes: dict[str, dict[str, str]] | None = None
        self.lock = threading.Lock()

    def _command(self, *args: str) -> list[str]:
        return [
            str(lan.ADB),
            "-s",
            self.active_serial,
            "shell",
            f"{REMOTE_ROOT}/hidlins",
            "--registry",
            f"{REMOTE_ROOT}/vaults.toml",
            *args,
        ]

    def _prepare(self, serial: str) -> None:
        lan.adb(serial, "install", "-r", "-g", str(self.shipping_apk))
        lan.install_registrar(serial, self.test_apk)
        lan.adb(serial, "shell", "mkdir", "-p", REMOTE_ROOT)
        lan.adb(serial, "push", str(self.binary), f"{REMOTE_ROOT}/hidlins")
        lan.adb(serial, "shell", "chmod", "700", f"{REMOTE_ROOT}/hidlins")

    def run_cli(
        self,
        args: list[str],
        stdin: str = "",
        expected: tuple[int, ...] = (0,),
    ) -> subprocess.CompletedProcess[str]:
        result = subprocess.run(
            self._command(*args),
            input=stdin,
            text=True,
            capture_output=True,
            timeout=120,
            check=False,
        )
        if result.returncode not in expected:
            detail = mobile.redact_text(
                result.stdout + result.stderr, [mobile.MASTER_PASSWORD]
            )
            raise RuntimeError(
                f"authority CLI command failed ({result.returncode}): {detail[-2000:]}"
            )
        return result

    def create(self) -> None:
        self._prepare(self.primary_serial)
        self.run_cli(
            [
                "--format",
                "json",
                "vault",
                "create",
                "--id",
                mobile.AUTHORITY_VAULT,
                "--path",
                f"{REMOTE_ROOT}/authority.kdbx",
                "--no-recovery-warning",
            ],
            f"{mobile.MASTER_PASSWORD}\n{mobile.MASTER_PASSWORD}\n",
        )

    def _drain_stderr(self, stream: Any) -> None:
        for line in stream:
            self.service_stderr.append(
                mobile.redact_text(line, [mobile.MASTER_PASSWORD])
            )

    def start(self, pairing_answers: list[str] | None = None) -> None:
        if self.service is not None:
            raise RuntimeError("authority is already running")
        command = self._command(
            "--format",
            "json",
            "sync",
            "serve",
            "--vault",
            mobile.AUTHORITY_VAULT,
            "--address",
            self.active_address,
            "--port",
            str(PORT),
            *([] if pairing_answers is None else ["--pairing-window"]),
        )
        self.service = subprocess.Popen(
            command,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        assert self.service.stdin is not None and self.service.stdout is not None
        answers = "" if pairing_answers is None else "".join(
            f"{answer}\n" for answer in pairing_answers
        )
        self.service.stdin.write(f"{mobile.MASTER_PASSWORD}\n{answers}")
        self.service.stdin.flush()
        selector = selectors.DefaultSelector()
        selector.register(self.service.stdout, selectors.EVENT_READ)
        ready = selector.select(timeout=60)
        selector.close()
        if not ready:
            self.stop(force=True)
            raise RuntimeError("authority CLI did not report readiness")
        try:
            payload = json.loads(self.service.stdout.readline())
        except json.JSONDecodeError as error:
            self.stop(force=True)
            raise RuntimeError("authority CLI readiness was not JSON") from error
        if payload.get("status") != "serving" or payload.get("endpoint") != (
            f"{self.active_address}:{PORT}"
        ):
            self.stop(force=True)
            raise RuntimeError("authority CLI reported an unexpected listener")
        assert self.service.stderr is not None
        self._stderr_thread = threading.Thread(
            target=self._drain_stderr,
            args=(self.service.stderr,),
            name="hidlins-android-authority-stderr",
            daemon=True,
        )
        self._stderr_thread.start()

    def stop(self, force: bool = False) -> None:
        service = self.service
        if service is None:
            return
        self.service = None
        if service.poll() is None:
            pid_result = subprocess.run(
                [
                    str(lan.ADB),
                    "-s",
                    self.active_serial,
                    "shell",
                    "pidof",
                    "hidlins",
                ],
                text=True,
                capture_output=True,
                check=False,
            )
            pids = pid_result.stdout.split()
            for pid in pids:
                subprocess.run(
                    [
                        str(lan.ADB),
                        "-s",
                        self.active_serial,
                        "shell",
                        "kill",
                        "-2",
                        pid,
                    ],
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                    check=False,
                )
            try:
                service.wait(timeout=15)
            except subprocess.TimeoutExpired:
                service.terminate()
                service.wait(timeout=5)
                if not force:
                    raise RuntimeError("authority CLI ignored interrupt")
        if not force and service.returncode != 0:
            raise RuntimeError(f"authority CLI exited with status {service.returncode}")
        if self._stderr_thread is not None:
            self._stderr_thread.join(timeout=2)
            self._stderr_thread = None

    def switch_service(self, kind: str) -> None:
        if kind not in {"pairing", "trusted"}:
            raise ValueError("invalid discovery service kind")
        if self.registrar_active:
            self.stop_registrar()
        self.registrar_active = True
        lan.start_registrar(self.active_serial, port=PORT, kind=kind)
        self.registrar_kind = kind
        self.registrations.append(
            {
                "serial": self.active_serial,
                "kind": kind,
                "locally_supplied_port": PORT,
                "independently_observed_wlan0": self.active_address,
                "unregistered": False,
            }
        )

    def stop_registrar(self) -> None:
        if not self.registrar_active:
            return
        lan.stop_registrar(self.active_serial)
        self.registrar_active = False
        if self.registrations:
            self.registrations[-1]["unregistered"] = True

    def restart(self) -> None:
        self.stop()
        self.start()

    def peers(self) -> list[dict[str, Any]]:
        result = self.run_cli(
            [
                "--format",
                "json",
                "sync",
                "peers",
                "list",
                "--vault",
                mobile.AUTHORITY_VAULT,
            ]
        )
        peers = json.loads(result.stdout).get("peers")
        if not isinstance(peers, list):
            raise RuntimeError("authority peer list was malformed")
        return peers

    def authority_add(self) -> None:
        self.stop()
        try:
            self.run_cli(
                [
                    "entry",
                    "add",
                    "--vault",
                    mobile.AUTHORITY_VAULT,
                    "--title",
                    mobile.AUTHORITY_ENTRY,
                ],
                f"{mobile.MASTER_PASSWORD}\n",
            )
        finally:
            self.start()

    def _authority_has(self, title: str) -> bool:
        result = self.run_cli(
            [
                "--format",
                "json",
                "entry",
                "get",
                "--vault",
                mobile.AUTHORITY_VAULT,
                "--title",
                title,
            ],
            f"{mobile.MASTER_PASSWORD}\n",
            expected=(0, 1),
        )
        return result.returncode == 0

    def assert_client_write(self, expected: bool) -> None:
        self.stop()
        try:
            actual = self._authority_has(mobile.CLIENT_ENTRY)
            if actual != expected:
                raise RuntimeError(
                    f"client write presence was {actual}, expected {expected}"
                )
        finally:
            self.start()

    def revoke(self) -> None:
        self.stop()
        try:
            active = [peer for peer in self.peers() if not peer.get("revoked", False)]
            if len(active) != 1 or not isinstance(active[0].get("peer"), str):
                raise RuntimeError("expected exactly one active client before revocation")
            self.run_cli(
                [
                    "sync",
                    "peers",
                    "revoke",
                    "--vault",
                    mobile.AUTHORITY_VAULT,
                    "--peer",
                    active[0]["peer"],
                ]
            )
            updated = self.peers()
            if len(updated) != 1 or not updated[0].get("revoked", False):
                raise RuntimeError("authority did not persist client revocation")
        finally:
            self.start()

    def export_authority_state(self) -> None:
        """Stop the old authority and export only encrypted durable state."""

        self.stop()
        self.stop_registrar()
        transfer = self.scratch / "authority-state"
        transfer.mkdir(mode=0o700)
        before: dict[str, str] = {}
        for name in ("vaults.toml", "authority.kdbx"):
            target = transfer / name
            lan.adb(
                self.primary_serial,
                "pull",
                f"{REMOTE_ROOT}/{name}",
                str(target),
            )
            before[name] = mobile.file_sha256(target)
        self.authority_state_hashes = {"before": before}

    def activate_replacement(self, address: str) -> None:
        """Import encrypted state and start the replacement CLI/registrar."""

        if self.authority_state_hashes is None:
            raise RuntimeError("authority state was not exported before replacement")
        transfer = self.scratch / "authority-state"
        self.replacement_address = address

        self._prepare(self.replacement_serial)
        for name in ("vaults.toml", "authority.kdbx"):
            lan.adb(
                self.replacement_serial,
                "push",
                str(transfer / name),
                f"{REMOTE_ROOT}/{name}",
            )
        self.active_serial = self.replacement_serial
        self.active_address = address

        verified = self.scratch / "replacement-state"
        verified.mkdir(mode=0o700)
        after: dict[str, str] = {}
        for name in ("vaults.toml", "authority.kdbx"):
            target = verified / name
            lan.adb(
                self.replacement_serial,
                "pull",
                f"{REMOTE_ROOT}/{name}",
                str(target),
            )
            after[name] = mobile.file_sha256(target)
        before = self.authority_state_hashes["before"]
        if before != after:
            raise RuntimeError("replacement authority state changed during encrypted transfer")
        self.authority_state_hashes = {"before": before, "after": after}
        self.start()
        self.switch_service("trusted")

    def replace_authority(self) -> None:
        """Move encrypted state to an already-booted replacement guest."""

        if self.replacement_address is None:
            raise RuntimeError("replacement authority address is not available")
        self.export_authority_state()
        self.activate_replacement(self.replacement_address)

    def cleanup(self) -> None:
        registrar_error: Exception | None = None
        try:
            self.stop_registrar()
        except Exception as error:  # preserve server cleanup after registrar failure
            registrar_error = error
        self.stop(force=True)
        if registrar_error is not None:
            raise registrar_error


def selected_matrix() -> list[tuple[str, str, str, str, str]]:
    output = subprocess.run(
        [sys.executable, str(ROOT / "tools/android-native/emulator_matrix.py"), "select"],
        text=True,
        capture_output=True,
        check=True,
    ).stdout
    return [tuple(line.split("\t")) for line in output.splitlines() if line]  # type: ignore[return-value]


def selected_scenario_avds(kind: str) -> list[tuple[str, str, str, str, str]]:
    output = subprocess.run(
        [sys.executable, str(ROOT / "tools/android-native/emulator_matrix.py"), kind],
        text=True,
        capture_output=True,
        check=True,
    ).stdout
    rows = [tuple(line.split("\t")) for line in output.splitlines() if line]
    if len(rows) != 2:
        raise RuntimeError(f"Android scenario selected {len(rows)} {kind} AVDs")
    return rows  # type: ignore[return-value]


def runtime_identity(serial: str, expected_avd: str, api: str, abi: str) -> None:
    actual_api = lan.adb(serial, "shell", "getprop", "ro.build.version.sdk").stdout.strip()
    actual_abi = lan.adb(serial, "shell", "getprop", "ro.product.cpu.abi").stdout.strip()
    actual_avd = lan.adb(serial, "emu", "avd", "name").stdout.splitlines()[0].strip()
    if (actual_avd, actual_api, actual_abi) != (expected_avd, api, abi):
        raise RuntimeError("Android scenario runtime identity mismatch")


def prepare_legacy_wifi_guest(serial: str) -> None:
    """Make the API 29 emulator's explicit Wi-Fi link the sole data network.

    The legacy emulator link assigns its synthetic cellular and Wi-Fi adapters
    addresses in the same subnet. A physical Wi-Fi-only device has no such
    ambiguous duplicate path, and API 29 cannot bind an NSD registration to a
    specific Network. Disabling only the emulator's cellular data keeps route
    selection with Android while preserving the genuine multicast Wi-Fi LAN.
    """

    lan.adb(serial, "shell", "svc", "data", "disable")
    mobile_data = lan.adb(
        serial, "shell", "settings", "get", "global", "mobile_data"
    ).stdout.strip()
    if mobile_data != "0":
        raise RuntimeError("Android emulator did not enter the Wi-Fi-only state")


def require_peer_reachable(serial: str, address: str) -> None:
    """Wait until an emulator's selected Wi-Fi route reaches its peer."""

    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        result = subprocess.run(
            [
                str(lan.ADB),
                "-s",
                serial,
                "shell",
                "ping",
                "-c",
                "1",
                "-W",
                "1",
                address,
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        if result.returncode == 0:
            return
        time.sleep(0.25)
    raise RuntimeError(f"{serial} did not establish its Wi-Fi route to the peer")


def cleanup_guest(serial: str) -> None:
    subprocess.run(
        [str(lan.ADB), "-s", serial, "uninstall", "app.hidlins.test"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    subprocess.run(
        [str(lan.ADB), "-s", serial, "uninstall", "app.hidlins"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    subprocess.run(
        [str(lan.ADB), "-s", serial, "shell", "rm", "-rf", REMOTE_ROOT],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    lan.stop_emulator(serial)


def prepare_client_permissions(serial: str, shipping_apk: pathlib.Path) -> None:
    """Install the shipping app and grant only available discovery permissions."""

    lan.adb(serial, "install", "-r", "-g", str(shipping_apk))
    declared = lan.adb(serial, "shell", "pm", "list", "permissions").stdout
    for permission in (
        "android.permission.NEARBY_WIFI_DEVICES",
        "android.permission.ACCESS_LOCAL_NETWORK",
    ):
        if permission not in declared:
            continue
        lan.adb(
            serial,
            "shell",
            "pm",
            "grant",
            "--user",
            "0",
            "app.hidlins",
            permission,
        )
        checked = lan.adb(
            serial,
            "shell",
            "pm",
            "check-permission",
            "--user",
            "0",
            permission,
            "app.hidlins",
        ).stdout.strip()
        if checked != "granted":
            raise RuntimeError(f"Android did not grant client permission {permission}")


def run_entry(
    row: tuple[str, str, str, str, str],
    *,
    authority_row: tuple[str, str, str, str, str],
    replacement_row: tuple[str, str, str, str, str],
    binary: pathlib.Path,
    test_apk: pathlib.Path,
    shipping_apk: pathlib.Path,
    emulator_version: str,
    results_dir: pathlib.Path,
) -> bool:
    avd, api, abi, form_factor, image = row
    authority_avd, authority_api, authority_abi, _, _ = authority_row
    replacement_avd, replacement_api, replacement_abi, _, _ = replacement_row
    if (authority_api, authority_abi) != (replacement_api, replacement_abi):
        raise RuntimeError("Android authority and replacement images differ")
    if authority_abi != abi:
        raise RuntimeError("Android client and authority ABIs differ")
    scratch = pathlib.Path(tempfile.mkdtemp(prefix="hidlins-android-mobile-sync."))
    logs: list[str] = []
    processes: list[subprocess.Popen[str]] = []
    harness: AndroidAuthorityHarness | None = None
    control: mobile.ControlServer | None = None
    control_thread: threading.Thread | None = None
    started = time.time()
    evidence_path = results_dir / f"android-{mobile.evidence_slug(avd)}.json"
    log_path = results_dir / f"android-{mobile.evidence_slug(avd)}.log"
    evidence: dict[str, object] = {
        "schema_version": 2,
        "scenario": "android-platform-discovered-cli-authority",
        "platform": "android",
        "git_revision": mobile.git_revision(ROOT),
        "cli_sha256": mobile.file_sha256(binary),
        "app_artifact_sha256": mobile.file_sha256(shipping_apk),
        "test_apk_sha256": mobile.file_sha256(test_apk),
        "matrix": {"api": int(api), "abi": abi, "form_factor": form_factor, "image": image},
        "route_injection": False,
        "sync_proxy": False,
        "client_endpoint_input": False,
        "registrar_test_only": True,
        "steps": [],
        "result": "FAIL",
    }
    try:
        with (scratch / "client.log").open("w", encoding="utf-8") as client_log, (
            scratch / "authority.log"
        ).open("w", encoding="utf-8") as authority_log, (
            scratch / "replacement.log"
        ).open("w", encoding="utf-8") as replacement_log:
            legacy_wifi = int(api) < 31
            wifi_link_port = mobile.reserve_port()
            client_network_args = (
                (
                    "-wifi-client-port",
                    str(wifi_link_port),
                    "-network-user-mode-options",
                    "dhcpstart=10.0.2.16",
                )
                if legacy_wifi
                else ()
            )
            authority_network_args = (
                (
                    "-wifi-server-port",
                    str(wifi_link_port),
                    "-network-user-mode-options",
                    "dhcpstart=10.0.2.17",
                )
                if legacy_wifi
                else ()
            )
            replacement_network_args = (
                (
                    "-wifi-server-port",
                    str(wifi_link_port),
                    "-network-user-mode-options",
                    "dhcpstart=10.0.2.18",
                )
                if legacy_wifi
                else ()
            )
            authority_process = lan.boot(
                authority_avd,
                AUTHORITY_SERIAL,
                5556,
                authority_log,
                authority_network_args,
            )
            processes.append(authority_process)
            client_process = lan.boot(
                avd,
                CLIENT_SERIAL,
                5554,
                client_log,
                client_network_args,
            )
            processes.append(client_process)
            replacement_process: subprocess.Popen[str] | None = None
            if not legacy_wifi:
                replacement_process = lan.boot(
                    replacement_avd,
                    REPLACEMENT_SERIAL,
                    5558,
                    replacement_log,
                )
                processes.append(replacement_process)
            runtime_identity(CLIENT_SERIAL, avd, api, abi)
            runtime_identity(
                AUTHORITY_SERIAL, authority_avd, authority_api, authority_abi
            )
            if legacy_wifi:
                prepare_legacy_wifi_guest(CLIENT_SERIAL)
                prepare_legacy_wifi_guest(AUTHORITY_SERIAL)
            client_address, prefix = lan.wlan_address(CLIENT_SERIAL)
            restarted_client_address = client_address
            authority_address, authority_prefix = lan.wlan_address(AUTHORITY_SERIAL)
            replacement_address: str | None = None
            replacement_prefix: int | None = None
            if replacement_process is not None:
                runtime_identity(
                    REPLACEMENT_SERIAL,
                    replacement_avd,
                    replacement_api,
                    replacement_abi,
                )
                replacement_address, replacement_prefix = lan.wlan_address(
                    REPLACEMENT_SERIAL
                )
            evidence["observed_routes"] = {
                "client": {"address": client_address, "prefix": prefix},
                "authority": {
                    "address": authority_address,
                    "prefix": authority_prefix,
                },
                "replacement": None,
            }
            if authority_prefix != prefix:
                raise RuntimeError("Android client and authority wlan0 prefixes differ")
            mobile.require_android_lan_topology(
                emulator_version=emulator_version,
                client_serial=CLIENT_SERIAL,
                authority_serial=AUTHORITY_SERIAL,
                client_address=client_address,
                authority_address=authority_address,
                prefix_length=prefix,
            )
            require_peer_reachable(
                CLIENT_SERIAL,
                authority_address,
            )
            require_peer_reachable(
                AUTHORITY_SERIAL,
                client_address,
            )
            if replacement_address is not None and replacement_prefix is not None:
                evidence["observed_routes"]["replacement"] = {  # type: ignore[index]
                    "address": replacement_address,
                    "prefix": replacement_prefix,
                }
                if replacement_prefix != prefix:
                    raise RuntimeError("Android replacement wlan0 prefix differs")
                mobile.require_android_lan_topology(
                    emulator_version=emulator_version,
                    client_serial=CLIENT_SERIAL,
                    authority_serial=REPLACEMENT_SERIAL,
                    client_address=client_address,
                    authority_address=replacement_address,
                    prefix_length=prefix,
                )
                require_peer_reachable(CLIENT_SERIAL, replacement_address)
                require_peer_reachable(REPLACEMENT_SERIAL, client_address)
                if len({client_address, authority_address, replacement_address}) != 3:
                    raise RuntimeError(
                        "Android scenario guests did not receive distinct DHCP routes"
                    )

            harness = AndroidAuthorityHarness(
                binary=binary,
                shipping_apk=shipping_apk,
                test_apk=test_apk,
                primary_serial=AUTHORITY_SERIAL,
                primary_address=authority_address,
                replacement_serial=REPLACEMENT_SERIAL,
                replacement_address=replacement_address,
                scratch=scratch,
            )
            harness.create()
            harness.start(["no", "yes"])
            harness.switch_service("pairing")
            token = secrets.token_hex(24)
            control = mobile.ControlServer(harness, token)  # type: ignore[arg-type]
            control_thread = threading.Thread(target=control.serve_forever, daemon=True)
            control_thread.start()
            control_port = int(control.server_address[1])
            mobile.require_local_endpoint(mobile.platform_route("android"), control_port)

            prepare_client_permissions(CLIENT_SERIAL, shipping_apk)
            pair_log = mobile.run_flutter(
                ROOT,
                "android",
                CLIENT_SERIAL,
                "pair",
                PORT,
                control_port,
                token,
            )
            logs.append(pair_log)
            pair_routes = mobile.assert_discovered_routes(
                pair_log, authority_address, PORT
            )
            if not control.failures.empty():
                raise RuntimeError(control.failures.get_nowait())

            if legacy_wifi:
                harness.export_authority_state()
                cleanup_guest(AUTHORITY_SERIAL)
                cleanup_guest(CLIENT_SERIAL)
                try:
                    authority_process.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    authority_process.terminate()
                    authority_process.wait(timeout=5)
                try:
                    client_process.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    client_process.terminate()
                    client_process.wait(timeout=5)
                replacement_process = lan.boot(
                    replacement_avd,
                    REPLACEMENT_SERIAL,
                    5558,
                    replacement_log,
                    replacement_network_args,
                )
                processes.append(replacement_process)
                restarted_client_process = lan.boot(
                    avd,
                    CLIENT_SERIAL,
                    5554,
                    client_log,
                    client_network_args,
                )
                processes.append(restarted_client_process)
                runtime_identity(
                    REPLACEMENT_SERIAL,
                    replacement_avd,
                    replacement_api,
                    replacement_abi,
                )
                runtime_identity(CLIENT_SERIAL, avd, api, abi)
                prepare_legacy_wifi_guest(REPLACEMENT_SERIAL)
                prepare_legacy_wifi_guest(CLIENT_SERIAL)
                restarted_client_address, restarted_prefix = lan.wlan_address(
                    CLIENT_SERIAL
                )
                replacement_address, replacement_prefix = lan.wlan_address(
                    REPLACEMENT_SERIAL
                )
                evidence["observed_routes"]["replacement"] = {  # type: ignore[index]
                    "address": replacement_address,
                    "prefix": replacement_prefix,
                }
                evidence["observed_routes"]["client_restart"] = {  # type: ignore[index]
                    "address": restarted_client_address,
                    "prefix": restarted_prefix,
                }
                if replacement_prefix != prefix:
                    raise RuntimeError("Android replacement wlan0 prefix differs")
                if restarted_prefix != prefix:
                    raise RuntimeError("Android restarted client wlan0 prefix differs")
                mobile.require_android_lan_topology(
                    emulator_version=emulator_version,
                    client_serial=CLIENT_SERIAL,
                    authority_serial=REPLACEMENT_SERIAL,
                    client_address=restarted_client_address,
                    authority_address=replacement_address,
                    prefix_length=prefix,
                )
                require_peer_reachable(CLIENT_SERIAL, replacement_address)
                require_peer_reachable(REPLACEMENT_SERIAL, restarted_client_address)
                if replacement_address == authority_address:
                    raise RuntimeError("replacement authority reused the original DHCP route")
                harness.activate_replacement(replacement_address)
            else:
                harness.replace_authority()
            assert replacement_address is not None
            prepare_client_permissions(CLIENT_SERIAL, shipping_apk)
            restart_log = mobile.run_flutter(
                ROOT,
                "android",
                CLIENT_SERIAL,
                "restart",
                PORT,
                control_port,
                token,
            )
            logs.append(restart_log)
            restart_routes = mobile.assert_discovered_routes(
                restart_log, replacement_address, PORT
            )
            if not control.failures.empty():
                raise RuntimeError(control.failures.get_nowait())
            if control.snapshot_hashes is None:
                raise RuntimeError("mobile client did not preserve encrypted restart state")

            step_names = (
                "mobile-server-rejected",
                "pairing-rejected-without-trust",
                "pairing-accepted-and-imported",
                "startup-sync",
                "authority-to-client-change",
                "save-does-not-sync",
                "manual-client-to-authority-sync",
                "client-process-restart",
                "pinned-trust-reconnect",
                "authority-restart",
                "revocation-fails-closed",
            )
            evidence["steps"] = [
                {"name": name, "result": "PASS"} for name in step_names
            ]
            evidence["emulator_version"] = emulator_version
            evidence["network_conditions"] = {
                "legacy_explicit_wifi": legacy_wifi,
                "cellular_data": "disabled" if legacy_wifi else "platform-default",
                "route_injected": False,
            }
            evidence["client"] = {
                "avd": avd,
                "serial": CLIENT_SERIAL,
                "initial_wlan0": client_address,
                "restart_wlan0": restarted_client_address,
            }
            evidence["authorities"] = {
                "initial": {
                    "avd": authority_avd,
                    "api": int(authority_api),
                    "abi": authority_abi,
                    "serial": AUTHORITY_SERIAL,
                    "wlan0": authority_address,
                },
                "replacement": {
                    "avd": replacement_avd,
                    "api": int(replacement_api),
                    "abi": replacement_abi,
                    "serial": REPLACEMENT_SERIAL,
                    "wlan0": replacement_address,
                },
                "route_changed": authority_address != replacement_address,
                "execution_model": "standalone-guest-cli",
                "mobile_server_enabled": False,
            }
            evidence["discovery"] = {
                "adapter": "shipping-Android-NsdManager",
                "registrar": "androidTest-NsdManager",
                "registrar_source_set": "androidTest",
                "registrar_apk_package": "app.hidlins.test",
                "registrar_received_address": False,
                "registrations": harness.registrations,
                "resolved_routes": {
                    "pairing_and_initial_trusted": pair_routes,
                    "replacement_trusted": restart_routes,
                },
                "pair_route_count": len(pair_routes),
                "restart_route_count": len(restart_routes),
                "rust_policy_validated": True,
                "rust_accepted_endpoints": [
                    {"address": authority_address, "port": PORT, "scope_id": 0},
                    {"address": replacement_address, "port": PORT, "scope_id": 0},
                ],
            }
            evidence["control_plane"] = {
                "transport": "authenticated-host-http",
                "android_host_route": "10.0.2.2",
                "carries_sync_route": False,
                "proxies_sync_bytes": False,
            }
            evidence["data_plane"] = {
                "transport": "direct-client-to-cli",
                "listener_port": PORT,
                "noise": ["XX", "IK"],
                "authenticated_phases": ["pairing-XX", "trusted-sync-IK"],
                "cli_owned": True,
            }
            evidence["packaging"] = {
                "scenario_boundary_check": "PASS",
                "registrar_absent_from_shipping_apks": True,
                "scenario_cli_absent_from_shipping_apks": True,
            }
            evidence["restart_state_transport"] = (
                "encrypted-kdbx-and-sealed-registry"
            )
            evidence["restart_snapshot_sha256"] = control.snapshot_hashes
            evidence["authority_state_sha256"] = harness.authority_state_hashes
            evidence["result"] = "PASS"
    except Exception as error:
        detail = mobile.redact_text(str(error), [mobile.MASTER_PASSWORD])
        if control is not None and not control.failures.empty():
            detail = f"{detail}\ncontrol: {control.failures.get_nowait()}"
        evidence["failure"] = detail
    finally:
        cleanup_failures: list[str] = []
        try:
            if control is not None:
                control.shutdown()
                control.server_close()
            if control_thread is not None:
                control_thread.join(timeout=5)
        except Exception as error:
            cleanup_failures.append(f"control: {error}")
        try:
            if harness is not None:
                harness.cleanup()
        except Exception as error:
            cleanup_failures.append(f"authority: {error}")
        emulators_stopped = True
        for serial in (CLIENT_SERIAL, AUTHORITY_SERIAL, REPLACEMENT_SERIAL):
            try:
                cleanup_guest(serial)
            except Exception as error:
                emulators_stopped = False
                cleanup_failures.append(f"{serial}: {error}")
        for process in processes:
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    emulators_stopped = False
                    cleanup_failures.append("emulator process ignored termination")
        scratch_removed = False
        try:
            shutil.rmtree(scratch)
            scratch_removed = True
        except Exception as error:
            cleanup_failures.append(f"scratch: {error}")
        registrar_unregistered = harness is None or (
            not harness.registrar_active
            and all(
                registration.get("unregistered") is True
                for registration in harness.registrations
            )
        )
        cli_stopped = harness is None or harness.service is None
        if cleanup_failures or not registrar_unregistered or not cli_stopped:
            evidence["result"] = "FAIL"
            evidence["cleanup_failure"] = mobile.redact_text(
                "\n".join(cleanup_failures) or "cleanup invariant failed",
                [mobile.MASTER_PASSWORD],
            )
        evidence["cleanup"] = {
            "registrar_unregistered": registrar_unregistered,
            "cli_stopped": cli_stopped,
            "emulators_stopped": emulators_stopped,
            "scratch_removed": scratch_removed,
        }
        evidence["duration_seconds"] = round(time.time() - started, 3)
        log_path.write_text(
            mobile.redact_text(
                "\n".join(logs + ([] if harness is None else harness.service_stderr)),
                [mobile.MASTER_PASSWORD],
            ),
            encoding="utf-8",
        )
        evidence_path.write_text(
            json.dumps(evidence, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    print(f"Android discovered local-sync scenario: avd={avd} result={evidence['result']}")
    print(f"evidence={evidence_path}")
    return evidence["result"] == "PASS"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--results-dir", default="build/verification/mobile-local-sync"
    )
    args = parser.parse_args()
    if os.environ.get("HIDLINS_ANDROID_STRICT") != "1":
        raise SystemExit("HIDLINS_ANDROID_STRICT=1 is required; scenarios have no fallback")
    if lan.connected_emulators():
        raise SystemExit("Android mobile scenarios require no pre-connected emulators")
    results_dir = pathlib.Path(args.results_dir).resolve()
    results_dir.mkdir(parents=True, exist_ok=True)
    results_dir.chmod(0o700)
    binary = lan.build_authority()
    test_apk = lan.build_test_apk()
    subprocess.run(
        [sys.executable, str(ROOT / "tools/android-native/scenario_boundary.py")],
        cwd=ROOT,
        check=True,
    )
    emulator_version = lan.emulator_version()
    authority_rows = {
        (row[1], row[2]): row for row in selected_scenario_avds("authority")
    }
    replacement_rows = {
        (row[1], row[2]): row for row in selected_scenario_avds("replacement")
    }
    with tempfile.TemporaryDirectory(prefix="hidlins-android-shipping.") as temp:
        shipping_apk = pathlib.Path(temp) / "app-debug.apk"
        shutil.copy2(lan.APP_APK, shipping_apk)
        for row in selected_matrix():
            key = (row[1], row[2])
            if key not in authority_rows or key not in replacement_rows:
                raise RuntimeError(f"Android scenario has no authority images for {key}")
            if not run_entry(
                row,
                authority_row=authority_rows[key],
                replacement_row=replacement_rows[key],
                binary=binary,
                test_apk=test_apk,
                shipping_apk=shipping_apk,
                emulator_version=emulator_version,
                results_dir=results_dir,
            ):
                raise SystemExit(1)


if __name__ == "__main__":
    main()
