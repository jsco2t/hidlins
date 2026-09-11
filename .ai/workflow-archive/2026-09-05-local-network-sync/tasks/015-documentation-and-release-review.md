# Task 015: Complete Documentation and Release Review

Delegation: main-only

## Goal

Make the shipped behavior and automated simulator scenarios authoritative in active product/operator documentation, record non-blocking manual residuals honestly, and complete focused security and supply-chain release review.

## Context

Tasks 001–014 implement and automate the local-network sync model. Revision 4 makes faithful simulator/CLI scenario evidence the completion gate and limits manual work to non-blocking observations that genuinely require a person or physical environment.

## Scope

### In scope

- Update README, CONTRIBUTING, running/testing, CLI/application help, platform privacy/verification docs, sync protocol/threat docs, and release evidence.
- Update the external Hidlins PRD, feature index, verification index, and Must-Have user-scenario documents to make local-network sync authoritative and remove active S3/auto-save requirements without migration guidance.
- Add copy-pasteable iOS Simulator and Android Emulator scenario procedures matching Task 014's actual Make targets and underlying provisioning/start/install/launch/routing/pair/sync/restart/evidence/cleanup commands.
- Replace the former blocking manual execution table with a precise non-blocking residual list limited to accessibility, exact physical permission presentation, consumer-router behavior, or other claims automation genuinely cannot establish.
- Final dependency/license/provenance/feature/transitive/vendor delta and Snow patch review.
- Final NCSA development/test-only exception, dependency-closure, packaging-hook, negative-control, and produced-artifact review.
- Focused protocol, cryptographic integration, zeroization, discovery/address, pre-auth privacy, resource, concurrency/CAS, storage, log/error, FFI/UI, and mobile-policy review; fix high-confidence findings test-first.

### Out of scope

- Requiring the human to run automatable tests.
- Blocking completion on accessibility or any other manual observation.
- New daemon/service, Internet sync, or speculative enhancements.
- Modifying immutable workflow archives or documenting S3 migration/legacy support.

## Implementation requirements

- Documentation commands must match executable Make targets/scripts and be validated against the passing Task 014 scenario evidence.
- The simulator guide identifies supported host/runtime prerequisites, exact virtual-device selection/boot commands, application build/install/launch, iOS and Android host routing, pair/import, bidirectional edits, restarts, revocation, evidence paths, failure diagnosis, and cleanup.
- Manual residuals state exactly why automation cannot prove the property and are labeled optional/non-blocking. They must never be rendered as passed, required for DONE, or silently skipped required work.
- Every Must-Have requirement maps to automated tests and user-scenario verification; the simulator scenarios supplement lower-level unit/property/fuzz coverage.
- External notebook writes preserve existing requirement/scenario IDs and history while recording Revision 4's automation-first acceptance policy.
- Active docs consistently state startup-then-manual sync, explicit CLI/TUI/desktop serving, mobile client-only behavior, exact local ranges/no override, discovery-as-routing, and pinned Noise trust.
- Complete every dependency-policy field and verify actual application artifacts exclude NCSA/libFuzzer code.

## Acceptance criteria

- [ ] Active repository and notebook documentation describes only the shipped local-network model and contains no active S3 or auto-on-save instruction.
- [ ] Every Must-Have requirement maps to automated coverage and a complete user-scenario verification, including Task 014's real simulator/CLI evidence.
- [ ] The manual verification guide contains validated, copy-pasteable simulator procedures and makes all genuinely human/physical residuals explicitly optional and non-blocking.
- [ ] Documentation does not ask the human to execute any scenario that Task 014 can automate.
- [ ] Dependency, Snow patch, NCSA boundary, packaging/artifact, and focused security reviews satisfy the approved policy with no unresolved high-confidence high/critical finding.
- [ ] Final commands, help, JSON contracts, Make targets, CI descriptions, troubleshooting, and evidence locations are accurate and reproducible.

## Validation

- `make verify`
- `make acceptance-evidence-check`
- `make s3-removal-check`
- `make deny`
- `make audit`
- `make ncsa-boundary-check`

## Dependencies

- Task 014

## Expected areas of change

- `README.md`
- `CONTRIBUTING.md`
- `docs/`
- `crates/hidlins-sync/docs/`
- `app/ios/PRIVACY.md`
- `app/ios/VERIFICATION.md`
- `app/android/VERIFICATION.md`
- CLI/TUI/Flutter help and localization resources
- `/Users/jason/Developer/sources/personal/notebook/projects/hidlins/prd.md`
- `/Users/jason/Developer/sources/personal/notebook/projects/hidlins/index.md`
- `/Users/jason/Developer/sources/personal/notebook/projects/hidlins/features/index.md`
- `/Users/jason/Developer/sources/personal/notebook/projects/hidlins/verifications/`

## Risks / notes

Simulator evidence is authoritative for the automated client scenarios it executes, not for physical OS wording, consumer-router firmware behavior, or human-perceived accessibility. Those limits must be visible without turning them into completion blockers.

NCSA appearing outside the isolated fuzz exception or in any produced application artifact remains a release blocker.
