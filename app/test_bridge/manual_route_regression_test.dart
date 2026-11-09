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

class _EmptyDiscovery implements LocalDiscoveryCapability {
  @override
  Future<PlatformResult<LocalDiscoveryStatus>> discover(
    DiscoveryKind kind,
  ) async => const PlatformSuccess(
    LocalDiscoveryStatus(permission: LocalDiscoveryPermission.granted),
  );
  @override
  Future<PlatformResult<PlatformUnit>> stop() async =>
      const PlatformSuccess(platformUnit);
  @override
  Future<PlatformResult<LocalDiscoveryStatus>> permissionStatus() async =>
      const PlatformSuccess(
        LocalDiscoveryStatus(permission: LocalDiscoveryPermission.granted),
      );
  @override
  Future<PlatformResult<PlatformUnit>> openSettings() async =>
      const PlatformSuccess(platformUnit);
}

// Discovery, manual routing, status, config and cleanup are the actual bridge.
// Observe the final operation boundary without dialing a network peer.
class _ObservedSync extends FakeSyncRepository {
  _ObservedSync(this.native)
    : repository = BridgeSyncRepository(
        native,
        useNativeDiscovery: true,
        nativeDiscovery: _EmptyDiscovery(),
      );
  final api.AppSession native;
  final BridgeSyncRepository repository;
  final used = <String>[];
  @override
  Future<SyncStatusDto> syncStatus() => repository.syncStatus();
  @override
  Future<List<SyncPeer>> listPeers() => repository.listPeers();
  @override
  Future<void> configureLocalSync(LocalSyncRole role) =>
      repository.configureLocalSync(role);
  @override
  Future<LocalDiscoveryStatus> discover(DiscoveryKind kind) =>
      repository.discover(kind);
  @override
  Future<void> setManualEndpoint(LocalEndpoint endpoint) =>
      repository.setManualEndpoint(endpoint);
  @override
  Future<void> cancelSync() => repository.cancelSync();
  @override
  Future<void> stopDiscovery() => repository.stopDiscovery();
  Future<void> capture(bridge.DiscoveryServiceDto service) async {
    final snapshot = await native.localDiscoveryStatusFor(service: service);
    used.addAll(
      snapshot.candidates.map((route) => '${route.address}:${route.port}'),
    );
  }

  @override
  Future<PairingPrompt> beginPairing() async {
    await capture(bridge.DiscoveryServiceDto.pairing);
    return pairingPrompt;
  }

  @override
  Future<void> syncNow() => capture(bridge.DiscoveryServiceDto.trusted);
}

void main() {
  setUpAll(() async {
    final path = Platform.environment['HIDLINS_API_LIB'];
    expect(path, isNotNull, reason: 'run make app-test-manual-route');
    expect(File(path!).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(path));
  });
  for (final pair in [true, false]) {
    test(
      'real bridge restores admitted manual route before ${pair ? 'pair' : 'sync'} after empty discovery',
      () async {
        final directory = Directory.systemTemp.createTempSync(
          'hidlins-manual-',
        );
        final native = await api.initApp(
          cfg: bridge.AppInitConfig(stateDir: directory.path),
        );
        final sync = _ObservedSync(native);
        final session = FakeSessionRepository()
          ..currentLockState = LockEvent.unlocked;
        final container = ProviderContainer(
          overrides: [
            sessionRepositoryProvider.overrideWithValue(session),
            syncRepositoryProvider.overrideWithValue(sync),
          ],
        );
        try {
          await native.createVault(
            name: 'fixture',
            masterPassword: 'manual-route-fixture-marker',
            confirmedNoRecovery: true,
          );
          await native.unlock(
            name: 'fixture',
            masterPassword: 'manual-route-fixture-marker',
          );
          await native.configureLocalSync(role: bridge.LocalSyncRoleDto.client);
          await container.read(lockStateProvider.future);
          await container.read(syncControllerProvider.future);
          final controller = container.read(syncControllerProvider.notifier);
          await controller.useManualEndpoint('8.8.8.8:42873');
          expect(
            container.read(syncControllerProvider).requireValue.failure,
            isNotNull,
            reason: 'Rust public-address admission is still authoritative',
          );
          await controller.useManualEndpoint('192.168.1.21:42873');
          expect(
            container.read(syncControllerProvider).requireValue.failure,
            isNull,
          );
          await controller.discover(DiscoveryKind.pairing);
          expect(
            (await native.localDiscoveryStatusFor(
              service: bridge.DiscoveryServiceDto.pairing,
            )).candidates,
            isEmpty,
          );
          if (pair) {
            await controller.pairThisVault();
          } else {
            await controller.syncNow();
          }
          expect(sync.used, ['192.168.1.21:42873']);
          expect(
            container.read(syncControllerProvider).requireValue.failure,
            isNull,
          );
        } finally {
          await container
              .read(syncControllerProvider.notifier)
              .cancelForegroundOperations();
          container.dispose();
          sync.dispose();
          session.dispose();
          await native.shutdown();
          directory.deleteSync(recursive: true);
        }
      },
    );
  }
}
