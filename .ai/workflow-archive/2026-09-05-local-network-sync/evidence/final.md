# Final Work-Package Evidence

## Work package

- ID: `2026-09-05-local-network-sync`
- Title: Local Network Sync
- Acceptance round: 2
- Approved plan revision: 7
- Approved plan SHA-256: `8b1d0a7545b26441cb70ae018b57a04991e444aba74bb0c12ca4dc0b33e1bc7a`
- Final acceptance attempt: 2 of 2
- Final Git revision: `e6df55b84edb53649d4f7f6c36842803c08884d8`

## Delivered outcome

The unshipped S3-compatible transport, SigV4 implementation, credentials,
MinIO harnesses, UI surfaces, dependencies, and vendored-only graph have been
removed without a migration path. Hidlins now uses an authoritative,
local-network-only sync protocol across the Rust core, CLI, TUI, Flutter
desktop, iOS, and Android. It provides private-address enforcement with no
override, DNS-SD discovery, bounded Noise XX pairing with bilateral SAS
confirmation, pinned Noise IK reconnect, revocation, sealed/zeroizing identity
state, atomic KDBX handling, and existing loss-preserving merge semantics.

The CLI provides foreground serving with signal cleanup. TUI and desktop can
run a server only while explicitly enabled and while their process is alive.
Mobile applications are clients only. Every application attempts startup sync
once for a configured client and thereafter syncs only on explicit request;
saves never trigger sync.

## Whole-package review

The primary thread reviewed the cumulative implementation and evidence from
baseline `e6df55b84edb53649d4f7f6c36842803c08884d8`, including all 22 tasks and
the eleven earlier composite-review repairs. The closing review covered S3
eradication, address and socket-boundary policy, framing and protocol bounds,
Noise integration and zeroization, identity sealing, pairing/trust/revocation,
server concurrency and cancellation, CAS/merge/backup/atomic-write behavior,
discovery privacy and DHCP churn, application lifecycle behavior, mobile
client-only enforcement, dependency/vendor provenance, NCSA isolation,
packaged artifacts, simulator evidence, and operator/security documentation.

No unresolved high-confidence defect remains. The approved-plan hash matches,
`git diff --check` passes, all task evidence exists, the generated bindings are
current, and every frozen final command passes in order.

## Final-review findings fixed

- The Android scenario initially ran the Rust raw-mDNS advertiser beside the
  approved test-only `NsdManager` registrar. A regression failed before the raw
  publisher was removed from the Android scenario runtime. The standalone CLI
  remains the real Noise/sync authority, while only the instrumentation APK
  advertises its locally bound port.
- API 29 emulator diagnosis showed synthetic cellular and explicit Wi-Fi
  interfaces in the same subnet. The scenario now disables only emulator
  cellular data to represent a real Wi-Fi-only device; Android still chooses
  the route and address. There is no endpoint injection, forwarding, proxy,
  route injection, or synthetic discovery result.
- The first frozen aggregate gate exposed a test-orchestration race after the
  authority was restarted to add an entry. The scenario called low-level
  `AppSession.syncNow()` without the fresh discovery performed by the shipping
  `SyncController`. New red/green assertions require platform discovery before
  each simulated manual sync. The isolated Android suite and the restarted
  aggregate gate both pass on API 29 and API 36.

## Acceptance coverage

| Area | Result |
| --- | --- |
| S3 removal and no migration | PASS — source/API/UI/CI/tests/dependencies/vendor removal gate |
| Local-only network boundary | PASS — complete IPv4/IPv6 policy, socket rechecks, no override |
| Pairing and trust | PASS — bounded XX/SAS, bilateral commit, IK pinning, revocation |
| Data safety | PASS — KDBX validation, CAS, three-way merge, loser history, backup, atomic writes |
| Application surfaces | PASS — CLI, TUI, desktop, iOS, Android lifecycle and policy tests |
| Discovery and DHCP churn | PASS — Rust desktop mDNS interop plus native iOS/Android discovery scenarios |
| Mobile topology | PASS — server APIs rejected; startup/manual-only client scheduling |
| Supply chain | PASS — exact vendoring, patched Snow review, deny/audit gates |
| NCSA exception | PASS — `libfuzzer-sys` confined to fuzz development/testing and absent from artifacts |
| Automated acceptance | PASS — iPhone, iPad, API 29 phone, API 36 tablet against separate CLI authorities |

## Final quality gate

| Command | Result |
| --- | --- |
| `make verify` | PASS |
| `make ncsa-boundary-check` | PASS |
| `make check-ios` | PASS |
| `make app-test-ios-simulator` | PASS |
| `make app-build-ios` | PASS — simulator and unsigned device artifacts scanned clean |
| `make android-emulator-provision` | PASS — required API/ABI/form-factor AVDs installed |
| `make test-local-sync-mobile-scenarios HIDLINS_ANDROID_STRICT=1` | PASS — iPhone, iPad, Android API 29, Android API 36 |

The final aggregate evidence files report PASS at:

- `build/verification/mobile-local-sync/ios-iPhone-17-Pro-Max.json`
- `build/verification/mobile-local-sync/ios-iPad-mini-A17-Pro.json`
- `build/verification/mobile-local-sync/android-hidlins-api29.json`
- `build/verification/mobile-local-sync/android-hidlins-api36-tablet.json`

## Non-blocking limits

- Simulator automation cannot prove interoperability between desktop raw mDNS
  and every physical router/Android combination. The documented physical-device
  check remains optional and non-gating; it is not represented as executed.
- Human-perceived screen-reader behavior remains an optional manual confidence
  check and does not block this completed work package.
- Flutter's warning that the local Rust bridge plugin lacks Swift Package
  Manager support is a future compatibility notice, not a present build,
  packaging, security, or runtime failure.
