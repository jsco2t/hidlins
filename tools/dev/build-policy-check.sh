#!/usr/bin/env bash
# Deterministic regression gate for the repository warning policy.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fixture="$repo_root/tools/build-policy/rust-warning-fixture/Cargo.toml"

fail() {
    echo "error: build policy check failed: $*" >&2
    exit 1
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

echo "  OK: caller flags preserved; Rust warning policy is enforced"
