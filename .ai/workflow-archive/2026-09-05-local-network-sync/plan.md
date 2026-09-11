# Local Network Sync

## Objective

Replace Hidlins' unshipped S3-compatible synchronization implementation with a secure local-network-only system across the Rust core, CLI, TUI, Flutter desktop, iOS, and Android surfaces. The completed package has one authoritative server per vault, multiple explicitly paired clients, automatic DNS-SD/mDNS discovery, fixed Noise XX/IK authentication, strict private/link-local/loopback address enforcement, pair-and-import bootstrap, startup-then-manual sync scheduling, explicit process-lifetime desktop/terminal serving, foreground-client-only mobile behavior, and comprehensive automated adversarial verification.

The outcome contains no active S3 code, configuration, migration, compatibility, credentials, UI, harness, dependency, or documentation path. Existing KDBX interoperability, three-way merge, loser-as-history preservation, backups, advisory locking, atomic writes, and offline-first behavior remain release gates.

## Current behavior

- `crates/hidlins-sync` combines a reusable blocking `SyncTransport`/`Sync` truth table and merge engine with S3 configuration, four AWS credential sources, RST-CRED-1 credential encryption, a hand-written SigV4 implementation, `ureq`/rustls HTTP, ETag handling, and MinIO integration tests.
- The reusable sync state machine implements already-current, push, fast-replace, and merge/CAS-retry paths. It reads and hashes the encrypted local KDBX, validates fetched databases with the master password and keyfile, checks KDF compatibility, creates `.kdbx.bak`, preserves merge losers as KDBX history, and updates registry divergence pointers.
- The registry stores a S3-shaped `[vault.sync]` table in `RegisteredVault::extra`, including `kind = "s3"`, S3 target/credential data, `last_synced_remote_etag`, and `last_synced_local_sha256`.
- A `Vault` owns an exclusive advisory lock for its full unlocked lifetime. TUI and API sync workers preserve single ownership by moving the vault and registry out of UI/session state during background work. Any server commit path must preserve that ownership model instead of opening or mutating the same vault concurrently.
- The Rust application API exposes S3 DTOs, configure/bootstrap functions, a three-phase background sync worker, sync events, and automatic post-save synchronization from every entry mutation.
- The CLI exposes `hidlins sync` plus `vault set-sync` S3 arguments. Individual command handlers open vaults independently; there is no common pre-operation startup-sync hook or foreground server command.
- The TUI has a background `SyncRuntime`, an S3 configuration overlay, optional “sync on unlock” and “sync on lock/quit” preferences, and manual sync. It directly owns the unlocked vault on its event-loop thread outside an in-flight sync.
- Flutter has S3 DTO/repository contracts, a S3 configuration page and pair-from-S3 bootstrap dialog, manual sync, and generated Rust bridge bindings. Android and iOS already have narrow native platform-channel dispatchers suitable for adding discovery/permission mechanisms without a Dart plugin dependency.
- The Makefile and CI contain SigV4, MinIO, real-S3, desktop bridge, iOS, and Android sync workflows. `make check` is the Rust gate, `make app-check` is the Flutter/bridge gate, and `make verify` is the repository-wide release gate.
- Current code caps an S3 response body at 256 MiB; this becomes the initial encrypted-vault transfer cap. Existing merge property, fault-injection, KDBX round-trip, and KeePassXC interoperability tests are retained rather than recreated.

## Proposed implementation

### Core layering

Keep `hidlins-sync` as the business/security owner and divide it into narrow modules for address policy, endpoint representation, discovery ports, framing, Noise sessions, identity sealing, trust state, pairing, application protocol, authoritative server coordination, LAN client transport, merge, backup, configuration, and orchestration. UI crates consume typed operations and events only.

The reusable remote contract remains a single encrypted object with opaque versioning and compare-and-swap. Rename S3/ETag concepts to `RemoteVersion`/`last_synced_remote_version` and make precondition failure a transport-neutral error. Use SHA-256 of the authoritative encrypted KDBX bytes as the fixed 32-byte server version. Preserve the existing four-state truth table and permit one CAS re-fetch/re-merge retry after the initial commit attempt.

### Protocol and cryptographic boundary

Use blocking `std::net::TcpStream`/`TcpListener` and bounded threads/channels; do not add an async runtime. Each connection begins with the fixed 12-byte preface `HIDLINS\0`, protocol major `1`, mode `PAIR` or `TRUSTED`, and zero reserved bytes. The exact preface is the Noise prologue, so modification causes handshake failure. No suite negotiation exists.

- Pairing: `Noise_XX_25519_ChaChaPoly_SHA256`.
- Trusted reconnect: `Noise_IK_25519_ChaChaPoly_SHA256` with both static public keys pinned.
- SAS: first 30 handshake-hash bits encoded as six Crockford Base32 characters and displayed as `XXX-XXX`; it is always derived locally and never accepted as peer payload.
- Noise dependency: exact `snow = 0.10.0`, default features disabled, with only `std`, `use-curve25519`, `use-chacha20poly1305`, `use-sha2`, and `use-getrandom` enabled.
- Snow patch: extend the repository's exact-version `tools/dev/vendor.py` patch mechanism to add zeroization to all secret-bearing Snow states and consumed transitions. An offline source guard verifies the exact patched inventory and fails on version/source drift.

After Noise transport mode, use a small in-repository binary codec rather than adding a general serialization dependency. Each encrypted Noise frame uses an unsigned 16-bit big-endian length prefix and is at most 65,535 bytes; application chunks are at most 60 KiB. Messages cover version/head, conditional fetch, upload begin/chunk/commit, cancellation, pairing commit/acknowledgement, and a small generic error set. The network layer yields an authenticated/authorized `SecureTransport`; sync logic cannot be called with an unauthenticated socket.

### Fixed resource bounds

- Encrypted vault transfer: 256 MiB maximum.
- Pairing window: 180 seconds; three failed attempts; one active SAS candidate.
- TCP connect timeout: 2 seconds per candidate; at most eight candidates/attempts and ten seconds total for a client operation.
- Noise handshake timeout: 5 seconds.
- Authenticated idle timeout: 15 seconds.
- Total normal sync session: 300 seconds.
- Concurrent accepted connections: 8 per vault server.
- Pending host vault operations: 16.
- Sync CAS retry: one retry after the initial failed conditional commit.
- Discovery result cache: 32 unique canonical scoped endpoints; stale/removed advertisements are evicted.

All limits are constants owned by the core and tested at below/equal/above boundaries. The startup attempt is asynchronous on TUI/Flutter so these budgets never block an interactive thread.

### Identity, trust, and configuration

