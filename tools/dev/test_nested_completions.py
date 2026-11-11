#!/usr/bin/env python3
"""Real nested Cargo/Make completion generation under the shared cache guard.

The tiny compiled packages are first-party MIT fixtures, not downloaded code.
"""
from __future__ import annotations

import os
from pathlib import Path
import shutil
import signal
import subprocess
import unittest

import test_vendor_cache as fixtures


FILES = {"bash": "hidlins.bash", "zsh": "_hidlins", "fish": "hidlins.fish"}
LIBRARY = r'''
pub fn script(shell: &str) -> String {
    format!("{}-{shell}\n", fixture_value::value())
}
pub fn generate() {
    let root = std::path::Path::new("shell-completions");
    std::fs::create_dir_all(root).unwrap();
    for (shell, file) in [("bash", "hidlins.bash"), ("zsh", "_hidlins"), ("fish", "hidlins.fish")] {
        std::fs::write(root.join(file), script(shell)).unwrap();
    }
}
'''
MAIN = r'''
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3);
    assert_eq!(args[1], "completions");
    if args[2] == "fish" && std::env::var_os("FIXTURE_COMPLETIONS_FAIL_FISH").is_some() {
        eprintln!("synthetic late completion failure");
        std::process::exit(17);
    }
    print!("{}", hidlins_cli::script(&args[2]));
}
'''
NESTED = r'''
#[test]
fn already_built_cli_generates_completions_inside_outer_guard() {
    println!("NESTED_COMPLETIONS_ENTERED");
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    let output = std::process::Command::new("make")
        .args(["--no-print-directory", "completions"])
        .env("HIDLINS_COMPLETIONS_BIN", env!("CARGO_BIN_EXE_hidlins"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    for (shell, file) in [("bash", "hidlins.bash"), ("zsh", "_hidlins"), ("fish", "hidlins.fish")] {
        assert_eq!(std::fs::read_to_string(format!("shell-completions/{file}")).unwrap(),
                   hidlins_cli::script(shell));
    }
}
'''


class NestedCompletionsTests(unittest.TestCase):
    def fixture(self, mode="configuration"):
        case = fixtures.CacheTests()
        case.setUp()
        self.addCleanup(case.doCleanups)
        manifest = case.root / "Cargo.toml"
        manifest.write_text(manifest.read_text().replace('name="cache-fixture"', 'name="hidlins-cli"')
                            + '\n[[bin]]\nname="hidlins"\npath="src/main.rs"\n')
        (case.root / "src/lib.rs").write_text(LIBRARY)
        (case.root / "src/main.rs").write_text(MAIN)
        (case.root / "examples").mkdir()
        (case.root / "examples/gen_completions.rs").write_text("fn main() { hidlins_cli::generate(); }\n")
        (case.root / "tests").mkdir()
        (case.root / "tests/nested_completions.rs").write_text(NESTED)
        helper = fixtures.ROOT / "tools/dev/generate_completions.py"
        if helper.exists():
            shutil.copyfile(helper, case.root / "tools/dev/generate_completions.py")
        if mode == "environment":
            case.env["CARGO_TARGET_DIR"] = str(case.root / "environment output with spaces")
            case.env["CARGO_BUILD_BUILD_DIR"] = str(case.root / "environment intermediates with spaces")
        else:
            with case.config.open("a") as stream:
                stream.write('[build]\ntarget-dir="configured output with spaces"\n'
                             'build-dir="configured intermediates with spaces"\n')
        case.run_ok([fixtures.CARGO, "generate-lockfile", "--offline"])
        return case

    def bounded(self, case, command, environment=None):
        process = subprocess.Popen(command, cwd=case.root, env=environment or case.env,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   text=True, start_new_session=True)
        try:
            output, errors = process.communicate(timeout=15)
        except subprocess.TimeoutExpired:
            # Make, Cargo and its test may all retain the output pipes. Kill
            # and reap the entire fresh, owned group before failing/cleanup.
            os.killpg(process.pid, signal.SIGKILL)
            output, errors = process.communicate(timeout=5)
            self.assertIn("NESTED_COMPLETIONS_ENTERED", output + errors,
                          "timeout must reach the actual nested fixture, not fail to build it")
            self.fail("NESTED_COMPLETIONS_BLOCKED_ON_OUTER_GUARD\n" + output + errors)
        return subprocess.CompletedProcess(command, process.returncode, output, errors)

    def assert_files(self, case, value):
        for shell, name in FILES.items():
            self.assertEqual((case.root / "shell-completions" / name).read_bytes(),
                             f"{value}-{shell}\n".encode())

    def test_real_nested_cargo_test_finishes_and_observes_changed_vendor_bytes(self):
        for mode in ("environment", "configuration"):
            with self.subTest(roots=mode):
                case = self.fixture(mode)
                command = ["make", "--no-print-directory", "test",
                           "CARGO_TEST_FLAGS=",  # This tiny fixture has no security crate/helpers.
                           "CARGO_FLAGS=--offline --locked --test nested_completions -- --nocapture --test-threads=1"]
                result = self.bounded(case, command)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertIn("NESTED_COMPLETIONS_ENTERED", result.stdout)
                self.assert_files(case, "A")
                case.patch()  # same sizes/restored mtimes; checksum and bytes change
                result = self.bounded(case, command)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assert_files(case, "B")

    def test_default_make_keeps_guarded_recompilation_after_vendor_change(self):
        case = self.fixture()
        case.run_ok(["make", "--no-print-directory", "completions"])
        self.assert_files(case, "A")
        case.patch()
        case.run_ok(["make", "--no-print-directory", "completions"])
        self.assert_files(case, "B")

    def seed(self, case):
        out = case.root / "shell-completions"
        out.mkdir()
        for name in FILES.values():
            (out / name).write_bytes(b"existing completion must survive")

    def assert_seed_unchanged(self, case):
        for name in FILES.values():
            self.assertEqual((case.root / "shell-completions" / name).read_bytes(),
                             b"existing completion must survive")

    def test_explicit_missing_binary_fails_without_cargo_fallback(self):
        case = self.fixture()
        self.seed(case)
        environment = dict(case.env, HIDLINS_COMPLETIONS_BIN=str(case.root / "missing CLI"))
        result = case.execute(["make", "--no-print-directory", "completions"], env=environment)
        self.assertNotEqual(result.returncode, 0)
        self.assert_seed_unchanged(case)

    def test_late_explicit_cli_failure_keeps_all_existing_files(self):
        case = self.fixture()
        case.make_build()
        binary = Path(case.metadata()["target_directory"]) / "debug/hidlins"
        self.assertTrue(binary.is_file(), "the fixture must compile its real CLI first")
        self.seed(case)
        environment = dict(case.env, HIDLINS_COMPLETIONS_BIN=str(binary),
                           FIXTURE_COMPLETIONS_FAIL_FISH="1")
        result = case.execute(["make", "--no-print-directory", "completions"], env=environment)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("synthetic late completion failure", result.stderr)
        self.assert_seed_unchanged(case)


if __name__ == "__main__":
    unittest.main(verbosity=2)
