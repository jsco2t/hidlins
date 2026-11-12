# Running and testing Hidlins

This guide shows how to start each Hidlins application and gives a compact
manual smoke journey. The repository's automated suites remain the acceptance
baseline; manual testing is useful for exploration, not a substitute for
`make verify` or the platform-specific gates.

Run every command from the repository root. Use only disposable vaults and
synthetic secrets while testing. Hidlins deliberately has no master-password
recovery.

## Prerequisites

Install the repository toolchain once:

```sh
make toolchain
```

Rust and dependency versions are pinned by the repository. Product
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

The generated file can be registered and opened by the CLI and TUI. Registration authenticates the existing vault before adding
it to the selected registry; it does not rewrite the KDBX.

## Release console and agent binaries

Build the optimized host binaries:

```sh
make release
```

Artifacts are written to:

- `target/release/hidlins` — reference TUI by default; one-shot CLI with a subcommand or `--cli`
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

The TUI accepts the shared `--registry` override. Preferences and themes
retain their normal XDG configuration directory. `--config` selects a different
preferences file; custom themes still load from the XDG themes directory.
Give this demo a temporary home and XDG directory to isolate both its registry
and preferences:

```sh
mkdir -p "$HIDLINS_DEMO_DIR/home"
HOME="$HIDLINS_DEMO_DIR/home" XDG_CONFIG_HOME="$HIDLINS_DEMO_DIR/home/.config" target/release/hidlins
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
target/release/hidlins --dump-keys
```

### Agent executable

```sh
target/release/hidlins-agent
```

The agent feature has not landed. This executable currently prints a stub
message and exits; there is no background unlock service to exercise yet.

## Local-network sync across two devices

Use two disposable copies of the same vault on the same private/local network.
On each machine, prepare its own temporary `HIDLINS_DEMO_DIR` and register the
copied kitchen-sink KDBX as `demo` in
`$HIDLINS_DEMO_DIR/cli/vaults.toml`, using the CLI registration commands above.
Copy the same generated KDBX to the client; independently generating a second
vault would create a different logical vault. Keep both disposable registries
separate from normal Hidlins state.

On the authoritative macOS or Linux machine, open a bounded pairing window and
keep the server in the foreground:

```sh
target/release/hidlins \
  --registry "$HIDLINS_DEMO_DIR/cli/vaults.toml" \
  sync serve --vault demo --pairing-window
```

On the client, allow discovery to choose the current DHCP route:

```sh
target/release/hidlins \
  --registry "$HIDLINS_DEMO_DIR/cli/vaults.toml" \
  sync pair --vault demo --name authority
```

Compare the six-character Crockford Base32 SAS (`XXX-XXX`) displayed on both
applications. Accept only if both values match; rejecting or closing either prompt must leave no trusted peer.
After successful bilateral acceptance, stop the foreground server with
`Ctrl+C`, then restart it on the authority without opening a pairing window:

```sh
target/release/hidlins \
  --registry "$HIDLINS_DEMO_DIR/cli/vaults.toml" \
  sync serve --vault demo
```

In the client's separate shell, request sync using its disposable registry:

```sh
target/release/hidlins \
  --registry "$HIDLINS_DEMO_DIR/cli/vaults.toml" \
  sync now --vault demo
```

Discovery provides candidate routes only. The saved Noise identity pin is the
authority; a discovered replacement key must fail. If multicast is unavailable,
`--address` and `--port` accept an IP literal diagnostic route. Both discovered
and manual routes are restricted to IPv4 `10/8`, `172.16/12`, `192.168/16`,
`169.254/16`, `127/8`, or IPv6 `fc00::/7`, scoped `fe80::/10`, and `::1`.
There is no public-address or DNS override.

The TUI exposes Start server, Stop server, Pair, Sync now and peer management
while it runs. Serving starts only on explicit request. Each configured client
vault gets one best-effort startup sync per process and then manual sync only;
save, lock, reconnection and timers do not trigger synchronization.

The [verification procedure](local-network-sync-manual-verification.md) covers
disposable two-device checks and optional physical observations. The desktop
Rust mDNS publisher/browser exchange is tested separately from network-router
behavior.

## Automated verification summary

Run `make check` for formatting, lint, offline builds, tests and supported Rust
feature checks. `make test-api` exercises application DTOs and session
operations; `make test-api-native` runs the unit harness without desktop
feature unification, including native ticket publication and revocation.

`make license-check` checks the repository license inventory, strict Cargo
license policies, the exact EFF asset notice and the isolated fuzz boundary.
`make deny` and `make audit` additionally check current advisory data.

`make verify` combines the Rust gates, developer-tool regressions, ignored
noninteractive tests, docs, KeePassXC interoperability, local-sync protocol
and process tests, and committed fuzz corpus replay. `make test-clipboard`
requires a working display or pasteboard; real OS lock/sleep observations use
`make test-os-events` on their native host.
