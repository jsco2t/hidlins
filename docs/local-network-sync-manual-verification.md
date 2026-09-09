# Local-network sync simulator verification and optional observations

Status: **AUTOMATED ACCEPTANCE COMPLETE.** The repository-owned scenario
harness runs real iPhone, iPad, Android phone, and Android tablet applications
against a separate release-mode Hidlins CLI authority. The optional observations
at the end of this document are confidence checks only. They do not block
development completion or release acceptance.

Run every command from the repository root on macOS. The aggregate gate needs
Xcode with at least two installed iOS Simulator runtimes, Flutter 3.47.2, the
pinned Rust toolchain, and the Android SDK. Android needs the repository's API
29 and API 36 system images; the provisioning target installs them when absent.
Use only synthetic secrets. The harness creates disposable KDBX vaults and
removes its private scratch directory after each scenario.

## One-command automated acceptance

Provision Android once, then run the isolated expensive scenario target:

```sh
make android-emulator-provision
make test-local-sync-mobile-scenarios HIDLINS_ANDROID_STRICT=1
```

The target builds `target/release/hidlins`, dynamically selects an iPhone and
iPad spanning two installed iOS runtimes, and runs the host-native API 29 phone
and API 36 tablet Android matrices. Each Android row has an API/ABI-matched
client, authority, and replacement-authority AVD. No human pairing, password
entry, edit, restart, or cleanup is required.

For platform-specific iteration, use:

```sh
make test-local-sync-mobile-scenarios-ios
make test-local-sync-mobile-scenarios-android HIDLINS_ANDROID_STRICT=1
```

Each device must pass all eleven scenario steps:

1. mobile server start is rejected;
2. rejected pairing creates no trust;
3. accepted bilateral pairing imports a valid encrypted KDBX vault;
4. startup sync runs;
5. an authority edit reaches the client;
6. saving alone does not sync;
7. explicit client sync sends the client edit to the authority;
8. the mobile process restarts with only encrypted KDBX and sealed registry
   state persisted;
9. pinned trust reconnects;
10. authority restart is tolerated; and
11. revocation fails closed.

The iOS scenario uses the simulator's host-loopback route. Android instead runs
the release CLI inside a distinct authority emulator, publishes only its local
listener port through an `androidTest`-only `NsdManager` registrar, and requires
the shipping client to discover the authority's private `wlan0` route through
its normal `NsdManager`/Flutter/Rust path. Sync bytes travel directly between
the client and CLI over emulator Wi-Fi. Android's `10.0.2.2` host alias carries
only authenticated orchestration messages and encrypted restart snapshots; it
never carries or supplies a sync route and never proxies sync bytes. Both paths
enforce private/local endpoint validation, bounded timeouts, CLI shutdown,
secret-redacted logs, and fail-closed cleanup. Evidence is written to:

```text
build/verification/mobile-local-sync/<platform>-<device-name>.json
build/verification/mobile-local-sync/<platform>-<device-name>.log
```

A passing Android JSON record additionally identifies every AVD/serial/API/ABI,
the independently observed private addresses, original/replacement route
inequality, registrar lifecycle, CLI-owned data plane, and explicit false
values for route injection, sync proxying, and client endpoint input. Every
record contains `"result": "PASS"`, eleven passing steps, application and CLI
hashes, duration, and hashes proving encrypted restart-state transport. The
paired `.log` is scanned
for the synthetic master password, vault contents, SAS values, private keys, and
unredacted scratch paths. A missing device, missing artifact, device-test error,
secret leak, incomplete step, cleanup failure, or nonzero child status fails the
Make target.

## Exact iOS Simulator procedure

The platform target performs these selection and boot operations. They are
included for diagnosis and for starting the exact simulators locally:

```sh
IOS_DEVICES="$(mktemp "${TMPDIR:-/tmp}/hidlins-ios-devices.XXXXXX")"
IOS_MATRIX="$(mktemp "${TMPDIR:-/tmp}/hidlins-ios-matrix.XXXXXX")"
xcrun simctl list --json devices available >"$IOS_DEVICES"
python3 tools/ios-native/select_simulators.py "$IOS_DEVICES" >"$IOS_MATRIX"
cat "$IOS_MATRIX"
while IFS=$'\t' read -r udid name runtime; do
  xcrun simctl boot "$udid" >/dev/null 2>&1 || true
  xcrun simctl bootstatus "$udid" -b
done <"$IOS_MATRIX"
```

