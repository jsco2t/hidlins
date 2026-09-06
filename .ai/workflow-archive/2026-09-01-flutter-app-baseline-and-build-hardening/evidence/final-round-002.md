# Final Work-Package Evidence

## Work package

- ID: `2026-09-01-flutter-app-baseline-and-build-hardening`
- Title: Flutter App Baseline and Build Hardening
- Acceptance rounds: 2

## Original objective

Re-evaluate the historical Flutter plans against the reviewed Rust baseline and
PRD; establish Flutter 3.47.2 as the implementation baseline; deliver the
shared Rust-backed desktop, iOS, and Android alpha with approved Hidlins
branding; enforce fatal first-party compiler warnings; add release builds; and
close viable July-plan gaps with automation-first acceptance. Acceptance round
2 additionally corrects the CLI so an arbitrary existing KDBX can be
authenticated and registered, and documents using the generated kitchen-sink
vault across every application form.

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

Round 1's preserved package evidence is in
`evidence/final-round-001.md`.

## Whole-package review

The main thread reviewed the frozen request, plan, all task/evidence records,
and the cumulative implementation from the main baseline through the current
working tree. The integrated review covered dependency direction, Rust/UI
responsibility, secret ownership, generated FFI and platform boundaries,
warning enforcement, artifact and acceptance contracts, and Task 015's
authentication/registry/no-data-loss behavior.

The existing-vault command remains a thin CLI adapter: `Vault::open` owns KDBX
authentication, `VaultRegistry::register_and_save` owns the locked atomic
registry transaction, and the CLI persists nothing before authentication.
Registration does not rewrite the KDBX; its tests pin byte preservation,
canonical paths, keyfile behavior, failure non-mutation, ID-based reuse, and
secret-free output. The running guide now uses that command with the generated
kitchen-sink vault.

No unresolved high-confidence in-scope finding remains.

## High-confidence findings fixed

- Round 1 fixed an obsolete native-process allowlist entry, non-unique platform
  capability counting, and FRB 2.12 generated-file manifest drift.
- Round 2 removed the demo generator's unconditional target deletion. The
  generator now relies on `Vault::create` for authoritative overwrite refusal,
  closing the race between the Makefile precheck and vault creation.

## Final acceptance criteria

- [x] All fourteen original tasks and the additive round-2 task are COMPLETE,
  with task-specific validation, standard gates, and evidence.
- [x] Flutter 3.47.2 / Dart 3.13.2 and the reviewed Android compatibility model
  are pinned and verified; the Rust, Dart, Linux, Apple, and Android
  first-party warning policies remain fatal.
- [x] `make release` and documented native Make targets produce the approved
  Rust, Linux, macOS, iOS, and Android unsigned artifacts.
- [x] Approved branding, July-task disposition, automated requirement evidence,
  user-skipped residuals, and CI/Make parity remain covered by the deterministic
  acceptance audit.
- [x] `vault register --id NAME --path FILE [--keyfile FILE]` authenticates and
  atomically registers an existing KDBX without changing its bytes.
- [x] Failed registration does not create or change registry state and does not
  expose secret material; duplicate IDs are rejected before prompting.
- [x] The registered ID works with the existing `vault open`, entry, and shared
  registry-based sync architecture.
- [x] Human/JSON output, help, and bash/zsh/fish completions expose the command
  accurately.
- [x] `docs/running-and-testing.md` generates and directly registers the
  kitchen-sink vault, distinguishes registration from the one-shot open probe,
  and documents CLI, TUI, desktop, iOS Simulator, and Android emulator startup.
- [x] `make demo-vault` is offline and warning-fatal, produces a
  KeePassXC-readable fixture, and refuses missing, relative, or existing output
  paths without adding a dependency or committing a generated KDBX.
- [x] The whole-package review is clean and every final gate passed.

## Final quality gate

| Command | Result |
| --- | --- |
| `make verify` | PASS — Rust/Flutter suites, ignored tests, docs, supply-chain audit, KeePassXC interoperability, acceptance audit, and boundary checks |
| `make app-build-macos` | PASS — unsigned release `Hidlins.app` built (67.5 MB) |
| `git diff --check` | PASS |

The macOS build retains the already-documented Flutter advisory that the local
Cargokit plugin does not yet support Swift Package Manager. It is a Flutter
tool migration notice, not a first-party compiler warning; first-party compiler
and analyzer warnings remain fatal.

## Cumulative diff

From baseline `ae2b2e5f1e5f23619613d56389c576f98dc361a1`, the package establishes
the pinned Flutter toolchain and vendored pub set; migrates the Flutter UI;
adds shared desktop/mobile Rust bridge and platform-service implementations;
integrates branding and native resources; adds iOS Simulator and Android
emulator build/test harnesses; hardens Make/CI warning and release policies;
adds acceptance, interop, supply-chain, and architecture gates; and adds the
authenticated existing-vault CLI workflow plus its documentation and tests.

Current Git revision at final validation:
`e6ebbba6d08ae3093a53fb971e2ec39d8235df28`. The round-2 acceptance fix is in
the working tree and was not committed by the workflow.

## Remaining non-blocking concerns

- Manual assistive-technology and OS-shell visual observations remain
  `SKIPPED — user decision`; deterministic semantics, focus, scaling,
  resource, lifecycle, simulator, and emulator precursors passed.
- Physical iOS and Android device testing is not required because hardware is
  unavailable; simulator/emulator coverage is the accepted baseline.
- Live credentialed S3 remains `SKIPPED — user decision / credentials not
  supplied`; managed MinIO, transport state-machine, and failure/recovery
  automation passed.
