# Local-network sync automated security coverage

This is the executable coverage index for the approved local-network-sync DRD.
`LNS-*` identifiers are stable test IDs embedded next to Rust tests; Flutter
and native-platform IDs use stable file/test names where those frameworks do
not support metadata IDs. Every command below is exposed through the top-level
Makefile and is run by CI directly or through `make verify`.

## Requirements matrix

| Requirement | Stable automated evidence | CI Make gate |
|---|---|---|
| FR-001 | `tools/dev/s3-removal-check.py` negative controls and active-tree scan | `s3-removal-check` |
| FR-002 | LNS-TRUST-002, LNS-TRUST-004, LNS-SERVER-007 | `test-local-sync-security`, `test-local-sync-integration` |
| FR-003 | LNS-DISCOVERY-001/002/004 plus the real desktop `mdns-sd` publisher/browser exchange | `test-local-sync-discovery` |
| FR-004 | LNS-ADDR-001 through 005, LNS-DISCOVERY-002/003/005, LNS-PROCESS-001 | all three local-sync gates |
| FR-005 | LNS-NOISE-001/002/005, LNS-PAIRING-001/002, LNS-TRUST-001/002 | `test-local-sync-security` |
| FR-006 | LNS-NOISE-003/004/005, LNS-TRUST-002/003, LNS-SERVER-003/006 | `test-local-sync-security`, `test-local-sync-integration` |
| FR-007 | LNS-IDENTITY-001/002/003, LNS-NOISE source-patch unit tests, LNS-LIFECYCLE-001 | `test-local-sync-security` |
| FR-008 | LNS-FRAME-001, LNS-PROTO-001 through 004, LNS-STATE-001 through 003, LNS-LIMIT-001, LNS-FUZZ-CORPUS-001 through 003 | `test-local-sync-security`, `fuzz-local-sync-corpus`, `fuzz-local-sync-ci` |
| FR-009 | LNS-SERVER-001/004/005/007, LNS-MERGE-FAULT-001 through 004, merge property tests | `test-local-sync-security`, `test-local-sync-integration`, `test-merge-properties`, `interop-sync` |
| FR-010 | LNS-CLIENT-002, `us_094_sync_api::startup_sync_is_once_and_local_failures_do_not_block_use`, CLI/TUI/Flutter lifecycle tests | `test-local-sync-security`, `app-check`, `check` |
| FR-011 | LNS-PROCESS-001, LNS-LIFECYCLE-001, TUI sync journey tests, Flutter desktop lifecycle integration | `test-local-sync-integration`, `test-local-sync-security`, `app-check` |
| FR-012 | LNS-MOBILE-001, iOS `RunnerTests` listener/background assertions, Android `HidlinsAndroidTest` client-only assertions, Flutter mobile control-absence widgets, and platform-discovered real mobile/CLI scenarios | `test-local-sync-security`, `check-ios`, `app-test-ios-simulator`, `check-android`, `app-test-android-emulator`, `test-local-sync-mobile-scenarios` |
| FR-013 | LNS-CLIENT-003/004, LNS-PROCESS-001, CLI/TUI/Flutter import contracts | `test-local-sync-integration`, `app-check` |
| FR-014 | `cli_sync`, TUI local-sync journeys, `us_094_sync_api`, Flutter `sync_test.dart` and bridge tests | `test-local-sync-security`, `test-tui-contracts`, `app-check` |
| FR-015 | LNS-DISCOVERY-005, Flutter platform-service and permission-state tests, iOS/Android native adapter tests | `test-local-sync-discovery`, `app-check`, `check-ios`, `check-android` |
| FR-016 | LNS-ADDR-003/004, LNS-DISCOVERY-005, CLI/TUI/Flutter manual-route tests | `test-local-sync-security`, `test-local-sync-discovery`, `app-check` |
| FR-017 | LNS-NOISE-006, LNS-PROTO-004, LNS-SERVER-003/005, CLI/API/Flutter redaction assertions | `test-local-sync-security`, `app-check` |
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
| NFR-012 | host cross-target checks plus Flutter desktop/iOS/Android jobs | `check-macos`, `check-ios`, `check-android`, `app-check` |
| NFR-013 | LNS-CLIENT-001, LNS-SERVER-002, LNS-PROCESS-001, lifecycle cancellation tests | `test-local-sync-security`, `test-local-sync-integration` |
| NFR-014 | LNS-NOISE-006, LNS-PROTO-004, platform/CLI JSON redaction and repository telemetry scan | `test-local-sync-security`, `telemetry-check`, `app-check` |

## Adversarial and resilience matrix

