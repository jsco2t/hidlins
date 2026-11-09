import 'dart:async';
import 'dart:io';

import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/frb_generated.dart';
import 'package:app/src/data/bridge_repositories.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/sync/sync_controller.dart';
import 'package:app/src/platform/local_discovery.dart';
import 'package:app/src/platform/platform_result.dart';
import 'package:app/src/providers/lock_state_provider.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';

import '../test/fakes/fake_repositories.dart';

class _Discovery implements LocalDiscoveryCapability {
  final entered = Completer<void>();
  final pending = Completer<PlatformResult<LocalDiscoveryStatus>>();
  int stops = 0;
  @override
  Future<PlatformResult<LocalDiscoveryStatus>> discover(DiscoveryKind kind) {
    entered.complete();
    return pending.future;
  }

  @override
  Future<PlatformResult<PlatformUnit>> stop() async {
    stops++;
    return const PlatformSuccess(platformUnit);
  }

  @override
  Future<PlatformResult<LocalDiscoveryStatus>> permissionStatus() async =>
      const PlatformSuccess(
        LocalDiscoveryStatus(permission: LocalDiscoveryPermission.granted),
      );
  @override
  Future<PlatformResult<PlatformUnit>> openSettings() async =>
      const PlatformSuccess(platformUnit);
}

void main() {
  setUpAll(() async {
    final path = Platform.environment['HIDLINS_API_LIB'];
    expect(path, isNotNull, reason: 'run make app-test-sync-cancel');
    expect(File(path!).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(path));
  });
  test('cancelled discovery cannot republish controller state through the real bridge repository', () async {
    final directory = Directory.systemTemp.createTempSync(
      'hidlins-controller-cancel-',
    );
    final native = await api.initApp(
      cfg: bridge.AppInitConfig(stateDir: directory.path),
    );
    final discovery = _Discovery();
    final session = FakeSessionRepository()
      ..currentLockState = LockEvent.unlocked;
    final container = ProviderContainer(
      overrides: [
        sessionRepositoryProvider.overrideWithValue(session),
        syncRepositoryProvider.overrideWithValue(
          BridgeSyncRepository(
            native,
            useNativeDiscovery: true,
            nativeDiscovery: discovery,
          ),
        ),
      ],
    );
    try {
      await native.createVault(
        name: 'fixture',
        masterPassword: 'sync-cancel-fixture-marker',
        confirmedNoRecovery: true,
      );
      await native.unlock(
        name: 'fixture',
        masterPassword: 'sync-cancel-fixture-marker',
      );
      await container.read(lockStateProvider.future);
      await container.read(syncControllerProvider.future);
      final controller = container.read(syncControllerProvider.notifier);
      final operation = controller.discover(DiscoveryKind.trusted);
      await discovery.entered.future;
      await controller.cancelForegroundOperations();
      expect(discovery.stops, 1);
      discovery.pending.complete(
        const PlatformSuccess(
          LocalDiscoveryStatus(
            permission: LocalDiscoveryPermission.granted,
            candidates: [LocalEndpoint(address: '192.168.1.22', port: 42873)],
          ),
        ),
      );
      await operation;
      expect(
        container
            .read(syncControllerProvider)
            .requireValue
            .discovery
            .candidates,
        isEmpty,
      );
      expect(
        container.read(syncControllerProvider).requireValue.working,
        isFalse,
      );
    } finally {
      container.dispose();
      if (!discovery.pending.isCompleted) {
        discovery.pending.complete(const PlatformCanceled());
      }
      session.dispose();
      await native.shutdown();
      directory.deleteSync(recursive: true);
    }
  });
}
