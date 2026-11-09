#!/usr/bin/env python3
"""Exercise real helper builds under native and unified adapter features."""

import json
from pathlib import Path
import subprocess
import sys
import unittest


ROOT = Path(__file__).resolve().parents[2]
HOSTS = {
    "linux": ("logind_helper", "logind", "iokit_helper", "iokit", "macOS"),
    "darwin": ("iokit_helper", "iokit", "logind_helper", "logind", "Linux"),
}


class NativeHelperFeatures(unittest.TestCase):
    def setUp(self):
        self.assertIn(sys.platform, HOSTS, "native helper gate needs Linux or macOS")
        self.native, self.native_feature, self.foreign, self.foreign_feature, self.foreign_os = HOSTS[sys.platform]

    def build(self, features, binary=None):
        flags = ["-p", "hidlins-security", "--offline", "--locked",
                 "--features", features, "--message-format=json-render-diagnostics"]
        flags.extend(["--bin", binary] if binary else ["--bins"])
        result = subprocess.run(
            ["make", "--no-print-directory", "build", "CARGO_FLAGS=" + " ".join(flags)],
            cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            timeout=240, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stdout[-6000:] + result.stderr[-6000:])
        executables = {}
        for line in result.stdout.splitlines():
            try:
                message = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(message, dict) and message.get("reason") == "compiler-artifact":
                executable = message.get("executable")
                if executable:
                    executables[message["target"]["name"]] = Path(executable)
        return executables

    def test_00_native_only_helper_builds(self):
        built = self.build("test-binaries," + self.native_feature, self.native)
        self.assertIn(self.native, built)
        self.assertTrue(built[self.native].is_file())

    def test_10_unified_adapter_features_build_all_helpers(self):
        built = self.build("test-binaries,logind,iokit")
        for name in ["sigstop_helper", "logind_helper", "iokit_helper"]:
            with self.subTest(helper=name):
                self.assertIn(name, built)
                self.assertTrue(built[name].is_file())

    def test_20_foreign_helper_rejects_execution(self):
        built = self.build("test-binaries," + self.foreign_feature, self.foreign)
        self.assertIn(self.foreign, built)
        result = subprocess.run(
            [str(built[self.foreign])], cwd=ROOT, text=True,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=5, check=False,
        )
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, "")
        self.assertEqual(
            result.stderr,
            f"{self.foreign} requires {self.foreign_os} and the {self.foreign_feature} feature\n",
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
