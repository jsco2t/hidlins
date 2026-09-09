# Task 022: Complete Platform-Discovered Android Mobile Acceptance

Delegation: main-only

## Goal

Run the complete Android mobile synchronization journey through the shipping
client's native automatic discovery and the real CLI authority, prove
rediscovery after authority address change, and update evidence/documentation
with precise emulator and physical-LAN coverage boundaries.

## Context

Task 021 establishes a real two-device private virtual LAN, runs the actual CLI
authority on one emulator, publishes that CLI's local port through an
instrumentation-only `NsdManager` registrar, and proves the shipping Android
client resolves the resulting route through its normal native/Flutter/Rust
path. This task uses that topology for every pairing and trusted-sync operation
in the API 29 phone and API 36 tablet scenario matrix.

The registrar is not a sync server or a client route injection mechanism. It
publishes only server-side service registration; all Noise and sync traffic
connects directly from the shipping client to the real CLI listener.

## Scope

### In scope

- Full Android pair/import, startup sync, manual-only behavior, bidirectional
  mutation, process restart, authority restart/address churn, revocation, and
  mobile-server rejection through platform-discovered real-CLI routes.
- Removal of Android sync endpoint defines and direct scenario candidates.
- Starting/stopping the authority-side test registrar for pairing and trusted
  service phases while the CLI remains the sole data-plane server.
- Address-change execution using preserved encrypted authority state/identity
  on a replacement authority emulator with a different private DHCP address.
- Redacted machine-readable evidence separating registrar/control/data planes.
- Harness, integration, documentation, acceptance manifest, and CI updates.
- Execution of repaired expensive Android and aggregate gates.

### Out of scope

- Product removal of the explicitly allowed manual diagnostic endpoint feature.
- Product changes to desktop Rust mDNS publication or shipping Android NSD.
- Shipping the test registrar, Android CLI authority, or any mobile server.
- Treating emulator tests as proof of physical access-point/router multicast
  interoperability.
- Making optional physical-device/router or accessibility observations block
  completion.
- Privileged host bridges, public networks, route injection, forwarding,
  proxies, synthetic discovery, custom UDP discovery, or new dependencies.

## Implementation requirements

- Add or strengthen the smallest scenario-source/harness regressions first and
  record their red result. Tests must fail if Android pairing or sync:
  - receives `HIDLINS_SCENARIO_SYNC_HOST`, `HIDLINS_SCENARIO_SYNC_PORT`, or any
    equivalent endpoint input;
  - constructs an authority route from a test constant;
  - calls `setDiscoveryCandidates`, `setManualEndpoint`, or session pairing with
    a test-provided candidate;
  - receives an address or resolved route from the registrar/control plane;
  - uses `10.0.2.2`, ADB forward/reverse, emulator redirection, a proxy, DNS
    hostname, manual endpoint, or synthetic native result as the sync path;
  - permits registrar classes/components in a release/debug application APK.
- Drive rejected and accepted pair/import through the instrumentation-published
  pairing service, shipping `MethodChannelLocalDiscovery`, Flutter
  repository/controller boundary, Rust candidate cache/policy, and normal Noise
  XX flow. Drive startup and later manual reconnect through a newly registered
  trusted service, native discovery, and Noise IK.
- For each authority phase:
  1. start the actual CLI listener on its observed allowed `wlan0` interface;
  2. start the test registrar with only the validated CLI port and service kind;
  3. require `NsdManager` registration readiness;
  4. require the client to resolve the authority's independently observed
     address and actual CLI port through shipping discovery;
  5. connect directly to that CLI and complete the expected authenticated flow;
  6. unregister/stop the registrar on phase completion or failure.
- Host control traffic, if retained for mid-process synchronization, must use a
  distinct authenticated control-only configuration. It must never communicate
  a sync address/port, select or rewrite a candidate, register a service on the
  client, proxy sync bytes, or appear as a Rust candidate. Evidence must name
  and separate this control plane.
- Preserve every existing scenario assertion: mobile cannot serve; rejected
  pairing leaves no peer; accepted pairing imports; startup fetches authority
  changes; saving does not sync; manual sync sends the client change; process
  restart preserves sealed identity/trust; authority restart recovers; revoked
  trust fails closed.
- Exercise address churn by copying only encrypted vault/registry state through
  host orchestration to a replacement authority emulator, starting the same
  CLI/identity on its distinct allowed DHCP address, and registering its local
  port from that emulator's test APK. Prove the client rediscovers the new route
  without any hint and accepts it only after authenticating the preserved
  pinned identity.
