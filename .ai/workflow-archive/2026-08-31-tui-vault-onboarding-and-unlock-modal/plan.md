# TUI Vault Onboarding and Unlock Modal

## Objective

Make the TUI own the first-run experience instead of exiting when
`vaults.toml` is absent or empty. The alternate-screen TUI will present one
consistent branded startup modal that:

- lets a first-run user enter or paste the path of an existing KDBX vault;
- authenticates the selected vault and saves it to the canonical
  `vaults.toml` registry;
- asks for the master password immediately when exactly one registered vault
  exists;
- presents a third, accessible vault-list form when multiple vaults are
  registered, transitions within the same modal to the selected vault's
  password form, and returns to the same highlighted list on Back or Escape;
- always exposes an explicit keyboard exit action; and
- uses the request's exact ASCII art in the normal spacious presentation.

The request's final sentence is interpreted as “when a vault is already in
settings, prompt for its password **or exit**.” It is not interpreted as
requiring a password in order to exit.

## Current behavior

`launch_tui` builds `App` before creating `Terminal` and entering the event
loop (`crates/hidlins-tui/src/lib.rs:83-93`). `App::from_registry` returns
`TuiError::NoVaultsRegistered` for an empty registry
(`crates/hidlins-tui/src/app.rs:599-610`). Consequently, first-run users see a
one-line error on the ordinary terminal and cannot recover inside the TUI.

When vaults exist, the current pre-unlock experience is split across a
full-screen list and a separate full-screen password prompt. A single
registered vault still requires an otherwise-redundant selection step. The
password widget already provides the correct secret lifetime: a
`Zeroizing<String>` buffer, masked rendering, and consume-on-submit semantics
(`crates/hidlins-tui/src/widgets/password_input.rs:29-123`).

The core registry already preserves unknown fields and writes atomically, but
the public add path is a separate `register` then `save` sequence. The new TUI
flow must not enter the workspace unless both KDBX authentication and registry
persistence succeed.

The worktree contains the completed prior package as uncommitted user-owned
changes. They will be preserved. This package's dependency baseline is the
aggregate fingerprint
`e8305fdc948cc1f58b189a88566c1e5f237c2e3827c4672853ea0f3e1b4e2804`
over all non-vendored workspace `Cargo.toml` files, `Cargo.lock`, and every
regular file under `vendor/`.

## UX and accessibility review

### Experience reconstruction

The surface is a keyboard-first Ratatui TUI on Linux and macOS, with a minimum
reviewed geometry of 60×16 and an existing 40×12 compact-state precedent.
Screen readers consume terminal cells in row-major reading order; Ratatui has
no semantic mechanism for marking decorative cells as hidden. `?` normally
opens help, but password input deliberately captures it as a printable
character. Mouse support is an accelerator and cannot be the only path.

### Critical finding

**Empty-registry startup is a dead end** — confidence 100/100. The pre-terminal
`NoVaultsRegistered` return prevents the interface from opening and provides no
in-app recovery. This blocks all first-run users and directly contradicts the
requested flow. The repair is the empty-registry onboarding state owned by
Task 002.

### Important findings

1. **Literal ASCII art becomes speech noise if always rendered** — confidence
   95/100. Terminal cells cannot label the art as decorative. The exact art
   will therefore render byte-for-byte in normal themes when geometry permits,
   while the explicit `accessible` theme uses a concise `HIDLINS` text heading.
   Compact geometry also uses the semantic heading instead of clipping the art.
2. **Bracketed actions alone do not communicate focus or keys** — confidence
   95/100. Actions will have stable text labels and explicit key carriers such
   as `Enter: Open vault`, `Enter: Unlock`, and `Ctrl+Q: Exit`; focus/selection
   also has a textual marker and never depends on color.
3. **A failed path or password must be recoverable without losing context** —
   confidence 95/100. Path validation keeps the entered path and posts an
   actionable error. Authentication clears only the zeroizing password buffer,
   counts only genuine authentication failures, and lets the user return to
   path/vault selection. Non-authentication failures are not mislabeled as bad
   passwords.
