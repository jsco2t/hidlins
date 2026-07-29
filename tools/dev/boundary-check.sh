#!/usr/bin/env bash
# boundary-check.sh — verify the API boundary is enforced.
#
# Checks (test plan §8.4.3, extended):
#   1. frb config exposes ONLY hidlins-api (rust_input is the load-bearing key)
#   2. app/ Dart sources do not bypass the bridge (dart:ffi, Process, channels,
#      and — in app/lib — dart:io)
#   3. only hidlins-api declares cdylib/staticlib
#   4. NFR-013: no network-capable pub package in the RESOLVED closure
#   5. NFR-013: network-capable crates only in hidlins-sync's tree
#   6. secret-bearing values never reach a string / log / exception message
#      (DTO toString redaction is structural; see dto_redaction_test.dart)
#   7. generated bindings take no WRITE guard on the frb opaque lock
#   8. app/lib/src/bridge/ contains ONLY the known generated files
#   9. the generated bindings expose exactly the approved opaque types
#
# Honest limits: these are grep/manifest gates against accidental drift, not
# an analyzer-grade defense against a determined insider (who could edit this
# script in the same commit). Aliasing defeats check 6's identifier regex;
# an aliased DTO is still covered by the redacted toString injected into the
# generated classes (see app/test/dto_redaction_test.dart), but an aliased
# raw VALUE (a password String, a field read out of a DTO) is covered by
# neither — that residual gap is accepted and documented at check 6.
#
# Design notes on why each check is shaped the way it is are inline.
# Run via `make boundary-check`. Returns 0 on success, 1 on violation.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FAIL=0

# `%s` does not interpret escapes, so multi-line detail must be printed
# separately rather than embedded as a literal "\n" in the summary line.
err() {
  printf '\033[1;31mFAIL:\033[0m %s\n' "$1" >&2
  if [ "$#" -gt 1 ] && [ -n "$2" ]; then
    printf '%s\n' "$2" | sed 's/^/       /' >&2
  fi
  FAIL=1
}
ok() { printf '\033[1;32m  OK:\033[0m %s\n' "$*"; }

require_file() {
  if [ ! -f "$1" ]; then
    err "$2 (missing: ${1#"$REPO_ROOT"/})"
    return 1
  fi
  return 0
}

# ---------------------------------------------------------------------------
# 1. frb config exposes only hidlins-api.
#
# `rust_input` — NOT `rust_root` — determines what codegen scans and exports.
# It is a comma-separated list that can name other crates, so checking
# `rust_root` alone would pass while `rust_input: crate::api,hidlins_core::vault`
# generated Dart bindings straight into the core. Values are matched exactly
# (grep -Fxq) so a commented-out or partial line cannot satisfy the gate.
# ---------------------------------------------------------------------------
FRB_CFG="$REPO_ROOT/flutter_rust_bridge.yaml"
if require_file "$FRB_CFG" "frb config missing"; then
  frb_bad=""
  grep -Fxq 'rust_input: crate::api' "$FRB_CFG" || frb_bad+="expected exactly 'rust_input: crate::api'"$'\n'
  grep -Fxq 'rust_root: crates/hidlins-api' "$FRB_CFG" || frb_bad+="expected exactly 'rust_root: crates/hidlins-api'"$'\n'
  # Check 8's manifest pins this exact directory; if dart_output moved, that
  # check would misreport "bridge directory missing" and the api-gen-check
  # hash gate would go vacuous.
  grep -Fxq 'dart_output: app/lib/src/bridge' "$FRB_CFG" || frb_bad+="expected exactly 'dart_output: app/lib/src/bridge'"$'\n'
  # full_dep: true would pull the dependency tree into the scanned surface.
  if grep -Eq '^[[:space:]]*full_dep:[[:space:]]*true' "$FRB_CFG"; then
    frb_bad+="full_dep must not be true (would scan dependencies)"$'\n'
  fi
  if grep -Eq '^[[:space:]]*third_party_crate_names:' "$FRB_CFG"; then
    frb_bad+="third_party_crate_names broadens the boundary"$'\n'
  fi
  if [ -n "$frb_bad" ]; then
    err "frb config does not restrict the boundary to hidlins-api" "$frb_bad"
  else
    ok "frb config exposes only hidlins-api (rust_input + rust_root pinned)"
  fi
fi

