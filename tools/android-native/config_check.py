#!/usr/bin/env python3
"""Verify the exact Flutter 3.47.2 Android build-model contract."""

from __future__ import annotations

import argparse
import json
import re
import tempfile
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def fail(message: str) -> None:
    raise SystemExit(f"error: {message}")


def uncomment_kotlin(value: str) -> str:
    value = re.sub(r"/\*.*?\*/", "", value, flags=re.DOTALL)
    return re.sub(r"//.*$", "", value, flags=re.MULTILINE)


def gradle_property(value: str, key: str) -> str:
    matches = re.findall(
        rf"^\s*{re.escape(key)}\s*[=:]\s*([^#\s]+)\s*(?:#.*)?$",
        value,
        flags=re.MULTILINE,
    )
    if len(matches) != 1:
        fail(f"{key} must appear exactly once")
    return matches[0]


@dataclass(frozen=True)
class AndroidConfig:
    flutter_version: str
    properties: str
    settings: str
    app_gradle: str
    plugin_gradle: str
    root_gradle: str
    wrapper: str
    lockfile: str
    verification_metadata: str
    workflow_text: str


def require(pattern: str, value: str, message: str) -> None:
    if re.search(pattern, value, flags=re.MULTILINE | re.DOTALL) is None:
        fail(message)


def validate(config: AndroidConfig) -> None:
    if config.flutter_version.strip() != "3.47.2":
        fail("Android build contract requires Flutter 3.47.2")
    if gradle_property(config.properties, "android.builtInKotlin") != "true":
        fail("android.builtInKotlin must be true")
    if gradle_property(config.properties, "android.newDsl") != "false":
        fail("Flutter 3.47.2 requires android.newDsl=false")

    settings = uncomment_kotlin(config.settings)
    require(
        r'id\("com\.android\.application"\)\s+version\s+"9\.1\.0"\s+apply\s+false',
        settings,
        "settings must pin AGP 9.1.0 with apply false",
    )
    require(
        r'id\("org\.jetbrains\.kotlin\.android"\)\s+version\s+"2\.4\.0"\s+apply\s+false',
        settings,
        "settings must pin Kotlin 2.4.0 with apply false",
    )
    require(
        r"gradle-9\.3\.1-(?:all|bin)\.zip",
        config.wrapper,
        "Gradle wrapper must pin 9.3.1",
    )

    app_gradle = uncomment_kotlin(config.app_gradle)
    plugin_gradle = uncomment_kotlin(config.plugin_gradle)
    applied_kgp_patterns = (
        r'id\(["\']org\.jetbrains\.kotlin\.android["\']\)',
        r'id\(["\']kotlin-android["\']\)',
        r"apply\s+plugin\s*:\s*[\"'](?:org\.jetbrains\.kotlin\.android|kotlin-android)[\"']",
    )
    for pattern in applied_kgp_patterns:
        if re.search(pattern, app_gradle) or re.search(pattern, plugin_gradle):
            fail("a Hidlins Android module applies the Kotlin Android plugin")

    require(
        r"kotlin\s*\{.*?compilerOptions\s*\{.*?allWarningsAsErrors\.set\(true\)",
        app_gradle,
        "first-party Kotlin warnings must be fatal",
    )
    require(r"minSdk\s*=\s*29", app_gradle, "application minSdk must be 29")
    require(
        r'abiFilters\s*\+=\s*listOf\("arm64-v8a",\s*"x86_64"\)',
        app_gradle,
        "application ABI filters must be exactly arm64-v8a and x86_64",
    )
    require(
        r"isMinifyEnabled\s*=\s*true",
        app_gradle,
        "release R8 minification must be enabled",
    )
    require(
        r'implementation\("rustls:rustls-platform-verifier:0\.1\.1"\)',
        app_gradle,
        "production app must package the pinned rustls platform verifier",
    )
    require(
        r"dependencyLocking\s*\{\s*lockAllConfigurations\(\)",
        uncomment_kotlin(config.root_gradle),
        "all Android projects must lock every dependency configuration",
    )
    for coordinate in (
        "rustls:rustls-platform-verifier:0.1.1=",
        "org.jetbrains.kotlin:kotlin-stdlib:2.4.0=",
    ):
        if coordinate not in config.lockfile:
            fail(f"Android dependency lockfile does not pin {coordinate[:-1]}")
    for unsupported_engine in (r"io\.flutter:armeabi_v7a_", r"io\.flutter:x86_(?!64)"):
        if re.search(unsupported_engine, config.lockfile):
            fail(f"Android dependency lockfile includes unsupported engine {unsupported_engine}")
    verifier = json.loads(
        (ROOT / "tools/android-native/artifact-contract.json").read_text(encoding="utf-8")
    )["verifier"]
    metadata = ET.fromstring(config.verification_metadata)
    hashes = {
        node.attrib["value"]
        for node in metadata.iter()
        if node.tag.rsplit("}", 1)[-1] == "sha256" and "value" in node.attrib
    }
    if verifier["aar_sha256"] not in hashes or verifier["pom_sha256"] not in hashes:
        fail("Android verification metadata does not pin the verifier AAR and POM")
    for host_artifact in (
        "aapt2-9.1.0-14792394-linux.jar",
        "aapt2-9.1.0-14792394-osx.jar",
    ):
        if host_artifact not in config.verification_metadata:
            fail(f"Android verification metadata does not pin {host_artifact}")
    validation_bypass = "--android-" + "skip-build-dependency-validation"
    if validation_bypass in config.workflow_text:
        fail("Android dependency validation bypass is forbidden")


