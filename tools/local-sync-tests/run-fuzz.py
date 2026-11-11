#!/usr/bin/env python3
"""Run bounded, reproducible fuzz campaigns against disposable corpus copies."""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[2]
FUZZ = ROOT / "fuzz"
TOOLCHAIN = "nightly-2026-09-01"
TARGETS = ("local_protocol", "local_address", "pairing_state")


def main() -> int:
    runs = os.environ.get("FUZZ_RUNS", "512")
    if not runs.isascii() or not runs.isdigit() or int(runs) < 1:
        print("error: FUZZ_RUNS must be a positive decimal integer", file=sys.stderr)
        return 2
    version = subprocess.run(
        ["cargo", "fuzz", "--version"], check=False, capture_output=True, text=True
    )
    if version.returncode != 0 or version.stdout.strip() != "cargo-fuzz 0.13.2":
        print("error: cargo-fuzz 0.13.2 required; run make fuzz-toolchain", file=sys.stderr)
        return 1

    environment = os.environ.copy()
    environment["CARGO_NET_OFFLINE"] = "true"
    helper = ROOT / "tools/dev/vendor_cache.py"
    description = subprocess.run(
        [sys.executable, str(helper), "describe", "--workspace", str(FUZZ), "--", "cargo", f"+{TOOLCHAIN}"],
        env=environment, capture_output=True, text=True, check=True,
    )
    data = json.loads(description.stdout)
    target_directory = data["target_directory"]
    with tempfile.TemporaryDirectory(prefix="hidlins-fuzz-corpus-") as temporary:
        scratch = Path(temporary)
        for index, target in enumerate(TARGETS, start=1):
            corpus = scratch / target
            shutil.copytree(FUZZ / "corpus" / target, corpus)
            command = [
                "cargo",
                f"+{TOOLCHAIN}",
                "fuzz",
                "run",
                target,
                "--target-dir",
                target_directory,
                "--features",
                "fuzzing",
                str(corpus),
                "--",
                f"-runs={runs}",
                f"-seed={0x4849444C + index}",
                "-max_len=262144",
            ]
            subprocess.run(
                [sys.executable, str(helper), "run", "--source-root", str(ROOT),
                 "--workspace", str(FUZZ), "--target-dir", target_directory, "--", *command],
                cwd=FUZZ, env=environment, check=True,
            )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
