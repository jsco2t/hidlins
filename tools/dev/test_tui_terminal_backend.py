#!/usr/bin/env python3
"""Check the real Cargo terminal backend closure on supported desktop targets."""

import subprocess
import sys


def tree(cargo, target, *arguments):
    return subprocess.run(
        [*cargo, "tree", "-p", "hidlins-tui", "--offline", "--locked",
         "--target", target, "--prefix", "none", *arguments],
        check=True, capture_output=True, text=True,
    ).stdout


def main():
    cargo = sys.argv[2:]
    if sys.argv[1:2] != ["--"] or not cargo:
        raise SystemExit("usage: test_tui_terminal_backend.py -- <cargo command>")
    for target in ("aarch64-apple-darwin", "x86_64-apple-darwin",
                   "aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"):
        macos = "apple-darwin" in target
        features = tree(cargo, target, "-e", "features", "-i", "crossterm")
        enabled = 'crossterm feature "use-dev-tty"' in features
        if enabled != macos:
            raise AssertionError(f"{target}: unexpected use-dev-tty selection {enabled}")
        normal = tree(cargo, target, "-e", "normal,build")
        for package in ("filedescriptor v0.8.3", "thiserror v1.0.69",
                        "thiserror-impl v1.0.69"):
            present = any(line.startswith(package + " ") or line == package
                          for line in normal.splitlines())
            if present != macos:
                raise AssertionError(f"{target}: unexpected normal closure for {package}")
        print(f"{target}: terminal backend and reviewed package closure OK")


if __name__ == "__main__":
    main()
