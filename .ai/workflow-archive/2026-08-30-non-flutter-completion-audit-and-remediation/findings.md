# Non-Flutter Completion, Correctness, and Architecture Findings

Audit date: 2026-08-31

Baseline: `883535f205d0937d2b90a8bf13051882e1a2c2c8`

Review modes:

- Completion audit against the notebook feature/task indexes and repository evidence.
- `$arch-reviewer`, whole-code mode, confidence threshold 80/100, run sequentially over the six included crates and their dependency boundaries.
- `comp-reviewomatic`, local whole-code mode, confidence threshold 80/100, applying all nine correctness perspectives in the primary thread.

Generated code, vendored sources, fixtures, snapshots, and SigV4 corpus data were excluded from architectural judgment. The explicit package scope is 206 relevant Rust source/test files across the six crates (core 49, genpw 13, security 24, sync 43, CLI 33, TUI 44), so the review was intentionally performed crate by crate rather than narrowed.

## Ordered fix ledger

### HIDLINS-DATA-001 — Merge history drops attachment bytes

- Severity: critical
- Confidence: 100/100
- Review dimension: systems correctness / data integrity
- Owner: Task 004
- Status: FIXED (Task 004)
- Evidence:
  - `crates/hidlins-sync/src/merge/mod.rs:74` explicitly documents that remote historical versions retain attachment references but not the referenced binary bytes.
  - `reconcile` repairs current-entry attachments through `repair_added_entry_attachments` and `propagate_both_side_attachments`, but its history backfill clones field versions without a pool-aware attachment copy.
  - Existing merge tests cover current attachment propagation and idempotence but intentionally do not assert history attachment bytes after save/reopen.
- Concrete cost: a losing secret version can appear in KDBX history while its attachment is missing or refers to the wrong binary pool entry. The one-generation encrypted backup does not satisfy the product's loser-as-history and no-data-loss promises.
- Required repair: add a fail-before regression that saves and reopens a collision containing a loser attachment, then preserve and repoint every historical attachment byte through the merge and KeePassXC-compatible serialization path.
- Resolution: pre-merge snapshots now retain one zeroizing byte buffer per binary-pool item and compare attachment content rather than replica-local ids. Reconcile repairs loser and pre-existing history references through a version-pinned Keepass API patch, fails closed on equal-version/different-byte ambiguity, and rebuilds current/history attachment back-references after parse, merge, tracked edits, and repair. The original encrypted save/reopen regression failed with `remote.bin` resolving to `LOCAL-BYTES`; it now passes, as do add/replace/remove history transitions, parsed-current-removal preservation, repeated-sync idempotence, the 4,096-case property gate, fault injection, and KeePassXC attachment interop.

### HIDLINS-ARCH-001 — TUI application state combines unrelated owners and invalid temporal states

- Severity: critical
- Confidence: 94/100
- Architecture dimensions: separation of concerns, state ownership, testability, hidden temporal coupling
- Owner: Task 006
- Status: FIXED (Task 006)
- Evidence:
  - `crates/hidlins-tui/src/app.rs:204` defines one `App` owner for vault lifecycle, registry/config persistence, recents/pins, auto-lock, command dispatch, mouse hit testing, overlays, entry CRUD, sync worker ownership, settings mutation, and rendering.
  - Production methods span `App::new` at line 339 through `App::render` at line 3564; the same impl performs filesystem writes (`persist_vault`, `persist_ui_state`, `save_ui_config`), domain mutation (`apply_add`, `apply_update`, bulk operations), sync lifecycle, and input/render coordination.
  - `vault` and `registry` are independent `Option` fields whose `None` values mean "temporarily moved to the sync worker". Correctness relies on phase/timing comments and `expect` calls rather than a type that rules out invalid combinations.