Define a V1 local-sync registry schema only; do not deserialize or translate S3 configuration. It stores the role (`server` or `client`), a sealed per-vault/per-installation X25519 private identity, its public key, trusted peer records, the pinned authoritative server key for clients, transport-neutral divergence pointers, and an optional last-known/manual scoped endpoint as an untrusted routing hint. Public peer keys and display names are held only in the mode-0600 registry, never advertised.

Retain the existing Argon2id + ChaCha20-Poly1305 + random salt/nonce implementation technique as a generic versioned sealed-secret container with domain-separated associated data for sync identity. Remove AWS credential shapes. Master-password change decrypts and re-seals the same private key before registry save; lock/drop zeroizes plaintext key material.

Pairing uses an explicit three-minute server window. Both sides exchange Noise-authenticated commit records bound to protocol version, both public keys, vault role, and a random transaction identifier. Durable records remain non-authorizing provisional state until the commit/ack path completes; incomplete records expire and cannot read vault data. Fault tests interrupt every transition and prove that incomplete pairing never creates usable authorization. A completed client pins one server; re-designation deletes the prior client relationship only through explicit user action and requires fresh pairing.

### Discovery

Use `_hidlins-sync._tcp.local.` for trusted service discovery and `_hidlins-pair._tcp.local.` only while pairing is open. Service-instance labels are freshly randomized on each server start; TXT records are empty except for a fixed protocol-major marker if the selected API requires a record. Discovery metadata never carries vault names, peer keys, stable IDs, versions, or capabilities.

For CLI, TUI, and Flutter desktop, use exact-pinned `mdns-sd = 0.20.3` behind a `desktop-discovery` feature. It is a safe-Rust, no-async-runtime implementation; Task 005 must complete its exact license, source, feature, and transitive review before it is executed or retained. For iOS and Android, use the existing narrow platform-channel architecture over native Bonjour/Network Service Discovery APIs, avoiding a new Flutter plugin. Native code returns bounded candidate endpoints and permission state only; Rust canonicalizes and authorizes every candidate.

Every advertisement address, discovery result, manual literal, outbound socket peer, and inbound accepted peer is revalidated by one core allowlist implementation. IPv4-mapped IPv6 is normalized before classification. IPv6 link-local endpoints require a nonzero valid interface scope. Discovery does not confer trust; clients try allowed candidates against the pinned server key.

### Authoritative server and vault ownership

The server's network threads parse, authenticate, bound, and enqueue requests but never directly mutate an unlocked `Vault`. A bounded host-operation queue carries authorized head/fetch/conditional-commit requests to the process's existing single owner of the `Vault` and registry:

- CLI foreground serve owns and pumps the vault until `Ctrl+C`.
- TUI integrates server requests with its existing move-out background-operation state so the event loop remains responsive and only one worker owns the vault.
- `AppSession` uses its existing claim/run/commit pattern and lock-pending semantics for desktop server requests.

Head/fetch reads coherent encrypted bytes from the currently saved canonical file. Conditional commit validates the expected version, size, KDBX identity, password/keyfile, KDF compatibility, and current local state, then applies through backup and atomic-save primitives while updating the live in-memory database. Local edits and incoming commits are serialized by the same owner. Lock, application shutdown, server-off, or fatal error closes listeners, rejects/finishes in-flight operations safely, removes advertisements, and drops/zeroizes identity/session material.

### Client scheduling and surfaces

Implement a process-local startup tracker: the first successful unlock of each configured vault in an interactive process schedules one background sync; later unlocks and mutations do not. CLI vault-opening operations run the same configured sync before their requested operation. A startup sync failure is a nonfatal, secret-free warning and the local operation proceeds.

Replace the CLI sync surface with `sync now`, `sync serve`, `sync pair`, `sync import`, `sync status`, and `sync peers {list,rename,revoke}`. `sync serve` is foreground and handles `Ctrl+C` safely; an explicit pairing flag/action opens the bounded pairing window. Regenerate completions.

Replace TUI and Flutter configuration forms with discovery/pairing/import/status/peer-management flows. TUI and desktop have an in-memory server toggle defaulting off at each process start plus a separate “allow pairing for three minutes” action. The toggle may remain set across a lock within the same process, but the listener remains stopped until successful re-unlock. Mobile renders no server control and the Rust API refuses server start before binding on mobile targets.

iOS/Android explain and request local-network permission through native adapters, distinguish denied/restricted/not-found states, and offer retry/settings guidance. Mobile performs startup/manual foreground client sync only and registers no background service or listener.

### Verification and removal

Build security tests with each implementation slice. Add stable test identifiers and Makefile targets for fast security tests, discovery simulations, real-process integration, static S3 removal, fuzz corpus regression, and bounded CI fuzz runs. Use an isolated cargo-fuzz workspace with exact `cargo-fuzz 0.13.2`, a pinned nightly toolchain, and audited `libfuzzer-sys`; fuzz the codec/state machine, address canonicalization, and pairing/application message ordering. Normal stable tests replay committed minimized corpora.

Revision 2 permits NCSA-licensed code only inside this isolated development/test fuzz stack. The production workspace keeps NCSA outside its license allowlist. The fuzz workspace receives a package-scoped exception for the exact audited `libfuzzer-sys` release and no other NCSA package. A first-party `ncsa-boundary-check` enforces workspace isolation, manifest/lock/config placement, production dependency-closure absence, packaging-target gate wiring, and absence of libFuzzer/NCSA payloads or symbols from produced CLI, TUI, agent, Flutter desktop, iOS, and Android artifacts. Its self-tests plant representative forbidden production dependencies, misplaced license exceptions, missing packaging hooks, and contaminated artifact fixtures and must prove they fail. Fuzz executables are explicitly non-distributable development artifacts.

After all surfaces use local sync, delete S3 modules/APIs/tests/harnesses and remove `ureq`, rustls-platform-verifier promotion, AWS auth/SigV4/MinIO sources, and every now-unreachable transitive/vendor entry. Update `make verify` and CI to run local security/integration/fuzz-corpus gates. Keep historical workflow archives immutable.

Finally update active repository documentation and the external Hidlins PRD/index/verification workspace. Produce a focused threat/security review and dependency delta. Revisions 3 and 4 add copy-pasteable iOS Simulator and Android Emulator playbooks plus an automated end-to-end suite in which each real simulator application acts only as a client of a separate host CLI authority. The suite covers provisioning/startup, application build/install/launch, platform-specific host routing, rejected and accepted pairing, pair/import, bidirectional changes, manual-only scheduling, authority/address restart, client-process restart with pinned trust, revocation, evidence capture, and cleanup. It runs through a dedicated expensive Make target rather than the routine fast gate. Physical-device and human observations remain documented as optional non-blocking confidence work; their absence cannot prevent DONE.

