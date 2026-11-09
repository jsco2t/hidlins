import 'dart:io';

import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/error.dart';
import 'package:app/src/bridge/frb_generated.dart';
import 'package:app/src/data/bridge_repositories.dart';
import 'package:app/src/data/failures.dart';
import 'package:app/src/data/models.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';

void main() {
  setUpAll(() async {
    final path = Platform.environment['HIDLINS_API_LIB'];
    expect(path, isNotNull, reason: 'run make app-test-generated-copy');
    expect(File(path!).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(path));
  });

  test('real generated value/origin crosses both directions and lock revokes old output', () async {
    final directory = Directory.systemTemp.createTempSync(
      'hidlins-generated-copy-',
    );
    final session = await api.initApp(
      cfg: bridge.AppInitConfig(stateDir: directory.path),
    );
    final generator = BridgeGeneratorRepository(session);
    final clipboard = BridgeSecretsRepository(session);
    try {
      await session.createVault(
        name: 'fixture',
        masterPassword: 'fixture-copy-master-marker',
        confirmedNoRecovery: true,
      );
      await session.unlock(
        name: 'fixture',
        masterPassword: 'fixture-copy-master-marker',
      );
      final displayed = await generator.generatePassword(
        const PasswordOptionsDto(length: 32),
      );
      final independent = await generator.generatePassphrase(
        const PassphraseOptionsDto(),
      );
      expect(displayed.copyOrigin, isNotNull);
      expect(displayed.copyOrigin, isNot(BigInt.zero));
      expect(independent.copyOrigin, displayed.copyOrigin);
      // A headless host may lack a system clipboard. The deterministic Rust
      // recording-port tests prove exact bytes/TTL; this check requires the
      // real bridge to reach that port or return its precise typed I/O error.
      try {
        await clipboard.copyGeneratedSecret(displayed);
      } on IoFailure catch (failure) {
        expect(failure.context, contains('clipboard'));
      }
      await expectLater(
        session.prepareGeneratedClipboardTransfer(
          result: bridge.GeneratedSecret(
            value: displayed.value,
            entropyBits: displayed.entropyBits,
            copyOrigin: displayed.copyOrigin!,
          ),
        ),
        throwsA(isA<HidlinsApiError_UnsupportedPlatform>()),
      );
      await session.lockNow();
      await session.unlock(
        name: 'fixture',
        masterPassword: 'fixture-copy-master-marker',
      );
      await expectLater(
        clipboard.copyGeneratedSecret(displayed),
        throwsA(isA<VaultIsLocked>()),
      );
      final current = await generator.generatePassphrase(
        const PassphraseOptionsDto(),
      );
      expect(current.copyOrigin, isNot(displayed.copyOrigin));
      await session.lockNow();
      final lockedGeneration = await generator.generatePassword(
        const PasswordOptionsDto(),
      );
      expect(lockedGeneration.value, isNotEmpty);
      await expectLater(
        clipboard.copyGeneratedSecret(lockedGeneration),
        throwsA(isA<VaultIsLocked>()),
      );
    } finally {
      await session.shutdown();
      directory.deleteSync(recursive: true);
    }
  });
}