- Concrete cost: a change to sync, settings, or vault lifecycle must reason about unrelated UI modes and can construct impossible combinations (`vault` present while registry absent, or vice versa). Tests require a near-full `App` fixture for logic that should be locally testable, and merge conflicts concentrate in a single state owner.
- Required repair: characterize current behavior, then extract cohesive owners/transition helpers. Represent ready-versus-syncing session ownership as an enum or equivalent valid-state type, and move persistence/domain actions out of the input/render coordinator without introducing a framework or changing user-visible behavior.
- Resolution: pre-refactor characterization pinned locked, ready, and syncing resource ownership. `SessionResources` now represents those states as an enum, owns all vault/registry transfers, makes an independently missing vault or registry unrepresentable, and has direct transition/recovery invariant tests. Entry form-to-domain mutation and filesystem persistence were extracted into cohesive `entry_actions` and `persistence` modules while `App` remains the input/render orchestrator. The unchanged 13 snapshots, 405-test default TUI suite, serialized accessibility-mode tests, and search-performance gate verify that the structural repair did not change user-visible behavior.

### HIDLINS-ARCH-002 — UI crates bypass the core boundary through raw `keepass-rs` types

- Severity: important
- Confidence: 88/100
- Architecture dimensions: dependency direction, API shape, separation of concerns
- Owner: Task 006, with prerequisite API work in Tasks 002 and 005
- Status: FIXED (Task 006)
- Evidence:
  - `crates/hidlins-core/src/lib.rs:72` publicly re-exports `Database`, `Entry`, `Group`, `Times`, `Value`, and field constants from `keepass-rs`.
  - `Vault::database` and `Vault::database_mut` expose the dependency's mutable database model.
  - CLI code implements raw database traversal in `crates/hidlins-cli/src/commands/entry.rs:394` and `:604`; TUI code reads recycle-bin and raw tree state in `crates/hidlins-tui/src/app.rs:2361` and `crates/hidlins-tui/src/widgets/entry_tree.rs:268`.
  - The sync merge adapter legitimately needs a narrow raw KDBX seam; interactive UIs do not need the full mutable dependency model.
- Concrete cost: a `keepass-rs` model/API change ripples into presentation crates, and UI code can bypass core invariants or duplicate queries such as group-name and recycle-bin traversal. This also makes it harder to test UI behavior with owned read models.
- Required repair: add owned/core query APIs for the UI data actually needed and migrate CLI/TUI callers. Keep the merge snapshot seam narrow and explicitly internal-facing; do not attempt a breaking public-API purge merely for aesthetics.
- Task-005 evidence: `EntrySummary` and `GroupSummary` provide owned, secret-free query records; `VaultReadOnly::get_entry` and title/group query methods keep KDBX traversal in core; `crates/hidlins-cli/src` contains no raw database traversal. Command characterization preserves list/search group labels and ambiguous-title behavior. Task 006 still must migrate the TUI and narrow the remaining raw exports/seam before this finding is fixed.
- Resolution: TUI recycle-bin, group-choice, tree, and test queries now consume the owned core query records and query helpers; presentation code no longer traverses `Database`, raw `keepass-rs` groups/entries, or raw field constants. The remaining `Vault::database`/`database_mut` methods are explicitly documented and hidden from generated docs as infrastructure seams for sync/interop, while the approved plan correctly avoids a breaking public-API purge. Source-boundary checks cover both presentation crates.

### HIDLINS-BUILD-001 — The supported no-default security build emits dead-code warnings

- Severity: important
- Confidence: 100/100
- Review dimension: build/configuration consistency
- Owner: Task 003
- Status: FIXED (Task 003)
- Evidence: `make check` reaches the no-default feature-gate build and reports `ZeroizingString::new` unused because `secret` remains compiled while clipboard support is disabled. The constructor at `crates/hidlins-security/src/secret.rs:31` is only consumed by the optional clipboard module.
- Concrete cost: supported feature combinations are not warning-clean, hiding future regressions and contradicting the repository's `-D warnings` quality posture.
- Required repair: add a feature-gate compile regression or equivalent exact command, align the module/type cfg with its only consumer, and restore warning-free checks without blanket `allow(dead_code)`.
- Resolution: `make check-feature-gates` now denies warnings for the no-default security build. The red gate reproduced the unused constructor; the `secret` module and its `zeroize` dependency are now both owned by the existing `desktop`/clipboard feature, and the exact warning-denying check plus default, clipboard, logind, and IOKit compile surfaces pass.

