import 'dart:async';

import 'package:app/src/app.dart';
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/data/models.dart';
import 'package:app/src/features/lock/unlock_screen.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import '../fakes/fake_repositories.dart';
import '../helpers/feature_test_helpers.dart';

const _subject = EntrySummary(
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
);
const _unrelated = EntrySummary(
  uuid: 'entry-2',
  title: 'unrelated',
  username: '',
  url: '',
  kind: EntryKindDto.secureNote,
  hasTotp: false,
  hasAttachments: false,
  isExpired: false,
  groupUuid: 'root-uuid',
  tags: [],
);

class _Entries extends FakeEntryRepository {
  final List<(String, String)> moves = [];
  final List<String> purges = [];
  Object? mutationError;

  _Entries({bool withDestination = true}) {
    tree = VaultTree(
      root: GroupNode(
        uuid: 'root-uuid',
        name: 'Root',
        entryCount: BigInt.two,
        children: [
          if (withDestination)
            GroupNode(
              uuid: 'work-uuid',
              name: 'Work',
              entryCount: BigInt.zero,
              children: const [],
            ),
        ],
      ),
      entries: const [_subject, _unrelated],
    );
    details['entry-1'] = const EntryDetail(
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
    );
  }

  @override
  Future<void> moveEntry(String uuid, String groupUuid) async {
    moves.add((uuid, groupUuid));
    if (mutationError case final error?) throw error;
    final entries = [
      for (final entry in tree.entries)
        if (entry.uuid == uuid)
          EntrySummary(
            uuid: entry.uuid,
            title: entry.title,
            username: entry.username,
            url: entry.url,
            kind: entry.kind,
            hasTotp: entry.hasTotp,
            hasAttachments: entry.hasAttachments,
            isExpired: entry.isExpired,
            groupUuid: groupUuid,
            tags: entry.tags,
          )
        else
          entry,
    ];
    tree = VaultTree(root: tree.root, entries: entries);
    onMutation?.call();
  }

  @override
  Future<void> purgeEntry(String uuid) async {
    purges.add(uuid);
    if (mutationError case final error?) throw error;
    tree = VaultTree(
      root: tree.root,
      entries: tree.entries.where((e) => e.uuid != uuid).toList(),
    );
    details.remove(uuid);
    onMutation?.call();
  }
}

class _PendingSync extends FakeSyncRepository {
  final gate = Completer<void>();
  @override
  Future<SyncStatusDto> syncStatus() async {
    await gate.future;
    return super.syncStatus();
  }
}

Future<TestHarness> _mount(
  WidgetTester tester,
  _Entries entries, {
  double width = 1280,
  _PendingSync? pending,
}) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = Size(width, 1000);
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
        syncRepositoryProvider.overrideWithValue(pending ?? h.sync),
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
  await tester.tap(find.text('subject').first);
  await tester.pumpAndSettle();
  return h;
}

Future<void> _close(WidgetTester tester, TestHarness h) async {
  await tester.pumpWidget(const SizedBox.shrink());
  await tester.pump();
  h.dispose();
  await tester.pump();
}

TextButton _button(WidgetTester tester, String action) {
  final text = find.text(action);
  expect(text, findsOneWidget, reason: 'shipping detail offers $action');
  return tester.widget<TextButton>(
    find
        .ancestor(
          of: text,
          matching: find.byWidgetPredicate((w) => w is TextButton),
        )
        .first,
  );
}

Future<void> _open(WidgetTester tester, String action) async {
  _button(tester, action);
  await tester.ensureVisible(find.text(action));
  await tester.tap(find.text(action));
  await tester.pumpAndSettle();
  expect(find.widgetWithText(AlertDialog, action), findsOneWidget);
}