## Architectural decisions

1. One authoritative server per vault; clients never elect or discover a replacement authority.
2. Discovery is mandatory but untrusted. Only pinned Noise identity establishes trust.
3. Fixed Noise suites: XX for first pairing and IK for reconnect; no negotiation or fallback.
4. Six-character, 30-bit Crockford Base32 SAS derived locally from the Noise handshake hash.
5. Exact-pinned, minimally featured Snow 0.10.0 with a maintained, reproducible Hidlins zeroization patch. Shipping unpatched is forbidden.
6. Blocking standard-library TCP plus bounded threads/channels; no Tokio or general async runtime.
7. Small hand-written bounded binary application codec; no general serialization/protobuf dependency.
8. `RemoteVersion` is a fixed SHA-256 digest of canonical encrypted KDBX bytes; it is not an ETag string.
9. Preserve the existing transport-neutral merge algorithm and KDBX history/backup semantics.
10. Network workers never share mutable vault access. Host-owned claim/run/commit adapters serialize incoming commits, local edits, and client sync.
11. Local-address policy is an exhaustive allowlist and is rechecked against actual socket endpoints. There is no override.
12. Desktop discovery uses optional `mdns-sd 0.20.3`; mobile discovery uses first-party native adapters and the existing platform-channel boundary.
13. Server enablement is process-local, explicit, and off by default. Mobile server/background mode is unavailable.
14. Startup sync is once per configured vault per interactive process after unlock; all later sync is manual. CLI performs a pre-operation attempt for every vault-opening command.
15. The local-sync schema is V1-only. S3 configuration is neither migrated nor parsed.
16. Every practical adversarial behavior is automated. Manual evidence cannot substitute for deterministic security assertions.
17. NCSA is a development/test-only exception limited to the exact audited `libfuzzer-sys` package in the isolated fuzz workspace. The production workspace still rejects NCSA, and code-enforced dependency, packaging, and artifact gates prevent NCSA-bearing code from entering any distributed Hidlins application.
18. Mobile scenario acceptance uses real iOS Simulator and Android Emulator application processes as clients of a separate host CLI authority. A mobile test must never enable or emulate a mobile server path.
19. The expensive cross-process mobile scenarios have one discoverable aggregate Make target, platform-specific implementation targets where CI separation requires them, bounded timeouts, deterministic cleanup, redacted logs, and machine-readable evidence. They are excluded from routine `make check`/`make verify` execution but are mandatory in the final workflow gate and dedicated CI jobs.
20. Manual accessibility, exact physical-device permission presentation, consumer-router behavior, and other irreducibly physical observations are non-blocking residual confidence checks. Automated substitutes must be maximized and clearly state their remaining fidelity limits.

## Work included

1. Characterization tests, protocol specification, threat model, resource constants, and dependency baseline.
2. Snow dependency integration, reproducible zeroization patch, secure-session wrapper, and patch drift guard.
3. Local-address policy, scoped endpoints, binary framing/application codec, protocol state validation, and property tests.
4. Identity sealing, V1 trust/config schema, XX/SAS pairing transaction, IK reconnect authorization, rewrap, and revocation.
5. Desktop mDNS/DNS-SD and mobile discovery-candidate/permission boundary, including DHCP and spoof simulations.
6. Bounded authoritative server runtime, host vault-operation queue, canonical versions, CAS, validation, locking, and atomic commits.
7. LAN client transport, generic orchestration, pair-and-import, startup tracker, and offline failure behavior.
8. Rust application API/session/FFI contracts, background lifecycle, event streams, and generated bridge changes.
9. Complete CLI command surface, foreground serving, secure prompting, startup warnings, JSON/human output, and completions.
10. Complete TUI discovery/pairing/import/status/peer/server UX with keyboard/accessibility contracts.
11. Flutter desktop/mobile UX plus iOS/Android native discovery/permission adapters and lifecycle tests.
12. Complete S3 deletion, dependency/vendor cleanup, static absence gate, and active harness/CI cleanup.
13. Automated adversarial, fuzz, real-process, isolated-network, cross-target, simulator/emulator, and CI suite completion, including executable enforcement that the NCSA fuzz exception cannot enter production graphs or artifacts.
14. Cross-process iOS Simulator and Android Emulator client scenarios against a separate host CLI authority, with an isolated expensive Make target, CI wiring, redacted evidence, and test-forward repair of the current invalid same-process mobile scenario.
15. Product/operator documentation, external PRD/index/scenarios, precise simulator procedures, non-blocking manual residuals, dependency dossier, NCSA exception review, and focused release security review.

Every code task includes the smallest meaningful red-before/green-after test record. Task 001 records characterization rather than a contrived red result; documentation-only portions record their static validation rationale.

## Task sequence

1. `tasks/001-characterize-and-freeze-design.md`
2. `tasks/002-snow-zeroization-and-secure-sessions.md`
3. `tasks/003-address-framing-and-protocol.md`
4. `tasks/004-identity-trust-and-pairing.md`
5. `tasks/005-secure-discovery.md`
6. `tasks/006-authoritative-server.md`
7. `tasks/007-client-sync-and-import.md`
8. `tasks/008-application-api-and-lifecycle.md`
9. `tasks/009-cli-surface.md`
10. `tasks/010-tui-surface.md`
11. `tasks/011-flutter-and-mobile-platforms.md`
12. `tasks/012-eradicate-s3.md`
13. `tasks/013-security-fuzz-and-platform-automation.md`
14. `tasks/014-automated-mobile-simulator-scenarios.md`
15. `tasks/015-documentation-and-release-review.md`

Tasks execute strictly in this order. All are `main-only` because each crosses a security, concurrency, data-integrity, lifecycle, supply-chain, or frozen-document boundary.

## Quality gate

The per-task standard gate is:

1. `make check` — repository Rust warning policy, format, Clippy, build, unit/integration tests, macOS cross-checks, and feature-gated compile checks.
2. `make app-check` — pinned Flutter/tooling checks, vendored pub integrity, analysis, formatting, branding, widget tests, real bridge smoke test, and generated binding drift.
3. `make ncsa-boundary-check` — prove the development-only NCSA exception remains isolated from every production dependency graph, packaging workflow, and produced application artifact.

The expensive mobile scenario suite is intentionally not part of this per-task
fast gate or `make verify`. It has its own bounded target and is mandatory for
Task 014 and the final package gate.

The final gate is:

