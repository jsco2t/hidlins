# Local Sync Test Matrix

This matrix assigns stable identifiers to the preserved characterization suite
and the V1 security requirements. IDs remain stable when tests move files or
change transport terminology.

## Preserved characterization baseline

| Stable ID | Invariant | Current executable evidence |
| --- | --- | --- |
| `SYNC-STATE-001` | unchanged local and remote is already current | `sync::tests::second_sync_is_already_in_sync` (`TC-SYNC-001`) |
| `SYNC-STATE-002` | local-only change conditionally pushes | `sync::tests::state_local_changed_only_pushes` (`TC-SYNC-002`) |
| `SYNC-STATE-003` | remote-only change fast-replaces and creates backup | `sync::tests::state_remote_changed_only_fast_replaces` (`TC-SYNC-003`) and `tests/us_041_fast_replace_open.rs` |
| `SYNC-STATE-004` | both changed merges and conditionally commits | `tests/us_043_disjoint_merge.rs`, `tests/us_044_collision_merge.rs` |
| `SYNC-CAS-001` | failed conditional commits re-fetch/re-merge within a bound | `sync::tests::merge_with_precondition_failed_retries_then_succeeds` (`TC-SYNC-005`) |
| `SYNC-CAS-002` | retry exhaustion is explicit and preserves backup | `sync::tests::conditional_put_exhausted_when_remote_keeps_advancing` (`TC-SYNC-006`) |
| `SYNC-KDF-001` | incompatible KDF aborts before replacement | `sync::tests::kdf_param_mismatch_aborts_before_merge` (`TC-SYNC-008`) |
| `SYNC-ID-001` | a different logical vault cannot replace the open vault | `hidlins-core` `Vault::replace_database` UUID tests and sync fast-replace tests |
| `SYNC-PTR-001` | successful divergence pointers persist together | `sync::tests::successful_sync_updates_both_last_synced_pointers_atomically` (`TC-SYNC-014`) |
| `SYNC-OFFLINE-001` | remote failure leaves local bytes and backup policy intact | `tests/us_045_sync_failure.rs` and `sync::tests::head_error_surfaces_remote_unreachable_without_backup` |
| `SYNC-BACKUP-001` | merge conflict keeps live file and pre-merge backup | `tests/fault_injection.rs::unresolvable_merge_surfaces_error_and_preserves_disk_state` |
| `SYNC-BACKUP-002` | crash after backup/before fetch is recoverable | `TC-FAULT-001` |
| `SYNC-BACKUP-003` | crash after save/before remote commit preserves backup | `TC-FAULT-002` |
| `SYNC-BACKUP-004` | manual backup restoration opens and restores old state | `TC-FAULT-003` |
| `SYNC-MERGE-001` | contested loser survives under the same UUID in history | `merge_semantics::remote_newer_wins_and_local_loser_is_preserved_in_history` plus property suite |
| `SYNC-MERGE-002` | password loser and attachment bytes survive history/save/reopen | focused `merge_semantics` tests plus property suite |
| `SYNC-MERGE-003` | merge is deterministic/idempotent and disjoint changes commute | `tests/merge_property_tests.rs` |
| `SYNC-OWNER-001` | API moves the vault out during sync and restores it only on allowed completion | `us_094_sync_api::queries_during_sync_return_vault_busy_syncing_and_vault_returns_intact` |
| `SYNC-OWNER-002` | lock/shutdown during API sync discards the returned vault | the lock/shutdown success, failure, panic, grace-window tests in `us_094_sync_api.rs` |
| `SYNC-OWNER-003` | TUI worker takes and returns one owned vault/registry pair | `sync_runtime::tests::drain_yields_activity_then_done_and_clears_inflight`; the consuming `start` signature is compile-time evidence |
| `SYNC-OWNER-004` | lost TUI worker cannot leave a false in-flight state | `sync_runtime::tests::drain_on_worker_disconnect_yields_worker_lost_and_clears_inflight` |

The current CAS tests intentionally characterize a configurable three-attempt
budget. Protocol V1 deliberately replaces that with one retry after the initial
attempt; Task 007 changes the assertion to exactly two attempts. Remote-version
names are likewise current behavior to rename, not compatibility requirements.

## V1 automated requirement map

