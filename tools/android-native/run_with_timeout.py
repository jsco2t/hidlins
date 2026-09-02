#!/usr/bin/env python3
"""Run a command with a fail-closed process-group timeout."""

from __future__ import annotations

import argparse
import os
import signal
import subprocess


def run(command: list[str], timeout_seconds: float) -> int:
    process = subprocess.Popen(command, start_new_session=True)
    try:
        return process.wait(timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        print(
            f"error: command exceeded {timeout_seconds:g}-second timeout",
            flush=True,
        )
        os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
        return 124


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("timeout_seconds", type=float)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.timeout_seconds <= 0 or not args.command:
        parser.error("a positive timeout and command are required")
    raise SystemExit(run(args.command, args.timeout_seconds))


if __name__ == "__main__":
    main()
