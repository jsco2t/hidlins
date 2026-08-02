# Desktop D1 review record

This is the durable evidence record for flutter-app Phase 5. Automated results
are produced by Makefile targets; human-only observations remain explicit so a
headless run cannot accidentally claim them.

## UX and keyboard

- Global shortcuts: Ctrl/Cmd-F search, `n` new entry, `l` lock,
  Ctrl/Cmd-C copy selected password, Escape dismiss.
- Plain-letter shortcuts yield while an editable text field owns focus.
- Entry rows and group nodes expose right-click actions; hover-only buttons
  have labels, tooltips, and keyboard/global equivalents.
- The expanded single-pane layout fills its workspace; compact/medium/expanded
  layouts are pinned by tests.
- Reduced-motion mode makes navigation/list transitions instantaneous and
  removes generator rotation.

Automated evidence: `make app-test`, including shortcut, semantics-guideline,
context-menu, reduced-motion, layout, and 24 golden matrix tests.

## Integration and performance

- `make app-test-integration`: actual cdylib create→unlock→edit→search→lock,
  real one-second idle auto-lock, and 5,000-entry search.
- Production-KDF unlock recorded 603–638 ms across the final local runs. The number includes the
  configured Argon2id work plus bridge/open/parse overhead and is printed as
  `HIDLINS_APP_UNLOCK_MS` for each run.
- Search sanity: 20 post-warmup samples. Final local runs recorded
  14.303–14.897 ms p95,
  below the 50 ms NFR-002 target; each run prints its exact
  `HIDLINS_APP_SEARCH_P95_MS` value. The authoritative performance gate remains
  Rust-side so hosted-runner noise cannot make this informational check flaky.
- `make app-test-integration-minio`: two real state directories and sessions,
  disjoint three-way merge, and same-entry conflict with a real `.kdbx.bak`
  path rendered in the blocking dialog.
- `make interop-app`: API-authored KDBX alternates writes with KeePassXC 2.7.12+
  and passes normalized XML comparison.
- `make test-minio-managed`: starts the pinned fixture when needed, runs both
  Rust and app live-wire suites, and tears down only a fixture it started.

## Human sign-off still required

- Complete the keyboard-only walkthrough on Linux and macOS.
- Run Orca on Linux and VoiceOver on macOS; record issues and fixes.
- Confirm the 24 golden cases are green in two consecutive CI runs on Linux
  and macOS (golden raster comparisons execute on Linux; macOS runs the same
  widget/layout suite without raster comparison).
- Record a representative machine/build-mode performance value from the CI log.
- Select final brand seed/icon direction if the current provisional identity is
  not accepted for D1.
