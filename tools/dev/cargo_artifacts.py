#!/usr/bin/env python3
"""Resolve configured Cargo output roots and guard canonical compilation."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]


def metadata(cargo: list[str], workspace: Path = ROOT,
             arguments: list[str] | None = None) -> dict:
    command = [*cargo, 'metadata', '--no-deps', '--format-version', '1', '--offline', '--locked']
    args = arguments or []
    if '--' in args:
        args = args[:args.index('--')]
    for name in ('--manifest-path', '--config'):
        for index, item in enumerate(args):
            if item == name and index + 1 < len(args):
                command.extend([name, args[index + 1]])
            elif item.startswith(name + '='):
                command.append(item)
    environment = os.environ.copy()
    for index, item in enumerate(args):
        target = None
        if item == '--target-dir' and index + 1 < len(args):
            target = args[index + 1]
        elif item.startswith('--target-dir='):
            target = item.split('=', 1)[1]
        if target is not None:
            environment['CARGO_TARGET_DIR'] = str(Path(target).absolute())
    result = subprocess.run(command, cwd=workspace, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, text=True, check=True, env=environment)
    data = json.loads(result.stdout)
    if not isinstance(data, dict):
        raise ValueError('Cargo metadata must be an object')
    return data


def target_directory(cargo: list[str], workspace: Path = ROOT) -> Path:
    value = metadata(cargo, workspace).get('target_directory')
    if not isinstance(value, str) or not value or not Path(value).is_absolute():
        raise ValueError('Cargo metadata has no absolute target_directory')
    return Path(value)


def compile_command(arguments: list[str]) -> int:
    boundary = arguments.index('--')
    cargo = arguments[:boundary] or ['cargo']
    command = arguments[boundary + 1:]
    if not command:
        raise ValueError('A Cargo command is required')
    data = metadata(cargo, arguments=command)
    packages = data.get('packages')
    # Only a completely dependency-free workspace can omit the vendor policy
    # module. Path dependencies with registry transitives cannot take this path.
    if not isinstance(packages, list) or not packages:
        raise ValueError('Cargo metadata has no packages')
    if all(isinstance(package, dict) and package.get('dependencies') == [] for package in packages):
        return subprocess.run([*cargo, *command], cwd=ROOT, check=False).returncode

    import vendor_cache

    roots = {}
    for key in ('target_directory', 'build_directory'):
        value = data.get(key)
        if not isinstance(value, str) or not Path(value).is_absolute():
            raise ValueError('Cargo metadata has no absolute ' + key)
        roots[key] = str(Path(value).resolve())
    roots['lock_paths'] = sorted({str(vendor_cache.lock_path(Path(value))) for value in roots.values()})
    return vendor_cache.guard(ROOT, roots, [*cargo, *command], ROOT, invalidate_sources=command[0] != 'clean')


def main() -> int:
    try:
        args = sys.argv[1:]
        if args[:1] == ['run']:
            return compile_command(args[1:])
        parser = argparse.ArgumentParser(description=__doc__)
        parser.add_argument('command', choices=['target-dir'])
        parser.add_argument('cargo', nargs=argparse.REMAINDER)
        options = parser.parse_args(args)
        cargo = options.cargo
        if cargo[:1] == ['--']:
            cargo = cargo[1:]
        print(target_directory(cargo or ['cargo']))
        return 0
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print('error: Cargo artifact/cache guard failed: ' + str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
