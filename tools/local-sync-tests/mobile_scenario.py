#!/usr/bin/env python3
"""Drive a real mobile Hidlins client against a separate CLI authority."""

from __future__ import annotations

import argparse
import base64
from dataclasses import dataclass
import hashlib
import http.server
import ipaddress
import json
import os
import pathlib
import queue
import re
import secrets
import selectors
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time
from typing import Any


MASTER_PASSWORD = "integration-master-marker"
AUTHORITY_VAULT = "authority"
AUTHORITY_ENTRY = "authority-origin"
CLIENT_ENTRY = "mobile-origin"
SAS_PATTERN = re.compile(r"\b[0-9A-HJKMNP-TV-Z]{3}-[0-9A-HJKMNP-TV-Z]{3}\b")
SCRATCH_PATTERN = re.compile(r"/[^\s]*hidlins-mobile-sync\.[^\s/]*(?:/[^\s]*)?")
DISCOVERY_PATTERN = re.compile(
    r"HIDLINS_DISCOVERY_CANDIDATE=([^:\s]+):(\d+):(\d+)"
)
ALLOWED_NETWORKS = tuple(
    ipaddress.ip_network(value)
    for value in (
        "10.0.0.0/8",
        "172.16.0.0/12",
        "192.168.0.0/16",
        "169.254.0.0/16",
        "127.0.0.0/8",
        "fc00::/7",
        "fe80::/10",
        "::1/128",
    )
)
MIN_SHARED_WIFI_EMULATOR_VERSION = (36, 5, 0)


@dataclass(frozen=True)
class AndroidLanTopology:
    """Validated two-device Android virtual-LAN identity."""

    client: ipaddress.IPv4Address
    authority: ipaddress.IPv4Address
    network: ipaddress.IPv4Network


def _version_tuple(value: str) -> tuple[int, int, int]:
    match = re.fullmatch(r"(\d+)\.(\d+)(?:\.(\d+))?(?:\.\d+)?", value.strip())
    if match is None:
        raise ValueError("Android Emulator version is malformed")
    return tuple(int(part or 0) for part in match.groups())  # type: ignore[return-value]


def require_android_lan_topology(
    *,
    emulator_version: str,
    client_serial: str,
    authority_serial: str,
    client_address: str,
    authority_address: str,
    prefix_length: int,
) -> AndroidLanTopology:
    """Require two distinct private peers on Emulator 36.5+ shared Wi-Fi."""

    if _version_tuple(emulator_version) < MIN_SHARED_WIFI_EMULATOR_VERSION:
        raise ValueError("Android Emulator 36.5 or newer is required for shared Wi-Fi NSD")
    if not client_serial or client_serial == authority_serial:
        raise ValueError("Android LAN scenario requires two distinct emulator serials")
    try:
        client = ipaddress.ip_address(client_address)
        authority = ipaddress.ip_address(authority_address)
        client_interface = ipaddress.ip_interface(f"{client_address}/{prefix_length}")
        authority_interface = ipaddress.ip_interface(f"{authority_address}/{prefix_length}")
    except ValueError as error:
        raise ValueError("Android LAN addresses are malformed") from error
    if not isinstance(client, ipaddress.IPv4Address) or not isinstance(
        authority, ipaddress.IPv4Address
    ):
        raise ValueError("Android shared Wi-Fi scenario requires IPv4 peers")
    if client == authority or client_interface.network != authority_interface.network:
        raise ValueError("Android emulator peers must have distinct addresses on one subnet")
    for address in (client, authority):
        if (
            not any(address in network for network in ALLOWED_NETWORKS)
            or address.is_loopback
            or address.is_multicast
            or address.is_unspecified
        ):
            raise ValueError("Android emulator peer is outside the private LAN policy")
    return AndroidLanTopology(client, authority, client_interface.network)


