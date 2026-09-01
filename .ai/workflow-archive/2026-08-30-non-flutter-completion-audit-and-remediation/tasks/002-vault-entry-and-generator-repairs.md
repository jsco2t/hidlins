# Task 002: Vault, Entry, and Generator Repairs

Delegation: main-only

## Goal

Repair and test all Task-001 findings owned by vault lifecycle/storage, entry management, KDBX serialization, and password/passphrase generation.

## Context

These modules hold the core data and secret-generation contracts consumed by every surface. Changes are security- and data-integrity-sensitive.

## Scope

### In scope

- `hidlins-core` vault, registry, path, locking, atomic-write, secret, entry, TOTP, attachment, history, and search behavior.
- `hidlins-genpw` random and diceware generation.
- Core KDBX/keepass dependency findings, including yanked/advisory drift where a safe compatible repair belongs here.
- Regression, fault, property, and KeePassXC interop tests for fixed defects.

### Out of scope

- Sync orchestration/merge behavior owned by Task 004.
- UI behavior.

## Implementation requirements

- Fix every ledger finding assigned to Task 002 without changing product scope.
- For each behavior defect, add or strengthen the smallest regression test first, record its failure, then implement the repair and record the pass. For architecture refactors, establish characterization/invariant coverage before moving responsibilities.
- Fix every ≥80-confidence architecture finding assigned to Task 002; prefer narrow owned APIs over leaking dependency internals, while avoiding speculative abstraction.
- Preserve KDBX3-read/KDBX4-write interoperability, UUID stability, history semantics, atomic replacement, and lock stability.
- Preserve zeroization and best-effort master-key locking.
- Any keepass/dependency change must be exact-pinned, vendored, audited, and exercised by all interop tests.
- Follow the pre-change decision in `.ai/workflow/dependency-review.md`; verify license before use and prefer safe removal/in-repository implementation only for small non-cryptographic functionality.

## Acceptance criteria

- [ ] Every Task-002 ledger finding is marked fixed with code and regression-test evidence.
- [ ] Evidence identifies each defect's fail-before and pass-after test result, and each structural refactor's pre-existing characterization/invariant protection.
- [ ] Core vault/entry behavior round-trips through save/reopen and KeePassXC.
- [ ] Generator outputs retain CSPRNG, class, length, exclusion, and entropy contracts.
- [ ] No new plaintext-disk, secret-debug, lock, or atomic-write regression is introduced.

## Validation

- `cargo test -p hidlins-core --offline --locked`
- `cargo test -p hidlins-genpw --offline --locked`
- `make interop`
- `make interop-entry`

## Dependencies

- Task 001

## Expected areas of change

- `crates/hidlins-core/`
- `crates/hidlins-genpw/`
- `Cargo.toml`
- `Cargo.lock`
- `vendor/`
- `deny.toml`
- `.cargo/audit.toml`

## Risks / notes

Any KDBX dependency change is high risk and requires full interop and merge regression even when the originating finding appears local.
