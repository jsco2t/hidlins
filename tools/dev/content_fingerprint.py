#!/usr/bin/env python3
"""Portable byte fingerprints for generated outputs and offline resolution."""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import stat
import sys


def file_digest(path: Path) -> bytes:
    if not stat.S_ISREG(path.lstat().st_mode):
        raise ValueError(f"expected a regular file: {path}")
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(chunk)
    return value.digest()


def field(value, data: bytes) -> None:
    value.update(len(data).to_bytes(8, "big"))
    value.update(data)


def entries(directory: Path) -> list[Path]:
    # Path.rglob can suppress enumeration errors. A missing/unreadable input
    # must never become an apparently valid empty signature.
    with os.scandir(directory) as children:
        return sorted((Path(child.path) for child in children), key=os.fsencode)


def tree_digest(paths: list[Path]) -> bytes:
    value = hashlib.sha256()

    def visit(path: Path) -> None:
        mode = path.lstat().st_mode
        field(value, os.fsencode(path))
        if stat.S_ISDIR(mode):
            field(value, b"directory")
            for child in entries(path):
                visit(child)
        elif stat.S_ISREG(mode):
            field(value, b"file")
            field(value, file_digest(path))
        else:
            raise ValueError(f"expected an ordinary generated file/directory: {path}")

    for path in sorted(paths, key=os.fsencode):
        visit(path)
    return value.digest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for command in ("file", "tree"):
        child = commands.add_parser(command)
        child.add_argument("paths", type=Path, nargs="+" if command != "file" else 1)
    arguments = parser.parse_args()
    try:
        if arguments.command == "file":
            digest = file_digest(arguments.paths[0])
        elif arguments.command == "tree":
            digest = tree_digest(arguments.paths)
        print(digest.hex())
        return 0
    except (OSError, ValueError) as error:
        print(f"error: cannot fingerprint inputs: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
