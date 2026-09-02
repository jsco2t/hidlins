#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
app_dir="$repo_root/app"
device_json="$(mktemp "${TMPDIR:-/tmp}/hidlins-ios-devices.XXXXXX")"
matrix="$(mktemp "${TMPDIR:-/tmp}/hidlins-ios-matrix.XXXXXX")"
defines_dir="$(mktemp -d "${TMPDIR:-/tmp}/hidlins-ios-s3.XXXXXX")"
defines="$defines_dir/config.json"
flutter_config_dir="$(mktemp -d "${TMPDIR:-/tmp}/hidlins-flutter-config.XXXXXX")"
build_rel=".dart_tool/hidlins-ios-s3-build.$PPID.$RANDOM"
build_dir="$app_dir/$build_rel"
udid=""

cleanup() {
  if [ -n "$udid" ]; then
    xcrun simctl uninstall "$udid" app.hidlins.ios >/dev/null 2>&1 || true
  fi
  rm -f "$device_json" "$matrix" "$defines"
  rmdir "$defines_dir" 2>/dev/null || true
  case "$flutter_config_dir" in
    "${TMPDIR:-/tmp}"/hidlins-flutter-config.*)
      rm -rf "$flutter_config_dir"
      ;;
    *)
      echo "error: refusing to remove unexpected Flutter config directory: $flutter_config_dir" >&2
      ;;
  esac
  case "$build_dir" in
    "$app_dir"/.dart_tool/hidlins-ios-s3-build.*)
      rm -rf "$build_dir"
      ;;
    *)
      echo "error: refusing to remove unexpected build directory: $build_dir" >&2
      ;;
  esac
}
trap cleanup EXIT

xcrun simctl list --json devices available >"$device_json"
python3 "$repo_root/tools/ios-native/select_simulators.py" "$device_json" >"$matrix"
IFS=$'\t' read -r udid name runtime <"$matrix"

case "${1:-}" in
  minio)
    bucket="hidlins-ios-$PPID-$RANDOM"
    "$repo_root/tools/sync-tests/fixtures/make_bucket.sh" "$bucket"
    python3 "$repo_root/tools/ios-native/secure_s3_config.py" minio \
      "$repo_root/tools/sync-tests/fixtures/.minio-env" "$defines" \
      --bucket "$bucket"
    service="managed MinIO"
    ;;
  real)
    config_path="${HIDLINS_IOS_S3_CONFIG:-}"
    if [ -z "$config_path" ]; then
      echo "error: HIDLINS_IOS_S3_CONFIG must name a mode-0600 JSON config" >&2
      echo "       See app/ios/VERIFICATION.md; the path is non-secret, values never enter argv or environment." >&2
      exit 2
    fi
    python3 "$repo_root/tools/ios-native/secure_s3_config.py" real \
      "$config_path" "$defines"
    service="configured S3-compatible service"
    ;;
  *)
    echo "usage: $0 {minio|real}" >&2
    exit 2
    ;;
esac

echo "iOS S3 integration: service=$service simulator=$name runtime=iOS $runtime id=$udid"
xcrun simctl boot "$udid" >/dev/null 2>&1 || true
xcrun simctl bootstatus "$udid" -b
(
  cd "$app_dir"
  XDG_CONFIG_HOME="$flutter_config_dir" flutter config --build-dir="$build_rel" >/dev/null
  XDG_CONFIG_HOME="$flutter_config_dir" flutter test --no-pub -d "$udid" \
    --dart-define-from-file="$defines" \
    integration_test/ios_real_bridge_s3_test.dart
)
