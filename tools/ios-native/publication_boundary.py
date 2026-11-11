#!/usr/bin/env python3
"""Prove isolated native publication fixtures cannot enter normal artifacts."""
import os
from pathlib import Path
import shlex
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
PREFIX = 'hidlins_ios_publication_fixture_'
EXPECTED = {PREFIX + name for name in ('create', 'lock', 'status', 'finish')}
MESSAGE = 'iOS publication fixtures require a debug simulator build without desktop features'


def symbols(path):
    # Xcode debug builds can put app code in Runner.debug.dylib and leave
    # Runner as a small loader. Inspect both actual code images, fail closed.
    images = [path]
    debug_image = path.with_name(path.name + '.debug.dylib')
    if debug_image.is_file():
        images.append(debug_image)
    # CocoaPods links Rust into its embedded framework rather than the app
    # loader. Cover every embedded framework code image in the actual bundle.
    for framework in sorted(path.parent.glob('Frameworks/*.framework')):
        image = framework / framework.stem
        if not image.is_file():
            raise RuntimeError('Embedded framework code image missing: ' + str(framework))
        images.append(image)
    names = set()
    for image in images:
        result = subprocess.run(['nm', '-gU', str(image)], capture_output=True, text=True, check=True)
        names.update(line.split()[-1].lstrip('_') for line in result.stdout.splitlines() if line.split())
    return names


def fixture_environment():
    environment = os.environ.copy()
    encoded = environment.get('CARGO_ENCODED_RUSTFLAGS')
    flags = encoded.split('\x1f') if encoded is not None else shlex.split(environment.get('RUSTFLAGS', ''))
    flags.extend(['-D', 'warnings', '--cfg', 'hidlins_ios_publication_fixture'])
    environment['CARGO_ENCODED_RUSTFLAGS'] = '\x1f'.join(flags)
    return environment


def main():
    if sys.argv[1:] and sys.argv[1] == 'present':
        names = symbols(Path(sys.argv[2]))
        if not EXPECTED <= names:
            raise RuntimeError('Dedicated simulator fixture symbols missing')
        print('PASS dedicated simulator artifact contains all four fixture controls')
        return
    if sys.argv[1:] == ['lint']:
        subprocess.run(['make', 'lint', 'CLIPPY_FLAGS=-p hidlins-api --lib --no-default-features '
                        '--target aarch64-apple-ios-sim --offline --locked -- -D warnings'],
                       cwd=ROOT, env=fixture_environment(), check=True)
        return
    if sys.argv[1:] != ['check']:
        raise RuntimeError('usage: publication_boundary.py present <binary> | check | lint')
    environment = fixture_environment()
    for label, target, release in [('host', None, False),
                                   ('device', 'aarch64-apple-ios', False),
                                   ('release', 'aarch64-apple-ios-sim', True)]:
        output = ROOT / '.codex-tmp' / 'ios-publication-boundary' / label
        command = [sys.executable, str(ROOT / 'tools/dev/cargo_artifacts.py'), 'run', 'cargo', '--',
                   'check', '-p', 'hidlins-api', '--lib', '--no-default-features',
                   '--offline', '--locked', '--target-dir', str(output)]
        if target:
            command.extend(['--target', target])
        if release:
            command.append('--release')
        result = subprocess.run(command, cwd=ROOT, env=environment, capture_output=True, text=True, timeout=500)
        if result.returncode == 0 or MESSAGE not in result.stdout + result.stderr:
            raise RuntimeError('Fixture boundary did not reject ' + label + ':\n' + result.stderr[-4000:])
        print('PASS fixture build rejected for ' + label, flush=True)
    for platform in ('iphonesimulator', 'iphoneos'):
        binary = ROOT / 'app/build/ios' / platform / 'Runner.app/Runner'
        names = symbols(binary)
        if 'hidlins_ios_consume_clipboard' not in names:
            raise RuntimeError('Ordinary artifact lacks production clipboard ABI control: ' + platform)
        if any(name.startswith(PREFIX) for name in names):
            raise RuntimeError('Fixture controls leaked into ordinary ' + platform + ' artifact')
        print('PASS no fixture controls in ordinary ' + platform + ' artifact')


if __name__ == '__main__':
    main()
