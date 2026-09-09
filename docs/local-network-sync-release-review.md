# Local-network sync release review

Review date: 2026-09-08. Overall disposition: **READY.** Automated iPhone, iPad,
Android phone, and Android tablet scenarios passed against a separate release
CLI authority. The focused code/security review has no unresolved
high-confidence high or critical finding. Human/physical observations in
[`local-network-sync-manual-verification.md`](local-network-sync-manual-verification.md)
are optional and non-blocking.

## Focused security review

| Boundary | Review result and evidence |
| --- | --- |
| Protocol and cryptographic integration | Frozen version/preface/state transitions, Noise XX pairing, IK pinned reconnect, downgrade/key-substitution/replay tests, and bounded fuzz targets pass. Hidlins does not negotiate algorithms or implement primitives. See `protocol-v1.md`, `threat-model.md`, and `security-coverage.md`. |
| Zeroization | Hidlins secret owners use zeroizing wrappers/drop behavior; exact Snow secret owners and consumed transitions are covered by the maintained digest-guarded patch. Vendor-patch negative controls fail closed. See `snow-dependency.md`. |
| Discovery and address boundary | Discovery crosses only address/port/scope candidate data and grants no authority. `LocalEndpoint` is required before connect, after `peer_addr`, before bind, and after accept. IPv4 `10/8`, `172.16/12`, `192.168/16`, `169.254/16`, `127/8` and IPv6 `fc00::/7`, scoped `fe80::/10`, `::1` are the entire allowlist; mapped, public, special, DNS, and missing/extra-scope forms fail without override. |
| Pre-auth privacy and authorization | Normal advertisements contain only routing data; bounded pairing advertisements are explicit and temporary. Unknown/revoked keys receive no vault metadata or differentiated authorization oracle. Live trust replacement closes removed peers, and revocable host permits prevent already-dequeued commits from crossing the revocation boundary. SAS confirmation is bilateral and transaction-bound before activation; the live listener enforces one candidate, three failures, expiry, and explicit close. |
| Resource limits and availability | Frame, message, chunk, vault, candidate, connection, queue, retry, and absolute handshake/session deadlines are enforced before unbounded work. File-backed fetch snapshots remain coherent across atomic replacement and emit one bounded frame at a time. Guard-owned connection slots recover after rejected/disconnected peers, and completed connection workers are reaped during service. |
| Concurrency, ownership, and CAS | One owner moves an unlocked vault into a host operation; UI/API lock and shutdown cancel or discard returned state safely. Server commits use expected versions; simultaneous-client tests prove one winner and a valid recoverable loser/retry path. Trust preparation/activation/revocation writes are transactional. |
| Atomic storage and no data loss | Uploads stage to sibling files, validate length/digest/order, then commit through locked atomic persistence. Pre-merge `.kdbx.bak`, loser-as-history, fault injection, properties, and KeePassXC round trips pass. Pair/import crash boundaries leave no active partial trust or partial vault. |
| Logs, errors, and process output | Errors, status DTOs, CLI JSON, Flutter events, discovery payloads, and process tests exclude keys, transcripts, SAS internals, vault contents, and master material. Telemetry/crash/update endpoints remain absent. |
| FFI and UI policy | Rust owns protocol/trust/address/storage. Generated FFI exposes bounded opaque DTOs and rejects extra discovery metadata. CLI/TUI/desktop provide explicit serving; mobile APIs reject server start before bind and expose no server control. Flutter explicitly discovers before consuming its once-per-process startup attempt; later work is manual-only. The TUI uses a bind-only ephemeral endpoint and exposes only the kernel-assigned nonzero route. |
| Mobile permissions and lifecycle | iOS Bonjour/local-network and Android NSD adapters are routing-only. Manifests have no background sync service/mode; every non-resumed transition stops native discovery and cancels Rust sync/pairing while preserving the vault-lock grace. Simulator/emulator/native tests cover the programmatic contract. Physical OS wording and consumer-router multicast remain optional confidence checks. |
| End-to-end application scenarios | `make test-local-sync-mobile-scenarios HIDLINS_ANDROID_STRICT=1` passed on iPhone 17 Pro Max, iPad mini (A17 Pro), API 29 phone, and API 36 tablet. Android ran the real release CLI inside API/ABI-matched authority emulators, advertised its port with an instrumentation-only `NsdManager` registrar, resolved it through the shipping client, and carried Noise/sync bytes directly over private emulator Wi-Fi. API 29 used its required explicit Wi-Fi peer link with synthetic cellular data disabled to avoid the emulator's duplicate same-subnet interfaces; Android still selected the NSD address and no route was injected. Each device emitted eleven passing steps, clean secret scans, release CLI/application hashes, encrypted restart-state hashes, and redacted logs under `build/verification/mobile-local-sync/`. |
| Discovery-layer separation | `make test-local-sync-discovery` performs a real Rust `mdns-sd` desktop publisher/browser exchange. Android emulator acceptance separately proves platform NSD plus the real CLI data plane. A physical desktop-Rust-to-Android run is optional and is the only check that can characterize a particular access point/router multicast boundary. |