| Stable ID family | Required behavior | Implemented task |
| --- | --- | ---: |
| `LNS-NOISE-001..099` | exact XX/IK suites and prologue, key pinning, no downgrade, secret-state zeroization/source guard | 002 |
| `LNS-ADDR-001..099` | exhaustive allowlist, mapped normalization, IPv6 scopes, actual socket rechecks, no override | 003 |
| `LNS-FRAME-001..099` | exact framing/codec, checked lengths, chunks, malformed/truncated/duplicate/reordered handling | 003 |
| `LNS-PROTO-001..099` | message/state ordering, replay rejection, generic errors, no pre-auth disclosure | 003 |
| `LNS-IDENTITY-001..099` | sealed identity, domain separation, zeroization, password rewrap preserves public key | 004 |
| `LNS-PAIRING-001..099`, `LNS-PAIR-001..099` | state-model pairing invariants (`PAIRING`) and runtime/process pairing boundaries (`PAIR`) | 004 |
| `LNS-TRUST-001..099` | one pinned authority, multiple clients, rename/revoke, key mismatch and revoked sessions fail | 004 |
| `LNS-DISCOVERY-001..099`, `LNS-DISC-001..099` | discovery policy/application behavior (`DISCOVERY`) and real desktop adapter interoperability (`DISC`) | 005 |
| `LNS-SERVER-001..099` | bounded listener/queue/connections/deadlines, host ownership, coherent head/fetch/commit | 006 |
| `LNS-COMMIT-001..099` | SHA-256 CAS, identity/KDF validation, staging cleanup, backup/atomic fault boundaries | 006 |
| `LNS-CLIENT-001..099` | candidate and deadline budgets, bounded sole-route handshake retry, one CAS retry, four-state preservation, pair-and-import, offline failure | 007 |
| `LNS-SCHED-001..099` | startup once per configured vault/process then manual only; no mutation/lock/quit sync | 007, 008 |
| `LNS-API-001..099` | typed session/FFI events, lifecycle races, secret-free errors, mobile server refusal | 008 |
| `LNS-CLI-001..099` | now/serve/pair/import/status/peer commands, Ctrl+C, secure prompts, JSON and exit codes | 009 |
| `LNS-TUI-001..099` | keyboard/accessibility flows, server toggle and lock behavior, responsive ownership transitions | 010 |
| `LNS-FLUTTER-001..099` | desktop/mobile flows, no mobile server UI, lifecycle/offline behavior | 011 |
| `LNS-IOS-001..099` | Bonjour permission states/candidates, foreground-only behavior, no listener, and generation-scoped stop/restart callbacks | 011 |
| `LNS-ANDROID-001..099` | NSD permission states/candidates, foreground-only behavior, no listener, and generation-scoped stop/restart callbacks | 011 |
| `LNS-ABSENCE-001..099` | static absence from code/config/API/UI/tests/build/CI/dependencies/docs | 012 |
| `LNS-FUZZ-001..099` | address/codec/state-machine/pairing fuzz corpus and bounded fuzz execution | 013 |
| `LNS-PROCESS-001..099` | real client/server processes, adversarial network, concurrency, crash and resource bounds | 013 |
| `LNS-SECRET-001..099` | redaction across logs/errors/events/CLI/TUI/Flutter/native bridge | 013 |
| `LNS-INTEROP-001..099` | KDBX3 read/KDBX4 write, KeePassXC round trip, merged history | 013 |
| `LNS-MOBILE-SCENARIO-001..099` | real iPhone/iPad/Android phone/tablet against a separate CLI authority: native discovery, reject/accept pairing, import, startup/manual-only bidirectional sync, restart, address-churn reconnect, and revocation | 014, 021, 022 |

Every concrete new test must include one ID from its family in the test name,
adjacent comment, golden/scenario metadata, or harness case name. A behavior may
map to several tests, but no automatable DRD behavior may lack an ID and a test.

## Post-completion composite review

`LNS-REVIEW-001` through `LNS-REVIEW-011` are mapped one-for-one in
[`security-coverage.md`](security-coverage.md). The runtime suite now includes
live-session and dequeued-commit revocation; real pairing admission limits;
absolute slow-peer deadlines; guard-owned connection slots; file-backed,
one-frame-at-a-time fetches; wrong-key-first fallback; actual listener and
advertisement replacement; Flutter native-discovery-before-startup ordering;
and a real ephemeral TUI listener lifecycle. The removal checker owns the final
case-variant negative control.

