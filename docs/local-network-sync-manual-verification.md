# Local-network sync verification

Use two supported Linux/macOS hosts on the same private network. Build each
with `make release`, create disposable test vaults and keep their passwords in
secure prompts. The console's `sync --help` and subcommand help list the exact
options. Do not put master passwords on the command line or in environment
variables.

On the authority, run `hidlins sync serve --vault work --pairing-window`.
On the client, run `hidlins sync pair --vault work --name laptop`. Compare the
six-character code on both terminals and approve only matching codes. Exercise
rejection first with disposable state. Importing a new client uses
`hidlins sync import`; no vault or registration may appear before successful
bilateral confirmation and KDBX validation.

Run `hidlins sync now --vault work` on the client, edit disjoint entries, then
sync again. Open the vault with KeePassXC to verify values and history. For a
same-field conflict, verify that the losing value survives as history under
the same UUID and that `.kdbx.bak` preserves pre-merge state. Repeat after
restarting the authority, then revoke the client through `sync peers` and
verify that its next authenticated operation is rejected.

The TUI Settings surface exposes pairing, manual sync, peers and explicit
server lifetime controls. Configured client vaults attempt startup sync once
per process; later synchronization requires a manual action. Saving, locking,
reconnecting or timer ticks must not trigger another attempt.

Run the deterministic verification through:

```sh
make test-local-sync-security
make test-local-sync-discovery
make test-local-sync-integration
make interop-sync
make fuzz-local-sync-corpus
make license-check
```

The integration gate includes real-loopback processes. Its isolated Linux
network harness needs network-namespace capability; it must report unsupported
host capabilities honestly. Optional physical observations include human SAS
comparison, consumer-router multicast behavior, and terminal screen-reader
speech. These observations do not replace deterministic security checks.
