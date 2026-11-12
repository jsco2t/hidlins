#!/usr/bin/env python3
"""Closed Ratatui patch replay tests and its owned-terminal release benchmark."""

from __future__ import annotations

import errno
import fcntl
import os
import pty
import select
import signal
import struct
import sys
import termios
import time


def render_benchmark(argv: list[str]) -> int:
    """Provide the real cursor-query environment required by Terminal.clear.

    Rendering itself targets io::Sink; only nonsensitive test diagnostics and
    Crossterm's cursor-position queries pass through this owned terminal.
    Nothing is written to a recording file or an external terminal process.
    """
    pid, master = pty.fork()
    if pid == 0:
        fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
        os.execvp(argv[0], argv)
    status = None
    pending = b""
    deadline = time.monotonic() + 850
    try:
        while time.monotonic() < deadline:
            if select.select([master], [], [], 0.02)[0]:
                try:
                    chunk = os.read(master, 65536)
                except OSError as error:
                    if error.errno != errno.EIO:
                        raise
                    break
                if not chunk:
                    break
                pending += chunk
                while b"\x1b[6n" in pending:
                    before, pending = pending.split(b"\x1b[6n", 1)
                    sys.stdout.buffer.write(before)
                    os.write(master, b"\x1b[1;1R")
                if len(pending) > 3:
                    sys.stdout.buffer.write(pending[:-3])
                    pending = pending[-3:]
                sys.stdout.buffer.flush()
        else:
            raise TimeoutError("owned render benchmark exceeded 850 seconds")
        _, status = os.waitpid(pid, 0)
        sys.stdout.buffer.write(pending)
        sys.stdout.buffer.flush()
        return os.waitstatus_to_exitcode(status)
    finally:
        if status is None:
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            os.waitpid(pid, 0)
        os.close(master)


if __name__ == "__main__" and sys.argv[1:2] == ["--render-benchmark"]:
    raise SystemExit(render_benchmark(sys.argv[2:]))

# The tests below exercise the maintained transformation on isolated copies;
# they never mutate the production vendor tree or resolve/download packages.
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("hidlins_ratatui_vendor", ROOT / "tools/dev/vendor.py")
if spec is None or spec.loader is None:
    raise RuntimeError("cannot load maintained vendor patches")
vendor = importlib.util.module_from_spec(spec)
spec.loader.exec_module(vendor)
FILES = ("Cargo.toml", "src/buffer/cell.rs", "src/buffer/buffer.rs")


class RatatuiMemoryPatchTests(unittest.TestCase):
    def fixture(self, directory: str) -> Path:
        destination = Path(directory)
        shutil.copytree(ROOT / "vendor/ratatui-core", destination / "ratatui-core")
        return destination

    def apply(self, destination: Path) -> None:
        with patch.object(vendor, "VENDOR", destination):
            vendor.patch_ratatui_memory()

    def restore_pristine(self, destination: Path) -> None:
        root = destination / "ratatui-core"
        for relative, edits in vendor.RATATUI_MEMORY_EDITS.items():
            path = root / relative
            source = path.read_text()
            for before, after in reversed(edits):
                self.assertEqual(source.count(after), 1)
                source = source.replace(after, before, 1)
            self.assertEqual(hashlib.sha256(source.encode()).hexdigest(), vendor.RATATUI_MEMORY_HASHES[relative][0])
            path.write_text(source)
        vendor.refresh_checksum(root)

    def snapshot(self, destination: Path) -> dict[str, bytes]:
        return {str(path.relative_to(destination)): path.read_bytes() for path in destination.rglob("*") if path.is_file()}

    def test_committed_patch_and_checksums_are_complete_and_idempotent(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-ratatui-replay-") as directory:
            destination = self.fixture(directory)
            self.apply(destination)
            first = self.snapshot(destination)
            checksums = json.loads((destination / "ratatui-core/.cargo-checksum.json").read_text())
            for relative in FILES:
                actual = first["ratatui-core/" + relative]
                self.assertEqual(checksums["files"][relative], hashlib.sha256(actual).hexdigest())
                self.assertEqual(hashlib.sha256(actual).hexdigest(), vendor.RATATUI_MEMORY_HASHES[relative][1])
            self.apply(destination)
            self.assertEqual(self.snapshot(destination), first)

    def test_exact_pristine_tree_replays_to_identical_committed_tree(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-ratatui-pristine-") as directory:
            destination = self.fixture(directory)
            expected = self.snapshot(destination)
            self.restore_pristine(destination)
            self.apply(destination)
            self.assertEqual(self.snapshot(destination), expected)

    def test_each_source_drift_is_rejected_before_any_file_changes(self) -> None:
        for relative in FILES:
            with self.subTest(relative=relative), tempfile.TemporaryDirectory(prefix="hidlins-ratatui-drift-") as directory:
                destination = self.fixture(directory)
                self.restore_pristine(destination)
                path = destination / "ratatui-core" / relative
                path.write_text(path.read_text() + "\n// unexpected drift\n")
                before = self.snapshot(destination)
                with self.assertRaises(RuntimeError):
                    self.apply(destination)
                self.assertEqual(self.snapshot(destination), before)

    def test_partial_patch_is_rejected_without_completing_or_reblessing_it(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-ratatui-partial-") as directory:
            destination = self.fixture(directory)
            patched = (destination / "ratatui-core/Cargo.toml").read_bytes()
            self.restore_pristine(destination)
            (destination / "ratatui-core/Cargo.toml").write_bytes(patched)
            before = self.snapshot(destination)
            with self.assertRaises(RuntimeError):
                self.apply(destination)
            self.assertEqual(self.snapshot(destination), before)

    def test_wrong_version_is_rejected_before_writing(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-ratatui-version-") as directory:
            destination = self.fixture(directory)
            path = destination / "ratatui-core/Cargo.toml"
            path.write_text(path.read_text().replace('version = "0.1.2"', 'version = "0.1.3"', 1))
            before = self.snapshot(destination)
            with self.assertRaises(RuntimeError):
                self.apply(destination)
            self.assertEqual(self.snapshot(destination), before)

    def test_unmodified_source_checksum_tamper_is_not_reblessed(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-ratatui-checksum-") as directory:
            destination = self.fixture(directory)
            path = destination / "ratatui-core/src/lib.rs"
            path.write_text(path.read_text() + "\n// unexpected change\n")
            before = self.snapshot(destination)
            with self.assertRaises(RuntimeError):
                self.apply(destination)
            self.assertEqual(self.snapshot(destination), before)

    def test_missing_checksum_entry_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-ratatui-missing-") as directory:
            destination = self.fixture(directory)
            path = destination / "ratatui-core/.cargo-checksum.json"
            data = json.loads(path.read_text())
            del data["files"]["src/lib.rs"]
            path.write_text(json.dumps(data))
            before = self.snapshot(destination)
            with self.assertRaises(RuntimeError):
                self.apply(destination)
            self.assertEqual(self.snapshot(destination), before)


if __name__ == "__main__":
    unittest.main()
