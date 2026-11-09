#!/usr/bin/env python3
"""Invalidate Cargo intermediates when reviewed vendored bytes change."""
from __future__ import annotations

import argparse
from contextlib import ExitStack, contextmanager
import fcntl
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile

from content_fingerprint import entries, field, file_digest, tree_digest

ROOT = Path(__file__).resolve().parents[2]
STAMP = '.hidlins-vendor-generation'


def lock_path(root: Path) -> Path:
    return root.parent / ('.' + root.name + '.hidlins-vendor.lock')


def descriptor(workspace: Path, cargo: list[str], target: Path | None = None) -> dict:
    command = [*cargo, 'metadata', '--no-deps', '--format-version', '1', '--offline', '--locked']
    environment = os.environ.copy()
    if target is not None:
        environment['CARGO_TARGET_DIR'] = str(target.absolute())
    result = subprocess.run(command, cwd=workspace, capture_output=True, text=True, check=True, env=environment)
    data = json.loads(result.stdout)
    roots = {}
    for key in ('target_directory', 'build_directory'):
        value = data.get(key)
        if not isinstance(value, str) or not Path(value).is_absolute():
            raise ValueError('Cargo metadata has no absolute ' + key)
        roots[key] = str(Path(value).resolve())
    roots['lock_paths'] = sorted({str(lock_path(Path(value))) for value in roots.values()})
    return roots


@contextmanager
def cache_lock(name: Path):
    name.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(name, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, 'r+b') as stream:
        if not stat.S_ISREG(os.fstat(stream.fileno()).st_mode):
            raise ValueError('Cargo cache lock must be a regular file')
        fcntl.lockf(stream, fcntl.LOCK_EX)
        yield


def generation(source: Path) -> str:
    value = hashlib.sha256()
    field(value, b'hidlins-vendor-invalidation-v1')
    # Policy changes invalidate old intermediates too. No mtime/size shortcut.
    field(value, file_digest(Path(__file__)))
    field(value, file_digest(Path(__file__).with_name('content_fingerprint.py')))
    field(value, tree_digest([source / 'vendor']))
    return value.hexdigest()


def ordinary_directory(path: Path) -> bool:
    mode = path.lstat().st_mode
    if stat.S_ISLNK(mode):
        raise ValueError('Cargo cleanup path must not be a symbolic link: ' + str(path))
    return stat.S_ISDIR(mode)


def remove(path: Path) -> None:
    mode = path.lstat().st_mode
    if stat.S_ISLNK(mode):
        raise ValueError('Cargo cleanup path must not be a symbolic link: ' + str(path))
    if stat.S_ISDIR(mode):
        # Validate every descendant before shutil.rmtree; errors propagate.
        for child in entries(path):
            remove(child)
        path.rmdir()
    elif stat.S_ISREG(mode):
        path.unlink()
    else:
        raise ValueError('Unexpected Cargo cache entry: ' + str(path))


def profiles(root: Path):
    if not root.exists():
        return
    if not ordinary_directory(root):
        raise ValueError('Cargo output root must be an ordinary directory')
    for child in entries(root):
        if not ordinary_directory(child):
            continue
        if (child / '.fingerprint').exists() or (child / '.cargo-lock').exists():
            yield child
            continue
        # Explicit target caches have exactly one extra target-triple level.
        for profile in entries(child):
            if ordinary_directory(profile) and ((profile / '.fingerprint').exists() or
                                                 (profile / '.cargo-lock').exists()):
                yield profile


def invalidate(source: Path, roots: list[Path]) -> None:
    wanted = generation(source)
    stale = []
    for root in sorted(set(roots)):
        stamp = root / STAMP
        try:
            if stamp.is_symlink():
                raise ValueError('Cargo generation stamp must not be a symbolic link')
            current = stamp.read_text()
        except FileNotFoundError:
            current = None
        if current != wanted:
            stale.append(root)
    for root in stale:
        for profile in profiles(root):
            fingerprint = profile / '.fingerprint'
            if fingerprint.exists() or fingerprint.is_symlink():
                if not ordinary_directory(fingerprint):
                    raise ValueError('Cargo fingerprint must be an ordinary directory')
                # Keep the marker for a retry after interrupted partial cleanup.
                for child in entries(fingerprint):
                    remove(child)
            for name in ('deps', 'build', 'incremental'):
                path = profile / name
                if path.exists() or path.is_symlink():
                    remove(path)
    # This is an invalidation receipt, not a successful-build receipt. Retain
    # it through ordinary compilation failure so retries keep warm dependencies.
    for root in stale:
        root.mkdir(parents=True, exist_ok=True)
        descriptor, temporary = tempfile.mkstemp(prefix=STAMP + '-', dir=root)
        try:
            with os.fdopen(descriptor, 'w') as stream:
                stream.write(wanted)
                stream.flush()
                os.fsync(stream.fileno())
            os.replace(temporary, root / STAMP)
        finally:
            if os.path.exists(temporary):
                os.unlink(temporary)


def guard(source: Path, description: dict, command: list[str], workspace: Path,
          invalidate_sources: bool = True) -> int:
    with ExitStack() as stack:
        for name in description['lock_paths']:
            stack.enter_context(cache_lock(Path(name)))
        if invalidate_sources:
            invalidate(source, [Path(description[key]) for key in ('target_directory', 'build_directory')])
        return subprocess.run(command, cwd=workspace, check=False).returncode


def verify_parent_locks(roots: list[Path]) -> None:
    # Dart owns POSIX record locks in its parent process. A child must never
    # reacquire them blocking, and must not invalidate without that ownership.
    for root in sorted(set(roots)):
        name = lock_path(root)
        fd = os.open(name, os.O_RDWR | os.O_NOFOLLOW)
        with os.fdopen(fd, 'r+b') as stream:
            try:
                fcntl.lockf(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                continue
            raise ValueError('Caller does not hold the Cargo vendor cache lock')


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['describe', 'invalidate-locked', 'run'])
    parser.add_argument('--source-root', type=Path, default=ROOT)
    parser.add_argument('--workspace', type=Path, default=ROOT)
    parser.add_argument('--target-dir', type=Path)
    parser.add_argument('--build-dir', type=Path)
    # Split explicitly: argparse REMAINDER would swallow the named options.
    argv = sys.argv[1:]
    index = argv.index('--') if '--' in argv else len(argv)
    arguments = parser.parse_args(argv[:index])
    tail = argv[index + 1:]
    try:
        if arguments.command == 'invalidate-locked':
            if arguments.target_dir is None or arguments.build_dir is None:
                raise ValueError('Both Cargo cache roots are required')
            roots = [arguments.target_dir.resolve(), arguments.build_dir.resolve()]
            verify_parent_locks(roots)
            invalidate(arguments.source_root, roots)
            return 0
        data = descriptor(arguments.workspace, tail if arguments.command == 'describe' else ['cargo'], arguments.target_dir)
        if arguments.command == 'describe':
            print(json.dumps(data))
            return 0
        if not tail:
            raise ValueError('A guarded command is required')
        return guard(arguments.source_root, data, tail, arguments.workspace)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print('error: vendor cache guard failed: ' + str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
