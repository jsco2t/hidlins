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
    def __init__(self, label: str = "runTest", flags: tuple[str, ...] = ()):
        super().__init__("runTest")
        self.label = label
        self.flags = flags

    def shortDescription(self):
        return self.label

    def runTest(self):
        command = [
            "cargo", "check", "-p", "hidlins-api", "--lib", "--offline", "--locked",
            *self.flags,
        ]
        result = subprocess.run(
            command, cwd=ROOT, text=True, stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT, timeout=600, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stdout)


def feature_suite():
    cases = [("default API configuration", ())]
    # logind's implementation and dependency exist only on Linux. Both Apple
    # targets are already required by the repository's check-macos toolchain.
    if platform.system() == "Linux":
        cases.append(("native Linux logind API", ("--features", "logind")))
    for target in ("aarch64-apple-ios", "aarch64-apple-ios-sim", "x86_64-apple-ios",
                   "aarch64-linux-android", "x86_64-linux-android"):
        cases.append((f"native API {target}", ("--target", target, "--no-default-features")))
    for architecture in ("aarch64", "x86_64"):
        cases.append((
            f"{architecture} Apple IOKit API",
            ("--target", f"{architecture}-apple-darwin", "--features", "iokit"),
        ))
    return unittest.TestSuite(ApiFeatureCompile(label, flags) for label, flags in cases)


if __name__ == "__main__":
    result = unittest.TextTestRunner(verbosity=2).run(feature_suite())
    raise SystemExit(not result.wasSuccessful())

# Cross-target compilation runs through make test-api-lock-features.
def load_tests(loader, tests, pattern):
    return unittest.TestSuite()
