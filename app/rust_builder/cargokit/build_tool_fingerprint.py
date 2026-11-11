#!/usr/bin/env python3
"""Hidlins patch: hash reviewed build-tool and Pub dependency bytes, excluding runtime cache state."""
import hashlib
import os
from pathlib import Path
import stat
import sys


def digest(root: Path) -> str:
    value = hashlib.sha256()

    def field(data):
        value.update(len(data).to_bytes(8, 'big'))
        value.update(data)

    def visit(path: Path, label: str):
        mode = path.lstat().st_mode
        field(os.fsencode(label))
        if stat.S_ISDIR(mode):
            field(b'directory')
            with os.scandir(path) as stream:
                children = sorted((Path(entry.path) for entry in stream), key=os.fsencode)
            for child in children:
                if label == 'vendor-pub/hosted/pub.dev' and child.name == '.cache':
                    continue
                visit(child, label + '/' + child.name)
        elif stat.S_ISREG(mode):
            field(b'file')
            hashed = hashlib.sha256()
            with path.open('rb') as stream:
                for chunk in iter(lambda: stream.read(1024 * 1024), b''):
                    hashed.update(chunk)
            field(hashed.digest())
        else:
            raise ValueError('Expected a regular build-tool input: ' + str(path))

    for name in ('lib', 'pubspec.yaml', 'pubspec.lock'):
        visit(root / name, name)
    # The top-level cache may be relocated/symlinked by a disposable test or
    # workstation. Its reviewed package sources and lock hashes are inputs;
    # active_roots, _temp and hosted/.cache are mutable Pub bookkeeping.
    cache = (root / 'vendor-pub').resolve(strict=True)
    for name in ('hosted/pub.dev', 'hosted-hashes/pub.dev'):
        visit(cache / name, 'vendor-pub/' + name)
    visit(Path(__file__).resolve(), 'build_tool_fingerprint.py')
    return value.hexdigest()


if __name__ == '__main__':
    try:
        print(digest(Path(sys.argv[1]).resolve(strict=True)))
    except (OSError, ValueError) as error:
        print('error: cannot fingerprint build-tool inputs: ' + str(error), file=sys.stderr)
        raise SystemExit(1)
