# Work Request — Flutter App Baseline and Build Hardening

Use the existing Flutter application planning index at:

`/Users/jason/Developer/sources/personal/notebook/projects/hidlins/features/flutter-app/plans/index.md`

Review the plan and architecture documents referenced from that folder. They describe the intended work for this branch, but must be evaluated for correctness and viability rather than trusted as-is.

Create a new implementation plan, based on the original goals and grounded in:

- the current repository implementation and architecture;
- the authoritative application PRD at `/Users/jason/Developer/sources/personal/notebook/projects/hidlins/prd.md`;
- the latest significant stable Flutter release, which should become the implementation baseline;
- current official Flutter/Dart platform guidance and compatibility constraints.

The work package must also include:

1. A new `make` target that builds release binaries.
2. Changes to the `make` build process, and every other repository target that performs a build, so compiler warnings are treated as errors. Code emitting compiler warnings must not be considered done.

The resulting workflow plan must replace or correct assumptions from the original Flutter planning documents where repository or current-platform evidence shows they are no longer viable. Implementation must remain approval-gated under the repository feature workflow.

## Baseline clarification

The current branch is based on `main` immediately after an in-depth review and fix-up merge. Treat the current `main` implementation as being in reasonably good shape to build on. The new work package should not reopen or replay already-completed implementation merely because the July planning documents describe it as future work; it should identify and implement only the deltas needed for the new Flutter baseline, requested build hardening, and any concrete correctness or viability gap that remains relevant to the requested outcome.

## Pre-approval revision — Revision 2

Re-examine the July Flutter task documents individually. Although the current
`main` implementation is the trusted baseline, those tasks were precursors to
landing a Flutter application and may contain valuable fixes, requirements, or
ideas that are not yet represented in current source. Fold every still-relevant,
evidence-backed item into this work package; do not discard an idea merely because
its historical task is marked complete, and do not replay behavior already proven
by current source and tests.

The user supplied `.ai/workflow/hidlins-logo-package.zip`, containing the approved
Hidlins logo in multiple formats, including launcher-icon candidates. Inventory
the package and include integrating the correct assets into desktop and mobile
application branding, launcher icons, and relevant in-app presentation in the
plan and tests.

Before implementation begins, the first task must walk the user through updating
the locally installed Flutter toolchain to the exact approved baseline. The user
will perform or explicitly authorize machine-level toolchain changes. Verify the
installed result before any repository implementation task starts.

Remain test-forward and explicitly review the revised testing plan using
`eng-test-planning`. Review the planned architecture using `arch-reviewer`,
adapting its architecture dimensions to compare the proposed plan against the
existing code because no implementation diff exists yet. Incorporate concrete,
high-confidence findings into the plan and tasks before requesting approval.

## Pre-approval revision — Revision 3

Make automated verification the default throughout this work package. Every
deterministic acceptance behavior should be exercised by repository-owned test
automation wherever reliable automation is technically viable. Physical hardware,
external credentials, or cross-host execution do not by themselves justify a
manual procedure; automate those cases when the platform permits it and expose
the workflow through `make`.

Reserve manual user verification for the smallest set of behaviors where
automation is unreliable, would impose unreasonable implementation cost, or
would require dependencies that violate Hidlins' dependency and supply-chain
rules. Every retained manual case must state its rationale, automated precursor
coverage, exact reproducible procedure, and evidence requirements.

## Approved-plan revision — Revision 4

Remove physical-device testing and physical-device evidence as requirements for
both iOS and Android. The required hardware is not available and must not block
this work package. Supersede the physical-hardware requirements introduced by
Revision 3 while preserving its automation-first intent: exercise every behavior
that can be tested reliably through iOS simulators, Android emulators, native
hosted tests, static boundary checks, and built-artifact inspection. Simulator and
emulator coverage should be as complete as the platform toolchains permit.

## Approved-plan revision — Revision 5

The user will not perform manual validation at this time. Mark every remaining
manual acceptance gate as `SKIPPED — user decision` and do not block the work
package on VoiceOver/TalkBack spoken-navigation observations or mobile
launcher/app-switcher visual observations.

Maximize reliable automated coverage instead: simulator/emulator integration,
native-hosted tests, semantics and focus assertions, accessibility guidelines,
artifact/resource inspection, lifecycle/snapshot-state assertions, controlled
screenshots, managed MinIO, and network-failure/recovery automation must do their
best to verify the corresponding functionality. The previously requested live
credentialed-S3 executions are also non-gating when the user does not provide
external credentials; retain their automated Make targets and secure harnesses,
but accept managed MinIO plus transport/state-machine coverage as the executed
automation baseline. Record the omitted live-service runs explicitly as
`SKIPPED — user decision / credentials not supplied`, rather than claiming they
passed or silently deleting them from the evidence model.

## Approved-plan revision — Revision 6

Permit the Flutter 3.47.2-required Android DSL compatibility configuration and
replan the built-in Kotlin compiler-version resolution without applying the
Kotlin Android plugin.

This supersedes Revision 5's incomplete Android migration assumption that
`android.newDsl=false` and every `org.jetbrains.kotlin.android` declaration must
be removed. Flutter 3.47.2's supported migration enables built-in Kotlin while
retaining the old AGP DSL compatibility flag, and its settings-level Kotlin
plugin declaration may remain with `apply false` solely to pin the compiler
toolchain. The Kotlin Android plugin must not be applied to the application or
any Hidlins Android module.

## Acceptance follow-up — Round 2

In `docs/running-and-testing.md`, the guide states:

> The CLI has no command that registers an arbitrary existing vault, so the
> CLI section creates a separate disposable vault through its supported
> `vault create` flow.

This is nonsensical. Why cannot the CLI open an existing vault? That seems
broken. Unless there is a reason why the CLI cannot open, register, or both,
fix the limitation.

## Acceptance follow-up — Round 3

Before this work package is wrapped up, fix these acceptance bugs:

1. `hidlins -h` currently renders an ASCII-art application name that reads
   `falach`. It must identify the application as `hidlins`.
2. The TUI startup vault modal is very difficult to understand. Its text is
   presented as one undifferentiated block, it lacks a clear visual marker for
   the field accepting input, and the phrase `Focused field` is not a useful or
   conventional affordance. Redesign the modal so the active entry field is
   visually obvious.
3. When the TUI opens with a vault already selected, there is no apparent way
   to choose a different vault. Provide a discoverable way to choose or add a
   different vault.
4. Replace the decorative ASCII art on the TUI login/startup modal with this
   exact logo:

   ```text
            ▄▄▄▄▄▄▄
          ▄██▀▀▀▀▀██▄
          ██       ██
          ██       ██
     ╔════██═══════██════╗
     ║  ┌─────────────┐  ║
     ║  │o  HIDLINS  o│  ║
     ║  └─────────────┘  ║
     ║       ▄▄▄▄▄       ║
     ║         █         ║
     ║       ▄▄█▄▄       ║
     ╚═══════════════════╝
   ```
5. In the desktop Flutter application, an entry with attachments currently
   offers no way to download or view them. Add a usable attachment-access
   action.
