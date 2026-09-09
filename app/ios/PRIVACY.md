# iOS privacy and platform behavior

Hidlins collects no data and includes no telemetry, analytics, crash-reporting,
advertising, attribution, or update-check framework. The shipping app uses the
network only for user-configured local-network discovery, pairing, import, or
foreground synchronization. Internet synchronization and background sync are
not implemented. iOS is client-only and never starts a listener. Discovery
supplies routing candidates but not trust; Noise identity pins authorize peers.
All routes pass the shared fixed allowlist (IPv4 `10/8`, `172.16/12`,
`192.168/16`, `169.254/16`, `127/8`; IPv6 `fc00::/7`, scoped `fe80::/10`,
and `::1`) with no public-address, DNS, or user override.

The committed `Runner/PrivacyInfo.xcprivacy` therefore declares tracking false,
no tracking domains, and no collected-data types. Its required-reason APIs cover
only functionality present in the application or Flutter runtime:

- file timestamps for files inside Hidlins' container (`C617.1`) and files the
  user explicitly selects through a document provider (`3B52.1`);
- system boot time for elapsed-time and timer behavior (`35F9.1`); and
- application-only preferences (`CA92.1`).

Clipboard writes use `UIPasteboard` with `localOnly` and an OS expiration date.
Only a one-shot Rust transfer ticket crosses the ordinary Dart/native channel;
the native layer consumes the secret directly from Rust. The app-switcher view
is covered before inactive/background state is acknowledged.

Vault import uses `UIDocumentPicker`, copies the selected KDBX atomically into
the Hidlins Application Support directory, and does not modify the source.
Keyfiles use security-scoped bookmarks; the bookmark is persisted, but keyfile
bytes are not. Vault contents, passwords, pairing transcripts, and unlocked KDBX data
must never be written as plaintext logs or diagnostics.

`make app-test-ios-simulator` verifies the manifest through the installed app's
main bundle. `make app-build-ios` additionally inspects both built artifacts,
requires this manifest and its exact declarations, and rejects packaged files
whose names identify common telemetry, crash-reporting, or update frameworks.
