import 'dart:async';

import 'package:app/src/app.dart';
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entry_edit.dart';
import 'package:app/src/features/lock/unlock_screen.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import '../fakes/fake_repositories.dart';
import '../helpers/feature_test_helpers.dart';

const _originalExpiry = 1900000123;
final _chosenDate = DateTime.utc(2031, 4, 8);
final _chosenEpoch = _chosenDate.millisecondsSinceEpoch ~/ 1000;

EntryDetail _detail() => const EntryDetail(
  uuid: 'entry-1',
  title: 'subject',
  username: 'user',
  hasPassword: true,
  url: '',
  notes: '',
  kind: EntryKindDto.credential,
  tags: [],
  customFields: [],
  attachments: [],
  expiryTime: _originalExpiry,
);

class _Entries extends FakeEntryRepository {
  final edits = <(String, EntryEditDto, dynamic)>[];
  final creates = <(String, EntryDraftDto, int?)>[];

  _Entries() {
    tree = VaultTree(
      root: GroupNode(
        uuid: 'root-uuid',
        name: 'Root',
        entryCount: BigInt.one,
        children: const [],
      ),
      entries: const [
        EntrySummary(
          uuid: 'entry-1',
          title: 'subject',
          username: 'user',
          url: '',
          kind: EntryKindDto.credential,
          hasTotp: false,
          hasAttachments: false,
          isExpired: false,
          groupUuid: 'root-uuid',
          tags: [],
        ),
      ],
    );
    details['entry-1'] = _detail();
  }

  // Additional optional capture parameters keep the fixture valid against both
  // the original interface and its source-compatible extension.
  @override
  Future<void> updateEntry(
    String uuid,
    EntryEditDto edit, {
    dynamic expiration,
  }) async {
    edits.add((uuid, edit, expiration));
    await super.updateEntry(uuid, edit);
  }

  @override
  Future<String> createEntry(
    String groupUuid,
    EntryDraftDto draft, {
    int? expiryTime,
  }) async {
    creates.add((groupUuid, draft, expiryTime));
    return super.createEntry(groupUuid, draft);
  }
}

Future<TestHarness> _mount(WidgetTester tester, _Entries entries) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = const Size(1280, 1100);
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final h = TestHarness()..session.currentLockState = LockEvent.unlocked;
  registerTestVault(h);
  await tester.pumpWidget(
    HidlinsApp(
      overrides: [
        sessionRepositoryProvider.overrideWithValue(h.session),
        entryRepositoryProvider.overrideWithValue(entries),
        secretsRepositoryProvider.overrideWithValue(h.secrets),
        searchRepositoryProvider.overrideWithValue(h.search),
        totpRepositoryProvider.overrideWithValue(h.totp),
        generatorRepositoryProvider.overrideWithValue(h.generator),
        syncRepositoryProvider.overrideWithValue(h.sync),
        prefsRepositoryProvider.overrideWithValue(h.prefs),
        keyfileAccessCapabilityProvider.overrideWithValue(h.keyfiles),
        vaultImportCapabilityProvider.overrideWithValue(h.vaultImport),
        attachmentExportCapabilityProvider.overrideWithValue(
          h.attachmentExport,
        ),
      ],
    ),
  );
  await tester.pumpAndSettle();
  return h;
}

Future<void> _close(WidgetTester tester, TestHarness h) async {
  await tester.pumpWidget(const SizedBox.shrink());
  await tester.pump();
  h.dispose();
  await tester.pump();
}

Future<void> _edit(WidgetTester tester) async {
  await tester.tap(find.text('subject').first);
  await tester.pumpAndSettle();
  await tester.tap(find.byTooltip('Edit'));
  await tester.pumpAndSettle();
  expect(find.byType(EntryEditDialog), findsOneWidget);
}

Future<void> _openPicker(WidgetTester tester) async {
  final action = find.text('Set expiration');
  expect(action, findsOneWidget, reason: 'shipping editor exposes expiration');
  await tester.ensureVisible(action);
  await tester.tap(action);
  await tester.pumpAndSettle();
  expect(find.byType(DatePickerDialog), findsOneWidget);
}

