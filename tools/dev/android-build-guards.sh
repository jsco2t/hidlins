#!/usr/bin/env bash
# Android build-chain invariants shared by Makefile gates.

set -euo pipefail

readonly JNI_INIT_SYMBOL="Java_app_hidlins_HidlinsNative_initVerifier"

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

has_api_floor() {
  local gradle_file="$1"
  local api_level="$2"
  local pattern="^[[:space:]]*minSdk[[:space:]]*=[[:space:]]*${api_level}[[:space:]]*(//.*)?$"
  grep -Eq "$pattern" "$gradle_file"
}

assert_api_floor() {
  local gradle_file="$1"
  local api_level="$2"

  [[ "$api_level" =~ ^[0-9]+$ ]] || die "Android API level must be numeric, got '$api_level'."
  [[ -f "$gradle_file" ]] || die "Gradle file not found: $gradle_file"
  has_api_floor "$gradle_file" "$api_level" || die \
    "Android API level $api_level does not match the live minSdk assignment in $gradle_file."
}

has_exact_jni_symbol() {
  awk -v symbol="$JNI_INIT_SYMBOL" '$NF == symbol { found = 1 } END { exit !found }'
}

assert_jni_symbol() {
  local nm_bin="$1"
  local library="$2"
  local nm_output

  [[ -x "$nm_bin" ]] || die "llvm-nm is not executable: $nm_bin"
  [[ -f "$library" ]] || die "Android cdylib not found: $library"
  nm_output="$("$nm_bin" -D --defined-only "$library")" || die "llvm-nm failed for $library"
  printf '%s\n' "$nm_output" | has_exact_jni_symbol || die \
    "JNI export $JNI_INIT_SYMBOL is missing from $library."
}

self_test() {
  local scratch_dir
  scratch_dir="$(mktemp -d "${TMPDIR:-/tmp}/hidlins-android-guards.XXXXXX")"
  trap 'rm -rf "$scratch_dir"' RETURN

  printf '    minSdk = 29\n' >"$scratch_dir/valid.gradle.kts"
  assert_api_floor "$scratch_dir/valid.gradle.kts" 29

  printf '    minSdk=29 // design floor\n' >"$scratch_dir/valid-comment.gradle.kts"
  assert_api_floor "$scratch_dir/valid-comment.gradle.kts" 29

  printf '// stale minSdk = 29\n    minSdk = 30\n' >"$scratch_dir/stale-comment.gradle.kts"
  if has_api_floor "$scratch_dir/stale-comment.gradle.kts" 29; then
    die "API-floor guard accepted a commented-out stale assignment."
  fi

  printf '00000000 T %s\n' "$JNI_INIT_SYMBOL" | has_exact_jni_symbol || die \
    "JNI-symbol guard rejected the exact export."
  if printf '00000000 T %sV2\n' "$JNI_INIT_SYMBOL" | has_exact_jni_symbol; then
    die "JNI-symbol guard accepted a suffixed export."
  fi
  if printf '00000000 T %s__Ljava_lang_Object_2\n' "$JNI_INIT_SYMBOL" | has_exact_jni_symbol; then
    die "JNI-symbol guard accepted an overload-mangled export."
  fi

  printf '  OK: Android build guards reject comments and partial JNI symbols\n'
}

case "${1:-}" in
  api-floor)
    [[ $# -eq 3 ]] || die "usage: $0 api-floor <build.gradle.kts> <api-level>"
    assert_api_floor "$2" "$3"
    ;;
  jni-symbol)
    [[ $# -eq 3 ]] || die "usage: $0 jni-symbol <llvm-nm> <libhidlins_api.so>"
    assert_jni_symbol "$2" "$3"
    ;;
  self-test)
    [[ $# -eq 1 ]] || die "usage: $0 self-test"
    self_test
    ;;
  *)
    die "usage: $0 {api-floor <build.gradle.kts> <api-level>|jni-symbol <llvm-nm> <library>|self-test}"
    ;;
esac
