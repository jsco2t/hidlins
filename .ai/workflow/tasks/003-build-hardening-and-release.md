# Task 003: Enforce fatal warnings and add release builds

Delegation: main-only

## Goal

Make warnings fatal across first-party builds invoked through `make` and add the
canonical `make release` target for optimized Rust workspace binaries.

## Context

Clippy and the Linux runner already reject warnings, but normal Cargo builds,
tests, Apple runner compilation, and Android Kotlin/Java compilation are not
consistently hardened. There is no discoverable workspace release target.

## Scope

### In scope

- Append/export `-D warnings` for all Cargo invocations reached through the
  Makefile while preserving caller `RUSTFLAGS`.
- Ensure Flutter artifact targets require the fatal Dart analyzer gate.
- Enable warnings-as-errors for first-party Linux, Apple, Android Kotlin, and
  Android Java source without applying the policy to vendored/generated code.
- Add a deterministic warning-policy regression check and wire it into
  `make check`.
- Add and document `make release` as an offline, locked, optimized Rust workspace
  binary build.

### Out of scope

- Cross-compiling all Flutter platforms from one host.
- Signing, notarization, installers, app stores, or artifact publication.
- Repairing warnings emitted solely by third-party/generated source by weakening
  or globally mis-scoping the policy.

## Implementation requirements

- Begin with an intentional warning fixture/probe and record that the current
  build incorrectly succeeds; after enforcement the same probe must fail.
- Do not overwrite externally supplied `RUSTFLAGS`; append the fatal-warning flag.
- Audit every Makefile target containing Cargo, Flutter build, CMake/native,
  Gradle, or Xcode compilation and document how it inherits the policy.
- Keep `make release` host-scoped and ensure its help text names the CLI, TUI, and
  agent optimized artifacts.
- Platform settings must target first-party runner/app modules, not Pods,
  vendored crates/packages, or generated FRB code.

## Acceptance criteria

- [ ] `make release` runs `cargo build --workspace --offline --locked --release`
  with the repository fatal-warning policy and is listed by `make help`.
- [ ] Every Makefile Cargo build/check/test/doc/integration path rejects
  first-party Rust warnings, including native bridge builds.
- [ ] Flutter build targets require fatal Dart analysis; Linux, Apple, and Android
  first-party native compilers reject warnings.
- [ ] A deterministic `make build-policy-check` proves warning-only first-party
  code cannot pass, and it is included in `make check`.
- [ ] Contributor/build documentation describes artifact locations and policy
  scope without claiming that one host builds every Flutter platform.

## Validation

- `make build-policy-check`
- `make build`
- `make release`
- `make check`
- `make app-check`
- `make app-build-macos`

## Dependencies

- Task 002

## Expected areas of change

- `Makefile`
- A small build-policy test fixture or script
- Linux CMake, Apple Xcode, and Android Gradle settings
- Contributor/build documentation and CI entry points

## Risks / notes

Using a process-wide flag carelessly can make vendored dependency warnings fatal.
The contract must prove first-party enforcement while keeping dependency builds
viable and all caller-provided compiler flags intact.
