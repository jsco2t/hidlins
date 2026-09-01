# Task 003: Branded Accessible Startup Modal

Delegation: main-only

## Goal

Finish the startup experience with the exact requested normal-layout ASCII
art, a responsive and screen-reader-conscious shared modal, and comprehensive
automated journey/accessibility evidence.

## Context

Task 002 makes the flow functional. This task applies the UX review: decorative
art cannot be hidden semantically in a terminal buffer, bracketed controls are
not sufficient focus/keyboard cues, and the 13-row logo cannot be clipped into
small terminals. The existing TestBackend contracts and snapshot harness cover
the deterministic surface without a new dependency.

## Scope

### In scope

- One shared startup renderer for onboarding, registered-vault selection, and
  password entry.
- Exact literal ASCII art in normal themes when geometry permits.
- Accessible-theme and compact semantic alternatives.
- Text labels, reading order, focus/selection carriers, key hints, retry and
  error states.
- TestBackend journeys, accessibility contracts, and focused goldens.
- Current module/help/documentation updates.

### Out of scope

- Real AT-SPI/speech-engine or terminal-emulator claims.
- GUI file chooser, animation, image assets, or new UI dependencies.
- Product behavior outside pre-unlock/re-entry states.

## Implementation requirements

- Add failing render/contract tests before finalizing the renderer.
- Store the request's 13 art rows in one literal constant. Tests must compare
  every row exactly; do not “improve,” Unicode-convert, or re-space it.
- Render the exact art only when the theme is not explicitly `accessible` and
  the available modal area can preserve the entire art and all controls.
- In `accessible` theme or compact geometry, render a concise `HIDLINS` heading
  and the complete semantic form; never crop primary prompt, errors, or exit.
- Use text markers/labels for current vault, focused field, masked password,
  Open/Unlock, Exit, failure counts, and disabled/unavailable states. Color and
  reverse video may supplement but never replace text.
- Keep row-major reading order meaningful. The accessible transcript must put
  heading, prompt, field/selection, error/status, and actions in that order.
- Assert secret canaries and raw ESC/C0/C1 controls are absent from every
  rendered startup state.
- Add journeys for first-run success and recovery plus configured one/multiple
  vault flows using real fast-KDF KDBX fixtures and a reload of `vaults.toml`.
- Extend `make test-tui-contracts` through the existing test modules rather than
  creating another test runner or target.
- Add/update hand-rolled startup snapshot goldens and current documentation.
- Add no dependency, feature, unsafe code, asset license, or license allowance.

## Acceptance criteria

- [ ] Normal 80×24 and 60×16 startup renders contain all 13 exact ASCII-art
      rows in order wherever the full-layout predicate is true.
- [ ] Accessible-theme and 40×12 renders omit decorative art, retain the
      `HIDLINS` semantic heading, and preserve every primary field/action/error.
- [ ] Empty, one-vault, multiple-vault, wrong-password, invalid-path,
      persistence-error, and retry frames have coherent reading order and
      explicit text carriers.
- [ ] The password is masked and both path/password canaries are absent from
      inappropriate frames, snapshots, errors, and debug output.
- [ ] Color and accessible variants retain the same required labels/states and
      no required meaning is color-only.
- [ ] No rendered reviewable buffer contains raw terminal escape or other
      forbidden control characters.
- [ ] In-process journeys prove first-run selection, no pre-auth registry
      mutation, wrong-password recovery, correct-password save/reload/workspace,
      configured direct unlock, multiple selection, and exit.
- [ ] Snapshot goldens are deliberate, stable, and current.
- [ ] Current docs no longer claim an empty registry is fatal or instruct the
      user to leave the TUI to register the first vault.
- [ ] The dependency fingerprint is unchanged.

## Validation

- `make test-tui-contracts`
- `make snapshots-check`
- `cargo test -p hidlins-tui --offline --locked`
- `python3 -c 'from pathlib import Path; import hashlib; roots=[p for p in Path(".").rglob("Cargo.toml") if "target" not in p.parts and ".git" not in p.parts and "vendor" not in p.parts]; files=[Path("Cargo.lock"),*sorted(roots),*sorted(p for p in Path("vendor").rglob("*") if p.is_file())]; h=hashlib.sha256(); [(h.update(p.as_posix().encode()),h.update(b"\0"),h.update(p.read_bytes()),h.update(b"\0")) for p in files]; assert h.hexdigest()=="e8305fdc948cc1f58b189a88566c1e5f237c2e3827c4672853ea0f3e1b4e2804", h.hexdigest()'`

## Dependencies

- Task 002

## Expected areas of change

- `crates/hidlins-tui/src/screens/`
- `crates/hidlins-tui/src/app.rs`
- `crates/hidlins-tui/src/test_support.rs`
- `crates/hidlins-tui/src/journey_tests.rs`
- `crates/hidlins-tui/src/accessibility_contract_tests.rs`
- `crates/hidlins-tui/src/snapshot_tests.rs`
- `crates/hidlins-tui/tests/snapshots/`
- `crates/hidlins-tui/src/lib.rs`

## Risks / notes

The exact art is a visual requirement, not semantic content. Tests must enforce
both its exact normal rendering and its deliberate absence from the explicit
accessible layout.

