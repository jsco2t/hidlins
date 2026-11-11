import 'dart:async';

import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/data/failures.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/sync/sync_controller.dart';
import 'package:app/src/features/sync/sync_page.dart';
import 'package:app/src/providers/lock_state_provider.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import '../fakes/fake_repositories.dart';
import '../helpers/test_helpers.dart';

const _configured = SyncStatusDto(configured: true, inFlight: false);
const _latePrompt = PairingPrompt(
  transactionHandle: 'late-handle',
  sas: 'ABC-DEF',
);

class _Sync extends FakeSyncRepository {
  _Sync() {
    status = _configured;
  }
  Completer<LocalDiscoveryStatus>? discoveryGate;
  Completer<PairingPrompt>? pairingGate;
  Completer<PairingPrompt>? importGate;
  Completer<VaultSummary?>? confirmationGate;
  Completer<void>? rejectionGate;
  Completer<void>? cancellationGate;
  Completer<void>? stopGate;
  AppFailure? cancellationError;
  int discoveries = 0;
  int pairings = 0;
  int imports = 0;
  int syncs = 0;
  int cancellations = 0;
  int stops = 0;
  int closes = 0;
  final rejected = <String>[];
  final effects = <String>[];

  @override
  Future<LocalDiscoveryStatus> discover(DiscoveryKind kind) async {
    discoveries++;
    effects.add('discover');
    return discoveryGate == null ? discovery : discoveryGate!.future;
  }

  @override
  Future<PairingPrompt> beginPairing() async {
    pairings++;
    effects.add('pair');
    return pairingGate == null ? pairingPrompt : pairingGate!.future;
  }

  @override
  Future<PairingPrompt> beginPairImport({
    required String name,
    required String masterPassword,
    KeyfileRef? keyfile,
  }) async {
    imports++;
    return importGate == null ? pairingPrompt : importGate!.future;
  }

  @override
  Future<VaultSummary?> confirmPairing({
    required String transactionHandle,
    required bool accepted,
    required String peerDisplayName,
  }) async {
    if (!accepted) {
      rejected.add(transactionHandle);
      await rejectionGate?.future;
      effects.add('reject');
      return null;
    }
    return confirmationGate?.future;
  }

  @override
  Future<void> cancelSync() async {
    cancellations++;
    await cancellationGate?.future;
    effects.add('cancel');
    if (cancellationError case final failure?) throw failure;
  }

  @override
  Future<void> stopDiscovery() async {
    stops++;
    await stopGate?.future;
    effects.add('stop');
  }

  @override
  Future<void> syncNow() async {
    syncs++;
    effects.add('sync');
  }

  @override
  Future<void> openPairingWindow() async {
    status = const SyncStatusDto(
      configured: true,
      inFlight: false,
      pairingOpen: true,
    );
  }

  @override
  Future<void> closePairingWindow() async {
    closes++;
  }