# ---------------------------------------------------------------------------
# 2. Dart must reach Rust only through the generated bridge.
#
# Scans app/lib, app/test, app/test_bridge and app/integration_test (a bypass
# in a test is still a bypass). The generated bridge is excluded via --exclude-dir, not by
# filtering the grep output: `grep -v "src/bridge/"` matched against
# "path:line:content" and so was defeated by a trailing comment mentioning the
# directory. `import package:hidlins_core` is deliberately NOT checked — Dart
# cannot import a Rust crate, so it can never fire; the real bypass vectors are
# below.
# ---------------------------------------------------------------------------
dart_roots=()
for d in lib test test_bridge integration_test; do
  [ -d "$REPO_ROOT/app/$d" ] && dart_roots+=("$REPO_ROOT/app/$d")
done
if [ ${#dart_roots[@]} -eq 0 ]; then
  err "no Dart source roots found under app/ — expected at least app/lib"
else
  ffi_violations=$(grep -rn --include="*.dart" --exclude-dir=bridge \
    -E "import[[:space:]]+'dart:ffi'|DynamicLibrary\.|NativeFunction<" \
    "${dart_roots[@]}" 2>/dev/null || true)
  proc_violations=$(grep -rn --include="*.dart" --exclude-dir=bridge \
    -E "Process\.(run|start|runSync|startSync)|MethodChannel\(" \
    "${dart_roots[@]}" 2>/dev/null || true)
  # dart:io is banned in PRODUCTION code only (app/lib): File/RandomAccessFile
  # bypass the core's atomic-write + advisory-lock discipline, and
  # HttpClient/Socket bypass the NFR-013 "only user-configured sync talks to
  # the network" policy. Platform checks belong to
  # `package:flutter/foundation` (defaultTargetPlatform). Test harnesses
  # (app/test*, driven by flutter_test) legitimately need temp dirs and env
  # vars, so they are exempt; they remain covered by the ffi/proc checks
  # above. Both quote styles and `export` are matched (dart accepts all
  # four, no enabled lint normalizes them), and the capability names are a
  # usage-level backstop for indirect access to the same primitives.
  io_violations=$(grep -rn --include="*.dart" --exclude-dir=bridge \
    -E "(import|export)[[:space:]]+[\"']dart:io[\"']|HttpClient\(|Socket\.|RandomAccessFile" \
    "$REPO_ROOT/app/lib" 2>/dev/null || true)

  # Independent ifs: a PR can introduce several, and an `elif` would hide one.
  [ -n "$ffi_violations" ] && err "Dart uses dart:ffi outside the generated bridge" "$ffi_violations"
  [ -n "$proc_violations" ] && err "Dart shells out or opens a platform channel (bridge bypass)" "$proc_violations"
  [ -n "$io_violations" ] && err "app code imports dart:io (file/socket I/O belongs to the Rust core)" "$io_violations"
  if [ -z "$ffi_violations" ] && [ -z "$proc_violations" ] && [ -z "$io_violations" ]; then
    ok "Dart reaches Rust only through the generated bridge"
  fi
fi

# ---------------------------------------------------------------------------
# 3. Only hidlins-api may be a linkable FFI artifact.
#
# The exclusion is by exact crate directory, not a substring filter: piping
# through `grep -v hidlins-api` would also exempt a future `hidlins-api-extras`.
# ---------------------------------------------------------------------------
shopt -s nullglob
manifests=("$REPO_ROOT/crates/"*/Cargo.toml)
shopt -u nullglob
if [ ${#manifests[@]} -eq 0 ]; then
  err "no crate manifests found under crates/ — check ran against the wrong tree"
else
  cdylib_crates=""
  for m in "${manifests[@]}"; do
    [ "$(dirname "$m")" = "$REPO_ROOT/crates/hidlins-api" ] && continue
    if grep -Eq '^[[:space:]]*crate-type[[:space:]]*=.*(cdylib|staticlib)' "$m"; then
      cdylib_crates+="${m#"$REPO_ROOT"/}"$'\n'
    fi
  done
  if [ -n "$cdylib_crates" ]; then
    err "crates other than hidlins-api declare cdylib/staticlib" "$cdylib_crates"
  else
    ok "only hidlins-api declares cdylib/staticlib"
  fi
fi

# ---------------------------------------------------------------------------
# 4. NFR-013 (Dart): allowlist over the RESOLVED dependency closure.
#
# The previous denylist over pubspec.yaml direct deps could not see that
# file_selector pulled package:http into the production closure transitively.
# An allowlist over pubspec.lock cannot have that blind spot: anything new,
# direct or transitive, must be reviewed and added to the allowlist file in the
# same change.
# ---------------------------------------------------------------------------
LOCK="$REPO_ROOT/app/pubspec.lock"
ALLOW="$REPO_ROOT/app/allowed-packages.txt"
if require_file "$LOCK" "app/pubspec.lock missing" && require_file "$ALLOW" "app/allowed-packages.txt missing"; then
  # Package names are the 2-space-indented keys under `packages:`.
  locked=$(grep -E '^  [a-z0-9_]+:$' "$LOCK" | sed 's/^  //; s/:$//' | sort -u)
  allowed=$(grep -vE '^[[:space:]]*(#|$)' "$ALLOW" | tr -d ' \t' | sort -u)
  unlisted=$(comm -23 <(printf '%s\n' "$locked") <(printf '%s\n' "$allowed"))
  if [ -n "$unlisted" ]; then
    err "pub packages in the resolved closure are not on the reviewed allowlist" "$unlisted"
  else
    ok "every resolved pub package is on the reviewed allowlist"
  fi
fi

# ---------------------------------------------------------------------------
# 5. NFR-013 (Rust): network-capable crates only under hidlins-sync.
#
# Matches both `ureq = ...` and the dotted `ureq.workspace = true` form the
# repo uses elsewhere. POSIX character classes only — \s and \| are GNU
# extensions and would silently fail to match on BSD grep, turning an
# "expect no match" check into an unconditional pass.
# ---------------------------------------------------------------------------
NET_CRATES='ureq|reqwest|hyper|curl|isahc|attohttpc|surf|tonic'
if [ ${#manifests[@]} -gt 0 ]; then
  net_direct=""
  for m in "${manifests[@]}"; do
    [ "$(dirname "$m")" = "$REPO_ROOT/crates/hidlins-sync" ] && continue
    hits=$(grep -En "^[[:space:]]*(${NET_CRATES})(\.workspace)?[[:space:]]*=" "$m" || true)
    [ -n "$hits" ] && net_direct+="${m#"$REPO_ROOT"/}: $hits"$'\n'
  done
  if [ -n "$net_direct" ]; then
    err "crate(s) other than hidlins-sync directly depend on network crates" "$net_direct"
  else
    ok "network-capable crates are direct deps of hidlins-sync only"
  fi
fi

# ---------------------------------------------------------------------------
# 6. Secret-bearing values must never reach a string.
#
# The structural half of this guarantee lives in the generated DTOs: a
# redacted toString() is injected via #[frb(dart_code)] on the Rust enum, so
# interpolating a KeyfileRef prints 'KeyfileRef(***)' (pinned by
# app/test/dto_redaction_test.dart). This grep is the residual defense for
# what no toString override can catch — code that stringifies a secret
# *value* (a password String, a field read out of a DTO) into a log or
# exception message.
# ---------------------------------------------------------------------------
SECRET_IDENTS='keyfile|keyfileRef|masterPassword|password|secretAccessKey|totpUri'
if [ ${#dart_roots[@]} -gt 0 ]; then
  leak=$(grep -rn --include="*.dart" --exclude-dir=bridge \
    -E "(print|debugPrint|log)\([^)]*(${SECRET_IDENTS})|(Exception|Error)\([^)]*(${SECRET_IDENTS})|[\"'][^\"']*\\\$\{?(${SECRET_IDENTS})" \
    "${dart_roots[@]}" 2>/dev/null || true)
  if [ -n "$leak" ]; then
    err "secret-bearing value reaches a string/log/exception in Dart" "$leak"
  else
    ok "no secret-bearing DTO reaches a string, log, or exception message"
  fi
fi

# ---------------------------------------------------------------------------
# 7. No WRITE guard on the frb opaque lock.
#
# frb wraps opaque method receivers in RustAutoOpaqueImplicit, so every bridged
# call takes a guard on its per-object tokio RwLock — and #[frb(sync)] calls
# take it on the Dart UI thread. All-&self methods yield read guards, which
# never contend. A single &mut self method would emit blocking_write(), and
# tokio's RwLock is write-preferring, so a queued writer would stall
# report_activity on the UI thread — the hazard design D-1 rejected. All state
# lives behind Arc<Mutex<SessionState>>, so &mut self is never necessary.
# ---------------------------------------------------------------------------
GEN="$REPO_ROOT/crates/hidlins-api/src/frb_generated.rs"
if require_file "$GEN" "generated bindings missing (run 'make api-gen')"; then
  write_guards=$(grep -n "lockable_decode_sync_ref_mut\|lockable_decode_async_ref_mut\|blocking_write" "$GEN" || true)
  if [ -n "$write_guards" ]; then
    err "generated bindings take a WRITE guard — a bridged method takes &mut self (design D-1)" "$write_guards"
  else
    ok "generated bindings take read guards only (no &mut self on the boundary)"
  fi
fi

# ---------------------------------------------------------------------------
# 8. app/lib/src/bridge/ contains ONLY the known generated files.
#
# Checks 2 and 6 exempt the bridge directory wholesale (--exclude-dir), which
# would otherwise let a hand-written bypass hide beside the generated
# bindings and survive regeneration (api-gen-check hashes before/after its
# own run, so a pre-existing foreign file hashes identically and passes).
# Pinning the directory to an exact manifest makes the exemption safe: the
# only way to add a file here is to update this list in the same reviewed
# change. Update it when api-gen legitimately emits a new module (e.g. the
# T1.4+ API files).
# ---------------------------------------------------------------------------
# Mirrors `dart_output` in flutter_rust_bridge.yaml (pinned by check 1) and
# FRB_DART_OUT in the Makefile.
BRIDGE_DIR="$REPO_ROOT/app/lib/src/bridge"
BRIDGE_MANIFEST="api/session.dart
dto.dart
dto.freezed.dart
error.dart
error.freezed.dart
frb_generated.dart
frb_generated.h
frb_generated.io.dart"
if [ ! -d "$BRIDGE_DIR" ]; then
  err "bridge directory missing (run 'make api-gen')"
else
  # Symlinks are enumerated too: `find -type f` alone would let a symlink
  # (which the Dart compiler happily follows) hide in the exempted
  # directory while the manifest still reported it clean. No entry here is
  # legitimately a symlink, so one showing up fails the diff below.
  bridge_actual=$(cd "$BRIDGE_DIR" && find . \( -type f -o -type l \) | sed 's|^\./||' | LC_ALL=C sort)
  bridge_expected=$(printf '%s\n' "$BRIDGE_MANIFEST" | LC_ALL=C sort)
  bridge_diff=$(diff <(printf '%s\n' "$bridge_expected") <(printf '%s\n' "$bridge_actual") || true)
  if [ -n "$bridge_diff" ]; then
    err "app/lib/src/bridge/ deviates from the generated-file manifest ('<' expected, '>' on disk)" "$bridge_diff"
  else
    ok "bridge directory contains exactly the known generated files"
  fi
fi

# `--exclude-dir=bridge` in checks 2 and 6 matches directory BASE NAMES at
# any depth, so a second directory named `bridge` anywhere under the scan
# roots would silently inherit the exemption without appearing in the
# manifest above. Exactly one such directory may exist.
stray_bridge=$(find "${dart_roots[@]}" -type d -name bridge 2>/dev/null | grep -Fxv "$BRIDGE_DIR" || true)
if [ -n "$stray_bridge" ]; then
  err "directories named 'bridge' outside app/lib/src/bridge inherit the grep exemption" "$stray_bridge"
fi

# ---------------------------------------------------------------------------
# 9. The generated bindings expose exactly the approved opaque types.
#
# rust_input pins the scanned MODULE, not the types in its signatures: frb
# auto-opaques any type it cannot translate, so a signature that leaked
# `hidlins_core::Vault` would silently hand Dart an opaque handle to a core
# type without failing any other gate. Every opaque registration in the
# generated file must be on this allowlist.
# ---------------------------------------------------------------------------
OPAQUE_ALLOWLIST='AppSession'
if [ -f "$GEN" ]; then
  # Anchor on the one-per-type registration macro rather than every
  # `RustAutoOpaqueInner<...>` usage site: usage lines carry unrelated
  # trailing generics that defeat any line regex, while the macro's argument
  # is exactly the registered inner type. Stripping a single trailing `>`
  # keeps nested generics intact (`RustAutoOpaqueInner<Mutex<Vault>>` →
  # `Mutex<Vault>` — a bracket-class grep would not match those at all and
  # would silently escape the allowlist). `|| true` because zero matches
  # must reach the explicit "surface is missing" branch below, not kill the
  # script via pipefail.
  opaque_types=$(grep -A1 'frb_generated_moi_arc_impl_value!' "$GEN" \
    | grep -o 'RustAutoOpaqueInner<.*' \
    | sed 's/RustAutoOpaqueInner<//; s/[[:space:]]*$//; s/);$//; s/>$//' \
    | LC_ALL=C sort -u || true)
  unapproved=""
  while IFS= read -r t; do
    [ -z "$t" ] && continue
    if ! printf '%s\n' "$OPAQUE_ALLOWLIST" | grep -Fxq "$t"; then
      unapproved+="$t"$'\n'
    fi
  done <<< "$opaque_types"
  if [ -n "$unapproved" ]; then
    err "generated bindings expose opaque types outside the approved boundary surface" "$unapproved"
  elif [ -z "$opaque_types" ]; then
    err "no opaque types found in the generated bindings — the AppSession surface is missing"
  else
    ok "opaque surface is exactly: $(printf '%s' "$opaque_types" | tr '\n' ' ')"
  fi
fi

exit $FAIL
