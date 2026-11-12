#!/usr/bin/env python3
"""Audit repository license policy, first-party declarations and vendor provenance.

Cargo-deny owns SPDX expression evaluation for the supported feature/target
closures. This companion closes gaps outside that graph: source inventory,
first-party fixtures, exact exceptions and unused or modified vendor trees.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[2]
ALLOWED = {
    "MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Zlib",
    "Unicode-DFS-2016", "Unicode-3.0", "Unlicense", "CC0-1.0",
}
DESKTOP_TARGETS = {
    "x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin", "aarch64-apple-darwin",
}
API_TARGETS = DESKTOP_TARGETS | {
    "aarch64-apple-ios", "aarch64-apple-ios-sim", "x86_64-apple-ios",
    "aarch64-linux-android", "x86_64-linux-android",
}
EXCEPTION = {"allow": ["NCSA"], "crate": "libfuzzer-sys@0.4.13"}
FIXTURE = "tools/build-policy/rust-warning-fixture"
LOCKS = ("Cargo.lock", "fuzz/Cargo.lock", f"{FIXTURE}/Cargo.lock")
OTHER_MANIFESTS = {
    "pubspec.yaml", "pubspec.lock", "package.json", "package-lock.json",
    "yarn.lock", "pnpm-lock.yaml", "pom.xml", "build.gradle", "build.gradle.kts",
    "settings.gradle", "settings.gradle.kts", "gradle.lockfile", "Podfile",
    "Podfile.lock", "Package.swift", "Package.resolved", "requirements.txt",
    "pyproject.toml", "Pipfile", "Pipfile.lock", "poetry.lock", "uv.lock",
    "go.mod", "go.sum", "Gemfile", "Gemfile.lock", "composer.json", "composer.lock",
}
LICENSE_FILES = {
    "LICENSE", "crates/hidlins-genpw/LICENSES/EFF-wordlist-NOTICE.txt",
}


class PolicyError(RuntimeError):
    """The repository is outside its reviewed license boundary."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise PolicyError(message)


def load(root: Path, relative: str) -> dict:
    return tomllib.loads((root / relative).read_text())


def check_policy(root: Path) -> None:
    for relative, targets, exceptions in (
        ("deny.toml", API_TARGETS, []),
        ("fuzz/deny.toml", DESKTOP_TARGETS, [EXCEPTION]),
    ):
        data = load(root, relative)
        licenses = data["licenses"]
        allowed = licenses.get("allow", [])
        require(set(allowed) == ALLOWED and len(allowed) == len(ALLOWED),
                f"{relative}: license allow-list differs from reviewed policy")
        require(licenses.get("exceptions", []) == exceptions,
                f"{relative}: license exceptions differ from exact approved scope")
        require(not licenses.get("private", {}).get("ignore", False),
                f"{relative}: private packages must not escape license checks")
        require(licenses.get("confidence-threshold", 0) >= 0.93,
                f"{relative}: license confidence threshold was weakened")
        require(not licenses.get("clarify"), f"{relative}: unreviewed license clarification")
        graph = data["graph"]
        require(graph.get("all-features") is True and not graph.get("no-default-features", False),
                f"{relative}: all optional and default feature graphs must be audited")
        require(not graph.get("exclude"), f"{relative}: packages must not be excluded")
        observed = graph.get("targets", [])
        require(len(observed) == len(targets)
                and all(isinstance(item, dict) and set(item) == {"triple"} for item in observed)
                and {item["triple"] for item in observed} == targets,
                f"{relative}: supported target audit coverage changed")
    toolchain = load(root, "rust-toolchain.toml")["toolchain"]
    require(set(toolchain["targets"]) == API_TARGETS,
            "Rust toolchain and audited target matrix disagree")


def check_first_party(root: Path) -> set[str]:
    workspace = load(root, "Cargo.toml")["workspace"]
    require(workspace["package"].get("license") == "MIT", "workspace license must be MIT")
    require(not workspace["package"].get("license-file"), "unreviewed workspace license-file override")
    manifests = {"Cargo.toml", "fuzz/Cargo.toml", f"{FIXTURE}/Cargo.toml"}
    for member in workspace["members"]:
        path = Path(member)
        require(len(path.parts) == 2 and path.parts[0] == "crates"
                and not any(character in member for character in "*?[]"),
                f"unreviewed workspace member path: {member}")
        require(not (root / member).is_symlink(), f"indirect workspace member: {member}")
        manifests.add(f"{member}/Cargo.toml")
    for relative in sorted(manifests - {"Cargo.toml"}):
        package = load(root, relative)["package"]
        declared = package.get("license")
        require(declared == "MIT" or declared == {"workspace": True},
                f"{relative}: missing or non-MIT first-party license")
        require(not package.get("license-file"), f"{relative}: unreviewed license-file override")
    license_text = (root / "LICENSE").read_text()
    for clause in ("MIT License", "Permission is hereby granted, free of charge",
                   "THE SOFTWARE IS PROVIDED", "WITHOUT WARRANTY OF ANY KIND"):
        require(clause in license_text, "root MIT license text is missing or incomplete")
    return manifests