1. `make verify` — the repository-wide release gate. Task 013 updates this existing target to include the new local-sync security, real-process integration, fuzz-corpus, removal, interop, supply-chain, NCSA-boundary, and acceptance-evidence targets while deleting S3/MinIO targets.
2. `make ncsa-boundary-check` — independently repeat the production graph/package/artifact exclusion policy at final acceptance.
3. `make check-ios` — Rust mobile target and native iOS security/storage checks.
4. `make app-test-ios-simulator` — native iOS permission/discovery/lifecycle suite.
5. `make app-build-ios` — simulator and no-codesign device artifacts.
6. `make android-emulator-provision` — reproducible host-native emulator matrix.
7. `make test-local-sync-mobile-scenarios HIDLINS_ANDROID_STRICT=1` — isolated expensive aggregate that preserves the existing iPhone/iPad and Android emulator matrices and adds separate-host-CLI, client-only mobile scenarios with redacted evidence.

Except for the policy-specific NCSA target and the new isolated mobile-scenario aggregate, these are existing repository command surfaces. The aggregate depends on or faithfully preserves the existing iOS integration and Android emulator coverage while adding the separate-process topology, so replacing their two direct final invocations does not weaken the gate. Dedicated platform CI jobs invoke the corresponding platform-specific scenario target through `make`; the aggregate is the single local/full-acceptance entry point. Physical permission wording, consumer-router behavior, human usability of SAS comparison, and assistive-technology output are recorded as non-blocking residuals because they cannot be faithfully established by automation. Their absence does not block completion.

## Risks