def platform_route(platform: str) -> str:
    if platform == "ios":
        return "127.0.0.1"
    if platform == "android":
        return "10.0.2.2"
    raise ValueError(f"unsupported mobile platform: {platform}")


def scenario_defines(
    *,
    platform: str,
    phase: str,
    sync_port: int,
    control_port: int,
    token: str,
) -> list[str]:
    """Build scenario defines without exposing an Android sync data-plane route."""

    defines = [
        f"--dart-define=HIDLINS_SCENARIO_PHASE={phase}",
        f"--dart-define=HIDLINS_SCENARIO_CONTROL_HOST={platform_route(platform)}",
        f"--dart-define=HIDLINS_SCENARIO_CONTROL_PORT={control_port}",
        f"--dart-define=HIDLINS_SCENARIO_TOKEN={token}",
    ]
    if platform == "ios":
        defines.extend(
            (
                f"--dart-define=HIDLINS_SCENARIO_SYNC_HOST={platform_route(platform)}",
                f"--dart-define=HIDLINS_SCENARIO_SYNC_PORT={sync_port}",
            )
        )
    return defines


def require_local_endpoint(address: str, port: int) -> None:
    try:
        parsed = ipaddress.ip_address(address)
    except ValueError as error:
        raise ValueError("sync endpoint must be an IP literal") from error
    allowed = any(parsed in network for network in ALLOWED_NETWORKS)
    if not allowed or parsed.is_multicast or parsed.is_unspecified or not 0 < port < 65536:
        raise ValueError("sync endpoint is outside the local-only test policy")


def redact_text(value: str, sensitive: list[str]) -> str:
    redacted = SAS_PATTERN.sub("<redacted-sas>", value)
    redacted = SCRATCH_PATTERN.sub("<redacted-path>", redacted)
    for marker in sensitive:
        if marker:
            redacted = redacted.replace(marker, "<redacted-secret>")
    return redacted


def file_sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def flutter_output_passed(output: str) -> bool:
    return (
        "All tests passed" in output
        and "Some tests failed" not in output
        and "[E]" not in output
    )


def assert_discovered_routes(
    output: str, address: str, port: int
) -> list[dict[str, object]]:
    """Require the active authority among only allowed NSD candidates."""

    expected = (address, port, 0)
    routes = [
        (candidate, int(candidate_port), int(scope))
        for candidate, candidate_port, scope in DISCOVERY_PATTERN.findall(output)
    ]
    if not routes:
        raise ValueError("mobile scenario emitted no native discovery route")
    for route in routes:
        require_local_endpoint(route[0], route[1])
        if route[1] != port:
            raise ValueError("mobile scenario discovered an unexpected service port")
    if expected not in routes:
        raise ValueError("mobile scenario did not discover the active authority")
    return [
        {"address": route[0], "port": route[1], "scope_id": route[2]}
        for route in routes
    ]


def evidence_slug(value: str) -> str:
    slug = re.sub(r"[^a-zA-Z0-9._-]+", "-", value).strip("-.")
    if not slug or len(slug) > 96:
        raise ValueError("device identity is unsuitable for an evidence filename")
    return slug


def reserve_port() -> int:
    import socket

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
        listener.bind(("127.0.0.1", 0))
        return int(listener.getsockname()[1])


