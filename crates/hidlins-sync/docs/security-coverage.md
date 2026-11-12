# Local-network sync automated security coverage

This is the executable coverage index for the approved local-network-sync DRD.
`LNS-*` identifiers are stable test IDs embedded next to Rust tests. Every
command below is exposed through the top-level Makefile.

## Requirements matrix

| Requirement | Stable automated evidence | CI Make gate |
|---|---|---|
| FR-001 | `tools/dev/s3-removal-check.py` negative controls and active-tree scan | `s3-removal-check` |
| FR-002 | LNS-TRUST-002, LNS-TRUST-004, LNS-SERVER-007 | `test-local-sync-security`, `test-local-sync-integration` |
| FR-003 | LNS-DISCOVERY-001/002/004/006 plus the real desktop `LNS-DISC-006` publisher/browser exchange | `test-local-sync-discovery` |
| FR-004 | LNS-ADDR-001 through 005, LNS-DISCOVERY-002/003/005, LNS-PROCESS-001 | all three local-sync gates |
| FR-005 | LNS-NOISE-001/002/005, LNS-PAIRING-001/002, LNS-PAIR-004..007, LNS-PROCESS-002/003, LNS-TRUST-001/002 | `test-local-sync-security`, `test-local-sync-integration` |
| FR-006 | LNS-NOISE-003/004/005, LNS-TRUST-002/003, LNS-SERVER-003/006 | `test-local-sync-security`, `test-local-sync-integration` |
| FR-007 | LNS-IDENTITY-001..011, LNS-NOISE source-patch unit tests, LNS-LIFECYCLE-001 | `test-local-sync-security`, `test-local-sync-integration` |
| FR-008 | LNS-FRAME-001, LNS-PROTO-001 through 004, LNS-STATE-001 through 003, LNS-LIMIT-001, LNS-FUZZ-CORPUS-001 through 003 | `test-local-sync-security`, `fuzz-local-sync-corpus`, `fuzz-local-sync-ci` |
| FR-009 | LNS-SERVER-001/004/005/007, LNS-MERGE-FAULT-001 through 004, merge property tests | `test-local-sync-security`, `test-local-sync-integration`, `test-merge-properties`, `interop-sync` |
| FR-010 | LNS-CLIENT-002, `us_094_sync_api::startup_sync_is_once_and_local_failures_do_not_block_use`, CLI/TUI/Rust API lifecycle tests | `test-local-sync-security`, `test-api`, `check` |
| FR-011 | LNS-PROCESS-001, LNS-LIFECYCLE-001, TUI sync journey tests, Rust API server lifecycle integration | `test-local-sync-integration`, `test-local-sync-security`, `test-api` |
| FR-012 | Rust client-only native API listener refusal and lifecycle tests | `test-api`, `test-api-native` |
| FR-013 | LNS-CLIENT-003/004, LNS-PROCESS-001/003, API bilateral-absence assertions, CLI/TUI/Rust API import contracts | `test-local-sync-integration`, `test-api` |
| FR-014 | `cli_sync`, TUI local-sync journeys, `us_094_sync_api`, Rust API sync tests | `test-local-sync-security`, `test-tui-contracts`, `test-api` |
| FR-015 | LNS-DISCOVERY-005 and Rust permission/candidate port tests | `test-local-sync-discovery`, `test-api` |
| FR-016 | LNS-ADDR-003/004, LNS-DISCOVERY-005, CLI/TUI/Rust API manual-route tests | `test-local-sync-security`, `test-local-sync-discovery`, `test-api` |
| FR-017 | LNS-NOISE-006, LNS-PROTO-004, LNS-SERVER-003/005, CLI/API/TUI redaction assertions | `test-local-sync-security`, `test-api` |
| NFR-001 | This matrix, the threat model, Snow patch guard, NCSA boundary negative controls, and final focused review | `verify`, `ncsa-boundary-check` |
| NFR-002 | exact Snow graph test plus vendored patch hash/zeroization negative controls in `test_vendor_patches.py` | `test-local-sync-security`, `vendor-patches` |
| NFR-003 | isolated fuzz dependency dossier; production and fuzz license policies | `deny`, `audit`, `ncsa-boundary-check` |
| NFR-004 | Retired cloud-object reachability and lock/vendor removal assertions | `s3-removal-check`, `deny` |
| NFR-005 | crate-level `forbid(unsafe_code)` plus all parser/fuzz targets | `check`, `fuzz-local-sync-ci` |
| NFR-006 | LNS-LIMIT-001, LNS-DISCOVERY-002, LNS-SERVER-002/005, LNS-CLIENT-001, `finished_connection_workers_are_reaped_while_server_is_running` | all three local-sync gates |
| NFR-007 | Adversarial matrix below | all local-sync, platform, and fuzz gates |
| NFR-008 | immutable workflow evidence 001–014 records fail-before/pass-after or characterization evidence | feature-workflow evidence validation |
| NFR-009 | address/frame/merge properties and three fuzz targets with committed corpus replay | `test-merge-properties`, `fuzz-local-sync-corpus`, `fuzz-local-sync-ci` |
| NFR-010 | LNS-PROCESS-001 on loopback and Linux private IPv4/IPv6 namespaces; adapter simulations elsewhere | `test-local-sync-integration`, platform gates |
| NFR-011 | server upload validation, merge faults/properties, KeePassXC round trips | `test-local-sync-security`, `interop`, `interop-entry`, `interop-sync` |
| NFR-012 | Rust workspace and native API cross-target checks | `check-macos`, `test-api` |
| NFR-013 | LNS-CLIENT-001, LNS-SERVER-002, LNS-PROCESS-001, lifecycle cancellation tests | `test-local-sync-security`, `test-local-sync-integration` |
| NFR-014 | LNS-NOISE-006, LNS-PROTO-004, Rust API, TUI and CLI JSON redaction | `test-local-sync-security`, `test-api`, `test-console` |

