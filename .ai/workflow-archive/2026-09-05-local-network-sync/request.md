# Local Network Sync Work Request

## Human invocation

`$feature-workflow /Users/jason/Developer/sources/personal/notebook/projects/hidlins/features/local-network-sync/drd.md`

The referenced approved DRD is the source requirement for this work package:

`/Users/jason/Developer/sources/personal/notebook/projects/hidlins/features/local-network-sync/drd.md`

## Required outcome

Completely remove the unshipped and unused S3-compatible sync implementation and replace it with a secure, local-network-only synchronization system across the CLI, TUI, Flutter desktop, iOS, and Android application surfaces.

### Product and topology requirements

- Each synchronized vault has exactly one designated authoritative Hidlins server and may have multiple explicitly paired, individually revocable clients.
- DNS-SD/mDNS automatic discovery is required in V1 so DHCP address changes require no user maintenance. Discovery supplies untrusted routing candidates only and must not establish identity or advertise vault names, public keys, stable identifiers, or secret metadata.
- A restricted literal address-and-port diagnostic fallback is allowed where multicast is unavailable. V1 does not accept DNS hostnames.
- Synchronization remains offline-first and optional.
- Every application attempts one bounded sync after a configured vault is successfully unlocked on startup. A one-shot CLI vault command attempts sync before the requested operation. After that, synchronization happens only when the user explicitly requests it; automatic post-save/post-mutation sync must be removed.
- The CLI provides a foreground sync server command that exits safely on `Ctrl+C`.
- The TUI and desktop application provide an explicit server toggle that keeps the server running only while the application is running and the vault is unlocked. Lock stops the listener and zeroizes usable secrets; a previously enabled server may restart only after successful unlock.
- Normal startup does not implicitly enable persistent server mode.
- iOS and Android are foreground clients only. Persistent/background service or server operation is explicitly unsupported and must not be exposed.
- Pair-and-import replaces remote S3 bootstrap for a client without a local vault.
- All applicable flows—discovery, pairing/import, manual sync, status, errors, server controls, peer management, and revocation—must be implemented across their required application surfaces, with the Rust core owning business/security logic.

### Network and protocol requirements

