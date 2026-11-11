import 'dart:io';

import 'package:app/src/app.dart';
import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/frb_generated.dart';
import 'package:app/src/features/entries/entries_page.dart';
import 'package:app/src/features/lock/unlock_screen.dart';
import 'package:app/src/features/vaults/vault_management_page.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

Future<void> _drive(WidgetTester tester, bool Function() completed) async {
  final elapsed = Stopwatch()..start();
  while (!completed() && elapsed.elapsed < const Duration(seconds: 10)) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 1)),
    );
    await tester.pump(const Duration(milliseconds: 20));
  }
  expect(completed(), isTrue, reason: 'native/UI operation completed');
}

Future<void> _removeMenu(WidgetTester tester) async {
  final tile = find.ancestor(
    of: find.text('second'),
    matching: find.byType(ListTile),
  );
  await tester.tap(
    find.descendant(
      of: tile,
      matching: find.byWidgetPredicate((w) => w is PopupMenuButton),
    ),
  );
  await tester.pumpAndSettle();
  await tester.tap(find.text('Remove vault'));
  await tester.pumpAndSettle();
}

void main() {
  setUpAll(() async {
    final path = Platform.environment['HIDLINS_API_LIB'];
    expect(path, isNotNull, reason: 'run make app-test-vault-management-route');
    expect(File(path!).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(path));
  });

  testWidgets(
    'shipping nonempty registry creates/removes vault without losing either KDBX',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(1200, 1000);
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final directory = Directory.systemTemp.createTempSync('hidlins-manager-');
      final session = (await tester.runAsync(
        () => api.initApp(
          cfg: bridge.AppInitConfig(stateDir: '${directory.path}/app'),
        ),
      ))!;
      api.AppSession? verifier;
      try {
        final original = (await tester.runAsync(
          () => session.createVault(
            name: 'original',
            masterPassword: 'original-marker',
            confirmedNoRecovery: true,
          ),
        ))!;
        final originalBytes = File(original.path).readAsBytesSync();
        await tester.pumpWidget(
          HidlinsApp(
            overrides: [appSessionProvider.overrideWithValue(session)],
          ),
        );
        await _drive(
          tester,
          () => find
              .byType(DropdownButtonFormField<String>)
              .evaluate()
              .isNotEmpty,
        );
        final manage = find.text('Vaults');
        expect(
          manage,
          findsOneWidget,
          reason: 'nonempty locked registry offers management',
        );
        await tester.tap(manage);
        await _drive(
          tester,
          () => find.byType(VaultManagementPage).evaluate().isNotEmpty,
        );
        await tester.pumpAndSettle();
        await tester.tap(find.byTooltip('Create vault'));
        await tester.pumpAndSettle();
        for (final entry in {
          'Vault name': 'second',
          'Master password': 'second-marker',
          'Confirm password': 'second-marker',
          'Type CONFIRM to acknowledge': 'CONFIRM',
        }.entries) {
          final field = find.widgetWithText(TextFormField, entry.key);
          await tester.ensureVisible(field);
          await tester.enterText(field, entry.value);
        }
        final create = find.widgetWithText(FilledButton, 'Create');
        await tester.ensureVisible(create);
        await tester.tap(create);
        await _drive(
          tester,
          () =>
              find.byType(VaultManagementPage).evaluate().isNotEmpty &&
              find.text('second').evaluate().isNotEmpty,
        );
        await tester.pumpAndSettle();
        final vaults = (await tester.runAsync(session.listVaults))!;
        expect(vaults.map((v) => v.name).toSet(), {'original', 'second'});
        final secondPath = vaults.singleWhere((v) => v.name == 'second').path;
        expect(File(original.path).readAsBytesSync(), originalBytes);
        await _removeMenu(tester);
        await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
        await tester.pumpAndSettle();
        expect((await tester.runAsync(session.listVaults))!.length, 2);
        expect(File(secondPath).existsSync(), isTrue);
        await _removeMenu(tester);
        await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
        await _drive(tester, () => find.text('second').evaluate().isEmpty);
        await tester.pumpAndSettle();
        expect(
          (await tester.runAsync(session.listVaults))!.single.name,
          'original',
        );
        expect(
          File(secondPath).existsSync(),
          isTrue,
          reason: 'default removal preserves KDBX file',
        );
        expect(File(original.path).readAsBytesSync(), originalBytes);
        verifier = (await tester.runAsync(
          () => api.initApp(
            cfg: bridge.AppInitConfig(stateDir: '${directory.path}/verify'),
          ),
        ))!;
        final verification = verifier;
        await tester.runAsync(
          () => verification.registerExistingVault(
            name: 'retained',
            kdbxPath: secondPath,
          ),
        );
        final retainedTree = await tester.runAsync(
          () => verification.unlock(
            name: 'retained',
            masterPassword: 'second-marker',
          ),
        );
        expect(
          retainedTree,
          isNotNull,
          reason: 'removed second vault still decrypts',
        );
        await tester.runAsync(verification.shutdown);
        verifier = null;
        await tester.tap(find.byTooltip('Back'));
        await _drive(
          tester,
          () => find.byType(UnlockScreen).evaluate().isNotEmpty,
        );
        await tester.pumpAndSettle();
        await tester.enterText(find.byType(TextField), 'original-marker');
        await tester.tap(find.widgetWithText(FilledButton, 'Unlock'));
        await _drive(
          tester,
          () => find.byType(EntriesPage).evaluate().isNotEmpty,
        );
        expect(await tester.runAsync(session.vaultTree), isNotNull);
        expect(tester.takeException(), isNull);
      } finally {
        await tester.pumpWidget(const SizedBox.shrink());
        await tester.pump();
        if (verifier != null) await tester.runAsync(verifier.shutdown);
        await tester.runAsync(session.shutdown);
        await tester.pump();
        directory.deleteSync(recursive: true);
      }
    },
  );
}
