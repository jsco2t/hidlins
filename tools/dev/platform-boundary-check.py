#!/usr/bin/env python3
"""Fail closed around Hidlins' five fixed native capability adapters."""

from __future__ import annotations

import argparse
import json
import re
import tempfile
from pathlib import Path

EXPECTED = {
    "lifecycle": ("lifecycle.dart", "app.hidlins/lifecycle", ("reportState",)),
    "clipboard": ("secure_clipboard.dart", "app.hidlins/clipboard", ("copySecret",)),
    "paths": ("app_paths.dart", "app.hidlins/paths", ("applicationSupportPath",)),
    "vault_import": ("vault_import.dart", "app.hidlins/vault_import", ("pickVault",)),
    "keyfile": (
        "keyfile_access.dart",
        "app.hidlins/keyfile",
        ("pickReference", "resolveReference", "releaseReference"),
    ),
}
CHANNEL_DECL = re.compile(r"channelName\s*=\s*['\"]([^'\"]+)['\"]")
METHOD_DECL = re.compile(r"method:\s*['\"]([^'\"]+)['\"]")
CHANNEL_CTOR = re.compile(
    r"(?:MethodChannel|EventChannel|BasicMessageChannel)(?:<[^>]+>)?\(\s*['\"]([^'\"]+)['\"]"
)


def validate(root: Path) -> list[str]:
    errors: list[str] = []
    platform = root / "app/lib/src/platform"
    manifest_path = platform / "channels.json"
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"cannot read platform channel manifest: {error}"]
    if manifest.get("schema") != 1:
        errors.append("platform channel manifest schema must be 1")
    entries = manifest.get("capabilities")
    if not isinstance(entries, list):
        return errors + ["platform channel capabilities must be a list"]
    actual: dict[str, tuple[str, str, tuple[str, ...]]] = {}
    for entry in entries:
        if not isinstance(entry, dict) or not isinstance(entry.get("name"), str):
            errors.append("platform channel capability entry is malformed")
            continue
        name = entry["name"]
        if name in actual:
            errors.append(f"duplicate capability owner: {name}")
            continue
        if entry.get("policy_owner") != "rust" or entry.get("mechanism_owner") != "native":
            errors.append(f"{name} must have rust policy and native mechanism owners")
        actual[name] = (
            entry.get("file"),
            entry.get("channel"),
            tuple(entry.get("methods", [])),
        )
    if actual != EXPECTED:
        errors.append("platform channel manifest differs from the fixed capability contract")

    declared_channels: set[str] = set()
    for name, (file_name, channel, methods) in EXPECTED.items():
        path = platform / file_name
        try:
            source = path.read_text(encoding="utf-8")
        except OSError as error:
            errors.append(f"missing {name} adapter: {error}")
            continue
        channels = CHANNEL_DECL.findall(source)
        # A capability may invoke the same approved native method from more
        # than one adapter operation. The boundary is the set of method names,
        # not the number of call sites, so preserve first-seen order while
        # removing duplicates before comparing with the reviewed contract.
        found_methods = tuple(dict.fromkeys(METHOD_DECL.findall(source)))
        if channels != [channel]:
            errors.append(f"{file_name} must declare only channel {channel}")
        if found_methods != methods:
            errors.append(f"{file_name} methods {found_methods} differ from {methods}")
        declared_channels.update(channels)
    if declared_channels != {value[1] for value in EXPECTED.values()}:
        errors.append("declared platform channel set is incomplete or contains an undeclared channel")

    for dart in (root / "app").rglob("*.dart"):
        source = dart.read_text(encoding="utf-8")
        constructors = CHANNEL_CTOR.findall(source)
        if constructors:
            relative = dart.relative_to(root).as_posix()
            errors.append(
                f"native channel constructor outside fixed dispatcher: {relative}: {constructors}"
            )
    dispatcher = platform / "platform_channel.dart"
    if "MethodChannel(channel)" not in dispatcher.read_text(encoding="utf-8"):
        errors.append("fixed dispatcher must own the sole dynamic MethodChannel constructor")
    return errors


def write_fixture(root: Path) -> None:
    platform = root / "app/lib/src/platform"
    platform.mkdir(parents=True)
    capabilities = []
    for name, (file_name, channel, methods) in EXPECTED.items():
        capabilities.append(
            {
                "name": name,
                "file": file_name,
                "channel": channel,
                "methods": list(methods),
                "policy_owner": "rust",
                "mechanism_owner": "native",
            }
        )
        method_source = "\n".join(f"method: '{method}'," for method in methods)
        (platform / file_name).write_text(
            f"static const channelName = '{channel}';\n{method_source}\n",
            encoding="utf-8",
        )
    (platform / "channels.json").write_text(
        json.dumps({"schema": 1, "capabilities": capabilities}), encoding="utf-8"
    )
    (platform / "platform_channel.dart").write_text(
        "final value = MethodChannel(channel);\n", encoding="utf-8"
    )


def self_test() -> None:
    mutations = {
        "undeclared channel": lambda root: (root / "app/lib/src/platform/lifecycle.dart").write_text(
            "static const channelName = 'app.hidlins/undeclared';\nmethod: 'reportState',\n",
            encoding="utf-8",
        ),
        "declared channel outside platform directory": lambda root: (
            root / "app/test/channel_bypass.dart"
        ).write_text("final c = MethodChannel('app.hidlins/lifecycle');\n", encoding="utf-8"),
        "unlisted method": lambda root: (root / "app/lib/src/platform/lifecycle.dart").write_text(
            "static const channelName = 'app.hidlins/lifecycle';\n"
            "method: 'reportState',\nmethod: 'lockNow',\n",
            encoding="utf-8",
        ),
    }
    for label, mutate in mutations.items():
        with tempfile.TemporaryDirectory(prefix="hidlins-platform-boundary-") as scratch:
            root = Path(scratch)
            write_fixture(root)
            (root / "app/test").mkdir(parents=True, exist_ok=True)
            if validate(root):
                raise SystemExit("error: valid platform-boundary fixture was rejected")
            mutate(root)
            if not validate(root):
                raise SystemExit(f"error: {label} negative control was accepted")
    print("  OK: undeclared channel, wrong location, and unlisted method controls fail")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("check", "self-test"))
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    if args.command == "self-test":
        self_test()
        return
    errors = validate(args.root)
    if errors:
        raise SystemExit("\n".join(f"error: {error}" for error in errors))
    print("  OK: platform channels match the fixed five-capability manifest")


if __name__ == "__main__":
    main()
