import 'dart:io';

import 'package:app/src/app.dart';
import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/frb_generated.dart';
import 'package:app/src/features/entries/entries_page.dart';
import 'package:app/src/platform/keyfile_access.dart';
import 'package:app/src/platform/platform_result.dart';
import 'package:app/src/platform/vault_import.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import '../test/fakes/fake_platform_capabilities.dart';

Future<void> _drive(WidgetTester tester, bool Function() completed) async {
  final elapsed = Stopwatch()..start;
  while (!completed() && elapsed.elapsed < const Duration(seconds: 10)) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 1)),
    );
    await tester.pump(const Duration(milliseconds: 20));
  }
  expect(completed(), isTrue, reason: 'shipping native/UI operation completed');
}

void main() {
  setUpAll(() async {
    final path = Platform.environment['HIDLINS_API_LIB'];
    expect(path, isNotNull, reason: 'run make app-test-imported-keyfile');
    expect(File(path!).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(path));
  });

  testWidgets(
    'shipping app imports and unlocks real keyfile KDBX with unknown registry metadata',
    (tester) async {
      final directory = Directory.systemTemp.createTempSync('hidlins-keyfile-');
      final key = File('${directory.path}/fixture.key')
        ..writeAsBytesSync(List<int>.generate(32, (index) => index));
      final source = (await tester.runAsync(
        () => api.initApp(
          cfg: bridge.AppInitConfig(stateDir: '${directory.path}/source'),
        ),
      ))!;
      final created = (await tester.runAsync(
        () => source.createVault(
          name: 'source',
          masterPassword: 'imported-keyfile-fixture-marker',
          keyfile: bridge.KeyfileRef.path(key.path),
          confirmedNoRecovery: true,
        ),
      ))!;
      await tester.runAsync(source.shutdown);
      final target = (await tester.runAsync(
        () => api.initApp(
          cfg: bridge.AppInitConfig(stateDir: '${directory.path}/target'),
        ),
      ))!;
      final reference = KeyfileReference(
        reference: key.path,
        displayName: 'fixture.key',
      );
      final keys = FakeKeyfileAccessCapability(
        pickResults: [PlatformSuccess(reference)],
        resolveResults: [PlatformSuccess(key.path)],
        releaseResults: const [PlatformSuccess(platformUnit)],
      );
      final imports = FakeVaultImportCapability([
        PlatformSuccess(
          ImportedVault(sourceReference: created.path, displayName: 'imported'),
        ),
      ]);
      try {
        await tester.pumpWidget(
          HidlinsApp(
            overrides: [
              appSessionProvider.overrideWithValue(target),
              keyfileAccessCapabilityProvider.overrideWithValue(keys),
              vaultImportCapabilityProvider.overrideWithValue(imports),
            ],
          ),
        );
        await _drive(
          tester,
          () => find.text('Import a .kdbx file').evaluate().isNotEmpty,
        );
        await tester.tap(find.text('Import a .kdbx file'));
        await _drive(
          tester,
          () => find
              .byType(DropdownButtonFormField<String>)
              .evaluate()
              .isNotEmpty,
        );
        final registered = (await tester.runAsync(target.listVaults))!;
        expect(
          registered.single.hasKeyfile,
          isFalse,
          reason: 'import did not know decryption credentials',
        );
        final select = find.widgetWithText(TextButton, 'Select keyfile');
        expect(select, findsOneWidget);
        await tester.tap(select);
        await tester.pump();
        await tester.enterText(
          find.byType(TextField),
          'imported-keyfile-fixture-marker',
        );
        await tester.tap(find.text('Unlock'));
        await _drive(
          tester,
          () => find.byType(EntriesPage).evaluate().isNotEmpty,
        );
        final tree = await tester.runAsync(target.vaultTree);
        expect(
          tree,
          isNotNull,
          reason: 'Rust actually decrypted the imported KDBX',
        );
        expect(keys.resolved, [reference]);
        expect(keys.released, [reference]);
        expect(tester.takeException(), isNull);
      } finally {
        await tester.pumpWidget(const SizedBox.shrink());
        await tester.pump();
        await tester.runAsync(target.shutdown);
        await tester.pump();
        directory.deleteSync(recursive: true);
      }
    },
  );
}
