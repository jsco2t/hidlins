#!/usr/bin/env python3
"""Normalize Flutter's Android registrant for offline debug/release builds."""

from __future__ import annotations

import argparse
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = (
    ROOT
    / "app/android/app/src/main/java/io/flutter/plugins/GeneratedPluginRegistrant.java"
)

SOURCE = """package io.flutter.plugins;

import androidx.annotation.Keep;
import androidx.annotation.NonNull;
import io.flutter.Log;
import io.flutter.embedding.engine.FlutterEngine;
import io.flutter.embedding.engine.plugins.FlutterPlugin;

/**
 * Repository-normalized Flutter registrant.
 *
 * The production Android plugin is FFI-only and needs no method-channel
 * registration. The SDK's integration_test dev dependency is present only in
 * debug builds, so resolve it reflectively without creating a release compile
 * dependency on test code.
 */
@Keep
public final class GeneratedPluginRegistrant {
    private static final String TAG = "GeneratedPluginRegistrant";

    private GeneratedPluginRegistrant() {}

    public static void registerWith(@NonNull FlutterEngine flutterEngine) {
        try {
            FlutterPlugin plugin =
                    Class.forName("dev.flutter.plugins.integration_test.IntegrationTestPlugin")
                            .asSubclass(FlutterPlugin.class)
                            .getDeclaredConstructor()
                            .newInstance();
            flutterEngine.getPlugins().add(plugin);
        } catch (ClassNotFoundException ignored) {
            // Expected in production builds, which exclude dev dependencies.
        } catch (ReflectiveOperationException exception) {
            Log.e(TAG, "Could not register the integration-test plugin", exception);
        }
    }
}
"""


def fail(message: str) -> None:
    raise SystemExit(f"error: {message}")


def install(path: Path = OUTPUT) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(SOURCE, encoding="utf-8")


def check(path: Path = OUTPUT) -> None:
    if not path.is_file() or path.read_text(encoding="utf-8") != SOURCE:
        fail("Android GeneratedPluginRegistrant is missing or was not normalized")


def self_test() -> None:
    with tempfile.TemporaryDirectory(prefix="hidlins-plugin-registrant-") as scratch:
        output = Path(scratch) / "GeneratedPluginRegistrant.java"
        install(output)
        check(output)
        output.write_text(SOURCE.replace("Class.forName", "new IntegrationTestPlugin"))
        try:
            check(output)
        except SystemExit:
            pass
        else:
            fail("direct dev-plugin registrant negative control was accepted")
    print("  OK: Android registrant normalizer rejects release-incompatible dev-plugin wiring")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("install", "check", "self-test"))
    args = parser.parse_args()
    if args.command == "install":
        install()
    elif args.command == "check":
        check()
    else:
        self_test()


if __name__ == "__main__":
    main()
