# Task 005: Close deterministic desktop alpha gaps

Delegation: main-only

## Goal

Complete the remaining deterministic desktop UI and sync experience on the
reviewed application shell before extending it to mobile.

## Context

The current app implements vault and entry flows but the sync route is still a
placeholder, the full golden matrix is absent, and the documented desktop polish
and accessibility contracts are incomplete.

## Scope

### In scope

- Implement the desktop sync route over the existing Rust-backed repository/data
  layer, including configuration, progress, success, conflict, and error states.
- Finish documented keyboard shortcuts, focus behavior, context menus, destructive
  confirmations, and desktop affordances needed for alpha usability.
- Add the July matrix of 24 deterministic goldens: lock, list+detail, generator,
  and settings × light/dark × compact/medium/expanded, plus branded first-run and
  sync-state coverage where a layout regression would otherwise escape.
- Strengthen semantics, focus-order, tooltip, scaling, and reduced-motion tests.
- Re-run the July 5,000-entry list/search performance sanity and record numbers
  through a deterministic repository-owned target without replacing the existing
  Rust performance gates.

### Out of scope

- Real MinIO/KeePass/device integration (Task 006).
- Mobile navigation and native platform services.
- Product capabilities beyond the PRD alpha surface.

## Implementation requirements

- Flutter renders state; Rust remains responsible for sync, merge, conflicts,
  vault persistence, and secret handling.
- Conflict presentation must retain loser-as-history and merge-before-present
  semantics.
- Every repaired defect begins with the smallest failing widget/unit test.
- Preserve the July shortcut/interaction contract: search, new entry, manual lock,
  copy selected password, generator, Escape dismissal, entry/group context menus,
  keyboard-only unlock→find→copy→lock, and `MediaQuery.disableAnimations`.
- Goldens must be deterministic and use the repository Linux policy; platform
  font differences must not be hidden by accepting arbitrary host output.
- Destructive actions require explicit confirmation, and secret values remain
  concealed by default.

## Acceptance criteria

- [ ] The sync route exposes configure/run/progress/result/conflict/error flows
  through the existing Rust sync API and has deterministic tests for each state.
- [ ] All documented desktop actions are keyboard reachable with correct focus,
  tooltip, scaling, and semantics coverage.
- [ ] The full documented alpha golden matrix exists and passes on its canonical
  host in light/dark and compact/expanded layouts.
- [ ] The automated controlled-repository 5,000-entry search/list sanity is measured
  and remains within the PRD budget or the workflow stops for a scoped repair;
  Task 006 repeats the measurement through the real bridge.
- [ ] Startup degradation warnings, OS-lock behavior, foreign-clipboard ownership,
  and two-layer lock guarding remain visible/tested after the UI migration.
- [ ] No vault, crypto, merge, or secret-lifecycle logic is implemented in Dart.

## Validation

- `make app-analyze`
- `make app-test`
- `make app-test-performance`
- `make app-fmt-check`
- `make app-build-macos`

## Dependencies

- Task 004

## Expected areas of change

- `app/lib/features/sync/`
- Existing shell, vault, entry, settings, theme, and routing code
- `app/test/`, golden fixtures, and accessibility test helpers

## Risks / notes

UI convenience can accidentally duplicate Rust sync state or expose secret data.
Keep view models as typed projections and make conflict/error states explicit.
