# Task 003: Temporal Discovery Fallback

Delegation: main-only

## Goal

Ensure every Rust application surface can retain and authenticate candidates discovered across multiple temporal batches, so an early stale or wrong-key route cannot suppress a later valid pinned authority.

## Context

Transport fallback already tries multiple candidates when they are supplied together. The defect occurs earlier: discovery consumers stop polling after the first non-empty batch. Existing tests either publish candidates in one batch or bypass production discovery and hand all routes directly to transport.

## Scope

### In scope

- Shared bounded temporal candidate collection for CLI, API, TUI, and Flutter-facing Rust calls.
- Deduplication, deterministic priority, candidate caps, discovery deadlines, service-kind correctness, and authenticated fallback.
- Deterministic simulated-discovery tests with separate early-wrong and later-valid batches.
- Active discovery/test-matrix claim corrections.

### Out of scope

- Changes to private/local address admission, TXT authenticity, Noise authentication, or identity pinning.
- Bonjour/Avahi dependency replacement.
- iOS native timer generation, owned by Task 005.

## Implementation requirements

- Before product changes, add a production-consumer regression that emits a wrong-key or unreachable candidate in the first batch, a valid pinned authority in a later batch within the same attempt, and fails on the baseline because polling stops early.
- Record red and green results for the identical command in `evidence/003.md`.
- Centralize the polling/collection policy rather than retaining divergent copies in CLI, API, and TUI.
- Accumulate only admissible records, deduplicate stable endpoints, apply the existing deterministic route order, and retain total deadline and candidate-cap bounds.
- Authenticate every candidate; discovery order must never override the pinned authority.
- Ensure pairing and sync query the intended distinct service kind in every production caller.
- Tests must use controllable discovery batches/clock behavior and must not depend on host multicast timing.
- Inspect adjacent cases: empty early batches, duplicate candidates, cached hints, all candidates failing, cap exhaustion, and valid-first fast behavior. Add tests for distinct policy guarantees and avoid redundant transport-only assertions.

## Acceptance criteria

- [ ] The temporal wrong-first/valid-later regression fails against the baseline and passes after repair through a production application seam.
- [ ] CLI, API, TUI, and Flutter-facing Rust discovery use the same bounded collection semantics.
- [ ] A later pinned authority is tried within the attempt budget even when an earlier discovered route fails authentication or connection.
- [ ] Candidate caps, address admission, identity pinning, deterministic ordering, and total time bounds remain enforced.
- [ ] Pairing and sync service kinds are correct at every caller.
- [ ] Adjacent discovery coverage decisions and corrected coverage claims are recorded in task evidence.

## Validation

- `make test-local-sync-discovery`
- `cargo test -p hidlins-sync --offline --locked --test server_integration --test client_policy -- --test-threads=1`
- `cargo test -p hidlins-api --offline --locked --test us_094_sync_api -- --test-threads=1`
- `cargo test -p hidlins-cli --offline --locked --test cli_local_sync_process -- --test-threads=1`
- `cargo test -p hidlins-tui --offline --locked --test local_sync_surface -- --test-threads=1`

## Dependencies

- Task 002

## Expected areas of change

- `crates/hidlins-sync/src/discovery/`
- `crates/hidlins-sync/src/client/`
- `crates/hidlins-sync/tests/secure_discovery.rs`
- `crates/hidlins-sync/tests/server_integration.rs`
- `crates/hidlins-api/src/api/sync.rs`
- `crates/hidlins-cli/src/`
- `crates/hidlins-tui/src/`
- Flutter-facing Rust bridge code under `crates/`
- `crates/hidlins-sync/docs/`

## Risks / notes

Naively waiting for the full deadline after a good first record would regress latency. The implementation must define a bounded settlement/collection policy that preserves later-batch correctness without converting every discovery into the maximum timeout.

