#!/usr/bin/env bash
# install-toolchain.sh — bootstrap the Hidlins developer toolchain.
#
# Cross-platform (Linux + macOS; Windows is out of scope per CLAUDE.md).
# Run via `make toolchain`. After it succeeds you can immediately run the
# full developer loop: `make check`, `make verify`, etc.
#
# Idempotent: every step checks whether the tool is already present and
# skips it, so re-running is cheap and safe.
#
# Tool versions are pinned to match CI (.github/workflows/ci.yml) so that
# "works in CI" and "works on my machine" stay the same thing.

set -euo pipefail

# --- versions (keep in lock-step with .github/workflows/ci.yml) -----------
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CARGO_DENY_VERSION="0.19.6"
CARGO_AUDIT_VERSION="0.22.1"
# Required by flutter_rust_bridge_codegen: it shells out to `cargo expand` to
# evaluate cfg attributes, which is how the `#[cfg(feature = "test-fixtures")]`
# impl block is kept out of the generated bindings.
CARGO_EXPAND_VERSION="1.0.124"
# flutter_rust_bridge codegen tool — must match the runtime crate version
# in crates/hidlins-api/Cargo.toml and the Dart pub package in
# app/pubspec.yaml. See workspace Cargo.toml for the triple-pin comment.
FRB_CODEGEN_VERSION="2.12.0"
# Flutter SDK — pinned in .flutter-version at the repo root.
FLUTTER_VERSION_FILE="$(cd "$(dirname "$0")/../.." && pwd)/.flutter-version"

# --- output helpers --------------------------------------------------------
log()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
ok()   { printf '\033[1;32m  ✓\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33mwarning:\033[0m %s\n' "$*" >&2; }
err()  { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; }
have() { command -v "$1" >/dev/null 2>&1; }

OS="$(uname -s)"

# Install a package using whichever native Linux package manager is present.
# Per-manager package names are passed positionally because they occasionally
# differ (e.g. oathtool vs oath-toolkit). Positional args (no associative
# arrays) keep this working on macOS's bash 3.2 as well.
#   usage: linux_install <label> <apt-pkg> <dnf-pkg> <pacman-pkg> <zypper-pkg>
linux_install() {
  local label="$1" apt_pkg="$2" dnf_pkg="$3" pacman_pkg="$4" zypper_pkg="$5"
  if   have apt-get; then sudo apt-get update && sudo apt-get install -y "$apt_pkg"
  elif have dnf;     then sudo dnf install -y "$dnf_pkg"
  elif have pacman;  then sudo pacman -S --needed --noconfirm "$pacman_pkg"
  elif have zypper;  then sudo zypper install -y "$zypper_pkg"
  else
    err "no supported package manager (apt/dnf/pacman/zypper) found to install ${label}."
    return 1
  fi
}

# --------------------------------------------------------------------------
# 1. Rust toolchain — pinned by rust-toolchain.toml, managed via rustup.
# --------------------------------------------------------------------------
log "Rust toolchain (pinned by rust-toolchain.toml)"
if have rustup; then
  # `rustup show` honours rust-toolchain.toml: it installs the pinned
  # channel, its components (rustfmt, clippy) and declared targets if any
  # are missing, then prints the active toolchain.
  rustup show >/dev/null
  ok "$(rustup show active-toolchain 2>/dev/null || echo 'active toolchain ready')"
else
  err "rustup is not installed — it is the supported way to manage Hidlins's pinned Rust toolchain."
  case "$OS" in
    Darwin) err "Install it with:  brew install rustup-init && rustup-init -y    (or see https://rustup.rs)" ;;
    *)      err "Install it from https://rustup.rs, then re-run 'make toolchain'." ;;
  esac
  exit 1
fi

# --------------------------------------------------------------------------
# 2. keepassxc-cli — required by the interop test harnesses (make interop*).
# --------------------------------------------------------------------------
log "keepassxc-cli (KDBX interop test automation)"
if have keepassxc-cli; then
  ok "$(keepassxc-cli --version 2>/dev/null | head -n1) already installed"
