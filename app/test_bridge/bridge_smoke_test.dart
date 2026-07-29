import 'dart:async';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart';
import 'package:app/src/bridge/error.dart';
import 'package:app/src/bridge/frb_generated.dart';

/// Real-bridge smoke test: loads the actual `hidlins-api` cdylib and drives
/// the generated bindings end to end — native library loading, SSE
/// serialization in both directions, the opaque `AppSession` receiver, typed
/// error decoding, the lock-event stream (registration snapshot + shutdown
/// close), and `#[frb(sync)]` calls.
///
/// Runs headless under `flutter test` (no display, no device): the library
/// is opened directly instead of being resolved from an app bundle. Run via
/// `make app-test-bridge`, which builds the cdylib and points
/// `HIDLINS_API_LIB` at it — the env var is REQUIRED (no fallback path here;
/// the Makefile is the single source of truth for artifact locations, and a
/// guessed path could silently load a stale build). Deliberately kept out of
/// `app/test/` so plain `make app-test` needs no Rust build.
///
/// This is the D1 interim gate for the bridge; the full user-flow
/// integration suite (successful unlock against a fixture vault, sync,
/// interop) is T5.4 and builds on the T1.7 test fixtures.
void main() {
  Directory? stateDir;

  setUpAll(() async {
    stateDir = Directory.systemTemp.createTempSync('hidlins-bridge-smoke-');
    final libPath = Platform.environment['HIDLINS_API_LIB'];
    expect(
      libPath,
      isNotNull,
      reason: 'HIDLINS_API_LIB not set — run via `make app-test-bridge`',
    );
    expect(
      File(libPath!).existsSync(),
      isTrue,
      reason:
          'hidlins-api cdylib not found at $libPath — '
          'run via `make app-test-bridge`',
    );
    await RustLib.init(externalLibrary: ExternalLibrary.open(libPath));
  });

  tearDownAll(() {
    stateDir?.deleteSync(recursive: true);
  });

  AppInitConfig cfg() => AppInitConfig(stateDir: stateDir!.path);

  test('apiVersion crosses the bridge', () async {
    final version = await api.apiVersion();
    expect(version, isNotEmpty);
    expect(version.split('.'), hasLength(3), reason: 'semver-shaped');
  });

  test(
    'initApp returns a live opaque session; shutdown is idempotent',
    () async {
      final session = await api.initApp(cfg: cfg());
      expect(session.droppedLockEvents(), BigInt.zero);
      session.reportActivity();
      session.reportLifecycleState(state: LifecycleStateDto.resumed);
      await session.shutdown();
      await session.shutdown();
    },
  );

  test('typed errors decode across the boundary', () async {
    final session = await api.initApp(cfg: cfg());
    try {
      await expectLater(
        session.unlock(name: 'no-such-vault', masterPassword: 'pw'),
        throwsA(isA<HidlinsApiError_FileNotFound>()),
      );
    } finally {
      await session.shutdown();
    }
  });

  test('KeyfileRef.bytes round-trips the inbound wire path', () async {
    final session = await api.initApp(cfg: cfg());
    try {
      // The vault doesn't exist, but the keyfile bytes still cross the wire
      // and decode Rust-side before the registry lookup fails — this
      // exercises the Option<KeyfileRef>::Bytes encode/decode pair.
      await expectLater(
        session.unlock(
          name: 'no-such-vault',
          masterPassword: 'pw',
          keyfile: KeyfileRef.bytes(Uint8List.fromList([1, 2, 3, 4])),
        ),
        throwsA(isA<HidlinsApiError_FileNotFound>()),
      );
    } finally {
      await session.shutdown();
    }
  });

  test('lock-event stream: authoritative snapshot on subscribe, '
      'closed by shutdown', () async {
    final session = await api.initApp(cfg: cfg());
    final events = <LockEvent>[];
    final first = Completer<LockEvent>();
    final done = Completer<void>();
    final sub = session.lockEvents().listen((e) {
      events.add(e);
      if (!first.isCompleted) first.complete(e);
    }, onDone: done.complete);

    // Registration pushes the current state as the first event — wait on
    // the event itself, never on a clock.
    expect(
      await first.future.timeout(const Duration(seconds: 5)),
      LockEvent.locked,
      reason: 'locked-session subscription snapshots Locked',
    );

    // lockNow on an already-locked session is a no-op — no extra event.
    await session.lockNow();

    // shutdown closes the stream: a definitive end-of-stream, not silence.
    await session.shutdown();
    await done.future.timeout(
      const Duration(seconds: 5),
      onTimeout: () => fail('lock-event stream not closed by shutdown'),
    );
    expect(
      events,
      [LockEvent.locked],
      reason:
          'no spurious events from lockNow/shutdown on a locked '
          'session',
    );
    expect(session.droppedLockEvents(), BigInt.zero);
    await sub.cancel();
  });

  test('calls on a dead session surface a typed internal error', () async {
    final session = await api.initApp(cfg: cfg());
    await session.shutdown();
    await expectLater(
      session.unlock(name: 'any', masterPassword: 'pw'),
      throwsA(isA<HidlinsApiError_Internal>()),
    );
  });

  test(
    'a disposed handle is rejected Dart-side before anything is sent',
    () async {
      final session = await api.initApp(cfg: cfg());
      await session.shutdown();
      session.dispose();
      // The generated client refuses to encode a disposed opaque handle, so
      // secret parameters can never reach the Rust dispatcher for a dead
      // receiver (see the pre-dispatch exposure notes in hidlins-api dto.rs).
      // The matcher pins the REJECTION ORIGIN: a HidlinsApiError here would
      // mean the call crossed the bridge and was rejected Rust-side instead —
      // exactly the regression this test exists to catch.
      expect(
        () => session.unlock(name: 'any', masterPassword: 'pw'),
        throwsA(isNot(isA<HidlinsApiError>())),
      );
    },
  );
}
