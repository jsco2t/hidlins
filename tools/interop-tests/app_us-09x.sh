#!/usr/bin/env bash
# Flutter/API boundary KDBX interop: create and edit through the same Rust API
# used by the desktop app, save once with KeePassXC, and compare normalized XML.
set -euo pipefail

DRIVER="${HIDLINS_API_TEST_DRIVER:-target/debug/api-test-driver}"
PASSWORD="interop-master"

command -v keepassxc-cli >/dev/null 2>&1 || {
    echo "keepassxc-cli is required" >&2
    exit 1
}

version="$(keepassxc-cli --version | awk '{print $NF}')"
python3 - "$version" <<'PY'
import sys

raw = sys.argv[1].split("-")[0]
parts = tuple(int(part) for part in raw.split(".")[:3])
if parts < (2, 7, 12) or parts >= (3, 0, 0):
    raise SystemExit(
        f"KeePassXC 2.7.12+ in the 2.x line is required; found {sys.argv[1]}"
    )
PY

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT

name="desktop-interop"
vault="$workdir/$name.kdbx"
baseline_xml="$workdir/baseline.xml"
final_xml="$workdir/final.xml"

printf '%s\n' "$PASSWORD" | "$DRIVER" create-vault "$workdir" "$name" >/dev/null
printf '%s\n' "$PASSWORD" | "$DRIVER" add-corpus "$workdir" "$name" >/dev/null

# A real KeePassXC write lands between two API-boundary operations.
printf '%s\n%s\n%s\n' "$PASSWORD" "kpxc-secret" "kpxc-secret" |
    keepassxc-cli add -q -u "kpxc@example.test" -p \
        "$vault" "KeePassXC desktop entry" >/dev/null

uuid="$(printf '%s\n' "$PASSWORD" | "$DRIVER" find-entry "$workdir" "$name" "GitHub")"
printf '%s\n%s\n' "$PASSWORD" "desktop-app-user" |
    "$DRIVER" edit-entry "$workdir" "$name" "$uuid" username >/dev/null

printf '%s\n' "$PASSWORD" |
    keepassxc-cli export -q -f xml "$vault" >"$baseline_xml"

# Force a KeePassXC save without changing logical content, then prove the
# API-authored KDBX survives byte-independent normalized round-trip comparison.
printf '%s\n' "$PASSWORD" |
    keepassxc-cli edit -q -u "desktop-app-user" "$vault" "GitHub" >/dev/null
printf '%s\n' "$PASSWORD" |
    keepassxc-cli export -q -f xml "$vault" >"$final_xml"

tools/interop-tests/lib/kpxc-diff.py "$baseline_xml" "$final_xml"
echo "app_us-09x: OK (KeePassXC $version)"
