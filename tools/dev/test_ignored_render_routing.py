#!/usr/bin/env python3
"""Exercise real Make routing with isolated Cargo and owned-terminal controls."""
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
BENCHMARK = "framework_memory_regressions::render_workload_performance_baseline"
PROBE = r'''
import fcntl, json, os, select, struct, sys, termios, tty
args = sys.argv[1:]
benchmark = "framework_memory_regressions::render_workload_performance_baseline"
is_render = benchmark in args and "--exact" in args
record = {"args": args, "render": is_render, "tty": os.isatty(0)}
if is_render:
    fd = os.open("/dev/tty", os.O_RDWR | os.O_NOCTTY)
    try:
        record["size"] = list(struct.unpack("HHHH", fcntl.ioctl(fd, termios.TIOCGWINSZ, bytes(8))))[:2]
        previous = termios.tcgetattr(fd)
        try:
            tty.setraw(fd)
            os.write(1, b"\x1b[6n")
            if not select.select([fd], [], [], 2)[0]:
                raise RuntimeError("owned terminal did not answer cursor query")
            record["cursor_reply"] = os.read(fd, 64).decode("ascii")
        finally:
            termios.tcsetattr(fd, termios.TCSANOW, previous)
    finally:
        os.close(fd)
with open(os.environ["HIDLINS_IGNORED_ROUTING_LOG"], "a") as stream:
    stream.write(json.dumps(record) + "\n")
if args[0] == "build":
    raise SystemExit(0)
if is_render:
    assert record["tty"] and record["size"] == [40, 120]
    assert record["cursor_reply"] == "\x1b[1;1R"
    assert "--release" in args and "--ignored" in args
    raise SystemExit(43 if os.environ.get("HIDLINS_IGNORED_ROUTING_FAILURE") == "render" else 0)
assert not record["tty"], "ordinary ignored cases must retain their headless environment"
if os.environ.get("HIDLINS_IGNORED_ROUTING_FAILURE") == "ordinary":
    raise SystemExit(42)
if not any(args[i:i+2] == ["--skip", benchmark] for i in range(len(args))):
    print("headless sweep includes the terminal-dependent render case", flush=True)
    raise SystemExit(76)
print("ordinary ignored cases retained", flush=True)
'''


class IgnoredRenderRoutingTests(unittest.TestCase):
    def run_make(self, target, *, flags="--workspace --offline --locked", failure=None):
        with tempfile.TemporaryDirectory(prefix="hidlins ignored routing ") as directory:
            directory = Path(directory)
            probe = directory / "cargo probe.py"
            log = directory / "calls.jsonl"
            probe.write_text(PROBE)
            environment = os.environ.copy()
            for key in ("MAKEFLAGS", "MFLAGS", "HIDLINS_IGNORED_ROUTING_FAILURE"):
                environment.pop(key, None)
            environment["HIDLINS_IGNORED_ROUTING_LOG"] = str(log)
            if failure:
                environment["HIDLINS_IGNORED_ROUTING_FAILURE"] = failure
            result = subprocess.run(
                ["make", "--no-print-directory", "-f", str(ROOT / "Makefile"), target,
                 "CARGO_COMPILE=" + shlex.join([sys.executable, str(probe)]),
                 "CARGO_FLAGS=" + flags],
                cwd=ROOT, env=environment, stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                start_new_session=True, timeout=20,
            )
            calls = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
            return result, calls

    def test_headless_sweep_retains_ordinary_cases_and_runs_render_once(self):
        result, calls = self.run_make("test-ignored")
        self.assertEqual(result.returncode, 0, result.stdout)
        ordinary = [call for call in calls if call["args"][0] == "test" and not call["render"]]
        rendered = [call for call in calls if call["render"]]
        self.assertEqual(len(ordinary), 1)
        self.assertFalse(ordinary[0]["tty"])
        self.assertIn("--ignored", ordinary[0]["args"])
        self.assertIn("--test-threads=1", ordinary[0]["args"])
        self.assertIn("ordinary ignored cases retained", result.stdout)
        self.assertEqual(len(rendered), 1)
        self.assertTrue(rendered[0]["tty"])
        self.assertEqual(rendered[0]["size"], [40, 120])
        self.assertEqual(rendered[0]["cursor_reply"], "\x1b[1;1R")

    def test_custom_cargo_flags_reach_the_ordinary_sweep(self):
        flags = "--workspace --exclude hidlins-cli --offline --locked"
        result, calls = self.run_make("test-ignored", flags=flags)
        self.assertEqual(result.returncode, 0, result.stdout)
        ordinary = next(call for call in calls if call["args"][0] == "test" and not call["render"])
        expected = shlex.split(flags)
        self.assertEqual(ordinary["args"][1:1 + len(expected)], expected)
        self.assertEqual(sum(call["render"] for call in calls), 1)

    def test_ordinary_failure_stops_before_render(self):
        result, calls = self.run_make("test-ignored", failure="ordinary")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Error 42", result.stdout)
        self.assertFalse(any(call["render"] for call in calls))

    def test_render_failure_is_returned_after_ordinary_cases(self):
        result, calls = self.run_make("test-ignored", failure="render")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Error 43", result.stdout)
        self.assertIn("ordinary ignored cases retained", result.stdout)
        self.assertEqual(sum(call["render"] for call in calls), 1)

    def test_existing_release_target_has_real_owned_cursor_query_environment(self):
        result, calls = self.run_make("bench-tui-render")
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual(len(calls), 1)
        self.assertTrue(calls[0]["render"])
        self.assertEqual(calls[0]["cursor_reply"], "\x1b[1;1R")


if __name__ == "__main__":
    unittest.main(verbosity=2)
