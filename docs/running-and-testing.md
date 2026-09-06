# Running and testing Hidlins

This guide shows how to start each Hidlins application and gives a compact
manual smoke journey. The repository's automated suites remain the acceptance
baseline; manual testing is useful for exploration, not a substitute for
`make verify` or the platform-specific gates.

Run every command from the repository root. Use only disposable vaults and
synthetic secrets while testing. Hidlins deliberately has no master-password
recovery.

## Prerequisites

Install the repository toolchain once, then confirm that the installed Flutter
SDK matches the pinned version:

```sh
make toolchain
make flutter-version-check
```

Flutter, Rust, and dependency versions are pinned by the repository. Product
builds and tests go through `make`; they run against vendored dependencies and
treat first-party compiler and analyzer warnings as errors.

## Create a temporary kitchen-sink vault

The repository contains a source generator rather than a committed KDBX
binary. It creates a KDBX4 vault containing credentials, secure notes, TOTP,
attachments, custom fields, nested groups, tags, and entry history.

```sh
HIDLINS_DEMO_DIR="$(mktemp -d "${TMPDIR:-/tmp}/hidlins-demo.XXXXXX")"
make demo-vault DEMO_VAULT="$HIDLINS_DEMO_DIR/kitchen-sink.kdbx"
```

The fixture master password is `Password123`. Every value in the fixture is
synthetic. `make demo-vault` requires an absolute path and refuses to overwrite
an existing file.

Keep this shell open so `$HIDLINS_DEMO_DIR` remains available. When finished,
inspect the variable and remove only that temporary directory:

```sh
printf '%s\n' "$HIDLINS_DEMO_DIR"
rm -R -- "$HIDLINS_DEMO_DIR"
```

The generated file can be registered and opened by the CLI and TUI, and imported
by the mobile apps. Registration authenticates the existing vault before adding
it to the selected registry; it does not rewrite the KDBX.

## Release CLI, TUI, and agent binaries

Build the optimized host binaries:

```sh
make release
```

Artifacts are written to:

- `target/release/hidlins` — one-shot CLI
- `target/release/hidlins-tui` — reference terminal UI
- `target/release/hidlins-agent` — reserved agent executable; currently a stub

### CLI

Use an isolated registry so the test cannot change your normal vault list.
Register the generated kitchen-sink vault, then address it by the `demo` ID in
subsequent commands:

```sh
mkdir -p "$HIDLINS_DEMO_DIR/cli"
target/release/hidlins \
  --registry "$HIDLINS_DEMO_DIR/cli/vaults.toml" \
  vault register \
  --id demo \
  --path "$HIDLINS_DEMO_DIR/kitchen-sink.kdbx"
target/release/hidlins \
  --registry "$HIDLINS_DEMO_DIR/cli/vaults.toml" \
  vault list
target/release/hidlins \
  --registry "$HIDLINS_DEMO_DIR/cli/vaults.toml" \
  vault open --id demo
target/release/hidlins \
  --registry "$HIDLINS_DEMO_DIR/cli/vaults.toml" \
  entry list --vault demo
target/release/hidlins \
  --registry "$HIDLINS_DEMO_DIR/cli/vaults.toml" \
  entry search --vault demo --mode fuzzy "github"
```

Enter `Password123` at each no-echo master-password prompt. `vault register`
authenticates before writing the registry and stores a canonical path;
`vault open` is a one-shot authentication probe for that registered ID. Entry
commands unlock the selected vault for their operation. Never put a real master
password in an argument or environment variable.

Explore the remaining commands with:

```sh
target/release/hidlins --help
target/release/hidlins entry --help
target/release/hidlins sync --help
```

### Terminal UI

The TUI does not have a registry override flag, but it resolves state from
`HOME`. Give it a temporary home to keep the session separate from your normal
registry and configuration:

```sh
mkdir -p "$HIDLINS_DEMO_DIR/home"
HOME="$HIDLINS_DEMO_DIR/home" target/release/hidlins-tui
```

On first run, enter the absolute path printed by this command:

```sh
printf '%s\n' "$HIDLINS_DEMO_DIR/kitchen-sink.kdbx"
```

Unlock it with `Password123`. The TUI saves the registration beneath the
temporary home only. A useful smoke journey is:

1. Navigate the group tree and open credential, note, TOTP, attachment, and
   history-bearing entries.
2. Search by title, group, and tag.
3. Create and edit an entry, inspect its history, then delete it.
4. Generate a password, copy a disposable secret, and confirm clipboard
   clearing.
5. Lock and unlock the vault.

Use `?` for the in-app key reference, or print the complete keymap without
starting the terminal UI:

```sh
target/release/hidlins-tui --dump-keys
```

### Agent executable

```sh
target/release/hidlins-agent
```

The agent feature has not landed. This executable currently prints a stub
message and exits; there is no background unlock service to exercise yet.

