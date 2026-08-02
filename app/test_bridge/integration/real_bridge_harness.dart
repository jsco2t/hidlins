import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/frb_generated.dart';

Future<void> initializeRealBridge() async {
  final libPath = Platform.environment['HIDLINS_API_LIB'];
  expect(
    libPath,
    isNotNull,
    reason: 'HIDLINS_API_LIB is set by `make app-test-integration`',
  );
  expect(File(libPath!).existsSync(), isTrue);
  await RustLib.init(externalLibrary: ExternalLibrary.open(libPath));
}

Future<Directory> createTestVault({
  required String name,
  required String password,
}) async {
  final stateDir = Directory.systemTemp.createTempSync(
    'hidlins-app-integration-',
  );
  api.AppSession? session;
  try {
    session = await api.initApp(
      cfg: bridge.AppInitConfig(stateDir: stateDir.path),
    );
    await session.createVault(
      name: name,
      masterPassword: password,
      confirmedNoRecovery: true,
    );
    await session.shutdown();
    session = null;
    return stateDir;
  } catch (_) {
    if (session != null) {
      try {
        await session.shutdown();
      } on Object {
        // Preserve the setup failure; the temp directory is still removed.
      }
    }
    if (stateDir.existsSync()) stateDir.deleteSync(recursive: true);
    rethrow;
  }
}

Directory preparedFixtureDirectory(String environmentVariable) {
  final path = Platform.environment[environmentVariable];
  expect(
    path,
    isNotNull,
    reason: '$environmentVariable is prepared by `make app-test-integration`',
  );
  final directory = Directory(path!);
  expect(
    directory.existsSync(),
    isTrue,
    reason: '$environmentVariable must name an existing fixture directory',
  );
  return directory;
}
