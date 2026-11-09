import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';
import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart';
import 'package:app/src/bridge/error.dart';
import 'package:app/src/bridge/frb_generated.dart';

void main() {
  late Directory directory;

  setUpAll(() async {
    directory = Directory.systemTemp.createTempSync('hidlins-discovery-stop-');
    final path = Platform.environment['HIDLINS_API_LIB'];
    expect(path, isNotNull, reason: 'run make app-test-discovery-stop');
    expect(File(path!).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(path));
  });

  tearDownAll(() => directory.deleteSync(recursive: true));

  test(
    'real bridge stop preserves routes, permits restart and rejects dead polls',
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
        final first = await session.pollLocalDiscoveryFor(
          service: DiscoveryServiceDto.trusted,
        );
        expect(first.candidates.first, manual);
        await session.stopLocalDiscovery();
        await session.stopLocalDiscovery();
        expect((await session.localDiscoveryStatus()).candidates.first, manual);
        final restarted = await session.pollLocalDiscoveryFor(
          service: DiscoveryServiceDto.pairing,
        );
        expect(restarted.candidates.first, manual);
        await session.stopLocalDiscovery();
        await session.shutdown();
        await expectLater(
          session.pollLocalDiscovery(),
          throwsA(isA<HidlinsApiError_SyncCanceled>()),
        );
        await session.stopLocalDiscovery();
      } finally {
        await session.shutdown();
      }
    },
  );
}
