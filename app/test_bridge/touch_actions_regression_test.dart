import 'dart:io';

import 'package:app/src/app.dart';
import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/frb_generated.dart';
import 'package:app/src/features/entries/entry_edit.dart';
import 'package:app/src/features/lock/unlock_screen.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:material_ui/material_ui.dart';

Future<void> _drive(WidgetTester tester, bool Function() completed) async {
  final elapsed = Stopwatch()..start;
  while (!completed() && elapsed.elapsed < const Duration(seconds: 10)) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 1)),
    );
    await tester.pump(const Duration(milliseconds: 20));
  }
  expect(completed(), isTrue, reason: 'native/UI operation settled');
}

void main() {
  setUpAll(() async {
    final path = Platform.environment['HIDLINS_API_LIB'];
    expect(path, isNotNull, reason: 'run make app-test-touch-actions');
    expect(File(path!).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(path));
  });

  testWidgets(
    'shipping phone touch actions create delete and lock a real vault without losing saved state',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(390, 1000);
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final directory = Directory.systemTemp.createTempSync('hidlins-touch-');
      final native = (await tester.runAsync(
        () => api.initApp(cfg: bridge.AppInitConfig(stateDir: directory.path)),
      ))!;
      try {
        await tester.runAsync(() async {
          await native.createVault(
            name: 'fixture',
            masterPassword: 'touch-fixture-marker',
            confirmedNoRecovery: true,
          );
          await native.unlock(
            name: 'fixture',
            masterPassword: 'touch-fixture-marker',
          );
        });
        await tester.pumpWidget(
          HidlinsApp(overrides: [appSessionProvider.overrideWithValue(native)]),
        );
        await _drive(
          tester,
          () => find.text('No entries yet').evaluate().isNotEmpty,
        );
        final newEntry = find.widgetWithText(FilledButton, 'New entry');
        expect(newEntry, findsOneWidget);
        await tester.tap(newEntry);
        await _drive(
          tester,
          () => find.byType(EntryEditDialog).evaluate().isNotEmpty,
        );
        await tester.pump(const Duration(milliseconds: 350));
        await tester.enterText(
          find.widgetWithText(TextFormField, 'Title'),
          'touch-fixture-entry',
        );
        await tester.tap(find.widgetWithText(FilledButton, 'Save'));
        await _drive(
          tester,
          () => find.byType(EntryEditDialog).evaluate().isEmpty,
        );
        await _drive(
          tester,
          () => find.text('touch-fixture-entry').evaluate().isNotEmpty,
        );
        final before = (await tester.runAsync(native.vaultTree))!;
        final entry = before.entries.singleWhere(
          (entry) => entry.title == 'touch-fixture-entry',
        );
        expect(entry.groupUuid, before.root.uuid);
        await tester.longPress(find.text('touch-fixture-entry').first);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 350));
        final delete = find.widgetWithText(MenuItemButton, 'Delete');
        expect(delete, findsOneWidget);
        await tester.tap(delete);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 350));
        await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 350));
        expect(
          (await tester.runAsync(native.vaultTree))!.entries
              .singleWhere((value) => value.uuid == entry.uuid)
              .groupUuid,
          before.root.uuid,
        );
        await tester.longPress(find.text('touch-fixture-entry').first);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 350));
        await tester.tap(find.widgetWithText(MenuItemButton, 'Delete'));
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 350));
        await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
        await _drive(
          tester,
          () => find.widgetWithText(FilledButton, 'Delete').evaluate().isEmpty,
        );
        bridge.VaultTree? removed;
        final deletion = Stopwatch()..start;
        do {
          await tester.pump();
          removed = await tester.runAsync(native.vaultTree);
          if (removed!.entries
              .where(
                (value) =>
                    value.uuid == entry.uuid &&
                    value.groupUuid == before.root.uuid,
              )
              .isEmpty) {
            break;
          }
          await tester.runAsync(
            () => Future<void>.delayed(const Duration(milliseconds: 1)),
          );
        } while (deletion.elapsed < const Duration(seconds: 10));
        expect(
          removed.entries.where(
            (value) =>
                value.uuid == entry.uuid && value.groupUuid == before.root.uuid,
          ),
          isEmpty,
        );
        expect(find.byTooltip('Lock'), findsOneWidget);
        await tester.tap(find.byTooltip('Lock'));
        await _drive(
          tester,
          () => find.byType(UnlockScreen).evaluate().isNotEmpty,
        );
        await tester.runAsync(
          () => native.unlock(
            name: 'fixture',
            masterPassword: 'touch-fixture-marker',
          ),
        );
        final reopened = (await tester.runAsync(native.vaultTree))!;
        expect(
          reopened.entries.where(
            (value) =>
                value.uuid == entry.uuid && value.groupUuid == before.root.uuid,
          ),
          isEmpty,
        );
        expect(tester.takeException(), isNull);
      } finally {
        await tester.pumpWidget(const SizedBox.shrink());
        await tester.pump();
        await tester.runAsync(native.shutdown);
        await tester.pump();
        directory.deleteSync(recursive: true);
      }
    },
  );
}
