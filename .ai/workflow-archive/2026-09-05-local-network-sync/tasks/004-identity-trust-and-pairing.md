# Task 004: Implement Identity, Trust, Pairing, and Revocation

Delegation: main-only

## Goal

Deliver per-vault/per-installation identity storage and the complete XX/SAS/provisional-commit/IK trust state machine without exposing vault data to incomplete or revoked relationships.

## Context

Discovery cannot be an identity system. Trust must be created only through explicit human-confirmed XX pairing and used only through pinned IK reconnects. Private installation keys must meet Hidlins' at-rest and zeroization rules.

## Scope

### In scope

- Generic versioned sealed-secret container for installation private keys using the existing Argon2id/ChaCha20-Poly1305 technique with new domain-separated associated data.
- V1 local-sync registry schema for server/client roles, sealed identity, public identity, peer records, provisional pairing transactions, revocation, routing hints, and transport-neutral sync pointers.
- Static X25519 identity generation, decrypt/use/drop handling, and master-password rewrap.
- Six-character Crockford Base32 SAS derivation from the first 30 handshake-hash bits.
- Explicit pairing window, single candidate, three-failure limit, local bilateral confirmation, authenticated commit/ack transaction, provisional expiry/recovery, and no-data-access invariant.
- Pinned mutual IK authorization, peer listing/naming, explicit re-designation, and revocation.

### Out of scope

- DNS-SD/mDNS, real TCP sockets, vault fetch/commit, or UI flows.
- Parsing or translating S3 configuration.

## Implementation requirements

- Private key, derived sealing key, decrypted container, and session confirmation material must be zeroizing types and have redacted `Debug`/errors.
- Registry writes use `update_registered_extra`/atomic persistence and preserve unrelated concurrent fields.
- An unknown/non-local sync kind is treated as not configured; do not retain a S3 enum variant or legacy parser in the new schema.
- Provisional state cannot authorize version/fetch/upload; it is bounded, expires, and can only complete/recover the matching authenticated transaction.
- Re-designation is explicit and requires fresh pairing; discovery alone cannot alter pinned authority.
- Fault-inject every persistence and commit/ack boundary and capture red/green evidence.

## Acceptance criteria

- [ ] Every generated vault/installation relationship has a distinct identity and only sealed private bytes reach disk.
- [ ] Password change rewraps the same identity; wrong password, tampering, wrong vault context, or wrong associated data fails closed without changing the registry.
- [ ] Both endpoints derive the same documented SAS; transcript/MITM changes derive different SAS values.
- [ ] Timeout, third failure, rejection, mismatch, disconnect, malformed commit, wrong transaction/key, and failed acknowledgement create no usable authorization or vault access.
- [ ] Completed IK accepts only the exact pinned and non-revoked mutual identities; revocation and silent re-pair/downgrade attempts fail.
- [ ] Multi-client records remain isolated and atomic under concurrent registry updates.

## Validation

- `cargo test -p hidlins-sync --offline --locked identity`
- `cargo test -p hidlins-sync --offline --locked pairing`
- `cargo test -p hidlins-sync --offline --locked trust`
- `make test-local-sync-security`

## Dependencies

- Task 003

## Expected areas of change

- `crates/hidlins-sync/src/config.rs`
- `crates/hidlins-sync/src/identity.rs`
- `crates/hidlins-sync/src/trust.rs`
- `crates/hidlins-sync/src/pairing.rs`
- `crates/hidlins-sync/src/error.rs`
- `crates/hidlins-sync/tests/`

## Risks / notes

Distributed storage cannot provide literal cross-device atomic disk writes. The acceptance invariant is stronger and testable: incomplete/provisional state never grants data authorization, expires, and can be safely recovered or discarded without silent trust.
