# Task 002: Integrate Patched Snow and Secure Noise Sessions

Delegation: main-only

## Goal

Introduce the exact minimal Noise dependency, reproducible zeroization patch, and authenticated secure-session abstraction that later pairing and trusted sync can use without exposing raw Snow state.

## Context

Snow 0.10.0 implements the required Noise patterns but does not meet Hidlins' zeroization-on-drop rule unchanged. This task closes that release blocker before protocol or trust code depends on it.

## Scope

### In scope

- Exact `snow = 0.10.0` dependency with default features off and only the approved primitive/std features.
- Full license, maintenance, audit-history, source, build-script, enabled-feature, transitive, and narrower-alternative evidence before executing/retaining it.
- Cargo lock/vendor refresh and removal of unused feature paths from Snow's resolved/compiled graph where Cargo permits.
- A minimal exact-version Snow zeroization patch integrated into `tools/dev/vendor.py`'s reproducible patch flow.
- Automated source/version/context drift checks for every patched secret-bearing state and transition.
- A narrow Hidlins wrapper for XX/IK construction, fixed preface prologue binding, handshake timeouts, authenticated peer key extraction, transport-mode framing hooks, and zeroizing ownership.
- A `test-local-sync-security` Makefile target established now and expanded by later tasks.

### Out of scope

- Pairing UX, trust persistence, TCP listener/client, discovery, or application messages.
- Any alternate Noise suite or fallback.

## Implementation requirements

- Patch HandshakeState, TransportState, CipherState, SymmetricState, ephemeral/static private key buffers, chaining/hash/key state, and consumed transitions based on an explicit source inventory.
- Avoid logging/debugging secret state; wrapper `Debug` implementations must redact.
- Use test-only instrumentation and exact source checks where post-drop memory observation cannot be made soundly in ordinary safe Rust; explain the evidence boundary rather than relying on undefined behavior.
- The vendor patch must refuse unknown Snow versions or changed source contexts and `make vendor-patches` must be idempotent.
- Add no hand-written cryptographic primitive.
- Capture a failing patch/security test before applying the fix and the passing result afterward.

## Acceptance criteria

- [ ] The compiled Snow graph contains only the approved algorithms/features and passes permissive-license/advisory checks.
- [ ] `make vendor` deterministically recreates the patched source and `make vendor-patches` verifies it offline.
- [ ] Automated guards fail against an unpatched or source-drifted Snow tree and pass against the committed tree.
- [ ] The Hidlins session wrapper can complete fixed XX and IK handshakes, binds the exact preface as prologue, rejects wrong modes/suites/keys, and exposes only authenticated transport plus peer identity.
- [ ] Hidlins-owned and patched Snow secret state has documented zeroization evidence with no secret-bearing `Debug` or error output.

## Validation

- `make vendor-patches`
- `make test-local-sync-security`
- `make deny`
- `make audit`

## Dependencies

- Task 001

## Expected areas of change

- `Cargo.toml`
- `Cargo.lock`
- `vendor/snow-0.10.0/`
- `tools/dev/vendor.py`
- `crates/hidlins-sync/Cargo.toml`
- `crates/hidlins-sync/src/noise/`
- `crates/hidlins-sync/tests/`
- `Makefile`
- `CONTRIBUTING.md`

## Risks / notes

If the required Snow secret inventory cannot be zeroized without altering public cryptographic behavior or expanding the patch beyond a reviewable scope, the workflow must enter `PLAN_CHANGE_REQUIRED`; shipping unpatched is not an option.