class AuthorityHarness:
    def __init__(self, cli: pathlib.Path, scratch: pathlib.Path, sync_port: int) -> None:
        self.cli = cli
        self.scratch = scratch
        self.registry = scratch / "vaults.toml"
        self.vault = scratch / "authority.kdbx"
        self.sync_port = sync_port
        self.service: subprocess.Popen[str] | None = None
        self.service_stderr: list[str] = []
        self._stderr_thread: threading.Thread | None = None
        self.lock = threading.Lock()

    def _base(self) -> list[str]:
        return [str(self.cli), "--registry", str(self.registry)]

    def run_cli(
        self,
        args: list[str],
        stdin: str = "",
        expected: tuple[int, ...] = (0,),
    ) -> subprocess.CompletedProcess[str]:
        environment = os.environ.copy()
        environment.pop("HIDLINS_MASTER_PASSWORD", None)
        environment.pop("HIDLINS_STATE_DIR", None)
        result = subprocess.run(
            self._base() + args,
            input=stdin,
            text=True,
            capture_output=True,
            timeout=120,
            env=environment,
            check=False,
        )
        if result.returncode not in expected:
            detail = redact_text(result.stdout + result.stderr, [MASTER_PASSWORD])
            raise RuntimeError(f"CLI command failed ({result.returncode}): {detail[-2000:]}")
        return result

    def create(self) -> None:
        self.run_cli(
            [
                "--format",
                "json",
                "vault",
                "create",
                "--id",
                AUTHORITY_VAULT,
                "--path",
                str(self.vault),
                "--no-recovery-warning",
            ],
            f"{MASTER_PASSWORD}\n{MASTER_PASSWORD}\n",
        )

    def _drain_stderr(self, stream: Any) -> None:
        for line in stream:
            self.service_stderr.append(redact_text(line, [MASTER_PASSWORD]))

    def start(self, pairing_answers: list[str] | None = None) -> None:
        if self.service is not None:
            raise RuntimeError("authority is already running")
        command = self._base() + [
            "--format",
            "json",
            "sync",
            "serve",
            "--vault",
            AUTHORITY_VAULT,
            "--address",
            "127.0.0.1",
            "--port",
            str(self.sync_port),
        ]
        if pairing_answers is not None:
            command.append("--pairing-window")
        self.service = subprocess.Popen(
            command,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        assert self.service.stdin is not None
        answers = "" if pairing_answers is None else "".join(f"{answer}\n" for answer in pairing_answers)
        self.service.stdin.write(f"{MASTER_PASSWORD}\n{answers}")
        self.service.stdin.flush()
        assert self.service.stdout is not None
        selector = selectors.DefaultSelector()
        selector.register(self.service.stdout, selectors.EVENT_READ)
        ready = selector.select(timeout=45)
        selector.close()
        if not ready:
            self.stop(force=True)
            raise RuntimeError("authority did not report readiness before timeout")
        line = self.service.stdout.readline()
        try:
            payload = json.loads(line)
        except json.JSONDecodeError as error:
            self.stop(force=True)
            raise RuntimeError("authority readiness was not JSON") from error
        expected = f"127.0.0.1:{self.sync_port}"
        if payload.get("status") != "serving" or payload.get("endpoint") != expected:
            self.stop(force=True)
            raise RuntimeError("authority reported an unexpected endpoint")
        assert self.service.stderr is not None
        self._stderr_thread = threading.Thread(
            target=self._drain_stderr,
            args=(self.service.stderr,),
            name="hidlins-mobile-authority-stderr",
            daemon=True,
        )
        self._stderr_thread.start()

    def stop(self, force: bool = False) -> None:
        service = self.service
        if service is None:
            return
        self.service = None
        if service.poll() is None:
            if force:
                service.kill()
            else:
                service.send_signal(signal.SIGINT)
            try:
                service.wait(timeout=15)
            except subprocess.TimeoutExpired:
                service.kill()
                service.wait(timeout=5)
                if not force:
                    raise RuntimeError("authority ignored SIGINT")
        if not force and service.returncode != 0:
            raise RuntimeError(f"authority exited with status {service.returncode}")
        if self._stderr_thread is not None:
            self._stderr_thread.join(timeout=2)
            self._stderr_thread = None

    def restart(self) -> None:
        self.stop()
        self.start()

    def switch_service(self, kind: str) -> None:
        # Desktop/iOS uses the CLI's normal Rust advertiser, which publishes
        # trusted service continuously and pairing service while the window is
        # open. Android overrides this on its authority-side test registrar.
        if kind not in {"pairing", "trusted"}:
            raise ValueError("invalid discovery service kind")

    def peers(self) -> list[dict[str, Any]]:
        result = self.run_cli(
            ["--format", "json", "sync", "peers", "list", "--vault", AUTHORITY_VAULT]
        )
        payload = json.loads(result.stdout)
        peers = payload.get("peers")
        if not isinstance(peers, list):
            raise RuntimeError("peer list response was malformed")
        return peers

    def authority_add(self) -> None:
        self.stop()
        try:
            self.run_cli(
                ["entry", "add", "--vault", AUTHORITY_VAULT, "--title", AUTHORITY_ENTRY],
                f"{MASTER_PASSWORD}\n",
            )
        finally:
            self.start()

    def _authority_has(self, title: str) -> bool:
        result = self.run_cli(
            ["--format", "json", "entry", "get", "--vault", AUTHORITY_VAULT, "--title", title],
            f"{MASTER_PASSWORD}\n",
            expected=(0, 1),
        )
        return result.returncode == 0

    def assert_client_write(self, expected: bool) -> None:
        self.stop()
        try:
            actual = self._authority_has(CLIENT_ENTRY)
            if actual != expected:
                raise RuntimeError(f"client write presence was {actual}, expected {expected}")
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
                    AUTHORITY_VAULT,
                    "--peer",
                    active[0]["peer"],
                ]
            )
            updated = self.peers()
            if len(updated) != 1 or not updated[0].get("revoked", False):
                raise RuntimeError("authority did not persist client revocation")
        finally:
            self.start()


