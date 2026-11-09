#!/usr/bin/env python3
"""Exercise the actual broad API generator without rewriting shipped bindings."""

import json
from pathlib import Path
import re
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


class ApiBridgeBoundary(unittest.TestCase):
    def test_private_cache_is_not_a_bridge_surface(self):
        # FRB requires Rust outputs beneath the crate's src directory to derive
        # a module namespace. This temporary directory is never declared as a
        # Rust module and is removed before the check returns.
        with tempfile.TemporaryDirectory(
            prefix="api_codegen_regression_", dir=ROOT / "crates/hidlins-api/src"
        ) as directory, tempfile.TemporaryDirectory(
            prefix="api_codegen_regression_", dir=ROOT / "app/lib"
        ) as dart_directory:
            scratch = Path(directory)
            # Preserve canonical parsing policy, replacing only output locations
            # and disabling presentation-only formatting/build_runner work.
            config = (ROOT / "flutter_rust_bridge.yaml").read_text()
            replacements = {
                "rust_root": str(ROOT / "crates/hidlins-api"),
                "dart_output": dart_directory,
                "rust_output": str(scratch / "frb_generated.rs"),
                "c_output": str(scratch / "frb_generated.h"),
            }
            for key, value in replacements.items():
                config, count = re.subn(
                    rf"(?m)^{key}:.*$", f"{key}: {json.dumps(value)}", config
                )
                self.assertEqual(count, 1, f"canonical {key} must be explicit")
            config += (
                f"\nbase_dir: {json.dumps(str(ROOT))}\n"
                f"dart_root: {json.dumps(str(ROOT / 'app'))}\n"
                "add_mod_to_lib: false\n"
                "build_runner: false\n"
                "dart_format: false\n"
                "dart_fix: false\n"
                "rust_format: false\n"
                "auto_upgrade_dependency: false\n"
            )
            config_path = scratch / "config.yaml"
            config_path.write_text(config)
            subprocess.run(
                ["flutter_rust_bridge_codegen", "generate", "--config-file", str(config_path)],
                cwd=ROOT,
                check=True,
                timeout=180,
            )
            generated = (scratch / "frb_generated.rs").read_text()
            operations = set(re.findall(r"fn (wire__\w+)_impl\(", generated))
            baseline = set(
                re.findall(
                    r"fn (wire__\w+)_impl\(",
                    (ROOT / "crates/hidlins-api/src/frb_generated.rs").read_text(),
                )
            )
            self.assertGreaterEqual(len(baseline), 65, "all existing public operations are the control")
            self.assertTrue(baseline <= operations, "generation must preserve every public operation")
            for private in ("TotpCache", "RustAutoOpaqueInner<Slot>", "totp_cache_default"):
                self.assertFalse(private in generated, f"private Rust cache exported: {private}")
            self.assertEqual(operations, baseline, "no unintended public operation")
            self.assertFalse((Path(dart_directory) / "api/totp_cache.dart").exists())


if __name__ == "__main__":
    unittest.main(verbosity=2)
