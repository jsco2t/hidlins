# Task 021: Establish Platform-Registered Android Emulator Discovery

Delegation: main-only

## Goal

Complete the deterministic two-emulator private-LAN topology with the real
Hidlins CLI as the authority and an instrumentation-only Android NSD registrar,
then prove that the shipping Android client discovers and validates the real
CLI listener without any client route injection.

## Context

Revision 6 established two Emulator 37.1.11 guests with distinct allowed
`wlan0` DHCP leases in one `/24` and ran the actual cross-built
`hidlins sync serve` process on the authority guest. The Rust `mdns-sd`
publisher reported successful registration under both shell and application
UIDs, but the shipping Android `NsdManager` browser received zero records.
Android documents shared-emulator NSD between Android applications, not
interoperability between arbitrary guest-native multicast sockets and the
platform NSD stack.

Revision 7 therefore permits an `androidTest`-only `NsdManager` registrar to
publish the real CLI listener. It changes only the emulator's advertisement
mechanism. The CLI still owns the entire server, Noise, protocol, vault, and
data path; the shipping client still discovers through its normal native
adapter and Rust validation.

## Scope

### In scope

- Test-first registrar, lifecycle, source-set, packaging, and anti-injection
  assertions.
- Existing Emulator >=36.5 two-device shared-Wi-Fi topology preflight and AVD
  provisioning for supported host ABIs.
- The test-only Android scenario-authority CLI build that reuses normal
  `hidlins sync serve` and excludes only unsupported clipboard paths.
- An instrumentation-only native NSD registrar running on the authority
  emulator and registering the CLI's local port for the exact V1 pairing or
  trusted service type.
- Bounded build/install/start/readiness/stop/cleanup orchestration for the CLI,
  test APK, registrar, client, and emulators.
- A native-discovery smoke pass through the shipping client plus
  machine-readable topology/boundary evidence.
- Automated desktop tests of the real Rust `mdns-sd` publisher/browser.
- Static source-set, dependency-graph, and built-artifact negative controls.
- The existing dedicated `make test-local-sync-android-lan-discovery` target.

### Out of scope

- The full pair/import/bidirectional/restart/revocation journey (Task 022).
- Changing production Rust discovery or Android client discovery architecture.
- Shipping an Android CLI, registrar, server, background service, or mobile
  server capability.
- Claiming that emulator coverage proves desktop-Rust-to-Android multicast
  interoperability across a physical access point/router.
- Host `vmnet`/TAP bridges, root privileges, physical-LAN traffic, ADB
  forwarding/reverse, emulator console redirection, manual routes, proxies,
  synthetic discovery, custom UDP discovery, or new external dependencies.

## Implementation requirements

- Add or strengthen the smallest failing tests before the registrar repair and
  record the red result. They must reject:
  - sync host/port Dart defines or any endpoint supplied to the client;
  - direct pairing candidate construction, manual endpoints, synthetic native
    results, or client-side test routes;
  - `10.0.2.2`, host loopback, DNS, public, documentation, multicast,
    unspecified, or otherwise disallowed sync data-plane destinations;
  - ADB forward/reverse, emulator redirection, or any sync proxy;
  - one-emulator, duplicate-serial, same-address, off-subnet, or Emulator <36.5
    topologies;
  - an NSD result that differs from the authority emulator's independently
    observed allowed `wlan0` address or the real CLI listener port;
  - registrar source/components outside `androidTest`, or registrar content in
    release/debug application APKs or distributable dependency graphs.
- Preserve the already-recorded real failure: raw Rust guest mDNS registration
  is not visible to the shipping `NsdManager` client on the pinned emulator.
  The green result must exercise Android platform registration; tests must not
  be weakened to reinterpret registration logs as discovery.
- Provision and boot a dedicated authority AVD using the same exact
  host-native ABI and pinned SDK image policy as the client. Require distinct
  serials and distinct allowed non-loopback `wlan0` addresses in one subnet.
- Pin/preflight Android Emulator >=36.5. An older emulator is a hard failure;
  no compatibility fallback may inject a route.
- Build and run the actual `hidlins` binary on the authority emulator with the
  pinned NDK and explicit test-only scenario feature. The only Android-specific
  feature reduction may remove unsupported clipboard operations/dependencies.
  Vault, peer management, secure prompt, Noise, protocol, and `sync serve`
  behavior must remain the normal CLI implementation.
