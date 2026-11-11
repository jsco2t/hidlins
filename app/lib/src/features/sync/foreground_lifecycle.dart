import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

// Session/host events have a separate lifetime. This only orders user-initiated
// native effects and their cleanup across the Sync page and locked import UI.
final syncForegroundLifecycleProvider = Provider(
  (_) => SyncForegroundLifecycle(),
);

typedef ForegroundCleanup = Future<void> Function(bool mayCancelNative);
typedef PrepareForegroundCleanup = ForegroundCleanup Function();

class SyncForegroundLifecycle {
  SyncForegroundLease? _active;
  Future<void> _cleanup = Future<void>.value();
  Future<void> _admission = Future<void>.value();

  SyncForegroundLease acquire(Object owner, PrepareForegroundCleanup prepare) {
    final current = _active;
    if (current != null && identical(current.owner, owner)) return current;
    if (current != null) unawaited(_retire(current));
    return _active = SyncForegroundLease._(this, owner, prepare, _admission);
  }

  Future<void> cancel(Object owner, PrepareForegroundCleanup prepare) {
    final current = _active;
    if (current != null && identical(current.owner, owner)) {
      return _retire(current, prepare: prepare);
    }
    // Explicit Cancel while idle can cancel startup work, but an old page may
    // never issue global cleanup against another active dialog/controller.
    return _queue(prepare(), current == null, const []);
  }

  Future<void> releaseOwner(Object owner) {
    final current = _active;
    return current != null && identical(current.owner, owner)
        ? _retire(current)
        : Future<void>.value();
  }

  Future<void> _retire(
    SyncForegroundLease lease, {
    PrepareForegroundCleanup? prepare,
  }) {
    if (!lease._current) return lease._released;
    lease._current = false;
    if (identical(_active, lease)) _active = null;
    // Preparing cleanup revokes continuations and forgets retained input now,
    // before the first native cleanup await.
    return lease._released = _queue(
      (prepare ?? lease._prepare)(),
      true,
      lease._pending.toList(),
    );
  }

  Future<void> _queue(
    ForegroundCleanup cleanup,
    bool mayCancelNative,
    List<Future<void>> pending,
  ) {
    final acknowledged = _cleanup
        .then((_) => cleanup(mayCancelNative))
        .onError((_, _) {});
    _cleanup = acknowledged;
    // Stop acknowledgement can precede a platform discovery Future's final
    // Rust candidate publication. Cancel returns promptly; replacement effects
    // also wait for that owned Future (and late-handle rejection) to settle.
    _admission = Future.wait<void>([_admission, acknowledged, ...pending])
        .then((_) {});
    return acknowledged;
  }
}

class SyncForegroundLease {
  SyncForegroundLease._(this._lifecycle, this.owner, this._prepare, this.ready);
  final SyncForegroundLifecycle _lifecycle;
  final Object owner;
  final PrepareForegroundCleanup _prepare;
  final Future<void> ready;
  final _pending = <Future<void>>{};
  bool _current = true;
  Future<void> _released = Future<void>.value();

  bool get isCurrent => _current;

  Future<T> track<T>(Future<T> operation) {
    late final Future<void> settled;
    settled = operation.then<void>(
      (_) => _pending.remove(settled),
      onError: (Object _, StackTrace stack) {
        _pending.remove(settled);
      },
    );
    _pending.add(settled);
    return operation;
  }

  Future<void> release() => _lifecycle._retire(this);
}
