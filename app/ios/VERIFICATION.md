# iOS verification

The repository's automated simulator, native-hosted, widget, real-bridge,
network, artifact-inspection, and real simulator/CLI scenario targets are the
release authority. Physical-device permission presentation, representative
router behavior, human SAS perception, and VoiceOver speech are optional,
non-blocking confidence observations documented in
[`../../docs/local-network-sync-manual-verification.md`](../../docs/local-network-sync-manual-verification.md).

## Automated targets

- `make app-test-ios-integration` requires at least two installed supported iOS
  runtimes, dynamically selects an iPhone on the newest and an iPad on the next
  newest runtime, records their names/runtime/UDIDs, then runs the shipping-size
  UI, first-run, vault/CRUD/import, and lifecycle/idle-lock journeys through the
  linked Rust framework. Install another runtime from Xcode > Settings >
  Components if the harness reports that only one is available.
- `make app-test-ios-simulator` runs native XCTest coverage for clipboard,
  snapshot shielding, storage/import, lifecycle acknowledgement, platform error
  redaction, local discovery bounds, bridge loading, resources, and the privacy
  manifest.
- `make app-test-integration` runs a two-session real-bridge local-network
  journey covering server start, SAS pairing, paired-vault import, startup and
  manual sync, DHCP candidate replacement, peer rename/revocation, and shutdown.
- `make app-build-ios` builds and inspects simulator-debug and unsigned
  device-release artifacts, including identifiers, OS floor, icons, privacy,
  telemetry-framework absence, and Rust bridge symbols.
- `make test-local-sync-mobile-scenarios-ios` runs the expensive iPhone/iPad
  application scenarios against a separate release-mode CLI authority and
  records redacted per-device evidence under
  `build/verification/mobile-local-sync/`.

## Optional manual observations

Physical iOS local-network permission presentation/recovery, foreground-only
OS scheduling, human SAS comparison, representative-router discovery/DHCP
churn, and VoiceOver speech may be observed on physical hardware, but do not
block acceptance. Programmatic foreground cancellation/resume, pairing,
discovery, restart, revocation, and client-only behavior are covered by the
automated simulator and process scenarios.

The OS shell's final launcher masking and app-switcher compositing remains a
separate optional visual confidence observation; it does not replace the
required sync lifecycle and accessibility cases.

Automated acceptance instead requires semantics, focus order, labels/roles/state,
secret concealment, scaling, touch-target guidelines, keyboard traversal, brand
hash/dimension/opacity/safe-zone checks, compiled resource inspection, controlled
captures, lifecycle tests, and proof that the opaque snapshot cover is installed
before backgrounding.

The simulator procedures below remain useful for optional visual observation.
The automated local-sync scenario procedure, including exact simulator startup
commands, is in the shared guide.

Build, install, and launch the inspected simulator artifact before either
observation:

```sh
make app-prepare-ios-observation
```

The target prints the selected simulator name, runtime, and UDID for the
evidence record.

### Simulator VoiceOver precursor

1. With the installed build open, enable VoiceOver in Settings >
   Accessibility > VoiceOver.
2. On the unlock screen, move forward through vault, password, reveal, and
   unlock controls. Unlock a disposable fixture and move through Entries,
   Search, Generator, Sync, and Settings.
3. Confirm focus order matches visual order, headings are announced as
   headings, controls announce their label/role/state, and an unrevealed
   password or protected custom field never announces either its value or a
   bullet string. Reveal one disposable value explicitly and confirm it is then
   announced.

Expected: the shipping assistive technology matches the automated semantics,
focus, concealment, and touch-target contract without a trap or unreachable
action.

### Optional launcher and app-switcher rendering

1. Return to the simulated Home screen and observe the Hidlins launcher icon at
   normal size. Open Hidlins, unlock a disposable fixture, then enter the app
   switcher.
2. Confirm the launcher uses the approved navy Hidlins mark without clipping or
   transparency artifacts and the app-switcher card is fully opaque with no
   vault content visible.

Expected: approved launcher branding is legible and the snapshot shield exposes
no secret or vault content. Pixel identity, resource presence, safe-zone rules,
and shield installation are already automated; this observation covers only the
OS shell's final rendering.
