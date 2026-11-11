#!/usr/bin/env python3
"""Select an iPhone/iPad pair spanning the two newest available runtimes."""

from __future__ import annotations

import json
import sys
from pathlib import Path


def runtime_version(identifier: str) -> tuple[int, ...]:
    marker = ".iOS-"
    if marker not in identifier:
        return ()
    return tuple(int(part) for part in identifier.split(marker, 1)[1].split("-"))


def select_matrix(
    candidates: list[tuple[str, dict[str, object]]],
) -> list[tuple[str, dict[str, object]]]:
    phones = [item for item in candidates if "iPhone" in str(item[1].get("name", ""))]
    tablets = [item for item in candidates if "iPad" in str(item[1].get("name", ""))]
    if not phones:
        raise SystemExit("no available iPhone simulator is installed")
    if not tablets:
        raise SystemExit("no available iPad simulator is installed")

    pairs = [
        (phone, tablet)
        for phone in phones
        for tablet in tablets
        if phone[0] != tablet[0]
    ]
    if not pairs:
        raise SystemExit(
            "at least two available iOS simulator runtimes with iPhone/iPad "
            "devices are required"
        )

    phone, tablet = max(
        pairs,
        key=lambda pair: (
            runtime_version(pair[0][0]),
            runtime_version(pair[1][0]),
            pair[0][1].get("state") == "Booted",
            pair[1][1].get("state") == "Booted",
            "Pro" in str(pair[0][1].get("name", "")),
            "Pro" in str(pair[1][1].get("name", "")),
            str(pair[0][1].get("name", "")),
            str(pair[1][1].get("name", "")),
        ),
    )
    return [phone, tablet]


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: select_simulators.py <simctl-devices.json>")
    payload = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    candidates: list[tuple[str, dict[str, object]]] = []
    for runtime, devices in payload.get("devices", {}).items():
        if ".iOS-" not in runtime:
            continue
        for device in devices:
            if device.get("isAvailable", True):
                candidates.append((runtime, device))

    for runtime, device in select_matrix(candidates):
        version = ".".join(str(part) for part in runtime_version(runtime))
        print(f"{device['udid']}\t{device['name']}\t{version}")


if __name__ == "__main__":
    main()