## Dependency and licensing disposition

The final counts, provenance, exact licenses, features, maintenance signals,
alternatives, transitives, vendor delta, and patch obligations are in
[`../crates/hidlins-sync/docs/dependency-review.md`](../crates/hidlins-sync/docs/dependency-review.md).
`make deny`, `make audit`, and `make vendor-patches` are clean in the automated
precursor record.

NCSA is permitted only for exact `libfuzzer-sys 0.4.13` in the isolated
development/test fuzz workspace. It compiles bundled LLVM libFuzzer into fuzz
executables only; Hidlins has no direct dependency on a system LLVM library.
Those fuzz executables are non-distributable. Root `deny.toml` rejects NCSA,
the production lock and graph exclude the crate, and `fuzz/deny.toml` cannot
authorize any other package/version.

The NCSA boundary checker validates every packaging hook—host CLI/TUI/agent,
Flutter Linux/macOS, iOS, and Android—before compilation and scans output after
build. Its negative suite proves rejection of policy expansion, production
manifest/lock injection, missing hooks, copied fuzz executables, raw binaries,
recursive bundles, APKs, and IPAs containing NCSA/libFuzzer markers. Produced
host CLI/TUI/agent binaries, the macOS desktop application, the iOS device
application, and Android debug/release APKs were inspected clean with
`ncsa-boundary-check.py artifacts`; the Linux desktop packaging target has the
same fail-closed pre/post hooks and is built by its Linux CI job. A produced
artifact is required: source labels alone are not accepted as packaging
evidence.

## Composite-review repair register

| Finding | Fixed boundary | Regression evidence |
| ---: | --- | --- |
| 1 | Live and queued authorization is revocable | `LNS-REVIEW-001` |
| 2 | Rust, Swift, Android, and manifests share canonical service names | `LNS-REVIEW-002` |
| 3 | Flutter discovers and validates routes before startup sync | `LNS-REVIEW-003` |
| 4 | TUI uses a checked ephemeral bind and publishes a nonzero endpoint | `LNS-REVIEW-004` |
| 5 | Live pairing enforces candidate, failure, window, and cancellation limits | `LNS-REVIEW-005` |
| 6 | Absolute deadlines bound every blocking handshake/session boundary | `LNS-REVIEW-006` |
| 7 | Connection capacity is guard-owned and cannot underflow | `LNS-REVIEW-007` |
| 8 | Fetch is coherent, file-backed, and one-frame-at-a-time | `LNS-REVIEW-008` |
| 9 | Untrusted wrong-key/stale routes cannot block later pinned routes | `LNS-REVIEW-009` |
| 10 | Authorities safely rebind and republish allowed interface changes | `LNS-REVIEW-010` |
| 11 | Retired-transport matching rejects upper/lower/mixed case | `LNS-REVIEW-011` |

The mobile startup scenario now reaches the authority through the native
discovery repository after unlock and before the Rust startup operation; it no
longer pre-injects a route into the startup cache. Android retries normal NSD
scans when a just-rejoined guest temporarily retains an unreachable private
record, and proceeds only when a platform-discovered candidate is reachable.
This establishes the simulator/emulator application and native-adapter path,
not multicast behavior for arbitrary physical routers or VLANs.

No unresolved high-confidence critical or important finding remains in these
repaired paths. Physical OS wording, consumer-router firmware, human SAS
perception, and assistive-technology speech remain accurately scoped as
optional, non-gating observations. Lower-confidence speculative refactors and
new transport/service functionality were not promoted into this approved work
package.