4. **The supplied art cannot fit every terminal** — confidence 95/100. The
   renderer will use a tested breakpoint and a compact semantic layout rather
   than crop the logo, controls, errors, or exit hint.

### Review limitations

This is a code-based reconstruction; no real terminal emulator or assistive
technology is executed during planning. Implementation will extend the
existing in-process Ratatui journey and accessibility-contract tests. It will
not claim that those tests prove AT-SPI or speech-engine integration.

## Proposed implementation

1. Add a narrow core `VaultRegistry` transaction that reloads the newest
   registry under its existing advisory write lock, inserts one registration,
   atomically writes it, and replaces the caller's snapshot only after success.
   This prevents first-run onboarding from losing a concurrent registry change
   or retaining a false in-memory success.
2. Add an explicit empty-registry onboarding phase holding a normal
   `tui_input::Input` path buffer and a pending-registration source on the
   unlock prompt. Empty registries become a valid locked session.
3. Resolve path input with standard-library facilities only: trim it, expand a
   leading `~/`, resolve relative paths from the process working directory,
   canonicalize it, require an existing regular file, and derive the initial
   registry name from its UTF-8 file stem. Do not add a graphical picker, shell
   expansion engine, or dependency.
4. Open the selected KDBX with the existing zeroizing password flow. A new
   registration is persisted only after successful authentication. This
   sequencing avoids permanently registering a missing, corrupt, or
   incorrectly selected file. If persistence fails, the opened vault is
   dropped and the user remains in the startup flow with a secret-free,
   actionable error; the workspace never opens with an unsaved selection.
5. Render onboarding, registered-vault selection, and password entry inside a
   shared centered startup shell. Exactly one registered vault opens directly
   at password entry. Multiple registrations open a dedicated list form whose
   current row is carried by a visible text marker; Up/Down and `j`/`k` move,
   and Enter transitions the same shell to the selected vault's password form.
   That password form exposes `Esc: Back to vault list`; Back/Escape zeroizes
   the password input and restores the list with the same vault highlighted.
   `--vault NAME` retains its direct selection behavior.
6. Render the exact requested art from one literal constant in normal themes
   at supported geometry. Use the compact semantic heading for the
   `accessible` theme and undersized terminals. Keep prompt labels, current
   selection, errors, retry counts, and Open/Unlock/Exit keys present as text.
7. Extend the existing in-process TestBackend journeys, semantic buffer
   contracts, and hand-rolled snapshot system. No PTY, GUI picker, snapshot
   library, or other dependency is needed.

## Architectural decisions

- `VaultRegistry` remains the source of truth for saved vault selections.
  “Saved in config” means an atomic record in
  `$HOME/.local/state/hidlins/vaults.toml`, not a duplicate path in the TUI's
  preference-only `config.toml` or `tui.toml`.
- Core owns concurrent/atomic registry mutation; TUI owns path-entry state,
  presentation, and orchestration of existing `Vault::open` plus registry APIs.
- The TUI does not store a master password on `App`. Pending onboarding data
  may contain a non-secret canonical path and generated registry name, while
  the password remains solely in `PasswordInput`/`MasterPassword` zeroizing
  owners.
- Successful authentication precedes first registration. “Select a vault” is
  complete when the chosen file has authenticated and been atomically recorded;
  merely typing a path does not mutate settings.
- The existing registry insertion order remains the multiple-vault order.
  This package does not introduce a separate default-vault preference.
- Startup is one shell with three explicit form states: first-vault path,
  single-vault password, and multiple-vault picker/password. The picker index
  is preserved across its password transition; it is not inferred again from
  a mutable registry row after the transition.
- Password prompts carry an explicit return destination. A pending first-vault
  prompt returns to its path form; a multiple-vault prompt returns to its
  picker; a single-vault or explicit `--vault` prompt has no fabricated list
  predecessor. Every return drops/zeroizes the current password input.