- Internet synchronization is out of scope and prohibited without an escape hatch.
- The exhaustive allowed sync endpoint ranges are IPv4 `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `169.254.0.0/16`, and `127.0.0.0/8`; IPv6 `fc00::/7`, `fe80::/10`, and `::1`.
- Carrier-grade NAT, globally routable, documentation, benchmarking, unspecified, multicast sync destinations, IPv4-mapped bypasses, and every other address are rejected.
- Address validation applies to advertisements, discovered candidates, each outbound connection, and each accepted inbound connection. mDNS multicast is allowed only for discovery.
- Pair with exact `Noise_XX_25519_ChaChaPoly_SHA256`, a three-minute explicit pairing window, no more than three failed attempts, a handshake-derived SAS confirmed at both endpoints, and an authenticated commit/acknowledgement that leaves no partial trust on any failed path.
- Reconnect with exact `Noise_IK_25519_ChaChaPoly_SHA256` and pinned static keys at both ends. Do not negotiate suites, downgrade, silently re-pair, or trust discovery metadata.
- Use a per-vault/per-installation static X25519 identity encrypted under vault-protected key material. Never store it in plaintext; zeroize plaintext identity and session secrets. Master-password changes rewrap without changing identity.
- Carry a small versioned and bounded application protocol inside short-lived Noise transport sessions. It supports version inquiry, conditional fetch/upload, bounded chunks, explicit commit, structured errors, cancellation, and fail-closed handling of invalid order, replay, duplication, truncation, stale versions, timeouts, and disconnects.
- Preserve the existing transport-neutral sync truth table, three-way merge, one bounded CAS retry, loser-as-history preservation, `.kdbx.bak`, advisory locking, KDBX/KDF validation, and atomic writes.
- Before authenticated authorization the server must not disclose vault existence, name, version, size, KDF details, peer list, metadata, or encrypted bytes.

### Security, quality, and supply-chain requirements

- The implementation is security-critical and requires a documented threat model plus focused protocol, cryptography-integration, memory-hygiene, address-boundary, concurrency, storage, and UI/bridge review.
- Use an exact, minimally featured `snow` dependency rather than hand-rolled cryptography.
- Commit to a narrow Hidlins-maintained patch that zeroizes Snow handshake, transport, cipher, chaining-key, hash, ephemeral-key, and static-key secret state until an upstream release is independently verified to provide equivalent coverage.
- All new, retained, patched, copied, or platform-native dependencies must use an allowed permissive license and have exact version/source, maintenance, audit history, enabled-feature, transitive-footprint, alternatives, and vendored-source review evidence.
- Remove `ureq`, S3/HTTP-only dependencies, and their unreachable transitive/vendor graph where they have no other consumer.
- Keep external dependencies as thin as possible; do not hand-roll cryptography, TLS, authentication primitives, or other security-sensitive machinery.
- Use safe Rust for protocol, framing, discovery input, address canonicalization, and state transitions. Any unavoidable platform-boundary `unsafe` requires a narrow audited wrapper, safety argument, and tests.
- Enforce explicit bounds for frames, messages, chunks, transfers, connections, attempts, timeouts, queues, and retries. The outer TCP frame may not exceed 65,535 bytes.
- Network sync transfers only encrypted KDBX bytes and must preserve KDBX3-read/KDBX4-write and KeePassXC interoperability.
- Logs, errors, events, UI text, and CLI JSON must not expose key material, SAS derivation inputs, vault contents, credentials, or sensitive protocol payloads. No telemetry or remote reporting is added.

### Test-forward and verification requirements

- Test-before-implementation applies to new behavior; test-before-removal applies to S3 behavior and transport/merge refactors. Each implementation task records red-before/green-after evidence where a meaningful failing test can exist.
- Automate every expected security behavior that can be tested deterministically or with controlled fault injection.
- Automated coverage includes: Noise configuration and state conformance; successful and failed pairing; SAS mismatch; MITM substitution; spoofed discovery; all public/special-address rejection and normalization edge cases; pinned-key mismatch; revoked clients; replay, reordering, duplication, truncation, and downgrade attempts; malformed framing; allocation/connection exhaustion; stale and concurrent writes; disconnect/crash at commit boundaries; zeroization obligations; secret-free output; permission adapters; DHCP/address changes; lifecycle behavior; and total S3 removal.
- Fuzz stateful parsers and message ordering. Property-test address policy, framing, synchronization invariants, merge preservation, and conditional updates.
- Exercise real client/server processes on controlled loopback/private networks for macOS and Linux and automate Flutter bridge/platform-adapter behavior where simulation is faithful.
- Add discoverable Makefile targets for every new test/fuzz/integration workflow and invoke those targets from CI.
- Manual verification is still required, but only for irreducible human or physical-platform behavior: SAS comparison/rejection, real iOS/Android permission presentation, representative physical multicast/router behavior, real-device lifecycle, and assistive technology.

### Explicit deletion and compatibility boundary

- There is zero S3 migration. S3 has not shipped and is not used locally.
- Delete S3 code, SigV4, configuration/credential schemas, APIs, DTOs, UI, tests, MinIO harnesses, build/CI targets, documentation, and exclusive dependencies.
- Do not add migration, import, translation, legacy parsing, aliases, compatibility shims, or deprecation behavior.
- Historical immutable workflow archives or provenance documents may continue to mention S3.

### Out of scope

- Public-Internet sync, relays, cloud services, VPN exceptions, and public-address overrides.
- Peer mesh or multiple authoritative servers for one vault.
- Persistent/background mobile serving.
- A new operating-system daemon or implementation of the future `hidlins` service/agent.
- Account systems, certificate authorities, cloud-mediated pairing, or a proprietary vault format.

## Approved-plan revision — Revision 2

Task 13 requires a change to our license policy to use `libfuzzer-sys`. A linked dependency includes NCSA-licensed code. The human approves an adjustment to the license policy to allow NCSA-licensed code under these firm requirements:

1. Code covered by the NCSA License can **only** be used for development or testing of Hidlins.
2. Absolutely no packaged or distributed binary/application produced by Hidlins may contain NCSA-licensed code. This includes the CLI, TUI, and every Flutter desktop or mobile application.
3. The policy change and its limits must be enforced in code.

The preceding discussion establishes that `libfuzzer-sys` is a Rust crate used by the isolated fuzz workspace and that its default build compiles bundled NCSA-licensed LLVM libFuzzer sources into fuzz executables. Those fuzz executables are development/CI artifacts only. The approved exception does not authorize NCSA in the production Cargo workspace, shipping dependency graphs, release binaries, application bundles, or unrelated dependencies.

## Approved-plan revision — Revision 3

The manual verification document must include precise, copy-pasteable instructions for performing the applicable local-network sync scenarios with iOS Simulator and Android Emulator. It must include the local commands required to provision and start each simulator/emulator, build/install/launch the development application, start the host authority, select an emulator-reachable local endpoint, pair or pair-and-import, exercise basic bidirectional client sync, restart the client, and clean up.

Simulator/emulator execution is sufficient evidence for basic mobile client synchronization. The documentation must distinguish that coverage from properties a simulator cannot faithfully establish, including physical OS permission presentation, representative real-router multicast/DHCP behavior, and physical-device assistive-technology output; those irreducible observations remain separate requirements.

## Approved-plan revision — Revision 4

The human requires Hidlins to leverage local iOS/Android simulators together with the CLI or TUI to perform end-to-end scenario testing instead of the previously requested blocking manual verification. Testing must be automated as much as possible, including scenario-level verification. Because these simulator scenarios are expected to be substantially more expensive than normal tests, they must be exposed through an isolated, discoverable `make` target.

Only testing that genuinely requires human observation, such as accessibility testing, may remain manual. No remaining manual test may block completion of this work package. This supersedes the earlier requirement that missing physical devices, a representative router, human SAS observation, or assistive-technology sessions force the workflow to remain `BLOCKED`.

## Acceptance follow-up — Round 2

The human requires all eleven high-confidence findings from the post-completion composite review to be fixed:

1. Revoked peers retain access through already-authenticated sessions.
2. iOS and Android browse DNS-SD service types different from those advertised by the Rust authority.
3. Flutter startup sync runs before discovery and normally has no routing candidate after process restart.
4. The TUI authoritative server requests port zero through an endpoint type that rejects port zero and therefore cannot start.
5. The live server does not enforce the one-active-candidate, three-failure, or active-window cancellation pairing policy.
6. Handshake, connection, and session deadlines do not bound blocking socket I/O.
7. Rejected inbound source addresses can underflow the active-connection counter.
8. Maximum-size fetches duplicate the complete encrypted vault into an in-memory vector of chunk messages and can exhaust memory.
9. A spoofed or stale first discovery candidate with the wrong Noise identity prevents trying a later legitimate authority.
10. A running authority does not rebind or republish after DHCP/interface-address changes.
11. The S3-removal gate misses uppercase `MINIO`, allowing a stale first-party reference and future case-variant regressions.

The fixes must remain test-forward, preserve the frozen local-only address and pinned-identity security boundaries, introduce no new external dependency, and add automated regression coverage at the real runtime/surface boundary rather than relying only on isolated state-model tests.

## Approved-plan revision — Revision 6

The human requires investigation and repair of Android Emulator discovery. The
repair must mimic a live local-network environment rather than manufacture a
test-only success. In particular, manually supplying the application with the
authority address or port does not prove that a real device would discover and
sync with an authority on its local network.

The Android end-to-end scenario must therefore exercise secure automatic
discovery through the shipping Android native discovery adapter and the normal
Rust candidate-validation path. It must not inject sync candidates, pass a sync
host or port through Dart defines, use `10.0.2.2` as a hidden sync route, or use
ADB TCP/UDP forwarding as a substitute for discovery. Test orchestration may
use a separate authenticated control plane only when it cannot influence the
sync endpoint selected by the application.

The final-acceptance retry budget is explicitly reset for this revision. All
previously completed tasks and evidence remain complete and immutable; the
repair is new work numbered after Task 020.

## Approved-plan revision — Revision 7

The human approves revising the emulator test architecture without changing
the shipping discovery design. The failure is treated as specific to
advertising mDNS directly from the Rust CLI inside an Android emulator: Android
documents shared emulator Wi-Fi NSD between Android applications, but does not
establish identical behavior for arbitrary native mDNS sockets. Changing the
CLI process UID did not make its advertisement visible to the shipping Android
`NsdManager` browser.

The revised solution must:

1. Keep the real `hidlins sync serve` CLI running on the authority emulator. It
   remains the actual Noise-authenticated synchronization server and processes
   every pairing and synchronization request.
2. Add an `androidTest`-only NSD registrar on the authority emulator. It uses
   `NsdManager.registerService()` to advertise the CLI's locally bound port.
   Android chooses the service network and address; the registrar does not
   supply an address to the client.
3. Have the shipping Android application discover the service through its
   existing `NsdManager` implementation, pass it through the Flutter bridge and
   normal Rust private-endpoint validation, and connect directly to the real
   CLI server.
4. Continue to prohibit every discovery or routing shortcut: no endpoint is
   passed to the client, no `10.0.2.2` sync route, no ADB forwarding or reverse
   forwarding, no proxy, no synthetic discovery result, and no route injection.

The registrar knowing the server's locally bound port is ordinary server-side
service registration, not client route injection. The client may receive the
route only from Android NSD. The registrar must exist exclusively in the
instrumentation/test APK. Automated packaging and dependency-graph checks must
prove that its classes, components, and any test-only permissions are absent
from every distributable CLI, TUI, and Flutter application. Mobile Hidlins
applications remain foreground clients only, and this test mechanism adds no
shipping mobile server capability or external dependency.

Verification is split into three layers:

- An automated two-emulator scenario using Android NSD for discovery and the
  real CLI for the complete Noise pairing/trusted-sync data path.
- Automated desktop tests of the actual Rust `mdns-sd` publisher/browser
  interoperability.
- An optional, non-gating physical-device test of desktop Rust advertisement
  to an Android `NsdManager` client over a representative physical LAN.

Simulator-only coverage must not claim to prove desktop raw-mDNS-to-Android
interoperability across real access points or router firmware. That exact
boundary requires the optional physical-device test and does not block work
package completion. Do not introduce a custom UDP discovery protocol or weaken
automatic discovery with stored/manual addresses. Preserve the correction that
`ACCESS_LOCAL_NETWORK` runtime enforcement begins with Android 17 / API 37,
not API 36.

Task 021 must be revised to permit and tightly packaging-gate the test-only
`NsdManager` registrar while retaining the CLI as the real authority. Task 022
must use that topology for its automated mobile scenarios.
