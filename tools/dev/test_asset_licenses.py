"""Negative controls for the narrowly scoped non-code asset exception."""

from pathlib import Path
import shutil
import tempfile
import tomllib
import unittest

import asset_license_check as policy


class AssetLicenseTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name)
        for relative in (
            policy.WORDLIST, policy.NOTICE, policy.APP_NOTICE, "CLAUDE.md",
            "crates/hidlins-genpw/LICENSES/EFF-wordlist.md",
            "deny.toml", "fuzz/deny.toml",
        ):
            destination = self.root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(policy.ROOT / relative, destination)
        self.assertEqual(policy.check(self.root), [])

    def test_actual_repository_satisfies_exception(self):
        self.assertEqual(policy.check(policy.ROOT), [])

    def test_replacement_wordlist_is_not_authorized(self):
        with (self.root / policy.WORDLIST).open("a") as stream:
            stream.write("66667\treplacement\n")
        self.assertTrue(any("wordlist digest" in e for e in policy.check(self.root)))

    def test_exception_requires_the_exact_hash(self):
        path = self.root / "CLAUDE.md"
        path.write_text(path.read_text().replace(policy.DIGEST, "0" * 64))
        self.assertTrue(any("policy lacks exact" in e for e in policy.check(self.root)))

    def test_asset_exception_does_not_allow_code_license_exceptions(self):
        for relative in ("deny.toml", "fuzz/deny.toml"):
            with self.subTest(policy=relative):
                path = self.root / relative
                original = path.read_text()
                changed = original.replace(
                    'exceptions = [',
                    'exceptions = [{ name="example", allow=["CC-BY-4.0"] },',
                    1,
                )
                tomllib.loads(changed)
                path.write_text(changed)
                self.assertTrue(any("broadened code licenses" in e for e in policy.check(self.root)))
                path.write_text(original)

    def test_asset_exception_does_not_allow_global_code_licenses(self):
        for relative in ("deny.toml", "fuzz/deny.toml"):
            with self.subTest(policy=relative):
                path = self.root / relative
                original = path.read_text()
                changed = original.replace('allow = [', 'allow = ["CC-BY-4.0",', 1)
                tomllib.loads(changed)
                path.write_text(changed)
                self.assertTrue(any("broadened code licenses" in e for e in policy.check(self.root)))
                path.write_text(original)

    def test_compiled_notice_must_match_and_be_registered(self):
        path = self.root / policy.APP_NOTICE
        original = path.read_text()
        path.write_text("missing attribution")
        self.assertTrue(any("Flutter notice differs" in e for e in policy.check(self.root)))
        path.write_text(original.replace("LicenseRegistry.addLicense", "omitted_registration"))
        self.assertTrue(any("does not register" in e for e in policy.check(self.root)))

    def test_creator_attribution_is_required(self):
        path = self.root / policy.NOTICE
        path.write_text(path.read_text().replace("Joseph Bonneau", ""))
        self.assertTrue(any("Joseph Bonneau" in e for e in policy.check(self.root)))

    def test_optional_authoritative_specs_are_checked(self):
        specs = self.root / "project"
        for relative in policy.SPEC_FILES:
            path = specs / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(f"Exact non-code asset CC-BY-4.0 exception: {policy.DIGEST}\n")
        self.assertEqual(policy.check(self.root, specs), [])
        path = specs / policy.SPEC_FILES[-1]
        path.write_text(path.read_text() + "CC0-equivalent\n")
        self.assertTrue(any("unsupported asset license" in e for e in policy.check(self.root, specs)))


if __name__ == "__main__":
    unittest.main()
