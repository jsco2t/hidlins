#!/usr/bin/env python3
"""Run simulator integration tests without discovering the VM through logs.

Used by the existing iOS make targets. The pinned Flutter SDK starts unified
logging asynchronously alongside simctl launch, so the one-shot VM URL can be
missed. Build and launch explicitly, then attach the standard integration driver
to a verified loopback VM service. No application or SDK source is patched.
"""

from __future__ import annotations

import argparse
import http.client
import json
from pathlib import Path
import signal
import socket
import subprocess
import sys
import time


APP = Path(__file__).resolve().parents[2] / 'app'
BUNDLE = APP / 'build/ios/iphonesimulator/Runner.app'
IDENTIFIER = 'app.hidlins.ios'


def reserve_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
        listener.bind(('127.0.0.1', 0))
        return int(listener.getsockname()[1])


def wait_for_vm(port: int, timeout: float = 60) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        connection = http.client.HTTPConnection(
            '127.0.0.1', port, timeout=min(1, max(0.001, deadline - time.monotonic())))
        try:
            connection.request('GET', '/getVM')
            response = connection.getresponse()
            payload = json.loads(response.read(65536))
            if (response.status == 200 and isinstance(payload, dict)
                    and isinstance(payload.get('result'), dict)
                    and payload['result'].get('type') == 'VM'):
                return
        except (OSError, ValueError, http.client.HTTPException):
            pass
        finally:
            connection.close()
        time.sleep(min(0.1, max(0, deadline - time.monotonic())))
    raise TimeoutError('simulator VM service did not become ready')


def run(device: str, target: str, defines: list[str]) -> int:
    build = subprocess.run(
        ['flutter', 'build', 'ios', '--simulator', '--debug', '--no-pub',
         f'--target={target}', *defines], cwd=APP, check=False)
    if build.returncode:
        return build.returncode
    install = subprocess.run(
        ['xcrun', 'simctl', 'install', device, str(BUNDLE)], check=False)
    if install.returncode:
        return install.returncode

    port = reserve_port()
    try:
        launch = subprocess.run(
            ['xcrun', 'simctl', 'launch', '--terminate-running-process', device, IDENTIFIER,
             '--enable-dart-profiling', '--enable-checked-mode', '--verify-entry-points',
             f'--vm-service-port={port}', '--vm-service-host=127.0.0.1',
             # These runtime flags apply only to this disposable debug test
             # process. Keep the unauthenticated service on loopback, with no
             # Bonjour publication; release builds have no VM service.
             '--disable-service-auth-codes', '--disable-vm-service-publication'], check=False)
        if launch.returncode:
            return launch.returncode
        print('Waiting for simulator VM service on loopback...', flush=True)
        try:
            wait_for_vm(port)
        except TimeoutError:
            print('error: simulator VM service did not become ready', flush=True)
            return 124
        return subprocess.run(
            ['flutter', 'drive', '--no-pub', '-d', device,
             '--driver=test_driver/integration_test.dart', f'--target={target}',
             f'--use-existing-app=http://127.0.0.1:{port}/'],
            cwd=APP, check=False).returncode
    finally:
        # The driver keeps an explicitly attached app running. Terminate it on
        # success, assertion failure, startup failure, and timeout cancellation.
        try:
            subprocess.run(['xcrun', 'simctl', 'terminate', device, IDENTIFIER],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                           timeout=10, check=False)
        except (OSError, subprocess.TimeoutExpired):
            print('warning: unable to terminate simulator test app', file=sys.stderr)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('device')
    parser.add_argument('target')
    parser.add_argument('defines', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if any(not define.startswith('--dart-define=') for define in args.defines):
        parser.error('only --dart-define arguments may follow the test target')

    def interrupted(_signum: int, _frame: object) -> None:
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, interrupted)
    try:
        return run(args.device, args.target, args.defines)
    except KeyboardInterrupt:
        return 130
    except OSError:
        # Never echo command arguments: scenario defines carry control tokens.
        print('error: could not run simulator integration command', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