else
  case "$OS" in
    Darwin)
      if have brew; then
        brew install --cask keepassxc
        # The cask ships the CLI inside the app bundle; expose it on PATH
        # exactly as CI does.
        if ! have keepassxc-cli && [ -x /Applications/KeePassXC.app/Contents/MacOS/keepassxc-cli ]; then
          sudo ln -sf /Applications/KeePassXC.app/Contents/MacOS/keepassxc-cli /usr/local/bin/keepassxc-cli
        fi
      else
        err "Homebrew not found. Install it (https://brew.sh) or install KeePassXC manually, then re-run."
        exit 1
      fi
      ;;
    Linux)
      linux_install "keepassxc-cli" keepassxc keepassxc keepassxc keepassxc
      ;;
    *)
      err "unsupported OS '$OS' for automatic keepassxc-cli install."
      exit 1
      ;;
  esac
  have keepassxc-cli && ok "$(keepassxc-cli --version 2>/dev/null | head -n1) installed"
fi

# --------------------------------------------------------------------------
# 3. MinIO Client — required to provision buckets for the S3 live-wire
#    integration suite. MinIO's `minio/stable/mc` formula supports both
#    macOS and Linux and installs the `mc` executable.
# --------------------------------------------------------------------------
log "MinIO Client (S3 integration bucket provisioning)"
is_minio_mc() {
  have mc && mc --version 2>/dev/null | grep -q '^mc version RELEASE\.'
}

if is_minio_mc; then
  ok "$(mc --version 2>/dev/null | head -n1) already installed"
else
  if have mc; then
    err "'mc' is installed but is not MinIO Client (it may be Midnight Commander)."
    err "MinIO's Homebrew formula conflicts with other packages that provide 'mc'."
    exit 1
  fi
  if ! have brew; then
    err "Homebrew not found. Install it from https://brew.sh, then re-run 'make toolchain'."
    err "Hidlins uses 'brew install minio/stable/mc' on both Linux and macOS."
    exit 1
  fi
  brew install minio/stable/mc
  if is_minio_mc; then
    ok "$(mc --version 2>/dev/null | head -n1) installed"
  else
    err "Homebrew completed, but the MinIO 'mc' executable is not available on PATH."
    exit 1
  fi
fi

# --------------------------------------------------------------------------
# 4. cargo-deny / cargo-audit — supply-chain gates (make deny / make audit).
#
# `cargo install` must run OUTSIDE the repo: the project's
# .cargo/config.toml forces offline builds against the vendored tree, which
# does not contain these tools. Running from a scratch dir with
# CARGO_NET_OFFLINE=false (and the pinned toolchain) mirrors CI exactly.
# --------------------------------------------------------------------------
cargo_install_global() {
  local tool="$1" version="$2"
  if have "$tool"; then
    ok "$tool already installed ($("$tool" --version 2>/dev/null | head -n1))"
    return 0
  fi
  log "installing $tool $version (CI-pinned)"
  ( cd "${TMPDIR:-/tmp}" && CARGO_NET_OFFLINE=false RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-}" \
      cargo install --locked "$tool" --version "$version" )
}
cargo_install_global cargo-deny  "$CARGO_DENY_VERSION"
cargo_install_global cargo-audit "$CARGO_AUDIT_VERSION"

# --------------------------------------------------------------------------
# 5. oathtool — OPTIONAL. Used by the entry-management TOTP interop test
#    (make interop-entry) to cross-check generated codes. Best-effort: a
#    failure here does not fail the bootstrap.
# --------------------------------------------------------------------------
log "oathtool (optional — TOTP interop cross-check)"
if have oathtool; then
  ok "oathtool already installed"
else
  case "$OS" in
    Darwin) have brew && brew install oath-toolkit || warn "could not install oath-toolkit (optional); skipping." ;;
    Linux)  linux_install "oathtool" oathtool oathtool oath-toolkit oath-toolkit \
              || warn "could not install oathtool (optional); skipping." ;;
    *)      warn "unsupported OS for oathtool (optional); skipping." ;;
  esac
fi

# --------------------------------------------------------------------------
# 6. Flutter SDK — REQUIRED for app/ development. Not auto-installed;
#    the developer must install it themselves (mirrors the keepassxc-cli
#    pattern for missing-but-recommended tools, except Flutter is too
#    large to auto-install). Warns and continues if missing — Rust-only
#    development works without Flutter: `make check` is Rust-only by design.
#    The Flutter gates live in `make app-check` (and `make verify`).
# --------------------------------------------------------------------------
log "Flutter SDK (pinned by .flutter-version)"
# Shared with CI (which runs it --strict). One parser, one source of truth.
"$SCRIPT_DIR/flutter-version-check.sh" || true

