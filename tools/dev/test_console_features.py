#!/usr/bin/env python3
"""Verify desktop console and CLI-only Android authority dependency boundaries."""

import subprocess
import sys


def main():
    if sys.argv[1:2] != ["--"] or len(sys.argv) < 3:
        raise SystemExit("usage: test_console_features.py -- <cargo command>")
    cargo = sys.argv[2:]
    # Select the authority package explicitly to avoid desktop workspace
    # feature unification masking the standalone build contract.
    for target in ("aarch64-linux-android", "x86_64-linux-android"):
        result = subprocess.run(
            [*cargo, "tree", "-p", "hidlins-cli", "--target", target,
             "--no-default-features", "--features", "android-scenario-authority",
             "--edges", "normal,build", "--prefix", "none", "--offline", "--locked"],
            check=True, capture_output=True, text=True,
        )
        packages = {line.split()[0] for line in result.stdout.splitlines() if line}
        forbidden = packages & {"hidlins-tui", "ratatui", "crossterm", "arboard", "zbus", "objc2"}
        if forbidden:
            raise AssertionError(f"{target}: desktop packages in authority: {sorted(forbidden)}")
        for required in ("hidlins-cli", "hidlins-sync", "mdns-sd", "if-addrs"):
            if required not in packages:
                raise AssertionError(f"{target}: authority missing {required}")
        print(f"{target}: CLI authority excludes desktop UI/clipboard/lock packages")


if __name__ == "__main__":
    main()
