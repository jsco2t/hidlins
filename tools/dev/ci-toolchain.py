#!/usr/bin/env python3
"""Select repository-pinned SDKs for CI without relying on runner PATH defaults."""

import argparse
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def append_environment(name, key, value):
    if destination := os.environ.get(name):
        with Path(destination).open('a') as output:
            output.write(f'{key}={value}\n')


def flutter_version():
    version = (ROOT / '.flutter-version').read_text().strip()
    if not re.fullmatch(r'\d+\.\d+\.\d+', version):
        raise ValueError('.flutter-version must contain an exact stable SDK version')
    append_environment('GITHUB_OUTPUT', 'version', version)
    print(version)


def android_ndk():
    configured = os.environ.get('ANDROID_SDK_ROOT') or os.environ.get('ANDROID_HOME')
    if not configured:
        raise ValueError('set ANDROID_SDK_ROOT or ANDROID_HOME to the Android SDK directory')
    sdk = Path(configured).resolve()
    manager = Path(os.environ.get('SDKMANAGER') or sdk / 'cmdline-tools/latest/bin/sdkmanager')
    if not manager.is_file() or not os.access(manager, os.X_OK):
        raise ValueError(f'Android SDK command-line tools are missing: {manager}')
    contract = json.loads((ROOT / 'tools/android-native/artifact-contract.json').read_text())
    version = contract['ndk_version']
    subprocess.run([str(manager), f'--sdk_root={sdk}', f'ndk;{version}'], check=True)
    ndk = sdk / 'ndk' / version
    append_environment('GITHUB_ENV', 'ANDROID_NDK_HOME', ndk)
    print(f'Pinned Android NDK: {ndk}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=['flutter-version', 'android-ndk'])
    arguments = parser.parse_args()
    try:
        if arguments.operation == 'flutter-version':
            flutter_version()
        else:
            android_ndk()
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'error: {error}\n')