The mobile scenario's startup assertion no longer calls the Rust candidate
injection API. It unlocks, invokes iOS Bonjour or Android NSD through the Dart
repository, validates returned routes in Rust, and then consumes the startup
attempt. Android pair/import and trusted reconnect also use only these
platform-discovered routes. Its API-matched authority emulator runs the real
CLI; an instrumentation-only registrar publishes the local CLI port without
choosing an address, while the shipping client resolves and connects directly
over emulator Wi-Fi. `LNS-DISC-006` separately exercises the real Rust desktop
publisher/browser pair. Simulator networking does not represent arbitrary
router multicast behavior; the physical desktop-to-Android observation remains
optional and non-gating. The API 29 emulator scenario disables its synthetic
cellular data because the legacy explicit Wi-Fi link otherwise gives cellular
and Wi-Fi duplicate same-subnet paths; this models a Wi-Fi-only device and does
not inject, select, or disclose a sync route. API 36 retains platform-default
network state on shared emulator Wi-Fi.

## Five-finding repair coverage contract (2026-09-10)

The review repairs use the established ID families above. A core helper test is
supporting evidence only when the escaped defect was in application
orchestration.

| Finding | Production seam and injected boundary | Observable regression evidence |
| --- | --- | --- |
| Pairing durability/recovery | Real `ClientPairingSession::confirm`; authority activates and withholds the final acknowledgement; a new `hidlins sync pair` or `sync import` process loads durable state | `LNS-PROCESS-002/003` require provisional state before restart, no early import file/registration, and active trust after authenticated recovery. `LNS-PAIR-005/006` are explicitly core-protocol support. |
| Pairing authority restart | A real `hidlins sync serve` process pairs a client, exits cleanly, and restarts from the same sealed registry before the client's first IK sync | `LNS-PROCESS-004` requires the restarted authority to admit the paired client from durable active trust rather than relying on the original process's in-memory authorization registry. |
| TUI cancellation ownership | `PairingRuntime` owner plus real lock/quit event paths; barrier-held workers and an established socket are canceled | `LNS-TUI-012..015` require join-before-return and capture release; `LNS-PAIR-007` requires live socket interruption. |
| Temporal discovery fallback | `AppSession::poll_local_discovery` receives separate batches; `LanTransport` sees a wrong first route and valid pinned route later | `LNS-DISCOVERY-006` retains both batches; `LNS-SERVER-006` authenticates only the later pinned authority. |
| Restarted sole authority transient | `LanTransport` connects to the only platform-discovered route after an authority restart; the first TCP connection drops before IK completes | `LNS-CLIENT-005` requires a second connection to the same route to authenticate and complete through the real server while the original absolute handshake deadline remains enforced by `LNS-CLIENT-001`. |
| Password/identity rotation | Password-change test process is terminated at each durable phase; a fresh `AppSession` performs startup recovery | `LNS-IDENTITY-004..011` require an openable KDBX, matching unwrapped identity, idempotence, unrelated-record preservation, and fail-closed corrupt/missing artifacts. |
| Native discovery generations | iOS production service receives retained timer/browser callbacks from attempt A after B starts; Android exercises the token claimed by every guarded callback | `LNS-IOS-012..014` require no cross-generation mutation and exact-once completion. `LNS-ANDROID-012` pins the token invariant and the emulator matrix compiles/runs the callback wiring. |

## Optional non-blocking observation residue

The following IDs are not substitutes for automation and never block acceptance.
They cover only behavior
that cannot be represented faithfully without a person, physical device, or
representative multicast network:

| ID | Observation |
| --- | --- |
| `LNS-MANUAL-001` | two humans compare and reject/accept displayed SAS |
| `LNS-MANUAL-002` | physical iOS local-network permission presentation and recovery |
| `LNS-MANUAL-003` | physical Android nearby/local-network permission presentation and recovery |
| `LNS-MANUAL-004` | representative router/Wi-Fi multicast discovery across DHCP change |
| `LNS-MANUAL-006` | VoiceOver, TalkBack, and terminal assistive-technology output/action reachability |

Task 014 records literal simulator/emulator results in
`build/verification/mobile-local-sync/`. Foreground/background/termination
lifecycle is no longer a manual requirement: controlled native, Flutter, and
real mobile-process scenarios cover it. Missing automated evidence fails the
scenario target; missing optional observations do not block completion.
