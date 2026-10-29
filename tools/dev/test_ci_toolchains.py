#!/usr/bin/env python3
"""Regressions for the CI SDK selection and Android NDK admission boundary."""

import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / '.github/workflows/ci.yml'
CONTRACT = json.loads((ROOT / 'tools/android-native/artifact-contract.json').read_text())


def job_text(name):
    workflow = WORKFLOW.read_text()
    jobs = re.split(r'^  ([a-z][a-z-]*):\s*$', workflow, flags=re.MULTILINE)
    return dict(zip(jobs[1::2], jobs[2::2]))[name]


class CiToolchainTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='hidlins-ci-toolchains-')
        self.addCleanup(temporary.cleanup)
        self.temp = Path(temporary.name)
        self.bin = self.temp / 'bin'
        self.bin.mkdir()
        for name in ('make', 'python3', 'bash', 'uname', 'dirname', 'grep'):
            (self.bin / name).symlink_to(shutil.which(name))
        self.sdk = self.temp / 'Android SDK'
        self.ndk = self.sdk / 'ndk' / CONTRACT['ndk_version']
        self.env = dict(os.environ, PATH=str(self.bin), ANDROID_SDK_ROOT=str(self.sdk))
        for name in ('ANDROID_HOME', 'ANDROID_NDK_HOME', 'SDKMANAGER'):
            self.env.pop(name, None)
        self.env['GITHUB_ENV'] = str(self.temp / 'github-env')
        self.env['GITHUB_OUTPUT'] = str(self.temp / 'github-output')
        self.env['TEST_TOOL_LOG'] = str(self.temp / 'tool.log')

    def executable(self, path, script):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text('#!/bin/bash\nset -eu\n' + script)
        path.chmod(0o755)

    def run_command(self, command):
        return subprocess.run(command, cwd=ROOT, env=self.env, capture_output=True,
                              text=True, timeout=20)

    def test_every_flutter_job_selects_the_repository_version(self):
        for name in ('flutter-app', 'android-rust', 'android-emulator'):
            with self.subTest(job=name):
                job = job_text(name)
                self.assertNotIn('flutter-version-file:', job)
                self.assertIn('id: flutter-version', job)
                self.assertIn('run: make ci-flutter-version', job)
                self.assertIn('flutter-version: ${{ steps.flutter-version.outputs.version }}', job)

    def test_flutter_output_matches_pin_and_preserves_prior_outputs(self):
        output = Path(self.env['GITHUB_OUTPUT'])
        output.write_text('previous=value\n')
        result = self.run_command(['make', '--no-print-directory', 'ci-flutter-version'])
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(output.read_text(), 'previous=value\nversion=' +
                         (ROOT / '.flutter-version').read_text().strip() + '\n')

    def test_ndk_ci_setup_finds_sdkmanager_outside_path_and_pins_build_root(self):
        self.executable(self.sdk / 'cmdline-tools/latest/bin/sdkmanager',
                        'printf "%s\\n" "$@" > "$TEST_TOOL_LOG"\n')
        self.env['ANDROID_NDK_HOME'] = str(self.temp / 'old-ndk')
        for name in ('android-rust', 'android-emulator'):
            with self.subTest(job=name):
                job = job_text(name)
                match = re.search(r'- name: Install exact Flutter NDK\s+run: ([^\n]+)', job)
                self.assertIsNotNone(match)
                result = self.run_command(['bash', '-c', match.group(1)])
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(Path(self.env['TEST_TOOL_LOG']).read_text().splitlines(),
                                 ['--sdk_root=' + str(self.sdk), 'ndk;' + CONTRACT['ndk_version']])
                self.assertIn('ANDROID_NDK_HOME=' + str(self.ndk) + '\n',
                              Path(self.env['GITHUB_ENV']).read_text())

    def check_ndk(self, revision_line):
        host = 'darwin-x86_64' if platform.system() == 'Darwin' else 'linux-x86_64'
        binary_dir = self.ndk / 'toolchains/llvm/prebuilt' / host / 'bin'
        for target in ('aarch64-linux-android', 'x86_64-linux-android'):
            self.executable(binary_dir / f'{target}{CONTRACT["android_api_level"]}-clang', 'exit 0\n')
        self.executable(binary_dir / 'llvm-ar', 'exit 0\n')
        (self.ndk / 'source.properties').write_text('Pkg.Desc = Android NDK\n' + revision_line + '\n')
        self.executable(self.bin / 'cargo', 'printf "%s\\n" "$*" >> "$TEST_TOOL_LOG"\n')
        return self.run_command(['bash', 'tools/android-native/build.sh', 'check'])

    def test_ndk_build_accepts_the_real_revision_property(self):
        result = self.check_ndk('Pkg.Revision = ' + CONTRACT['ndk_version'])
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        commands = Path(self.env['TEST_TOOL_LOG']).read_text().splitlines()
        self.assertEqual(len(commands), 2)
        self.assertIn('--target aarch64-linux-android', commands[0])
        self.assertIn('--target x86_64-linux-android', commands[1])

    def test_ndk_build_rejects_wrong_revision_before_cargo(self):
        for revision in ('28.2.136763580', '28x2x13676358', '27.0.0'):
            with self.subTest(revision=revision):
                result = self.check_ndk('Pkg.Revision = ' + revision)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(Path(self.env['TEST_TOOL_LOG']).exists())

    def test_ndk_build_does_not_accept_the_wrong_property_name(self):
        result = self.check_ndk('Pkg.BaseRevision = ' + CONTRACT['ndk_version'])
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(Path(self.env['TEST_TOOL_LOG']).exists())


if __name__ == '__main__':
    unittest.main(verbosity=2)
