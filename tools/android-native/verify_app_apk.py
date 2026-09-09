#!/usr/bin/env python3
"""Inspect Hidlins Android debug/release APKs without trusting Gradle output."""

from __future__ import annotations

import hashlib
import json
import os
import re
import subprocess
import tempfile
import zipfile
from pathlib import Path

from artifacts import elf_metadata

ROOT = Path(__file__).resolve().parents[2]
DEBUG_APK = ROOT / "app/build/app/outputs/apk/debug/app-debug.apk"
RELEASE_APK = ROOT / "app/build/app/outputs/apk/release/app-release-unsigned.apk"
MANIFEST = ROOT / "app/android/native-artifacts/manifest.json"
EXPECTED_ABIS = {"arm64-v8a", "x86_64"}
REQUIRED_CLASSES = (
    b"io/flutter/plugins/GeneratedPluginRegistrant",
    b"app/hidlins/HidlinsNative",
    b"consumeClipboard",
    b"app.hidlins/clipboard",
    b"hidlins.snapshot-cover",
)


def fail(message: str) -> None:
    raise SystemExit(f"error: {message}")


def sdk_root() -> Path:
    configured = os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT")
    if configured:
        return Path(configured)
    properties = ROOT / "app/android/local.properties"
    if properties.is_file():
        match = re.search(r"^sdk\.dir=(.+)$", properties.read_text(encoding="utf-8"), re.MULTILINE)
        if match:
            return Path(match.group(1).replace(r"\:", ":").replace(r"\\", "\\"))
    home = Path.home()
    fallback = home / ("Library/Android/sdk" if os.uname().sysname == "Darwin" else "Android/Sdk")
    return fallback


def version_key(path: Path) -> tuple[int, ...]:
    values = re.findall(r"\d+", path.name)
    return tuple(int(value) for value in values)


def build_tool(name: str) -> Path:
    candidates = [
        path / name
        for path in (sdk_root() / "build-tools").iterdir()
        if path.is_dir() and (path / name).is_file()
    ]
    if not candidates:
        fail(f"Android SDK build tool is missing: {name}")
    return max(candidates, key=lambda path: version_key(path.parent))


def run(tool: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    environment = os.environ.copy()
    android_studio_java = Path("/Applications/Android Studio.app/Contents/jbr/Contents/Home")
    if "JAVA_HOME" not in environment and android_studio_java.is_dir():
        environment["JAVA_HOME"] = str(android_studio_java)
    result = subprocess.run(
        [str(tool), *args],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        env=environment,
        check=False,
    )
    if check and result.returncode != 0:
        fail(f"{tool.name} failed for {' '.join(args)}:\n{result.stdout}")
    return result


def archive_facts(apk: Path) -> tuple[set[str], bytes, list[str]]:
    if not apk.is_file():
        fail(f"missing APK: {apk.relative_to(ROOT)}")
    with zipfile.ZipFile(apk) as archive:
        names = archive.namelist()
        libraries = [name for name in names if name.startswith("lib/") and name.endswith(".so")]
        abis = {name.split("/", 2)[1] for name in libraries}
        dex = b"".join(archive.read(name) for name in names if name.endswith(".dex"))
    return abis, dex, libraries


def validate_contents(abis: set[str], dex: bytes, label: str) -> None:
    if abis != EXPECTED_ABIS:
        fail(f"{label} APK ABI set is {sorted(abis)}, expected {sorted(EXPECTED_ABIS)}")
    for class_name in REQUIRED_CLASSES:
        if class_name not in dex:
            fail(f"{label} APK is missing required R8/JNI class {class_name.decode()}")


def verify_native_hashes(apk: Path, native_manifest: dict) -> None:
    expected = {
        f"lib/{item['abi']}/{native_manifest['library']}": item["sha256"]
        for item in native_manifest["artifacts"]
    }
    with zipfile.ZipFile(apk) as archive:
        for name, expected_hash in expected.items():
            if name not in archive.namelist():
                fail(f"{apk.name} does not contain staged Rust bridge {name}")
            actual_hash = hashlib.sha256(archive.read(name)).hexdigest()
            if actual_hash != expected_hash:
                fail(f"{apk.name} Rust bridge hash differs from staged contract: {name}")


def verify_release_elfs(apk: Path, libraries: list[str], minimum: int) -> None:
    with tempfile.TemporaryDirectory(prefix="hidlins-release-apk-") as scratch:
        scratch_root = Path(scratch)
        with zipfile.ZipFile(apk) as archive:
            for name in libraries:
                destination = scratch_root / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(archive.read(name))
                _, alignments = elf_metadata(destination)
                if any(alignment < minimum for alignment in alignments):
                    fail(f"release native library {name} has LOAD alignment below {minimum}")


def verify_android_metadata(apk: Path, *, release: bool) -> None:
    aapt = build_tool("aapt")
    badging = run(aapt, "dump", "badging", str(apk)).stdout
    for marker in ("package: name='app.hidlins'", "sdkVersion:'29'", "application-label:'Hidlins'"):
        if marker not in badging:
            fail(f"{apk.name} badging is missing {marker}")
    if "application-icon-" not in badging:
        fail(f"{apk.name} has no installed launcher icon")
    manifest = run(aapt, "dump", "xmltree", str(apk), "AndroidManifest.xml").stdout
    permissions = run(aapt, "dump", "permissions", str(apk)).stdout
    if "uses-permission: name='android.permission.INTERNET'" not in permissions:
        fail(f"{apk.name} cannot reach the user-configured sync service")

    apksigner = build_tool("apksigner")
    signature = run(apksigner, "verify", "--verbose", str(apk), check=False)
    if release:
        if signature.returncode == 0 or "DOES NOT VERIFY" not in signature.stdout:
            fail("release APK must remain deliberately unsigned")
    elif signature.returncode != 0:
        fail(f"debug APK signature does not verify:\n{signature.stdout}")


def self_test() -> None:
    valid_dex = b"\0".join(REQUIRED_CLASSES)
    validate_contents(set(EXPECTED_ABIS), valid_dex, "fixture")
    cases = (
        ({"arm64-v8a"}, valid_dex, "missing ABI"),
        (EXPECTED_ABIS | {"armeabi-v7a"}, valid_dex, "extra ABI"),
        (set(EXPECTED_ABIS), b"missing", "missing JNI/R8 class"),
    )
    for abis, dex, label in cases:
        try:
            validate_contents(abis, dex, label)
        except SystemExit:
            continue
        fail(f"{label} negative control was accepted")
    print("  OK: Android APK inspector rejects missing/extra ABIs and removed JNI/R8 classes")


def main() -> None:
    self_test()
    native_manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    release_libraries: list[str] = []
    for apk, release in ((DEBUG_APK, False), (RELEASE_APK, True)):
        abis, dex, libraries = archive_facts(apk)
        validate_contents(abis, dex, "release" if release else "debug")
        verify_native_hashes(apk, native_manifest)
        verify_android_metadata(apk, release=release)
        if release:
            release_libraries = libraries
    verify_release_elfs(
        RELEASE_APK,
        release_libraries,
        json.loads((ROOT / "tools/android-native/artifact-contract.json").read_text())["minimum_load_alignment"],
    )
    run(build_tool("zipalign"), "-c", "-P", "16", "4", str(RELEASE_APK))
    print(
        "  OK: debug-signed and release-unsigned APKs preserve the real Rust bridge, "
        "R8/JNI classes, API 29 floor, exact ABIs, sync permission, launcher metadata, "
        "and 16 KiB alignment"
    )


if __name__ == "__main__":
    main()
