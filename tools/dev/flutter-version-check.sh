#!/usr/bin/env bash
# flutter-version-check.sh — assert the installed Flutter matches .flutter-version.
#
# ONE implementation shared by `make toolchain` (lenient: warn) and CI
# (strict: fail). Previously the check existed twice — once here and once
# inline in the CI workflow — with different parsers AND different severities,
# so the two could reach opposite conclusions about the same machine. That is
# precisely the drift CLAUDE.md's "CI invokes make" rule exists to prevent.
#
# Usage: flutter-version-check.sh [--strict]
#   --strict  exit non-zero on mismatch or missing SDK (CI). Default warns.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
VERSION_FILE="$REPO_ROOT/.flutter-version"
STRICT=0
[ "${1:-}" = "--strict" ] && STRICT=1

fail_or_warn() {
  if [ "$STRICT" -eq 1 ]; then
    printf 'FAIL: %s\n' "$1" >&2
    exit 1
  fi
  printf 'WARN: %s\n' "$1" >&2
  exit 0
}

if [ ! -f "$VERSION_FILE" ]; then
  fail_or_warn ".flutter-version not found at repo root"
fi
EXPECTED="$(tr -d '[:space:]' < "$VERSION_FILE")"

if ! command -v flutter >/dev/null 2>&1; then
  fail_or_warn "Flutter is not installed (expected $EXPECTED). See https://docs.flutter.dev/install"
fi

# Prefer the machine-readable form; fall back to the banner. An unparseable
# result stays empty and therefore never accidentally compares equal.
ACTUAL="$(flutter --version --machine 2>/dev/null \
  | sed -n 's/.*"frameworkVersion"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)"
if [ -z "$ACTUAL" ]; then
  ACTUAL="$(flutter --version 2>/dev/null | sed -n 's/^Flutter \([0-9][0-9.]*\).*/\1/p' | head -n1)"
fi

if [ -z "$ACTUAL" ]; then
  fail_or_warn "could not parse the installed Flutter version (expected $EXPECTED)"
fi

if [ "$ACTUAL" != "$EXPECTED" ]; then
  fail_or_warn "Flutter version mismatch: installed=$ACTUAL, expected=$EXPECTED (.flutter-version)"
fi

printf '  OK: Flutter %s (matches .flutter-version)\n' "$ACTUAL"