def load_production() -> AndroidConfig:
    workflow_paths = [ROOT / "Makefile", ROOT / ".github/workflows", ROOT / "tools"]
    workflow_files: list[Path] = []
    for path in workflow_paths:
        if path.is_file():
            workflow_files.append(path)
        elif path.is_dir():
            workflow_files.extend(
                item
                for item in path.rglob("*")
                if item.is_file()
                and "__pycache__" not in item.parts
                and "build" not in item.parts
                and item.suffix in {"", ".py", ".sh", ".yml", ".yaml"}
            )
    return AndroidConfig(
        flutter_version=(ROOT / ".flutter-version").read_text(encoding="utf-8"),
        properties=(ROOT / "app/android/gradle.properties").read_text(encoding="utf-8"),
        settings=(ROOT / "app/android/settings.gradle.kts").read_text(encoding="utf-8"),
        app_gradle=(ROOT / "app/android/app/build.gradle.kts").read_text(encoding="utf-8"),
        plugin_gradle=(ROOT / "app/rust_builder/android/build.gradle").read_text(
            encoding="utf-8"
        ),
        root_gradle=(ROOT / "app/android/build.gradle.kts").read_text(encoding="utf-8"),
        wrapper=(ROOT / "app/android/gradle/wrapper/gradle-wrapper.properties").read_text(
            encoding="utf-8"
        ),
        lockfile=(ROOT / "app/android/app/gradle.lockfile").read_text(encoding="utf-8"),
        verification_metadata=(
            ROOT / "app/android/gradle/verification-metadata.xml"
        ).read_text(encoding="utf-8"),
        workflow_text="\n".join(path.read_text(encoding="utf-8", errors="ignore") for path in workflow_files),
    )


def valid_fixture() -> AndroidConfig:
    return AndroidConfig(
        flutter_version="3.47.2\n",
        properties="android.builtInKotlin=true\nandroid.newDsl=false\n",
        settings='''
plugins {
    id("com.android.application") version "9.1.0" apply false
    id("org.jetbrains.kotlin.android") version "2.4.0" apply false
}
''',
        app_gradle='''
plugins { id("com.android.application") }
android {
  defaultConfig { minSdk = 29; ndk { abiFilters += listOf("arm64-v8a", "x86_64") } }
  buildTypes { release { isMinifyEnabled = true } }
}
kotlin { compilerOptions { allWarningsAsErrors.set(true) } }
dependencies { implementation("rustls:rustls-platform-verifier:0.1.1") }
''',
        plugin_gradle="plugins { id 'com.android.library' }\n",
        root_gradle="allprojects { dependencyLocking { lockAllConfigurations() } }\n",
        wrapper="distributionUrl=https\\://services.gradle.org/distributions/gradle-9.3.1-all.zip\n",
        lockfile=(
            "rustls:rustls-platform-verifier:0.1.1=releaseRuntimeClasspath\n"
            "org.jetbrains.kotlin:kotlin-stdlib:2.4.0=releaseRuntimeClasspath\n"
        ),
        verification_metadata=(
            "<verification-metadata><components><component>"
            '<artifact name="aapt2-9.1.0-14792394-linux.jar"><sha256 value="linux"/></artifact>'
            '<artifact name="aapt2-9.1.0-14792394-osx.jar"><sha256 value="osx"/></artifact>'
            '<artifact><sha256 value="667292cadd8fa589229dd0f716541236a761f29b774930868d218175633830fd"/></artifact>'
            '<artifact><sha256 value="5468629ab3793f4768c0527b8ecca25219a00b5d9503d1ad657cc664c87e9081"/></artifact>'
            "</component></components></verification-metadata>"
        ),
        workflow_text="flutter build apk --no-pub\n",
    )


def expect_failure(config: AndroidConfig, label: str) -> None:
    try:
        validate(config)
    except SystemExit:
        return
    fail(f"{label} negative control was accepted")


def replace(config: AndroidConfig, **values: str) -> AndroidConfig:
    fields = {
        "flutter_version": config.flutter_version,
        "properties": config.properties,
        "settings": config.settings,
        "app_gradle": config.app_gradle,
        "plugin_gradle": config.plugin_gradle,
        "root_gradle": config.root_gradle,
        "wrapper": config.wrapper,
        "lockfile": config.lockfile,
        "verification_metadata": config.verification_metadata,
        "workflow_text": config.workflow_text,
    }
    fields.update(values)
    return AndroidConfig(**fields)


def self_test() -> None:
    with tempfile.TemporaryDirectory(prefix="hidlins-android-config-"):
        config = valid_fixture()
        validate(config)
        cases = {
            "built-in-disabled": replace(
                config,
                properties=config.properties.replace("builtInKotlin=true", "builtInKotlin=false"),
            ),
            "unsupported-new-dsl": replace(
                config,
                properties=config.properties.replace("newDsl=false", "newDsl=true"),
            ),
            "agp-drift": replace(config, settings=config.settings.replace("9.1.0", "9.2.0")),
            "kotlin-drift": replace(config, settings=config.settings.replace("2.4.0", "2.3.20")),
            "gradle-drift": replace(config, wrapper=config.wrapper.replace("9.3.1", "9.4.0")),
            "applied-kgp": replace(
                config,
                app_gradle=config.app_gradle.replace(
                    'id("com.android.application")',
                    'id("com.android.application"); id("org.jetbrains.kotlin.android")',
                ),
            ),
            "validation-bypass": replace(
                config,
                workflow_text="flutter build apk --android-" + "skip-build-dependency-validation",
            ),
        }
        for label, invalid in cases.items():
            expect_failure(invalid, label)
    print("  OK: Android build-model contract rejects seven invalid configurations")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("self-test", "check"))
    args = parser.parse_args()
    if args.command == "self-test":
        self_test()
    else:
        validate(load_production())
        print("  OK: Flutter 3.47.2 Android built-in Kotlin/old-DSL contract matches")


if __name__ == "__main__":
    main()
