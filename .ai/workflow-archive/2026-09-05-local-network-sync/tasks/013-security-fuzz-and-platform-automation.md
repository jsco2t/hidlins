# Task 013: Complete Security, Fuzz, Real-Process, and Platform Automation

Delegation: main-only

## Goal

Close every automatable DRD security/resilience requirement with stable test IDs, bounded fuzzing, real processes/networks, platform matrices, Makefile targets, and CI execution.

## Context

Security tests have accumulated with each implementation task. This task audits the complete requirements matrix, fills cross-component gaps, adds continuous fuzzing/corpus replay, and makes the repository-wide gates authoritative.

## Scope

### In scope

- Audit every FR/NFR against stable automated test IDs and fill all practical gaps.
- Exact `cargo-fuzz 0.13.2` tool pin, pinned nightly, isolated fuzz workspace, audited `libfuzzer-sys`, protocol/address/pairing-state fuzz targets, committed minimized corpora, stable corpus replay, and bounded CI campaigns.
- Revise the repository license policy to permit NCSA only for development/testing and only through the exact audited `libfuzzer-sys` package in the isolated fuzz workspace; keep NCSA rejected by the production workspace.
- Add executable NCSA-boundary enforcement covering license configuration, workspace/lockfile placement, production dependency closures, every release/application packaging target, and produced CLI/TUI/agent/Flutter desktop/iOS/Android artifacts.
- Full adversarial cases: MITM, substitution, spoof, replay, reorder, duplicate, truncate, downgrade, wrong key, revocation, oracle probing, malformed frames/messages, range bypass, candidate flood, connection/allocation/queue exhaustion, timeout/cancel, secret scanning, and panic containment.
- Multi-client CAS races, local-write races, DHCP/interface changes, server restart/lock, and fault injection at every pairing/upload/commit boundary.
- Real CLI/client/server processes on macOS/Linux loopback; Linux isolated private IPv4 and IPv6 ULA/link-local namespaces where supported; deterministic simulations on unsupported host paths.
- Flutter bridge, permission adapters, desktop lifecycle, iOS simulator, Android emulator, and cross-target compile coverage.
- Final Makefile targets: `test-local-sync-security`, `test-local-sync-discovery`, `test-local-sync-integration`, `fuzz-local-sync-corpus`, `fuzz-local-sync-ci`, `ncsa-boundary-check`, and `s3-removal-check`.
- CI jobs/steps invoking only Make targets and `make verify` integration.

### Out of scope

- Physical-device permission UI, human SAS comparison, router-specific multicast, or screen-reader observation.
- Product behavior changes not required to close a high-confidence test finding.
- Permitting NCSA for production code, application functionality, any package other than the exact audited fuzz dependency, or any distributed Hidlins binary/application.

## Implementation requirements

- New dependency/tool execution follows the full permissive-license/source/transitive review before use.
- `libfuzzer-sys` is a development/test-only exception: pin its exact version; keep the fuzz crate in an isolated non-production workspace; allow NCSA only through a crate/version-scoped fuzz-policy exception; and retain the production `deny.toml` rejection of NCSA.
- Add a first-party `ncsa-boundary-check` with self-tested negative controls. It must reject `libfuzzer-sys` or any NCSA allowance in the production workspace, an exception for any other crate/version/location, fuzz workspace membership in the production workspace, missing boundary hooks on a packaging target, and representative contaminated application artifacts.
- Every Make target that produces a distributable CLI, TUI, agent, Flutter desktop, iOS, or Android artifact must run the production dependency boundary before building and inspect the produced artifact afterward. CI and `make verify` must run the same checks; fuzz executables must never be copied into product artifact roots.
- Dependency-graph checks are authoritative for compile inclusion and artifact checks independently reject recognizable libFuzzer runtime/source/license markers and fuzz executables in recursive directories and supported archive/package formats. Tests must demonstrate both layers fail closed.
- Fuzz CI is time-bounded and deterministic enough to avoid flaky gating; every discovered crash is minimized and replayed by stable offline tests.
- Tests use controlled clocks, injected discovery, bounded fake sockets, and isolated state directories rather than sleeps/live multicast when equivalent.
- Real-network tests never contact or bind public addresses and assert actual peer classification.
- Secret scanning seeds unique markers across logs, errors, JSON, events, panic paths, temporary files, and process output.
- Resource tests demonstrate bounded memory/thread/queue behavior without destabilizing shared CI hosts.
- Any high-confidence defect found is repaired within the approved architecture with red/green evidence.

## Acceptance criteria

- [ ] Every automatable DRD requirement maps to at least one passing stable test ID and CI Make target; manual-only rows include a written technical justification.
- [ ] Fuzz targets compile and complete bounded campaigns on supported CI hosts; committed corpora replay under the stable offline gate.
- [ ] NCSA is allowed only for the exact audited `libfuzzer-sys` development/test dependency in the isolated fuzz workspace; the production license policy and dependency graphs continue to reject it.
- [ ] Executable negative controls prove that misplaced/expanded NCSA allowances, production dependency injection, missing package hooks, fuzz-artifact copying, and contaminated CLI/TUI/agent/Flutter desktop/iOS/Android artifacts fail the boundary gate.
- [ ] Every product packaging workflow runs the boundary checks, and all produced application artifacts pass inspection without NCSA/libFuzzer code or fuzz executables.
- [ ] All listed adversarial, address, privacy, resource, lifecycle, pairing, concurrency, crash, and removal cases pass.
- [ ] Real multi-process local sync passes on macOS/Linux loopback and Linux isolated private/IPv6 networks where supported without public traffic.
- [ ] iOS simulator and Android emulator/native adapter tests cover permission/discovery/lifecycle behavior; mobile listener absence is statically and dynamically enforced.
- [ ] CI and `make verify` run the local-sync security, discovery, integration, fuzz-corpus, S3-removal, NCSA-boundary, merge, interop, and supply-chain gates with local/CI command parity.

## Validation

- `make test-local-sync-security`
- `make test-local-sync-discovery`
- `make test-local-sync-integration`
- `make fuzz-local-sync-corpus`
- `make fuzz-local-sync-ci`
- `make ncsa-boundary-check`
- `make s3-removal-check`
- `make deny`
- `make check-ios`
- `make check-android`

## Dependencies

- Task 012

## Expected areas of change

- `crates/hidlins-sync/tests/`
- `crates/hidlins-api/tests/`
- `crates/hidlins-cli/tests/`
- `crates/hidlins-tui/src/*tests*`
- `app/test/`
- `app/integration_test/`
- `app/ios/RunnerTests/`
- `app/android/app/src/androidTest/`
- `fuzz/`
- `tools/local-sync-tests/`
- `tools/dev/install-toolchain.sh`
- `tools/dev/ncsa-boundary-check.py`
- `CLAUDE.md` / `AGENTS.md`
- `deny.toml`
- `fuzz/deny.toml`
- `Makefile`
- `.github/workflows/ci.yml`
- `Cargo.lock`
- `vendor/`

## Risks / notes

Network namespace and sanitizer availability differ by host. CI must gate the strongest supported environment and use deterministic equivalent adapters elsewhere; unsupported capability is reported explicitly, never silently treated as a pass for a required assertion.

`libfuzzer-sys` may use `[dependencies]` inside the isolated fuzz executable crate even though it is development-only in product terms. Isolation, the package-scoped license exception, production graph rejection, packaging hooks, and artifact inspection collectively enforce the shipping boundary; naming it a Cargo `dev-dependency` is not relied upon as a security control.