### HIDLINS-DEAD-001 — Completed code retains explicit future/deferred production seams

- Severity: important
- Confidence: 92/100
- Review dimension: convention/documentation stewardship
- Owner: Task 002
- Status: FIXED (Task 002)
- Evidence: `crates/hidlins-core/src/secret.rs:155` suppresses dead code on `Keyfile::bytes` with the statement that it will be consumed by a future unlock module, although current unlock handling consumes `Keyfile` through `KeyfileMaterial` and the repository forbids silent deferred work.
- Concrete cost: unused secret-bearing accessors expand the sensitive API surface and make reviewers distinguish live security behavior from abandoned scaffolding.
- Required repair: characterize the current keyfile-bytes unlock path, then remove the unused accessor and stale deferral rather than retaining a warning suppression.
- Resolution: the internal keyfile adapter and real create/reopen integration path were characterized before and after the change. `KeyfileMaterial::from_keyfile` remains the sole owned, zeroize-on-drop byte path; the unused accessor, dead-code suppression, and future-work claim were removed.

### HIDLINS-SUPPLY-001 — Two reachable optional/UI dependency versions carry current RustSec unsoundness advisories

- Severity: important
- Confidence: 100/100
- Review dimension: dependency safety
- Owner: Task 001
- Status: FIXED (Task 001)
- Evidence:
  - `event-listener 5.4.1` is reachable when `hidlins-security/logind` enables `zbus -> async-broadcast/async-lock/async-process`; RustSec reports its listener-count overflow unsoundness and identifies 5.4.2 as patched.
  - `lru 0.16.4` is reachable through `ratatui-core -> ratatui/tui-input`; RustSec reports iterator unsoundness and identifies 0.18.2 as patched.
  - `make audit` currently exits successfully only because the repository policy treats warnings separately; neither item is presently recorded as an intentional non-reachability decision.
- Concrete cost: the lockfile contains known unsound code on supported feature/runtime paths. Even if Hidlins does not exercise the triggering API, leaving the graph unexplained makes the security gate's green exit misleading.
- Required repair: move both paths to compatible patched resolutions during Task 001 after license and footprint review. Validate the supported logind build and production TUI/snapshot behavior. Do not add a blanket ignore or defer the paths to final reconciliation.
- Resolution: `event-listener` is 5.4.2 and `lru` is 0.18.3 through `ratatui 0.30.2`. The supported logind compile, workspace tests, TUI snapshots, `make deny`, and `make audit` pass; no advisory ignore was added.

### HIDLINS-SUPPLY-002 — `keepass 0.12.9` resolves a yanked `chacha20 0.10.0`

- Severity: important
- Confidence: 96/100
- Review dimension: dependency durability
- Owner: Task 001
- Status: FIXED (Task 001)
- Evidence: `cargo tree -i chacha20@0.10.0` resolves `chacha20 0.10.0 -> keepass 0.12.9 -> hidlins-core`; `make audit` reports the yank.
- Concrete cost: future reproducibility and security maintenance rely on a withdrawn crypto dependency release. The current vendored/locked build still works, so this is remediation rather than evidence of an exploit.
- Required repair: evaluate the narrowest compatible `keepass`/transitive upgrade or an audited downstream pin/patch, preserving KDBX interop and merge behavior. Do not hand-roll the cipher.
- Resolution: `keepass` is 0.13.25 and resolves released `chacha20 0.10.2`; the yanked 0.10.0 is absent. Cipher code was not patched or reimplemented. The parent upgrade exposed a real KDBX 4.0 write-back incompatibility; red interop and focused KDBX3/4.0 unit regressions led to core version normalization before save. KDBX unit/integration, merge, snapshot, and all KeePassXC interop gates now pass.

