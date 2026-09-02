#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
app_dir="$repo_root/app"
device_json="$(mktemp "${TMPDIR:-/tmp}/hidlins-ios-devices.XXXXXX")"
matrix="$(mktemp "${TMPDIR:-/tmp}/hidlins-ios-matrix.XXXXXX")"
trap 'rm -f "$device_json" "$matrix"' EXIT

xcrun simctl list --json devices available >"$device_json"
python3 "$repo_root/tools/ios-native/select_simulators.py" "$device_json" >"$matrix"

while IFS=$'\t' read -r udid name runtime; do
  echo "iOS integration simulator: name=$name runtime=iOS $runtime id=$udid"
  xcrun simctl boot "$udid" >/dev/null 2>&1 || true
  xcrun simctl bootstatus "$udid" -b
  (
    cd "$app_dir"
    flutter test --no-pub -d "$udid" integration_test/ios_simulator_ui_test.dart
  )
  (
    cd "$app_dir"
    flutter test --no-pub -d "$udid" integration_test/ios_real_bridge_test.dart
  )
done <"$matrix"
