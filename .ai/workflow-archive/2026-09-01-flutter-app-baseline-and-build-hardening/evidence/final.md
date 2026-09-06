# Final Work-Package Evidence

## Work package

- ID: `2026-09-01-flutter-app-baseline-and-build-hardening`
- Title: Flutter App Baseline and Build Hardening
- Acceptance rounds: 3

## Original objective

Re-evaluate the historical Flutter plans against the reviewed Rust baseline and
PRD; establish Flutter 3.47.2 as the implementation baseline; deliver the
shared Rust-backed desktop, iOS, and Android alpha with approved Hidlins
branding; enforce fatal first-party compiler warnings; add release builds; and
fold viable July-plan work into an automation-first implementation. The two
acceptance follow-ups add authenticated registration of an arbitrary existing
KDBX, correct the CLI identity, make TUI startup and vault switching clear and
reachable, and add safe desktop attachment export.

## Completed tasks

| Task | Result | Evidence |
| --- | --- | --- |
| 001 — Flutter 3.47 baseline | COMPLETE | `evidence/001.md` |
| 002 — Standalone Material migration | COMPLETE | `evidence/002.md` |
| 003 — Build hardening and release | COMPLETE | `evidence/003.md` |
| 004 — Brand assets and launcher icons | COMPLETE | `evidence/004.md` |
| 005 — Desktop alpha closure | COMPLETE | `evidence/005.md` |
| 006 — Desktop real-bridge interoperability | COMPLETE | `evidence/006.md` |
| 007 — Android Rust/JNI foundation | COMPLETE | `evidence/007.md` |
| 008 — Stable mobile boundary | COMPLETE | `evidence/008.md` |
| 009 — iOS security/storage foundation | COMPLETE | `evidence/009.md` |
| 010 — iOS alpha verification | COMPLETE | `evidence/010.md` |
| 011 — Android production integration | COMPLETE | `evidence/011.md` |
| 012 — Android security/storage | COMPLETE | `evidence/012.md` |
| 013 — Android alpha verification | COMPLETE | `evidence/013.md` |
| 014 — Release, CI, and acceptance | COMPLETE | `evidence/014.md` |
| 015 — Existing-vault CLI registration | COMPLETE | `evidence/015.md` |
| 016 — Correct CLI Hidlins banner | COMPLETE | `evidence/016.md` |
| 017 — Redesign TUI startup and vault selection | COMPLETE | `evidence/017.md` |
| 018 — Desktop attachment export | COMPLETE | `evidence/018.md` |

Earlier acceptance-round snapshots are preserved in
`evidence/final-round-001.md` and `evidence/final-round-002.md`.

## Whole-package review

The main thread reviewed the frozen request and plan, all eighteen task and
evidence records, and the cumulative implementation from the baseline through
the current working tree. The integrated review covered Rust/UI dependency
direction, secret ownership and zeroization, KDBX and registry no-data-loss
behavior, generated FFI and native platform boundaries, warning enforcement,
release and acceptance contracts, CLI/TUI reachability, attachment-export
privacy, and repository hygiene.

The final acceptance additions preserve the thin-interface architecture. The
CLI delegates authenticated vault opening and atomic registry persistence to
the core. The TUI reuses those operations while making the active field and
vault chooser explicit. Desktop attachment export sends only a sanitized
suggested filename over a narrow native destination-picker channel; attachment
bytes remain in Rust and are written through the existing atomic export path.

No unresolved high-confidence in-scope finding remains.

## High-confidence findings fixed

- Round 1 fixed an obsolete native-process allowlist entry, non-unique platform
  capability counting, and FRB 2.12 generated-file manifest drift.
- Round 2 removed the demo generator's unconditional target deletion, leaving
  overwrite refusal to the authoritative atomic vault-creation path.
- The final round removed accidentally tracked build reports, compiler caches,
  fixture targets, and an ARM QEMU core dump from the repository index. Exact
  ignore rules preserve those local generated files while preventing them from
  being committed again. The repository contains no Git submodules, gitlinks,
  or `.gitmodules` file.

## Final acceptance criteria

