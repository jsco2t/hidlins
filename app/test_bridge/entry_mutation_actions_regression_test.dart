import 'dart:io';

import 'package:app/src/app.dart';
import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/error.dart' as api_error;
import 'package:app/src/bridge/frb_generated.dart';
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

Future<void> _open(WidgetTester tester, String action) async {
  final label = find.text(action);
  expect(label, findsOneWidget, reason: 'shipping detail offers $action');
  await tester.ensureVisible(label);
  await tester.tap(label);
  await _drive(
    tester,
    () => find.widgetWithText(AlertDialog, action).evaluate().isNotEmpty,
  );
  await tester.pump(const Duration(milliseconds: 350));
}

void main() {
  setUpAll(() async {
    final path = Platform.environment['HIDLINS_API_LIB'];
    expect(path, isNotNull, reason: 'run make app-test-entry-mutation-actions');
    expect(File(path!).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(path));
  });

  testWidgets(
    'shipping move/purge preserve identity/history/password and unrelated KDBX data',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(1280, 1000);
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final directory = Directory.systemTemp.createTempSync(
        'hidlins-entry-actions-',
      );
      final session = (await tester.runAsync(
        () => api.initApp(cfg: bridge.AppInitConfig(stateDir: directory.path)),
      ))!;
      try {
        await tester.runAsync(() async {
          await session.createVault(
            name: 'fixture',
            masterPassword: 'entry-actions-master',
            confirmedNoRecovery: true,
          );
          await session.unlock(
            name: 'fixture',
            masterPassword: 'entry-actions-master',
          );
        });
        final initial = (await tester.runAsync(session.vaultTree))!;
        final destination = (await tester.runAsync(
          () => session.createGroup(parent: initial.root.uuid, name: 'Work'),
        ))!;
        final uuid = (await tester.runAsync(
          () => session.createEntry(
            group: initial.root.uuid,
            draft: const bridge.EntryDraftDto(
              kind: bridge.EntryKindDto.credential,
              title: 'prior subject',
              username: 'user',
              password: 'move-preserved-secret-marker',
              tags: [],
              customFields: [],
            ),
          ),
        ))!;
        final unrelated = (await tester.runAsync(
          () => session.createEntry(
            group: initial.root.uuid,
            draft: const bridge.EntryDraftDto(
              kind: bridge.EntryKindDto.secureNote,
              title: 'unrelated',
              notes: 'retained note',
              tags: [],
              customFields: [],
            ),
          ),
        ))!;
        await tester.runAsync(
          () => session.updateEntry(
            uuid: uuid,
            edit: const bridge.EntryEditDto(
              title: 'subject',
              notes: 'preserved note',
            ),
          ),
        );
        final historyBefore = (await tester.runAsync(
          () => session.entryHistory(uuid: uuid),
        ))!;
        expect(historyBefore.length, 1);
        await tester.pumpWidget(
          HidlinsApp(
            overrides: [appSessionProvider.overrideWithValue(session)],
          ),
        );
        await _drive(tester, () => find.text('subject').evaluate().isNotEmpty);
        await tester.tap(find.text('subject').first);
        await _drive(
          tester,
          () => find.byTooltip('Edit').evaluate().isNotEmpty,
        );
        await tester.pump(const Duration(milliseconds: 350));

        await _open(tester, 'Move entry');
        await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
        await tester.pumpAndSettle();
        expect(
          (await tester.runAsync(session.vaultTree))!.entries
              .singleWhere((e) => e.uuid == uuid)
              .groupUuid,
          initial.root.uuid,
        );
        await _open(tester, 'Move entry');
        await tester.tap(find.byType(DropdownButtonFormField<String>));
        await tester.pumpAndSettle();
        final choice = find.byWidgetPredicate(
          (w) => w is DropdownMenuItem<String> && w.value == destination,
        );
        await tester.tap(choice.last);
        await tester.pumpAndSettle();
        await tester.tap(find.widgetWithText(FilledButton, 'Move'));
        await _drive(
          tester,
          () =>
              find.byType(AlertDialog).evaluate().isEmpty &&
              find.text('subject').evaluate().isNotEmpty &&
              find.text('unrelated').evaluate().isEmpty,
        );
        final moved = (await tester.runAsync(session.vaultTree))!.entries
            .singleWhere((e) => e.uuid == uuid);
        expect(moved.groupUuid, destination);
        expect(moved.title, 'subject');
        expect(
          (await tester.runAsync(() => session.entryDetail(uuid: uuid)))!.notes,
          'preserved note',
        );
        expect(
          await tester.runAsync(
            () => session.revealField(
              uuid: uuid,
              field: const bridge.RevealField.password(),
            ),
          ),
          'move-preserved-secret-marker',
        );
        final historyAfter = (await tester.runAsync(
          () => session.entryHistory(uuid: uuid),
        ))!;
        expect(historyAfter.map((h) => h.title).toList(), [
          'subject',
          'prior subject',
        ]);
        expect(historyAfter.last.username, historyBefore.single.username);
        expect(
          historyAfter.last.lastModificationTime,
          historyBefore.single.lastModificationTime,
        );
        expect(find.text('subject'), findsWidgets);
        expect(find.text('move-preserved-secret-marker'), findsNothing);

        await _open(tester, 'Permanently delete');
        expect(find.text('This cannot be undone.'), findsOneWidget);
        await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
        await tester.pumpAndSettle();
        expect(
          (await tester.runAsync(() => session.entryDetail(uuid: uuid)))!.uuid,
          uuid,
        );
        await _open(tester, 'Permanently delete');
        await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
        await _drive(
          tester,
          () =>
              find.text('subject').evaluate().isEmpty &&
              find.text('No entries yet').evaluate().isNotEmpty &&
              find.byType(AlertDialog).evaluate().isEmpty,
        );
        await tester.runAsync(
          () => expectLater(
            session.entryDetail(uuid: uuid),
            throwsA(
              isA<api_error.HidlinsApiError_InvalidInput>().having(
                (e) => e.field,
                'field',
                'uuid',
              ),
            ),
          ),
        );
        expect(
          (await tester.runAsync(() => session.entryDetail(uuid: unrelated)))!
              .notes,
          'retained note',
        );
        await tester.runAsync(session.lockNow);
        await tester.runAsync(
          () => session.unlock(
            name: 'fixture',
            masterPassword: 'entry-actions-master',
          ),
        );
        await tester.runAsync(
          () => expectLater(
            session.entryDetail(uuid: uuid),
            throwsA(
              isA<api_error.HidlinsApiError_InvalidInput>().having(
                (e) => e.field,
                'field',
                'uuid',
              ),
            ),
          ),
        );
        expect(
          (await tester.runAsync(() => session.entryDetail(uuid: unrelated)))!
              .title,
          'unrelated',
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