class ControlServer(http.server.ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, harness: AuthorityHarness, token: str) -> None:
        self.harness = harness
        self.token = token
        self.failures: queue.Queue[str] = queue.Queue()
        self.snapshot: dict[str, str] | None = None
        self.snapshot_hashes: dict[str, str] | None = None
        super().__init__(("127.0.0.1", 0), ControlHandler)


class ControlHandler(http.server.BaseHTTPRequestHandler):
    server: ControlServer

    def log_message(self, _format: str, *_args: object) -> None:
        return

    def _reply(self, status: int, payload: dict[str, object]) -> None:
        encoded = json.dumps(payload, sort_keys=True).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler contract
        if self.headers.get("X-Hidlins-Scenario") != self.server.token:
            self._reply(403, {"ok": False})
            return
        if self.path == "/snapshot":
            if self.server.snapshot is None:
                self._reply(404, {"ok": False})
            else:
                self._reply(200, {"ok": True, **self.server.snapshot})
            return
        actions = {
            "/assert-no-peer": self._assert_no_peer,
            "/authority-add": self.server.harness.authority_add,
            "/assert-client-absent": lambda: self.server.harness.assert_client_write(False),
            "/assert-client-present": lambda: self.server.harness.assert_client_write(True),
            "/service/trusted": lambda: self.server.harness.switch_service("trusted"),
            "/restart": self.server.harness.restart,
            "/revoke": self.server.harness.revoke,
        }
        action = actions.get(self.path)
        if action is None:
            self._reply(404, {"ok": False})
            return
        try:
            with self.server.harness.lock:
                action()
            self._reply(200, {"ok": True})
        except Exception as error:  # fail closed across the test-only control boundary
            detail = redact_text(str(error), [MASTER_PASSWORD])
            self.server.failures.put(detail)
            self._reply(500, {"ok": False})

    def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler contract
        if self.headers.get("X-Hidlins-Scenario") != self.server.token:
            self._reply(403, {"ok": False})
            return
        if self.path != "/snapshot":
            self._reply(404, {"ok": False})
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
            if not 0 < length <= 32 * 1024 * 1024:
                raise ValueError("snapshot length is outside the test bound")
            payload = json.loads(self.rfile.read(length))
            if set(payload) != {"registry", "vault"} or not all(
                isinstance(payload[key], str) for key in ("registry", "vault")
            ):
                raise ValueError("snapshot payload is malformed")
            decoded = {
                key: base64.b64decode(payload[key], validate=True)
                for key in ("registry", "vault")
            }
            if not decoded["registry"] or len(decoded["vault"]) < 32:
                raise ValueError("snapshot payload is incomplete")
            self.server.snapshot = payload
            self.server.snapshot_hashes = {
                key: hashlib.sha256(value).hexdigest() for key, value in decoded.items()
            }
            self._reply(200, {"ok": True})
        except Exception as error:
            detail = redact_text(str(error), [MASTER_PASSWORD])
            self.server.failures.put(detail)
            self._reply(400, {"ok": False})

    def _assert_no_peer(self) -> None:
        active = [peer for peer in self.server.harness.peers() if not peer.get("revoked", False)]
        if active:
            raise RuntimeError("rejected pairing left an active peer")


