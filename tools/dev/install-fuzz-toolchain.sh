#!/usr/bin/env bash
# Install the exact development-only local-sync fuzz toolchain.

set -euo pipefail

CARGO_FUZZ_VERSION="0.13.2"
FUZZ_NIGHTLY="nightly-2026-09-01"

rustup toolchain install "$FUZZ_NIGHTLY" --profile minimal

installed=""
if command -v cargo-fuzz >/dev/null 2>&1; then
  installed="$(cargo fuzz --version 2>/dev/null | sed -n 's/^cargo-fuzz //p')"
fi
if [ "$installed" != "$CARGO_FUZZ_VERSION" ]; then
  (
    cd "${TMPDIR:-/tmp}"
    CARGO_NET_OFFLINE=false RUSTUP_TOOLCHAIN=1.95.0 \
      cargo install --locked --force cargo-fuzz --version "$CARGO_FUZZ_VERSION"
  )
fi

test "$(cargo fuzz --version | sed -n 's/^cargo-fuzz //p')" = "$CARGO_FUZZ_VERSION"
rustup run "$FUZZ_NIGHTLY" rustc --version
