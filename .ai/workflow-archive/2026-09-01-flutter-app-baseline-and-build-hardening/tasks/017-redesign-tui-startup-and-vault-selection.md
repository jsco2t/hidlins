# Task 017: Redesign TUI startup and vault selection

Delegation: main-only

## Goal

Make TUI startup input and selection immediately understandable, install the
requested login logo, and ensure every configured-vault flow can reach a
chooser that can also add an existing vault.

## Context

The startup modal currently renders input as plain adjacent text under the
nonstandard phrase `Focused field`. A one-vault registry bypasses the chooser,
Escape from its direct password prompt quits, and the chooser cannot start the
existing authenticate-before-register onboarding flow. The lock screen also
promises a vault list but returns directly to a one-vault password prompt.

## Scope

### In scope

- Bordered/marked active input controls in normal layouts and a clear compact
  fallback.
- Exact replacement of the startup decorative logo.
- A chooser destination from direct prompts and lock screens.
- A discoverable chooser command for adding an existing KDBX through the
  current authenticate-before-register flow.
- Explicit first-run versus chooser onboarding provenance and correct Escape,
  error-recovery, and selection restoration.
- State-machine, journey, accessibility, geometry, and snapshot regressions.

### Out of scope

- Switching an unlocked vault without passing through the existing lock path.
- Registry deregistration, rename, path-alias prohibition, or keyfile onboarding
  changes.
- TUI entry/workspace redesign or CLI behavior.
- Any new dependency.

## Implementation requirements

1. Before implementation, add the smallest regressions for the direct-prompt
   chooser path, add-existing action, and active-field affordance; run them
   against the current implementation and record the red results.
2. Replace `HIDLINS_STARTUP_ART` byte-for-byte with the exact 12-line logo in
   the round request. Update layout constants for its 23-cell width and measure
   geometry with the existing Unicode display-width support, not `str::len`.
3. In normal mode, give vault-path and password inputs their own visible borders,
   labels, active-focus styling, and terminal cursor. In compact/accessibility
   mode, use an explicit textual active-input marker if the bordered form cannot
   fit. Remove every rendered `Focused field` phrase.
4. Preserve password masking, zeroize-on-drop behavior, control-character
   filtering, status/error visibility, and the supported 40×12 compact floor.
5. Give vault selection a color-independent textual marker in addition to the
   selected style. Show complete, accurate chooser actions including selection,
   `a: Add existing vault`, and `Ctrl+Q: Exit`.
6. Add a distinct centrally registered `Add existing vault` command in the
   unlock-list context, with default `a`, so key dispatch, help/palette, and
   discoverability derive from the same command registry.
7. Preserve explicit onboarding provenance. Zero registered vaults start in
   first-run onboarding, while chooser-initiated onboarding returns to the same
   chooser row on Escape or recoverable cancellation. Escaping its password
   prompt returns to its path field without retaining secret input.
8. Cancelling any configured-vault password prompt must return to the chooser
   with its vault highlighted. It must not quit or fabricate a registration.
9. Any key on the lock screen must return to the chooser for a non-empty
   registry with the previously unlocked vault highlighted. Keep `Ctrl+Q` as
   the explicit exit and preserve sync-on-lock behavior.
10. Reuse `resolve_existing_vault_path`, `Vault::open`, and
    `VaultRegistry::register_and_save`. Authentication must precede persistence;
    failed/cancelled additions must not mutate the registry or KDBX.
11. Update all affected journeys, semantic contracts, key registry checks, and
    reviewed startup snapshots. Add supported-size and compact-layout coverage
    for the new art and fields.
12. Record fail-before/pass-after evidence for each acceptance gap.

## Acceptance criteria

- [ ] Normal startup forms visibly separate, label, style, and mark the active
      input; compact forms retain an explicit active marker; `Focused field` is
      absent.
- [ ] The exact requested 12-row logo renders without clipping at 60×16 and
      80×24, with a safe no-art compact/accessibility fallback.
- [ ] Escape from a configured-vault password prompt reaches the chooser and
      highlights that vault instead of quitting.
- [ ] The chooser exposes and executes `a: Add existing vault` through the
      central command registry and existing authenticated registration path.
- [ ] Escape and error recovery return to the correct first-run or chooser
      origin without registry mutation or stale password input.
- [ ] Manual and automatic lock return to the chooser with the prior vault
      highlighted, allowing another registered vault to be selected.
- [ ] Existing unlock, retry, registration concurrency, sync-on-lock, compact
      layout, and secret-concealment behavior remains green.
- [ ] No external dependency is added.

## Validation

- `cargo test -p hidlins-tui --offline --locked --lib`
- `make test-tui-contracts`
- `git diff --check`

## Dependencies

Task 016

## Expected areas of change

- `crates/hidlins-tui/src/app.rs`
- `crates/hidlins-tui/src/command/registry.rs`
- `crates/hidlins-tui/src/command/keymap.rs`
- `crates/hidlins-tui/src/screens/startup_modal.rs`
- `crates/hidlins-tui/src/screens/lock_screen.rs`
- `crates/hidlins-tui/src/widgets/password_input.rs`
- `crates/hidlins-tui/src/journey_tests.rs`
- `crates/hidlins-tui/src/accessibility_contract_tests.rs`
- `crates/hidlins-tui/src/snapshot_tests.rs`
- `crates/hidlins-tui/tests/snapshots/startup_*.txt`

## Risks / notes

- This task changes a security-sensitive state machine and remains main-only.
- Any visual cursor for the password field may reveal only position/length; it
  must never render or log the password.
- Snapshot regeneration is allowed only through `make test-update-snapshots`,
  followed by review of the actual text diff.
