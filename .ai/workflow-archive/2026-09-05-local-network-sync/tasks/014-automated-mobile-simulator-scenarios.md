# Task 014: Automate Mobile Simulator Client Scenarios

Delegation: main-only

## Goal

Create and execute deterministic end-to-end iOS Simulator and Android Emulator scenarios in which the real mobile application is strictly a client of a separate host CLI authority.

## Context

The existing shared real-bridge scenario creates both endpoints inside one application process, calls server APIs that production mobile builds deliberately reject, and verifies only an already-in-sync state. Existing Android evidence predates that scenario's addition. Revision 4 replaces blocking physical/manual acceptance with the strongest faithful automation available and requires the expensive scenarios to remain isolated from routine developer checks.

## Scope

### In scope

- Add a host-side scenario orchestrator using the real Hidlins CLI authority and real mobile simulator application artifacts.
- Add client-only iOS Simulator and Android Emulator scenario drivers over the shipping Rust bridge.
- Repair or split the current shared real-bridge scenario so mobile tests never start a server, while retaining valid desktop coverage.
- Cover rejected and accepted pairing, pair-and-import, bidirectional vault changes, explicit sync, startup/manual-only scheduling, authority restart or route replacement, client-process restart with pinned trust, revocation, and mobile-server rejection.
- Provision/select/boot supported simulators deterministically, validate the runtime identity, and clean up applications, devices, processes, ports, and disposable vault state on success, failure, timeout, and interruption.
- Emit redacted logs and machine-readable evidence containing Git revision, artifact hashes, host/simulator identity, scenario steps, and PASS/FAIL without secret material.
- Add platform-specific Make targets plus one discoverable aggregate `make test-local-sync-mobile-scenarios` target; keep this expensive category out of routine `make check` and `make verify` while invoking the platform targets from dedicated CI jobs.

### Out of scope

- Enabling a mobile server or background service.
- Adding a production test backdoor, weakening SAS/trust/address validation, or using public/DNS endpoints.
- Claiming simulator evidence proves physical permission wording, consumer-router multicast, or accessibility output.
- New external dependencies.

## Implementation requirements

- Write the smallest failing regression first for each missing scenario capability and record red-before/green-after evidence.
- The authority must be a separate spawned `target/release/hidlins` or repository-built CLI process with an isolated registry and synthetic KDBX data; do not substitute an in-process mobile server.
- iOS uses an allowed loopback or host-private literal reachable from the Simulator. Android uses the emulator host alias `10.0.2.2` or an equally explicit validated mapping. Discovery may be exercised where available, but the restricted manual endpoint is the deterministic fallback.
- Drive the real bundled Rust bridge inside the installed simulator application. Widget-only fakes do not satisfy this task.
- Prove one authority-side mutation reaches the client and one client-side mutation reaches the authority. Compare stable non-secret fixture fields and KDBX validity, not secret values in logs.
- Restart the authority and mobile application process, then prove the client reconnects under the existing pinned key without re-pairing. Exercise an allowed route replacement where supported.
- Prove saving alone and waiting do not trigger another sync, while explicit Sync now does. Prove mobile server endpoint/start APIs fail before binding.
- Pair rejection and authority revocation must leave/finally restore no usable authorization.
- Bounds, process deadlines, port allocation, interrupt handling, output redaction, and cleanup must be deterministic and tested with negative controls.
- `make help` documents the aggregate expensive target. CI uses Make targets only and does not duplicate raw harness commands.

## Acceptance criteria

- [ ] The iOS Simulator scenario passes against a separate host CLI authority using the real installed application and bundled Rust bridge.
- [ ] The Android Emulator scenario passes against a separate host CLI authority using the real installed application and bundled Rust bridge.
- [ ] Both platforms prove rejected/accepted pairing, pair/import, authority-to-client and client-to-authority changes, explicit/manual-only sync, authority and client restart with pinned trust, revocation, and absence of mobile serving.
- [ ] The scenario suite uses only policy-allowed local endpoints and proves no public/DNS fallback or test bypass exists.
- [ ] The isolated aggregate Make target is discoverable, bounded, reproducible, excluded from routine fast gates, wired to dedicated platform CI jobs, and emits redacted machine-readable evidence.
- [ ] The obsolete same-process mobile-server assumption is removed while valid desktop real-bridge coverage remains passing.

## Validation

- `make test-local-sync-mobile-scenarios HIDLINS_ANDROID_STRICT=1`
- `make test-local-sync-integration`
- `make boundary-check`
- `make ncsa-boundary-check`

## Dependencies

- Task 013

## Expected areas of change

- `Makefile`
- `.github/workflows/ci.yml`
- `app/integration_test/`
- `app/test_driver/`
- `tools/ios-native/`
- `tools/android-native/`
- `tools/local-sync-tests/`
- `crates/hidlins-cli/tests/`
- `crates/hidlins-api/`
- `build/verification/` outputs (ignored runtime evidence)

## Risks / notes

This is a security-, lifecycle-, and process-coordination-sensitive task. The test harness must fail closed rather than silently skipping a platform or converting an incomplete scenario into a pass.

The aggregate target is intentionally expensive and is not a dependency of routine `make check` or `make verify`; it is nevertheless mandatory for this task, final acceptance, and the dedicated CI coverage.