- Implement the registrar only under `app/android/app/src/androidTest`. It must:
  - use Android `NsdManager.registerService()` with no plugin or dependency;
  - accept only a validated local port and `pairing` or `trusted` service kind;
  - use the exact V1 service type and non-sensitive/ephemeral instance data;
  - allow Android to select the active network and address rather than setting
    or transmitting an address;
  - report registration readiness through bounded test orchestration;
  - stay alive for the bounded discovery phase and always unregister on normal
    completion, cancellation, timeout, or process teardown;
  - never proxy bytes or participate in Noise, pairing, authorization, vault,
    or application-protocol state.
- Install the shipping application plus instrumentation APK on the authority
  only as test infrastructure. The production application APK being tested on
  the client remains the normal shipping client surface.
- Before any vault fetch/upload, start the CLI listener, register its local port
  through the test registrar, browse through the shipping client's
  `LocalDiscoveryController`, cross the Flutter bridge, and validate the
  resulting endpoint in Rust. The result must equal the authority `wlan0`
  address and CLI port, and a direct TCP connection to that CLI must be proven.
- Preserve Android permission behavior: API 33+ requests
  `NEARBY_WIFI_DEVICES`; `ACCESS_LOCAL_NETWORK` is required only where the
  platform defines/enforces it beginning with API 37, not API 36.
- The host may pass the local listener port and service kind only to the
  authority registrar. It must never pass an address, port, candidate, or
  discovery result to the client. Host ADB is limited to process lifecycle,
  installation, secure stdin, readiness, and redacted evidence collection.
- Keep secrets out of argv, environment, ADB command strings, logs, evidence,
  and process listings. Add cleanup traps for both emulators, CLI and registrar
  processes, installed test artifacts, and scratch state.
- Extend shipping-boundary checks to prove:
  - registrar classes/components/declarations exist only in the instrumentation
    APK and not release/debug Hidlins application APKs;
  - no normal CLI/TUI/Flutter release graph enables or packages the Android
    scenario-authority binary or feature;
  - Flutter Android remains client-only.
- Run the focused desktop discovery target and retain its coverage claim
  separately from the platform-NSD emulator claim.

## Acceptance criteria

- [ ] Fail-before/pass-after evidence covers registrar behavior, lifecycle,
  packaging isolation, exact route equality, and every route-injection ban.
- [ ] A dedicated Make target boots two host-native emulator processes on one
  private virtual Wi-Fi LAN and records distinct allowed DHCP addresses.
- [ ] The actual `hidlins sync serve` CLI runs on the authority emulator and is
  the only process handling Noise and sync traffic.
- [ ] An `androidTest`-only `NsdManager` registrar publishes only the CLI's
  validated local port/service kind, lets Android select the network/address,
  and unregisters reliably.
- [ ] The shipping Android client resolves the authority through its normal
  native/Flutter/Rust path to the exact `wlan0` address and CLI listener before
  vault transfer, with no endpoint supplied to the client.
- [ ] No `10.0.2.2` sync route, direct candidate, manual endpoint, ADB
  forward/reverse, emulator redirect, proxy, synthetic result, DNS hostname, or
  custom discovery protocol participates.
- [ ] Source, graph, and built-artifact checks prove the registrar and scenario
  authority are absent from every distributable application and mobile remains
  client-only.
- [ ] Automated real Rust desktop `mdns-sd` publisher/browser tests pass, with
  coverage recorded separately from emulator platform NSD.
- [ ] Focused validation and the unchanged standard quality gate pass.

## Validation

- `make test-local-sync-mobile-harness`
- `make test-local-sync-discovery`
- `make test-local-sync-android-lan-discovery HIDLINS_ANDROID_STRICT=1`
- `make check-android`
- `make android-artifacts-check`
- `make ncsa-boundary-check`

## Dependencies

Tasks 001–020

## Expected areas of change

- `app/android/app/src/androidTest/`
- `app/android/app/src/main/java/app/hidlins/LocalDiscoveryController.java`
- `crates/hidlins-cli/Cargo.toml`
- `crates/hidlins-cli/src/`
- `crates/hidlins-sync/Cargo.toml`
- `crates/hidlins-sync/src/discovery/`
- `tools/android-native/`
- `tools/local-sync-tests/`
- `app/integration_test/`
- `Makefile`
- `.github/workflows/ci.yml`
- shipping-boundary tests/scripts

## Risks / notes

The registrar is a test-sidecar for standards-based route publication, not a
server or discovery mock. It is acceptable only because it supplies no route to
the client and the subsequent socket, Noise session, and all sync bytes go
directly to the real CLI listener. Registration success is not acceptance:
client resolution, independent route equality, Rust policy acceptance, and CLI
connection ownership must all be proven.

This emulator scenario does not establish physical-router interoperability
between a desktop Rust advertiser and an Android client. That observation is
optional and non-gating, and documentation must not claim otherwise.
