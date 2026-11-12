"""Negative controls for the repository-wide license inventory."""
from pathlib import Path
import hashlib
import json
import shutil
import subprocess
import tempfile
import unittest

import repository_licenses as policy


def write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


class RepositoryLicenseTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="hidlins-license-inventory-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for relative, targets, exception in (
            ("deny.toml", policy.API_TARGETS, ""),
            ("fuzz/deny.toml", policy.DESKTOP_TARGETS,
             '{allow=["NCSA"],crate="libfuzzer-sys@0.4.13"}'),
        ):
            triples = ",".join('{triple=' + json.dumps(target) + '}' for target in sorted(targets))
            write(self.root / relative,
                  '[graph]\nall-features=true\ntargets=[' + triples + ']\n'
                  '[licenses]\nconfidence-threshold=0.93\nallow=' + json.dumps(sorted(policy.ALLOWED))
                  + '\nexceptions=[' + exception + ']\n')
        write(self.root / "rust-toolchain.toml", '[toolchain]\ntargets=' + json.dumps(sorted(policy.API_TARGETS)))
        write(self.root / "Cargo.toml", '[workspace]\nmembers=["crates/fixture"]\n[workspace.package]\nlicense="MIT"\n')
        for relative in ("crates/fixture/Cargo.toml", "fuzz/Cargo.toml", policy.FIXTURE + "/Cargo.toml"):
            write(self.root / relative, '[package]\nname="fixture"\nversion="0.1.0"\nlicense="MIT"\n')
        write(self.root / "LICENSE", (policy.ROOT / "LICENSE").read_text())
        write(self.root / "Cargo.lock", 'version=4\n[[package]]\nname="permitted"\nversion="1.0.0"\n'
              'source="registry+https://github.com/rust-lang/crates.io-index"\nchecksum="' + '1' * 64 + '"\n')
        for relative in ("fuzz/Cargo.lock", policy.FIXTURE + "/Cargo.lock"):
            write(self.root / relative, "version=4\n")
        self.vendor = self.root / "vendor/permitted"
        write(self.vendor / "Cargo.toml", '[package]\nname="permitted"\nversion="1.0.0"\nlicense="MIT"\n')
        write(self.vendor / "src/lib.rs", "pub fn permitted() {}\n")
        write(self.vendor / "LICENSE", (policy.ROOT / "LICENSE").read_text())
        files = {p.relative_to(self.vendor).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
                 for p in self.vendor.rglob("*") if p.is_file()}
        write(self.vendor / ".cargo-checksum.json", json.dumps({"files": files, "package": "1" * 64}))
        self.assertEqual(self.check(), 1)

    def check(self):
        return policy.check(self.root, [p.relative_to(self.root).as_posix()
                                       for p in self.root.rglob("*") if p.is_file()])

    def reject(self, phrase):
        with self.assertRaisesRegex(policy.PolicyError, phrase):
            self.check()

    def test_forbidden_licenses_and_broadened_exceptions_are_rejected(self):
        for relative in ("deny.toml", "fuzz/deny.toml"):
            path = self.root / relative
            original = path.read_text()
            for license_id in ("EPL-1.0", "EPL-2.0", "GPL-3.0", "NCSA", "CC-BY-4.0"):
                with self.subTest(policy=relative, license=license_id):
                    path.write_text(original.replace('allow=[', 'allow=[' + json.dumps(license_id) + ',', 1))
                    self.reject("allow-list differs")
            path.write_text(original)
        path = self.root / "fuzz/deny.toml"
        path.write_text(path.read_text().replace("libfuzzer-sys@0.4.13", "libfuzzer-sys@0.4.14"))
        self.reject("exceptions differ")

    def test_target_features_and_private_packages_cannot_escape_audit(self):
        path = self.root / "deny.toml"
        original = path.read_text()
        for before, after, phrase in (
            ("all-features=true", "all-features=false", "all optional"),
            ('{triple="aarch64-apple-ios"},', '', "target audit coverage"),
            ('[licenses]', '[licenses]\nprivate={ignore=true}', "private packages"),
            ('[graph]', '[graph]\nexclude=["permitted"]', "must not be excluded"),
        ):
            with self.subTest(mutation=phrase):
                self.assertIn(before, original)
                path.write_text(original.replace(before, after, 1))
                self.reject(phrase)
        path.write_text(original)

    def test_first_party_license_and_root_notice_are_required(self):
        path = self.root / policy.FIXTURE / "Cargo.toml"
        original = path.read_text()
        for value in ('license="EPL-2.0"', ''):
            path.write_text(original.replace('license="MIT"', value))
            self.reject("missing or non-MIT")
        path.write_text(original)
        (self.root / "LICENSE").write_text("missing license")
        self.reject("root MIT license")

    def test_unregistered_dependency_ecosystems_and_packages_are_rejected(self):
        for relative in ("hidden/pom.xml", "hidden/pubspec.yaml", "hidden/package.json",
                         "hidden/requirements.txt", "hidden/Cargo.toml", "hidden/Cargo.lock",
                         "hidden/LICENSE-GPL"):
            with self.subTest(path=relative):
                path = self.root / relative
                write(path, "unreviewed")
                self.reject("outside the audited|unregistered|unreviewed license")
                path.unlink()

    def test_modified_extra_and_missing_vendor_bytes_are_rejected(self):
        path = self.vendor / "src/lib.rs"
        original = path.read_text()
        path.write_text("modified source\n")
        self.reject("vendor bytes differ")
        path.write_text(original)
        extra = self.vendor / "EXTRA"
        write(extra, "unreviewed source")
        self.reject("file inventory differs")
        extra.unlink()
        path.unlink()
        self.reject("file inventory differs")

    def test_vendor_license_and_registry_provenance_are_required(self):
        path = self.vendor / "Cargo.toml"
        original = path.read_text()
        path.write_text(original.replace('license="MIT"', ''))
        self.reject("missing vendor license")
        path.write_text(original)
        path = self.vendor / ".cargo-checksum.json"
        value = json.loads(path.read_text()); value["package"] = "2" * 64
        path.write_text(json.dumps(value))
        self.reject("registry provenance differs")

    def test_orphan_missing_and_indirect_vendor_packages_are_rejected(self):
        extra = self.root / "vendor/orphan"
        shutil.copytree(self.vendor, extra)
        path = extra / "Cargo.toml"
        path.write_text(path.read_text().replace('name="permitted"', 'name="orphan"'))
        self.reject("orphan vendor package")
        shutil.rmtree(extra)
        indirect = self.vendor / "indirect"
        indirect.symlink_to(self.root, target_is_directory=True)
        self.reject("indirect vendor directory")
        indirect.unlink()
        shutil.rmtree(self.vendor)
        self.reject("locked vendor sources missing")

    def test_git_inventory_includes_untracked_metadata_and_rejects_gitlinks(self):
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        write(self.root / "new/pom.xml", "unreviewed")
        self.assertIn("new/pom.xml", policy.repository_paths(self.root))
        with self.assertRaisesRegex(policy.PolicyError, "outside the audited"):
            policy.check(self.root)
        subprocess.run(["git", "update-index", "--add", "--cacheinfo",
                        "160000," + "1" * 40 + ",embedded"], cwd=self.root, check=True)
        with self.assertRaisesRegex(policy.PolicyError, "gitlink"):
            policy.repository_paths(self.root)


if __name__ == "__main__":
    unittest.main(verbosity=2)