Future<void> _chooseWork(WidgetTester tester) async {
  await tester.tap(find.byType(DropdownButtonFormField<String>));
  await tester.pumpAndSettle();
  expect(
    find.byWidgetPredicate(
      (w) => w is DropdownMenuItem<String> && w.value == 'root-uuid',
    ),
    findsNothing,
    reason: 'current direct group is not a move destination',
  );
  final work = find.byWidgetPredicate(
    (w) => w is DropdownMenuItem<String> && w.value == 'work-uuid',
  );
  await tester.tap(work.last);
  await tester.pumpAndSettle();
}

Future<void> _confirm(WidgetTester tester, String action) async {
  await tester.tap(
    find.widgetWithText(
      FilledButton,
      action == 'Move entry' ? 'Move' : 'Delete',
    ),
  );
  await tester.pumpAndSettle();
}

void main() {
  for (final width in [390.0, 1280.0]) {
    testWidgets(
      'shipping move cancels or selects destination and refreshes list at $width',
      (tester) async {
        final entries = _Entries();
        final h = await _mount(tester, entries, width: width);
        try {
          await _open(tester, 'Move entry');
          expect(
            tester
                .widget<FilledButton>(find.widgetWithText(FilledButton, 'Move'))
                .onPressed,
            isNull,
          );
          await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
          await tester.pumpAndSettle();
          expect(entries.moves, isEmpty);
          await _open(tester, 'Move entry');
          await _chooseWork(tester);
          await _confirm(tester, 'Move entry');
          expect(entries.moves, [('entry-1', 'work-uuid')]);
          expect(
            entries.tree.entries
                .singleWhere((e) => e.uuid == 'entry-1')
                .groupUuid,
            'work-uuid',
          );
          if (width == 390) {
            await tester.tap(find.byTooltip('Back'));
            await tester.pumpAndSettle();
          }
          expect(find.text('subject'), findsWidgets);
          expect(
            find.text('unrelated'),
            findsNothing,
            reason: 'destination group becomes the active list',
          );
          expect(tester.takeException(), isNull);
        } finally {
          await _close(tester, h);
        }
      },
    );

    testWidgets(
      'shipping permanent delete requires confirmation and clears selected detail at $width',
      (tester) async {
        final entries = _Entries();
        final h = await _mount(tester, entries, width: width);
        try {
          await _open(tester, 'Permanently delete');
          expect(find.text('This cannot be undone.'), findsOneWidget);
          await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
          await tester.pumpAndSettle();
          expect(entries.purges, isEmpty);
          await _open(tester, 'Permanently delete');
          await _confirm(tester, 'Permanently delete');
          expect(entries.purges, ['entry-1']);
          expect(find.text('subject'), findsNothing);
          expect(find.text('unrelated'), findsOneWidget);
          expect(tester.takeException(), isNull);
        } finally {
          await _close(tester, h);
        }
      },
    );
  }

  testWidgets('move with no other groups stays nonmutating', (tester) async {
    final entries = _Entries(withDestination: false);
    final h = await _mount(tester, entries);
    try {
      await _open(tester, 'Move entry');
      expect(find.text('No other groups'), findsOneWidget);
      expect(
        tester
            .widget<FilledButton>(find.widgetWithText(FilledButton, 'Move'))
            .onPressed,
        isNull,
      );
      await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
      await tester.pumpAndSettle();
      expect(entries.moves, isEmpty);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  for (final action in ['Move entry', 'Permanently delete']) {
    testWidgets('$action is disabled while sync status is unknown', (
      tester,
    ) async {
      final entries = _Entries();
      final pending = _PendingSync();
      final h = await _mount(tester, entries, pending: pending);
      try {
        expect(_button(tester, action).onPressed, isNull);
        expect(entries.moves, isEmpty);
        expect(entries.purges, isEmpty);
        expect(tester.takeException(), isNull);
      } finally {
        pending.gate.complete();
        await tester.pump();
        await _close(tester, h);
        pending.dispose();
        await tester.pump();
      }
    });

    testWidgets('$action cannot run a captured callback after sync starts', (
      tester,
    ) async {
      final entries = _Entries();
      final h = await _mount(tester, entries);
      try {
        final callback = _button(tester, action).onPressed!;
        h.sync.syncController.add(
          const bridge.SyncEvent.started(automatic: false),
        );
        await tester.pump();
        await tester.pump();
        expect(_button(tester, action).onPressed, isNull);
        callback();
        // The intentionally active sync spinner never settles.
        await tester.pump(const Duration(milliseconds: 400));
        await tester.pump();
        expect(find.byType(AlertDialog), findsNothing);
        expect(entries.moves, isEmpty);
        expect(entries.purges, isEmpty);
        expect(tester.takeException(), isNull);
      } finally {
        await _close(tester, h);
      }
    });

    testWidgets('$action does not commit a choice from before completed sync', (
      tester,
    ) async {
      final entries = _Entries();
      final h = await _mount(tester, entries);
      try {
        await _open(tester, action);
        if (action == 'Move entry') await _chooseWork(tester);
        h.sync.syncController.add(
          const bridge.SyncEvent.started(automatic: false),
        );
        await tester.pump();
        await tester.pump();
        h.sync.syncController.add(
          const bridge.SyncEvent.done(bridge.SyncOutcomeDto.alreadyInSync()),
        );
        await tester.pump();
        await tester.pump();
        await _confirm(tester, action);
        expect(entries.moves, isEmpty);
        expect(entries.purges, isEmpty);
        expect(tester.takeException(), isNull);
      } finally {
        await _close(tester, h);
      }
    });

    testWidgets('$action removes the dialog and cannot mutate after lock', (
      tester,
    ) async {
      final entries = _Entries();
      final h = await _mount(tester, entries);
      try {
        await _open(tester, action);
        h.session.emitLockState(LockEvent.locked);
        await tester.pump();
        await tester.pump();
        expect(find.byType(AlertDialog), findsNothing);
        expect(find.byType(UnlockScreen), findsOneWidget);
        expect(entries.moves, isEmpty);
        expect(entries.purges, isEmpty);
        expect(tester.takeException(), isNull);
      } finally {
        await _close(tester, h);
      }
    });

    testWidgets('$action opens only one dialog for duplicate callbacks', (
      tester,
    ) async {
      final entries = _Entries();
      final h = await _mount(tester, entries);
      try {
        final callback = _button(tester, action).onPressed!;
        callback();
        callback();
        await tester.pumpAndSettle();
        expect(find.byType(AlertDialog), findsOneWidget);
        await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
        await tester.pumpAndSettle();
        expect(find.byType(AlertDialog), findsNothing);
        expect(entries.moves, isEmpty);
        expect(entries.purges, isEmpty);
        expect(tester.takeException(), isNull);
      } finally {
        await _close(tester, h);
      }
    });

    testWidgets('$action failure keeps the entry and hides diagnostics', (
      tester,
    ) async {
      final entries = _Entries()
        ..mutationError = Exception('mutation-secret-marker');
      final h = await _mount(tester, entries);
      try {
        await _open(tester, action);
        if (action == 'Move entry') await _chooseWork(tester);
        await _confirm(tester, action);
        expect(find.text('An error occurred'), findsOneWidget);
        expect(find.textContaining('mutation-secret-marker'), findsNothing);
        expect(find.text('subject'), findsWidgets);
        expect(entries.tree.entries.length, 2);
        expect(_button(tester, action).onPressed, isNotNull);
        expect(tester.takeException(), isNull);
      } finally {
        await _close(tester, h);
      }
    });
  }

  testWidgets(
    'existing repository moves and purges preserve unrelated fixture entry',
    (tester) async {
      final entries = _Entries();
      await entries.moveEntry('entry-1', 'work-uuid');
      expect(
        entries.tree.entries.singleWhere((e) => e.uuid == 'entry-1').groupUuid,
        'work-uuid',
      );
      await entries.purgeEntry('entry-1');
      expect(entries.tree.entries.single.uuid, 'entry-2');
      expect(entries.moves, [('entry-1', 'work-uuid')]);
      expect(entries.purges, ['entry-1']);
    },
  );
}
