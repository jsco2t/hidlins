# Task 002: Migrate to the standalone Material package

Delegation: main-only

## Goal

Move first-party Hidlins UI code from the frozen framework Material library to
the official Flutter 3.47 standalone Material package with no behavior, theme,
accessibility, or supply-chain regression.

## Context

Flutter 3.47 makes the standalone UI packages available as the forward baseline.
This pre-alpha migration is preferable to keeping a growing compatibility layer,
but third-party packages may still expose framework Material types.

## Scope

### In scope

- Add the exact official `material_ui` 1.0.0 dependency and vendor its resolved
  package set.
- Review license, maintenance signal, transitive footprint, feature surface, and
  the in-repository alternative before executing the newly obtained code.
- Migrate first-party imports, localization delegates, theme types, tests, and
  golden harnesses.
- Add the official compatibility bridge only at demonstrated third-party type
  boundaries.

### Out of scope

- Adding `cupertino_ui` without a direct source-level need.
- Redesigning the Hidlins theme or screens.
- Replacing navigation/state-management packages solely for this migration.

## Implementation requirements

- Verify the exact package license is allowed before product code imports it.
- Pin exact versions and refresh `app/vendor-pub` plus its integrity manifest.
- First add or strengthen a focused compilation/widget test that exposes every
  real framework/standalone type mismatch.
- Before the broad import rewrite, compile a minimal Hidlins shell using the pinned
  router/state/localization packages and the official compatibility bridge. If a
  public API forces an irreducible dual Material type system, set
  `PLAN_CHANGE_REQUIRED`; do not normalize a permanent mixed-type architecture.
- Keep compatibility shims narrow and documented; first-party code should import
  standalone Material directly after the task.
- Preserve localization, semantics, text scaling, and platform-adaptive behavior.

## Acceptance criteria

- [ ] No first-party application or test file imports
  `package:flutter/material.dart`; any legacy-framework use is isolated inside one
  narrowly named compatibility adapter and never appears in feature APIs.
- [ ] `material_ui` is exact-pinned, vendored, integrity-checked, and accompanied
  by the repository dependency-review evidence.
- [ ] Themes, localizations, widget tests, and existing goldens pass unchanged or
  have reviewed 3.47-specific updates.
- [ ] No unneeded Cupertino dependency or broad compatibility wrapper is added.

## Validation

- `make pub-vendor-check`
- `make app-analyze`
- `make app-fmt-check`
- `make app-test`
- `make app-build-macos`

## Dependencies

- Task 001

## Expected areas of change

- `app/pubspec.yaml`, `app/pubspec.lock`, `app/vendor-pub/`
- `app/lib/`, `app/test/`
- Pub integrity and dependency-audit configuration/evidence

## Risks / notes

Public types from router or state-management packages can force a dual type system
that the context bridge cannot solve. The feasibility gate must happen before the
mechanical migration; a structural blocker requires plan revision, not scattered
legacy imports.
