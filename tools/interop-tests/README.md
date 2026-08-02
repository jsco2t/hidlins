# KeePassXC Interop Tests

These scripts verify the vault-core KDBX compatibility promise against a real
`keepassxc-cli` binary. KeePassXC is invoked as an external test tool only; it
is not linked into Hidlins and is not part of the Rust dependency graph.

Supported local version: KeePassXC CLI 2.7.x or newer in the 2.x line. The
current local baseline is 2.7.12.

Run from the repository root:

```sh
make interop
```

The Flutter desktop/API boundary round-trip is a separate discoverable target:

```sh
make interop-app
```

It requires KeePassXC 2.7.12 or newer in the 2.x line. The harness creates a
fast-KDF vault through `hidlins-api`, alternates writes with `keepassxc-cli`,
then compares normalized XML exports with `lib/kpxc-diff.py`. Secrets and the
temporary vault live only in a `mktemp` directory removed on exit.

`make interop` builds the `hidlins-test-driver` helper and runs:

- `us_090_rust_to_kpxc.sh`: Hidlins-created vault opens in KeePassXC.
- `us_091_kpxc_to_rust.sh`: KeePassXC-created vault opens in Hidlins.
- `us_092_round_trip.sh`: alternating Hidlins/KeePassXC edits remain readable.

Each script uses `mktemp -d` and removes its working directory with an EXIT
trap. If a script fails, rerun it directly with `sh -x` to inspect the command
sequence:

```sh
HIDLINS_TEST_DRIVER=target/debug/hidlins-test-driver sh -x tools/interop-tests/us_090_rust_to_kpxc.sh
```