- [x] All eighteen tasks are COMPLETE, with task-specific validation, standard
  gates, and an evidence file for every task.
- [x] Flutter 3.47.2 / Dart 3.13.2 and the reviewed Android DSL compatibility
  model are pinned and verified; first-party Rust, Dart, Linux, Apple, Kotlin,
  and Java compiler/analyzer warnings are fatal.
- [x] Release Make targets and native build targets produce the approved Rust,
  Linux, macOS, iOS, and Android unsigned artifacts; Make and CI remain aligned.
- [x] Approved branding and launcher resources appear across the application
  artifacts, and every July task has a recorded disposition.
- [x] Every automatable requirement has automated evidence. User-declined
  manual and physical-device checks are recorded as skipped, never as passing.
- [x] `vault register --id NAME --path FILE [--keyfile FILE]` authenticates and
  atomically registers an existing KDBX without modifying it; failures do not
  mutate registry state or expose secret material.
- [x] `docs/running-and-testing.md` uses the generated kitchen-sink vault and
  accurately covers CLI, TUI, desktop, iOS Simulator, and Android emulator
  startup and testing.
- [x] `hidlins -h` and `hidlins --help` render the tested HIDLINS banner with no
  Falach wordmark fragment.
- [x] Normal and compact TUI startup frames clearly identify the active input,
  render the approved logo where space permits, and expose a reachable vault
  chooser before initial unlock and after locking.
- [x] Failed or cancelled TUI vault addition does not mutate registry or KDBX
  state, and startup rendering does not expose passwords or entry secrets.
- [x] macOS and Linux attachment rows expose an accessible `Save as…` action;
  cancellation is non-destructive and success/failure receives localized
  feedback.
- [x] Attachment bytes never cross the Dart/native picker channel, no temporary
  plaintext viewing file is created, and mobile behavior remains unchanged.
- [x] No new external dependency was added for the round-3 fixes, and repository
  policy prevents embedded Git repositories or submodules from being checked
  in.
- [x] The final whole-package review is clean, the approved-plan hash matches,
  and every final quality command passed.

## Final quality gate

| Command | Result |
| --- | --- |
| `make verify` | PASS — Rust/Flutter suites, ignored tests, documentation, supply-chain audits, KeePassXC interoperability, acceptance audit, and architecture/boundary checks |
| `make app-build-macos` | PASS — warning-fatal analysis and unsigned release `Hidlins.app` build (67.5 MB) |
| `git diff --check` | PASS |

The macOS build retains the documented Flutter advisory that the local
Cargokit plugin does not yet support Swift Package Manager. It is a Flutter
tool migration notice rather than a first-party compiler warning; all
first-party compiler and analyzer warnings remain fatal.

## Cumulative diff

From baseline `ae2b2e5f1e5f23619613d56389c576f98dc361a1`, the package pins and
vendors the Flutter baseline; migrates the UI; establishes shared Rust bridge
and native platform-service implementations; integrates branding; adds iOS
Simulator and Android emulator automation; hardens Make/CI warning and release
policies; adds acceptance, interoperability, supply-chain, and architecture
gates; supports authenticated registration of existing vaults; documents the
kitchen-sink test workflow; corrects CLI/TUI identity and startup behavior; and
adds atomic desktop attachment export.

Current Git revision at final validation:
`e6ebbba6d08ae3093a53fb971e2ec39d8235df28`. Acceptance-round implementation
and final repository-hygiene repairs remain in the working tree; the workflow
did not create a commit.

## Remaining non-blocking concerns

- Manual assistive-technology and OS-shell visual observations remain
  `SKIPPED — user decision`; deterministic semantics, focus, scaling,
  resource, lifecycle, simulator, and emulator precursors passed.
- Physical iOS and Android device testing is not required because hardware is
  unavailable; simulator/emulator coverage is the approved baseline.
- Live credentialed S3 remains `SKIPPED — user decision / credentials not
  supplied`; managed MinIO, transport state-machine, and failure/recovery
  automation passed.
- Flutter's Cargokit Swift Package Manager migration advisory remains
  non-blocking and is already documented as third-party/generated tooling.
