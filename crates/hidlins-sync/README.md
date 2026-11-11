# hidlins-sync

Secure local-network synchronization for encrypted KDBX vaults.

The crate provides:

- strict private/local address policy with no public-address escape;
- Noise XX pairing and pinned IK reconnects;
- per-vault sealed device identities and explicit peer trust;
- metadata-minimal local discovery with bounded candidate selection;
- an authoritative local server with compare-and-swap commits;
- transport-neutral three-way merge with loser-as-history preservation;
- pre-merge backups, atomic writes, bounded retries, and cancellation.

The Rust core owns protocol, cryptographic, policy, and merge behavior. CLI,
TUI, desktop, and mobile surfaces remain thin callers. Mobile applications
attempt startup and manual synchronization only; they do not expose or run the
persistent server. CLI foreground serving exits on Ctrl+C. TUI and desktop
serving lasts only while explicitly enabled and while the application runs.

## Verification

From the repository root:

```sh
make test-local-sync-security
make test-local-sync-discovery
make test-local-sync-integration
make test-local-sync-mobile-scenarios HIDLINS_ANDROID_STRICT=1
make test-merge-properties
make interop-sync
make fuzz-local-sync-corpus
make fuzz-local-sync-ci
make vendor-patches
make ncsa-boundary-check
```

All dependency and vendoring rules are enforced by the repository-wide
`make check`, `make deny`, and `make audit` gates.

The exact wire contract, security model, executable coverage, dependency
review, automated simulator procedure, and optional observation residue are
documented in:

- [`docs/protocol-v1.md`](docs/protocol-v1.md)
- [`docs/threat-model.md`](docs/threat-model.md)
- [`docs/security-coverage.md`](docs/security-coverage.md)
- [`docs/dependency-review.md`](docs/dependency-review.md)
- [`../../docs/local-network-sync-manual-verification.md`](../../docs/local-network-sync-manual-verification.md)