- Run the client matrix on API 29 phone and API 36 tablet for each supported CI
  host ABI. Do not mark a missing or failed Android run as skipped when
  `HIDLINS_ANDROID_STRICT=1`.
- Expand evidence with:
  - emulator version, client/authority serials, AVDs, APIs, and actual private
    `wlan0` addresses;
  - test registrar source-set/APK identity, exact service kind, locally supplied
    port, Android-selected resolved route, and clean unregister result;
  - Rust-accepted endpoint and proof the authenticated socket/data path was the
    real CLI process;
  - authenticated phase, old/new authority route inequality, app/CLI hashes,
    encrypted restart hashes, packaging checks, and cleanup result;
  - explicit `route_injection=false`, `sync_proxy=false`,
    `client_endpoint_input=false`, and `registrar_test_only=true` fields.
  Redact secrets, SAS, private scratch paths, and protocol contents.
- Correct active verification, running/testing, release-review, acceptance
  JSON, security coverage, and test-matrix documents. They must distinguish:
  - emulator Android-platform NSD plus real CLI Noise/data-plane coverage;
  - automated real Rust desktop `mdns-sd` publisher/browser coverage;
  - optional non-gating physical desktop-Rust-to-Android discovery across a
    representative access point/router.
  They must not claim that the emulator proves the final physical multicast
  boundary or that `10.0.2.2`/manual routing proves Android discovery.
- Document the optional physical-device observation accurately without marking
  it executed or required for DONE. Preserve the established non-gating status
  of physical permission presentation, representative router behavior,
  lifecycle observation, SAS human comparison, and assistive technology.
- Preserve iOS scenario behavior and the aggregate Make target. Do not weaken
  any standard or final command.

## Acceptance criteria

- [ ] Test-forward evidence shows the prohibited injected topology and
  packaging leakage fail the new contract before the repaired scenario passes.
- [ ] API 29 phone and API 36 tablet complete every existing scenario step
  using shipping native discovery and direct Noise/sync connections to the real
  CLI authority.
- [ ] The test-only registrar receives only the local CLI port/service kind;
  Android selects the route, the client receives it only through NSD, and the
  registrar never handles sync bytes or authentication.
- [ ] A replacement authority emulator uses a different allowed DHCP address;
  the client rediscovers it without a hint and authenticates the preserved
  pinned Noise identity.
- [ ] Machine-readable evidence proves no manual/injected/proxied/forwarded sync
  route participated, identifies CLI ownership of the data path, and cleanly
  separates instrumentation registrar, control plane, and sync data plane.
- [ ] Built artifacts prove the registrar and Android scenario authority remain
  test-only and absent from every distributed application.
- [ ] Android, security, acceptance, operator, simulator, and release documents
  make only demonstrated claims and keep physical desktop-to-Android multicast
  observation optional and non-gating.
- [ ] Focused validations, unchanged standard gate, and every final command pass
  in order.

## Validation

- `make test-local-sync-mobile-harness`
- `make test-local-sync-discovery`
- `make test-local-sync-android-lan-discovery HIDLINS_ANDROID_STRICT=1`
- `make test-local-sync-mobile-scenarios-android HIDLINS_ANDROID_STRICT=1`
- `make acceptance-evidence-check`
- `make s3-removal-check`

## Dependencies

Task 021

## Expected areas of change

- `app/android/app/src/androidTest/`
- `tools/local-sync-tests/mobile_scenario.py`
- `tools/local-sync-tests/mobile_scenario_test.py`
- `tools/android-native/lan_discovery.py`
- `tools/android-native/run_emulator_suite.sh`
- `tools/android-native/scenario_boundary.py`
- Android emulator provisioning/matrix tooling
- `app/integration_test/mobile_local_sync_scenario_test.dart`
- `app/android/app/src/main/java/app/hidlins/LocalDiscoveryController.java`
- `Makefile`
- `.github/workflows/ci.yml`
- `app/android/VERIFICATION.md`
- `docs/local-network-sync-manual-verification.md`
- `docs/running-and-testing.md`
- `docs/local-network-sync-release-review.md`
- `docs/flutter-alpha-acceptance.json`
- `crates/hidlins-sync/docs/security-coverage.md`
- `crates/hidlins-sync/docs/test-matrix.md`

## Risks / notes

The emulator scenario proves platform NSD, shipping client discovery, Rust
private-address validation, and the complete real CLI Noise/data plane. It does
not prove raw Rust desktop multicast traversal through arbitrary physical
router firmware. Only the optional physical-device observation covers that
last environmental boundary.

Any implementation that gives the client an endpoint through instrumentation
or control traffic, proxies the sync stream, moves authority logic into the
test APK, or packages the registrar in a normal application violates this task
even if the scenario passes.
