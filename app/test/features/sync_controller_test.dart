import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/data/models.dart';
import 'package:app/src/features/sync/sync_controller.dart';
import 'package:app/src/providers/lock_state_provider.dart';
import 'package:app/src/providers/providers.dart';

import '../fakes/fake_repositories.dart';

const _configured = SyncStatusDto(configured: true, inFlight: false);

class _DelayedSync extends FakeSyncRepository {
  Completer<SyncStatusDto>? statusGate;
  Completer<List<SyncPeer>>? peersGate;
  Completer<LocalDiscoveryStatus>? discoveryGate;

  @override
  Future<SyncStatusDto> syncStatus() async =>
      statusGate == null ? super.syncStatus() : statusGate!.future;

  @override
  Future<List<SyncPeer>> listPeers() async =>
      peersGate == null ? super.listPeers() : peersGate!.future;

  @override
  Future<LocalDiscoveryStatus> discover(DiscoveryKind kind) async =>
      discoveryGate == null ? super.discover(kind) : discoveryGate!.future;
}

Future<void> _flush() async {
  await Future<void>.delayed(Duration.zero);
  await Future<void>.delayed(Duration.zero);
}

void main() {
  late _DelayedSync sync;
  late FakeSessionRepository session;
  late ProviderContainer container;

  setUp(() async {
    sync = _DelayedSync()..status = _configured;
    session = FakeSessionRepository()..currentLockState = LockEvent.unlocked;
    container = ProviderContainer(
      overrides: [
        sessionRepositoryProvider.overrideWithValue(session),
        syncRepositoryProvider.overrideWithValue(sync),
      ],
    );
    await container.read(lockStateProvider.future);
  });
  tearDown(() {
    container.dispose();
    sync.dispose();
    session.dispose();
  });

  test('retains an event while initial status is loading', () async {
    final gate = sync.statusGate = Completer<SyncStatusDto>();
    final loaded = container.read(syncControllerProvider.future);
    await _flush();
    sync.syncController.add(const bridge.SyncEvent.started(automatic: true));
    await _flush();
    gate.complete(_configured);
    expect((await loaded).inFlight, isTrue);
  });

  test('applies loading events in order through peer loading', () async {
    final gate = sync.peersGate = Completer<List<SyncPeer>>();
    final loaded = container.read(syncControllerProvider.future);
    await _flush();
    sync.syncController.add(const bridge.SyncEvent.started(automatic: true));
    sync.syncController.add(
      const bridge.SyncEvent.done(
        bridge.SyncOutcomeDto.pushed(isFirstSeed: false),
      ),
    );
    await _flush();
    gate.complete([]);
    final state = await loaded;
    expect(state.inFlight, isFalse);
    expect(state.lastOutcome, SyncOutcomeDto.pushed);
  });

  test(
    'lock discards an old refresh and unlock reloads the next vault',
    () async {
      await container.read(syncControllerProvider.future);
      final gate = sync.statusGate = Completer<SyncStatusDto>();
      final refresh = container.read(syncControllerProvider.notifier).refresh();
      session.emitLockState(LockEvent.locked);
      await _flush();
      gate.complete(_configured);
      await refresh;
      await _flush();
      expect(
        container.read(syncControllerProvider).valueOrNull?.configured,
        isFalse,
      );
      sync.statusGate = null;
      session.emitLockState(LockEvent.unlocked);
      await _flush();
      expect(
        (await container.read(syncControllerProvider.future)).configured,
        isTrue,
      );
    },
  );

  test('late discovery cannot populate a locked vault', () async {
    await container.read(syncControllerProvider.future);
    final gate = sync.discoveryGate = Completer<LocalDiscoveryStatus>();
    final discover = container
        .read(syncControllerProvider.notifier)
        .discover(DiscoveryKind.trusted);
    session.emitLockState(LockEvent.locked);
    await _flush();
    gate.complete(sync.discovery);
    await discover;
    await _flush();
    final state = container.read(syncControllerProvider).valueOrNull!;
    expect(state.discovery.candidates, isEmpty);
    expect(state.configured, isFalse);
    expect(state.working, isFalse);
  });

  test('a refresh cannot overwrite a newer sync event', () async {
    await container.read(syncControllerProvider.future);
    final gate = sync.statusGate = Completer<SyncStatusDto>();
    final refresh = container.read(syncControllerProvider.notifier).refresh();
    sync.syncController.add(const bridge.SyncEvent.started(automatic: false));
    await _flush();
    gate.complete(_configured);
    await refresh;
    expect(
      container.read(syncControllerProvider).valueOrNull!.inFlight,
      isTrue,
    );
  });
}
