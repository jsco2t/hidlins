"""Negative controls for retained Rust application boundaries."""

from pathlib import Path
import tempfile
import unittest

import api_boundary_check as boundary


class ApiBoundaryTests(unittest.TestCase):
    def test_artifact_and_aliased_target_dependency_boundaries(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "crates" / "fixture" / "Cargo.toml"
            path.parent.mkdir(parents=True)
            manifest = '[package]\nname="hidlins-api"\nversion="0.1.0"\n[lib]\ncrate-type=["cdylib"]\n'
            path.write_text(manifest)
            self.assertEqual(boundary.check(root), [])
            path.write_text(manifest.replace('name="hidlins-api"', 'name="hidlins-core"'))
            self.assertTrue(any("native library" in error for error in boundary.check(root)))
            manifest += '\n[target.\'cfg(unix)\'.dependencies]\nclient={package="reqwest", version="1"}\n'
            path.write_text(manifest)
            self.assertTrue(any("network dependency" in error for error in boundary.check(root)))
            path.write_text(manifest.replace('name="hidlins-api"', 'name="hidlins-sync"').replace('crate-type=["cdylib"]', 'crate-type=["lib"]'))
            self.assertEqual(boundary.check(root), [])

    def test_empty_inventory_fails_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            self.assertTrue(boundary.check(Path(temporary)))