def run_flutter(
    repo: pathlib.Path,
    platform: str,
    device: str,
    phase: str,
    sync_port: int,
    control_port: int,
    token: str,
) -> str:
    defines = scenario_defines(
        platform=platform,
        phase=phase,
        sync_port=sync_port,
        control_port=control_port,
        token=token,
    )
    target = "integration_test/mobile_local_sync_scenario_test.dart"
    if platform == "ios":
        command = ["flutter", "test", "--no-pub", "-d", device, *defines, target]
    else:
        command = [
            "flutter",
            "drive",
            "--no-pub",
            "-d",
            device,
            "--timeout=240",
            "--driver=test_driver/integration_test.dart",
            f"--target={target}",
            "--android-project-arg=target-platform=android-arm64,android-x64",
            *defines,
        ]
    result = subprocess.run(
        command,
        cwd=repo / "app",
        text=True,
        capture_output=True,
        timeout=420,
        check=False,
    )
    output = redact_text(result.stdout + result.stderr, [MASTER_PASSWORD, token])
    if result.returncode != 0 or not flutter_output_passed(output):
        raise RuntimeError(f"Flutter {platform} phase {phase} failed:\n{output[-6000:]}")
    return output


def git_revision(repo: pathlib.Path) -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=repo, text=True, capture_output=True, check=True
    )
    return result.stdout.strip()


