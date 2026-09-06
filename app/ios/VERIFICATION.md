# iOS simulator verification

Physical hardware is not required. The acceptance baseline is the repository's
automated simulator, native-hosted, widget, real-bridge, network, and artifact
inspection targets.

## Automated targets

- `make app-test-ios-integration` requires at least two installed supported iOS
  runtimes, dynamically selects an iPhone on the newest and an iPad on the next
  newest runtime, records their names/runtime/UDIDs, then runs the shipping-size
  UI, first-run, vault/CRUD/import, and lifecycle/idle-lock journeys through the
  linked Rust framework. Install another runtime from Xcode > Settings >
  Components if the harness reports that only one is available.
- `make app-test-ios-simulator` runs native XCTest coverage for clipboard,
  snapshot shielding, storage/import, lifecycle acknowledgement, platform error
  redaction, bridge loading, resources, and the privacy manifest.
- `make app-test-ios-simulator-minio` creates an isolated bucket and runs the
  two-session real-bridge suite against managed MinIO. It covers bootstrap and
  rollback, offline edits, network failure/retry, merge, same-field loser history,
  same-second conflict backup/recovery, password rotation, and suspend/resume/
  detach locking.
- `make app-build-ios` builds and inspects simulator-debug and unsigned
  device-release artifacts, including identifiers, OS floor, icons, privacy,
  telemetry-framework absence, and Rust bridge symbols.

## Optional credentialed S3-compatible service

This live-service run is not an acceptance gate. Revision 5 records it as
`SKIPPED — user decision / credentials not supplied`. Managed MinIO plus the
transport/state-machine suites are the executed sync baseline. The target remains
available as a later confidence run without changing that recorded result.

Create a JSON file outside the repository with exactly these keys:

```json
{
  "endpoint": "https://s3.example.invalid",
  "bucket": "hidlins-test-bucket",
  "region": "us-east-1",
  "path_style": false,
  "access_key_id": "REDACTED",
  "secret_access_key": "REDACTED"
}
```

Protect it with `chmod 600`, then run:

```sh
HIDLINS_IOS_S3_CONFIG=/absolute/path/to/config.json make app-test-ios-simulator-s3
```

The environment contains only the non-secret path. The harness validates the
file permissions, creates a protected temporary define file, never prints the
values, uses a unique object key, directs Flutter into an isolated build
directory, and removes both that directory and the installed test app on exit.
The configured account must be restricted to a disposable test bucket.

## User-skipped residual observations

Manual iOS validation is not required for this work package. The following
residuals are recorded as `SKIPPED — user decision` and must not be represented
as passing:

- VoiceOver's exact spoken output and gesture traversal.
- The OS shell's final launcher masking and app-switcher compositing.

Automated acceptance instead requires semantics, focus order, labels/roles/state,
secret concealment, scaling, touch-target guidelines, keyboard traversal, brand
hash/dimension/opacity/safe-zone checks, compiled resource inspection, controlled
captures, lifecycle tests, and proof that the opaque snapshot cover is installed
before backgrounding.

The commands and procedures below remain optional future confidence checks only.
If someone chooses to run them later, record simulator name, iOS runtime, Git
revision, Flutter version, expected result, and a redacted observation; they do
not alter this package's recorded skip without a new acceptance decision.

Build, install, and launch the inspected simulator artifact before either
observation:

```sh
make app-prepare-ios-observation
```

The target prints the selected simulator name, runtime, and UDID for the
evidence record.

### Optional VoiceOver spoken navigation

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
