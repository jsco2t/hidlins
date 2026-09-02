#!/usr/bin/env bash
# Deterministic regression gate for the repository warning policy.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fixture="$repo_root/tools/build-policy/rust-warning-fixture/Cargo.toml"

fail() {
    echo "error: build policy check failed: $*" >&2
    exit 1
}

require_text() {
    local path="$1"
    local text="$2"
    local description="$3"
    grep -Fq -- "$text" "$repo_root/$path" || fail "$description"
}

# The public Make target adds this harmless caller flag before recursively
# invoking us. Seeing both values proves Make appended rather than overwrote.
case " ${RUSTFLAGS:-} " in
    *" -C debuginfo=0 "*) ;;
    *) fail "caller-provided RUSTFLAGS were not preserved" ;;
esac
case " ${RUSTFLAGS:-} " in
    *" -D warnings "*) ;;
    *) fail "-D warnings is absent from exported RUSTFLAGS" ;;
esac

require_text "Makefile" 'app-build-linux: app-deps app-analyze' \
    "Linux Flutter builds do not require fatal Dart analysis"
require_text "Makefile" 'app-build-macos: app-deps app-analyze' \
    "macOS Flutter builds do not require fatal Dart analysis"
require_text "Makefile" 'dart analyze --fatal-warnings --fatal-infos' \
    "Dart analyzer warnings and infos are not fatal"
require_text "app/linux/CMakeLists.txt" \
    'target_compile_options(${BINARY_NAME} PRIVATE -Werror)' \
    "Linux runner warnings are not fatal or are not runner-scoped"
require_text "app/macos/Runner/Configs/AppInfo.xcconfig" \
    'GCC_TREAT_WARNINGS_AS_ERRORS = YES' \
    "macOS Runner C-family warnings are not fatal"
require_text "app/macos/Runner/Configs/AppInfo.xcconfig" \
    'SWIFT_TREAT_WARNINGS_AS_ERRORS = YES' \
    "macOS Runner Swift warnings are not fatal"
require_text "app/ios/Flutter/Debug.xcconfig" \
    'SWIFT_TREAT_WARNINGS_AS_ERRORS = YES' \
    "iOS Debug Runner Swift warnings are not fatal"
require_text "app/ios/Flutter/Release.xcconfig" \
    'SWIFT_TREAT_WARNINGS_AS_ERRORS = YES' \
    "iOS Release/Profile Runner Swift warnings are not fatal"
require_text "app/pubspec.yaml" \
    'enable-swift-package-manager: false' \
    "the CocoaPods/Cargokit Rust bridge exception is not project-pinned"
require_text "app/android/app/build.gradle.kts" \
    'allWarningsAsErrors.set(true)' \
    "Android app Kotlin warnings are not fatal"
require_text "app/android/app/build.gradle.kts" \
    'options.compilerArgs.addAll(listOf("-Xlint:all", "-Werror"))' \
    "Android app Java warnings are not fatal"
require_text "app/rust_builder/cargokit/build_tool/lib/src/android_environment.dart" \
    "Platform.environment['RUSTFLAGS']" \
    "Android Cargokit bridge builds do not preserve Make's Rust warning policy"

probe_dir="$(mktemp -d "${TMPDIR:-/tmp}/hidlins-build-policy.XXXXXX")"
trap 'rm -rf "$probe_dir"' EXIT

if CARGO_TARGET_DIR="$probe_dir/target" \
    cargo check --manifest-path "$fixture" --offline --locked \
    >"$probe_dir/output" 2>&1; then
    cat "$probe_dir/output" >&2
    fail "intentional first-party Rust warning compiled successfully"
fi

grep -Fq 'error: unused variable: `warning_only_first_party_code`' \
    "$probe_dir/output" || {
    cat "$probe_dir/output" >&2
    fail "probe failed for a reason other than the intentional warning"
}

echo "  OK: caller flags preserved; Rust, Dart, Linux, Apple, and Android warning policies are enforced"
