# Task 016: Repair Live Authorization, Pairing Admission, and Connection Accounting

Delegation: main-only

## Goal

Make live server authorization, pairing-window enforcement, and connection accounting satisfy the already-approved security policy under concurrency and revocation.

## Context

Round 1 persists revocation and provides a correct isolated `PairingWindow`, but the network runtime authorizes a peer only once, does not connect pairing admission to that state machine, and can decrement an unreserved connection count after rejecting a source address.

## Scope

### In scope

- Peer-identified live connection and queued-operation ownership.
- Immediate live-session/queued-work invalidation when trusted keys are removed.
- Authorization recheck at request and commit boundaries.
- Runtime integration of the one-candidate, three-failure, three-minute `PairingWindow`.
- Cancellation of active pairing candidates on close, expiry, or failure exhaustion.
- Underflow-proof connection-slot accounting.
- Real-socket and deterministic concurrency regression tests.

### Out of scope

- Changing Noise patterns, trust schema, sync truth table, or address allowlist.
- Deadline/framing and fetch-streaming changes owned by Task 017.
- UI redesign.

## Implementation requirements

- A revoked key must lose authorization for current sockets and host operations, not only future handshakes.
- Host commits must carry a revocable authorization token/generation so work dequeued before revocation cannot commit afterward.
- Pairing failures include failed XX attempts, explicit rejection/mismatch, disconnects after admission, and invalid authenticated pairing sequences as defined by the existing protocol policy.
- Pairing closure must withdraw its advertisement and wake/cancel an admitted authority prompt without stopping ordinary trusted service.
- Connection count changes must be represented by an owned guard created only after a successful reservation.
- Tests must record red-before and green-after results for every repaired behavior.

## Acceptance criteria

- [ ] A peer revoked during an authenticated session cannot read, submit another request, or commit an in-flight upload.
- [ ] Replacing trust closes removed peers while leaving still-trusted peers usable.
- [ ] Only one SAS candidate can be active; a fourth failed attempt is never admitted and failure exhaustion closes pairing.
- [ ] Closing or expiring pairing cancels an already-admitted candidate and withdraws only pairing advertisement.
- [ ] Rejected inbound addresses and connection-cap rejection cannot underflow/leak capacity; a later valid client is admitted.
- [ ] Regression evidence includes fail-before/pass-after results at the live runtime boundary.

## Validation

- `cargo test -p hidlins-sync --offline --locked --test server_integration -- --test-threads=1`
- `cargo test -p hidlins-sync --offline --locked --test identity_pairing_trust`
- `make test-local-sync-security`
- `make test-local-sync-integration`

## Dependencies

Task 015

## Expected areas of change

- `crates/hidlins-sync/src/server/runtime.rs`
- `crates/hidlins-sync/src/server/mod.rs`
- `crates/hidlins-sync/src/pairing.rs`
- `crates/hidlins-sync/tests/server_integration.rs`
- `crates/hidlins-sync/tests/identity_pairing_trust.rs`
- API/CLI/TUI trust-replacement callers as required by the peer-aware runtime contract

## Risks / notes

Revocation must not depend solely on closing a socket: a host operation may already be dequeued. The commit boundary is the final authorization authority. Pairing failure accounting must avoid double-counting one failed candidate while still counting pre-SAS handshake abuse.
