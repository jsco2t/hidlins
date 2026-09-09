#!/usr/bin/env python3
"""Enforce that the Android CLI authority remains test-only and unpackaged."""

from __future__ import annotations

import pathlib
import subprocess
import sys
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
SHIPPING_APKS = (
    ROOT / "app/build/app/outputs/flutter-apk/app-debug.apk",
    ROOT / "app/build/app/outputs/flutter-apk/app-release.apk",
)
TEST_APK = ROOT / "app/build/app/outputs/apk/androidTest/debug/app-debug-androidTest.apk"
REGISTRAR_MARKER = b"ScenarioNsdRegistrarActivity"


def cargo_tree(*args: str) -> str:
    result = subprocess.run(
        ["cargo", "tree", "--offline", "--locked", *args],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit(result.stderr)
    return result.stdout


def require_absent(value: str, markers: tuple[str, ...], context: str) -> None:
    found = [marker for marker in markers if marker in value]
    if found:
        raise SystemExit(f"{context} contains test-authority dependency: {', '.join(found)}")


def apk_contains(path: pathlib.Path, marker: bytes) -> bool:
    with zipfile.ZipFile(path) as archive:
        return any(marker in archive.read(name) for name in archive.namelist())


def main() -> None:
    shipping = cargo_tree(
        "-p", "hidlins-api", "--target", "aarch64-linux-android",
        "--no-default-features", "--edges", "normal"
    )
    require_absent(shipping, ("hidlins-cli", "mdns-sd", "if-addrs", "arboard"), "shipping Android graph")

    scenario = cargo_tree(
        "-p", "hidlins-cli", "--target", "aarch64-linux-android",
        "--no-default-features", "--features", "android-scenario-authority",
        "--edges", "normal",
    )
    for required in ("hidlins-cli", "mdns-sd", "if-addrs"):
        if required not in scenario:
            raise SystemExit(f"scenario authority graph is missing {required}")
    require_absent(scenario, ("arboard",), "scenario authority graph")

    shipping_inputs = (
        ROOT / "tools/android-native/build.sh",
        ROOT / "tools/android-native/build_app.sh",
        ROOT / "tools/android-native/artifact-contract.json",
        ROOT / "app/android/app/build.gradle.kts",
    )
    for path in shipping_inputs[1:]:
        require_absent(path.read_text(encoding="utf-8"), ("android-scenario-authority",), str(path))

    for apk in SHIPPING_APKS:
        if not apk.is_file():
            raise SystemExit(f"shipping APK is unavailable: {apk}")
        with zipfile.ZipFile(apk) as archive:
            forbidden = [
                name for name in archive.namelist()
                if pathlib.PurePosixPath(name).name == "hidlins"
            ]
        if forbidden:
            raise SystemExit(f"shipping APK contains the scenario CLI binary: {apk}")
        if apk_contains(apk, REGISTRAR_MARKER):
            raise SystemExit(f"shipping APK contains the test-only NSD registrar: {apk}")

    if not TEST_APK.is_file():
        raise SystemExit(f"instrumentation APK is unavailable: {TEST_APK}")
    if not apk_contains(TEST_APK, REGISTRAR_MARKER):
        raise SystemExit("instrumentation APK is missing the test-only NSD registrar")
    print(
        "  OK: Android scenario authority and NSD registrar are confined to test artifacts"
    )


if __name__ == "__main__":
    main()