## Adversarial and resilience matrix

| Attack/failure class | Stable evidence |
|---|---|
| MITM, key substitution, wrong pin, cross-mode downgrade | LNS-NOISE-003/004/005, LNS-TRUST-003, LNS-SERVER-006 |
| Discovery spoof, metadata correlation, candidate flood, DHCP/interface churn | LNS-DISCOVERY-001 through 006 and Rust desktop publisher/browser exchange |
| Public/special address and mapped/scope bypass | LNS-ADDR-001 through 005, LNS-DISCOVERY-002/003/005 |
| Replay, reorder, duplicate, truncate, malformed record/frame, invalid commit order | LNS-FRAME-001, LNS-PROTO-001/002/003, LNS-STATE-001/002/003 and all fuzz targets |
| Pairing rejection, expiry, failed acknowledgement, transcript/role/key substitution | LNS-PAIRING-001/002, LNS-PAIR-004..007, LNS-PROCESS-002/003, LNS-TRUST-001/002 |
| Revocation and authorization oracle probing | LNS-TRUST-002/003, LNS-SERVER-003 |
| Frame/chunk/vault/allocation/connection/queue exhaustion | LNS-LIMIT-001, LNS-DISCOVERY-002, LNS-SERVER-002/005 |
| Timeout, cancellation, disconnect, server shutdown/lock/restart | LNS-CLIENT-001/005, LNS-SERVER-002/004, LNS-LIFECYCLE-001, LNS-PROCESS-001..004, LNS-TUI-012..015 |
| Multi-client CAS and registry/local-write races | LNS-SERVER-007, LNS-TRUST-004, API concurrent sync/lock tests |
| Interrupted pairing, unauthorized/invalid import, and upload/commit crash boundaries | LNS-PROCESS-002/003, LNS-CLIENT-004, LNS-SERVER-004, LNS-MERGE-FAULT-001 through 004 |
| Secret leakage through debug/error/JSON/event/process output | LNS-NOISE-006, LNS-PROTO-004, LNS-SERVER-003/005 and surface redaction contracts |
| Panic containment | fuzz targets, LNS-MERGE-FAULT-002/003, API `sync_panic_is_contained_locks_and_emits_one_terminal_failure` |
| NCSA/libFuzzer production or artifact contamination | `ncsa-boundary-check.py` self-test fixtures for manifest, lock, policy, hooks, Rust binaries, API static/shared libraries, directory bundles and ZIP archives |

## Round 2 composite-review regression map

These tests close the eleven accepted review findings at the runtime or
application boundary where each defect occurred. Isolated state-model tests
remain supporting evidence, not substitutes for this table.