- The startup interface is keyboard-complete. Mouse behavior, if retained as
  an accelerator, routes through the same actions and is never required.
- Exact art fidelity applies to the normal, sufficiently large visual layout.
  Omitting decorative art in the explicit accessible mode and compact fallback
  is an intentional accessibility adaptation, not a different logo.
- No manifest, lockfile, or vendored-source change is permitted. Existing
  `ratatui`, `crossterm`, `tui-input`, core registry, and test helpers cover the
  complete feature.

## Work included

- Empty-registry startup within the alternate-screen TUI.
- Safe first-vault path entry, validation, authentication, and registration.
- Direct password startup for one registered vault and branded selection for
  multiple registered vaults, including list-to-password transition and
  Back/Escape-to-the-same-list behavior.
- Exit, cancel/back, validation failure, wrong-password, non-authentication
  failure, and persistence-failure behavior.
- Exact normal-layout ASCII art plus accessible/compact alternatives.
- Textual focus, labels, hints, errors, password masking, non-color semantics,
  and raw-control/secret absence.
- Test-first unit, state-machine, persistence, journey, semantic-buffer, and
  snapshot coverage.
- Current TUI documentation/error contract cleanup.

## Task sequence

1. `tasks/001-transactional-vault-registration.md` — add the core atomic,
   reload-before-write registration primitive with concurrency-preservation
   tests.
2. `tasks/002-startup-onboarding-state-machine.md` — make empty registries
   valid and implement path selection, authentication-before-registration,
   configured-vault startup, exit/back, and failure recovery test-forward.
3. `tasks/003-branded-accessible-startup-modal.md` — finish the shared branded
   renderer, exact art, responsive/accessibility behavior, automated journeys,
   snapshots, and documentation.

Tasks execute sequentially and are main-only because they cross a durable
registry boundary, secret-input lifetime, and the central TUI phase machine.

## Quality gate

Every task runs, in order:

1. `make check` — formatting, warning-denying Clippy, offline locked build and
   tests, doctests, feature gates, and supported cross-target checks.
2. `make deny` — license, advisory, source, and ban policy.
3. `make audit` — RustSec audit of the locked dependency graph.
4. `make doc` — warning-denying API documentation.

The final gate additionally runs `make test-ignored`, the focused
`make test-tui-contracts` journey/accessibility suite, and
`make snapshots-check`. KeePassXC serialization interop and S3/clipboard live
tests are omitted because this feature neither writes KDBX data nor touches
sync/clipboard behavior. Flutter gates are outside this Rust-TUI package.

## Risks

- The registry may change between startup and first successful authentication.
  The new core transaction must reload while locked and preserve unrelated
  registrations.
- A successful KDBX open followed by a registry-write failure must not leave an
  unlocked but unregistered session or retain the password.
- File stems can be empty, non-UTF-8, or later collide with a concurrently
  registered name. These cases must remain in the modal with an actionable
  error and no partial write.
- Path errors must not be counted as password failures. Wrong-password errors
  must not erase the selected path, while password buffers must still zeroize.
- A 13-row logo competes with controls at the 60×16 floor. The tested compact
  fallback is mandatory when the full layout cannot preserve every action and
  error line.
- `?` is a valid path/password character. Help dispatch must not steal it while
  either text field owns input.
- A multiple-vault registry could change while its list or password form is
  open. Selection must be identified by vault name, with the saved list index
  used only for restored focus; a vanished selection returns safely to a
  refreshed picker instead of unlocking a different row.
- The dirty baseline includes prior completed work. Reviews must preserve that
  state and restrict repairs to the files and behaviors owned by this package.

## Out of scope

- Creating a new KDBX vault; onboarding selects an existing file.
- Native graphical file choosers, directory browsers, shell execution, full
  environment-variable expansion, or path-completion dependencies.
- Adding or changing keyfile selection. The requested flow explicitly asks for
  a vault password and retains the TUI's current password-only unlock boundary.
