import 'dart:io';

import 'package:app/src/app.dart';
import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/frb_generated.dart';
import 'package:app/src/features/entries/entry_edit.dart';
import 'package:app/src/features/entries/entry_history.dart';
import 'package:app/src/features/lock/unlock_screen.dart';
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

void main() {
  setUpAll(() async {
    final path = Platform.environment['HIDLINS_API_LIB'];
    expect(path, isNotNull, reason: 'run make app-test-history-access');
    expect(File(path!).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(path));
  });

  testWidgets(
    'shipping UI edit exposes actual prior Rust snapshot and clears history on lock',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(390, 1000);
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final directory = Directory.systemTemp.createTempSync('hidlins-history-');
      final session = (await tester.runAsync(
        () => api.initApp(cfg: bridge.AppInitConfig(stateDir: directory.path)),
      ))!;
      try {
        await tester.runAsync(() async {
          await session.createVault(
            name: 'fixture',
            masterPassword: 'history-fixture-master',
            confirmedNoRecovery: true,
          );
          await session.unlock(
            name: 'fixture',
            masterPassword: 'history-fixture-master',
          );
        });
        final tree = (await tester.runAsync(session.vaultTree))!;
        final uuid = (await tester.runAsync(
          () => session.createEntry(
            group: tree.root.uuid,
            draft: const bridge.EntryDraftDto(
              kind: bridge.EntryKindDto.credential,
              title: 'before-history-title',
              username: 'before-history-user',
              password: 'history-secret-marker',
              tags: [],
              customFields: [],
            ),
          ),
        ))!;
        await tester.pumpWidget(
          HidlinsApp(
            overrides: [appSessionProvider.overrideWithValue(session)],
          ),
        );
        await _drive(
          tester,
          () => find.text('before-history-title').evaluate().isNotEmpty,
        );
        await tester.tap(find.text('before-history-title').first);
        await _drive(
          tester,
          () => find.byTooltip('Edit').evaluate().isNotEmpty,
        );
        await tester.pump(const Duration(milliseconds: 350));
        await tester.tap(find.byTooltip('Edit'));
        await _drive(
          tester,
          () => find.byType(EntryEditDialog).evaluate().isNotEmpty,
        );
        await tester.pump(const Duration(milliseconds: 350));
        await tester.enterText(
          find.widgetWithText(TextFormField, 'Title'),
          'after-history-title',
        );
        await tester.enterText(
          find.widgetWithText(TextFormField, 'Username'),
          'after-history-user',
        );
        await tester.tap(find.widgetWithText(FilledButton, 'Save'));
        await _drive(
          tester,
          () => find.byType(EntryEditDialog).evaluate().isEmpty,
        );
        await _drive(
          tester,
          () => find.text('after-history-title').evaluate().isNotEmpty,
        );
        final snapshots = (await tester.runAsync(
          () => session.entryHistory(uuid: uuid),
        ))!;
        expect(snapshots.single.title, 'before-history-title');
        expect(snapshots.single.username, 'before-history-user');
        final history = find.text('Entry history');
        expect(
          history,
          findsOneWidget,
          reason: 'actual shipping detail exposes persisted history',
        );
        await tester.ensureVisible(history);
        await tester.tap(history);
        await _drive(
          tester,
          () => find
              .descendant(
                of: find.byType(EntryHistorySheet),
                matching: find.text('before-history-title'),
              )
              .evaluate()
              .isNotEmpty,
        );
        expect(find.text('before-history-user'), findsOneWidget);
        expect(find.text('history-secret-marker'), findsNothing);
        await tester.runAsync(session.lockNow);
        await _drive(
          tester,
          () => find.byType(UnlockScreen).evaluate().isNotEmpty,
        );
        expect(find.byType(EntryHistorySheet), findsNothing);
        expect(find.text('before-history-title'), findsNothing);
        expect(find.text('history-secret-marker'), findsNothing);
        await tester.runAsync(
          () => session.unlock(
            name: 'fixture',
            masterPassword: 'history-fixture-master',
          ),
        );
        expect(
          (await tester.runAsync(() => session.entryHistory(uuid: uuid)))!
              .single
              .title,
          'before-history-title',
        );
        expect(tester.takeException(), isNull);
      } finally {
        await tester.pumpWidget(const SizedBox.shrink());
        await tester.pump();
        await tester.runAsync(session.shutdown);
        await tester.pump();
        directory.deleteSync(recursive: true);
      }
    },
  );
}
