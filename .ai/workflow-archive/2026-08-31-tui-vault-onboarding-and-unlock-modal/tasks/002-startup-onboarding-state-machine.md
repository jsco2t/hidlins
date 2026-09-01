# Task 002: Startup Onboarding State Machine

Delegation: main-only

## Goal

Make an empty registry a valid TUI startup state and implement the complete
path-selection, authentication, registration, configured-vault, exit/back, and
failure-recovery behavior behind a coherent startup state machine.

## Context

`App::from_registry` currently rejects an empty registry before terminal setup.
The existing unlock prompt already has the required zeroizing secret owner and
correct error classification. This task extends those invariants to a pending
first registration without storing a master password on `App`.

## Scope

### In scope

- Characterization of current configured-vault unlock, retry, cancel, and
  `--vault` behavior before structural changes.
- Empty-registry onboarding phase and path-input event handling.
- Portable standard-library path resolution and validation.
- Pending registration carried through password authentication.
- Successful-authentication-before-registration sequencing using Task 001.
- Direct password startup for one registered vault; branded selector state for
  multiple registered vaults; and an explicit list → selected-password →
  Back/Escape-to-list transition.
- Exit/back, invalid path, wrong password, non-authentication failure, registry
  save failure, lock/re-entry, and state ownership.
- A functional semantic renderer sufficient to keep all gates green; final
  visual/accessibility composition belongs to Task 003.

### Out of scope

- Final ASCII-art layout and snapshots.
- Native file picker or directory browser.
- Keyfile input.
- Creating a new KDBX vault.

## Implementation requirements

- Add fail-before tests for empty-registry construction and the first-vault
  journey before removing `NoVaultsRegistered` behavior.
- Keep the registry owned by `SessionResources::Locked` throughout onboarding.
- Use existing `tui_input::Input`; do not introduce another path widget.
- Trim input, expand only leading `~/`, resolve relative paths from
  `current_dir`, canonicalize, and require an existing regular file. Do not
  invoke a shell or expand arbitrary environment expressions.
- Derive the initial registry name from a non-empty UTF-8 file stem and surface
  collision/unusable-name errors inside the modal.
- Carry a `RegisteredVault` (or an equivalently narrow non-secret pending
  value) with the unlock state. Do not write it before `Vault::open` succeeds.
- Model the three forms explicitly rather than overloading an empty list:
  first-vault path entry, single-vault password, and multiple-vault selection.
- In the multiple-vault form, navigate the registry-order list with both
  Up/Down and `j`/`k`, identify the selection by vault name, and preserve its
  list index solely for focus restoration.
- Enter on a multiple-vault row must transition to a password prompt carrying a
  typed return destination for that picker. Escape/back must drop the current
  `PasswordInput` and restore the same list row. It must not fall through to
  first-run path entry, quit, or silently select another vault.
- On successful open, call Task 001's transaction before moving to Workspace.
  If persistence fails, drop the opened vault, retain no password, and restore
  a recoverable startup state.
- Preserve the existing three-attempt authentication behavior and ensure only
  `AuthenticationFailed` consumes an attempt.
- Extend command contexts/hints so `Enter` and `Ctrl+Q` are discoverable and
  `?` remains typeable in path/password fields.
- Remove or update stale `NoVaultsRegistered` error/docs/tests.
- Add no dependency, feature, unsafe code, or license allowance.

## Acceptance criteria

- [ ] `App::from_registry` returns a valid onboarding app for an empty
      registry and never fabricates an unlocked/session-invalid state.
- [ ] Empty path, missing path, directory, unusable file name, and invalid vault
      paths remain in onboarding with their path text and an actionable error.
- [ ] A valid path transitions to a masked password prompt without writing
      `vaults.toml`.
- [ ] Wrong passwords clear/zeroize the secret buffer, increment only the
      authentication count, retain the pending vault, and leave the registry
      absent/empty.
- [ ] Correct authentication transactionally registers the canonical path and
      enters Workspace only after registry persistence succeeds.
- [ ] A simulated persistence failure drops the opened vault and returns to a
      retryable startup state with no secret leakage or partial success.
- [ ] One registered vault starts directly at its password prompt; two or more
      start in an explicit list form with registry-order rows, a textual current
      selection, Up/Down and `j`/`k` navigation, and Enter transition to the
      selected vault's password prompt.
- [ ] Back or Escape from a multiple-vault password prompt zeroizes its password
      input, restores the picker with the same vault highlighted, and permits a
      different vault to be selected and unlocked.
- [ ] Repeated list/password/back transitions do not mutate `vaults.toml`, move
      authentication attempts between vaults, or unlock a row that changed
      underneath the named selection; `--vault` direct selection remains.
- [ ] `Ctrl+Q` exits from every startup state, while Escape/back behavior never
      traps the user or discards a registered vault. In particular, the
      multiple-vault password form advertises and implements `Esc: Back to vault
      list`.
- [ ] Lock/re-entry returns to the selected registered vault's password flow.
- [ ] Existing unlock/list/auto-lock tests pass after state-machine migration.
- [ ] The dependency fingerprint is unchanged.

## Validation

- `cargo test -p hidlins-tui --offline --locked --lib app::tests`
- `cargo test -p hidlins-tui --offline --locked --lib session::tests`
- `python3 -c 'from pathlib import Path; import hashlib; roots=[p for p in Path(".").rglob("Cargo.toml") if "target" not in p.parts and ".git" not in p.parts and "vendor" not in p.parts]; files=[Path("Cargo.lock"),*sorted(roots),*sorted(p for p in Path("vendor").rglob("*") if p.is_file())]; h=hashlib.sha256(); [(h.update(p.as_posix().encode()),h.update(b"\0"),h.update(p.read_bytes()),h.update(b"\0")) for p in files]; assert h.hexdigest()=="e8305fdc948cc1f58b189a88566c1e5f237c2e3827c4672853ea0f3e1b4e2804", h.hexdigest()'`

## Dependencies

- Task 001

## Expected areas of change

- `crates/hidlins-tui/src/app.rs`
- `crates/hidlins-tui/src/session.rs`
- `crates/hidlins-tui/src/command/registry.rs`
- `crates/hidlins-tui/src/screens/`
- `crates/hidlins-tui/src/error.rs`
- `crates/hidlins-tui/src/lib.rs`

## Risks / notes

The pending vault path is non-secret, but errors must still avoid echoing
arbitrary file contents. Password ownership must remain exactly as narrow as
the current `PasswordInput` → `MasterPassword` flow.