### HIDLINS-SUPPLY-003 — Direct dependencies remain for unused or trivial local behavior

- Severity: important
- Confidence: 100/100 (`anyhow`); 96/100 (`hex`)
- Review dimension: dependency minimization / supply-chain surface
- Owner: Task 001
- Status: FIXED (Task 001)
- Evidence:
  - `hidlins-tui` declares `anyhow 1.0.103`, but no production Rust call site imports or invokes it; the top-level process already returns the typed exit status and a comment is stale.
  - `hidlins-sync` uses `hex 0.4.3` only to produce lowercase ASCII encoding of byte arrays in the SigV4/hash paths and their tests. No parsing, mixed-case, streaming, or allocation-policy feature is used.
- Concrete cost: even leaf direct dependencies require ongoing version, license, vendor, and advisory review. `anyhow` provides no current behavior, while the used hex operation is a small stable non-cryptographic algorithm that can be exhaustively characterized locally.
- Required repair: remove unused `anyhow`. Add behavior-pin tests for the current lowercase hex output, replace `hex` with one narrowly scoped local encoder, migrate all call sites, and remove the crate from the resolved graph. Do not expand this rationale to cryptography, formats, protocols, Unicode tables, or platform bindings.
- Resolution: the unused TUI `anyhow` edge is removed. Lowercase encoding and mixed-case keyfile behavior were pinned before replacement; local audited helpers now cover the narrow behavior in Hidlins and the two affected vendored packages. `hex` is absent from the whole resolved workspace graph, and reproducible fail-closed vendor patching is part of `make vendor`.

### HIDLINS-DOC-001 — Shipped documentation still describes transitional or placeholder states

- Severity: important
- Confidence: 97/100
- Review dimension: documentation accuracy / operability
- Owner: Task 007
- Status: FIXED (Task 007)
- Evidence:
  - `crates/hidlins-sync/Cargo.toml:3` describes the S3 transport as future work even though it is shipped.
  - `crates/hidlins-tui/Cargo.toml:3` describes the TUI as a placeholder.
  - `crates/hidlins-core/src/vault.rs:214` describes an in-memory remote snapshot as a "git blob" after git sync was removed.
  - Several TUI command comments still claim later task phases will consume production members that are now consumed.
- Concrete cost: maintainers and generated package/docs output cannot reliably distinguish current architecture from abandoned implementation history.
- Required repair: update current repository documentation and comments to describe the implemented S3/TUI state; keep historical notebook records untouched.
- Resolution: crate metadata and current source/contributor documentation now describe the shipped S3 transport, tabbed TUI, CLI verbs, registry extensions, OS-event tests, sync activity callback, and current project status. The obsolete empty `hidlins-sync::activity` scaffold was removed because `SyncOptions::on_activity` is the live implementation and the TUI wires it to auto-lock activity. Static stale-claim/conflict-marker checks, generated completions/snapshots, warning-denying rustdoc, and the full standard gate pass. External notebook corrections are listed separately without modifying the notebook.

### HIDLINS-VERIFY-001 — Required Orca accessibility verification has no human audio result

- Severity: environment/manual gate
- Confidence: 100/100
- Owner: Task 008
- Status: SUPERSEDED / AUTOMATED DETERMINISTIC COVERAGE COMPLETE
- Historical evidence: before Revision 4, `tui-skeleton` T7.4 and
  `tui-enhancements` T5.3 remained open in their authoritative task indexes
  because automated observation cannot judge spoken output. Task 009 has now
  reconciled both indexes to the approved automation-first completion model.
- Resolution: the approved verification model changed in Revision 4. Scripted
  Ratatui journeys and accessibility semantic contracts now verify reading
  order, textual state carriers, password-canary absence, theme-independent
  semantics, and raw-control exclusion. The duplicated manual suites were
  removed. One Linux Orca/AT-SPI case and one macOS VoiceOver case remain as
  optional, non-gating, explicitly unexecuted platform-confidence checks; no
  speech-integration pass is claimed.