Each matrix line is `<UDID><tab><device name><tab><runtime>`. The selector
fails unless it can choose an iPhone and iPad across two installed runtimes.
Install another runtime from Xcode > Settings > Components when needed.

Build, install, and launch the first selected simulator manually when diagnosing
the application shell:

```sh
make app-build-ios
IFS=$'\t' read -r IOS_UDID IOS_NAME IOS_RUNTIME <"$IOS_MATRIX"
xcrun simctl install "$IOS_UDID" app/build/ios/iphonesimulator/Runner.app
xcrun simctl launch "$IOS_UDID" app.hidlins.ios
```

The actual acceptance scenario is deliberately invoked through
`make test-local-sync-mobile-scenarios-ios`; that target additionally runs the
shipping-size UI and real-bridge suites before calling the protected harness
for both selected devices. Do not substitute a hand-driven pairing journey for
that target.

For diagnosis after the release CLI and simulator app have been built, the
underlying scenario invocation for the selected device is:

```sh
python3 tools/local-sync-tests/mobile_scenario.py ios \
  --device "$IOS_UDID" --device-name "$IOS_NAME" \
  --runtime "iOS $IOS_RUNTIME" \
  --artifact app/build/ios/iphonesimulator/Runner.app/Runner \
  --cli target/release/hidlins \
  --results-dir build/verification/mobile-local-sync
```

This one command owns the `127.0.0.1` route, rejected and accepted pair flows,
import, startup and manual bidirectional sync, process restart, pinned
reconnect, authority restart, revocation, evidence, authority Ctrl+C, and
scratch cleanup.

Cleanup for a diagnostic session:

```sh
while IFS=$'\t' read -r udid name runtime; do
  xcrun simctl uninstall "$udid" app.hidlins.ios >/dev/null 2>&1 || true
  xcrun simctl shutdown "$udid" >/dev/null 2>&1 || true
done <"$IOS_MATRIX"
rm -f "$IOS_DEVICES" "$IOS_MATRIX"
```

## Exact Android Emulator procedure

Provision and inspect the host-native matrix:

```sh
make android-emulator-provision
python3 tools/android-native/emulator_matrix.py select
```

Apple Silicon selects `hidlins-api29` and `hidlins-api36-tablet`; an x86_64
host selects `hidlins-api29-x86_64` and
`hidlins-api36-x86_64-tablet`. The `authority` and `replacement` selector
commands print the matching sidecar AVD names. The automated runner requires no
pre-connected ADB device and always uses clean snapshots.

The authoritative way to start and drive every AVD is the isolated target:

```sh
make test-local-sync-mobile-scenarios-android HIDLINS_ANDROID_STRICT=1
```

It invokes `tools/android-native/android_mobile_scenario.py`, which owns the
ports, Wi-Fi topology, runtime permissions, application/registrar installation,
scenario, evidence, and teardown. To inspect the exact runner directly:

```sh
HIDLINS_ANDROID_STRICT=1 python3 tools/android-native/android_mobile_scenario.py
```

For a manual Apple-Silicon API 29 topology diagnosis, start the client and
authority on the same emulator Wi-Fi peer link in separate terminals:

```sh
ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}"
WIFI_LINK_PORT=47771
"$ANDROID_SDK_ROOT/emulator/emulator" \
  -avd hidlins-api29 -port 5554 -no-audio -no-snapshot -wipe-data \
  -no-boot-anim -wifi-client-port "$WIFI_LINK_PORT" \
  -network-user-mode-options dhcpstart=10.0.2.16
```

```sh
ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}"
WIFI_LINK_PORT=47771
"$ANDROID_SDK_ROOT/emulator/emulator" \
  -avd hidlins-api29-authority -port 5556 -no-audio -no-snapshot \
  -wipe-data -no-boot-anim -wifi-server-port "$WIFI_LINK_PORT" \
  -network-user-mode-options dhcpstart=10.0.2.17
```

After both API 29 guests finish booting, remove the emulator-only duplicate
cellular path. The legacy peer link otherwise places synthetic cellular and
Wi-Fi interfaces in the same subnet, which is unlike an ordinary device and
can make API 29's system NSD daemon select the wrong interface:

```sh
"$ANDROID_SDK_ROOT/platform-tools/adb" -s emulator-5554 shell svc data disable
"$ANDROID_SDK_ROOT/platform-tools/adb" -s emulator-5556 shell svc data disable
```

This creates a Wi-Fi-only device condition; it does not add a route, forward a
port, provide an endpoint, or choose an address for the registrar/client. The
automated acceptance runner performs and verifies this preparation itself.

