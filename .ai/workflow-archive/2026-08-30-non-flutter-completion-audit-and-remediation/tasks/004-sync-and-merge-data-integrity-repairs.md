# Task 004: Sync and Merge Data-Integrity Repairs

Delegation: main-only

## Goal

Repair and test all Task-001 findings in S3 transport, credentials, orchestration, merge semantics, backups, and no-data-loss behavior.

## Context

Sync is the highest-risk subsystem. Two notebook bugs appear already fixed in current code, but merge history entries still lose attachment bytes. This task closes the root defect rather than relabeling a backup workaround as success.

## Scope

### In scope

- SigV4, HTTP, endpoints, ETags, credentials, config, transport abstraction, orchestration, retry/concurrency, backups, and merge.
- Verification of both-side attachment propagation and RST-CRED-1 rekey behavior.
- Root-cause repair for history-version attachment preservation.
- Fault, property, collision, KDBX save/reopen, and KeePassXC merge interop coverage.

### Out of scope

- New transports or clock-anomaly warnings.
- CLI/TUI rendering of outcomes except changes required by a corrected shared contract.

## Implementation requirements

- Fix every ledger finding assigned to Task 004.
- Add a failing regression/property/interop test before each behavior fix and capture the failure; establish characterization/invariant tests before architectural movement; capture green results after repair.
- Fix every ≥80-confidence architecture finding assigned to Task 004, keeping transport, merge, credential, and orchestration responsibilities explicit and testable.
- Preserve loser-as-history, UUID identity, conditional-write safety, pre-merge recovery, and no plaintext credential storage.
- Historical versions with attachments must reference valid bytes in the merged local pool after save/reopen.
- If keepass public API is insufficient, use the narrow audited dependency approach authorized by `plan.md`; do not use unsafe layout access or plaintext intermediate serialization.
- Follow `.ai/workflow/dependency-review.md`; license verification precedes any dependency source use, and a local implementation may replace only small non-cryptographic glue—not crypto, TLS, SigV4 primitives already intentionally owned, or KDBX machinery without an approved plan change.
- Keep/add rolling encrypted backups only as defense in depth, not as the acceptance proof.

## Acceptance criteria

- [ ] Every Task-004 ledger finding is fixed with regression evidence.
- [ ] Every behavior repair has fail-before/pass-after evidence and every architecture refactor has characterization/invariant coverage.
- [ ] Attachment add/replace/remove converges for current values and for loser history entries.
- [ ] History attachments survive KDBX save/reopen and are KeePassXC-compatible.
- [ ] RST-CRED-1 credentials remain usable after a master-password change.
- [ ] Merge properties, failure atomicity, and collision exit behavior remain correct.

## Validation

- `cargo test -p hidlins-sync --offline --locked`
- `make test-sigv4`
- `make test-merge-properties`
- `make interop-sync`

## Dependencies

- Task 003

## Expected areas of change

- `crates/hidlins-sync/`
- `crates/hidlins-core/`
- `vendor/keepass/` or a narrowly scoped replacement source if required
- `Cargo.toml`
- `Cargo.lock`
- `deny.toml`
- `.cargo/audit.toml`
- `tools/interop-tests/`
- `Makefile` if a new recurring validation command is introduced

## Risks / notes

This task is data-integrity-sensitive and cannot be delegated. Any dependency patch must remain reproducible under offline builds and must be visibly documented rather than hidden in generated vendor state.
