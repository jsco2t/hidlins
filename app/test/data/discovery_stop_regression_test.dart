import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/error.dart';
import 'package:app/src/data/bridge_repositories.dart';
import 'package:app/src/data/failures.dart';
import 'package:app/src/data/models.dart' as model;
import 'package:app/src/platform/local_discovery.dart';
import 'package:app/src/platform/platform_result.dart';

const _route = bridge.LocalEndpointDto(
  address: '192.168.1.30',
  port: 48103,
  scopeId: 0,
);

void main() {
  test('desktop stop closes its Rust browser owner', () async {
    final session = _Session();
    final repo = BridgeSyncRepository(session);
    await repo.stopDiscovery();
    expect(session.browsing, isFalse);
    expect(session.shutdowns, 1);
    await repo.stopDiscovery();
    expect(session.shutdowns, 1);
  });

  test('desktop stop cancels the pending discovery result', () async {
    final session = _Session();
    final repo = BridgeSyncRepository(session);
    final pending = repo.discover(model.DiscoveryKind.pairing);
    final outcome = pending.then<Object>(
      (value) => value,
      onError: (Object error, StackTrace stack) => error,
    );
    await session.started.future;
    await repo.stopDiscovery();
    session.completeForCleanup();
    final result = await outcome;
    expect(result, isA<SyncUnreachable>());
    expect(session.shutdowns, 1);
  });

  test('stop preserves explicit manual routes', () async {
    final session = _Session();
    final repo = BridgeSyncRepository(session);
    await repo.setManualEndpoint(
      const model.LocalEndpoint(address: '192.168.1.30', port: 48103),
    );
    await repo.stopDiscovery();
    expect(session.injected, [_route]);
  });

  test('native stop remains delegated to the native adapter', () async {
    final session = _Session();
    final native = _Native();
    final repo = BridgeSyncRepository(
      session,
      useNativeDiscovery: true,
      nativeDiscovery: native,
    );
    await repo.stopDiscovery();
    expect(native.stops, 1);
    expect(session.shutdowns, 0);
  });
}

final class _Session implements api.AppSession {
  bool browsing = true;
  int shutdowns = 0;
  List<bridge.LocalEndpointDto> injected = [];
  final started = Completer<void>();
  final pending = Completer<bridge.DiscoveryStatusDto>();

  @override
  Future<bridge.DiscoveryStatusDto> pollLocalDiscoveryFor({
    required bridge.DiscoveryServiceDto service,
  }) {
    if (!started.isCompleted) started.complete();
    return pending.future;
  }

  @override
  Future<void> setDiscoveryCandidates({
    required bridge.DiscoveryPermissionDto permission,
    required List<bridge.LocalEndpointDto> candidates,
  }) async {
    injected = List.of(candidates);
  }

  void completeForCleanup() {
    if (!pending.isCompleted) {
      pending.complete(
        const bridge.DiscoveryStatusDto(
          permission: bridge.DiscoveryPermissionDto.granted,
          candidates: [_route],
        ),
      );
    }
  }

  void _stop() {
    if (browsing) {
      browsing = false;
      shutdowns++;
    }
    if (started.isCompleted && !pending.isCompleted) {
      pending.completeError(const HidlinsApiError.syncCanceled());
    }
  }

  @override
  dynamic noSuchMethod(Invocation invocation) {
    // Keep the existing public repository executable before stop's additive
    // generated method exists; a missing symbol is never the regression red.
    if (invocation.isMethod && invocation.memberName == #stopLocalDiscovery) {
      return Future<void>.sync(_stop);
    }
    return super.noSuchMethod(invocation);
  }
}

final class _Native implements LocalDiscoveryCapability {
  int stops = 0;

  @override
  Future<PlatformResult<PlatformUnit>> stop() async {
    stops++;
    return const PlatformSuccess(platformUnit);
  }

  @override
  Future<PlatformResult<PlatformUnit>> openSettings() async =>
      const PlatformSuccess(platformUnit);

  @override
  Future<PlatformResult<model.LocalDiscoveryStatus>> permissionStatus() async =>
      const PlatformSuccess(
        model.LocalDiscoveryStatus(
          permission: model.LocalDiscoveryPermission.granted,
        ),
      );

  @override
  Future<PlatformResult<model.LocalDiscoveryStatus>> discover(
    model.DiscoveryKind kind,
  ) => permissionStatus();
}
