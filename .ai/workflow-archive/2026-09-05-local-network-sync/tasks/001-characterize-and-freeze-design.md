# Task 001: Characterize Existing Sync and Freeze the Security Design

Delegation: main-only

## Goal

Pin the reusable sync/data-integrity behavior before responsibilities move, and commit an implementation-grade protocol/threat/dependency baseline for all later tasks.

## Context

The existing sync crate mixes valuable transport-neutral merge/orchestration behavior with S3-specific naming and dispatch. The approved DRD also leaves lower-level resource constants to implementation planning. This task prevents later deletion from silently changing no-data-loss behavior and gives security-sensitive tasks one concrete specification.

## Scope

### In scope

- Characterization and invariant tests for the four-state sync truth table, remote-version comparison, CAS retry, KDF/identity validation, `.kdbx.bak`, atomic replacement, loser history, and offline failure behavior.
- Characterization of API/TUI single-owner move-out behavior and current automatic scheduling hooks before they are replaced.
- A checked-in V1 wire/state specification containing the fixed preface, Noise prologues/suites, SAS encoding, message/state table, resource constants, generic remote errors, pairing transaction, discovery metadata policy, and fail-closed rules from `plan.md`.
- A checked-in threat model and an inventory of S3 implementation reachability plus baseline direct/transitive/vendor dependency counts.
- Stable test identifiers and a requirements mapping seed used by later tasks.

### Out of scope

- Adding Noise, discovery, networking, or other dependencies.
- Implementing local sync or deleting S3.
- Changing application-visible sync behavior.

## Implementation requirements

- Tests must exercise public/transport-neutral seams where possible and must not encode S3 wire details as invariants to preserve.
- Record characterization as pre-refactor evidence; do not manufacture a failing test for behavior that already works.
- The protocol document must fix all constants and state transitions listed in `plan.md`, including the distinction between provisional pairing state and usable authorization.
- The threat model must enumerate assets, trust boundaries, attacker capabilities, excluded threats, and controls for discovery spoofing, MITM, replay, exhaustion, concurrent commits, local storage, logs, UI/FFI, and mobile permissions.
- The dependency baseline must be reproducible from committed commands and exclude immutable archives and vendored Dart source noise where appropriate.

## Acceptance criteria

- [ ] Reusable sync, merge, backup, and ownership invariants have passing characterization coverage before refactoring.
- [ ] The V1 protocol/state document contains no placeholder, undecided constant, suite negotiation, or public-address escape hatch.
- [ ] The threat model covers every security boundary named by the DRD.
- [ ] The S3 reachability and dependency baseline identifies every active Rust, Flutter, native, Makefile, CI, harness, documentation, and vendor/dependency removal area.
- [ ] Test identifiers and the initial DRD-to-test matrix are checked in.

## Validation

- `cargo test -p hidlins-sync --offline --locked --test merge_property_tests --test merge_semantics --test fault_injection`
- `cargo test -p hidlins-api --offline --locked --test us_094_sync_api`
- `make test-tui-contracts`

## Dependencies

None

## Expected areas of change

- `crates/hidlins-sync/docs/`
- `crates/hidlins-sync/tests/`
- `crates/hidlins-api/tests/`
- `crates/hidlins-tui/src/*tests*`
- `docs/`

## Risks / notes

The existing state machine uses ETag-shaped field names and a retry default of three. Characterization must distinguish behavior being preserved from naming/count behavior deliberately changed by the approved DRD.
