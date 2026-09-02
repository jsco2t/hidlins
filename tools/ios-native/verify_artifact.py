#!/usr/bin/env python3
"""Fail closed when an iOS artifact omits Hidlins resources or the Rust bridge."""

from __future__ import annotations

import hashlib
import plistlib
import subprocess
import sys
from pathlib import Path


def fail(message: str) -> None:
    raise SystemExit(f"iOS artifact verification failed: {message}")


def main() -> None:
    if len(sys.argv) != 2:
        fail("usage: verify_artifact.py <Runner.app>")

    app = Path(sys.argv[1]).resolve()
    info_path = app / "Info.plist"
    assets_path = app / "Assets.car"
    privacy_path = app / "PrivacyInfo.xcprivacy"
    bridge_path = app / "Frameworks" / "rust_lib_app.framework" / "rust_lib_app"
    for required in (info_path, assets_path, privacy_path, bridge_path):
        if not required.is_file() or required.stat().st_size == 0:
            fail(f"missing or empty resource: {required}")

    with info_path.open("rb") as stream:
        info = plistlib.load(stream)
    if info.get("CFBundleIdentifier") != "app.hidlins.ios":
        fail("unexpected bundle identifier")
    if info.get("CFBundleDisplayName") != "Hidlins":
        fail("unexpected display name")
    minimum = tuple(int(part) for part in str(info.get("MinimumOSVersion", "0")).split("."))
    if minimum < (16, 0):
        fail("deployment target is below iOS 16")
    if info.get("UILaunchStoryboardName") != "LaunchScreen":
        fail("Hidlins launch storyboard is not installed")
    primary_icon = info.get("CFBundleIcons", {}).get("CFBundlePrimaryIcon", {})
    if primary_icon.get("CFBundleIconName") != "AppIcon":
        fail("compiled Hidlins AppIcon catalog is not registered")

    with privacy_path.open("rb") as stream:
        privacy = plistlib.load(stream)
    if privacy.get("NSPrivacyTracking") is not False:
        fail("privacy manifest must declare tracking disabled")
    if privacy.get("NSPrivacyTrackingDomains") != []:
        fail("privacy manifest must not declare tracking domains")
    if privacy.get("NSPrivacyCollectedDataTypes") != []:
        fail("privacy manifest must not declare collected data")
    reasons = {
        item.get("NSPrivacyAccessedAPIType"): set(
            item.get("NSPrivacyAccessedAPITypeReasons", [])
        )
        for item in privacy.get("NSPrivacyAccessedAPITypes", [])
    }
    expected_reasons = {
        "NSPrivacyAccessedAPICategoryFileTimestamp": {"C617.1", "3B52.1"},
        "NSPrivacyAccessedAPICategorySystemBootTime": {"35F9.1"},
        "NSPrivacyAccessedAPICategoryUserDefaults": {"CA92.1"},
    }
    if reasons != expected_reasons:
        fail("privacy required-reason API declarations differ from policy")

    forbidden = (
        "sentry",
        "firebase",
        "crashlytics",
        "appcenter",
        "datadog",
        "posthog",
        "amplitude",
        "segment",
        "sparkle",
    )
    for packaged in app.rglob("*"):
        name = packaged.name.lower()
        framework_resource = name.endswith(
            (".framework", ".xcframework", ".bundle", ".dylib")
        )
        if framework_resource and any(name.startswith(marker) for marker in forbidden):
            fail(f"telemetry/crash/update framework or resource present: {packaged.name}")
        if name == "googleservice-info.plist":
            fail("Firebase configuration resource present")

    symbols = subprocess.run(
        ["nm", "-gj", str(bridge_path)],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.splitlines()
    if "_hidlins_ios_consume_clipboard" not in symbols:
        fail("real Rust iOS clipboard bridge export is absent")

    digest = hashlib.sha256(bridge_path.read_bytes()).hexdigest()
    platform = info.get("DTPlatformName", "unknown")
    sdk = info.get("DTSDKName", "unknown")
    print(
        f"iOS artifact OK: {app} platform={platform} sdk={sdk} "
        f"minimum={info['MinimumOSVersion']} bridge_sha256={digest}"
    )


if __name__ == "__main__":
    main()