### HIDLINS-VERIFY-002 — Vault-core macOS/CI/M1 evidence remains external

- Severity: external verification gap
- Confidence: 100/100
- Owner: Task 008
- Status: OPEN/EXTERNAL
- Evidence: `features/vault-core/index.md` says implementation is locally complete while the macOS matrix and M1 benchmark remain pending. This package is executing on Linux.
- Concrete cost: platform-specific behavior and the M1 target cannot be claimed from Linux evidence.
- Required repair: inspect cross-target/CI coverage and run all Linux equivalents; record the remaining macOS/M1 checks as external rather than fabricated failures or successes.

## Completion-claim inventory

| Feature or bucket | Notebook claim | Repository evidence reviewed | Verdict |
| --- | --- | --- | --- |
| `vault-core` | 20/20, locally complete; macOS/CI/M1 pending | `hidlins-core` vault/registry/locking/atomic/secret modules; US-001..007, US-054/055, randomized round-trip, KeePassXC harnesses | Implemented locally; external verification remains `HIDLINS-VERIFY-002`. |
| `entry-management` | 16/16 complete | Entry builder/view/CRUD/groups/search/TOTP/attachments/expiration; US-010..018 and entry interop scripts | Implemented. No independent completion gap found. |
| `password-generation` | 11/11 complete | `hidlins-genpw` rejection sampling, random/password/passphrase builders, EFF list; US-020..023 and distribution tests | Implemented. Architecture review clean. |
| `security-behaviors` | 12/12 complete | Auto-lock controller, zeroizing clipboard, signal/logind/IOKit sources, hardening; US-051..053 and helper binaries | Implemented; the no-default warning gap `HIDLINS-BUILD-001` was fixed in Task 003. |
| `cli-skeleton` | all four phases shipped | Clap tree, secure prompt, stable exit mapping, human/JSON views, completion producer; CLI integration tests | Implemented. The SSH command is an explicitly honest post-MVP slot, not a false completion claim. |
| `s3-sync` | 25/25 complete | Transport trait, SigV4 corpus, S3/HTTP/auth/config, sync truth table, merge properties/fault tests/MinIO/interop | Implemented; the history-attachment data-loss defect found by this audit is fixed by Task 004 with encrypted save/reopen and KeePassXC evidence. |
| `tui-skeleton` | 32/32 complete after verification-model correction | Unlock/workspace/tree/detail/overlays/tabs/settings/sync integration, snapshots, scripted Ratatui journeys, accessibility semantic contracts, and 400+ current unit tests | Implemented. Architecture findings `HIDLINS-ARCH-001/002` are fixed. Optional Orca/AT-SPI and VoiceOver platform-confidence cases remain explicitly unexecuted and non-gating. |
| `pre-mvp-fixes` | 5/5 complete | Protected-field fidelity, hardening, sync timeouts, TUI fixes, secret writes and tests | Implemented; no reopened behavior defect found in the bucket. |
| `tui-enhancements` | 23/23 complete after verification-model correction | Command registry/keymaps/themes/config, fuzzy search, navigation/mouse/read-only coverage, scripted journeys, and accessibility semantic contracts | Implemented. T5.3 is complete from automated deterministic evidence; optional external platform-confidence cases remain unexecuted and non-gating. |
| `cli-sync-wiring` | 8/8 complete | Real `Sync` calls, S3 set-sync schema, error/exit mapping, command tests and completion coverage | Implemented. |

## Recorded bug reconciliation

