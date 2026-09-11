# Final Work-Package Evidence

## Work package

- ID: `2026-09-05-local-network-sync`
- Title: Local Network Sync
- Approved plan SHA-256: `23244f24f5d8ab98e31995cefe96b468964cb0698b008717e3631656e9bc272b`
- Final acceptance attempt: 2 of 2

## Original objective

Completely remove the unshipped S3-compatible sync implementation and replace it across the Rust core, CLI, TUI, Flutter desktop, iOS, and Android with a secure, test-forward, local-network-only mechanism. The replacement must use one authoritative server, explicit pairing and revocation, secure automatic discovery, pinned Noise authentication, an exhaustive no-override local-address allowlist, startup-then-manual scheduling, explicit process-lifetime desktop/terminal serving, client-only mobile behavior, and no S3 migration path.

## Completed tasks

1. Task 001 — characterize and freeze the design — `evidence/001.md`
2. Task 002 — patched Snow and secure sessions — `evidence/002.md`
3. Task 003 — address, framing, and protocol — `evidence/003.md`
4. Task 004 — identity, trust, pairing, and revocation — `evidence/004.md`
5. Task 005 — secure automatic discovery — `evidence/005.md`
6. Task 006 — authoritative server — `evidence/006.md`
7. Task 007 — client sync and pair/import — `evidence/007.md`
8. Task 008 — application API and lifecycle — `evidence/008.md`
9. Task 009 — CLI surface — `evidence/009.md`
10. Task 010 — TUI surface — `evidence/010.md`
11. Task 011 — Flutter and mobile platforms — `evidence/011.md`
12. Task 012 — complete S3 eradication and dependency cleanup — `evidence/012.md`
13. Task 013 — security, fuzz, NCSA, real-process, and platform automation — `evidence/013.md`
14. Task 014 — automated mobile simulator scenarios — `evidence/014.md`
15. Task 015 — documentation and release review — `evidence/015.md`

## Whole-package review

The primary thread re-read the approved request, plan, all task documents, all evidence, and reviewed the cumulative change from baseline `e6df55b84edb53649d4f7f6c36842803c08884d8`. The integrated review covered scope completeness, cross-task contracts, S3 deletion, protocol state and bounds, cryptographic integration, zeroization, address enforcement at every socket boundary, discovery privacy, pairing/trust/revocation, concurrency and CAS behavior, atomic storage and KDBX validation, secret-free errors, FFI/UI boundaries, lifecycle scheduling, mobile client-only enforcement, dependency/vendor provenance, NCSA isolation, artifact scans, automated acceptance evidence, and documentation consistency.

No unresolved high-confidence defect remains. `git diff --check`, the frozen plan hash, task/evidence consistency, S3 absence, and production dependency/artifact boundaries all pass. Optional physical/human observations are not represented as automated proof.

## High-confidence findings fixed

- The final `make verify` exposed a nondeterministic KeePassXC TOTP interop comparison that computed two expected windows before invoking the external Argon2id operation. The failing result was recorded, and `tools/interop-tests/entry_us-012.sh` now brackets the external invocation and compares only against the bounded set of TOTP counters that actually existed during that interval. Shell syntax, focused interop validation, and the complete restarted final gate pass.

## Final acceptance criteria

