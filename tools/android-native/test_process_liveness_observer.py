#!/usr/bin/env python3
"""Deterministic controls for the real owned-build fixture's liveness observer."""
import errno
import importlib.util
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    'build_cleanup_fixture', Path(__file__).with_name('test_release_build_cleanup.py')
)
FIXTURE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FIXTURE)


class ProcessLivenessObserverTests(unittest.TestCase):
    def observe(self, text='123 (fixture) S 0', error=None):
        with patch.object(FIXTURE.os, 'kill'), \
                patch.object(FIXTURE.Path, 'exists', return_value=True), \
                patch.object(FIXTURE.Path, 'read_text', return_value=text, side_effect=error):
            return FIXTURE.alive(123)

    def test_disappearance_during_proc_read_is_not_alive(self):
        for missing in (FileNotFoundError(errno.ENOENT, 'fixture exited'),
                        ProcessLookupError(errno.ESRCH, 'fixture exited')):
            with self.subTest(error=type(missing).__name__):
                self.assertFalse(self.observe(error=missing))

    def test_live_process_is_still_alive(self):
        self.assertTrue(self.observe())

    def test_zombie_process_is_not_alive(self):
        self.assertFalse(self.observe(text='123 (fixture) Z 0'))

    def test_missing_pid_is_not_alive(self):
        with patch.object(FIXTURE.os, 'kill', side_effect=ProcessLookupError):
            self.assertFalse(FIXTURE.alive(123))

    def test_permission_error_is_not_hidden(self):
        with self.assertRaises(PermissionError):
            self.observe(error=PermissionError(errno.EPERM, 'fixture denied'))

    def test_malformed_proc_state_is_not_hidden(self):
        with self.assertRaises(IndexError):
            self.observe(text='malformed fixture')

    def test_non_proc_platform_fallback_preserves_live_and_dead_states(self):
        for status, output, expected in [(0, 'S\n', True), (0, 'Z\n', False), (1, '', False)]:
            with self.subTest(status=status, output=output):
                result = subprocess.CompletedProcess(['ps'], status, output)
                with patch.object(FIXTURE.os, 'kill'), \
                        patch.object(FIXTURE.Path, 'exists', return_value=False), \
                        patch.object(FIXTURE.subprocess, 'run', return_value=result):
                    self.assertEqual(FIXTURE.alive(123), expected)


if __name__ == '__main__':
    unittest.main(verbosity=2)
