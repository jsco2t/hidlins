#!/usr/bin/env python3
"""Validate and select the repository-owned Android emulator matrix."""

from __future__ import annotations

import argparse
import json
import platform
from pathlib import Path

MATRIX = Path(__file__).with_name("emulator-matrix.json")
SUPPORTED_APIS = {29, 36}
SUPPORTED_ABIS = {"arm64-v8a", "x86_64"}
HOST_ABI = {"arm64": "arm64-v8a", "aarch64": "arm64-v8a", "x86_64": "x86_64"}


def validate(data: object) -> list[dict[str, object]]:
    if not isinstance(data, dict) or data.get("schema_version") != 1:
        raise SystemExit("Android emulator matrix has an unsupported schema")
    if data.get("minimum_api") != 29 or data.get("current_api") != 36:
        raise SystemExit("Android emulator matrix must cover API 29 and current API 36")
    entries = data.get("entries")
    if not isinstance(entries, list):
        raise SystemExit("Android emulator matrix entries must be a list")
    combinations: set[tuple[int, str]] = set()
    avds: set[str] = set()
    form_factors: set[str] = set()
    for raw in entries:
        if not isinstance(raw, dict):
            raise SystemExit("Android emulator matrix entry must be an object")
        api = raw.get("api")
        abi = raw.get("abi")
        host_arch = raw.get("host_arch")
        form_factor = raw.get("form_factor")
        avd = raw.get("avd")
        image = raw.get("system_image")
        if api not in SUPPORTED_APIS or abi not in SUPPORTED_ABIS:
            raise SystemExit(f"unsupported Android matrix entry: API {api}, ABI {abi}")
        if HOST_ABI.get(str(host_arch)) != abi:
            raise SystemExit(f"host architecture {host_arch} cannot execute {abi}")
        if form_factor not in {"phone", "tablet"}:
            raise SystemExit(f"unsupported Android form factor: {form_factor}")
        if not isinstance(avd, str) or not avd.startswith("hidlins-") or avd in avds:
            raise SystemExit(f"invalid or duplicate Hidlins AVD name: {avd}")
        expected_suffix = f"android-{api};default;{abi}"
        if not isinstance(image, str) or not image.endswith(expected_suffix):
            raise SystemExit(f"system image does not match API/ABI for {avd}")
        combinations.add((int(api), str(abi)))
        avds.add(avd)
        form_factors.add(str(form_factor))
    expected = {(api, abi) for api in SUPPORTED_APIS for abi in SUPPORTED_ABIS}
    if combinations != expected:
        raise SystemExit(f"Android matrix combinations are {sorted(combinations)}, expected {sorted(expected)}")
    if form_factors != {"phone", "tablet"}:
        raise SystemExit("Android emulator matrix must cover phone and tablet layouts")
    return entries


def load(path: Path = MATRIX) -> list[dict[str, object]]:
    return validate(json.loads(path.read_text(encoding="utf-8")))


def normalized_host(value: str) -> str:
    if value in {"arm64", "aarch64"}:
        return "arm64"
    if value == "x86_64":
        return value
    raise SystemExit(f"unsupported Android emulator host architecture: {value}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("check", "select"))
    parser.add_argument("--host", default=platform.machine())
    args = parser.parse_args()
    entries = load()
    if args.command == "check":
        print("  OK: Android matrix covers API 29/current, both ABIs, phone, and tablet")
        return
    host = normalized_host(args.host)
    selected = [entry for entry in entries if entry["host_arch"] == host]
    if len(selected) != 2:
        raise SystemExit(f"Android matrix has no complete two-API selection for {host}")
    for entry in selected:
        print("\t".join(str(entry[key]) for key in ("avd", "api", "abi", "form_factor", "system_image")))


if __name__ == "__main__":
    main()
