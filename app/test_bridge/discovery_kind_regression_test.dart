import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart';
import 'package:app/src/bridge/frb_generated.dart';

void main() {
  late Directory directory;

  setUpAll(() async {
    directory = Directory.systemTemp.createTempSync('hidlins-discovery-kind-');
    final path = Platform.environment['HIDLINS_API_LIB'];
    expect(path, isNotNull, reason: 'run make app-test-discovery-kind');
    expect(File(path!).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(path));
  });

  tearDownAll(() => directory.deleteSync(recursive: true));

  test(
    'both typed services cross the real bridge with explicit routes',
    () async {
      final session = await api.initApp(
        cfg: AppInitConfig(stateDir: directory.path),
      );
      try {
        const manual = LocalEndpointDto(
          address: '192.168.1.30',
          port: 48103,
          scopeId: 0,
        );
        await session.setDiscoveryCandidates(
          permission: DiscoveryPermissionDto.granted,
          candidates: [manual],
        );
        for (final service in DiscoveryServiceDto.values) {
          final status = await session.localDiscoveryStatusFor(
            service: service,
          );
          expect(status.permission, DiscoveryPermissionDto.granted);
          expect(status.candidates.first, manual);
          expect(
            status.candidates.where((route) => route == manual),
            hasLength(1),
          );
          final polled = await session.pollLocalDiscoveryFor(service: service);
          expect(polled.permission, DiscoveryPermissionDto.granted);
          expect(polled.candidates.first, manual);
          expect(
            polled.candidates.where((route) => route == manual),
            hasLength(1),
          );
        }
        final legacy = await session.localDiscoveryStatus();
        expect(legacy.candidates.first, manual);
      } finally {
        await session.shutdown();
      }
    },
  );
}
