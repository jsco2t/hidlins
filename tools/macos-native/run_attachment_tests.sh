#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
derived_data_dir="$(mktemp -d "${TMPDIR:-/tmp}/hidlins-macos-xctest.XXXXXX")"
trap 'rm -rf "$derived_data_dir"' EXIT

# Restore a stable app entrypoint after Flutter integration-test builds.
(cd "$repo_root/app" && flutter build macos --debug --config-only --no-pub)
xcodebuild test \
  -workspace "$repo_root/app/macos/Runner.xcworkspace" \
  -scheme Runner \
  -configuration Debug \
  -destination 'platform=macOS' \
  -derivedDataPath "$derived_data_dir" \
  CODE_SIGNING_ALLOWED=NO
