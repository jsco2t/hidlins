# Task 018: Repair Discovery Contracts, Candidate Fallback, and DHCP Resilience

Delegation: main-only

## Goal

Make automatic discovery interoperable on every platform and resilient to spoofed/stale candidates and live authority address changes without weakening pinned identity or local-only routing.

## Context

The Rust authority and native mobile adapters currently use different DNS-SD names. Client selection terminates on the first wrong-key route even though discovery is untrusted. The desktop authority binds and advertises one immutable address, while the DHCP test changes only a simulated client cache.

## Scope

### In scope

- One canonical trusted/pairing service-type contract across Rust, Swift, Android, manifests, and tests.
- Candidate fallback after wrong-key/stale endpoints within existing attempt/time bounds.
- Allowed-interface monitoring and safe listener/advertisement refresh after DHCP/address changes.
- Cross-language static contract tests and deterministic/isolated-network runtime tests.

### Out of scope

- DNS hostnames, public addresses, wildcard serving without per-interface policy, or discovery-based trust.
- Flutter startup sequencing owned by Task 019.
- Consumer-router multicast claims that require physical hardware.

## Implementation requirements

- Platform-specific syntax may omit `.local.` where the native API supplies the domain, but the semantic service names must be `_hidlins-sync._tcp` and `_hidlins-pair._tcp` everywhere.
- Authentication mismatch is remembered but does not prevent trying a later candidate; final error precedence remains stable and secret-free.
- Authority refresh keeps the same Noise identity and port where possible, publishes only successfully bound allowed endpoints, and withdraws obsolete advertisements.
- Interface loss without an allowed replacement stops serving rather than binding broadly.
- Tests must distinguish simulated cache replacement from actual listener/advertiser refresh.

## Acceptance criteria

- [ ] Native iOS and Android discovery resolve services advertised by the Rust authority using the canonical V1 names.
- [ ] Cross-language contract tests fail if any native constant or manifest diverges from the Rust contract.
- [ ] A forged/wrong-key first candidate followed by the valid pinned authority succeeds within the fixed budget.
- [ ] A stale DHCP candidate followed by the authority's replacement address succeeds without changing pinned identity.
- [ ] A running desktop authority rebinds and republishes after an allowed interface address change and withdraws the obsolete route.
- [ ] No refresh/fallback path admits public, multicast, malformed, or unscoped link-local endpoints.

## Validation

- `make test-local-sync-discovery`
- `cargo test -p hidlins-sync --offline --locked --test client_policy -- --test-threads=1`
- `cargo test -p hidlins-sync --offline --locked --test server_integration -- --test-threads=1`
- `make app-test`
- `make check-ios`

## Dependencies

Task 017

## Expected areas of change

- `crates/hidlins-sync/src/discovery/`
- `crates/hidlins-sync/src/client/mod.rs`
- `crates/hidlins-sync/src/server/runtime.rs`
- `app/ios/Runner/HidlinsPlatformServices.swift`
- `app/ios/Runner/Info.plist`
- `app/android/app/src/main/java/app/hidlins/LocalDiscoveryController.java`
- Native/Flutter contract tests and isolated-network harnesses

## Risks / notes

Network-interface APIs are platform-sensitive. Keep enumeration behind the existing adapter and make refresh state testable with injected snapshots; retain a real isolated-network test where the host supports it.
