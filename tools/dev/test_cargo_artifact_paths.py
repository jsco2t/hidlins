#!/usr/bin/env python3
"""Exercise artifact consumers with real Cargo metadata and isolated tool spies."""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
CARGO = shutil.which("cargo")
HOST_LIBRARY = "libhidlins_api.so"  # Tool spies model the Linux host contract.

# Spies replace compilation, UI and namespace effects. Metadata still goes
# through the installed Cargo, so the fixture does not implement Cargo's path rules.
SPY = r'''
import json,os,pathlib,subprocess,sys
p=pathlib.Path
env=os.environ
name=p(sys.argv[0]).name
args=sys.argv[1:]
root=p(env['FIXTURE_ROOT'])
out=p(env['FIXTURE_OUTPUT'])
def record(role,**fields):
 with (root/'receipts.jsonl').open('a') as f:f.write(json.dumps(dict(role=role,**fields))+'\n')
def require(value,message):
 if not value: print('FIXTURE_ASSERT '+message,file=sys.stderr);sys.exit(71)
if name=='cargo':
 if args[0]=='metadata':
  failure=env.get('FIXTURE_METADATA_FAILURE')
  if failure:
   print('{}' if failure=='malformed' else json.dumps({'target_directory':str(out)}))
   sys.exit(23 if failure=='nonzero' else 0)
  os.execv(env['FIXTURE_REAL_CARGO'],[env['FIXTURE_REAL_CARGO'],*args])
 record('cargo',args=args)
 for profile in ['debug','release']:
  directory=out/profile;directory.mkdir(parents=True,exist_ok=True)
  (directory/env['FIXTURE_LIBRARY_NAME']).write_text('FRESH_LIBRARY')
  (directory/'libhidlins_api.a').write_text('FRESH_ARCHIVE')
  for binary in ['hidlins','hidlins-agent','hidlins-test-driver','merge-interop-driver','api-test-driver','deps/hidlins_api','deps/cli_local_sync_process-fresh']:
   artifact=directory/binary;artifact.parent.mkdir(exist_ok=True)
   artifact.write_text('#!'+sys.executable+'\n'+(root/'spy.py').read_text());artifact.chmod(0o755)
 if args[0]=='test':
  require(env.get('HIDLINS_CLI_BIN')==str(out/'debug/hidlins'),'TUI CLI coexistence path')
  record('tui-process')
elif name=='inspect.py':
 require(args==['artifacts',*[str(out/'release'/x) for x in ['hidlins','hidlins-agent',env['FIXTURE_LIBRARY_NAME'],'libhidlins_api.a']]],'release inspection paths')
 record('release-inspection')
elif name=='ip': pass
elif name=='uname': print('x86_64' if args==['-m'] else 'Linux')
elif name=='sudo':
 if args==['-n','true']: sys.exit(0)
 require(args[0]=='ip','unexpected sudo action')
 if args[1:3]==['netns','exec']:
  require(args[4]=='env','namespace env missing')
  command=args[5:];child=dict(env)
  while command and '=' in command[0]:
   key,value=command.pop(0).split('=',1);child[key]=value
  sys.exit(subprocess.run(command,env=child,check=False).returncode)
else:
 require(p(sys.argv[0]).is_relative_to(out),'poison/default artifact selected')
 if name=='hidlins_api' and '--list' in args:
  print('pending_idle_lock_revokes_native_clipboard_ticket_before_handback: test')
  print('caught_panic_revokes_ticket_before_locked_event: test')
 if name=='api-test-driver': require(args[0]=='create-search-corpus','performance fixture command')
 record('driver',path=str(p(sys.argv[0])),args=args)
'''


class ArtifactPathTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="hidlins-artifacts-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "Cargo.toml").write_text(
            '[package]\nname="artifact_fixture"\nversion="0.1.0"\nedition="2021"\n'
            '[lib]\npath="lib.rs"\n'
        )
        (self.root / "lib.rs").write_text("pub fn fixture() {}\n")
        self.environment = dict(os.environ, RUSTUP_TOOLCHAIN="1.95.0", CARGO_NET_OFFLINE="true")
        for key in ("MAKEFLAGS", "MFLAGS", "MAKELEVEL", "MAKEFILES", "CARGO_TARGET_DIR"):
            self.environment.pop(key, None)
        subprocess.run([CARGO, "generate-lockfile", "--offline"], cwd=self.root,
                       env=self.environment, check=True, capture_output=True)
        shutil.copyfile(ROOT / "Makefile", self.root / "Makefile")
        (self.root / "spy.py").write_text(SPY)
        binaries = self.root / "bin"
        binaries.mkdir()
        for name in ("cargo", "ip", "sudo", "uname"):
            self.spy(binaries / name)
        self.environment.update(
            PATH=str(binaries) + os.pathsep + self.environment["PATH"],
            FIXTURE_ROOT=str(self.root), FIXTURE_REAL_CARGO=CARGO,
            FIXTURE_LIBRARY_NAME=HOST_LIBRARY,
        )
        for name in ("cargo_artifacts.py",):
            source = ROOT / "tools/dev" / name
            if source.exists():
                target = self.root / "tools/dev" / name
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, target)
        self.spy(self.root / "tools/dev/ncsa-boundary-check.py", dispatch="inspect.py")
        for name in ("us_090_rust_to_kpxc.sh", "us_091_kpxc_to_rust.sh", "us_092_round_trip.sh",
                     "entry_us-010.sh", "entry_us-012.sh", "entry_us-013.sh", "entry_us-014.sh", "entry_us-016.sh", "entry_us-018.sh", "sync_us-044.sh"):
            target = self.root / "tools/interop-tests" / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text('#!/bin/bash\n"${HIDLINS_TEST_DRIVER:-$HIDLINS_SYNC_MERGE_DRIVER}"\n')
            target.chmod(0o755)

    def spy(self, path, dispatch=None):
        path.parent.mkdir(parents=True, exist_ok=True)
        source = SPY
        if dispatch:
            source = source.replace("name=p(sys.argv[0]).name", "name=" + repr(dispatch))
        path.write_text("#!" + sys.executable + "\n" + source)
        path.chmod(0o755)

    def configure(self, mode="absolute"):
        if mode in ("config", "precedence"):
            config = self.root / ".cargo/config.toml"
            config.parent.mkdir()
            config.write_text('[build]\ntarget-dir="configured outputs"\n')
        if mode in ("absolute", "precedence"):
            self.environment["CARGO_TARGET_DIR"] = str(self.root / "absolute outputs")
        elif mode == "relative":
            self.environment["CARGO_TARGET_DIR"] = "relative outputs"
        metadata = subprocess.check_output(
            [CARGO, "metadata", "--no-deps", "--format-version", "1", "--offline", "--locked"],
            cwd=self.root, env=self.environment, text=True,
        )
        self.output = Path(json.loads(metadata)["target_directory"])
        self.environment.update(FIXTURE_OUTPUT=str(self.output))
        poison = self.root / "target"
        for name in ("debug/" + HOST_LIBRARY, "debug/hidlins", "debug/hidlins-test-driver",
                     "debug/merge-interop-driver", "debug/api-test-driver", "debug/deps/hidlins_api",
                     "debug/deps/cli_local_sync_process-poison"):
            path = poison / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("#!/bin/bash\nexit 79\n")
            path.chmod(0o755)

    def run_make(self, *targets, **environment):
        return subprocess.run(
            ["make", "--no-print-directory", "-o", "ncsa-boundary-precheck",
             "-o", "asset-license-check", "-o", "license-check", *targets, *([self.override] if hasattr(self, "override") else [])],
            cwd=self.root, env=dict(self.environment, **environment), text=True,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=30,
        )

    def receipts(self):
        path = self.root / "receipts.jsonl"
        return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []

    def test_release_inspects_the_built_output(self):
        self.configure()
        result = self.run_make("release")
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertTrue(any(r["role"] == "release-inspection" for r in self.receipts()))

    def test_native_harness_uses_config_root(self):
        self.configure("config")
        result = self.run_make("test-api-lock-cache-native-ticket", "test-api-worker-panic-ticket")
        self.assertEqual(result.returncode, 0, result.stdout)

    def test_interop_drivers_use_the_built_output(self):
        self.configure()
        result = self.run_make("interop", "interop-sync", "interop-entry")
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual(sum(r["role"] == "driver" for r in self.receipts()), 10)

    def test_metadata_failures_stop_consumers(self):
        self.configure("default")
        for failure in ("nonzero", "malformed"):
            with self.subTest(failure=failure):
                result = self.run_make("test-api-lock-cache-native-ticket", FIXTURE_METADATA_FAILURE=failure)
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertFalse(any(r["role"] == "driver" for r in self.receipts()))

    def test_help_does_not_require_cargo(self):
        result = self.run_make("help", "CARGO=nonexistent-cargo-command")
        self.assertEqual(result.returncode, 0, result.stdout)

    def test_network_script_selects_configured_absolute_candidate(self):
        self.configure()
        subprocess.run(["cargo", "build"], cwd=self.root, env=self.environment, check=True)
        path = self.root / "tools/local-sync-tests/isolated-network.sh"
        path.parent.mkdir(parents=True)
        shutil.copyfile(ROOT / "tools/local-sync-tests/isolated-network.sh", path)
        result = subprocess.run(["bash", str(path)], cwd=self.root, env=self.environment,
                                text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=30)
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual(sum(r["role"] == "driver" for r in self.receipts()), 2)


if __name__ == "__main__":
    unittest.main(verbosity=2)
