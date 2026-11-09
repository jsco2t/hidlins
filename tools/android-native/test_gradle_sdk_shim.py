#!/usr/bin/env python3
"""Keep Gradle SDK lookup correct for ordinary executable symlink shims."""
import os
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools/dev'))
import test_ci_toolchains as ci


class GradleSdkTests(unittest.TestCase):
    def setUp(self):
        self.fixture = ci.CiToolchainTests(
            methodName='test_gradle_wrapper_uses_one_default_or_explicit_project_directory')
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.temp = self.fixture.temp
        self.env = self.fixture.env
        self.env['TMPDIR'] = str(self.temp)
        self.env['JAVA_HOME'] = str(self.temp / 'jdk')
        self.fixture.executable(Path(self.env['JAVA_HOME']) / 'bin/java',
                                'printf "%s\\n" "$@" > "$TEST_TOOL_LOG"\n')
        self.real_sdk = self.temp / 'actual Flutter SDK'
        self.flutter = self.real_sdk / 'bin/flutter'
        self.fixture.executable(self.flutter, 'exit 0\n')
        wrapper = self.real_sdk / 'bin/cache/artifacts/gradle_wrapper/gradle/wrapper/gradle-wrapper.jar'
        wrapper.parent.mkdir(parents=True)
        wrapper.write_bytes(b'fixture wrapper; fake Java records arguments only')
        self.shim = self.fixture.bin / 'flutter'
        self.capture = Path(self.env['TEST_TOOL_LOG'])

    def run_wrapper(self, *arguments):
        return self.fixture.run_command(['bash', 'tools/android-native/gradle.sh', *arguments])

    def assert_success(self, result, arguments):
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        argv = self.capture.read_text().splitlines()
        self.assertEqual(argv[:5], ['-classpath', 'gradle/wrapper/gradle-wrapper.jar',
                                   'org.gradle.wrapper.GradleWrapperMain', '--no-daemon',
                                   '--stacktrace'])
        self.assertEqual(argv[5:], arguments)

    def test_absolute_sdk_symlink_keeps_default_project_and_arguments(self):
        self.shim.symlink_to(self.flutter)
        self.assert_success(self.run_wrapper('help', '--offline'),
                            ['-p', str(ROOT / 'app/android'), 'help', '--offline'])

    def test_relative_symlink_chain_and_spaces_keep_explicit_project(self):
        intermediate = self.temp / 'shim directory/intermediate'
        intermediate.parent.mkdir()
        intermediate.symlink_to(os.path.relpath(self.flutter, intermediate.parent))
        self.shim.symlink_to(os.path.relpath(intermediate, self.shim.parent))
        project = str(self.temp / 'explicit project')
        self.assert_success(self.run_wrapper('--project-dir', project, 'help', '--offline'),
                            ['--project-dir', project, 'help', '--offline'])

    def test_direct_sdk_entrypoint_remains_supported(self):
        # The existing fixture represents a real executable at <sdk>/bin/flutter.
        self.fixture.test_gradle_wrapper_uses_one_default_or_explicit_project_directory()

    def test_missing_flutter_fails_without_invoking_java(self):
        result = self.run_wrapper('help')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.capture.exists())
        self.assertTrue(result.stderr.strip())

    def test_broken_flutter_symlink_fails_without_invoking_java(self):
        self.shim.symlink_to(self.temp / 'missing SDK/bin/flutter')
        result = self.run_wrapper('help')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.capture.exists())
        self.assertTrue(result.stderr.strip())


if __name__ == '__main__':
    unittest.main()
