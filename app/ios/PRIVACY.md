# iOS privacy and platform behavior

Hidlins collects no data and includes no telemetry, analytics, crash-reporting,
advertising, attribution, or update-check framework. The shipping app makes no
network request unless the user configures an S3-compatible remote and starts or
triggers foreground synchronization. Background sync is not implemented.

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
bytes are not. Vault contents, passwords, S3 secret keys, and unlocked KDBX data
must never be written as plaintext logs or diagnostics.

`make app-test-ios-simulator` verifies the manifest through the installed app's
main bundle. `make app-build-ios` additionally inspects both built artifacts,
requires this manifest and its exact declarations, and rejects packaged files
whose names identify common telemetry, crash-reporting, or update frameworks.