| Attack/failure class | Stable evidence |
|---|---|
| MITM, key substitution, wrong pin, cross-mode downgrade | LNS-NOISE-003/004/005, LNS-TRUST-003, LNS-SERVER-006 |
| Discovery spoof, metadata correlation, candidate flood, DHCP/interface churn | LNS-DISCOVERY-001 through 004; Android API 29/36 replacement-authority scenarios |
| Public/special address and mapped/scope bypass | LNS-ADDR-001 through 005, LNS-DISCOVERY-002/003/005 |
| Replay, reorder, duplicate, truncate, malformed record/frame, invalid commit order | LNS-FRAME-001, LNS-PROTO-001/002/003, LNS-STATE-001/002/003 and all fuzz targets |
| Pairing rejection, expiry, failed acknowledgement, transcript/role/key substitution | LNS-PAIRING-001/002, LNS-TRUST-001/002 |
| Revocation and authorization oracle probing | LNS-TRUST-002/003, LNS-SERVER-003 |
| Frame/chunk/vault/allocation/connection/queue exhaustion | LNS-LIMIT-001, LNS-DISCOVERY-002, LNS-SERVER-002/005 |
| Timeout, cancellation, disconnect, server shutdown/lock/restart | LNS-CLIENT-001, LNS-SERVER-002/004, LNS-LIFECYCLE-001, LNS-PROCESS-001 |
| Multi-client CAS and registry/local-write races | LNS-SERVER-007, LNS-TRUST-004, API concurrent sync/lock tests |
| Pair/upload/commit crash boundaries and partial import | LNS-TRUST-001, LNS-SERVER-004, LNS-CLIENT-004, LNS-MERGE-FAULT-001 through 004 |
| Secret leakage through debug/error/JSON/event/process output | LNS-NOISE-006, LNS-PROTO-004, LNS-SERVER-003/005 and surface redaction contracts |
| Panic containment | fuzz targets, LNS-MERGE-FAULT-002/003, API `sync_panic_is_contained_locks_and_emits_one_terminal_failure` |
| NCSA/libFuzzer production or artifact contamination | `ncsa-boundary-check.py` self-test fixtures for manifest, lock, policy, hooks, raw binaries, directory bundles, APK, and IPA |

## Round 2 composite-review regression map

These tests close the eleven accepted review findings at the runtime or
application boundary where each defect occurred. Isolated state-model tests
remain supporting evidence, not substitutes for this table.

| ID | Repaired defect | Automated runtime/surface evidence |
| --- | --- | --- |
| `LNS-REVIEW-001` | Revoked peers retained authenticated sessions or queued commits | `trust_replacement_closes_only_removed_live_peers`, `revocation_cancels_a_dequeued_commit_before_execution` |
| `LNS-REVIEW-002` | Native and Rust service names diverged | `native_service_type_constants_match_the_rust_contract`, iOS/Android native gates |
| `LNS-REVIEW-003` | Flutter consumed startup before discovery | API `startup_sync_runs_once_then_only_manual_and_never_post_save`, Flutter unlock/bridge ordering tests, mobile scenario startup after native discovery |
| `LNS-REVIEW-004` | TUI passed port zero to a connectable endpoint | `explicit_tui_server_binds_a_real_ephemeral_port_and_stops`, bind-type address-policy test |
| `LNS-REVIEW-005` | Live pairing omitted candidate/failure/window enforcement | `runtime_admits_one_pairing_candidate_and_close_cancels_it`, `three_failed_xx_handshakes_close_pairing_before_a_fourth_attempt` |
| `LNS-REVIEW-006` | Blocking socket I/O escaped absolute deadlines | `slow_preface_peer_is_closed_by_the_noise_handshake_deadline`, `client_noise_read_is_bounded_by_the_absolute_handshake_deadline` |
| `LNS-REVIEW-007` | Rejected connections could corrupt capacity accounting | `connection_capacity_recovers_after_rejection_and_disconnect` |
| `LNS-REVIEW-008` | Fetch duplicated a vault-sized message collection | `file_backed_fetch_snapshot_remains_coherent_after_live_path_replacement` plus bounded-frame integration coverage |
| `LNS-REVIEW-009` | A wrong-key first route prevented valid fallback | `client_falls_back_from_forged_route_but_never_accepts_a_wrong_pin` |
| `LNS-REVIEW-010` | Authority listener/advertisement stayed stale after interface change | `listener_refresh_rebinds_to_an_injected_allowed_snapshot`, discovery DHCP/cache tests |
| `LNS-REVIEW-011` | Retired-transport removal matching missed case variants | removal check self-test exact uppercase and mixed-case negative controls |

## Optional non-blocking observation residue

Only observations that automation cannot faithfully establish remain optional:

- A person comparing and deliberately rejecting mismatched SAS values on two
  physical devices. Automated tests already prove identical derivation,
  mismatch rejection, bilateral confirmation, and every persistence boundary;
  they cannot prove human perception or intent.
- Physical iOS/Android local-network permission presentation and settings
  recovery. Simulator/emulator and native adapter tests cover all programmatic
  states, but OS-owned physical-device UI is not faithfully scriptable.
- Multicast discovery across representative consumer routers, VLANs, Wi-Fi
  isolation, and interface transitions. Automated real Rust desktop
  publisher/browser exchange and Android platform-NSD emulator scenarios cover
  the implementations without claiming third-party router behavior.
- Real-device foreground suspension/resume and cross-device lifecycle timing.
  Controlled lifecycle clocks prove state transitions; OS scheduling remains
  observational.
- VoiceOver/TalkBack/terminal screen-reader output. Semantics and keyboard
  reachability are automated, while assistive-technology speech is human
  observable.

These cases supplement automation and never block or substitute for a security,
address-policy, authorization, storage, or mobile-listener assertion. Real
simulator/emulator client scenarios are the application-level authority. On
Android, an API/ABI-matched authority emulator runs the real CLI and an
instrumentation-only registrar publishes only its port; the shipping app
resolves the private route through `NsdManager`, validates it in Rust, and
connects directly for Noise and sync. The startup phase does not pre-seed
Rust's candidate cache. This proves application/native orchestration on the
emulator network, but does not claim representative consumer-router multicast
behavior.
The copy-pasteable automated procedure and optional observation registry are in
[`../../../docs/local-network-sync-manual-verification.md`](../../../docs/local-network-sync-manual-verification.md).
