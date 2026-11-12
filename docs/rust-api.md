# Rust application API

`hidlins-api` is a reusable Rust library over the vault, generation, security
and synchronization crates. It owns session state, operation orchestration,
DTO conversion, secret lifetimes, lifecycle policy and event delivery.
Applications provide presentation and platform mechanisms through Rust ports.

Create a session with `api::session::init_app(AppInitConfig)`. Configure paths
explicitly when isolation is needed. Use `AppSession` methods for vault and
entry operations, generation, secret access, pairing, discovery and sync.
`AppSession` accepts shared references and keeps ownership and synchronization
inside Rust. `report_activity` updates an atomic counter. Call `shutdown` when
the application finishes; dropping the session also shuts down owned workers.

Register `lock_events` or `sync_events` with `Box<dyn EventSink<_>>`.
`MpscEventSink::new(sender)` adapts a standard-library channel. Sink callbacks
must return promptly and must not call back into the session, because delivery
runs under its state lock. Lock subscriptions receive an authoritative initial
snapshot. Replacing a subscription drops its previous sink; session shutdown
closes the lock channel. A client must treat channel closure or an increase in
`dropped_lock_events` as stale lock state, then resubscribe for a fresh snapshot.
Sinks that can fail override `try_send`; existing infallible sinks implement
only `send`. The counter includes refused sync events as well as lock events.

The default `desktop` feature supplies desktop clipboard and discovery
mechanisms. `logind` and `iokit` select native lock sources on their respective
platforms. Without desktop features, client-only lifecycle and single-use
native clipboard tickets remain available. Rust JNI/C entry points consume
those tickets synchronously and keep protected values inside bounded native
callbacks. Discovery candidates still pass through Rust address admission and
pinned authentication. A native platform adapter cannot grant trust.

`KeyfileRef` Debug output is redacted. Consuming operations immediately move
keyfile bytes into the core zeroizing owner. Callers retaining an unconsumed
byte DTO must scrub it explicitly with `Zeroize`. Reveal results are secrets;
callers are responsible for their presentation and must never log them.

`make test-api` verifies DTO fidelity, secret handling, lifecycle races and
operation behavior. `make test-api-native` verifies the feature-reduced native
contracts. `make test-api-lock-features` type-checks the complete API on desktop
and native target graphs. `tests/public_api_contract.rs` compiles every operation
in the reviewed public surface. `make doc` generates the full API reference.