Future<void> _choose(WidgetTester tester) async {
  final picker = tester.widget<CalendarDatePicker>(
    find.byType(CalendarDatePicker),
  );
  picker.onDateChanged(_chosenDate);
  await tester.pump();
  await tester.tap(find.text('OK'));
  await tester.pumpAndSettle();
}

Future<void> _clear(WidgetTester tester) async {
  final action = find.text('Clear expiration');
  expect(action, findsOneWidget);
  await tester.ensureVisible(action);
  await tester.tap(action);
  await tester.pumpAndSettle();
}

Future<void> _save(WidgetTester tester) async {
  await tester.tap(find.widgetWithText(FilledButton, 'Save'));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('untouched expiration retains the legacy exact-epoch path', (
    tester,
  ) async {
    final entries = _Entries();
    final h = await _mount(tester, entries);
    try {
      await _edit(tester);
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Title'),
        'ordinary edit',
      );
      await _save(tester);
      expect(entries.edits, hasLength(1));
      expect(entries.edits.single.$2.title, 'ordinary edit');
      expect(entries.edits.single.$3, isNull);
      expect(entries.setExpirationCalled, isFalse);
      expect(entries.clearExpirationCalled, isFalse);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets('chosen UTC date and fields use one combined edit', (
    tester,
  ) async {
    final entries = _Entries();
    final h = await _mount(tester, entries);
    try {
      await _edit(tester);
      await _openPicker(tester);
      await _choose(tester);
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Title'),
        'changed with date',
      );
      await _save(tester);
      expect(entries.edits, hasLength(1));
      expect(entries.edits.single.$1, 'entry-1');
      expect(entries.edits.single.$2.title, 'changed with date');
      expect(entries.edits.single.$3, isNotNull);
      expect(entries.edits.single.$3.epochSecs, _chosenEpoch);
      expect(entries.setExpirationCalled, isFalse);
      expect(entries.clearExpirationCalled, isFalse);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets('clear is explicit and coalesces with the field save', (
    tester,
  ) async {
    final entries = _Entries();
    final h = await _mount(tester, entries);
    try {
      await _edit(tester);
      await _clear(tester);
      expect(find.text('Never expires'), findsOneWidget);
      await _save(tester);
      expect(entries.edits, hasLength(1));
      expect(entries.edits.single.$3, isNotNull);
      expect(entries.edits.single.$3.epochSecs, isNull);
      expect(entries.setExpirationCalled, isFalse);
      expect(entries.clearExpirationCalled, isFalse);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets('new entry includes expiration in its only create call', (
    tester,
  ) async {
    final entries = _Entries();
    final h = await _mount(tester, entries);
    try {
      await tester.tap(find.text('New entry').first);
      await tester.pumpAndSettle();
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Title'),
        'created with date',
      );
      await _openPicker(tester);
      await _choose(tester);
      await _save(tester);
      expect(entries.creates, hasLength(1));
      expect(entries.creates.single.$1, 'root-uuid');
      expect(entries.creates.single.$2.title, 'created with date');
      expect(entries.creates.single.$3, _chosenEpoch);
      expect(entries.edits, isEmpty);
      expect(entries.setExpirationCalled, isFalse);
      expect(entries.clearExpirationCalled, isFalse);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets('cancelled picker leaves a clean editor and no mutation', (
    tester,
  ) async {
    final entries = _Entries();
    final h = await _mount(tester, entries);
    try {
      await _edit(tester);
      await _openPicker(tester);
      await tester.tap(find.text('Cancel'));
      await tester.pumpAndSettle();
      await tester.pageBack();
      await tester.pumpAndSettle();
      expect(find.byType(EntryEditDialog), findsNothing);
      expect(find.text('Discard changes?'), findsNothing);
      expect(entries.edits, isEmpty);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  for (final clear in [false, true]) {
    testWidgets('expiration-only ${clear ? 'clear' : 'selection'} is dirty', (
      tester,
    ) async {
      final entries = _Entries();
      final h = await _mount(tester, entries);
      try {
        await _edit(tester);
        if (clear) {
          await _clear(tester);
        } else {
          await _openPicker(tester);
          await _choose(tester);
        }
        await tester.pageBack();
        await tester.pumpAndSettle();
        expect(find.text('Discard changes?'), findsOneWidget);
        expect(entries.edits, isEmpty);
        expect(tester.takeException(), isNull);
      } finally {
        await _close(tester, h);
      }
    });
  }

  testWidgets('failed save retains date and requires secret reentry', (
    tester,
  ) async {
    final entries = _Entries()..updateError = Exception('raw-marker');
    final h = await _mount(tester, entries);
    try {
      await _edit(tester);
      await _openPicker(tester);
      await _choose(tester);
      final password = find.widgetWithText(TextFormField, 'Password');
      await tester.ensureVisible(password);
      await tester.enterText(password, 'submitted-secret');
      await _save(tester);
      expect(entries.edits, hasLength(1));
      expect(entries.edits.single.$3.epochSecs, _chosenEpoch);
      expect(tester.widget<TextFormField>(password).controller!.text, isEmpty);
      expect(
        tester
            .widget<FilledButton>(find.widgetWithText(FilledButton, 'Save'))
            .onPressed,
        isNull,
      );
      expect(find.textContaining('raw-marker'), findsNothing);
      entries.updateError = null;
      await tester.enterText(password, 'replacement-secret');
      await tester.pump();
      await _save(tester);
      expect(entries.edits, hasLength(2));
      expect(entries.edits.last.$3.epochSecs, _chosenEpoch);
      expect(entries.edits.last.$2.password, 'replacement-secret');
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets(
    'completed sync while picker is open blocks stale choice and save',
    (tester) async {
      final entries = _Entries();
      final h = await _mount(tester, entries);
      try {
        await _edit(tester);
        await _openPicker(tester);
        h.sync.syncController.add(
          const bridge.SyncEvent.done(bridge.SyncOutcomeDto.alreadyInSync()),
        );
        await tester.pump();
        await tester.pump();
        await _choose(tester);
        expect(find.text(_chosenDate.toIso8601String()), findsNothing);
        expect(
          tester
              .widget<FilledButton>(find.widgetWithText(FilledButton, 'Save'))
              .onPressed,
          isNull,
        );
        expect(entries.edits, isEmpty);
        expect(tester.takeException(), isNull);
      } finally {
        await _close(tester, h);
      }
    },
  );

  testWidgets('lock removes the picker and editor without saving', (
    tester,
  ) async {
    final entries = _Entries();
    final h = await _mount(tester, entries);
    try {
      await _edit(tester);
      await _openPicker(tester);
      h.session.lockController.add(LockEvent.locked);
      await tester.pump();
      await tester.pump();
      expect(find.byType(DatePickerDialog), findsNothing);
      await tester.pumpAndSettle();
      expect(find.byType(EntryEditDialog), findsNothing);
      expect(find.byType(UnlockScreen), findsOneWidget);
      expect(entries.edits, isEmpty);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets('date controls fail closed while sync status is unknown', (
    tester,
  ) async {
    final gate = Completer<SyncStatusDto>();
    final h = TestHarness()
      ..session.currentLockState = LockEvent.unlocked
      ..sync.statusCompleter = gate;
    try {
      await tester.pumpFeatureWithHarness(
        EntryEditDialog(detail: _detail()),
        h,
      );
      await tester.pump();
      await tester.pump();
      for (final label in ['Set expiration', 'Clear expiration']) {
        final finder = find.ancestor(
          of: find.text(label),
          matching: find.byWidgetPredicate((w) => w is TextButton),
        );
        expect(finder, findsOneWidget);
        expect(tester.widget<TextButton>(finder).onPressed, isNull);
      }
      expect(tester.takeException(), isNull);
    } finally {
      gate.complete(const SyncStatusDto(configured: false, inFlight: false));
      await tester.pump();
      await _close(tester, h);
    }
  });
}
