# Task 001: Transactional Vault Registration

Delegation: main-only

## Goal

Provide one core-owned operation that safely adds and persists a vault
registration without losing concurrent registry changes or exposing a false
in-memory success.

## Context

First-run TUI onboarding must save a chosen vault to `vaults.toml`. The current
public sequence is `register` followed by `save`; it operates on a potentially
stale snapshot. The registry module already has the lock, reload, unknown-field
preservation, and atomic-write primitives needed for a narrow transaction.

## Scope

### In scope

- Test-first `VaultRegistry` coverage for transactional insertion.
- A narrow core method that acquires the existing registry write lock, reloads
  the latest registry, inserts one `RegisteredVault`, writes atomically, and
  replaces `self` only after success.
- Duplicate-name and concurrent-unrelated-registration behavior.
- Public documentation for the new operation.

### Out of scope

- TUI state or rendering.
- Changing the registry schema.
- KDBX creation, opening, or serialization.
- Migrating unrelated CLI call sites unless compilation requires it.

## Implementation requirements

- Add failing tests before the production method.
- Reuse `acquire_exclusive`, `VaultRegistry::load`, `register`, and
  `save_unlocked`; do not create a second locking or serialization path.
- Reload only after the exclusive lock is held.
- Preserve every unrelated top-level and per-vault unknown TOML field.
- Do not replace the caller's `VaultRegistry` until the atomic save succeeds.
- Return the existing typed `VaultError` variants.
- Add no dependency, feature, unsafe code, or license allowance.

## Acceptance criteria

- [ ] A missing registry accepts one registration, writes it atomically, and
      reloads with every typed field intact.
- [ ] A stale caller preserves an unrelated registration completed by another
      process/snapshot before this transaction.
- [ ] A duplicate name returns `AlreadyRegistered` and leaves disk and caller
      state unchanged.
- [ ] The caller snapshot is replaced with the newly persisted complete
      registry only after success.
- [ ] Existing `register`, `save`, update, deregister, unknown-field, mode, and
      contention tests remain green.
- [ ] The dependency fingerprint is unchanged.

## Validation

- `cargo test -p hidlins-core --offline --locked registry::tests`
- `python3 -c 'from pathlib import Path; import hashlib; roots=[p for p in Path(".").rglob("Cargo.toml") if "target" not in p.parts and ".git" not in p.parts and "vendor" not in p.parts]; files=[Path("Cargo.lock"),*sorted(roots),*sorted(p for p in Path("vendor").rglob("*") if p.is_file())]; h=hashlib.sha256(); [(h.update(p.as_posix().encode()),h.update(b"\0"),h.update(p.read_bytes()),h.update(b"\0")) for p in files]; assert h.hexdigest()=="e8305fdc948cc1f58b189a88566c1e5f237c2e3827c4672853ea0f3e1b4e2804", h.hexdigest()'`

## Dependencies

None

## Expected areas of change

- `crates/hidlins-core/src/registry.rs`

## Risks / notes

The helper must not reacquire the same advisory lock through public `save`, and
must not assign the freshly loaded snapshot into `self` before persistence
succeeds.

