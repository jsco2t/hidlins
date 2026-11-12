# Local Sync Test Matrix

This matrix assigns stable identifiers to the preserved characterization suite
and the V1 security requirements. IDs remain stable when tests move files or
change transport terminology.

## Preserved characterization baseline

| Stable ID | Invariant | Current executable evidence |
| --- | --- | --- |
| `SYNC-STATE-001` | unchanged local and remote is already current | `us_043_disjoint_merge::merge_result_uploads_and_is_idempotent_on_resync` |
| `SYNC-STATE-002` | local-only change conditionally pushes | `us_042_conditional_put::local_changed_remote_unchanged_pushes_conditionally` |
| `SYNC-STATE-003` | remote-only change fast-replaces and creates backup | `us_041_fast_replace_open::remote_advanced_local_unchanged_fast_replaces` |
| `SYNC-STATE-004` | both changed merges and conditionally commits | `tests/us_043_disjoint_merge.rs`, `tests/us_044_collision_merge.rs` |
| `SYNC-CAS-001` | failed conditional commits re-fetch/re-merge within a bound | `sync_retry_delta_regressions::successful_retry_reports_imports_from_both_attempts` and `retry_delta_cancels_an_import_removed_by_the_next_authority_version` |
| `SYNC-CAS-002` | retry exhaustion is explicit and preserves backup | `preserved_contract_characterization::default_cas_exhausts_after_two_real_races_and_preserves_original_backup` |
| `SYNC-KDF-001` | incompatible KDF aborts before replacement | `preserved_contract_characterization::fast_replace_rejects_same_vault_kdf_mismatch_without_replacing_local_bytes` and `diverged_merge_rejects_same_vault_kdf_mismatch_and_keeps_pre_merge_backup` |
| `SYNC-ID-001` | a different logical vault cannot replace the open vault | `hidlins-core` `Vault::replace_database` UUID tests and sync fast-replace tests |
| `SYNC-PTR-001` | successful divergence pointers persist together | `preserved_contract_characterization::successful_divergence_returns_both_current_pointers_for_noop_resync` and `local_configuration_persists_both_distinct_divergence_versions_together`; `client::sync_vault_with_cancellation` persists the returned pair |
| `SYNC-OFFLINE-001` | remote failure leaves local bytes and backup policy intact | `us_045_sync_failure::remote_unreachable_leaves_local_vault_intact` |
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

The current default CAS budget is two total commit attempts: the initial
attempt plus one retry. `client_policy` asserts that default, and the exhaustion
characterization drives two real stale-version races through the state machine.
Run the focused retained-contract evidence with
`make test-sync-preserved-contracts`. The full test suite also runs these cases.

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
| `LNS-ABSENCE-001..099` | static absence from code/config/API/UI/tests/build/CI/dependencies/docs | 012 |
| `LNS-FUZZ-001..099` | address/codec/state-machine/pairing fuzz corpus and bounded fuzz execution | 013 |
| `LNS-PROCESS-001..099` | real client/server processes, adversarial network, concurrency, crash and resource bounds | 013 |
| `LNS-SECRET-001..099` | redaction across logs/errors/events/CLI/TUI/Rust API | 013 |
| `LNS-INTEROP-001..099` | KDBX3 read/KDBX4 write, KeePassXC round trip, merged history | 013 |

Every concrete new test must include one ID from its family in the test name,
adjacent comment, golden/scenario metadata, or harness case name. A behavior may
map to several tests, but no automatable DRD behavior may lack an ID and a test.

## Post-completion composite review

`LNS-REVIEW-001` through `LNS-REVIEW-011` are mapped one-for-one in
[`security-coverage.md`](security-coverage.md). The runtime suite now includes
live-session and dequeued-commit revocation; real pairing admission limits;
absolute slow-peer deadlines; guard-owned connection slots; file-backed,
one-frame-at-a-time fetches; wrong-key-first fallback; actual listener and
advertisement replacement;
and a real ephemeral TUI listener lifecycle. The removal checker owns the final
case-variant negative control.

`LNS-DISC-006` exercises the real Rust desktop publisher/browser exchange.
The Rust API retains candidate validation, permission-state and lifecycle
contracts for native consumers. See [security-coverage.md](security-coverage.md)
for the current automated coverage and optional physical observations.