| Criterion | Evidence |
| --- | --- |
| 1. No active S3/AWS/SigV4/MinIO path or migration remains. | `make s3-removal-check` and `make verify` pass; Task 012 evidence records source, API, UI, CI, test, dependency, and vendor deletion. |
| 2. One authority supports named/revocable clients and clients pin one server. | Pairing/trust, server, client, API, CLI/TUI/Flutter tests and all four Task 014 device scenarios pass. |
| 3. Discovery handles DHCP changes without conferring trust. | Secure-discovery DHCP/spoof/stale-policy tests pass; reconnect remains pinned. |
| 4. Only the exact local address allowlist is accepted, with socket rechecks and no override. | Complete prefix/property tests, mapped/scope tests, client/server socket checks, CLI negative tests, and scenario-route validation pass. |
| 5. Fixed XX pairing, SAS, window/failure bounds, bilateral commit, and failed-path privacy hold. | Noise, pairing transaction, fault-boundary, pre-auth protocol, and mobile rejected/accepted pairing tests pass. |
| 6. Fixed IK reconnect is mutually pinned and rejects mismatch/revocation/replay/downgrade. | Noise and trust suites plus restart/revocation device scenarios pass. |
| 7. Identity secrets are sealed, context-bound, zeroized, and rewrapped without rotation. | Identity, password-change, source-patch, wrapper, and debug-redaction tests pass. |
| 8. Protocol framing/state/resource limits fail closed. | Protocol boundary/corpus/fuzz tests and exhaustion/cancellation server tests pass. |
| 9. Merge, history, backup, CAS retry, KDBX validation, and atomic writes remain intact. | Merge/property/fault/interop/server/client suites and KeePassXC gates pass. |
| 10. CLI implements one-shot sync and foreground `Ctrl+C` serving. | CLI command-tree/JSON/process tests pass, including signal cleanup and pre-operation/offline behavior. |
| 11. TUI and desktop expose explicit process-lifetime serving, off by default. | Deterministic TUI journeys, API lifecycle tests, and Flutter surface tests pass. |
| 12. iOS/Android are foreground clients only. | Rust rejection, UI contract, manifest/channel, native, and installed-app scenarios prove no mobile server path. |
| 13. Startup attempts once and later sync is manual-only, never post-save. | Core/API/CLI/TUI/Flutter scheduling and save-without-sync scenario steps pass. |
| 14. Dependencies remain thin, reviewed, exact, vendored, and permissively licensed. | Dependency dossiers, vendor patch reproduction, `make deny`, `make audit`, and offline locked gates pass. |
| 15. NCSA remains exact and development/test-only. | Production graph negative controls, packaging hooks, and scans of CLI/TUI/agent/iOS/Android artifacts pass. |
| 16. Security/adversarial verification is automated and isolated behind discoverable Make targets. | `make verify`, fuzz-corpus/security/discovery/integration targets, CI wiring, and acceptance checker pass. |
| 17. Real mobile client sync is automated against a separate CLI authority. | Fresh iPhone, iPad, API 29 phone, and API 36 tablet evidence each reports PASS for all 11 steps. |
| 18. Every frozen gate command passes and the implementation stays in approved scope. | Final command table below and integrated cumulative review. |

## Final quality gate

| Command | Result |
| --- | --- |
| `make verify` | PASS |
| `make ncsa-boundary-check` | PASS |
| `make check-ios` | PASS |
| `make app-test-ios-simulator` | PASS |
| `make app-build-ios` | PASS — simulator and no-codesign device artifacts scanned clean |
| `make android-emulator-provision` | PASS — required API/ABI/form-factor matrix present |
| `make test-local-sync-mobile-scenarios HIDLINS_ANDROID_STRICT=1` | PASS — iPhone, iPad, Android API 29 phone, Android API 36 tablet; 11/11 steps each |

The first final-gate attempt stopped at the pre-existing TOTP interop race described above. The entire final gate was restarted after repair. A sandbox-only Flutter SDK cache denial required identical iOS commands to run with access to the installed SDK cache; no project assertion failed. The definitive commands above then passed in frozen order.

## Cumulative diff

- Replaced S3/SigV4/HTTP credential/configuration code with safe-Rust local endpoint policy, bounded framing/protocol, patched Snow XX/IK sessions, sealed identity/trust state, pairing, discovery, authoritative server, client transport, and generic orchestration.
- Replaced every CLI, TUI, Flutter desktop, iOS, and Android S3 surface with the approved local-sync lifecycle and client/server topology.
- Removed obsolete S3 tests, MinIO harnesses, CI/Make targets, exclusive dependencies, and their unreachable vendored graph; added narrowly reviewed Snow and optional desktop mDNS dependencies.
- Added adversarial, property, fuzz-corpus, fault, process, native-platform, artifact-boundary, and installed simulator/emulator scenario automation plus machine-readable evidence.
- Added and enforced the exact `libfuzzer-sys` NCSA development/test-only exception without placing NCSA in any application dependency graph or artifact.
- Updated active repository and notebook requirements, verification scenarios, operator/developer documentation, protocol/threat/security records, and non-blocking residual observations.

## Remaining non-blocking concerns

- Simulator automation cannot establish exact physical-device permission wording, representative consumer-router multicast behavior, or human-perceived screen-reader speech. These remain optional confidence observations with explicit procedures; they do not block completion and are not reported as passed.
- Flutter reports that the local Rust bridge plugin does not yet support Swift Package Manager. This is a future Flutter compatibility notice, not a current build, runtime, security, or acceptance failure.
