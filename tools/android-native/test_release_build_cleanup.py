#!/usr/bin/env python3
"""Actual descendant-process regressions for the canonical build owner."""
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('release_jni', Path(__file__).with_name('release_jni.py'))
PROBE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROBE)
REAL_POPEN = subprocess.Popen


def alive(pid):
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    state = Path(f'/proc/{pid}/stat')
    if state.exists():
        try:
            return state.read_text().split(') ', 1)[1].split()[0] != 'Z'
        except (FileNotFoundError, ProcessLookupError):
            return False
    result = subprocess.run(['ps', '-p', str(pid), '-o', 'stat='], text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=2)
    return result.returncode == 0 and not result.stdout.strip().startswith('Z')


def wait_until(check, seconds=5):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if check():
            return True
        time.sleep(0.02)
    return check()


class BuildOwnershipTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix='hidlins-build-owner-')
        self.root = Path(self.scratch.name)
        self.driver = None
        worker = ("import signal,time;from pathlib import Path;"
                  "signal.signal(signal.SIGTERM,signal.SIG_IGN);"
                  f"Path({str(self.root / 'ready')!r}).write_text('ready');time.sleep(300)")
        make = self.root / 'make'
        make.write_text(f'#!{sys.executable}\n' +
                        'import json,os,subprocess,sys,time\nfrom pathlib import Path\n' +
                        f'child=subprocess.Popen([sys.executable,"-c",{worker!r}])\n' +
                        f'Path({str(self.root / "pids.json")!r}).write_text(json.dumps([os.getpid(),child.pid]))\n' +
                        'time.sleep(300)\n')
        make.chmod(0o700)
        self.env = dict(os.environ, PATH=str(self.root) + os.pathsep + os.environ.get('PATH', ''))
        self.policy = self.root / '.gradle/android/init.d/hidlins-release-jni-offline.gradle'

    def tearDown(self):
        # These PIDs were created exclusively by this temporary fixture. Always
        # retire the deliberately stubborn child, including on an expected red.
        records = self.root / 'pids.json'
        if records.exists():
            for pid in json.loads(records.read_text()):
                if alive(pid):
                    try:
                        os.kill(pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
        if self.driver is not None:
            if self.driver.poll() is None:
                self.driver.kill()
            self.driver.wait(timeout=5)
        self.scratch.cleanup()

    def descendants_stopped(self):
        records = self.root / 'pids.json'
        self.assertTrue(records.exists(), 'fake build did not start')
        pids = json.loads(records.read_text())
        self.assertTrue(wait_until(lambda: all(not alive(pid) for pid in pids)),
                        'build descendant survived cancellation')
        self.assertFalse(self.policy.exists(), 'offline policy was not retired')

    def test_timeout_stops_the_build_and_its_stubborn_descendant(self):
        class ShortWait(REAL_POPEN):
            def wait(self, timeout=None):
                return super().wait(timeout=min(timeout, 1) if timeout is not None else None)

        with patch.object(PROBE, 'ROOT', self.root), patch.object(PROBE.subprocess, 'Popen', ShortWait):
            with self.assertRaises(subprocess.TimeoutExpired):
                PROBE.build_shipping(self.env)
        self.assertTrue((self.root / 'ready').exists(), 'descendant did not install its signal handler')
        self.descendants_stopped()

    def test_sigterm_stops_the_build_and_its_stubborn_descendant(self):
        code = (
            'import importlib.util,os,signal;from pathlib import Path;'
            f's=importlib.util.spec_from_file_location("probe",{PROBE.__file__!r});'
            'm=importlib.util.module_from_spec(s);s.loader.exec_module(m);'
            f'm.ROOT=Path({str(self.root)!r});'
            'signal.signal(signal.SIGTERM,lambda *_:(_ for _ in ()).throw(KeyboardInterrupt()));'
            'm.build_shipping(dict(os.environ))'
        )
        self.driver = REAL_POPEN([sys.executable, '-B', '-c', code], env=self.env,
                                 stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                 start_new_session=True)
        self.assertTrue(wait_until(lambda: (self.root / 'ready').exists()), 'fake build did not become ready')
        self.driver.terminate()
        self.driver.wait(timeout=10)
        self.descendants_stopped()


if __name__ == '__main__':
    unittest.main(verbosity=2)
