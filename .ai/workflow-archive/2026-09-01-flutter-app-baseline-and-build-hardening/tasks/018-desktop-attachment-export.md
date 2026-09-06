# Task 018: Add desktop attachment export

Delegation: main-only

## Goal

Give macOS and Linux Flutter users an accessible `Save as…` action for every
existing entry attachment without moving secret bytes out of the Rust core.

## Context

The desktop bridge and repository already support atomic attachment export to a
destination path. The entry-detail widget never calls that capability, and the
app has no approved desktop save-panel adapter. Pulling in a file-picker package
would violate the project's dependency-minimization posture and repeat a prior
production dependency-boundary failure.

## Scope

### In scope

- A narrow Dart attachment-export destination capability and provider.
- One fail-closed platform channel with an exact `chooseDestination` contract.
- Native `NSSavePanel` and GTK 3 save-dialog implementations for macOS and
  Linux using already-linked system frameworks.
- A desktop-only, localized, accessible `Save as…` attachment action.
- Wiring the chosen path to `EntryRepository.saveAttachmentTo`.
- Adapter, widget, boundary, sanitization, cancellation, success, failure, and
  desktop native-build coverage.

### Out of scope

- Passing attachment bytes through Dart or a method channel.
- Previewing attachments, opening an external viewer, or managing plaintext
  temporary files.
- Mobile attachment export or attachment mutation changes.
- Reworking the Rust attachment API or KDBX storage.
- Adding a pub package or other external dependency.

## Implementation requirements

1. First add a widget regression proving an existing attachment lacks a usable
   export action, run it against the current UI, and record the red result.
2. Add `AttachmentExportCapability.chooseDestination(suggestedName)` using the
   existing `PlatformResult` envelope and dynamic channel dispatcher. The exact
   channel is `app.hidlins/attachment_export` and its only method is
   `chooseDestination`.
3. Register the capability in Riverpod, `channels.json`, the fail-closed
   platform-boundary checker, its planted negative controls, and focused fakes.
   Extend boundary validation to pin the reviewed macOS and Linux native
   registrations as well as the Dart declaration.
4. Treat attachment names as untrusted metadata. Remove directory separators
   and control characters and use a safe fallback before passing a suggested
   filename to native code. Native code must treat it only as a suggestion.
5. Implement the macOS handler with `NSSavePanel` and the Linux handler with a
   GTK 3 save chooser. Return only success-with-path, canceled, or a short safe
   failure code in the existing envelope shape. Do not read attachment data or
   create a destination file in the handler.
6. Show `Save as…` for each attachment only on macOS and Linux. Give the control
   a localized tooltip/semantic label containing the attachment name and a
   comfortably reachable hit target.
7. On selection, call `EntryRepository.saveAttachmentTo(uuid, key, destPath)`.
   A canceled panel is a silent no-op. Announce successful export through a
   localized snackbar or live-region-equivalent and render a localized,
   secret-free error for adapter or repository failure.
8. Do not import `dart:io` in product Dart, pass bytes over the channel, log
   paths or attachment contents, or add a file-picker dependency.
9. Add exhaustive adapter tests for success, cancellation, unsupported,
   malformed replies, native failure, and unsafe error-code sanitization. Add
   widget tests for platform visibility, safe suggested names, correct
   UUID/key/path forwarding, cancel, failure, success, semantics, and absence of
   byte payloads.
10. Compile the macOS implementation locally through the existing app build
    target and keep the Linux runner buildable through the existing Linux CI
    target. No new developer command is introduced, so no new Make target is
    required.
11. Record fail-before/pass-after evidence and the available-host native build
    result. Do not replace missing headless OS-dialog automation with a manual
    gate.

## Acceptance criteria

- [ ] Each attachment in a macOS/Linux entry detail exposes a localized,
      accessible `Save as…` action; Android/iOS behavior is unchanged.
- [ ] Selecting a destination invokes `saveAttachmentTo` with the exact entry
      UUID, attachment key, and chosen path.
- [ ] Cancellation writes nothing; adapter and repository failures produce
      safe localized feedback; success is clearly announced.
- [ ] Suggested filenames cannot inject directories or terminal/control text.
- [ ] The method channel carries only a suggested filename and destination path,
      never attachment bytes or entry secrets.
- [ ] macOS uses `NSSavePanel`, Linux uses existing GTK 3 APIs, and neither
      handler pre-creates the destination.
- [ ] The boundary checker rejects undeclared channels/methods/native wiring and
      accepts the exact new six-capability manifest.
- [ ] Focused Flutter tests, the macOS desktop build, standard gates, and the
      cross-host Linux build configuration pass without a new dependency.

## Validation

- `make boundary-check`
- `make app-test`
- `make app-build-macos`
- `git diff --check`

## Dependencies

Task 017

## Expected areas of change

- `app/lib/src/platform/attachment_export.dart`
- `app/lib/src/platform/channels.json`
- `app/lib/src/providers/repository_providers.dart`
- `app/lib/src/features/entries/entry_detail.dart`
- `app/l10n/app_en.arb`
- `app/lib/src/l10n/`
- `app/test/fakes/fake_platform_capabilities.dart`
- `app/test/platform/platform_services_test.dart`
- `app/test/features/attachment_test.dart`
- `app/macos/Runner/`
- `app/macos/Runner.xcodeproj/project.pbxproj`
- `app/macos/RunnerTests/RunnerTests.swift`
- `app/linux/runner/`
- `tools/dev/platform-boundary-check.py`

## Risks / notes

- Export intentionally creates a plaintext copy at the path the user selects;
  the UI must make that persistence decision explicit and must not imply the
  copy remains protected by the vault.
- Headless tests cannot faithfully click vendor save panels. Adapter/fake tests,
  native contract checks, planted boundary negatives, and per-host compilation
  provide reliable automation without a flaky GUI driver.
- Linux native compilation is performed by the existing Linux CI job; the local
  macOS gate must not claim that host-specific compile ran locally.
