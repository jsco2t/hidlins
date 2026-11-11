import 'dart:async';

import 'package:app/src/data/failures.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/sync/sync_controller.dart';
import 'package:app/src/providers/lock_state_provider.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import '../fakes/fake_repositories.dart';

const _manual = LocalEndpoint(address: '192.168.1.21', port: 42873);
const _other = LocalEndpoint(address: '192.168.1.22', port: 42873);

class _Sync extends FakeSyncRepository {
  _Sync() {
    status = const SyncStatusDto(configured: true, inFlight: false);
  }
  LocalEndpoint? nativeRoute;
  LocalDiscoveryStatus discovered = const LocalDiscoveryStatus(
    permission: LocalDiscoveryPermission.granted,
  );
  int discoveries = 0;
  int pairings = 0;
  int imports = 0;
  int syncs = 0;
  final used = <LocalEndpoint?>[];
  Completer<void>? manualGate;

  @override
  Future<void> setManualEndpoint(LocalEndpoint endpoint) async {
    if (endpoint.address == '8.8.8.8') {
      throw const InvalidInputFailure(field: 'endpoint', reason: 'not private');
    }
    await manualGate?.future;
    nativeRoute = endpoint;
  }

  @override
  Future<LocalDiscoveryStatus> discover(DiscoveryKind kind) async {
    discoveries++;
    nativeRoute = discovered.candidates.firstOrNull;
    return discovered;
  }

  @override
  Future<PairingPrompt> beginPairing() async {
    pairings++;
    used.add(nativeRoute);
    return pairingPrompt;
  }

  @override
  Future<PairingPrompt> beginPairImport({
    required String name,
    required String masterPassword,
    KeyfileRef? keyfile,
  }) async {
    imports++;
    used.add(nativeRoute);
    return pairingPrompt;
  }

  @override
  Future<void> syncNow() async {
    syncs++;
    used.add(nativeRoute);
  }
}

class _Fixture {
  _Fixture(this.sync, this.session, this.container);
  final _Sync sync;
  final FakeSessionRepository session;
  final ProviderContainer container;
  SyncController get controller =>
      container.read(syncControllerProvider.notifier);
  SyncViewState get value =>
      container.read(syncControllerProvider).requireValue;
}

Future<_Fixture> _fixture() async {
  final sync = _Sync();
  final session = FakeSessionRepository()
    ..currentLockState = LockEvent.unlocked;
  final container = ProviderContainer(
    overrides: [
      sessionRepositoryProvider.overrideWithValue(session),
      syncRepositoryProvider.overrideWithValue(sync),
    ],
  );
  addTearDown(() {
    container.dispose();
    final gate = sync.manualGate;
    if (gate != null && !gate.isCompleted) gate.complete();
    sync.dispose();
    session.dispose();
  });
  await container.read(lockStateProvider.future);
  await container.read(syncControllerProvider.future);
  return _Fixture(sync, session, container);
}

Future<void> _flush() async {
  await Future<void>.delayed(Duration.zero);
  await Future<void>.delayed(Duration.zero);
}

Future<void> _operate(_Fixture f, String operation) => switch (operation) {
  'pair' => f.controller.pairThisVault(),
  'import' => f.controller.beginPairImport(
    name: 'fixture',
    masterPassword: 'fixture-marker',
  ),
  _ => f.controller.syncNow(),
};

void main() {
  for (final operation in ['pair', 'import', 'sync']) {
    for (final permission in [
      LocalDiscoveryPermission.granted,
      LocalDiscoveryPermission.denied,
      LocalDiscoveryPermission.unavailable,
    ]) {
      test(
        'manual route survives empty $permission discovery for $operation',
        () async {
          final f = await _fixture();
          f.sync.discovered = LocalDiscoveryStatus(permission: permission);
          await f.controller.useManualEndpoint('192.168.1.21:42873');
          await _operate(f, operation);
          expect(f.sync.used.map((route) => route?.toString()), [
            _manual.toString(),
          ]);
          expect(f.sync.discoveries, 0);
          expect(f.value.failure, isNull);
        },
      );
    }
  }

  test(
    'explicit discovery cannot silently replace a selected manual route',
    () async {
      final f = await _fixture();
      await f.controller.useManualEndpoint('192.168.1.21:42873');
      f.sync.discovered = const LocalDiscoveryStatus(
        permission: LocalDiscoveryPermission.granted,
        candidates: [_other],
      );
      await f.controller.discover(DiscoveryKind.pairing);
      expect(f.sync.nativeRoute, _other);
      await f.controller.pairThisVault();
      expect(f.sync.used.map((route) => route?.toString()), [
        _manual.toString(),
      ]);
      expect(f.value.discovery.candidates.map((route) => route.toString()), [
        _manual.toString(),
      ]);
    },
  );

  test(
    'rejected replacement leaves the previously admitted manual route selected',
    () async {
      final f = await _fixture();
      await f.controller.useManualEndpoint('192.168.1.21:42873');
      await f.controller.useManualEndpoint('8.8.8.8:42873');
      expect(f.value.failure, isA<InvalidInputFailure>());
      await f.controller.pairThisVault();
      expect(f.sync.used.map((route) => route?.toString()), [
        _manual.toString(),
      ]);
      expect(f.value.failure, isNull);
    },
  );

  test(
    'canceled in-flight replacement cannot become the selected manual route',
    () async {
      final f = await _fixture();
      await f.controller.useManualEndpoint('192.168.1.21:42873');
      final gate = f.sync.manualGate = Completer<void>();
      final replacement = f.controller.useManualEndpoint('192.168.1.22:42873');
      await _flush();
      await f.controller.cancelForegroundOperations();
      gate.complete();
      await replacement;
      f.sync.manualGate = null;
      await f.controller.pairThisVault();
      expect(f.sync.used.map((route) => route?.toString()), [
        _manual.toString(),
      ]);
    },
  );

  test('lock and next unlock discard prior vault manual selection', () async {
    final f = await _fixture();
    await f.controller.useManualEndpoint('192.168.1.21:42873');
    f.session.emitLockState(LockEvent.locked);
    await _flush();
    f.session.emitLockState(LockEvent.unlocked);
    await _flush();
    await f.container.read(syncControllerProvider.future);
    f.sync.discovered = const LocalDiscoveryStatus(
      permission: LocalDiscoveryPermission.granted,
      candidates: [_other],
    );
    await f.controller.pairThisVault();
    expect(f.sync.used.map((route) => route?.toString()), [_other.toString()]);
    expect(f.sync.discoveries, 1);
  });

  test(
    'without manual selection pairing still uses discovered private routes',
    () async {
      final f = await _fixture();
      f.sync.discovered = const LocalDiscoveryStatus(
        permission: LocalDiscoveryPermission.granted,
        candidates: [_other],
      );
      await f.controller.pairThisVault();
      expect(f.sync.used.map((route) => route?.toString()), [
        _other.toString(),
      ]);
      expect(f.sync.discoveries, 1);
    },
  );
}
