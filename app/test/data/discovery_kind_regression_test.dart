import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/error.dart';
import 'package:app/src/data/bridge_repositories.dart';
import 'package:app/src/data/models.dart' as model;
import 'package:app/src/platform/local_discovery.dart';
import 'package:app/src/platform/platform_result.dart';

const _closedTrusted = bridge.LocalEndpointDto(
  address: '192.168.1.10',
  port: 48101,
  scopeId: 0,
);
const _openPairing = bridge.LocalEndpointDto(
  address: '192.168.1.20',
  port: 48102,
  scopeId: 0,
);
const _manual = bridge.LocalEndpointDto(
  address: '192.168.1.30',
  port: 48103,
  scopeId: 0,
);

void main() {
  test('desktop Trusted discovery retains normal sync routes', () async {
    final repo = BridgeSyncRepository(_KindSession());
    final status = await repo.discover(model.DiscoveryKind.trusted);
    expect(status.permission, model.LocalDiscoveryPermission.granted);
    expect(status.candidates.map((route) => route.port), [_closedTrusted.port]);
  });

  test(
    'desktop Pairing discovery excludes a closed Trusted authority',
    () async {
      final repo = BridgeSyncRepository(_KindSession());
      final status = await repo.discover(model.DiscoveryKind.pairing);
      expect(status.permission, model.LocalDiscoveryPermission.granted);
      expect(status.candidates.map((route) => route.port), [_openPairing.port]);
    },
  );

  test(
    'pair import uses the open Pairing authority instead of Trusted',
    () async {
      final session = _KindSession();
      final repo = BridgeSyncRepository(session);
      final prompt = await repo.beginPairImport(
        name: 'imported',
        masterPassword: 'discovery fixture password',
      );
      expect(prompt.sas, 'ABC-123');
      expect(prompt.transactionHandle.length, 32);
      expect(session.importCandidates.first, _openPairing);
      expect(session.importCandidates, isNot(contains(_closedTrusted)));
    },
  );

  test('explicit manual import route stays first and usable', () async {
    final session = _KindSession();
    final repo = BridgeSyncRepository(session);
    await repo.setManualEndpoint(
      const model.LocalEndpoint(address: '192.168.1.30', port: 48103),
    );
    final prompt = await repo.beginPairImport(
      name: 'manual-import',
      masterPassword: 'discovery fixture password',
    );
    expect(prompt.sas, 'ABC-123');
    expect(session.importCandidates.first, _manual);
  });

  test(
    'native Pairing discovery still reaches Rust validation and import',
    () async {
      final session = _KindSession();
      final native = _NativeDiscovery();
      final repo = BridgeSyncRepository(
        session,
        useNativeDiscovery: true,
        nativeDiscovery: native,
      );
      final status = await repo.discover(model.DiscoveryKind.pairing);
      expect(native.lastKind, model.DiscoveryKind.pairing);
      expect(status.candidates.single.port, _openPairing.port);
      expect(session.injected, [_openPairing]);
      final prompt = await repo.beginPairImport(
        name: 'native-import',
        masterPassword: 'discovery fixture password',
      );
      expect(prompt.sas, 'ABC-123');
      expect(session.importCandidates.first, _openPairing);
    },
  );
}

/// Two distinct routing sets model a normal authority whose pairing window is
/// closed and another authority currently advertising an open pairing window.
/// These are untrusted routes: successful pairing still belongs to Rust.
final class _KindSession implements api.AppSession {
  List<bridge.LocalEndpointDto> injected = [];
  List<bridge.LocalEndpointDto> importCandidates = [];

  bridge.DiscoveryStatusDto _status(String service) {
    final route = switch (service) {
      'trusted' => _closedTrusted,
      'pairing' => _openPairing,
      _ => throw StateError('unrecognized discovery service'),
    };
    return bridge.DiscoveryStatusDto(
      permission: bridge.DiscoveryPermissionDto.granted,
      candidates: [...injected, if (!injected.contains(route)) route],
    );
  }

  @override
  Future<bridge.DiscoveryStatusDto> pollLocalDiscovery() async =>
      _status('trusted');

  @override
  Future<bridge.DiscoveryStatusDto> localDiscoveryStatus() async =>
      _status('trusted');

  @override
  Future<void> setDiscoveryCandidates({
    required bridge.DiscoveryPermissionDto permission,
    required List<bridge.LocalEndpointDto> candidates,
  }) async {
    expect(permission, bridge.DiscoveryPermissionDto.granted);
    injected = List.of(candidates);
  }

  @override
  Future<bridge.PairingPromptDto> beginPairImport({
    required String name,
    required String masterPassword,
    bridge.KeyfileRef? keyfile,
    required List<bridge.LocalEndpointDto> candidates,
  }) async {
    importCandidates = List.of(candidates);
    // A closed pairing window rejects the XX preface before an authenticated
    // prompt exists. A valid first manual or Pairing route remains usable.
    if (candidates.isEmpty) {
      throw const HidlinsApiError.syncNotFound();
    }
    if (candidates.first == _closedTrusted) {
      throw const HidlinsApiError.syncAuthFailed();
    }
    expect(candidates.first, anyOf(_openPairing, _manual));
    return const bridge.PairingPromptDto(
      transactionHandle: '0123456789abcdef0123456789abcdef',
      sas: 'ABC-123',
    );
  }

  @override
  dynamic noSuchMethod(Invocation invocation) {
    // Forward-compatible fixture: it compiles before the additive bridge
    // methods/type are generated, while the existing public repository runs
    // unchanged. Baseline failures concern returned routes/authentication,
    // never a missing generated symbol or a compiler failure.
    if (invocation.isMethod &&
        (invocation.memberName == #pollLocalDiscoveryFor ||
            invocation.memberName == #localDiscoveryStatusFor)) {
      final service = invocation.namedArguments[#service];
      if (service is! Enum) {
        throw StateError('discovery requires a typed service');
      }
      return Future<bridge.DiscoveryStatusDto>.value(_status(service.name));
    }
    return super.noSuchMethod(invocation);
  }
}

final class _NativeDiscovery implements LocalDiscoveryCapability {
  model.DiscoveryKind? lastKind;

  @override
  Future<PlatformResult<model.LocalDiscoveryStatus>> discover(
    model.DiscoveryKind kind,
  ) async {
    lastKind = kind;
    final endpoint = kind == model.DiscoveryKind.pairing
        ? _openPairing
        : _closedTrusted;
    return PlatformSuccess(
      model.LocalDiscoveryStatus(
        permission: model.LocalDiscoveryPermission.granted,
        candidates: [
          model.LocalEndpoint(
            address: endpoint.address,
            port: endpoint.port,
            scopeId: endpoint.scopeId,
          ),
        ],
      ),
    );
  }

  @override
  Future<PlatformResult<model.LocalDiscoveryStatus>> permissionStatus() async =>
      const PlatformSuccess(
        model.LocalDiscoveryStatus(
          permission: model.LocalDiscoveryPermission.granted,
        ),
      );

  @override
  Future<PlatformResult<PlatformUnit>> openSettings() async =>
      const PlatformSuccess(platformUnit);

  @override
  Future<PlatformResult<PlatformUnit>> stop() async =>
      const PlatformSuccess(platformUnit);
}
