import 'dart:async';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart';
import 'package:app/src/bridge/frb_generated.dart';

void main() {
  setUpAll(() async {
    final path = Platform.environment['HIDLINS_API_LIB'];
    expect(path, isNotNull, reason: 'run make app-test-lifecycle-async');
    expect(File(path!).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(path));
  });

  test(
    'lifecycle reconciliation returns a Future through the real bridge',
    () async {
      final directory = Directory.systemTemp.createTempSync(
        'hidlins-lifecycle-async-',
      );
      final session = await api.initApp(
        cfg: AppInitConfig(stateDir: directory.path),
      );
      try {
        final Object completion = session.reportLifecycleState(
          state: LifecycleStateDto.paused,
        );
        expect(
          completion,
          isA<Future<LockEvent>>(),
          reason: 'Rust mutex waits must run on an FRB worker, leaving the UI isolate free',
        );
        // Activity remains a deliberately tiny synchronous atomic operation.
        session.reportActivity();
        expect(await (completion as Future<LockEvent>), LockEvent.locked);
      } finally {
        await session.shutdown();
        directory.deleteSync(recursive: true);
      }
    },
  );

  test(
    'awaited lifecycle results retain authoritative locked snapshots',
    () async {
      final directory = Directory.systemTemp.createTempSync(
        'hidlins-lifecycle-snapshot-',
      );
      final session = await api.initApp(
        cfg: AppInitConfig(stateDir: directory.path),
      );
      final events = <LockEvent>[];
      final first = Completer<void>();
      final subscription = session.lockEvents().listen((event) {
        events.add(event);
        if (!first.isCompleted) first.complete();
      });
      try {
        await first.future.timeout(const Duration(seconds: 5));
        expect(
          await session.reportLifecycleState(state: LifecycleStateDto.inactive),
          LockEvent.locked,
        );
        expect(
          await session.reportLifecycleState(state: LifecycleStateDto.resumed),
          LockEvent.locked,
        );
        await session.shutdown();
        expect(
          events,
          [LockEvent.locked],
          reason: 'already-locked lifecycle reports do not invent transitions',
        );
      } finally {
        await subscription.cancel();
        await session.shutdown();
        directory.deleteSync(recursive: true);
      }
    },
  );
}