- **Snow secret erasure is incomplete upstream.** Mitigated by exact pinning, minimal feature selection, a reproducible exact-context vendor patch, source drift guard, wrapper zeroization, and focused review.
- **Pairing cannot make distributed disk writes literally atomic.** Mitigated by non-authorizing provisional records, authenticated transaction binding, expiry/recovery, and the invariant that incomplete state cannot fetch vault data. Fault tests interrupt every transition.
- **A long-held `Vault` lock conflicts with server concurrency.** Mitigated by a bounded host-operation queue and the existing single-owner move-out pattern; network workers never open a second write handle or mutate shared vault state.
- **mDNS is spoofable and can be flooded.** Mitigated by metadata minimization, strict endpoint filtering, candidate deduplication/caps, timeouts, rate limits, and pinned IK/XX+SAS authentication.
- **IPv4-mapped IPv6, scope IDs, interface churn, or socket TOCTOU could bypass local-only policy.** Mitigated by one canonical classifier, normalization, required link-local scope, and revalidation of actual connected/accepted peers.
- **Native discovery and permissions vary by mobile OS release.** Mitigated for release gating by thin native adapters, Rust-owned policy, API-level guards, simulated state coverage, and emulator/simulator matrices. Exact physical-device presentation remains an optional, non-blocking observation whose absence is reported rather than treated as automated proof.
- **A 256 MiB upload can exhaust memory or monopolize the server.** Mitigated by bounded chunks, streaming temporary storage, one active commit per vault, total/idle deadlines, queue/connection caps, and atomic promotion only after validation.
- **Surface migration can accidentally preserve hidden S3 paths or post-save sync.** Mitigated by a static absence gate and negative CLI/TUI/Flutter/API contract tests.
- **New dependency transitives can violate licensing or inflate the vendor tree.** Mitigated by exact staged audits, optional desktop feature gates, before/after graph evidence, `cargo deny`/`audit`, and rejection before dependency execution.
- **Fuzzing adds nightly/tooling complexity.** Mitigated by an isolated fuzz workspace, exact cargo-fuzz/nightly pins, committed corpora replayed on stable, and Makefile/CI parity.
- **The NCSA exception could accidentally expand into shipping code.** Mitigated by leaving NCSA forbidden in the production `deny.toml`, granting only an exact package-scoped exception in the isolated fuzz policy, rejecting any other NCSA occurrence, requiring all packaging targets to run the boundary guard, scanning produced artifacts, and exercising negative controls in CI.
- **Simulator networking differs from a physical LAN.** Mitigated by using the platform-specific host routes (`127.0.0.1` or an allowed host-private address for iOS Simulator and Android Emulator's `10.0.2.2` host alias), using the restricted manual endpoint path when multicast does not cross virtualization, retaining deterministic discovery/DHCP policy tests, and limiting claims about real-router multicast.
- **The existing shared mobile bridge scenario uses the wrong topology.** It starts both server and client sessions inside one mobile test process even though production mobile builds disable server capability; it also lacks bidirectional mutations and process-restart persistence. Task 014 replaces this with a separate host CLI authority and real client-only simulator process, first preserving the failure as a regression test.
- **Expensive simulator suites can be flaky or leave processes behind.** Mitigated by deterministic device selection, unique state directories and ports, repository-owned deadlines, process-group cleanup, explicit application uninstall/reset, secret-safe logs, artifact hashes, and machine-readable PASS/FAIL evidence.
- **Physical/manual fidelity is incomplete.** Accessibility output, exact physical permission presentation, and consumer-router behavior remain documented as non-blocking residual confidence checks. This limitation is reported honestly but cannot block workflow completion under Revision 4.

## Out of scope

- Public-Internet synchronization, relays, cloud sync, public IP ranges, VPN exceptions, or any network-policy override.
- Peer mesh, automatic leader election, or more than one authoritative server per vault.
- Persistent/background mobile sync or mobile listening/server behavior.
- A system daemon, service manager integration, or implementation of `hidlins-agent`.
- Account systems, cloud directories, certificate authorities, DNS hostnames, cloud-mediated pairing, or telemetry.
- A proprietary vault representation, plaintext vault transport/storage, or changes to the KDBX data model.
- S3 migration, import, translation, legacy parsing, deprecation aliases, or compatibility shims.
- Changes to immutable `.ai/workflow-archive/` history.

## Final acceptance criteria

1. No active production, test, build, CI, configuration, bridge, UI, dependency, or user-documentation path implements or offers S3/AWS/SigV4/MinIO sync; no migration or compatibility code exists.
2. A server-configured vault supports multiple named/revocable clients; a client pins exactly one authoritative server and cannot be silently redirected by discovery.
3. Automatic discovery reconnects after a simulated DHCP address change, while forged/stale/disallowed advertisements cannot establish trust or obtain vault information.
4. All sync endpoints are rejected unless they fall in the exact DRD allowlist, including actual socket endpoint rechecks, mapped-address normalization, and link-local IPv6 scope handling; no override exists.
5. XX pairing uses the fixed suite, three-minute/three-failure policy, local 30-bit SAS, bilateral confirmation, and non-authorizing provisional commit state. Every interrupted or rejected path is proven unable to access vault data.
6. Reconnect uses only fixed-suite IK with pinned mutual identities; key mismatch, revocation, replay, downgrade, and silent repair fail closed before vault disclosure.
7. Per-vault/per-installation private identities are sealed at rest, zeroized in plaintext/session state, preserved across password rewrap, and covered by the reviewed Snow patch/source guard.
8. The bounded protocol enforces every frame/message/chunk/transfer/connection/queue/time/retry limit and preserves the old canonical vault under malformed, duplicated, reordered, truncated, exhausted, disconnected, or crashed operations.
9. Existing already-current/push/fast-replace/merge behavior, one CAS retry, loser history, `.kdbx.bak`, KDF/identity validation, advisory locking, and atomic writes pass unit, property, fault, real-process, and KeePassXC interoperability tests.
10. CLI, TUI, Flutter desktop, iOS, and Android attempt sync once after first configured unlock and thereafter only manually; all automatic post-save/lock/quit sync paths are absent and sync failure never prevents offline local use.
11. CLI foreground serving exits safely on `Ctrl+C`; TUI/desktop serving is explicit, process-local, and stopped while locked; mobile has no usable listener/background-server path.
12. Pair/import, discovery, manual fallback, status, errors, and relevant peer/server controls work on every required surface with automated keyboard/accessibility-semantics and secret-free output coverage; human assistive-technology observation is optional and non-blocking.
13. iOS and Android distinguish permission denial/restriction from not-found and remain usable offline. The isolated mobile-scenario target passes with actual iOS Simulator and Android Emulator application processes acting only as clients of a separate host CLI authority, covering rejected/accepted pair-and-import, authenticated bidirectional changes, manual-only behavior, route/server restart, client-process restart with pinned trust, revocation, and mobile-server rejection.
14. Exact dependency/license/provenance/transitive/vendor evidence exists for Snow, mdns-sd, fuzz tooling, the Snow patch, removals, and every changed platform dependency; all license/advisory gates pass. NCSA is accepted only for the exact audited development/test `libfuzzer-sys` package and is rejected from every production workspace or application graph.
15. Fast security, discovery, real-process, isolated-network, fuzz-corpus, bounded fuzz, S3-absence, NCSA-boundary, bridge, UI, platform, KDBX, merge, and fault suites are exposed through Makefile targets and run in CI. Production packaging targets and artifact checks prove no CLI, TUI, agent, Flutter desktop, iOS, or Android deliverable contains NCSA-licensed code.
16. Every implementation task contains test-forward evidence; every automatable DRD behavior maps to a stable automated test ID; simulator/emulator scenarios are reproducible from a clean documented setup and emit redacted machine-readable evidence. Remaining genuinely human/physical observations are explicitly non-blocking and do not prevent DONE.
17. The focused final security review has no unresolved high-confidence high/critical finding, and active PRD/index/verification/operator documentation consistently describes the shipped local-network model.
18. Every command in `gate.json`, including the isolated expensive mobile-scenario target, passes in order; every task has evidence; and the cumulative diff contains no unrelated scope expansion.

## Acceptance follow-up — Round 2

### Observed acceptance failures and root causes

The post-completion composite review found eleven high-confidence defects that contradict existing Round 1 acceptance criteria. The defects cluster into four implementation gaps:

- Server authorization and pairing policy were modeled correctly in isolated types but not carried into live connection ownership. Trust replacement does not identify or close authenticated sockets, pairing admission does not use `PairingWindow`, and the rejected-peer counter path decrements without reserving a slot.
- Constants describe strict time and transfer limits, but blocking I/O and response construction do not enforce them. Socket timeouts are broader than Noise/connection budgets, session deadlines are not propagated through writes, and fetch builds a second full in-memory representation of the encrypted vault.
- Discovery routing is inconsistent across platforms and static at the authority. Native service names differ from Rust, the client treats the first wrong-key candidate as terminal, and the listener/advertiser never refreshes after an interface address changes.
- Application acceptance harnesses cover injected routing rather than the default startup path. Flutter startup invokes sync before native/desktop discovery, while the TUI asks endpoint enumeration to construct an invalid port-zero endpoint. The S3 absence checker also has a case-sensitive gap.

### Additive fix strategy and architecture

1. Make authenticated peer identity part of live server connection and host-operation ownership. Trust replacement closes removed peers and prevents their queued or in-progress pre-commit work from crossing an authorization generation boundary. Pairing admission uses one shared `PairingWindow`, a sole-candidate guard, exact failure accounting, and cancellation on close/expiry. Connection accounting becomes guard-owned so it cannot underflow.
2. Introduce deadline-aware framing helpers driven by absolute monotonic deadlines. Each blocking read/write uses the smallest remaining handshake, total-connect, idle, pairing-window, or total-session budget. Fetch responses become file-backed/streamed and emit one bounded frame at a time without constructing a complete vector of copied chunks.
3. Establish one canonical DNS-SD contract across Rust, Swift, Android, and platform manifests/tests. Candidate selection continues past untrusted wrong-key routes within the fixed total budget. Desktop authority ownership monitors the chosen allowed interface and performs fail-closed listener/advertisement replacement when its address changes; no wildcard/public bind or policy override is introduced.
4. Move Flutter startup sync into an explicit discover-then-sync orchestration that works across the desktop and native-mobile discovery ports while preserving the once-per-process tracker and nonfatal offline behavior. Give server binding a checked bind-address representation that can request an ephemeral port without weakening `LocalEndpoint`, and exercise the actual TUI runtime.
5. Make retired-transport matching case-insensitive, delete the stale reference, and add negative controls. Finish with focused cross-surface regression coverage and the unchanged full quality gate.

No new dependency is required. The existing `std::net`, bounded channel, `tempfile`, `if-addrs`, `mdns-sd`, native Bonjour/NSD, and application bridge boundaries are sufficient.

### Round 2 task sequence

16. `tasks/016-live-authorization-and-pairing-repair.md`
17. `tasks/017-deadline-and-streaming-repair.md`
18. `tasks/018-discovery-resilience-and-contract-repair.md`
19. `tasks/019-application-startup-and-tui-repair.md`
20. `tasks/020-removal-gate-and-acceptance-regression.md`

Tasks execute strictly in numeric order. All are `main-only`: Tasks 016–019 change security, concurrency, network, resource, or cross-platform lifecycle boundaries; Task 020 owns final acceptance repair evidence and enforcement consistency.

### Tests and validation

Every defect receives a smallest meaningful fail-before test before implementation. Runtime tests must exercise real sockets or real surface orchestration whenever the defect arises outside an isolated state type. New coverage includes live revocation during requests/commit, concurrent and failed pairing admission, slow handshake/read/write peers, maximum-size streaming ownership, rejected-peer counter recovery, wrong-key-first candidate fallback, authority interface refresh, canonical native service types, restart startup discovery without injection, actual TUI listener startup, and uppercase retired-transport negative controls.

`gate.json` remains unchanged: its standard commands already cover Rust, Flutter, and the NCSA boundary after each task, while its final commands include the complete release, mobile-native, artifact, emulator, and expensive cross-process scenario gates. Task-specific commands narrow feedback without weakening the repeated standard and final gates.

### Round-specific risks

- Closing revoked connections is insufficient if a dequeued commit can outlive the authorization decision. Authorization generation/cancellation must be checked at the host commit boundary as well as on the socket loop.
- Deadline-aware I/O must preserve platform error mapping and never turn timeout differences into a pre-authentication oracle.
- File-backed fetch streaming must retain a coherent encrypted snapshot across atomic vault replacement and must not hold the application event loop or unlocked-vault owner during network writes.
- Interface replacement must publish only an address that passed the existing exact allowlist and actual socket checks. Failure to acquire a replacement keeps the old listener only while its address remains valid and otherwise stops serving with an actionable local error.
- Mobile startup discovery cannot be initiated from Rust directly. The bridge/controller handshake must preserve unlock responsiveness, once-per-process scheduling, cancellation on lifecycle changes, and offline continuation.

### Round-specific out of scope

- New network transports, Internet/VPN exceptions, DNS hostnames, relay infrastructure, or a generalized service daemon.
- New dependencies or replacement of Noise, DNS-SD, native discovery, KDBX, or atomic-save primitives.
- Changes to the established sync truth table, merge policy, vault format, or mobile client-only posture.
- Modifying completed Round 1 task documents/evidence or immutable workflow archives.

### Round 2 acceptance criteria

1. Revocation closes and cancels every live/queued operation for the removed peer, and no request or commit can succeed after the revocation boundary.
2. The live pairing server enforces one active SAS candidate, at most three failed attempts, the exact remaining three-minute window, and immediate cancellation/advertisement withdrawal on close or exhaustion.
3. Every blocking handshake, candidate, authenticated I/O, host wait, and response write is bounded by the relevant absolute deadline; resource accounting recovers after rejected peers and slow/disconnected clients.
4. Fetching a maximum-size encrypted vault does not construct a second vault-sized collection of chunk messages, streams coherent bytes in bounded frames, and respects cancellation/session limits.
5. Rust, iOS, Android, and manifests use the same V1 trusted/pairing DNS-SD service types; wrong-key/stale candidates cannot prevent a later pinned candidate from succeeding.
6. A running desktop authority refreshes its listener and advertisement after an allowed-interface DHCP/address change without exposing public/wildcard routes or changing pinned identity.
7. Flutter desktop/mobile startup performs discovery before its once-per-process sync without pre-injected candidates, remains nonfatal offline, and mobile remains foreground-client-only.
8. TUI explicit server mode starts a real listener on an allowed interface using a safely allocated nonzero port and shuts it down through existing lock/toggle/process lifecycle rules.
9. The retired-transport gate rejects all case variants including uppercase `MINIO`, active stale references are removed, and negative controls prove the gate fails closed.
10. All eleven regression tests demonstrate fail-before/pass-after evidence, all Round 2 task validations pass, and every unchanged standard/final command in `gate.json` passes in order.

## Approved-plan revision — Revision 6: genuine Android LAN discovery

### Observed failure and investigation findings

The failed API 29 evidence is a real acceptance failure, not an emulator
timing flake. `tools/local-sync-tests/mobile_scenario.py` currently binds the
CLI authority to `127.0.0.1`, maps Android to `10.0.2.2`, and passes both the
sync host and sync port into the Flutter process as Dart defines.
`mobile_local_sync_scenario_test.dart` then constructs that endpoint and passes
it directly to both pairing attempts. Only the later startup-sync step uses the
native adapter, where it fails with `syncOffline` because Android NSD has no
usable route to the loopback-only advertisement. The old harness therefore
proves TCP reachability through an emulator alias but does not prove Android
automatic discovery.

This revision supersedes the earlier simulator-risk mitigation that permitted
`10.0.2.2` or a restricted manual endpoint as Android acceptance evidence.
Those mechanisms remain valid product diagnostics, but they cannot satisfy the
automated discovery scenario or support a claim that default discovery works.

The installed Android Emulator is 37.1.11. Official Android documentation says
36.5 and later place multiple emulators on a shared virtual Wi-Fi network where
they can discover one another using NSD. The normal single-emulator address
space still treats `10.0.2.2` as a special host-loopback alias, not as a
real-device LAN address. A diagnostic launch also confirmed that the emulator's
macOS `vmnet-bridged` backend requires elevated host networking privileges, so
making that privileged, host-specific path the mandatory test would be neither
portable nor reliably autonomous.

A read-only cross-target check found one expected portability gap before an
Android CLI authority can run: the CLI's unconditional clipboard dependency
pulls `arboard`, whose platform backend does not support Android. The sync core,
CLI parsing, and Android target otherwise progressed through compilation. This
is addressed with a narrow test-only CLI feature, not a new dependency and not
a mobile-server product capability.

### Additive repair strategy

1. Replace the Android scenario topology with two independent emulator
   processes on Emulator 36.5-or-newer's shared virtual Wi-Fi LAN. One runs the
   shipping Flutter Android application as a foreground-only client. The other
   runs the actual `hidlins` CLI binary and its normal `sync serve` command as
   the authority. Both receive distinct DHCP addresses on the same private
   virtual subnet and exchange actual DNS-SD/mDNS and TCP traffic.
2. Add an explicitly development/test-only Android scenario-authority feature
   for the CLI and sync crate. It enables the existing reviewed `mdns-sd` and
   `if-addrs` dependencies on this target and excludes unsupported clipboard
   operations. It adds no crate, changes no shipping CLI behavior, does not add
   server capability to the Flutter Android application, and is built only by
   the isolated scenario target. Static graph/packaging assertions must prove
   that no Android application or normal release target enables or packages
   this feature or binary.
3. Make the harness fail closed unless it observes two distinct emulator
   identities, two allowed non-loopback `wlan0` addresses on the same virtual
   LAN, Emulator >=36.5, and a real pairing-service NSD result whose resolved
   endpoint equals the authority emulator's current interface address and
   listener port. Run this discovery-only proof before allowing any sync vault
   bytes to cross the test network.
4. Remove the Android sync-host/sync-port Dart defines and direct
   `LocalEndpointDto` construction. Rejected and accepted pair-and-import use
   the shipping `MethodChannelLocalDiscovery` adapter, Flutter repository
   orchestration, Rust candidate cache/policy, and Noise authentication. Startup
   and manual sync reuse newly discovered routes. `10.0.2.2`, ADB forwarding,
   emulator console redirection, DNS hostnames, and manual candidate injection
   are forbidden for the sync data plane.
5. Keep test orchestration separate from routing. Host-side ADB may start/stop
   processes, provide secure stdin, and inspect/pull test state; an authenticated
   control channel may coordinate mid-scenario assertions only if it never
   supplies, rewrites, proxies, or forwards a sync endpoint. Evidence records
   control-plane use separately from the discovered data-plane route.
6. Exercise address churn by retaining the encrypted authority state and Noise
   identity while restarting it on a replacement authority emulator with a
   different DHCP address. The client must rediscover that new address and
   authenticate the same pinned identity without a route hint. If the virtual
   Wi-Fi implementation cannot provide distinct discoverable peers or real
   multicast on a supported host, the harness fails; it must never fall back to
   endpoint injection.

### Revision 6 task sequence

21. `tasks/021-genuine-android-emulator-lan.md`
22. `tasks/022-discovery-only-mobile-acceptance.md`

Tasks execute in numeric order and are `main-only` because they change
cross-platform build boundaries, native discovery, test networking, security
evidence, and final acceptance infrastructure. Tasks 001–020 and their evidence
remain immutable and complete.

### Test and quality-gate changes

Task 021 first adds negative harness tests that reject the current
loopback/`10.0.2.2`/Dart-define/direct-candidate topology, reject one-emulator
and off-subnet arrangements, reject emulator versions below 36.5, and reject a
discovery result that does not match the authority's observed interface and
listener. It then adds a focused isolated Make target for the two-device
discovery proof and records the current failure before implementation.

Task 022 converts the full Android scenario and preserves the existing API 29
phone and API 36 tablet client matrix. Evidence must contain the authority and
client device identities, observed DHCP routes, native NSD service-resolution
event, Rust-accepted candidate, Noise-authenticated outcome, address-change
rediscovery, redaction result, artifact hashes, and all existing scenario
steps. Evidence must also state that no manual/injected/proxied sync route was
used. Harness unit tests statically inspect the Flutter invocation and scenario
source so reintroducing a sync endpoint define or direct candidate causes a
failure.

`gate.json` remains unchanged. Its final Android scenario command is the
existing isolated expensive target and will execute the repaired topology. The
standard commands already cover the repository, Flutter application, and
shipping-boundary checks. Task-specific targets provide faster feedback without
weakening the final gate.

### Revision 6 risks and stop conditions

- Android's shared virtual Wi-Fi/NSD capability is an emulator-version feature,
  not available in older installations. Provisioning must pin and verify a
  version >=36.5 on macOS arm64 and Linux x86_64 CI instead of silently using an
  older binary.
- The test-only CLI target must not become a supported Android product surface.
  The Flutter application remains client-only and packaging must not contain
  the CLI executable, `mdns-sd`, or server-mode feature.
- Android shell multicast behavior may differ from an APK UID. The discovery
  smoke phase must demonstrate the actual CLI advertisement and Android
  `NsdManager` resolution before the expensive scenario proceeds. If that
  cannot work on the pinned emulator, implementation stops at
  `PLAN_CHANGE_REQUIRED`; route injection, an NSD-mocking companion app, a TCP
  proxy, or manual address input is not an acceptable fallback.
- Two-emulator tests consume materially more CPU, memory, and time. They remain
  isolated from routine `make check`/`make verify`, use bounded boot/test
  deadlines, and own deterministic process/device cleanup.

### Revision 6 out of scope

- Adding Android/mobile server functionality to a packaged Flutter application.
- Treating the Android test-only CLI build as a supported or distributable
  product artifact.
- Privileged host bridges, public/physical LAN traffic, ADB port forwarding,
  emulator-console redirection, discovery mocks, or a Hidlins-aware proxy.
- New networking, discovery, serialization, async-runtime, or test-framework
  dependencies.
- Relaxing the local-address allowlist, pinned Noise identity, foreground-only
  mobile lifecycle, or the existing manual diagnostic feature in the product.

### Revision 6 acceptance criteria

1. Android API 29 and API 36 scenarios use two distinct emulator processes on
   one private virtual Wi-Fi LAN and fail closed unless their observed DHCP
   addresses and emulator version prove that topology.
2. The shipping Android client receives pairing and trusted-service routes only
   through `NsdManager`, the Flutter platform channel, and Rust validation; no
   sync host/port define, direct candidate, `10.0.2.2` sync route, ADB forward,
   proxy, DNS hostname, or manual endpoint participates.
3. The authority is the actual Hidlins CLI `sync serve` process using the normal
   production server/protocol/Noise code and existing DNS-SD implementation.
   Its Android scenario build is test-only, adds no dependency, is absent from
   all application packages and normal release graphs, and exposes no packaged
   mobile server capability.
4. A discovery-only preflight proves native resolution to the authority's
   observed private interface/listener before vault transfer. Pair/import,
   startup sync, manual bidirectional sync, restart, revocation, and mobile
   server rejection then pass without route injection.
5. Replacing the authority emulator changes the advertised DHCP route while
   preserving encrypted vault state and Noise identity; the client rediscovers
   the new route and reconnects only after pinned-identity authentication.
6. The dedicated two-device discovery target, repaired Android scenario target,
   unchanged aggregate target, standard gates, and every final command pass;
   evidence is redacted, machine-readable, and sufficient to distinguish the
   control plane from the automatically discovered sync data plane.

## Approved-plan revision — Revision 7: platform-registered emulator discovery

### Correction to Revision 6

The Revision 6 two-emulator investigation established the intended shared
private Wi-Fi topology but falsified one test-environment assumption. Emulator
37.1.11 assigned the API 36 guests distinct `wlan0` leases (`10.0.2.16` and
`10.0.2.17`) in one `/24`, the cross-built real CLI bound the authority address
and reported DNS-SD registration, and the shipping Android client browsed with
`NsdManager` and granted permissions. Nevertheless, it received no service
record. Running the CLI under both the shell UID and the Hidlins application UID
did not change the outcome.

This revision supersedes only Revision 6's requirement that the raw Rust
`mdns-sd` advertisement from inside the Android guest itself must be the
emulator discovery advertisement. Android's shared-emulator documentation
promises NSD between Android applications; it does not guarantee that arbitrary
native multicast sockets inside a guest are reflected into the platform NSD
stack. The shipping discovery architecture remains unchanged.

### Revised repair architecture

1. Continue running the actual cross-built `hidlins sync serve` CLI on the
   authority emulator. It owns the vault, Noise XX/IK handshakes, authorization,
   protocol state, commits, and every byte of synchronization traffic. The
   Flutter Android application does not gain server behavior.
2. Add a registrar solely to the Android instrumentation source set and test
   APK. The registrar invokes `NsdManager.registerService()` for the CLI's
   locally bound port and the exact V1 pairing or trusted service type. It does
   not set or communicate an IP address: Android selects the authority
   emulator's active network/address. The registrar remains alive only for the
   bounded test phase and unregisters on completion or cancellation.
3. The shipping client continues to use `LocalDiscoveryController`, the normal
   Flutter method channel/repository/controller path, and Rust candidate
   validation. It must resolve the authority emulator's actual private
   `wlan0` address and CLI listener port from native NSD before it may pair or
   synchronize. Noise pinned identity remains the authorization boundary;
   discovery metadata remains untrusted routing data.
4. The host harness may tell the authority-side registrar only the local CLI
   listener port and service kind. This is equivalent to a production server
   registering its own listener and is not a client route hint. The client must
   not receive an endpoint, address, port, candidate, proxy, or discovery result
   through instrumentation, Dart defines, ADB, the control plane, or fixtures.
5. Keep all Revision 6 anti-shortcut rules: no `10.0.2.2` sync data route, ADB
   forward/reverse, emulator redirection, proxy, synthetic discovery result,
   DNS hostname, manual endpoint, or direct candidate construction. Host-side
   ADB remains process orchestration only.
6. Prove the boundary structurally and from built artifacts. Registrar source,
   components, and test-only declarations may exist only under `androidTest`;
   production source sets and release/debug application APKs must exclude the
   registrar. Normal CLI/TUI/Flutter dependency graphs and artifacts must
   exclude the Android scenario-authority feature/binary. The instrumentation
   APK is a development/test artifact and is never packaged or distributed as
   Hidlins.
7. Preserve the API-level permission correction: `NEARBY_WIFI_DEVICES` applies
   where required, while `ACCESS_LOCAL_NETWORK` is requested only where the
   platform defines and enforces it beginning with Android 17/API 37. API 36
   must not be blocked waiting for a nonexistent runtime grant.

### Layered verification and coverage claims

- The isolated two-emulator test proves Android platform NSD advertisement and
  shipping-client discovery on a shared private virtual Wi-Fi LAN, Rust
  address-policy acceptance, and direct connection to the real CLI's complete
  Noise and synchronization data plane.
- Existing/focused desktop discovery tests exercise the real Rust `mdns-sd`
  publisher and browser. They remain automated and are run explicitly as part
  of the revised Task 021 validation.
- A desktop Rust publisher to Android `NsdManager` client over a representative
  physical LAN remains an optional, non-gating observation. Simulator-only
  evidence must not claim to prove multicast interoperability across physical
  access points or router firmware. This honest limitation does not block
  completion under the approved automation policy.

No custom UDP discovery protocol, stored-address replacement for automatic
discovery, new dependency, privileged bridge, or mobile product server is
introduced.

### Revision 7 task sequence

21. `tasks/021-genuine-android-emulator-lan.md` — revised to add the
    instrumentation-only platform NSD registrar and its packaging boundary.
22. `tasks/022-discovery-only-mobile-acceptance.md` — revised to run the full
    Android acceptance journey through that discovered real-CLI route.

Tasks remain `main-only` because they cross security, native-platform,
packaging, process-orchestration, and acceptance-evidence boundaries. Tasks
001–020 and their evidence remain immutable and complete. The interrupted-plan
helper reset Tasks 021–022 to `PENDING` with fresh per-task retry budgets.

### Test and quality-gate changes

Task 021 adds fail-before/pass-after coverage for registrar lifecycle,
service-kind/port validation, authority-side-only configuration, discovery
route equality, test-source-set isolation, built-APK exclusion, and every
existing anti-injection rule. Its live target must first confirm successful
platform registration, then resolve that record through the shipping client,
then confirm Rust accepts the same private route and that the listener is the
real CLI process. `make test-local-sync-discovery` separately exercises the
actual Rust desktop publisher/browser implementation.

Task 022 retains the complete API 29 phone and API 36 tablet scenario matrix,
including pair/import, startup fetch, manual-only bidirectional sync, process
restart, authority address churn with preserved encrypted state/identity,
revocation, and mobile-server rejection. Each authority start registers its
real CLI port through the test-only platform registrar. The client discovers
the current authority address without a hint, and all sync traffic connects
directly to the CLI.

`gate.json` remains unchanged and is not weakened. The focused Task 021
validation gains the desktop discovery target; the existing isolated Android
and aggregate scenario targets remain the expensive final acceptance commands.

### Revision 7 risks and boundaries

- The registrar validates standards-based Android NSD and the real data plane,
  but it does not prove that Rust raw multicast advertisements traverse a
  particular physical router to Android. Documentation and evidence must state
  this boundary precisely.
- Because instrumentation targets the Hidlins package, source-set and APK
  negative controls are mandatory. A registrar class or component in a normal
  application artifact is a hard failure.
- Registration success alone is insufficient. The resolved client route must
  equal the authority emulator's independently observed allowed `wlan0` address
  and the actual CLI-bound port, and the subsequent authenticated socket must be
  owned by that CLI.
- The registrar may know only its service kind and local server port. It may not
  select an address for the client, proxy traffic, expose vault/peer/key/SAS
  metadata, or participate in Noise or application-protocol state.
- Physical-device/router and assistive-technology observations remain optional
  and non-gating; they must not be reported as executed when they were not.

### Revision 7 acceptance criteria

1. The real `hidlins sync serve` CLI runs on the authority emulator and handles
   every Noise pairing, trusted session, synchronization request, and commit;
   no mobile application server or proxy handles sync traffic.
2. An `androidTest`-only `NsdManager` registrar advertises only the CLI's local
   port and exact service kind. Android chooses the network/address, the
   shipping client resolves it through its normal native adapter, and Rust
   accepts the same private endpoint before connection.
3. No endpoint/address/port is supplied to the client; no direct candidate,
   `10.0.2.2` sync route, ADB forward/reverse, emulator redirection, proxy,
   synthetic result, DNS hostname, or manual endpoint participates.
4. Source-set, dependency-graph, and built-artifact checks prove that registrar
   code/components and the scenario CLI authority are absent from every
   distributable CLI, TUI, and Flutter application. Mobile remains client-only,
   and no external dependency is added.
5. The API 29 and API 36 two-emulator journeys pass all existing scenario
   assertions. Authority replacement produces a different private DHCP route,
   which the client discovers without a hint and accepts only with the preserved
   pinned Noise identity.
6. Automated desktop tests pass for the real Rust `mdns-sd`
   publisher/browser. Documentation distinguishes that coverage and the
   emulator platform-NSD coverage from the optional, non-gating physical
   desktop-to-Android multicast observation.
7. Revised task validations, every unchanged standard command, and every final
   command pass with redacted machine-readable evidence and explicit
   `route_injection=false`, `sync_proxy=false`, and test-registrar boundary
   fields.
