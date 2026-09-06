#!/usr/bin/env python3
"""Select one available, connected physical iOS device from xcdevice JSON."""

import json
import sys


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: select_device.py <xcdevice.json>", file=sys.stderr)
        return 2
    with open(sys.argv[1], encoding="utf-8") as handle:
        devices = json.load(handle)
    candidates = [
        item
        for item in devices
        if item.get("platform") == "com.apple.platform.iphoneos"
        and item.get("available") is True
        and item.get("simulator") is not True
        and item.get("identifier")
    ]
    if not candidates:
        print(
            "error: no available physical iOS device is connected; "
            "unlock and trust an iPhone, then retry",
            file=sys.stderr,
        )
        return 3
    device = sorted(candidates, key=lambda item: item["identifier"])[0]
    fields = [
        device["identifier"],
        str(device.get("name", "unknown-device")),
        str(device.get("operatingSystemVersion", "unknown-os")),
    ]
    print("\t".join(field.replace("\t", " ") for field in fields))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
