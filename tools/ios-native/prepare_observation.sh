#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
device_json="$(mktemp "${TMPDIR:-/tmp}/hidlins-ios-devices.XXXXXX")"
matrix="$(mktemp "${TMPDIR:-/tmp}/hidlins-ios-matrix.XXXXXX")"
trap 'rm -f "$device_json" "$matrix"' EXIT

xcrun simctl list --json devices available >"$device_json"
python3 "$repo_root/tools/ios-native/select_simulators.py" "$device_json" >"$matrix"
IFS=$'\t' read -r udid name runtime <"$matrix"

echo "Preparing iOS observation: name=$name runtime=iOS $runtime id=$udid"
xcrun simctl boot "$udid" >/dev/null 2>&1 || true
xcrun simctl bootstatus "$udid" -b
xcrun simctl install \
  "$udid" "$repo_root/app/build/ios/iphonesimulator/Runner.app"
xcrun simctl launch "$udid" app.hidlins.ios
