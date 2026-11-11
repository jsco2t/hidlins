import 'dart:io';

import 'package:app/src/app.dart';
import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/frb_generated.dart';
import 'package:app/src/data/bridge_repositories.dart';
import 'package:app/src/features/entries/entry_edit.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

const _originalExpiry = 1900000123;
final _chosenDate = DateTime.utc(2031, 4, 8);
final _chosenEpoch = _chosenDate.millisecondsSinceEpoch ~/ 1000;

Future<void> _drive(WidgetTester tester, bool Function() done) async {
  final elapsed = Stopwatch()..start();
  while (!done() && elapsed.elapsed < const Duration(seconds: 10)) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 1)),
    );
    await tester.pump(const Duration(milliseconds: 20));
  }
  expect(done(), isTrue, reason: 'native/UI operation completed');
}

Future<void> _openEdit(WidgetTester tester) async {
  await _drive(tester, () => find.byTooltip('Edit').evaluate().isNotEmpty);
  await tester.pump(const Duration(milliseconds: 350));
  await tester.tap(find.byTooltip('Edit'));
  await _drive(
    tester,
    () =>
        find.byType(EntryEditDialog).evaluate().isNotEmpty &&
        find.widgetWithText(TextFormField, 'Title').evaluate().isNotEmpty,
  );
  await tester.pump(const Duration(milliseconds: 350));
}

Future<void> _chooseDate(WidgetTester tester) async {
  final action = find.text('Set expiration');
  expect(action, findsOneWidget, reason: 'shipping editor date action');
  await tester.ensureVisible(action);
  await tester.tap(action);
  await tester.pumpAndSettle();
  expect(find.byType(DatePickerDialog), findsOneWidget);
  tester
      .widget<CalendarDatePicker>(find.byType(CalendarDatePicker))
      .onDateChanged(_chosenDate);
  await tester.pump();
  await tester.tap(find.text('OK'));
  await tester.pumpAndSettle();
}

Future<void> _save(WidgetTester tester, String title) async {
  await tester.tap(find.widgetWithText(FilledButton, 'Save'));
  await _drive(
    tester,
    () =>
        find.byType(EntryEditDialog).evaluate().isEmpty &&
        find.text(title).evaluate().isNotEmpty,
  );
}