def run_scenario(args: argparse.Namespace) -> int:
    repo = pathlib.Path(__file__).resolve().parents[2]
    cli = pathlib.Path(args.cli).resolve()
    if not cli.is_file():
        raise ValueError(f"CLI artifact does not exist: {cli}")
    route = platform_route(args.platform)
    sync_port = reserve_port()
    require_local_endpoint(route, sync_port)
    results_dir = pathlib.Path(args.results_dir).resolve()
    results_dir.mkdir(parents=True, exist_ok=True)
    results_dir.chmod(0o700)
    evidence_id = evidence_slug(args.device_name)
    evidence_path = results_dir / f"{args.platform}-{evidence_id}.json"
    log_path = results_dir / f"{args.platform}-{evidence_id}.log"
    started = time.time()
    evidence: dict[str, object] = {
        "schema_version": 1,
        "scenario": "mobile-client-cli-authority",
        "platform": args.platform,
        "device": {"id": args.device, "name": args.device_name, "runtime": args.runtime},
        "git_revision": git_revision(repo),
        "cli_sha256": file_sha256(cli),
        "route": route,
        "steps": [],
        "result": "FAIL",
    }
    logs: list[str] = []
    scratch_path = pathlib.Path(tempfile.mkdtemp(prefix="hidlins-mobile-sync."))
    harness = AuthorityHarness(cli, scratch_path, sync_port)
    control: ControlServer | None = None
    control_thread: threading.Thread | None = None
    exit_code = 1
    previous_sigint = signal.getsignal(signal.SIGINT)
    previous_sigterm = signal.getsignal(signal.SIGTERM)

    def interrupted(_signum: int, _frame: object) -> None:
        raise KeyboardInterrupt

    signal.signal(signal.SIGINT, interrupted)
    signal.signal(signal.SIGTERM, interrupted)
    try:
        harness.create()
        harness.start(["no", "yes"])
        token = secrets.token_hex(24)
        control = ControlServer(harness, token)
        control_thread = threading.Thread(target=control.serve_forever, daemon=True)
        control_thread.start()
        control_port = int(control.server_address[1])
        require_local_endpoint(route, control_port)
        phase_steps = {
            "pair": (
                "mobile-server-rejected",
                "pairing-rejected-without-trust",
                "pairing-accepted-and-imported",
                "startup-sync",
                "authority-to-client-change",
                "save-does-not-sync",
                "manual-client-to-authority-sync",
            ),
            "restart": (
                "client-process-restart",
                "pinned-trust-reconnect",
                "authority-restart",
                "revocation-fails-closed",
            ),
        }
        for phase in ("pair", "restart"):
            logs.append(run_flutter(repo, args.platform, args.device, phase, sync_port, control_port, token))
            for step in phase_steps[phase]:
                evidence["steps"].append({"name": step, "result": "PASS"})  # type: ignore[index]
            if not control.failures.empty():
                raise RuntimeError(control.failures.get_nowait())
        if control.snapshot_hashes is None:
            raise RuntimeError("mobile client did not preserve a restart snapshot")
        evidence["restart_state_transport"] = "encrypted-kdbx-and-sealed-registry"
        evidence["restart_snapshot_sha256"] = control.snapshot_hashes
        artifact = pathlib.Path(args.artifact).resolve()
        if not artifact.is_file():
            raise RuntimeError(f"mobile artifact was not produced: {artifact}")
        evidence["app_artifact_sha256"] = file_sha256(artifact)
        evidence["result"] = "PASS"
        exit_code = 0
    except Exception as error:
        detail = redact_text(str(error), [MASTER_PASSWORD])
        if control is not None and not control.failures.empty():
            detail = f"{detail}\ncontrol: {control.failures.get_nowait()}"
        evidence["failure"] = detail
    except KeyboardInterrupt:
        evidence["failure"] = "scenario interrupted"
        exit_code = 130
    finally:
        if control is not None:
            control.shutdown()
            control.server_close()
        if control_thread is not None:
            control_thread.join(timeout=5)
        try:
            harness.stop(force=evidence["result"] != "PASS")
        except Exception as error:
            evidence["result"] = "FAIL"
            evidence["cleanup_failure"] = redact_text(str(error), [MASTER_PASSWORD])
        shutil.rmtree(scratch_path, ignore_errors=True)
        evidence["duration_seconds"] = round(time.time() - started, 3)
        log_path.write_text(redact_text("\n".join(logs + harness.service_stderr), [MASTER_PASSWORD]), encoding="utf-8")
        evidence_path.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(f"mobile local-sync scenario: platform={args.platform} result={evidence['result']}")
        print(f"evidence={evidence_path}")
        signal.signal(signal.SIGINT, previous_sigint)
        signal.signal(signal.SIGTERM, previous_sigterm)
    if evidence["result"] != "PASS" and exit_code == 0:
        return 1
    return exit_code


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("platform", choices=("ios", "android"))
    parser.add_argument("--device", required=True)
    parser.add_argument("--device-name", required=True)
    parser.add_argument("--runtime", required=True)
    parser.add_argument("--artifact", required=True)
    parser.add_argument("--cli", default="target/release/hidlins")
    parser.add_argument("--results-dir", default="build/verification/mobile-local-sync")
    return parser.parse_args()


if __name__ == "__main__":
    try:
        raise SystemExit(run_scenario(parse_args()))
    except (ValueError, subprocess.TimeoutExpired) as error:
        print(f"error: {redact_text(str(error), [MASTER_PASSWORD])}", file=sys.stderr)
        raise SystemExit(2) from error
