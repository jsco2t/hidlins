#!/usr/bin/env bash

set -e

BASEDIR=$(dirname "$0")

# Hidlins patch: resolve BASEDIR to absolute so PUB_CACHE resolution works
# regardless of which directory the script is invoked from.
BASEDIR=$(cd "$BASEDIR" ; pwd -P)

mkdir -p "$CARGOKIT_TOOL_TEMP_DIR"

cd "$CARGOKIT_TOOL_TEMP_DIR"

BUILD_TOOL_PKG_DIR="$BASEDIR/build_tool"

# Hidlins patch: use the build_tool's vendored pub cache for offline builds.
export PUB_CACHE="$BUILD_TOOL_PKG_DIR/vendor-pub"

if [[ -z $FLUTTER_ROOT ]]; then
  DART=dart
else
  DART="$FLUTTER_ROOT/bin/cache/dart-sdk/bin/dart"
fi

cat << EOF > "pubspec.yaml"
name: build_tool_runner
version: 1.0.0
publish_to: none

environment:
  sdk: '>=3.0.0 <4.0.0'

dependencies:
  build_tool:
    path: "$BUILD_TOOL_PKG_DIR"
EOF

mkdir -p "bin"

cat << EOF > "bin/build_tool_runner.dart"
import 'package:build_tool/build_tool.dart' as build_tool;
void main(List<String> args) {
  build_tool.runMain(args);
}
EOF

if ! [ -x "$(command -v shasum)" ] && [ -x "$(command -v sha1sum)" ]; then
  shopt -s expand_aliases
  alias shasum="sha1sum"
fi

if [[ "$OSTYPE" == "darwin"* ]]; then
  PACKAGE_HASH=$(ls -lTR "$BUILD_TOOL_PKG_DIR" | shasum)
else
  PACKAGE_HASH=$(ls -lR --full-time "$BUILD_TOOL_PKG_DIR" | shasum)
fi

PACKAGE_HASH_FILE=".package_hash"

if [ -f "$PACKAGE_HASH_FILE" ]; then
    EXISTING_HASH=$(cat "$PACKAGE_HASH_FILE")
    if [ "$PACKAGE_HASH" != "$EXISTING_HASH" ]; then
        rm "$PACKAGE_HASH_FILE"
    fi
fi

# Hidlins patch: --offline resolves from vendored PUB_CACHE.
if [ ! -f "$PACKAGE_HASH_FILE" ]; then
    "$DART" pub get --no-precompile --offline
    "$DART" compile kernel bin/build_tool_runner.dart
    echo "$PACKAGE_HASH" > "$PACKAGE_HASH_FILE"
fi

if [ ! -f "bin/build_tool_runner.dill" ]; then
  "$DART" compile kernel bin/build_tool_runner.dart
fi

set +e

"$DART" bin/build_tool_runner.dill "$@"

exit_code=$?

if [ $exit_code == 253 ]; then
  "$DART" pub get --no-precompile --offline
  "$DART" compile kernel bin/build_tool_runner.dart
  "$DART" bin/build_tool_runner.dill "$@"
  exit_code=$?
fi

exit $exit_code