void main() {
  setUpAll(() async {
    final path = Platform.environment['HIDLINS_API_LIB'];
    expect(path, isNotNull, reason: 'run make app-test-expiration-editor');
    expect(File(path!).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(path));
  });

  testWidgets(
    'shipping expiration editor preserves exact epoch then saves fields and dates atomically',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(390, 1100);
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final directory = Directory.systemTemp.createTempSync('hidlins-expiry-');
      final session = (await tester.runAsync(
        () => api.initApp(cfg: bridge.AppInitConfig(stateDir: directory.path)),
      ))!;
      final repo = BridgeEntryRepository(session);
      var notifications = 0;
      try {
        await tester.runAsync(() async {
          await session.createVault(
            name: 'fixture',
            masterPassword: 'expiry-master-fixture',
            confirmedNoRecovery: true,
          );
          await session.unlock(
            name: 'fixture',
            masterPassword: 'expiry-master-fixture',
          );
        });
        final tree = (await tester.runAsync(session.vaultTree))!;
        final uuid = (await tester.runAsync(
          () => session.createEntryWithExpiration(
            group: tree.root.uuid,
            expiryTime: _originalExpiry,
            draft: const bridge.EntryDraftDto(
              kind: bridge.EntryKindDto.credential,
              title: 'subject',
              username: 'retained user',
              password: 'expiry-secret-marker',
              notes: 'retained notes',
              tags: ['kept'],
              customFields: [],
            ),
          ),
        ))!;
        await tester.pumpWidget(
          HidlinsApp(
            overrides: [
              appSessionProvider.overrideWithValue(session),
              entryRepositoryProvider.overrideWithValue(repo),
            ],
          ),
        );
        await _drive(tester, () => find.text('subject').evaluate().isNotEmpty);
        final notify = repo.onMutation!;
        repo.onMutation = () {
          notifications++;
          notify();
        };
        await tester.tap(find.text('subject').first);
        await _openEdit(tester);
        await tester.enterText(
          find.widgetWithText(TextFormField, 'Title'),
          'ordinary edit',
        );
        await _save(tester, 'ordinary edit');
        expect(notifications, 1);
        expect(
          (await tester.runAsync(() => session.entryDetail(uuid: uuid)))!
              .expiryTime,
          _originalExpiry,
          reason: 'untouched seconds and time of day are not rounded',
        );

        await _openEdit(tester);
        await _chooseDate(tester);
        await tester.enterText(
          find.widgetWithText(TextFormField, 'Title'),
          'date edit',
        );
        await _save(tester, 'date edit');
        expect(
          notifications,
          2,
          reason: 'one notification for fields and date',
        );
        final changed = (await tester.runAsync(
          () => session.entryDetail(uuid: uuid),
        ))!;
        expect(changed.uuid, uuid);
        expect(changed.expiryTime, _chosenEpoch);
        expect(changed.notes, 'retained notes');
        expect(changed.username, 'retained user');
        expect(changed.tags, ['kept']);
        final history = (await tester.runAsync(
          () => session.entryHistory(uuid: uuid),
        ))!;
        expect(history.map((h) => h.title), ['ordinary edit', 'subject']);
        expect(
          await tester.runAsync(
            () => session.revealField(
              uuid: uuid,
              field: const bridge.RevealField.password(),
            ),
          ),
          'expiry-secret-marker',
        );

        await _openEdit(tester);
        final clear = find.text('Clear expiration');
        expect(clear, findsOneWidget);
        await tester.ensureVisible(clear);
        await tester.tap(clear);
        await tester.pumpAndSettle();
        expect(find.text('Never expires'), findsOneWidget);
        await _save(tester, 'date edit');
        expect(notifications, 3);
        expect(
          (await tester.runAsync(() => session.entryDetail(uuid: uuid)))!
              .expiryTime,
          isNull,
        );
        expect(
          (await tester.runAsync(() => session.entryHistory(uuid: uuid)))!,
          hasLength(3),
        );

        await tester.tap(find.byType(BackButton));
        await tester.pumpAndSettle();
        await tester.tap(find.text('New entry').first);
        await _drive(
          tester,
          () =>
              find.widgetWithText(TextFormField, 'Title').evaluate().isNotEmpty,
        );
        await tester.pump(const Duration(milliseconds: 350));
        await tester.enterText(
          find.widgetWithText(TextFormField, 'Title'),
          'created with expiry',
        );
        await _chooseDate(tester);
        await _save(tester, 'created with expiry');
        expect(notifications, 4, reason: 'creation has one durable operation');
        final created = (await tester.runAsync(session.vaultTree))!.entries
            .singleWhere((e) => e.title == 'created with expiry');
        expect(
          (await tester.runAsync(
            () => session.entryDetail(uuid: created.uuid),
          ))!.expiryTime,
          _chosenEpoch,
        );
        expect(
          (await tester.runAsync(
            () => session.entryHistory(uuid: created.uuid),
          ))!,
          isEmpty,
        );
        expect(find.text('expiry-secret-marker'), findsNothing);
        await tester.runAsync(session.lockNow);
        await tester.runAsync(
          () => session.unlock(
            name: 'fixture',
            masterPassword: 'expiry-master-fixture',
          ),
        );
        expect(
          (await tester.runAsync(() => session.entryDetail(uuid: uuid)))!
              .expiryTime,
          isNull,
        );
        expect(
          (await tester.runAsync(
            () => session.entryDetail(uuid: created.uuid),
          ))!.expiryTime,
          _chosenEpoch,
        );
        expect(tester.takeException(), isNull);
      } finally {
        repo.onMutation = null;
        await tester.pumpWidget(const SizedBox.shrink());
        await tester.pump();
        await tester.runAsync(session.shutdown);
        await tester.pump();
        directory.deleteSync(recursive: true);
      }
    },
  );
}
