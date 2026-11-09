#!/usr/bin/env python3
"""Regression tests for audited vendored-source patch guards."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
VENDOR_SCRIPT = ROOT / "tools" / "dev" / "vendor.py"

spec = importlib.util.spec_from_file_location("hidlins_vendor", VENDOR_SCRIPT)
if spec is None or spec.loader is None:
    raise RuntimeError(f"cannot load {VENDOR_SCRIPT}")
vendor = importlib.util.module_from_spec(spec)
spec.loader.exec_module(vendor)


class KeepassPatchReplayTests(unittest.TestCase):
    def test_borrowed_entry_comparison_helper_replays(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-borrowed-comparison-patch-") as temporary:
            vendor_dir = Path(temporary)
            crate = vendor_dir / "keepass"
            shutil.copytree(ROOT / "vendor" / "keepass", crate)
            source = crate / "src/db/merge.rs"
            reviewed = source.read_text()
            marker = "// Hidlins: compare borrowed Entry properties without cloning secrets/history.\n"
            boundary = "/// Check entry content, ignoring ordering/access times but retaining expiration."
            self.assertTrue(marker in reviewed, "borrowed comparison helper must be present")
            prefix, tail = reviewed.split(marker, 1)
            _, suffix = tail.split(boundary, 1)
            source.write_text(prefix + boundary + suffix)
            with patch.object(vendor, "VENDOR", vendor_dir):
                vendor.patch_keepass()
                self.assertEqual(source.read_text(), reviewed)
                checksum = (crate / ".cargo-checksum.json").read_bytes()
                vendor.patch_keepass()
                self.assertEqual((crate / ".cargo-checksum.json").read_bytes(), checksum)

    def test_restored_group_move_metadata_patch_replays(self) -> None:
        before = "            dest_group.times = source.times.clone();"
        after = "            dest_group.times = source.times.clone();\n            dest_group.previous_parent_group = source.previous_parent_group;"
        with tempfile.TemporaryDirectory(prefix="hidlins-group-move-metadata-patch-") as temporary:
            vendor_dir = Path(temporary)
            crate = vendor_dir / "keepass"
            shutil.copytree(ROOT / "vendor" / "keepass", crate)
            source = crate / "src/db/merge.rs"
            reviewed = source.read_text()
            source.write_text(reviewed.replace(after, before, 1))
            with patch.object(vendor, "VENDOR", vendor_dir):
                vendor.patch_keepass()
                self.assertTrue(after in source.read_text(), "restored group move metadata must replay")
                if after in reviewed:
                    self.assertEqual(source.read_text(), reviewed)
                checksum = (crate / ".cargo-checksum.json").read_bytes()
                vendor.patch_keepass()
                self.assertEqual((crate / ".cargo-checksum.json").read_bytes(), checksum)

    def test_move_deletion_order_patches_replay(self) -> None:
        replacements = [
            ("group.times.last_modification.or(group.times.location_changed)", "group.times.last_modification.max(group.times.location_changed)"),
            ("e.times.last_modification.or(e.times.location_changed)", "e.times.last_modification.max(e.times.location_changed)"),
            (".last_modification\n                .or(source_entry.times.location_changed)", ".last_modification\n                .max(source_entry.times.location_changed)"),
            (".last_modification\n                .or(dest_entry.times.location_changed)", ".last_modification\n                .max(dest_entry.times.location_changed)"),
        ]
        with tempfile.TemporaryDirectory(prefix="hidlins-move-order-patch-") as temporary:
            vendor_dir = Path(temporary)
            crate = vendor_dir / "keepass"
            shutil.copytree(ROOT / "vendor" / "keepass", crate)
            source = crate / "src/db/merge.rs"
            reviewed = source.read_text()
            original = reviewed
            for before, after in replacements:
                original = original.replace(after, before, 1)
            source.write_text(original)
            with patch.object(vendor, "VENDOR", vendor_dir):
                vendor.patch_keepass()
                for _, after in replacements:
                    self.assertTrue(after in source.read_text(), "move ordering repair must replay")
                if all(after in reviewed for _, after in replacements):
                    self.assertEqual(source.read_text(), reviewed)
                checksum = (crate / ".cargo-checksum.json").read_bytes()
                vendor.patch_keepass()
                self.assertEqual((crate / ".cargo-checksum.json").read_bytes(), checksum)

    def test_tombstone_union_patches_replay(self) -> None:
        union = "        // Hidlins: propagate deletions even when neither side has the object.\n        // Concrete timestamps supersede unknown dates; never move a marker back.\n        for (id, deleted_at) in &other.deleted_objects {\n            self.deleted_objects.entry(*id)\n                .and_modify(|existing| *existing = (*existing).max(*deleted_at))\n                .or_insert(*deleted_at);\n        }\n\n"
        before = "dest_db.deleted_objects.insert(id.uuid(), *deletion_time);"
        after = "dest_db.deleted_objects.entry(id.uuid())\n                .and_modify(|existing| *existing = (*existing).max(*deletion_time))\n                .or_insert(*deletion_time);"
        with tempfile.TemporaryDirectory(prefix="hidlins-tombstone-union-patch-") as temporary:
            vendor_dir = Path(temporary)
            crate = vendor_dir / "keepass"
            shutil.copytree(ROOT / "vendor" / "keepass", crate)
            source = crate / "src/db/merge.rs"
            reviewed = source.read_text()
            source.write_text(reviewed.replace(union, "", 1).replace(after, before))
            with patch.object(vendor, "VENDOR", vendor_dir):
                vendor.patch_keepass()
                self.assertTrue(union in source.read_text(), "tombstone union must replay")
                self.assertEqual(source.read_text().count(after), 3)
                if union in reviewed and reviewed.count(after) == 3:
                    self.assertEqual(source.read_text(), reviewed)
                checksum = (crate / ".cargo-checksum.json").read_bytes()
                vendor.patch_keepass()
                self.assertEqual((crate / ".cargo-checksum.json").read_bytes(), checksum)

    def test_recursive_group_update_patches_replay(self) -> None:
        replacements = [
            ("use std::{collections::HashSet, ops::Deref};", "use std::{collections::{HashMap, HashSet}, ops::Deref};"),
            ("fn get_last_update(group: GroupRef<'_>) -> Option<NaiveDateTime> {", "fn get_last_update(\n    group: GroupRef<'_>,\n    cache: &mut HashMap<crate::db::GroupId, Option<NaiveDateTime>>,\n) -> Option<NaiveDateTime> {\n    let id = group.id();\n    if let Some(update) = cache.get(&id) {\n        return *update;\n    }"),
            ("    group\n        .entries()\n", "    let latest = group\n        .entries()\n"),
            ("        .chain(\n            group\n                .groups()\n                .filter_map(|g| g.times.last_modification.or(g.times.location_changed)),\n        )", "        .chain(group.groups().filter_map(|child| get_last_update(child, cache)))"),
            ("        .chain(last_update)\n        .max()\n}", "        .chain(last_update)\n        .max();\n    cache.insert(id, latest);\n    latest\n}"),
            ("    let source_groups = source_db.groups.keys().cloned().collect::<HashSet<_>>();", "    let source_groups = source_db.groups.keys().cloned().collect::<HashSet<_>>();\n    // Visit each requested subtree only once, even when ancestors overlap.\n    let mut source_updates = HashMap::new();\n    let mut dest_updates = HashMap::new();"),
            ("get_last_update(source);", "get_last_update(source, &mut source_updates);"),
            ("get_last_update(dest.as_ref());", "get_last_update(dest.as_ref(), &mut dest_updates);"),
        ]
        with tempfile.TemporaryDirectory(prefix="hidlins-subtree-update-patch-") as temporary:
            vendor_dir = Path(temporary)
            crate = vendor_dir / "keepass"
            shutil.copytree(ROOT / "vendor" / "keepass", crate)
            source = crate / "src/db/merge.rs"
            reviewed = source.read_text()
            original = reviewed
            for before, after in reversed(replacements):
                original = original.replace(after, before, 1)
            source.write_text(original)
            with patch.object(vendor, "VENDOR", vendor_dir):
                vendor.patch_keepass()
                for _, after in replacements:
                    self.assertTrue(after in source.read_text(), "recursive subtree repair must replay")
                if all(after in reviewed for _, after in replacements):
                    self.assertEqual(source.read_text(), reviewed)
                checksum = (crate / ".cargo-checksum.json").read_bytes()
                vendor.patch_keepass()
                self.assertEqual((crate / ".cargo-checksum.json").read_bytes(), checksum)

    def test_expiration_merge_patches_replay(self) -> None:
        replacements = [
            ("            dest_entry.times.last_modification = source_entry.times.last_modification;\n            dest_entry.fields = source_entry.fields.clone();", "            dest_entry.times.last_modification = source_entry.times.last_modification;\n            dest_entry.times.expires = source_entry.times.expires;\n            dest_entry.times.expiry = source_entry.times.expiry;\n            dest_entry.fields = source_entry.fields.clone();"),
            ("fn have_entries_diverged(a: &Entry, b: &Entry) -> bool {\n    let new_times = Times::default();\n\n    let mut a = a.clone();\n    a.times = new_times.clone();\n    a.history = None;\n\n    let mut b = b.clone();\n    b.times = new_times.clone();\n    b.history = None;\n\n    !a.eq(&b)\n}", "fn have_entries_diverged(a: &Entry, b: &Entry) -> bool {\n    !same_entry_properties(a, b)\n        || a.times.expires != b.times.expires\n        || a.times.expiry != b.times.expiry\n        || a.attachments != b.attachments\n}"),
        ]
        with tempfile.TemporaryDirectory(prefix="hidlins-expiration-merge-patch-") as temporary:
            vendor_dir = Path(temporary)
            crate = vendor_dir / "keepass"
            shutil.copytree(ROOT / "vendor" / "keepass", crate)
            source = crate / "src/db/merge.rs"
            reviewed = source.read_text()
            original = reviewed
            for before, after in replacements:
                original = original.replace(after, before, 1)
            source.write_text(original)
            with patch.object(vendor, "VENDOR", vendor_dir):
                vendor.patch_keepass()
                for _, after in replacements:
                    self.assertTrue(after in source.read_text(), "expiration repair must replay")
                if all(after in reviewed for _, after in replacements):
                    self.assertEqual(source.read_text(), reviewed)
                checksum = (crate / ".cargo-checksum.json").read_bytes()
                vendor.patch_keepass()
                self.assertEqual((crate / ".cargo-checksum.json").read_bytes(), checksum)

    def test_history_union_deduplication_patch_replays(self) -> None:
        before = "    Ok(History { entries })\n}"
        after = """    // Hidlins: a timestamp can contain several distinct secret versions.
    // Pairwise merging can repeat one of them when one replica has a subset.
    // Remove only exact duplicates, retaining all different metadata/content.
    // Distinct timestamps cannot contain exact duplicate versions.
    if entries.windows(2).all(|pair| {
        pair[0].times.last_modification != pair[1].times.last_modification
    }) {
        return Ok(History { entries });
    }
    let mut unique: Vec<Entry> = Vec::with_capacity(entries.len());
    for entry in entries {
        // The merge emits descending timestamps. Compare only the equal-time
        // suffix so ordinary histories retain linear deduplication cost.
        let duplicate = unique.iter().rev()
            .take_while(|prior| prior.times.last_modification == entry.times.last_modification)
            .any(|prior| prior == &entry);
        if !duplicate {
            unique.push(entry);
        }
    }
    Ok(History { entries: unique })
}"""
        with tempfile.TemporaryDirectory(prefix="hidlins-history-union-patch-") as temporary:
            vendor_dir = Path(temporary)
            crate = vendor_dir / "keepass"
            shutil.copytree(ROOT / "vendor" / "keepass", crate)
            source = crate / "src/db/merge.rs"
            reviewed = source.read_text()
            source.write_text(reviewed.replace(after, before, 1))
            with patch.object(vendor, "VENDOR", vendor_dir):
                vendor.patch_keepass()
                self.assertTrue(after in source.read_text(), "history union deduplication must replay")
                if after in reviewed:
                    self.assertEqual(source.read_text(), reviewed)
                checksum = (crate / ".cargo-checksum.json").read_bytes()
                vendor.patch_keepass()
                self.assertEqual((crate / ".cargo-checksum.json").read_bytes(), checksum)

    def test_history_pool_import_replays_without_touching_other_bytes(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-history-pool-patch-") as temporary:
            vendor_dir = Path(temporary)
            crate = vendor_dir / "keepass"
            shutil.copytree(ROOT / "vendor" / "keepass", crate)
            source = crate / "src/db/merge.rs"
            reviewed = source.read_text()
            marker = "// Hidlins: historical attachment ids belong to the source pool.\n"
            boundary = "/// Merge two histories together, returning the merged history."
            self.assertTrue(marker in reviewed, "history pool normalization must be present")
            prefix, tail = reviewed.split(marker, 1)
            _, suffix = tail.split(boundary, 1)
            source.write_text(prefix + boundary + suffix)
            with patch.object(vendor, "VENDOR", vendor_dir):
                vendor.patch_keepass()
                self.assertEqual(source.read_text(), reviewed)
                checksum = (crate / ".cargo-checksum.json").read_bytes()
                vendor.patch_keepass()
                self.assertEqual((crate / ".cargo-checksum.json").read_bytes(), checksum)

    def test_equal_current_history_patch_replays_and_is_idempotent(self) -> None:
        before = """                return Err(MergeError::EntryModificationTimeNotUpdated(id));
            }
            continue;"""
        after = """                return Err(MergeError::EntryModificationTimeNotUpdated(id));
            }
            // Hidlins: equal current versions can carry different prior secrets.
            if dest_entry.history != source_history {
                let empty_history = History::default();
                dest_entry.history = Some(merge_history(
                    dest_entry.history.as_ref().unwrap_or(&empty_history),
                    source_history.as_ref().unwrap_or(&empty_history),
                    log,
                )?);
            }
            continue;"""
        with tempfile.TemporaryDirectory(prefix="hidlins-equal-history-patch-") as temporary:
            vendor_dir = Path(temporary)
            crate = vendor_dir / "keepass"
            shutil.copytree(ROOT / "vendor" / "keepass", crate)
            source = crate / "src/db/merge.rs"
            reviewed = source.read_text()
            source.write_text(reviewed.replace(after, before, 1))
            with patch.object(vendor, "VENDOR", vendor_dir):
                vendor.patch_keepass()
                self.assertTrue(after in source.read_text(), "equal-current history repair must replay")
                if after in reviewed:
                    self.assertEqual(source.read_text(), reviewed)
                checksum = (crate / ".cargo-checksum.json").read_bytes()
                vendor.patch_keepass()
                self.assertEqual((crate / ".cargo-checksum.json").read_bytes(), checksum)

    def test_value_erasure_patch_replays_and_preserves_other_bytes(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-value-drop-patch-") as temporary:
            vendor_dir = Path(temporary)
            crate = vendor_dir / "keepass"
            shutil.copytree(ROOT / "vendor" / "keepass", crate)
            source = crate / "src/db/types/value.rs"
            reviewed = source.read_text()
            marker = "// Hidlins: wipe owned unprotected values"
            original = reviewed.replace(
                "use zeroize::{Zeroize, ZeroizeOnDrop};", "use zeroize::Zeroize;"
            )
            if marker in original:
                prefix, block = original.split(marker, 1)
                _, suffix = block.split("impl<T: Zeroize + Display>", 1)
                original = prefix + "impl<T: Zeroize + Display>" + suffix
            source.write_text(original)
            with patch.object(vendor, "VENDOR", vendor_dir):
                vendor.patch_keepass()
                replayed = source.read_text()
                self.assertIn("ZeroizeOnDrop for Value<T>", replayed)
                if marker in reviewed:
                    self.assertEqual(replayed, reviewed)
                first = (crate / ".cargo-checksum.json").read_bytes()
                vendor.patch_keepass()
                self.assertEqual(source.read_text(), replayed)
                self.assertEqual((crate / ".cargo-checksum.json").read_bytes(), first)

    def test_string_value_patch_replays_from_original_annotation(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-keepass-patch-") as temporary:
            vendor_dir = Path(temporary)
            crate = vendor_dir / "keepass"
            shutil.copytree(ROOT / "vendor" / "keepass", crate)
            relative = Path("src/format/xml_db/entry.rs")
            reviewed = (crate / relative).read_text()
            prefix, block = reviewed.split("pub struct StringValue {", 1)
            value, suffix = block.split("    value: Option<String>,", 1)
            # Reconstruct this upstream annotation even when the committed
            # tree already contains the reviewed fix. The other audited
            # patches remain applied; no downloads are needed.
            value = value.replace('rename = "$text"', 'rename = "$value"', 1)
            if 'with = "cs_opt_string"' not in value:
                value = value.replace(
                    '        rename = "$value",\n',
                    '        rename = "$value",\n        with = "cs_opt_string",\n',
                    1,
                )
            (crate / relative).write_text(
                prefix + "pub struct StringValue {" + value
                + "    value: Option<String>," + suffix
            )
            with patch.object(vendor, "VENDOR", vendor_dir):
                vendor.patch_keepass()
                self.assertEqual((crate / relative).read_text(), reviewed)
                first = {
                    path.relative_to(crate): path.read_bytes()
                    for path in crate.rglob("*") if path.is_file()
                }
                vendor.patch_keepass()
                second = {
                    path.relative_to(crate): path.read_bytes()
                    for path in crate.rglob("*") if path.is_file()
                }
                self.assertEqual(first, second)


class QuickXmlTextPatchReplayTests(unittest.TestCase):
    def test_text_map_patch_replays_and_preserves_other_bytes(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-xml-patch-") as temporary:
            vendor_dir = Path(temporary)
            crate = vendor_dir / "quick-xml"
            shutil.copytree(ROOT / "vendor" / "quick-xml", crate)
            source = crate / "src/de/map.rs"
            reviewed = source.read_bytes()
            fixed = (
                b"        if self.has_text_field {\r\n"
                b"            Ok(())\r\n"
                b"        } else {\r\n"
                b"            self.de.skip_whitespaces()\r\n"
                b"        }\r\n"
            )
            original = b"        self.de.skip_whitespaces()\r\n"
            source.write_bytes(reviewed.replace(fixed, original, 1))
            with patch.object(vendor, "VENDOR", vendor_dir):
                vendor.patch_quick_xml()
                self.assertEqual(source.read_bytes(), reviewed)
                first = (crate / ".cargo-checksum.json").read_bytes()
                vendor.patch_quick_xml()
                self.assertEqual(source.read_bytes(), reviewed)
                self.assertEqual((crate / ".cargo-checksum.json").read_bytes(), first)


class SnowPatchGuardTests(unittest.TestCase):
    def copy_snow(self, parent: Path, name: str) -> Path:
        destination = parent / name
        shutil.copytree(ROOT / "vendor" / "snow", destination)
        return destination

    def test_committed_patched_tree_passes(self) -> None:
        vendor.verify_snow_patch(ROOT / "vendor" / "snow")

    def test_compiled_feature_set_is_exact(self) -> None:
        metadata = json.loads(
            subprocess.check_output(
                ["cargo", "metadata", "--offline", "--locked", "--format-version", "1"],
                cwd=ROOT,
            )
        )
        packages = {package["id"]: package for package in metadata["packages"]}
        snow_node = next(
            node
            for node in metadata["resolve"]["nodes"]
            if packages[node["id"]]["name"] == "snow"
        )
        self.assertEqual(
            set(snow_node["features"]),
            {
                "chacha20poly1305",
                "curve25519-dalek",
                "default-resolver",
                "getrandom",
                "sha2",
                "std",
                "use-chacha20poly1305",
                "use-curve25519",
                "use-getrandom",
                "use-sha2",
            },
        )

    def test_unselected_ring_graph_is_absent(self) -> None:
        lock = (ROOT / "Cargo.lock").read_text()
        self.assertNotIn('name = "ring"', lock)
        self.assertFalse((ROOT / "vendor" / "ring").exists())
        self.assertNotIn('"ring?/std"', (ROOT / "vendor/snow/Cargo.toml").read_text())

    def test_vendor_pruning_uses_full_lock_membership(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-vendor-prune-") as temporary:
            root = Path(temporary)
            vendor_dir = root / "vendor"
            vendor_dir.mkdir()
            retained = vendor_dir / "target-only"
            retained.mkdir()
            (retained / "Cargo.toml").write_text(
                '[package]\nname = "target-only"\nversion = "1.2.3"\n'
            )
            stale = vendor_dir / "stale"
            stale.mkdir()
            (stale / "Cargo.toml").write_text(
                '[package]\nname = "stale"\nversion = "9.9.9"\n'
            )
            lock_path = root / "Cargo.lock"
            lock_path.write_text(
                'version = 4\n\n[[package]]\nname = "target-only"\n'
                'version = "1.2.3"\nsource = "registry+https://example.invalid/index"\n'
            )

            vendor.prune_unlocked_vendor(vendor_dir, lock_path)

            self.assertTrue(retained.is_dir())
            self.assertFalse(stale.exists())

    def test_vendor_pruning_uses_union_of_isolated_lockfiles(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-vendor-union-") as temporary:
            root = Path(temporary)
            vendor_dir = root / "vendor"
            vendor_dir.mkdir()
            for name, version in (("production", "1.0.0"), ("fuzz-only", "2.0.0")):
                crate = vendor_dir / name
                crate.mkdir()
                (crate / "Cargo.toml").write_text(
                    f'[package]\nname = "{name}"\nversion = "{version}"\n'
                )
            production_lock = root / "Cargo.lock"
            production_lock.write_text(
                'version = 4\n\n[[package]]\nname = "production"\n'
                'version = "1.0.0"\nsource = "registry+https://example.invalid/index"\n'
            )
            fuzz_lock = root / "fuzz.lock"
            fuzz_lock.write_text(
                'version = 4\n\n[[package]]\nname = "fuzz-only"\n'
                'version = "2.0.0"\nsource = "registry+https://example.invalid/index"\n'
            )

            vendor.prune_unlocked_vendor(
                vendor_dir, production_lock, additional_lock_paths=(fuzz_lock,)
            )

            self.assertTrue((vendor_dir / "production").is_dir())
            self.assertTrue((vendor_dir / "fuzz-only").is_dir())

    def test_missing_zeroization_patch_fails(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-snow-patch-") as temporary:
            snow = self.copy_snow(Path(temporary), "unpatched")
            builder = snow / "src" / "builder.rs"
            source = builder.read_text()
            builder.write_text(source.replace("#[derive(ZeroizeOnDrop)]\n", "", 1))

            with self.assertRaisesRegex(RuntimeError, "src/builder.rs"):
                vendor.verify_snow_patch(snow)

    def test_source_drift_fails(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-snow-drift-") as temporary:
            snow = self.copy_snow(Path(temporary), "drifted")
            types = snow / "src" / "types.rs"
            types.write_text(types.read_text() + "\n")

            with self.assertRaisesRegex(RuntimeError, "src/types.rs"):
                vendor.verify_snow_patch(snow)

    def test_unknown_version_fails(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-snow-version-") as temporary:
            snow = self.copy_snow(Path(temporary), "wrong-version")
            manifest = snow / "Cargo.toml"
            manifest.write_text(
                manifest.read_text().replace('version = "0.10.0"', 'version = "0.10.1"', 1)
            )

            with self.assertRaisesRegex(RuntimeError, "audited for 0.10.0"):
                vendor.verify_snow_patch(snow)


if __name__ == "__main__":
    unittest.main()
