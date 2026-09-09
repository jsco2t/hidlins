#!/usr/bin/env python3
"""Enforce that NCSA-licensed libFuzzer code remains test-only.

The production Cargo graph and produced application artifacts are independent
boundaries.  Both must be clean: graph inspection prevents accidental linking,
while marker/archive inspection catches copied fuzz binaries or source bundles.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import zipfile


ROOT = Path(__file__).resolve().parents[2]
FUZZ_CRATE = "libfuzzer-sys"
FUZZ_VERSION = "0.4.13"
NCSA = "NCSA"
PACKAGE_TARGETS = (
    "release",
    "app-build-linux",
    "app-build-macos",
    "build-android",
    "app-build-android",
    "app-build-ios",
)
MARKERS = (
    b"libfuzzer-sys",
    b"LLVMFuzzerTestOneInput",
    b"LLVMFuzzerInitialize",
    b"rust_fuzzer_test_input",
    b"University of Illinois/NCSA Open Source License",
)
FUZZ_EXECUTABLE_NAMES = {"local_protocol", "local_address", "pairing_state"}


class BoundaryError(RuntimeError):
    """A production/fuzz boundary invariant was violated."""


def load_toml(path: Path) -> dict:
    try:
        return tomllib.loads(path.read_text())
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise BoundaryError(f"cannot parse {path}: {error}") from error


def check_policy(root: Path) -> None:
    production = load_toml(root / "deny.toml")
    licenses = production.get("licenses", {})
    if NCSA in licenses.get("allow", []):
        raise BoundaryError("production deny.toml must not globally allow NCSA")
    for exception in licenses.get("exceptions", []):
        if NCSA in exception.get("allow", []):
            raise BoundaryError("production deny.toml must not contain an NCSA exception")

    fuzz = load_toml(root / "fuzz" / "deny.toml")
    exceptions = fuzz.get("licenses", {}).get("exceptions", [])
    expected = {"allow": [NCSA], "crate": f"{FUZZ_CRATE}@{FUZZ_VERSION}"}
    if exceptions != [expected]:
        raise BoundaryError(
            "fuzz/deny.toml must contain only the exact "
            f"{FUZZ_CRATE}@{FUZZ_VERSION} NCSA exception"
        )


def check_workspaces(root: Path) -> None:
    production = load_toml(root / "Cargo.toml").get("workspace", {})
    members = production.get("members", [])
    excluded = production.get("exclude", [])
    if any(Path(member).parts[:1] == ("fuzz",) for member in members):
        raise BoundaryError("fuzz workspace must not be a production workspace member")
    if "fuzz" not in excluded:
        raise BoundaryError("production workspace must explicitly exclude fuzz")

    fuzz_manifest = load_toml(root / "fuzz" / "Cargo.toml")
    dependency = fuzz_manifest.get("dependencies", {}).get(FUZZ_CRATE)
    if not isinstance(dependency, dict) or dependency.get("version") != f"={FUZZ_VERSION}":
        raise BoundaryError(f"fuzz dependency must pin {FUZZ_CRATE} exactly to ={FUZZ_VERSION}")
    if not dependency.get("optional"):
        raise BoundaryError("libfuzzer-sys must be optional outside explicit fuzz builds")

    for manifest in sorted((root / "crates").glob("*/Cargo.toml")):
        data = load_toml(manifest)
        for section in ("dependencies", "dev-dependencies", "build-dependencies"):
            if FUZZ_CRATE in data.get(section, {}):
                raise BoundaryError(f"production manifest references {FUZZ_CRATE}: {manifest}")


def lock_packages(path: Path) -> list[dict]:
    return load_toml(path).get("package", [])


def check_lockfiles(root: Path) -> None:
    production = [p for p in lock_packages(root / "Cargo.lock") if p.get("name") == FUZZ_CRATE]
    if production:
        raise BoundaryError(f"production Cargo.lock contains {FUZZ_CRATE}")
    fuzz = [p for p in lock_packages(root / "fuzz" / "Cargo.lock") if p.get("name") == FUZZ_CRATE]
    if len(fuzz) != 1 or fuzz[0].get("version") != FUZZ_VERSION:
        raise BoundaryError(f"fuzz/Cargo.lock must contain exactly {FUZZ_CRATE} {FUZZ_VERSION}")


def check_production_graph(root: Path) -> None:
    completed = subprocess.run(
        ["cargo", "metadata", "--offline", "--locked", "--format-version", "1"],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise BoundaryError(f"cannot inspect production dependency graph: {completed.stderr.strip()}")
    metadata = json.loads(completed.stdout)
    offenders = sorted(
        f"{package['name']}@{package['version']}"
        for package in metadata["packages"]
        if package["name"] == FUZZ_CRATE
        or NCSA in str(package.get("license") or "").split()
    )
    if offenders:
        raise BoundaryError("production dependency graph contains NCSA/libFuzzer: " + ", ".join(offenders))


def target_block(makefile: str, target: str) -> str:
    lines = makefile.splitlines()
    start = next((index for index, line in enumerate(lines) if line.startswith(f"{target}:")), None)
    if start is None:
        raise BoundaryError(f"required packaging target is missing: {target}")
    end = start + 1
    while end < len(lines) and (lines[end].startswith("\t") or not lines[end].strip()):
        end += 1
    return "\n".join(lines[start:end])


def check_packaging_hooks(root: Path) -> None:
    makefile = (root / "Makefile").read_text()
    for target in PACKAGE_TARGETS:
        block = target_block(makefile, target)
        header = block.splitlines()[0]
        if "ncsa-boundary-precheck" not in header:
            raise BoundaryError(f"{target} is missing the NCSA pre-build boundary hook")
        if "ncsa-boundary-check.py artifacts" not in block:
            raise BoundaryError(f"{target} is missing post-build artifact inspection")


def inspect_bytes(label: str, data: bytes) -> None:
    lowered_name = Path(label).name.lower()
    if lowered_name in FUZZ_EXECUTABLE_NAMES:
        raise BoundaryError(f"fuzz executable found in application artifacts: {label}")
    for marker in MARKERS:
        if marker.lower() in data.lower():
            raise BoundaryError(f"NCSA/libFuzzer marker {marker!r} found in artifact: {label}")


def inspect_artifact(path: Path) -> None:
    if path.is_dir():
        for child in sorted(path.rglob("*")):
            if child.is_file():
                inspect_artifact(child)
        return
    if not path.is_file():
        raise BoundaryError(f"expected application artifact does not exist: {path}")
    if zipfile.is_zipfile(path):
        with zipfile.ZipFile(path) as archive:
            for info in archive.infolist():
                if info.is_dir():
                    continue
                inspect_bytes(f"{path}!{info.filename}", archive.read(info))
        return
    inspect_bytes(str(path), path.read_bytes())


def run_check(root: Path) -> None:
    check_policy(root)
    check_workspaces(root)
    check_lockfiles(root)
    check_production_graph(root)
    check_packaging_hooks(root)


def write_fixture(root: Path) -> None:
    (root / "crates" / "app").mkdir(parents=True, exist_ok=True)
    (root / "fuzz").mkdir(exist_ok=True)
    (root / "Cargo.toml").write_text('[workspace]\nmembers=["crates/app"]\nexclude=["fuzz"]\n')
    (root / "crates" / "app" / "Cargo.toml").write_text(
        '[package]\nname="app"\nversion="0.1.0"\n'
    )
    (root / "deny.toml").write_text('[licenses]\nallow=["MIT"]\nexceptions=[]\n')
    (root / "fuzz" / "Cargo.toml").write_text(
        '[package]\nname="fuzz"\nversion="0.0.0"\n'
        '[workspace]\n[dependencies]\n'
        f'{FUZZ_CRATE}={{version="={FUZZ_VERSION}", optional=true}}\n'
    )
    (root / "fuzz" / "deny.toml").write_text(
        '[[licenses.exceptions]]\nallow=["NCSA"]\n'
        f'crate="{FUZZ_CRATE}@{FUZZ_VERSION}"\n'
    )
    (root / "Cargo.lock").write_text(
        'version = 4\n\n[[package]]\nname = "app"\nversion = "0.1.0"\n'
    )
    (root / "fuzz" / "Cargo.lock").write_text(
        'version = 4\n\n[[package]]\nname = "libfuzzer-sys"\n'
        f'version = "{FUZZ_VERSION}"\n'
    )


def expect_failure(action, phrase: str) -> None:
    try:
        action()
    except BoundaryError as error:
        if phrase not in str(error):
            raise BoundaryError(f"negative control failed for wrong reason: {error}") from error
    else:
        raise BoundaryError(f"negative control unexpectedly passed: {phrase}")


def self_test() -> None:
    with tempfile.TemporaryDirectory(prefix="hidlins-ncsa-boundary-") as temporary:
        root = Path(temporary)
        write_fixture(root)
        check_policy(root)
        check_workspaces(root)

        production_manifest = root / "crates" / "app" / "Cargo.toml"
        production_manifest.write_text(
            production_manifest.read_text() + f'\n[dependencies]\n{FUZZ_CRATE}="={FUZZ_VERSION}"\n'
        )
        expect_failure(lambda: check_workspaces(root), "production manifest references")
        production_manifest.write_text('[package]\nname="app"\nversion="0.1.0"\n')

        production_lock = root / "Cargo.lock"
        production_lock.write_text(
            production_lock.read_text()
            + f'\n[[package]]\nname = "{FUZZ_CRATE}"\nversion = "{FUZZ_VERSION}"\n'
        )
        expect_failure(lambda: check_lockfiles(root), "production Cargo.lock contains")
        write_fixture(root)

        fuzz_policy = root / "fuzz" / "deny.toml"
        fuzz_policy.write_text(
            '[[licenses.exceptions]]\nallow=["NCSA"]\ncrate="different-crate@1.0.0"\n'
        )
        expect_failure(lambda: check_policy(root), "must contain only the exact")

        makefile = root / "Makefile"
        makefile.write_text("release:\n\ttrue\n")
        expect_failure(lambda: check_packaging_hooks(root), "missing the NCSA pre-build")

        for relative in (
            "target/release/hidlins",
            "target/release/hidlins-tui",
            "target/release/hidlins-agent",
            "app/build/linux/Hidlins",
            "app/build/macos/Hidlins.app/Contents/MacOS/Hidlins",
            "app/build/ios/Hidlins.app/Hidlins",
        ):
            contaminated = root / relative
            contaminated.parent.mkdir(parents=True, exist_ok=True)
            contaminated.write_bytes(b"prefix LLVMFuzzerTestOneInput suffix")
            expect_failure(lambda path=contaminated: inspect_artifact(path), "marker")

        copied_fuzzer = root / "bundle" / "local_protocol"
        copied_fuzzer.parent.mkdir()
        copied_fuzzer.write_bytes(b"otherwise marker-free")
        expect_failure(lambda: inspect_artifact(copied_fuzzer.parent), "fuzz executable")

        for name, member in (
            ("app.apk", "lib/arm64-v8a/libapp.so"),
            ("app.ipa", "Payload/Hidlins.app/Hidlins"),
        ):
            archive = root / name
            with zipfile.ZipFile(archive, "w") as output:
                output.writestr(member, b"libfuzzer-sys")
            expect_failure(lambda path=archive: inspect_artifact(path), "marker")


def main() -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("check")
    subparsers.add_parser("self-test")
    artifacts = subparsers.add_parser("artifacts")
    artifacts.add_argument("paths", nargs="+", type=Path)
    args = parser.parse_args()
    try:
        if args.command == "check":
            run_check(ROOT)
        elif args.command == "self-test":
            self_test()
        else:
            for path in args.paths:
                inspect_artifact(path)
    except (BoundaryError, OSError, json.JSONDecodeError, zipfile.BadZipFile) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print("NCSA/libFuzzer development-only boundary is intact.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
