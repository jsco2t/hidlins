import 'dart:io';

import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/frb_generated.dart';
import 'package:app/src/data/bridge_repositories.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/platform/local_discovery.dart';
import 'package:app/src/platform/platform_result.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';

final class ObservedDiscovery implements LocalDiscoveryCapability {
  ObservedDiscovery({this.order, this.fail = false});
  final List<String>? order;
  final bool fail;
  int discoveries = 0;
  @override
  Future<PlatformResult<LocalDiscoveryStatus>> discover(
    DiscoveryKind kind,
  ) async {
    discoveries += 1;
    order?.add("discover");
    if (fail) throw StateError("fixture discovery unavailable");
    return const PlatformSuccess(
      LocalDiscoveryStatus(permission: LocalDiscoveryPermission.granted),
    );
  }

  @override
  Future<PlatformResult<LocalDiscoveryStatus>> permissionStatus() async =>
      const PlatformSuccess(
        LocalDiscoveryStatus(
          permission: LocalDiscoveryPermission.notDetermined,
        ),
      );
  @override
  Future<PlatformResult<PlatformUnit>> stop() async =>
      const PlatformSuccess(platformUnit);
  @override
  Future<PlatformResult<PlatformUnit>> openSettings() async =>
      const PlatformSuccess(platformUnit);
}

final class PairedSession implements api.AppSession {
  PairedSession(this.order);
  final List<String> order;
  @override
  Future<bridge.SyncStatusDto> syncStatus() async => bridge.SyncStatusDto(
    configured: true,
    inFlight: false,
    paired: true,
    role: bridge.LocalSyncRoleDto.client,
    activePeerCount: BigInt.one,
    serverEnabled: false,
    serverRunning: false,
    pairingOpen: false,
  );
  @override
  Future<void> setDiscoveryCandidates({
    required bridge.DiscoveryPermissionDto permission,
    required List<bridge.LocalEndpointDto> candidates,
  }) async {
    order.add('validate');
  }

  @override
  Future<void> startStartupSync() async {
    order.add('sync');
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

void main() {
  setUpAll(() async {
    final path = Platform.environment['HIDLINS_API_LIB'];
    expect(path, isNotNull, reason: 'requires the existing real Rust bridge');
    expect(File(path!).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(path));
  });

  test(
    'paired client keeps discovery validation before Rust scheduling',
    () async {
      final order = <String>[];
      final discovery = ObservedDiscovery(order: order);
      final repo = BridgeSyncRepository(
        PairedSession(order),
        useNativeDiscovery: true,
        nativeDiscovery: discovery,
      );
      await repo.startStartupSync();
      expect(order, ['discover', 'validate', 'sync']);
      expect(discovery.discoveries, 1);
    },
  );

  test('paired client still delegates to Rust after discovery fails', () async {
    final order = <String>[];
    final discovery = ObservedDiscovery(order: order, fail: true);
    final repo = BridgeSyncRepository(
      PairedSession(order),
      useNativeDiscovery: true,
      nativeDiscovery: discovery,
    );
    await expectLater(repo.startStartupSync(), throwsStateError);
    expect(order, ['discover', 'sync']);
    expect(discovery.discoveries, 1);
  });

  for (final role in <bridge.LocalSyncRoleDto?>[
    null,
    bridge.LocalSyncRoleDto.client,
    bridge.LocalSyncRoleDto.server,
  ]) {
    test(
      'real Rust ineligible ${role?.name ?? 'unconfigured'} vault never asks platform to discover',
      () async {
        final directory = Directory.systemTemp.createTempSync(
          'hidlins-startup-',
        );
        final native = await api.initApp(
          cfg: bridge.AppInitConfig(stateDir: directory.path),
        );
        final discovery = ObservedDiscovery();
        final repository = BridgeSyncRepository(
          native,
          useNativeDiscovery: true,
          nativeDiscovery: discovery,
        );
        try {
          await native.createVault(
            name: 'fixture',
            masterPassword: 'startup-eligibility-fixture',
            confirmedNoRecovery: true,
          );
          await native.unlock(
            name: 'fixture',
            masterPassword: 'startup-eligibility-fixture',
          );
          if (role != null) await native.configureLocalSync(role: role);
          final status = await native.syncStatus();
          expect(status.paired, isFalse);
          expect(status.configured, role != null);
          Object? failure;
          try {
            await repository.startStartupSync();
          } catch (error) {
            failure = error;
          }
          expect(
            discovery.discoveries,
            0,
            reason: 'Rust eligibility must be checked before platform permission/discovery',
          );
          expect(failure, isNull);
          // Repeated unlock callbacks must also stay entirely offline here.
          await repository.startStartupSync();
          expect(discovery.discoveries, 0);
        } finally {
          await native.shutdown();
          directory.deleteSync(recursive: true);
        }
      },
    );
  }
}
