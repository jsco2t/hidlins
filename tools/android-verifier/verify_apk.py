#!/usr/bin/env python3
"""Verify the release/R8 verifier APK contains exactly the checked native ABI set."""

from __future__ import annotations

import hashlib
import json
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
APK = ROOT / "tools/android-verifier/app/build/outputs/apk/release/app-release.apk"
MANIFEST = ROOT / "app/android/native-artifacts/manifest.json"


def die(message: str) -> None:
    raise SystemExit(f"error: {message}")


manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
expected = {f"lib/{item['abi']}/{manifest['library']}": item for item in manifest["artifacts"]}
with zipfile.ZipFile(APK) as archive:
    actual = sorted(
        name for name in archive.namelist() if name.startswith("lib/") and name.endswith(".so")
    )
    if actual != sorted(expected):
        die(f"APK native library set is {actual}, expected {sorted(expected)}")
    for name, record in expected.items():
        value = hashlib.sha256(archive.read(name)).hexdigest()
        if value != record["sha256"]:
            die(f"APK library hash differs from staged contract: {name}")
    dex = b"".join(archive.read(name) for name in archive.namelist() if name.endswith(".dex"))
    for class_name in (
        b"org/rustls/platformverifier/CertificateVerifier",
        b"app/hidlins/HidlinsNative",
    ):
        if class_name not in dex:
            die(f"R8 removed or renamed required class {class_name.decode()}")
print("  OK: release/R8 APK preserves verifier classes and exact staged ABI/hash set")
