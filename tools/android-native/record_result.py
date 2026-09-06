#!/usr/bin/env python3
"""Write a machine-readable Android emulator verification result."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
from datetime import UTC, datetime
from pathlib import Path


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def named_paths(values: list[str]) -> list[dict[str, object]]:
    results: list[dict[str, object]] = []
    for value in values:
        name, raw_path = value.split("=", 1)
        path = Path(raw_path).resolve()
        if not path.is_file():
            raise SystemExit(f"missing result input: {path}")
        results.append(
            {
                "name": name,
                "path": str(path),
                "sha256": digest(path),
                "bytes": path.stat().st_size,
            }
        )
    return results


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--suite", required=True)
    parser.add_argument("--avd", required=True)
    parser.add_argument("--serial", required=True)
    parser.add_argument("--api", type=int, required=True)
    parser.add_argument("--abi", required=True)
    parser.add_argument("--form-factor", required=True)
    parser.add_argument("--device-model", required=True)
    parser.add_argument("--artifact", action="append", default=[])
    parser.add_argument("--log", action="append", default=[])
    args = parser.parse_args()
    result = {
        "schema_version": 1,
        "status": "PASS",
        "suite": args.suite,
        "recorded_at": datetime.now(UTC).isoformat(),
        "host": {"os": platform.system(), "architecture": platform.machine()},
        "emulator": {
            "avd": args.avd,
            "serial": args.serial,
            "api": args.api,
            "abi": args.abi,
            "form_factor": args.form_factor,
            "device_model": args.device_model,
        },
        "artifacts": named_paths(args.artifact),
        "redacted_logs": named_paths(args.log),
        "skipped_residuals": [
            "TalkBack spoken and gesture traversal — user decision",
            "launcher and app-switcher OS-shell rendering — user decision",
            "physical-device execution — hardware unavailable and not required",
        ],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    temporary = args.output.with_suffix(args.output.suffix + ".tmp")
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
        json.dump(result, stream, indent=2, sort_keys=True)
        stream.write("\n")
    temporary.replace(args.output)


if __name__ == "__main__":
    main()
