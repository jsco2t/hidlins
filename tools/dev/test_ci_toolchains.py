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
import tomllib
import unittest
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / '.github/workflows/ci.yml'
CONTRACT = json.loads((ROOT / 'tools/android-native/artifact-contract.json').read_text())


def job_text(name):
    workflow = WORKFLOW.read_text()
    jobs = re.split(r'^  ([a-z][a-z-]*):\s*$', workflow, flags=re.MULTILINE)
    return dict(zip(jobs[1::2], jobs[2::2]))[name]


class CiToolchainTests(unittest.TestCase):
    def test_eclipse_licenses_are_explicitly_forbidden_without_exceptions(self):
        guidance = (ROOT / 'AGENTS.md').read_text()
        forbidden = next(line for line in guidance.splitlines() if line.startswith('- **Forbidden:**'))
        for license_id in ('EPL-1.0', 'EPL-2.0'):
            self.assertIn(license_id, forbidden)
            for file in ('deny.toml', 'fuzz/deny.toml'):
                with self.subTest(license=license_id, policy=file):
                    policy = tomllib.loads((ROOT / file).read_text())['licenses']
                    allowed = policy['allow'] + [value for exception in policy.get('exceptions', [])
                                                 for value in exception.get('allow', [])]
                    self.assertFalse(any(license_id in value for value in allowed))

    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='hidlins-ci-toolchains-')
        self.addCleanup(temporary.cleanup)
        # macOS temporary directories can be reached through /var symlinks;
        # compare the same canonical SDK paths that provisioning exports.
        self.temp = Path(temporary.name).resolve()
        self.bin = self.temp / 'bin'
        self.bin.mkdir()
        for name in ('make', 'python3', 'bash', 'uname', 'dirname', 'grep', 'mkdir', 'sed', 'rm',
                     'cp', 'mktemp'):
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

    def test_every_flutter_job_bootstraps_sdk_tools_before_app_commands(self):
        for name in ('flutter-app', 'android-rust', 'android-emulator'):
            with self.subTest(job=name):
                job = job_text(name)
                self.assertIn('run: make flutter-sdk-prepare', job)
                self.assertLess(job.index('uses: subosito/flutter-action@'),
                                job.index('run: make flutter-sdk-prepare'))
                self.assertLess(job.index('run: make flutter-sdk-prepare'),
                                job.index('- name: Disable telemetry'))

    def test_flutter_sdk_bootstrap_is_outside_reviewed_pub_cache(self):
        self.env['HOME'] = str(self.temp / 'home')
        self.executable(self.bin / 'flutter',
                        'printf "%s|%s\\n" "$PUB_CACHE" "$*" >> "$TEST_TOOL_LOG"\n')
        result = self.run_command(['make', '--no-print-directory', 'flutter-sdk-prepare'])
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = [line.split('|', 1) for line in
                 Path(self.env['TEST_TOOL_LOG']).read_text().splitlines()]
        # --version alone never validates the SDK's shipped package paths.
        # Its lazy PubDependencies cache update must precede app commands too.
        self.assertIn('precache --universal', [args for _, args in calls])
        caches = [cache for cache, _ in calls]
        self.assertEqual(len(set(caches)), 1)
        cache = Path(caches[0]).resolve()
        self.assertFalse(cache.is_relative_to(ROOT))
        self.assertEqual(cache.name, (ROOT / '.flutter-version').read_text().strip())

    def test_pinned_flutter_gradle_metadata_is_strictly_verified(self):
        metadata = ET.parse(ROOT / 'app/android/gradle/verification-metadata.xml')
        ns = {'v': 'https://schema.gradle.org/dependency-verification'}
        self.assertEqual(metadata.findtext('v:configuration/v:verify-metadata', namespaces=ns),
                         'true')
        # Flutter 3.47.2's included Gradle build resolves these BOMs before
        # selecting the plugin classpath. Keep its metadata inside verification.
        artifacts = (
            ('org.jetbrains.kotlin', 'kotlin-gradle-plugins-bom', '2.2.20', 'module'),
            ('org.jetbrains.kotlin', 'kotlin-gradle-plugins-bom', '2.2.20', 'pom'),
            ('org.jetbrains.kotlinx', 'kotlinx-coroutines-bom', '1.8.0', 'pom'),
            ('com.google.guava', 'guava-parent', '33.3.1-jre', 'pom'),
        )
        for group, name, version, extension in artifacts:
            with self.subTest(artifact=(name, version, extension)):
                path = (f'v:components/v:component[@group="{group}"]'
                        f'[@name="{name}"][@version="{version}"]/'
                        f'v:artifact[@name="{name}-{version}.{extension}"]/v:sha256')
                checksum = metadata.find(path, ns)
                self.assertIsNotNone(checksum, f'missing verified metadata: {path}')
                self.assertRegex(checksum.get('value', ''), r'^[0-9a-f]{64}$')

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

    def test_gradle_wrapper_uses_one_default_or_explicit_project_directory(self):
        self.executable(self.bin / 'flutter', 'exit 0\n')
        wrapper = self.temp / 'bin/cache/artifacts/gradle_wrapper/gradle/wrapper/gradle-wrapper.jar'
        wrapper.parent.mkdir(parents=True)
        wrapper.touch()
        self.env['JAVA_HOME'] = str(self.temp / 'jdk')
        self.executable(Path(self.env['JAVA_HOME']) / 'bin/java',
                        'printf "%s\\n" "$@" > "$TEST_TOOL_LOG"\n')
        for project, arguments in ((ROOT / 'app/android', []),
                                   (self.temp / 'fixture project', ['--project-dir',
                                                                  str(self.temp / 'fixture project')])):
            with self.subTest(arguments=arguments):
                result = self.run_command(['bash', 'tools/android-native/gradle.sh',
                                           *arguments, 'help'])
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                argv = Path(self.env['TEST_TOOL_LOG']).read_text().splitlines()
                indexes = [i for i, arg in enumerate(argv) if arg in ('-p', '--project-dir')]
                self.assertEqual(len(indexes), 1, argv)
                self.assertEqual(argv[indexes[0] + 1], str(project))
                self.assertEqual(argv[-1], 'help')

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