For API 36, Android Emulator's shared Wi-Fi supports all three guests. Start
`hidlins-api36-tablet`, `hidlins-api36-tablet-authority`, and
`hidlins-api36-tablet-authority-replacement` with ports 5554, 5556, and 5558
respectively, using the same clean flags but no `-wifi-*-port` or DHCP option.
On x86_64, use the names printed by the three selector commands. These manual
commands are diagnostic only; they do not install the test registrar or execute
acceptance. Build, install, and launch the development application on serial
5554 with:

```sh
make app-build-android HIDLINS_ANDROID_STRICT=1
"$ANDROID_SDK_ROOT/platform-tools/adb" -s emulator-5554 install -r \
  app/build/app/outputs/apk/debug/app-debug.apk
"$ANDROID_SDK_ROOT/platform-tools/adb" -s emulator-5554 shell am start -W \
  -n app.hidlins/.MainActivity
```

The acceptance path is `make test-local-sync-mobile-scenarios-android
HIDLINS_ANDROID_STRICT=1`. It owns the API-matched authority CLI, test-only NSD
registration, shipping-client platform discovery, direct private-Wi-Fi Noise
and sync traffic, address churn, process restart, evidence, and teardown. It
does not call the older single-device `mobile_scenario.py android` route.
Stop manually started diagnostic AVDs with:

```sh
"$ANDROID_SDK_ROOT/platform-tools/adb" -s emulator-5554 emu kill
"$ANDROID_SDK_ROOT/platform-tools/adb" -s emulator-5556 emu kill
"$ANDROID_SDK_ROOT/platform-tools/adb" -s emulator-5558 emu kill
```

## Failure diagnosis

- iOS selection failure: run `xcrun simctl list runtimes` and install the
  missing second iOS runtime in Xcode.
- Android provisioning failure: confirm `sdkmanager`, `avdmanager`, `emulator`,
  and `adb` exist under `ANDROID_SDK_ROOT`, then rerun
  `make android-emulator-provision`.
- Android identity failure: stop every attached emulator/device with `adb
  devices -l` as the inventory; the matrix runner refuses ambiguous routing.
- Authority readiness or route failure: inspect the redacted per-device log.
  iOS accepts only the simulator loopback route. Android requires the
  platform-discovered authority `wlan0` address and actual CLI port; public,
  DNS, manually injected, forwarded, redirected, and host-alias sync routes
  fail closed.
- Flutter device failure: inspect the same log plus
  `build/verification/android/`; device failure markers fail the run even when
  a host driver returns zero.
- Interrupted run: rerun the platform target. Its traps uninstall the app,
  stop devices it booted, terminate the CLI authority, and remove scratch data.

## Optional, non-blocking human or physical observations

These checks can improve confidence but automation cannot establish the human
perception or external hardware/firmware property. An unexecuted or failed
observation does not change automated acceptance and must never be recorded as
a passing automated result.

| ID | Optional observation | Why automation is insufficient |
| --- | --- | --- |
| `MV-LNS-001` | Two people compare the displayed SAS, deliberately reject once, then accept matching values. | Tests prove derivation, mismatch rejection, consent, and persistence, but not human perception or intent. |
| `MV-LNS-002` | A physical desktop running the real Rust publisher is discovered by an Android device through `NsdManager` across representative access points, DHCP churn, guest isolation, VLANs, and multicast filtering. | Automated desktop Rust publisher/browser tests and Android emulator NSD tests cover each implementation boundary, but emulator Wi-Fi cannot establish third-party router firmware interoperability. |
| `MV-LNS-004` | Exact physical iOS local-network permission wording and Settings recovery. | The OS owns presentation and may vary by physical-device release/locale. |
| `MV-LNS-005` | Exact physical Android nearby/local-network permission wording and Settings recovery. | The OS/vendor owns presentation and may vary by device skin/API. |
| `MV-LNS-006` | VoiceOver, TalkBack, Orca, and terminal screen-reader spoken output and action reachability. | Semantics, focus, concealment, touch targets, and keyboard reachability are automated; perceived speech requires a person. |

The former `MV-LNS-003` desktop/TUI/CLI lifecycle journey and mobile lifecycle
portions are retired as manual requirements: process, Ctrl+C, start/stop/lock,
no-listener, foreground/background, restart, save-does-not-sync, and revocation
behavior now have automated process and real simulator/emulator evidence.

If an optional observation is performed, record date, Git revision, artifact
hash, device/OS or router/firmware identity, observer, expected result, actual
result, and a redacted evidence path. Never capture a master password, entry
value, SAS, private sync key, or vault file.
