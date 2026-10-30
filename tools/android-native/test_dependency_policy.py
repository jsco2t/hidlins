#!/usr/bin/env python3
"""JUnit removal regressions, including a real offline Gradle resolution."""

import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import tempfile
import unittest
import xml.etree.ElementTree as ET
import zipfile

import verify_app_apk

ROOT = Path(__file__).resolve().parents[2]
ANDROID = ROOT / 'app/android'
POLICY = ANDROID / 'gradle/no-junit.init.gradle'
CASES = (
    'secureWindowAndSnapshotCoverAreInstalled',
    'clipboardClearIsOwnershipCheckedAndSensitive',
    'vaultImportIsAtomicAndAppOwned',
    'missingAndInvalidKeyfileReferencesFailClosed',
    'appBackupIsDisabled',
    'shippingApplicationCanReachConfiguredSyncStorage',
    'localDiscoveryPayloadIsBoundedAndContainsOnlyRoutes',
    'discoveryAttemptGenerationRejectsStoppedAndDuplicateCallbacks',
    'scenarioRegistrarAcceptsOnlyBoundedPortAndKnownServiceKind',
    'pickerCancellationAndActivityDestructionCompletePendingResults',
    'vaultImportReportsProviderPermissionDenial',
)


class DependencyPolicyTests(unittest.TestCase):
    def test_device_ui_diagnostics_emit_only_fixed_categories(self):
        with tempfile.TemporaryDirectory(prefix='hidlins-focus-diagnostic-') as scratch:
            adb = Path(scratch) / 'adb'
            adb.write_text('''#!/bin/bash
if [[ "$5" == window ]]; then
  printf '%s\\n' "$TEST_WINDOW"
  sleep 0.01
  for _ in {1..16}; do printf '%4096s\\n' 'ignored SECRET-CANARY'; done
  echo 'mCurrentFocus=Window{u0 app.hidlins/.MainActivity}'
else
  printf '%s\\n' "$TEST_POWER"
  echo 'mWakefulness=Asleep'
fi
''')
            adb.chmod(0o755)
            for window, power, expected in (
                ('mCurrentFocus=Window{u0 com.android.permissioncontroller/.GrantPermissionsActivity SECRET-CANARY}',
                 'mWakefulness=Awake SECRET-CANARY', 'focus-window=permission-dialog\nwakefulness=Awake\nerror-owner=none\n'),
                ('mCurrentFocus=SECRET-CANARY', 'mWakefulness=SECRET-CANARY',
                 'focus-window=other\nwakefulness=unknown\nerror-owner=none\n'),
                ('mCurrentFocus=Window{u0 Application Not Responding: com.android.systemui SECRET-CANARY}',
                 'mWakefulness=Awake',
                 'focus-window=error-dialog\nwakefulness=Awake\nerror-owner=system-ui\n'),
                ('mCurrentFocus=Window{u0 Application Error: app.hidlins SECRET-CANARY}',
                 'mWakefulness=Awake',
                 'focus-window=error-dialog\nwakefulness=Awake\nerror-owner=hidlins\n'),
                ('mCurrentFocus=Window{u0 Application Error: SECRET-CANARY}',
                 'mWakefulness=Awake',
                 'focus-window=error-dialog\nwakefulness=Awake\nerror-owner=other\n'),
            ):
                with self.subTest(window=window):
                    result = subprocess.run(
                        ['bash', '-c', 'set -euo pipefail; source "$1"; hidlins_device_ui_diagnostics "$2"',
                         'focus-fixture', str(ROOT / 'tools/android-native/instrumentation-result.sh'), str(adb)],
                        env=dict(os.environ, TEST_WINDOW=window, TEST_POWER=power),
                        capture_output=True, text=True, timeout=5)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(result.stdout, expected)
                    self.assertNotIn('SECRET-CANARY', result.stdout + result.stderr)

    def test_apk_inspection_rejects_forbidden_test_classes(self):
        valid = b'\0'.join(verify_app_apk.REQUIRED_CLASSES)
        for marker in (b'Lorg/junit/Test;', b'Ljunit/framework/TestCase;', b'Lorg/jacoco/agent/Agent;'):
            with self.subTest(marker=marker):
                with self.assertRaises(SystemExit):
                    verify_app_apk.validate_contents(
                        set(verify_app_apk.EXPECTED_ABIS), valid + b'\0' + marker, 'fixture')

    def test_native_result_requires_all_cases_and_successful_instrumentation(self):
        passed = 'INSTRUMENTATION_RESULT: stream=HIDLINS_DEVICE_TESTS 11 passed, 0 failed\n'
        status = 'INSTRUMENTATION_CODE: -1\n'
        cases = (
            (passed + status, True),
            ('', False),
            (passed, False),
            (status, False),
            (passed.replace('11 passed', '10 passed') + status, False),
            (passed.replace('0 failed', '1 failed') + status, False),
            (passed + 'INSTRUMENTATION_CODE: 0\n', False),
            (passed + status + 'INSTRUMENTATION_FAILED: crash\n', False),
            ('FAIL case\n' + passed + status, False),
        )
        helper = ROOT / 'tools/android-native/instrumentation-result.sh'
        for output, expected in cases:
            with self.subTest(output=output):
                result = subprocess.run(
                    ['bash', '-c', 'source "$1"; hidlins_device_tests_passed "$2"',
                     'check-result', str(helper), output], capture_output=True, text=True)
                self.assertEqual(result.returncode == 0, expected, result.stderr)

    def test_device_assertions_reject_failures_without_disclosing_values(self):
        with tempfile.TemporaryDirectory(prefix='hidlins-device-assertions-') as scratch:
            root = Path(scratch)
            runner = root / 'AssertionsProbe.java'
            runner.write_text('''package app.hidlins;
import static app.hidlins.DeviceAssertions.*;
public final class AssertionsProbe {
    public static void main(String[] args) {
        assertTrue(true); assertFalse(false); assertNotNull("value");
        assertEquals(null, null); assertEquals("same", "same");
        assertArrayEquals(new byte[]{1, 2}, new byte[]{1, 2});
        rejects(() -> assertTrue(false));
        rejects(() -> assertFalse(true));
        rejects(() -> assertNotNull(null));
        rejects(() -> assertEquals("secret-canary", "different"));
        rejects(() -> assertArrayEquals(new byte[]{1}, new byte[]{1, 2}));
        rejects(() -> assertArrayEquals(null, new byte[]{1}));
        AssertionError located = new AssertionError("secret-canary");
        located.setStackTrace(new StackTraceElement[]{
            new StackTraceElement("app.hidlins.DeviceAssertions", "assertTrue", "DeviceAssertions.java", 1),
            new StackTraceElement("app.hidlins.HidlinsAndroidTest", "caseName", "secret-canary", 123)});
        assertEquals("HidlinsAndroidTest.java:123", failureLocation(located));
        assertEquals("unknown", failureLocation(new AssertionError("secret-canary")));
    }
    private static void rejects(Runnable action) {
        boolean rejected = false;
        try { action.run(); }
        catch (AssertionError error) {
            rejected = true;
            if (error.getMessage().contains("secret-canary")) {
                throw new AssertionError("assertion leaked its operand");
            }
        }
        if (!rejected) { throw new AssertionError("failed condition was accepted"); }
    }
}''')
            java_home = Path(os.environ['JAVA_HOME'])
            compiled = subprocess.run(
                [str(java_home / 'bin/javac'), '-Xlint:all', '-Werror', '-d', str(root),
                 str(ANDROID / 'app/src/androidTest/java/app/hidlins/DeviceAssertions.java'), str(runner)],
                capture_output=True, text=True, timeout=30)
            self.assertEqual(compiled.returncode, 0, compiled.stdout + compiled.stderr)
            executed = subprocess.run(
                [str(java_home / 'bin/java'), '-cp', str(root), 'app.hidlins.AssertionsProbe'],
                capture_output=True, text=True, timeout=30)
            self.assertEqual(executed.returncode, 0, executed.stdout + executed.stderr)

    def test_native_instrumentation_timeout_fails_and_preserves_its_output(self):
        source = (ROOT / 'tools/android-native/run_app_tests.sh').read_text()
        reporting = source[source.index('"$ADB" install -r "$TEST_APK"'):]
        with tempfile.TemporaryDirectory(prefix='hidlins-instrumentation-timeout-') as scratch:
            root = Path(scratch)
            adb = root / 'adb'
            adb.write_text('''#!/bin/bash
printf '%s\\n' "$*" >> "$TEST_COMMAND_LOG"
if [[ "$1 $2 $3" != 'shell am instrument' ]]; then exit 0; fi
echo 'INSTRUMENTATION_STATUS: stream=PASS firstCase'
exec sleep 60
''')
            adb.chmod(0o755)
            env = dict(os.environ, ROOT=str(ROOT), ADB=str(adb), TEST_APK='fixture',
                       TEST_COMPONENT='app.hidlins.test/app.hidlins.HidlinsAndroidTest',
                       TEST_COMMAND_LOG=str(root / 'commands'),
                       HIDLINS_ANDROID_INSTRUMENTATION_TIMEOUT_SECONDS='0.1')
            process = subprocess.Popen(
                ['bash', '-c', 'set -euo pipefail\nsource "$ROOT/tools/android-native/instrumentation-result.sh"\n' + reporting],
                env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                start_new_session=True)
            try:
                output, error = process.communicate(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.communicate()
                self.fail('native instrumentation did not enforce its timeout')
            self.assertEqual(process.returncode, 124, output + error)
            self.assertIn('PASS firstCase', output)
            self.assertIn('timeout', output)
            commands = (root / 'commands').read_text().splitlines()
            self.assertLess(commands.index('shell input keyevent KEYCODE_WAKEUP'),
                            next(i for i, value in enumerate(commands) if value.startswith('shell am instrument')))
            self.assertLess(commands.index('shell wm dismiss-keyguard'),
                            next(i for i, value in enumerate(commands) if value.startswith('shell am instrument')))

    def test_no_junit_artifacts_are_admitted_or_locked(self):
        metadata = ET.parse(ANDROID / 'gradle/verification-metadata.xml')
        for component in metadata.iter():
            if component.tag.endswith('component'):
                self.assertNotIn('junit', component.get('group', '').lower())
                self.assertNotIn('junit', component.get('name', '').lower())
        self.assertNotIn('junit', (ANDROID / 'app/gradle.lockfile').read_text().lower())

    def test_device_launch_waits_for_its_own_activity_instead_of_a_previous_one(self):
        source = (ANDROID / 'app/src/androidTest/java/app/hidlins/HidlinsAndroidTest.java').read_text()
        launch = source[source.index('    private MainActivity launchActivity() {'):
                        source.index('    private static void runOnMain(')]
        with tempfile.TemporaryDirectory(prefix='hidlins-device-launch-') as scratch:
            root = Path(scratch)
            probe = root / 'LaunchProbe.java'
            probe.write_text('''public final class LaunchProbe {
    static final class Context { void startActivity(Intent intent) {} }
    static final class Intent {
        static final int FLAG_ACTIVITY_NEW_TASK = 1;
        Intent(Context context, Class<?> type) {}
        Intent addFlags(int flags) { return this; }
    }
    static final class MainActivity { boolean hasWindowFocus() { return true; } }
    static class Instrumentation {
        final MainActivity started = new MainActivity();
        boolean idle;
        Object startActivitySync(Intent intent) { return started; }
        void waitForIdleSync() { idle = true; }
    }
    static final class Runner extends Instrumentation {
        static final MainActivity previous = new MainActivity();
        Context context() { return new Context(); }
        static MainActivity awaitActivity() { return previous; }
''' + launch + '''
    }
    public static void main(String[] args) {
        Runner runner = new Runner();
        if (runner.launchActivity() != runner.started || !runner.idle) {
            throw new AssertionError("device case did not wait for its own activity");
        }
    }
}''')
            java_home = Path(os.environ['JAVA_HOME'])
            compiled = subprocess.run([str(java_home / 'bin/javac'), '-Xlint:all', '-Werror',
                                       '-d', str(root), str(probe)], capture_output=True,
                                      text=True, timeout=30)
            self.assertEqual(compiled.returncode, 0, compiled.stdout + compiled.stderr)
            result = subprocess.run([str(java_home / 'bin/java'), '-cp', str(root), 'LaunchProbe'],
                                    capture_output=True, text=True, timeout=5)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_epl_jacoco_artifacts_are_not_admitted_or_locked(self):
        self.assertNotIn('org.jacoco', (ANDROID / 'gradle/verification-metadata.xml').read_text())
        self.assertNotIn('org.jacoco', (ANDROID / 'app/gradle.lockfile').read_text())

    def test_all_existing_device_cases_use_platform_instrumentation(self):
        source = (ANDROID / 'app/src/androidTest/java/app/hidlins/HidlinsAndroidTest.java').read_text()
        self.assertNotIn('org.junit', source)
        self.assertNotIn('androidx.test', source)
        self.assertIn('extends Instrumentation', source)
        for name in CASES:
            self.assertIn('void ' + name + '()', source)
            self.assertIn('this::' + name, source)
        self.assertEqual(len(re.findall(r'runCase\("', source)), len(CASES))
        app_gradle = (ANDROID / 'app/build.gradle.kts').read_text()
        self.assertIn('testInstrumentationRunner = "app.hidlins.HidlinsAndroidTest"', app_gradle)

    def test_gradle_removes_direct_and_transitive_junit_in_all_builds(self):
        with tempfile.TemporaryDirectory(prefix='hidlins-gradle-policy-') as scratch:
            root = Path(scratch)
            repo = root / 'repository'
            artifact = repo / 'fixture/safe/1.0'
            artifact.mkdir(parents=True)
            jar = artifact / 'safe-1.0.jar'
            with zipfile.ZipFile(jar, 'w') as archive:
                archive.writestr('META-INF/MANIFEST.MF', 'Manifest-Version: 1.0\n\n')
            components = [('fixture', 'safe', '1.0', (jar, artifact / 'safe-1.0.pom'))]
            commons = (
                ('org.apache.commons', 'commons-compress', '1.27.1'),
                ('org.apache.commons', 'commons-lang3', '3.16.0'),
                ('commons-io', 'commons-io', '2.16.1'),
                ('commons-codec', 'commons-codec', '1.17.1'),
            )
            for group, name, version in commons:
                directory = repo / group.replace('.', '/') / name / version
                directory.mkdir(parents=True)
                archive_path = directory / f'{name}-{version}.jar'
                archive_path.write_bytes(jar.read_bytes())
                # A normal POM read must fail: this deliberately missing parent
                # represents the upstream parent's JUnit BOM import.
                (directory / f'{name}-{version}.pom').write_text(f'''<project>
<modelVersion>4.0.0</modelVersion><parent><groupId>org.apache.commons</groupId>
<artifactId>commons-parent</artifactId><version>72</version></parent>
<artifactId>{name}</artifactId><version>{version}</version></project>''')
                components.append((group, name, version, (archive_path,)))
            # The forbidden dependency deliberately has no files in this local
            # repository: success proves it was removed before fetching it.
            (artifact / 'safe-1.0.pom').write_text('''<project>
<modelVersion>4.0.0</modelVersion><groupId>fixture</groupId>
<artifactId>safe</artifactId><version>1.0</version>
<dependencies><dependency><groupId>junit</groupId><artifactId>junit</artifactId>
<version>4.13.2</version></dependency><dependency><groupId>org.apache.commons</groupId>
<artifactId>commons-compress</artifactId><version>1.27.1</version>
</dependency></dependencies></project>''')
            buildscript = f'''buildscript {{
    repositories {{ maven {{ url = uri({json.dumps(str(repo))}) }} }}
    dependencies {{ classpath 'fixture:safe:1.0' }}
}}
'''
            project_script = buildscript + f'''
repositories {{ maven {{ url = uri({json.dumps(str(repo))}) }} }}
configurations {{ probe }}
dependencies {{
    probe 'fixture:safe:1.0'
    probe 'junit:junit:4.13.2'
    probe platform('org.junit:junit-bom:5.10.2')
    probe 'org.junit.jupiter:junit-jupiter-api:5.10.2'
    probe 'org.jetbrains.kotlin:kotlin-test-junit5:2.2.21'
    probe 'org.jacoco:org.jacoco.core:0.8.14'
}}
tasks.register('verifyPolicy') {{
    doLast {{
        assert configurations.probe.files*.name.toSet() == [
            'safe-1.0.jar', 'commons-compress-1.27.1.jar', 'commons-codec-1.17.1.jar',
            'commons-io-2.16.1.jar', 'commons-lang3-3.16.0.jar'].toSet()
        println 'HIDLINS_POLICY_PASS'
    }}
}}
'''
            (root / 'settings.gradle').write_text(buildscript + "rootProject.name = 'policy-fixture'\nincludeBuild('included')\ninclude('integration_test')\n")
            (root / 'build.gradle').write_text(project_script)
            sdk_plugin = root / 'integration_test'
            sdk_plugin.mkdir()
            # Like Flutter's SDK integration plugin, this child redundantly
            # declares another AGP even though the root supplies its tooling.
            # It is deliberately unavailable: the child must not resolve it.
            (sdk_plugin / 'build.gradle').write_text(project_script.replace(
                "classpath 'fixture:safe:1.0'",
                "classpath 'fixture:safe:1.0'; classpath 'com.android.tools.build:gradle:8.11.0'"))
            included = root / 'included'
            included.mkdir()
            settings_repositories = f'''dependencyResolutionManagement {{
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {{ maven {{ url = uri({json.dumps(str(repo))}) }} }}
}}
'''
            (included / 'settings.gradle').write_text(
                buildscript + "rootProject.name = 'included-fixture'\n" + settings_repositories)
            (included / 'build.gradle').write_text(project_script.replace(
                f'repositories {{ maven {{ url = uri({json.dumps(str(repo))}) }} }}\nconfigurations',
                'configurations'))
            # Retain strict verification even in the fixture.
            verification = root / 'gradle/verification-metadata.xml'
            verification.parent.mkdir()
            entries = ''
            for group, name, version, files in components:
                entries += f'<component group="{group}" name="{name}" version="{version}">'
                entries += ''.join(
                    '<artifact name="' + file.name + '"><sha256 value="' +
                    hashlib.sha256(file.read_bytes()).hexdigest() + '"/></artifact>'
                    for file in files)
                entries += '</component>'
            verification.write_text('<verification-metadata><configuration>'
                                    '<verify-metadata>true</verify-metadata></configuration>'
                                    '<components>' + entries + '</components></verification-metadata>')
            command = [os.environ.get('HIDLINS_GRADLE_BIN', str(ROOT / 'tools/android-native/gradle.sh'))]
            if POLICY.exists():
                command += ['--init-script', str(POLICY)]
            command += ['--project-dir', str(root), '--offline', '--no-daemon',
                        '-Dhidlins.commons.repository=' + repo.as_uri(),
                        'verifyPolicy', ':included:verifyPolicy', ':integration_test:verifyPolicy']
            result = subprocess.run(command, cwd=ROOT, text=True, stdout=subprocess.PIPE,
                                    stderr=subprocess.STDOUT, timeout=180)
            self.assertEqual(result.returncode, 0, result.stdout)
            self.assertEqual(result.stdout.count('HIDLINS_POLICY_PASS'), 3, result.stdout)
            # Artifact-only metadata must not weaken the existing byte check.
            tampered = repo / 'commons-codec/commons-codec/1.17.1/commons-codec-1.17.1.jar'
            tampered.write_bytes(tampered.read_bytes() + b'tampered')
            rejected = subprocess.run(command, cwd=ROOT, text=True, stdout=subprocess.PIPE,
                                      stderr=subprocess.STDOUT, timeout=180)
            self.assertNotEqual(rejected.returncode, 0, rejected.stdout)
            self.assertIn('verification', rejected.stdout.lower(), rejected.stdout)


if __name__ == '__main__':
    unittest.main(verbosity=2)
