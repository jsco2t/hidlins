#!/usr/bin/env python3
"""Real offline compilation and shared Rust cache invalidation regressions.

Registry packages below are tiny first-party MIT fixtures, never downloaded.
"""
from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
CARGO = shutil.which('cargo')
HOST = subprocess.check_output(['rustc', '-vV'], text=True).split('host: ')[1].splitlines()[0]

class CacheTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='hidlins-vendor-cache-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.env = dict(os.environ, RUSTUP_TOOLCHAIN='1.95.0', CARGO_NET_OFFLINE='true')
        for name in ('MAKEFLAGS', 'MFLAGS', 'MAKELEVEL', 'MAKEFILES', 'CARGO_TARGET_DIR',
                     'CARGO_BUILD_BUILD_DIR', 'RUSTFLAGS', 'RUSTC_WRAPPER'):
            self.env.pop(name, None)
        self.config = self.root / '.cargo/config.toml'
        self.config.parent.mkdir()
        self.config.write_text('[source.crates-io]\nreplace-with="fixture"\n'
                               '[source.fixture]\ndirectory="vendor"\n')
        (self.root / 'Cargo.toml').write_text(
            '[package]\nname="cache-fixture"\nversion="0.1.0"\nedition="2021"\nlicense="MIT"\n'
            '[dependencies]\nfixture-value="=0.1.0"\nfixture-macro="=0.1.0"\n'
            '[build-dependencies]\nfixture-value="=0.1.0"\n')
        (self.root / 'src').mkdir()
        (self.root / 'src/main.rs').write_text(
            'fn main(){println!("{}|{}|{}",fixture_value::value(),'
            'fixture_macro::value!(),env!("FIXTURE_BUILD_VALUE"));}\n')
        (self.root / 'build.rs').write_text(
            'fn main(){println!("cargo:rustc-env=FIXTURE_BUILD_VALUE={}",fixture_value::value());'
            'if let Ok(p)=std::env::var("CACHE_PAUSE_FILE") {'
            'std::fs::write(format!("{p}.ready"),b"ready").unwrap();'
            'while !std::path::Path::new(&p).exists(){std::thread::sleep(std::time::Duration::from_millis(20));}'
            '}}\n')
        for name in ('fixture-value', 'fixture-macro'):
            package = self.root / 'vendor' / name
            (package / 'src').mkdir(parents=True)
            macro = name == 'fixture-macro'
            (package / 'Cargo.toml').write_text(
                f'[package]\nname="{name}"\nversion="0.1.0"\nedition="2021"\nlicense="MIT"\n'
                + ('[lib]\nproc-macro=true\n' if macro else ''))
            (package / 'src/lib.rs').write_text(
                'extern crate proc_macro;\n#[proc_macro]\npub fn value(_:proc_macro::TokenStream)'
                '->proc_macro::TokenStream{"\\"A\\"".parse().unwrap()}\n'
                if macro else 'pub fn value()->&\'static str{"A"}\n')
            self.checksums(package)
        self.copy_tools()
        self.run_ok([CARGO, 'generate-lockfile', '--offline'])

    def copy_tools(self):
        shutil.copyfile(ROOT / 'Makefile', self.root / 'Makefile')
        shutil.copyfile(ROOT / 'rust-toolchain.toml', self.root / 'rust-toolchain.toml')
        for name in ('cargo_artifacts.py', 'content_fingerprint.py', 'vendor_cache.py'):
            source = ROOT / 'tools/dev' / name
            if source.exists():
                dest = self.root / 'tools/dev' / name
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, dest)

    def checksums(self, package):
        files = {str(p.relative_to(package)): hashlib.sha256(p.read_bytes()).hexdigest()
                 for p in package.rglob('*') if p.is_file() and p.name != '.cargo-checksum.json'}
        (package / '.cargo-checksum.json').write_text(json.dumps({'files': files, 'package': '0' * 64}))

    def patch(self, checksums=True):
        for package in (self.root / 'vendor').iterdir():
            source = package / 'src/lib.rs'
            old = source.stat()
            source.write_text(source.read_text().replace('A', 'B'))
            os.utime(source, ns=(old.st_atime_ns, old.st_mtime_ns))
            if checksums:
                checksum = package / '.cargo-checksum.json'
                old = checksum.stat()
                self.checksums(package)
                os.utime(checksum, ns=(old.st_atime_ns, old.st_mtime_ns))

    def execute(self, command, env=None, timeout=90):
        return subprocess.run(command, cwd=self.root, env=env or self.env,
                              capture_output=True, text=True, timeout=timeout)

    def run_ok(self, command, **kwargs):
        result = self.execute(command, **kwargs)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return result

    def make_build(self, **kwargs):
        return self.run_ok(['make', '--no-print-directory', 'build'], **kwargs)

    def metadata(self):
        return json.loads(self.run_ok([CARGO, 'metadata', '--format-version', '1', '--offline', '--locked']).stdout)

    def output(self, triple=None):
        root = Path(self.metadata()['target_directory'])
        return root / triple / 'debug/cache-fixture' if triple else root / 'debug/cache-fixture'

    def value(self, expected, triple=None):
        self.assertEqual(self.run_ok([str(self.output(triple))]).stdout.strip(), expected,
                         'VENDORED_BYTES_MUST_REACH_BINARY')

    def test_make_real_library_macro_and_host_build_dependency(self):
        for mode in ('default', 'relative', 'absolute', 'separate-build-dir'):
            with self.subTest(mode=mode):
                # Each case has distinct target state; input remains A until its first build.
                for package in (self.root / 'vendor').iterdir():
                    file = package / 'src/lib.rs'
                    file.write_text(file.read_text().replace('B', 'A'))
                    self.checksums(package)
                self.env.pop('CARGO_TARGET_DIR', None)
                self.env.pop('CARGO_BUILD_BUILD_DIR', None)
                if mode == 'relative': self.env['CARGO_TARGET_DIR'] = 'relative output'
                if mode in ('absolute', 'separate-build-dir'):
                    self.env['CARGO_TARGET_DIR'] = str(self.root / mode)
                if mode == 'separate-build-dir':
                    self.env['CARGO_BUILD_BUILD_DIR'] = str(self.root / 'intermediates')
                self.make_build(); self.value('A|A|A')
                support = Path(self.metadata()['target_directory']) / 'native/support'
                support.mkdir(parents=True); (support / 'sentinel').write_text('KEEP')
                self.patch()
                self.make_build(); self.value('B|B|B')
                self.assertEqual((support / 'sentinel').read_text(), 'KEEP')
                warm = self.run_ok(['make', '--no-print-directory', 'build', 'CARGO_FLAGS=--workspace --offline --locked --message-format=json'])
                registry = [json.loads(line) for line in warm.stdout.splitlines() if line.startswith('{')]
                deps = [item for item in registry if item.get('reason') == 'compiler-artifact'
                        and any(name in item['package_id'] for name in ('fixture-value', 'fixture-macro'))]
                self.assertTrue(deps)
                self.assertTrue(all(item['fresh'] for item in deps), 'UNCHANGED_VENDOR_MUST_STAY_WARM')

    def guarded_cargo(self, *args):
        helper = self.root / 'tools/dev/cargo_artifacts.py'
        return [sys.executable, str(helper), 'run', CARGO, '--', *args]

    def test_raw_checksum_tamper_cannot_hide_behind_warm_artifact(self):
        self.make_build(); self.value('A|A|A')
        self.patch(checksums=False)
        result = self.execute(['make', '--no-print-directory', 'build'])
        self.assertNotEqual(result.returncode, 0, 'TAMPERED_VENDOR_MUST_FAIL')
        self.assertIn('checksum', result.stderr)

    def test_vendor_symlink_read_failure_stops_before_compile(self):
        self.make_build()
        source = self.root / 'vendor/fixture-value/src/lib.rs'
        external = self.root / 'external.rs'
        external.write_bytes(source.read_bytes())
        source.unlink(); source.symlink_to(external)
        result = self.execute(['make', '--no-print-directory', 'build'])
        self.assertNotEqual(result.returncode, 0, 'UNREADABLE_OR_INDIRECT_VENDOR_MUST_FAIL_CLOSED')
        self.assertNotIn('Compiling', result.stderr)

    def test_cleanup_failure_keeps_old_generation_and_retries(self):
        self.make_build()
        target = Path(self.metadata()['target_directory'])
        support = target / 'native-toolchain/support'
        support.mkdir(parents=True); (support / 'keep').write_text('KEEP')
        deps = target / 'debug/deps'; saved = target / 'saved-deps'
        deps.rename(saved); deps.symlink_to(saved, target_is_directory=True)
        stamp = target / '.hidlins-vendor-generation'
        old = stamp.read_bytes() if stamp.exists() else None
        self.patch()
        result = self.execute(['make', '--no-print-directory', 'build'])
        self.assertNotEqual(result.returncode, 0, 'CLEANUP_FAILURE_MUST_FAIL_CLOSED')
        self.assertNotIn('Compiling', result.stderr)
        self.assertEqual(stamp.read_bytes() if stamp.exists() else None, old)
        deps.unlink(); saved.rename(deps)
        self.make_build(); self.value('B|B|B')
        self.assertEqual((support / 'keep').read_text(), 'KEEP')

    def test_failed_first_party_compile_preserves_warm_dependency_retry(self):
        self.make_build()
        self.patch()
        source = self.root / 'src/main.rs'; good = source.read_text()
        source.write_text('intentional invalid Rust\n')
        failed = self.execute(['make', '--no-print-directory', 'build'])
        self.assertNotEqual(failed.returncode, 0)
        source.write_text(good)
        retry = self.run_ok(['make', '--no-print-directory', 'build',
                             'CARGO_FLAGS=--workspace --offline --locked --message-format=json'])
        events = [json.loads(line) for line in retry.stdout.splitlines() if line.startswith('{')]
        deps = [item for item in events if item.get('reason') == 'compiler-artifact'
                and any(name in item['package_id'] for name in ('fixture-value', 'fixture-macro'))]
        self.assertTrue(deps)
        self.assertTrue(all(item['fresh'] for item in deps), 'RETRY_MUST_PRESERVE_WARM_DEPENDENCIES')
        self.value('B|B|B')

    def test_stamp_publication_and_partial_cleanup_fail_closed(self):
        # Fault injection owns only the isolated fixture; no chmod assumptions
        # (which would become ineffective under a privileged test runner).
        helper = self.root / 'tools/dev/vendor_cache.py'
        self.assertTrue(helper.exists(), 'CACHE_FAILURE_POLICY_MISSING')
        sys.path.insert(0, str(helper.parent))
        self.addCleanup(sys.path.remove, str(helper.parent))
        spec = importlib.util.spec_from_file_location('fixture_vendor_cache', helper)
        module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
        self.make_build()
        target = Path(self.metadata()['target_directory']); stamp = target / module.STAMP
        old = stamp.read_bytes(); self.patch()
        real_remove = module.remove
        removed = []
        def interrupted(path):
            if path.is_file():
                if removed: raise OSError('injected partial cleanup failure')
                removed.append(path)
            real_remove(path)
        with mock.patch.object(module, 'remove', side_effect=interrupted):
            with self.assertRaisesRegex(OSError, 'partial cleanup'):
                module.invalidate(self.root, [target])
        self.assertEqual(stamp.read_bytes(), old)
        with mock.patch.object(module.os, 'replace', side_effect=OSError('injected stamp failure')):
            with self.assertRaisesRegex(OSError, 'stamp failure'):
                module.invalidate(self.root, [target])
        self.assertEqual(stamp.read_bytes(), old)
        self.make_build(); self.value('B|B|B')

    def test_fuzz_runner_forwards_guarded_explicit_target_to_real_cargo(self):
        # This is a first-party CLI contract driver, not a claim that nightly
        # libFuzzer is installed. The cache backend still executes real Cargo.
        fuzz = self.root / 'fuzz'; fuzz.mkdir()
        for name in ('Cargo.toml', 'Cargo.lock', 'build.rs'):
            shutil.copyfile(self.root / name, fuzz / name)
        shutil.copytree(self.root / 'src', fuzz / 'src')
        for name in ('local_protocol', 'local_address', 'pairing_state'):
            path = fuzz / 'corpus' / name; path.mkdir(parents=True)
            (path / 'seed').write_bytes(b'original')
        runner = self.root / 'tools/local-sync-tests/run-fuzz.py'
        runner.parent.mkdir(parents=True)
        shutil.copyfile(ROOT / 'tools/local-sync-tests/run-fuzz.py', runner)
        binary = self.root / 'bin'; binary.mkdir()
        shim = binary / 'cargo'
        shim.write_text('#!' + sys.executable + '\n' + r'''
import json,os,pathlib,subprocess,sys
args=sys.argv[1:];root=pathlib.Path(os.environ['CACHE_FIXTURE_ROOT'])
if args==['fuzz','--version']:print('cargo-fuzz 0.13.2');sys.exit(0)
if args and args[0].startswith('+'):args=args[1:]
if args[0]=='metadata':os.execv(os.environ['CACHE_REAL_CARGO'],[os.environ['CACHE_REAL_CARGO'],*args])
assert args[:2]==['fuzz','run'],args
assert '--target-dir' in args,'FUZZ_EXPLICIT_CACHE_ROOT_MISSING'
target=args[args.index('--target-dir')+1]
with (root/'fuzz-commands.jsonl').open('a') as stream:stream.write(json.dumps(args)+'\n')
sys.exit(subprocess.run([os.environ['CACHE_REAL_CARGO'],'build','--offline','--locked',
  '--target',os.environ['CACHE_HOST'],'--target-dir',target]).returncode)
''')
        shim.chmod(0o755)
        environment = dict(self.env, PATH=str(binary) + os.pathsep + self.env['PATH'],
                           CACHE_FIXTURE_ROOT=str(self.root), CACHE_REAL_CARGO=CARGO, CACHE_HOST=HOST,
                           CARGO_TARGET_DIR=str(self.root / 'fuzz-output'))
        self.run_ok([sys.executable, str(runner)], env=environment)
        output = self.root / 'fuzz-output' / HOST / 'debug/cache-fixture'
        self.assertEqual(self.run_ok([str(output)]).stdout.strip(), 'A|A|A')
        self.patch()
        self.run_ok([sys.executable, str(runner)], env=environment)
        self.assertEqual(self.run_ok([str(output)]).stdout.strip(), 'B|B|B', 'FUZZ_CACHE_MUST_FOLLOW_VENDOR_BYTES')
        commands = [json.loads(line) for line in (self.root / 'fuzz-commands.jsonl').read_text().splitlines()]
        self.assertEqual(len(commands), 6)
        for index, command in enumerate(commands):
            self.assertIn('-runs=512', command)
            self.assertIn(f'-seed={0x4849444C + index % 3 + 1}', command)
            self.assertIn('-max_len=262144', command)
            self.assertEqual(command[command.index('--target-dir')+1], str(self.root / 'fuzz-output'))
        for seed in (fuzz / 'corpus').rglob('seed'):
            self.assertEqual(seed.read_bytes(), b'original')

if __name__ == '__main__':
    unittest.main(verbosity=2)
