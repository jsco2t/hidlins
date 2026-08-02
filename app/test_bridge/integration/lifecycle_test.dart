import 'dart:async';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart';
import 'package:app/src/bridge/error.dart';

import 'real_bridge_harness.dart';

void main() {
  setUpAll(initializeRealBridge);

  test(
    'real bridge create, unlock, edit, search, and lock lifecycle',
    () async {
      final stateDir = Directory.systemTemp.createTempSync(
        'hidlins-app-lifecycle-',
      );
      api.AppSession? session;
      addTearDown(
        () => _cleanupResources(session: session, directory: stateDir),
      );
      final liveSession = await api.initApp(
        cfg: AppInitConfig(stateDir: stateDir.path),
      );
      session = liveSession;

      await liveSession.createVault(
        name: 'desktop',
        masterPassword: 'correct horse battery staple',
        confirmedNoRecovery: true,
      );
      final tree = await liveSession.unlock(
        name: 'desktop',
        masterPassword: 'correct horse battery staple',
      );
      final uuid = await liveSession.createEntry(
        group: tree.root.uuid,
        draft: const EntryDraftDto(
          kind: EntryKindDto.credential,
          title: 'Lifecycle target',
          username: 'before',
          password: 'integration-secret',
          tags: [],
          customFields: [],
        ),
      );
      await liveSession.updateEntry(
        uuid: uuid,
        edit: const EntryEditDto(username: 'after'),
      );

      final hits = await liveSession.search(
        opts: const SearchOptionsDto(
          query: 'Lifecycle target',
          mode: SearchModeDto.substring,
          scope: SearchScopeDto.all(),
          includeRecycled: false,
        ),
      );
      expect(hits, hasLength(1));
      expect(hits.single.entry.uuid, uuid);
      expect((await liveSession.entryDetail(uuid: uuid)).username, 'after');

      await liveSession.lockNow();
      await expectLater(
        liveSession.vaultTree(),
        throwsA(isA<HidlinsApiError_VaultLocked>()),
      );
    },
  );

  test('real idle timer locks a one-second configured vault', () async {
    const name = 'idle';
    const password = 'test-password';
    final stateDir = preparedFixtureDirectory('HIDLINS_APP_IDLE_STATE_DIR');
    api.AppSession? session;
    StreamSubscription<LockEvent>? subscription;
    addTearDown(
      () => _cleanupResources(
        session: session,
        additionalCleanup: () async => subscription?.cancel(),
      ),
    );
    final liveSession = await api.initApp(
      cfg: AppInitConfig(stateDir: stateDir.path),
    );
    session = liveSession;

    var sawUnlocked = false;
    final locked = Completer<void>();
    subscription = liveSession.lockEvents().listen((event) {
      if (event == LockEvent.unlocked) sawUnlocked = true;
      if (sawUnlocked && event == LockEvent.locked && !locked.isCompleted) {
        locked.complete();
      }
    });
    final unlockStopwatch = Stopwatch()..start();
    await liveSession.unlock(name: name, masterPassword: password);
    unlockStopwatch.stop();
    // Informational D1 sanity number: production-KDF bridge/open/parse time.
    // ignore: avoid_print
    print('HIDLINS_APP_UNLOCK_MS=${unlockStopwatch.elapsedMilliseconds}');
    await locked.future.timeout(
      const Duration(seconds: 5),
      onTimeout: () => fail('vault did not idle-lock within five seconds'),
    );
    await expectLater(
      liveSession.vaultTree(),
      throwsA(isA<HidlinsApiError_VaultLocked>()),
    );
  });

  test(
    '5k-entry search records the desktop p95 sanity number',
    () async {
      const name = 'performance';
      const password = 'test-password';
      final stateDir = preparedFixtureDirectory('HIDLINS_APP_PERF_STATE_DIR');
      api.AppSession? session;
      addTearDown(() => _cleanupResources(session: session));
      final liveSession = await api.initApp(
        cfg: AppInitConfig(stateDir: stateDir.path),
      );
      session = liveSession;
      await liveSession.unlock(name: name, masterPassword: password);
      SearchOptionsDto options(int index) => SearchOptionsDto(
        query: 'Search Entry ${index.toString().padLeft(5, '0')}',
        mode: SearchModeDto.substring,
        scope: const SearchScopeDto.all(),
        includeRecycled: false,
      );

      await liveSession.search(opts: options(4999));
      final samples = <int>[];
      for (var index = 0; index < 20; index++) {
        final stopwatch = Stopwatch()..start();
        final hits = await liveSession.search(opts: options(4980 + index));
        stopwatch.stop();
        expect(hits, hasLength(1));
        samples.add(stopwatch.elapsedMicroseconds);
      }
      samples.sort();
      final p95Micros = samples[(samples.length * 0.95).ceil() - 1];
      // Keep the raw informational number in CI logs. The authoritative NFR-002
      // CI gate remains Rust-side so hosted-runner noise cannot block Flutter.
      // ignore: avoid_print
      print('HIDLINS_APP_SEARCH_P95_MS=${p95Micros / 1000}');
      expect(p95Micros, isPositive);
    },
    timeout: const Timeout(Duration(minutes: 2)),
  );
}

Future<void> _cleanupResources({
  api.AppSession? session,
  Directory? directory,
  Future<void> Function()? additionalCleanup,
}) async {
  Object? firstError;
  StackTrace? firstStackTrace;

  Future<void> attempt(Future<void> Function() action) async {
    try {
      await action();
    } on Object catch (error, stackTrace) {
      firstError ??= error;
      firstStackTrace ??= stackTrace;
    }
  }

  if (session != null) await attempt(session.shutdown);
  if (additionalCleanup != null) await attempt(additionalCleanup);
  if (directory != null) {
    await attempt(() async {
      if (directory.existsSync()) directory.deleteSync(recursive: true);
    });
  }
  if (firstError case final error?) {
    Error.throwWithStackTrace(error, firstStackTrace!);
  }
}