| ID | Repaired defect | Automated runtime/surface evidence |
| --- | --- | --- |
| `LNS-REVIEW-001` | Revoked peers retained authenticated sessions or queued commits | `trust_replacement_closes_only_removed_live_peers`, `revocation_cancels_a_dequeued_commit_before_execution` |
| `LNS-REVIEW-002` | Native and Rust service names diverged | `service_type_constants_match_the_protocol_contract` |
| `LNS-REVIEW-004` | TUI passed port zero to a connectable endpoint | `explicit_tui_server_binds_a_real_ephemeral_port_and_stops`, bind-type address-policy test |
| `LNS-REVIEW-005` | Live pairing omitted candidate/failure/window enforcement | `runtime_admits_one_pairing_candidate_and_close_cancels_it`, `three_failed_xx_handshakes_close_pairing_before_a_fourth_attempt` |
| `LNS-REVIEW-006` | Blocking socket I/O escaped absolute deadlines | `slow_preface_peer_is_closed_by_the_noise_handshake_deadline`, `client_noise_read_is_bounded_by_the_absolute_handshake_deadline` |
| `LNS-REVIEW-007` | Rejected connections could corrupt capacity accounting | `connection_capacity_recovers_after_rejection_and_disconnect` |
| `LNS-REVIEW-008` | Fetch duplicated a vault-sized message collection | `file_backed_fetch_snapshot_remains_coherent_after_live_path_replacement` plus bounded-frame integration coverage |
| `LNS-REVIEW-009` | A wrong-key first route prevented valid fallback | `client_falls_back_from_forged_route_but_never_accepts_a_wrong_pin` |
| `LNS-REVIEW-010` | Authority listener/advertisement stayed stale after interface change | `listener_refresh_rebinds_to_an_injected_allowed_snapshot`, discovery DHCP/cache tests |
| `LNS-REVIEW-011` | Retired-transport removal matching missed case variants | removal check self-test exact uppercase and mixed-case negative controls |

## Five-finding repair map (2026-09-10)

This table is the current coverage contract for the five later review findings.
It supersedes any historical evidence text that described a directly invoked
core helper as application-orchestration coverage.

| Finding | Production entry point | Injected failure or ordering | Observable assertion | Owning test |
| --- | --- | --- | --- | --- |
| Pairing can strand a client after server activation | `ClientPairingSession::confirm`, followed by a newly spawned `hidlins sync pair` or `hidlins sync import` command | The authority activates the exact client, then deliberately withholds `PAIR_ACTIVATED` | Restart sees sealed provisional state; import has no KDBX or registration; the command chooses authenticated IK recovery and ends active; pending import state clears only after valid KDBX registration | `LNS-PROCESS-002/003` in `cli_local_sync_process.rs` |
| Active authority trust is only in memory | A real `hidlins sync serve` process and a paired client's first IK sync | The authority process exits after pairing and restarts from the same sealed registry before any trusted sync | The restarted process admits the client and completes authenticated sync using persisted active trust | `LNS-PROCESS-004` in `cli_local_sync_process.rs` |
| TUI lock/cancel can detach secret-bearing pairing work | `PairingRuntime::cancel` and `App`'s Ctrl+L/Ctrl+Q handlers | Barrier-held owned worker; operation replacement; real established pairing socket | Cancellation joins before return, prevents the modeled late mutation, drops the worker capture, and shuts down blocking socket I/O | `LNS-TUI-012..015`; `LNS-PAIR-007` |
| First discovery batch can hide a later valid authority | `AppSession::poll_local_discovery` and `LanTransport` candidate traversal | Wrong/stale route arrives in batch one; valid pinned route arrives in batch two | API returns the accumulated bounded set and authenticated sync succeeds only through the later pinned authority | `LNS-DISCOVERY-006`; `LNS-SERVER-006` (`client_falls_back_from_forged_route_but_never_accepts_a_wrong_pin`) |
| Restarted sole authority can lose one handshake | `LanTransport::connect` through the platform-discovered route | The first TCP connection to the only route drops before IK completes; the second reaches the production server | The same operation retries once, authenticates the pinned authority, and still satisfies the absolute handshake-deadline test | `LNS-CLIENT-005` (`client_retries_one_discovered_authority_after_a_transient_handshake_failure`) plus `LNS-CLIENT-001` |
| Password change can split KDBX and identity credentials | Password-change subprocess and `AppSession` startup recovery | Process termination at every durable stage/rename/commit boundary | Restart yields an openable KDBX and unwrap-able unchanged public identity; repeat recovery is a no-op; corrupt/missing stages fail closed | `LNS-IDENTITY-004..011` in `us_095_bootstrap_change_password.rs` |

## Optional non-blocking observations

Human comparison and deliberate rejection of SAS values, consumer-router
multicast behavior, physical lifecycle scheduling and terminal screen-reader
speech supplement deterministic Rust verification. They do not replace
address policy, authentication, storage or listener-refusal tests. The Rust
mDNS publisher/browser exchange and owned-process suite establish the retained
implementation boundaries without claiming arbitrary router behavior.

See [the verification procedure](../../../docs/local-network-sync-manual-verification.md).