# --------------------------------------------------------------------------
# 7. flutter_rust_bridge codegen tool — generates the Dart↔Rust bindings.
#    Installed via cargo install (same pattern as cargo-deny/audit).
#    Required for `make api-gen`; without it, codegen cannot run.
# --------------------------------------------------------------------------
# Unlike cargo-deny/audit, frb codegen version skew is a correctness
# problem (design §2.6 risk #5 — codegen/runtime/Dart triple must match).
# The generic cargo_install_global only checks binary presence, not version.
# This block also verifies the installed version matches the pin.
cargo_install_global cargo-expand "$CARGO_EXPAND_VERSION"

if have flutter_rust_bridge_codegen; then
  FRB_INSTALLED="$(flutter_rust_bridge_codegen --version 2>/dev/null | sed -n 's/.*[[:space:]]\([0-9][0-9.]*\).*/\1/p' | head -n1)"
  if [ "$FRB_INSTALLED" = "$FRB_CODEGEN_VERSION" ]; then
    ok "flutter_rust_bridge_codegen $FRB_INSTALLED (matches pin)"
  else
    warn "flutter_rust_bridge_codegen version mismatch: installed=$FRB_INSTALLED, expected=$FRB_CODEGEN_VERSION"
    log "Reinstalling flutter_rust_bridge_codegen $FRB_CODEGEN_VERSION (triple-pin requires exact match)"
    ( cd "${TMPDIR:-/tmp}" && CARGO_NET_OFFLINE=false RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-}" \
        cargo install --locked --force flutter_rust_bridge_codegen --version "$FRB_CODEGEN_VERSION" )
  fi
else
  cargo_install_global flutter_rust_bridge_codegen "$FRB_CODEGEN_VERSION"
fi

# --------------------------------------------------------------------------
# 8. CocoaPods — REQUIRED for macOS Flutter builds. Cargokit hooks into
#    the macOS build via a CocoaPods script_phase (the podspec at
#    app/rust_builder/macos/rust_lib_app.podspec). Without `pod`, the
#    macOS build chain in T2.6 cannot produce a .app bundle. Linux
#    builds use CMake (no CocoaPods equivalent needed).
# --------------------------------------------------------------------------
if [ "$OS" = "Darwin" ]; then
  log "CocoaPods (macOS Flutter build dependency)"
  if have pod; then
    ok "CocoaPods $(pod --version 2>/dev/null | head -n1) already installed"
  else
    if have brew; then
      log "Installing CocoaPods via Homebrew"
      brew install cocoapods
      have pod && ok "CocoaPods $(pod --version 2>/dev/null | head -n1) installed"
    else
      err "Homebrew not found. Install CocoaPods manually:"
      err "  brew install cocoapods"
      err "CocoaPods is required for macOS Flutter builds (Cargokit integration)."
    fi
  fi
fi

# --------------------------------------------------------------------------
# 9. Flutter analytics — NFR-013: no telemetry. Disable both Flutter and
#    Dart analytics if Flutter is present. Idempotent and silent if
#    analytics are already disabled.
# --------------------------------------------------------------------------
log "Disabling Flutter/Dart analytics (NFR-013)"
# Asserts the persisted telemetry config rather than parsing `flutter config`
# output — on CI, Flutter substitutes NoOpAnalytics and always reports
# "disabled" regardless of real state, so output-parsing is a tautology there.
"$SCRIPT_DIR/telemetry-check.sh" --write

# --------------------------------------------------------------------------
# Done.
# --------------------------------------------------------------------------

printf '\n'
log "Toolchain ready. Dependencies are vendored + committed, so you can work offline:"
printf '      make check      # Rust gate: fmt-check + lint + build + test\n'
printf '      make app-check  # Flutter gates (needs the Flutter SDK)\n'
printf '      make verify     # full gate incl. docs, supply-chain, and interop\n'
printf '\n'
printf '  Note: only `make vendor` needs network access, and only when ADDING a dependency.\n'
if ! have flutter; then
  printf '\n'
  printf '  Flutter is NOT installed. Install it to work on the Flutter app (app/).\n'
fi
