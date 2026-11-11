# Flutter App Tests

## Conventions

- **All user-facing strings go through `AppLocalizations`** (NFR-014). Hard-coded
  strings in widgets are a review blocker. English-only for now; the ARB source
  files live in `app/l10n/`.

- **Fakes live in `test/fakes/`** and follow the naming convention
  `Fake<Interface>`. Fakes are hand-written, not generated.

- **Widget tests use `test_helpers.dart`** for wrapping widgets in theme and
  l10n context. Import `helpers/test_helpers.dart`.

- **No hard-coded colors or text styles outside `ui/`.** All colors come from
  `Theme.of(context).colorScheme`; all text styles from
  `Theme.of(context).textTheme` or the `monospaceTextStyle()` helper in
  `ui/typography.dart`. Direct `Color(0x...)`, `Colors.xxx`, or bare
  `TextStyle(...)` constants in feature code are a review blocker. Tokens
  (spacing, motion, breakpoints) come from `ui/tokens.dart`.

## Golden Tests

Goldens validate **layout, color, and spacing** — not typeface fidelity. All
golden tests render with Flutter's built-in FlutterTest font (platform-default
fonts are a runtime concern per DR-1). This makes goldens deterministic across
CI runs without bundling any font assets.

**Execution guard:** golden comparison tests run on **Linux only**. On other
platforms they are skipped with a reason string. This avoids cross-platform
rasterization differences.

**Updating goldens:**

```
HIDLINS_UPDATE_GOLDENS=1 make app-goldens-update
```

Review the diff before committing — golden changes are a visual review, not a
rubber stamp.

**Missing goldens fail.** If a `matchesGoldenFile` call finds no committed
golden, the test fails. Generate the golden first with the update command above.

Golden files live in `test/goldens/` and are committed to the repo.