  void finishPending() {
    if (discoveryGate case final gate? when !gate.isCompleted) {
      gate.complete(discovery);
    }
    if (pairingGate case final gate? when !gate.isCompleted) {
      gate.complete(_latePrompt);
    }
    if (importGate case final gate? when !gate.isCompleted) {
      gate.complete(_latePrompt);
    }
    if (confirmationGate case final gate? when !gate.isCompleted) {
      gate.complete(null);
    }
    if (statusCompleter case final gate? when !gate.isCompleted) {
      gate.complete(status);
    }
    for (final gate in [rejectionGate, cancellationGate, stopGate]) {
      if (gate != null && !gate.isCompleted) {
        gate.complete();
      }
    }
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

Future<_Fixture> _fixture({_Sync? repository, bool load = true}) async {
  final sync = repository ?? _Sync();
  final session = FakeSessionRepository()
    ..currentLockState = LockEvent.unlocked;
  final container = ProviderContainer(
    overrides: [
      syncRepositoryProvider.overrideWithValue(sync),
      sessionRepositoryProvider.overrideWithValue(session),
    ],
  );
  addTearDown(() {
    container.dispose();
    sync.finishPending();
    sync.dispose();
    session.dispose();
  });
  await container.read(lockStateProvider.future);
  if (load) {
    await container.read(syncControllerProvider.future);
  }
  return _Fixture(sync, session, container);
}

Future<void> _flush() async {
  await Future<void>.delayed(Duration.zero);
  await Future<void>.delayed(Duration.zero);
}

void main() {
  for (final import in [false, true]) {
    test(
      'Cancel revokes delayed discovery before ${import ? 'import' : 'pairing'} begins',
      () async {
        final f = await _fixture();
        final gate = f.sync.discoveryGate = Completer<LocalDiscoveryStatus>();
        final operation = import
            ? f.controller.beginPairImport(
                name: 'fixture',
                masterPassword: 'fixture-marker',
              )
            : f.controller.pairThisVault();
        await _flush();
        expect(f.sync.discoveries, 1);
        await f.controller.cancelForegroundOperations();
        gate.complete(f.sync.discovery);
        await operation;
        expect(f.sync.pairings + f.sync.imports, 0);
        expect(f.value.prompt, isNull);
        expect(f.value.discovery.candidates, isEmpty);
        expect(f.value.working, isFalse);
      },
    );

    test(
      'late ${import ? 'import' : 'pairing'} handle is rejected after Cancel',
      () async {
        final f = await _fixture();
        final gate = Completer<PairingPrompt>();
        if (import) {
          f.sync.importGate = gate;
        } else {
          f.sync.pairingGate = gate;
        }
        final operation = import
            ? f.controller.beginPairImport(
                name: 'fixture',
                masterPassword: 'fixture-marker',
              )
            : f.controller.pairThisVault();
        await _flush();
        expect(f.sync.pairings + f.sync.imports, 1);
        await f.controller.cancelForegroundOperations();
        gate.complete(_latePrompt);
        await operation;
        expect(f.sync.rejected, ['late-handle']);
        expect(f.value.prompt, isNull);
        expect(f.value.working, isFalse);
      },
    );
  }

  test('Cancel revokes delayed trusted discovery before sync starts', () async {
    final f = await _fixture();
    final gate = f.sync.discoveryGate = Completer<LocalDiscoveryStatus>();
    final operation = f.controller.syncNow();
    await _flush();
    await f.controller.cancelForegroundOperations();
    gate.complete(f.sync.discovery);
    await operation;
    expect(f.sync.syncs, 0);
    expect(f.value.working, isFalse);
  });

  test('old discovery error cannot publish after Cancel', () async {
    final f = await _fixture();
    final gate = f.sync.discoveryGate = Completer<LocalDiscoveryStatus>();
    final operation = f.controller.discover(DiscoveryKind.trusted);
    await _flush();
    await f.controller.cancelForegroundOperations();
    gate.completeError(const SyncUnreachable());
    await operation;
    expect(f.value.failure, isNull);
    expect(f.value.working, isFalse);
  });

  for (final block in ['reject', 'cancel', 'stop']) {
    test(
      'new foreground work waits for blocked $block cleanup effect',
      () async {
        final f = await _fixture();
        if (block == 'reject') {
          await f.controller.pairThisVault();
        }
        final gate = Completer<void>();
        switch (block) {
          case 'reject':
            f.sync.rejectionGate = gate;
          case 'cancel':
            f.sync.cancellationGate = gate;
          case 'stop':
            f.sync.stopGate = gate;
        }
        final before = f.sync.discoveries;
        final cancelling = f.controller.cancelForegroundOperations();
        await _flush();
        expect(
          block == 'reject'
              ? f.sync.rejected.length
              : block == 'cancel'
              ? f.sync.cancellations
              : f.sync.stops,
          greaterThan(0),
        );
        final newer = f.controller.discover(DiscoveryKind.trusted);
        await _flush();
        expect(f.sync.discoveries, before);
        gate.complete();
        await cancelling;
        await newer;
        expect(f.sync.discoveries, before + 1);
        expect(f.sync.effects.last, 'discover');
        expect(f.value.discovery.candidates, isNotEmpty);
        expect(f.value.working, isFalse);
      },
    );
  }

  test('foreground refresh cannot overwrite state after Cancel', () async {
    final f = await _fixture();
    final gate = f.sync.statusCompleter = Completer<SyncStatusDto>();
    final operation = f.controller.renamePeer('fixture-peer', 'fixture-name');
    await _flush();
    await f.controller.cancelForegroundOperations();
    gate.complete(const SyncStatusDto(configured: false, inFlight: false));
    await operation;
    expect(f.value.configured, isTrue);
    expect(f.value.working, isFalse);
  });

  test(
    'failed cancellation still stops discovery and permits new work',
    () async {
      final f = await _fixture();
      f.sync.cancellationError = const SyncUnreachable();
      await expectLater(f.controller.cancelForegroundOperations(), completes);
      expect(f.sync.stops, 1);
      expect(f.value.working, isFalse);
      f.sync.cancellationError = null;
      await f.controller.discover(DiscoveryKind.trusted);
      expect(f.value.discovery.candidates, isNotEmpty);
      expect(f.value.failure, isNull);
    },
  );

  test(
    'late accepted confirmation cannot return an imported vault after Cancel',
    () async {
      final f = await _fixture();
      await f.controller.pairThisVault();
      final gate = f.sync.confirmationGate = Completer<VaultSummary?>();
      final operation = f.controller.confirmPairing(accepted: true);
      await _flush();
      await f.controller.cancelForegroundOperations();
      gate.complete(
        const VaultSummary(
          name: 'old-import',
          path: '/fixture.kdbx',
          hasKeyfile: false,
          hasSync: true,
        ),
      );
      expect(await operation, isNull);
      expect(f.value.working, isFalse);
    },
  );

  test(
    'Cancel preserves the persistent Started Done and inbound-prompt listener',
    () async {
      final f = await _fixture();
      await f.controller.cancelForegroundOperations();
      f.sync.syncController.add(
        const bridge.SyncEvent.started(automatic: true),
      );
      await _flush();
      expect(f.value.inFlight, isTrue);
      f.sync.syncController.add(
        const bridge.SyncEvent.done(bridge.SyncOutcomeDto.alreadyInSync()),
      );
      await _flush();
      expect(f.value.inFlight, isFalse);
      expect(f.value.lastOutcome, SyncOutcomeDto.alreadyInSync);
      f.sync.syncController.add(
        const bridge.SyncEvent.pairingRequested(
          bridge.PairingPromptDto(
            transactionHandle: 'inbound-handle',
            sas: 'ABC-DEF',
          ),
        ),
      );
      await _flush();
      expect(f.value.prompt?.transactionHandle, 'inbound-handle');
    },
  );

  test(
    'Cancel preserves initial loading and ordered queued host events',
    () async {
      final repo = _Sync();
      final gate = repo.statusCompleter = Completer<SyncStatusDto>();
      final f = await _fixture(repository: repo, load: false);
      final loaded = f.container.read(syncControllerProvider.future);
      await _flush();
      await f.controller.cancelForegroundOperations();
      repo.syncController.add(const bridge.SyncEvent.started(automatic: true));
      repo.syncController.add(
        const bridge.SyncEvent.done(
          bridge.SyncOutcomeDto.pushed(isFirstSeed: false),
        ),
      );
      await _flush();
      gate.complete(_configured);
      await loaded;
      await _flush();
      expect(f.value.configured, isTrue);
      expect(f.value.lastOutcome, SyncOutcomeDto.pushed);
      expect(f.value.inFlight, isFalse);
    },
  );

  testWidgets('unrelated foreground work preserves the pairing countdown', (
    tester,
  ) async {
    final f = await _fixture();
    try {
      await f.controller.openPairingWindow();
      expect(f.value.pairingSecondsRemaining, 180);
      await f.controller.renamePeer('fixture-peer', 'fixture-name');
      await tester.pump(const Duration(seconds: 1));
      expect(f.value.pairingSecondsRemaining, 179);
      await tester.pump(const Duration(seconds: 179));
      expect(f.value.pairingSecondsRemaining, 0);
      expect(f.sync.closes, 1);
    } finally {
      f.container.dispose();
      await tester.pump();
    }
  });

  testWidgets(
    'actual SyncPage disposal cancels late work and leaves controller reusable',
    (tester) async {
      final f = await _fixture();
      try {
        await tester.pumpApp(
          UncontrolledProviderScope(
            container: f.container,
            child: const SyncPage(desktopOverride: true),
          ),
        );
        await tester.pumpAndSettle();
        final gate = f.sync.discoveryGate = Completer<LocalDiscoveryStatus>();
        final operation = f.controller.pairThisVault();
        await tester.pump();
        await tester.pumpWidget(const SizedBox.shrink());
        await tester.pump();
        gate.complete(f.sync.discovery);
        await operation;
        await tester.pump();
        expect(f.sync.pairings, 0);
        expect(f.value.prompt, isNull);
        expect(f.value.working, isFalse);
        expect(tester.takeException(), isNull);
      } finally {
        f.container.dispose();
        await tester.pump();
      }
    },
  );

  test('provider disposal ignores a late foreground completion', () async {
    final f = await _fixture();
    final gate = f.sync.discoveryGate = Completer<LocalDiscoveryStatus>();
    final operation = f.controller.pairThisVault();
    await _flush();
    f.container.dispose();
    gate.complete(f.sync.discovery);
    await operation;
    expect(f.sync.pairings, 0);
  });
}