- Changing the CLI vault-create/open user experience.
- A persistent default-vault setting for multi-vault registries.
- Flutter/UI application behavior, packaging, or FFI.
- Claiming real terminal-emulator, AT-SPI, Orca, or VoiceOver acceptance from
  TestBackend automation.

## Final acceptance criteria

- Starting `hidlins-tui` with a missing or empty `vaults.toml` constructs the
  app, enters terminal mode, and renders the onboarding modal instead of
  returning `NoVaultsRegistered`.
- At normal supported geometry, the modal renders every line of the request's
  ASCII art exactly and in order from one literal source of truth.
- The explicit accessible theme and undersized terminals render a compact
  semantic `HIDLINS` heading with no clipped prompt, error, or action.
- Empty-registry onboarding provides a labeled path field and textual
  `Enter: Open vault` and `Ctrl+Q: Exit` actions. Relative paths and leading
  `~/` resolve portably without a dependency.
- Invalid, missing, directory, non-UTF-8-name, and unusable path cases preserve
  the path field, show an actionable non-secret error, do not write the
  registry, and do not exit.
- The first chosen vault is not registered until its KDBX password succeeds.
  On success it is atomically present after reloading `vaults.toml`; its
  canonical path, derived unique name, creation timestamp, and empty optional
  keyfile/extra fields round-trip.
- Wrong passwords clear and zeroize only the password input, increment the
  existing bounded authentication counter, preserve the chosen vault, and
  never modify `vaults.toml`. Other open failures are not reported as wrong
  passwords.
- A registry persistence failure after successful authentication drops the
  opened vault, retains no password, does not enter the workspace, and exposes
  a retry/recovery path.
- With exactly one registered vault, startup presents the same branded shell
  directly at a labeled, masked master-password prompt with textual
  `Enter: Unlock` and `Ctrl+Q: Exit` actions.
- With two or more registered vaults, startup presents a distinct third form in
  the same shell: a scrollable registry-order list with a textual selected-row
  carrier, Up/Down and `j`/`k` navigation, `Enter: Continue`, and
  `Ctrl+Q: Exit`. Enter opens the selected vault's masked password form without
  leaving the shared modal.
- The multiple-vault password form contains textual `Esc: Back to vault list`,
  `Enter: Unlock`, and `Ctrl+Q: Exit` actions. Back or Escape drops/zeroizes the
  entered password, returns to the list with the same vault highlighted, and
  permits a different selection. Repeated list → password → list transitions
  do not change `vaults.toml` or accumulate unlock attempts on another vault.
- `--vault NAME` still opens that registered vault's password prompt directly;
  an explicit unknown name remains a pre-terminal usage error.
- Auto-lock/re-entry, cancel/back, help dispatch, global exit, and existing
  unlock attempt behavior remain reachable and internally consistent.
- TestBackend journeys cover empty-registry select/wrong-password/success/save,
  configured single-vault unlock, multiple-vault list navigation, transition
  to the chosen password form, Escape/back restoration, choosing a different
  vault, invalid-path recovery, persistence failure, and exit.
- Accessibility contracts flatten actual buffers and prove meaningful reading
  order, visible labels/states/actions, text-only selection/focus, password
  canary absence with masking present, no raw escape/control sequences, and
  retained semantics without color across 80×24, 60×16, and 40×12 layouts.
- No password appears in the registry, errors, snapshots, debug output, logs,
  command line, or environment; existing zeroize-on-drop tests remain green.
- The core registration transaction preserves a concurrent unrelated registry
  entry, rejects duplicates without writing, and updates the caller only after
  a successful atomic save.
- The dependency fingerprint remains
  `e8305fdc948cc1f58b189a88566c1e5f237c2e3827c4672853ea0f3e1b4e2804`;
  no manifest, `Cargo.lock`, or `vendor/` change is introduced.
- Every standard and final command in `gate.json` passes in order.