## Flutter desktop app

The desktop app and terminal applications use the same Rust core, KDBX format,
and registry. If you completed the TUI setup above, start the desktop app with
the same temporary `HOME` to expose the registered kitchen-sink vault without
touching your normal Hidlins state.

### macOS

```sh
make app-build-macos
HOME="$HIDLINS_DEMO_DIR/home" \
  app/build/macos/Build/Products/Release/Hidlins.app/Contents/MacOS/Hidlins
```

### Linux

```sh
make app-build-linux
find app/build/linux -path '*/release/bundle/hidlins' -type f
```

Run the path printed by `find`, preserving the temporary home. For example:

```sh
HOME="$HIDLINS_DEMO_DIR/home" \
  app/build/linux/x64/release/bundle/hidlins
```

The architecture directory may be `x64`, `arm64`, or another value selected by
the installed toolchain; use the path that actually exists.

If you did not register the fixture through the TUI, create a disposable vault
through the app's first-run flow. Desktop does not currently provide the native
existing-vault file picker implemented by the mobile runners.

Exercise unlock failure and success, Entries, Search, Generator, Sync settings,
Settings, CRUD/history, secret reveal/copy, and lock/unlock. For a quick debug
launch on Flutter's selected device instead of running a release artifact, use:

```sh
HOME="$HIDLINS_DEMO_DIR/home" make app-run
```

## iOS simulator

iOS builds require macOS and Xcode. Physical hardware is not required.

Build the simulator and unsigned device artifacts, then install and launch the
inspected simulator build:

```sh
make app-build-ios
make app-prepare-ios-observation
```

The preparation target selects and boots a supported simulator, installs the
app, launches it, and prints the selected device details. Create a disposable
vault in the app. To use the kitchen-sink fixture instead, first make the file
available in the simulator's Files app (for example, drag it onto the running
simulator), then select it through Hidlins' existing-vault flow.

In addition to the common GUI journey, check background/resume locking,
rotation, larger text sizes, and both compact phone and expanded tablet
layouts.

The deterministic simulator gates are:

```sh
make app-test-ios-simulator
make app-test-ios-integration
make app-test-ios-simulator-minio
```

See [`app/ios/VERIFICATION.md`](../app/ios/VERIFICATION.md) for coverage and
requirements.

## Android emulator

Provision the repository's host-native API 29 phone and API 36 tablet AVDs:

```sh
make android-emulator-provision
```

On Apple Silicon, start the phone manually with:

```sh
/Users/jason/Library/Android/sdk/emulator/emulator -avd hidlins-api29
```

Use `hidlins-api36-tablet` for the tablet. On an x86_64 host, the corresponding
names are `hidlins-api29-x86_64` and `hidlins-api36-x86_64-tablet`.

Build, install, and launch the debug APK:

```sh
make app-build-android
/Users/jason/Library/Android/sdk/platform-tools/adb install -r \
  app/build/app/outputs/apk/debug/app-debug.apk
/Users/jason/Library/Android/sdk/platform-tools/adb shell am start \
  -n app.hidlins/.MainActivity
```

To import the generated fixture, copy it into the emulator, then choose it in
Hidlins' existing-vault flow:

```sh
/Users/jason/Library/Android/sdk/platform-tools/adb push \
  "$HIDLINS_DEMO_DIR/kitchen-sink.kdbx" \
  /sdcard/Download/hidlins-demo.kdbx
```

Use `Password123` to unlock it. In addition to the common GUI journey, check
system Back, keyboard/IME resizing, rotation, background/resume locking, cold
process restart, and both phone and tablet layouts.

The deterministic emulator gates are:

```sh
make app-test-android-integration
make app-test-android-emulator
make app-test-android-emulator-minio
```

`app-test-android-integration` uses an already selected emulator. The full
emulator targets manage the declared host-native matrix. See
[`app/android/VERIFICATION.md`](../app/android/VERIFICATION.md) for details.

## Automated verification summary

Use the narrowest target while iterating and the complete gate before calling a
change done:

```sh
make check
make app-check
make app-test-integration
make app-test-integration-performance
make interop-app
make verify
```

- `make check` runs the Rust formatting, lint, build, and default test gates.
- `make app-check` runs Flutter/Dart integrity, analysis, formatting, unit and
  widget tests, bridge smoke tests, branding, and generated-boundary checks.
- `make app-test-integration` exercises desktop lifecycle and CRUD through the
  real native bridge, including CLI/TUI process coexistence.
- `make app-test-integration-performance` checks the 5,000-entry path through
  the compiled bridge.
- `make interop-app` checks application KDBX interoperability.
- `make verify` is the broad repository gate.

Managed-MinIO targets require Docker Desktop on macOS. Live credentialed S3,
physical-device tests, and manual assistive-technology observations are
optional, non-gating confidence checks for the current alpha baseline; do not
record an unexecuted check as passing.
