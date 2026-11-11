#!/usr/bin/env python3
"""Remove trailing horizontal whitespace from generated Dart sources."""

from pathlib import Path
import sys


def normalize(root: Path) -> None:
    for path in sorted(root.rglob("*.dart")):
        original = path.read_bytes()
        normalized = b"\n".join(line.rstrip(b" \t") for line in original.split(b"\n"))
        if normalized != original:
            path.write_bytes(normalized)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: normalize-generated-dart.py DIRECTORY")
    normalize(Path(sys.argv[1]))
