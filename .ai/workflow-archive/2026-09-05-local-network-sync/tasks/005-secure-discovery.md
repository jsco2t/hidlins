# Task 005: Implement Secure Automatic Discovery

Delegation: main-only

## Goal

Provide DHCP-resilient DNS-SD/mDNS discovery and advertisement on desktop plus the bounded native-candidate boundary needed by mobile, without treating discovery as trusted input.

## Context

V1 cannot require users to track IP changes. Desktop can use a narrow safe-Rust mDNS implementation; mobile platforms require OS-native permission/discovery mechanisms that will be connected in Task 011.

## Scope

### In scope

- Exact `mdns-sd = 0.20.3` optional dependency behind `desktop-discovery`, including license/source/build/transitive/alternative review and vendoring.
- Desktop advertisement/browse adapters for trusted and pairing service types.
- Fresh random service-instance labels, metadata-minimal records, interface updates, TTL/removal handling, candidate deduplication/capping, and shutdown.
- A discovery port and bounded candidate DTO for injected native mobile results and permission states.
- Restricted manual address fallback through the Task 003 endpoint parser.
- Simulated discovery environment for deterministic spoof, stale, duplicate, DHCP, multi-interface, and permission tests.
- `test-local-sync-discovery` Makefile target.

### Out of scope

- Swift/Android discovery implementations and user-facing permission prompts.
- Noise pairing/reconnect over real sockets or vault synchronization.

## Implementation requirements

- Execute no newly obtained dependency code until exact licenses and source provenance for the applicable graph are verified.
- Disable optional logging/serde/default features not needed by Hidlins and document the before/after vendor cost.
- Advertise `_hidlins-sync._tcp.local.` normally and `_hidlins-pair._tcp.local.` only during the active pairing window.
- Instance labels are CSPRNG-generated per server start. TXT data is empty unless the API requires the fixed protocol-major marker.
- Filter every advertised/resolved address through the core policy; revalidation at connect/accept remains required later.
- A DHCP/address change updates candidate routing without changing the pinned server identity.

## Acceptance criteria

- [ ] A simulated client rediscovers the same pinned server after its IP changes without configuration edits.
- [ ] Malformed, duplicate, stale, spoofed, excessive, disallowed, and multi-interface records cannot establish trust or escape resource bounds.
- [ ] Advertisements contain no vault name, peer key, stable ID, version, size, or capability metadata.
- [ ] Pairing advertisements exist only inside the explicit three-minute window and all advertisements are withdrawn on stop/lock.
- [ ] Desktop dependency and feature graphs pass license/advisory/vendor review and mobile builds do not link the desktop discovery backend.

## Validation

- `make test-local-sync-discovery`
- `make test-local-sync-security`
- `make check-feature-gates`
- `make deny`
- `make audit`

## Dependencies

- Task 004

## Expected areas of change

- `crates/hidlins-sync/Cargo.toml`
- `crates/hidlins-sync/src/discovery/`
- `crates/hidlins-sync/tests/`
- `Cargo.lock`
- `vendor/`
- `Makefile`
- `CONTRIBUTING.md`

## Risks / notes

`mdns-sd` is routing convenience, not a security primitive. Any platform limitation must surface as an actionable discovery error or the restricted manual fallback; it must never relax address or identity validation.
