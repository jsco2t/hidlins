"""Compile real API feature branches; invoke with make test-api-lock-features.

These are type checks, with no claim about native event delivery or linking.
"""

from pathlib import Path
import json
import os
import platform
import shlex
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


class ApiFeatureCompile(unittest.TestCase):
    def __init__(self, label: str, flags: tuple[str, ...]):
        super().__init__("runTest")
        self.label = label
        self.flags = flags

    def shortDescription(self):
        return self.label

    def runTest(self):
        if platform.system() != "Darwin" and "--target" in self.flags:
            self.check_apple_caller()
            return
        command = [
            "cargo", "check", "-p", "hidlins-api", "--lib", "--offline", "--locked",
            *self.flags,
        ]
        result = subprocess.run(
            command, cwd=ROOT, text=True, stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT, timeout=600, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stdout)

    def check_apple_caller(self):
        """Compile the actual Rust caller without requiring Apple C SDKs.

        The complete API depends on dart-sys/oslog C build scripts. On Apple
        hosts runTest checks that complete crate; other hosts type-check the
        unchanged API function against the real target security library.
        This establishes constructor/trait agreement, not native execution.
        """
        target = self.flags[self.flags.index("--target") + 1]
        build = subprocess.run(
            ["cargo", "check", "-p", "hidlins-security", "--lib", "--features",
             "iokit", "--target", target, "--offline", "--locked",
             "--message-format=json"],
            cwd=ROOT, text=True, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, timeout=600, check=False,
        )
        self.assertEqual(build.returncode, 0, build.stdout + build.stderr)
        metadata = []
        for line in build.stdout.splitlines():
            item = json.loads(line)
            if (item.get("reason") == "compiler-artifact"
                    and item["target"]["name"] == "hidlins_security"):
                metadata.extend(Path(p) for p in item["filenames"] if p.endswith(".rmeta"))
        self.assertEqual(len(metadata), 1, "expected actual target security metadata")

        source = (ROOT / "crates/hidlins-api/src/api/session.rs").read_text()
        start = source.index("fn attach_platform_lock_sources(")
        end = source.index("\n#[cfg(not(any(", start)
        caller = source[start:end]
        with tempfile.TemporaryDirectory(prefix="hidlins-api-lock-types-") as scratch:
            probe = Path(scratch) / "caller.rs"
            probe.write_text(
                "use hidlins_security::AutoLockController;\n" + caller
                + "\npub fn verify(controller: AutoLockController) "
                + "-> (AutoLockController, Vec<String>) { "
                + "attach_platform_lock_sources(controller) }\n"
            )
            command = [
                "rustc", "--edition=2021", "--crate-type=lib", "--target", target,
                "--cfg", 'feature="iokit"', "--cfg", 'feature="desktop"',
                "--check-cfg", 'cfg(feature, values("desktop", "logind", "iokit"))',
                "--extern", f"hidlins_security={metadata[0]}",
                "-L", f"dependency={metadata[0].parent}",
                "-L", f"dependency={metadata[0].parents[3] / 'debug' / 'deps'}",
                "--emit=metadata",
                *shlex.split(os.environ.get("RUSTFLAGS", "")),
                str(probe), "-o", str(Path(scratch) / "caller.rmeta"),
            ]
            checked = subprocess.run(
                command, cwd=ROOT, text=True, stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT, timeout=600, check=False,
            )
            self.assertEqual(checked.returncode, 0, checked.stdout)


def feature_suite():
    cases = [("default API configuration", ())]
    # logind's implementation and dependency exist only on Linux. Both Apple
    # targets are already required by the repository's check-macos toolchain.
    if platform.system() == "Linux":
        cases.append(("native Linux logind API", ("--features", "logind")))
    for architecture in ("aarch64", "x86_64"):
        cases.append((
            f"{architecture} Apple IOKit API",
            ("--target", f"{architecture}-apple-darwin", "--features", "iokit"),
        ))
    return unittest.TestSuite(ApiFeatureCompile(label, flags) for label, flags in cases)


if __name__ == "__main__":
    result = unittest.TextTestRunner(verbosity=2).run(feature_suite())
    raise SystemExit(not result.wasSuccessful())
