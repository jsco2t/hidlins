#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT/app"

# Flutter's generated Gradle integration performs the application packaging;
# the repository-owned Rust build has already staged checked release libraries.
flutter build apk --debug --no-pub --target-platform android-arm64,android-x64
flutter build apk --release --no-pub --no-tree-shake-icons \
  --target-platform android-arm64,android-x64

cd "$ROOT"
python3 tools/android-native/verify_app_apk.py
