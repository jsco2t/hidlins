# Task 003: Implement Address Policy, Framing, and the V1 Protocol

Delegation: main-only

## Goal

Implement the single authoritative local-address boundary and bounded binary protocol/state machine beneath all client, discovery, and server paths.

## Context

Local-only reachability and parser/resource safety must be enforced centrally rather than repeated in UI or transport adapters. The V1 specification and constants were frozen in Task 001.

## Scope

### In scope

- Canonical scoped endpoint types for IPv4, IPv6, port, and IPv6 interface scope.
- Exhaustive allowlist classification and rejection of mapped/special/global/multicast/unspecified/bypass cases.
- Manual literal address parsing with no DNS names.
- Fixed 12-byte preface codec and Noise frame length handling.
- Bounded application message codec and strict message-order/state validator for version, fetch, conditional upload/chunks/commit, cancel, pairing commit/ack, and generic errors.
- Fixed 32-byte `RemoteVersion` representation.
- Property tables/generators, structured byte-seeded fuzz smoke tests, malformed corpus seeds, and boundary tests.

### Out of scope

- Opening sockets, discovery, cryptographic handshake execution, trust persistence, or vault writes.
- UI-level error wording.

## Implementation requirements

- Use safe Rust and standard-library byte parsing; do not add a serializer, DNS resolver, URL parser, or network crate.
- Reject trailing bytes, noncanonical encodings, zero/oversized frames, invalid reserved bits, invalid message order, allocation claims over limits, and invalid scope IDs before allocation or state mutation.
- Normalize IPv4-mapped IPv6 before allowlist evaluation.
- Permit only the exact DRD address ranges and mDNS multicast solely in a distinct discovery-destination API that cannot become a sync endpoint.
- Use checked arithmetic and bounded buffers/queues throughout.
- Add red-before/green-after evidence for each parser/policy class.

## Acceptance criteria

- [ ] The exact allowed address table passes and representative/full-prefix property tests reject every other special/global class, including CGNAT and mapped bypasses.
- [ ] IPv6 link-local endpoints require and preserve a valid nonzero interface scope.
- [ ] Frame/message codecs round-trip canonical inputs and reject malformed, truncated, duplicate, reordered, oversized, and trailing data without panic or excessive allocation.
- [ ] Protocol state tests prove unauthenticated/pre-authorized states cannot issue vault operations.
- [ ] All fixed limits from `plan.md` have below/equal/above tests and secret-free errors.

## Validation

- `cargo test -p hidlins-sync --offline --locked address`
- `cargo test -p hidlins-sync --offline --locked protocol`
- `make test-local-sync-security`

## Dependencies

- Task 002

## Expected areas of change

- `crates/hidlins-sync/src/address.rs`
- `crates/hidlins-sync/src/framing.rs`
- `crates/hidlins-sync/src/protocol.rs`
- `crates/hidlins-sync/src/transport/`
- `crates/hidlins-sync/tests/`
- `crates/hidlins-sync/docs/`

## Risks / notes

Address classification must use actual socket endpoints later as well as configured/discovered candidates; this task supplies the non-bypassable type/function, but Tasks 005–007 wire every boundary.