def repository_paths(root: Path) -> list[str]:
    staged = subprocess.check_output(["git", "ls-files", "--stage", "-z"], cwd=root)
    for entry in staged.split(b"\0"):
        require(not entry.startswith(b"160000 "), "embedded repository/gitlink has unaudited source")
    output = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=root,
    )
    return sorted({os.fsdecode(path) for path in output.split(b"\0") if path})


def check_inventory(root: Path, paths: list[str], manifests: set[str]) -> None:
    for relative in paths:
        path = root / relative
        if not path.exists():  # Working-tree deletions are the intended result.
            continue
        if relative.startswith("vendor/"):
            continue  # Every byte is audited by check_vendor, including nested metadata.
        if path.is_symlink():
            require(relative == "AGENTS.md" and path.resolve() == (root / "CLAUDE.md").resolve(),
                    f"unreviewed source symlink: {relative}")
        require(path.name != ".gitmodules", "embedded repository manifest is forbidden")
        require(path.name not in OTHER_MANIFESTS and path.suffix != ".podspec",
                f"dependency manifest outside the audited Rust graph: {relative}")
        if path.name == "Cargo.toml":
            require(relative in manifests, f"unregistered Rust package: {relative}")
        if path.name == "Cargo.lock":
            require(relative in LOCKS, f"unregistered Rust lockfile: {relative}")
        if path.name.upper().startswith(("LICENSE", "COPYING", "NOTICE")):
            require(relative in LICENSE_FILES, f"unreviewed license/notice file: {relative}")


def files_under(directory: Path) -> dict[str, Path]:
    result = {}
    def failed(error):
        raise error
    for parent, directories, names in os.walk(directory, onerror=failed):
        for name in directories:
            require(not (Path(parent) / name).is_symlink(), f"indirect vendor directory: {parent}/{name}")
        for name in names:
            path = Path(parent) / name
            require(path.is_file() and not path.is_symlink(), f"non-ordinary vendor file: {path}")
            result[path.relative_to(directory).as_posix()] = path
    return result


def check_vendor(root: Path) -> int:
    expected = {}
    for relative in LOCKS:
        for package in load(root, relative).get("package", []):
            source = package.get("source")
            if source is None:
                continue
            require(source == "registry+https://github.com/rust-lang/crates.io-index",
                    f"{relative}: unreviewed dependency source")
            identity = (package["name"], package["version"])
            checksum = package.get("checksum")
            require(isinstance(checksum, str) and len(checksum) == 64
                    and all(character in "0123456789abcdef" for character in checksum),
                    f"{relative}: missing registry checksum for {identity}")
            require(identity not in expected or expected[identity] == checksum,
                    f"lockfiles disagree about registry provenance: {identity}")
            expected[identity] = checksum
    actual = set()
    for directory in sorted((root / "vendor").iterdir()):
        require(directory.is_dir() and not directory.is_symlink(), f"non-crate vendor entry: {directory}")
        package = tomllib.loads((directory / "Cargo.toml").read_text())["package"]
        identity = (package["name"], package["version"])
        require(identity in expected, f"orphan vendor package: {identity}")
        require(identity not in actual, f"duplicate vendor package: {identity}")
        actual.add(identity)
        declared = package.get("license")
        require(isinstance(declared, str) and bool(declared.strip()), f"missing vendor license: {identity}")
        checksums = json.loads((directory / ".cargo-checksum.json").read_text())
        require(checksums["package"] == expected[identity], f"vendor registry provenance differs: {identity}")
        files = files_under(directory)
        del files[".cargo-checksum.json"]
        require(set(files) == set(checksums["files"]), f"vendor file inventory differs: {identity}")
        for relative, path in files.items():
            with path.open("rb") as stream:
                digest = hashlib.file_digest(stream, "sha256").hexdigest()
            require(digest == checksums["files"][relative], f"vendor bytes differ: {identity}/{relative}")
    require(actual == set(expected), f"locked vendor sources missing: {sorted(set(expected) - actual)}")
    return len(actual)


def check(root: Path, paths: list[str] | None = None) -> int:
    check_policy(root)
    manifests = check_first_party(root)
    check_inventory(root, repository_paths(root) if paths is None else paths, manifests)
    return check_vendor(root)


if __name__ == "__main__":
    try:
        count = check(ROOT)
    except (PolicyError, OSError, ValueError, KeyError, TypeError, subprocess.CalledProcessError) as error:
        raise SystemExit(f"ERROR: repository license inventory: {error}") from error
    print(f"OK: first-party MIT declarations, strict policies, dependency inventory and {count} locked vendor trees")
