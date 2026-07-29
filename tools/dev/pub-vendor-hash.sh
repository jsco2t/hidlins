#!/usr/bin/env bash
# pub-vendor-hash.sh — per-file SHA-256 manifest for the committed pub caches.
#
# cargo verifies vendor/ byte-for-byte at build time via each crate's
# .cargo-checksum.json. pub has no equivalent for the *extracted* package
# trees: `dart pub get` only cross-checks pubspec.lock against the archive
# hashes in hosted-hashes/, so a modified source file inside
# app/vendor-pub/hosted/... would execute in CI with every pub gate green.
# This manifest closes that gap: `generate` (run by `make pub-vendor`) records
# every file in both committed cache trees; `verify` (run by
# `make pub-vendor-check`) regenerates and diffs, so modification, addition,
# and removal all fail loudly.
#
# Only hosted/ and hosted-hashes/ are covered — the runtime dirs pub creates
# locally (_temp/, active_roots/) are machine state and not committed.
#
# Known limits (accepted): trust-on-first-use — `make pub-vendor` blesses
# whatever pub.dev served that day; and an attacker who can land a commit can
# regenerate the manifest in the same commit (the defense there is review of
# the paired diff, same as for vendor/).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
MANIFEST="$REPO_ROOT/tools/dev/pub-vendor.SHA256SUMS"

# Keep in sync with the `sig()` roots in the Makefile's pub-vendor-check.
TREES=(
  app/vendor-pub/hosted
  app/vendor-pub/hosted-hashes
  app/rust_builder/cargokit/build_tool/vendor-pub/hosted
  app/rust_builder/cargokit/build_tool/vendor-pub/hosted-hashes
)

# Stock macOS has no GNU sha256sum; shasum -a 256 emits the identical
# "HASH  path" line format, so manifests stay byte-compatible across hosts.
if command -v sha256sum >/dev/null 2>&1; then
  HASH_CMD=(sha256sum)
else
  HASH_CMD=(shasum -a 256)
fi

require_trees() {
  local missing=0
  for t in "${TREES[@]}"; do
    if [ ! -d "$REPO_ROOT/$t" ]; then
      echo "FAIL: expected vendored tree missing: $t" >&2
      missing=1
    fi
  done
  [ "$missing" -eq 0 ] || exit 1
}

reject_symlinks() {
  # `find -type f` alone would skip symlinks, which pub and the Dart
  # toolchain happily follow — a smuggled link would be invisible to the
  # manifest while its target's content executes. Neither tree legitimately
  # contains one.
  local links
  links=$(cd "$REPO_ROOT" && find "${TREES[@]}" -type l | head -20 || true)
  if [ -n "$links" ]; then
    echo "FAIL: symlink(s) inside a vendored pub cache (not hashable, not allowed):" >&2
    printf '%s\n' "$links" | sed 's/^/       /' >&2
    exit 1
  fi
}

hash_trees() {
  # Repo-relative paths + LC_ALL=C sort make the manifest stable across
  # machines; -print0 survives any filename pub can produce.
  (
    cd "$REPO_ROOT"
    find "${TREES[@]}" -type f -print0 | LC_ALL=C sort -z | xargs -0 "${HASH_CMD[@]}"
  )
}

case "${1:-}" in
  generate)
    require_trees
    reject_symlinks
    # Write-then-rename so an interrupted run can never leave a truncated
    # manifest behind (same discipline the vault writer follows).
    tmp="$(mktemp "$MANIFEST.XXXXXX")"
    trap 'rm -f "$tmp"' EXIT
    hash_trees > "$tmp"
    mv "$tmp" "$MANIFEST"
    trap - EXIT
    echo "  OK: wrote $(wc -l < "$MANIFEST") entries to ${MANIFEST#"$REPO_ROOT"/}"
    ;;
  verify)
    if [ ! -f "$MANIFEST" ]; then
      echo "FAIL: ${MANIFEST#"$REPO_ROOT"/} missing — run \`make pub-vendor\` and commit it." >&2
      exit 1
    fi
    require_trees
    reject_symlinks
    current="$(mktemp)"
    trap 'rm -f "$current"' EXIT
    hash_trees > "$current"
    if ! diff -u "$MANIFEST" "$current" > /dev/null; then
      echo "FAIL: vendored pub cache does not match its committed manifest" >&2
      echo "       ('<' = in manifest only: removed/modified; '>' = on disk only: added/modified;" >&2
      echo "        first 40 lines shown — run tools/dev/pub-vendor-hash.sh verify locally for the full diff):" >&2
      # diff exits 1 by construction here; without `|| true`, errexit would
      # kill the script before the remediation hint below.
      diff "$MANIFEST" "$current" | head -40 | sed 's/^/       /' >&2 || true
      echo "       If this change is intentional, run \`make pub-vendor\` and commit the result." >&2
      exit 1
    fi
    echo "  OK: vendored pub caches match the committed SHA-256 manifest"
    ;;
  *)
    echo "usage: $0 {generate|verify}" >&2
    exit 2
    ;;
esac