| Recorded bug | Source/test result | Verdict |
| --- | --- | --- |
| Merge attachment propagation | `repair_added_entry_attachments`, `propagate_both_side_attachments`, merge semantic/property tests | Fixed in current source; notebook status is stale. |
| Merge-history attachment loss | Zeroizing pool snapshots, pool-safe history repair, parsed back-reference reconstruction, encrypted save/reopen tests, and enhanced KeePassXC harness | Fixed as `HIDLINS-DATA-001` in Task 004. |
| RST-CRED-1 password-change re-encryption | The implemented `hidlins-api` session password-change flow and its boundary tests re-encrypt the stored credential before registry persistence; CLI/TUI do not expose password change | Fixed in current source; notebook status is stale. Flutter UI completion remains out of this package's scope. |

## Architecture review results

### `hidlins-core` — whole code, 49 files

- Important finding: `HIDLINS-ARCH-002` at the UI boundary.
- The atomic-write, advisory-lock, registry, secret-wrapper, and entry capability modules otherwise have clear ownership and are directly testable.
- The raw `Database` seam remains justified for the sync merge adapter; the finding is the much broader UI exposure, not the existence of a KDBX adapter.

### `hidlins-genpw` — whole code, 13 files

- No architecture finding above 80.
- Sampling is isolated, builders own policy, and deterministic byte seams make the cryptographic postconditions testable without abstract RNG frameworks.

### `hidlins-security` — whole code, 24 files

- No architecture finding above 80.
- `Clock` and `OsLockEventSource` are real polymorphic seams; optional platform backends stay behind feature/target boundaries. `HIDLINS-BUILD-001` is build hygiene, not an architecture defect.

### `hidlins-sync` — whole code, 43 files

- No architecture finding above 80.
- The transport trait has production and memory implementations, S3/auth/wire concerns are separated, and the large state-machine function represents one explicit truth table protected by integration/property tests.
- `HIDLINS-DATA-001` is a correctness/data-model limitation, not evidence that the transport abstraction is misplaced.

### `hidlins-cli` — whole code, 33 files

- Important cross-boundary finding: `HIDLINS-ARCH-002`.
- Command parsing, secure prompting, exit classification, and views are otherwise thin and separated from core operations.

### `hidlins-tui` — whole code, 44 files

- Critical finding: `HIDLINS-ARCH-001`.
- Important cross-boundary finding: `HIDLINS-ARCH-002`.
- Widgets, overlays, keymap parsing, themes, and persistence formats already live in cohesive modules; the repair should preserve those working boundaries and reduce only the central coordinator's mixed ownership.

### Cross-crate dependency direction

- Core/genpw/security do not depend upward on frontends.
- Sync depends on core; CLI/TUI depend on core/genpw/security/sync. No dependency cycle was found.
- The architectural issue is type/model leakage across an otherwise correct dependency direction.

Architecture finding count: 2 (critical 1, important 1).

## Composite code-review summary

- Critical findings: 2 (`HIDLINS-DATA-001`, `HIDLINS-ARCH-001`).
- Important findings: 7 (`HIDLINS-ARCH-002`, `HIDLINS-BUILD-001`, `HIDLINS-DEAD-001`, `HIDLINS-SUPPLY-001`, `HIDLINS-SUPPLY-002`, `HIDLINS-SUPPLY-003`, `HIDLINS-DOC-001`).
- Manual/external gaps: 2.
- Perspectives applied: API/schema, architecture/abstraction, conventions/docs, infrastructure hardening, integration/deployment, Rust language fit, operability, security/data protection, systems correctness.
- No additional ≥80-confidence secret logging, plaintext persistence, unsafe-boundary, advisory-lock, atomic-write, CLI exit-code, or sync failure-isolation defect survived source/test validation.

## Out of scope and non-findings

- Flutter and `hidlins-api` substantive behavior are excluded. `make check` may compile the API only as regression containment.
- Clock-anomaly warnings were planned but intentionally abandoned; git sync is archived; SSH keys and the optional agent are unimplemented roadmap items.
- Module size alone was not treated as a defect. Large search, signer, auto-lock, theme, and sync modules remained below the finding threshold where responsibilities and tests were cohesive.
- Generated bindings, vendored crates, fixtures, goldens, and third-party test corpora were not judged as application architecture.
