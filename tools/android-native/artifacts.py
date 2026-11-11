#!/usr/bin/env python3
"""Stage and verify Hidlins Android Rust artifacts without Gradle build logic."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import struct
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONTRACT_PATH = ROOT / "tools/android-native/artifact-contract.json"
STAGE_ROOT = ROOT / "app/android/native-artifacts"
MANIFEST_PATH = STAGE_ROOT / "manifest.json"
FRB_API_MANIFEST = ROOT / "tools/dev/frb-api-manifest.txt"


def fail(message: str) -> None:
    raise SystemExit(f"error: {message}")


def digest(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


def load_json(path: Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read {path.relative_to(ROOT)}: {error}")


def contract() -> dict:
    return load_json(CONTRACT_PATH)


def source_digest() -> str:
    roots = [
        ROOT / "Cargo.toml",
        ROOT / "Cargo.lock",
        ROOT / "rust-toolchain.toml",
        ROOT / "crates/hidlins-api",
        ROOT / "crates/hidlins-core",
        ROOT / "crates/hidlins-genpw",
        ROOT / "crates/hidlins-security",
        ROOT / "crates/hidlins-sync",
        FRB_API_MANIFEST,
    ]
    files: list[Path] = []
    for item in roots:
        if item.is_file():
            files.append(item)
        else:
            files.extend(
                path
                for path in item.rglob("*")
                if path.is_file() and path.suffix in {".rs", ".toml"}
            )
    value = hashlib.sha256()
    for path in sorted(files):
        value.update(str(path.relative_to(ROOT)).encode())
        value.update(b"\0")
        value.update(path.read_bytes())
        value.update(b"\0")
    return value.hexdigest()


def elf_metadata(path: Path) -> tuple[int, list[int]]:
    data = path.read_bytes()
    if len(data) < 64 or data[:4] != b"\x7fELF":
        fail(f"{path} is not an ELF file")
    if data[4] != 2 or data[5] != 1:
        fail(f"{path} must be a little-endian ELF64 library")
    machine = struct.unpack_from("<H", data, 18)[0]
    program_offset = struct.unpack_from("<Q", data, 32)[0]
    entry_size = struct.unpack_from("<H", data, 54)[0]
    entry_count = struct.unpack_from("<H", data, 56)[0]
    alignments: list[int] = []
    for index in range(entry_count):
        offset = program_offset + index * entry_size
        if offset + 56 > len(data):
            fail(f"{path} has a truncated program-header table")
        segment_type = struct.unpack_from("<I", data, offset)[0]
        if segment_type == 1:
            alignments.append(struct.unpack_from("<Q", data, offset + 48)[0])
    if not alignments:
        fail(f"{path} has no loadable segments")
    return machine, alignments


def verify_build_contract() -> None:
    app_gradle = (ROOT / "app/android/app/build.gradle.kts").read_text(encoding="utf-8")
    if f'ndkVersion = "{contract()["ndk_version"]}"' not in app_gradle:
        fail("application NDK version differs from the native artifact contract")
    plugin_gradle = (ROOT / "app/rust_builder/android/build.gradle").read_text(
        encoding="utf-8"
    )
    lowered_plugin = plugin_gradle.lower()
    if "apply from:" in lowered_plugin or "cargokit {" in lowered_plugin:
        fail("production Android plugin still references legacy Cargokit")

def validate_artifact(path: Path, item: dict, cfg: dict) -> dict:
    machine, alignments = elf_metadata(path)
    if machine != item["elf_machine"]:
        fail(f"{path} has ELF machine {machine}, expected {item['elf_machine']}")
    minimum = cfg["minimum_load_alignment"]
    if any(alignment < minimum for alignment in alignments):
        fail(f"{path} has LOAD alignment below {minimum}: {alignments}")
    return {
        "target": item["target"],
        "abi": item["abi"],
        "profile": cfg["profile"],
        "file": f"jniLibs/{item['abi']}/{cfg['library']}",
        "sha256": digest(path),
        "elf_machine": machine,
        "load_alignments": alignments,
    }


def stage() -> None:
    cfg = contract()
    verify_build_contract()
    if STAGE_ROOT.exists():
        shutil.rmtree(STAGE_ROOT)
    records = []
    for item in cfg["artifacts"]:
        source = ROOT / "target" / item["target"] / cfg["profile"] / cfg["library"]
        if not source.is_file():
            fail(f"missing release artifact {source.relative_to(ROOT)}")
        destination = STAGE_ROOT / "jniLibs" / item["abi"] / cfg["library"]
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
        records.append(validate_artifact(destination, item, cfg))
    manifest = {
        "schema": cfg["schema"],
        "contract_sha256": digest(CONTRACT_PATH),
        "frb_api_sha256": digest(FRB_API_MANIFEST),
        "source_sha256": source_digest(),
        "library": cfg["library"],
        "profile": cfg["profile"],
        "android_api_level": cfg["android_api_level"],
        "ndk_version": cfg["ndk_version"],
        "artifacts": records,
    }
    MANIFEST_PATH.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    check()


def check(
    stage_root: Path = STAGE_ROOT,
    manifest_path: Path = MANIFEST_PATH,
    *,
    announce: bool = True,
) -> None:
    cfg = contract()
    verify_build_contract()
    manifest = load_json(manifest_path)
    expected = {
        "schema": cfg["schema"],
        "contract_sha256": digest(CONTRACT_PATH),
        "frb_api_sha256": digest(FRB_API_MANIFEST),
        "source_sha256": source_digest(),
        "library": cfg["library"],
        "profile": cfg["profile"],
        "android_api_level": cfg["android_api_level"],
        "ndk_version": cfg["ndk_version"],
    }
    for key, value in expected.items():
        if manifest.get(key) != value:
            fail(f"staged manifest {key} is stale or invalid")
    expected_abis = [item["abi"] for item in cfg["artifacts"]]
    records = manifest.get("artifacts")
    if not isinstance(records, list) or [item.get("abi") for item in records] != expected_abis:
        fail(f"staged ABI set/order must be exactly {expected_abis}")
    staged_dirs = sorted(path.name for path in (stage_root / "jniLibs").iterdir() if path.is_dir())
    if staged_dirs != sorted(expected_abis):
        fail(f"staged ABI directories must be exactly {sorted(expected_abis)}")
    by_abi = {item["abi"]: item for item in cfg["artifacts"]}
    for record in records:
        path = stage_root / record["file"]
        actual = validate_artifact(path, by_abi[record["abi"]], cfg)
        if actual != record:
            fail(f"staged metadata/hash mismatch for {record['abi']}")
    if announce:
        print(
            "  OK: Android native artifacts match API, profile, source, ABI, hash, "
            "and 16 KiB contract"
        )


def write_test_elf(path: Path, machine: int, alignment: int) -> None:
    """Write the smallest ELF64 shape needed to exercise the local parser."""
    header = bytearray(64)
    header[:6] = b"\x7fELF\x02\x01"
    struct.pack_into("<H", header, 18, machine)
    struct.pack_into("<Q", header, 32, 64)
    struct.pack_into("<H", header, 54, 56)
    struct.pack_into("<H", header, 56, 1)
    program_header = bytearray(56)
    struct.pack_into("<I", program_header, 0, 1)
    struct.pack_into("<Q", program_header, 48, alignment)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(header + program_header)


def expect_check_failure(stage_root: Path, manifest_path: Path, label: str) -> None:
    try:
        check(stage_root, manifest_path, announce=False)
    except SystemExit:
        return
    fail(f"{label} negative control was accepted")


def self_test() -> None:
    with tempfile.TemporaryDirectory(prefix="hidlins-android-artifacts-") as scratch:
        cfg = contract()
        stage_root = Path(scratch) / "native-artifacts"
        manifest_path = stage_root / "manifest.json"
        records = []
        for item in cfg["artifacts"]:
            path = stage_root / "jniLibs" / item["abi"] / cfg["library"]
            write_test_elf(path, item["elf_machine"], cfg["minimum_load_alignment"])
            records.append(validate_artifact(path, item, cfg))
        baseline = {
            "schema": cfg["schema"],
            "contract_sha256": digest(CONTRACT_PATH),
            "frb_api_sha256": digest(FRB_API_MANIFEST),
            "source_sha256": source_digest(),
            "library": cfg["library"],
            "profile": cfg["profile"],
            "android_api_level": cfg["android_api_level"],
            "ndk_version": cfg["ndk_version"],
            "artifacts": records,
        }

        def write_manifest(value: dict) -> None:
            manifest_path.write_text(json.dumps(value), encoding="utf-8")

        write_manifest(baseline)
        check(stage_root, manifest_path, announce=False)

        wrong_profile = dict(baseline)
        wrong_profile["profile"] = "debug"
        write_manifest(wrong_profile)
        expect_check_failure(stage_root, manifest_path, "wrong-profile")

        stale = dict(baseline)
        stale["source_sha256"] = "0" * 64
        write_manifest(stale)
        expect_check_failure(stage_root, manifest_path, "stale-source")

        wrong_api = dict(baseline)
        wrong_api["frb_api_sha256"] = "0" * 64
        write_manifest(wrong_api)
        expect_check_failure(stage_root, manifest_path, "wrong-API")

        write_manifest(baseline)
        unexpected = stage_root / "jniLibs" / "armeabi-v7a"
        unexpected.mkdir()
        expect_check_failure(stage_root, manifest_path, "wrong-ABI")
        unexpected.rmdir()

        changed = stage_root / records[0]["file"]
        changed.write_bytes(changed.read_bytes() + b"changed")
        expect_check_failure(stage_root, manifest_path, "wrong-hash")

    print(
        "  OK: stale/wrong-API/wrong-profile/wrong-ABI/wrong-hash guards and "
        "native artifact hashes pass"
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("self-test", "stage", "check"))
    args = parser.parse_args()
    if args.command == "self-test":
        self_test()
    elif args.command == "stage":
        stage()
    else:
        check()


if __name__ == "__main__":
    main()
