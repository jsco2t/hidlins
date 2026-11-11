#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
scratch="$(mktemp -d "${TMPDIR:-/tmp}/hidlins-cocoa-selection.XXXXXX")"
trap 'rm -rf "$scratch"' EXIT
flutter_root="$(python3 -c 'from pathlib import Path; import shutil; print(Path(shutil.which("flutter")).resolve().parent.parent)')"
frameworks="$flutter_root/bin/cache/artifacts/engine/darwin-x64/FlutterMacOS.xcframework/macos-arm64_x86_64"
xcrun swiftc -swift-version 5 -warnings-as-errors \
  -F "$frameworks" -Xlinker -rpath -Xlinker "$frameworks" \
  "$repo_root/app/macos/Runner/HidlinsAttachmentExport.swift" \
  "$repo_root/tools/macos-native/AttachmentSelectionTests.swift" \
  -o "$scratch/native-selection"
"$scratch/native-selection"
