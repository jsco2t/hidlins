#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly CONTRACT="$ROOT/tools/android-native/artifact-contract.json"
readonly NDK_VERSION="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["ndk_version"])' "$CONTRACT")"
readonly API_LEVEL="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["android_api_level"])' "$CONTRACT")"

die() { printf 'error: %s\n' "$*" >&2; exit 1; }

detect_sdk() {
  if [[ -n "${ANDROID_SDK_ROOT:-}" ]]; then printf '%s\n' "$ANDROID_SDK_ROOT"; return; fi
  if [[ -n "${ANDROID_HOME:-}" ]]; then printf '%s\n' "$ANDROID_HOME"; return; fi
  case "$(uname -s)" in
    Darwin) printf '%s\n' "${HOME:?}/Library/Android/sdk" ;;
    Linux) printf '%s\n' "${HOME:?}/Android/Sdk" ;;
    *) die "Android cross-build supports macOS and Linux hosts only" ;;
  esac
}

readonly SDK_ROOT="$(detect_sdk)"
readonly NDK_ROOT="${ANDROID_NDK_HOME:-$SDK_ROOT/ndk/$NDK_VERSION}"
case "$(uname -s):$(uname -m)" in
  Darwin:arm64|Darwin:x86_64) readonly HOST_TAG="darwin-x86_64" ;;
  Linux:x86_64) readonly HOST_TAG="linux-x86_64" ;;
  Linux:*) die "the Android NDK does not provide a Linux $(uname -m) host toolchain" ;;
  *) die "unsupported Android build host" ;;
esac
readonly NDK_BIN="$NDK_ROOT/toolchains/llvm/prebuilt/$HOST_TAG/bin"
readonly AARCH64_CC="$NDK_BIN/aarch64-linux-android${API_LEVEL}-clang"
readonly X86_64_CC="$NDK_BIN/x86_64-linux-android${API_LEVEL}-clang"

[[ -x "$AARCH64_CC" && -x "$X86_64_CC" && -x "$NDK_BIN/llvm-ar" ]] || die \
  "NDK $NDK_VERSION is missing under $NDK_ROOT"
grep -q "Pkg.BaseRevision = $NDK_VERSION" "$NDK_ROOT/source.properties" || die \
  "NDK source.properties does not match required $NDK_VERSION"

export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$AARCH64_CC"
export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$X86_64_CC"
export CC_aarch64_linux_android="$AARCH64_CC"
export CC_x86_64_linux_android="$X86_64_CC"
export AR_aarch64_linux_android="$NDK_BIN/llvm-ar"
export AR_x86_64_linux_android="$NDK_BIN/llvm-ar"

readonly CARGO_ARGS=(-p hidlins-api --no-default-features --offline --locked)
case "$(uname -m)" in
  arm64|aarch64) readonly HOST_ANDROID_TARGET="aarch64-linux-android" ;;
  x86_64) readonly HOST_ANDROID_TARGET="x86_64-linux-android" ;;
  *) die "unsupported Android scenario host architecture" ;;
esac
readonly SCENARIO_CARGO_ARGS=(-p hidlins-cli --no-default-features --features android-scenario-authority --offline --locked)
case "${1:-}" in
  check)
    cargo check "${CARGO_ARGS[@]}" --target aarch64-linux-android
    cargo check "${CARGO_ARGS[@]}" --target x86_64-linux-android
    ;;
  build)
    cargo build "${CARGO_ARGS[@]}" --release --target aarch64-linux-android
    cargo build "${CARGO_ARGS[@]}" --release --target x86_64-linux-android
    python3 "$ROOT/tools/android-native/artifacts.py" stage
    printf '  OK: release native libraries pass for both Android ABIs\n'
    ;;
  scenario-check)
    cargo check "${SCENARIO_CARGO_ARGS[@]}" --target "$HOST_ANDROID_TARGET"
    ;;
  scenario-build)
    cargo build "${SCENARIO_CARGO_ARGS[@]}" --release --target "$HOST_ANDROID_TARGET"
    printf '%s\n' "$ROOT/target/$HOST_ANDROID_TARGET/release/hidlins"
    ;;
  *) die "usage: $0 {check|build|scenario-check|scenario-build}" ;;
esac
