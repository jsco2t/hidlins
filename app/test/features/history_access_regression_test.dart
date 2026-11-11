import 'dart:async';

import 'package:app/src/app.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entry_detail.dart';
import 'package:app/src/features/entries/entry_history.dart';
import 'package:app/src/features/lock/unlock_screen.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import '../fakes/fake_repositories.dart';
import '../helpers/feature_test_helpers.dart';

const _history = [
  HistorySummary(
    title: 'newer prior title',
    username: 'newer prior user',
    lastModificationTime: 1704240000,
  ),
  HistorySummary(
    title: 'older prior title',
    username: 'older prior user',
    lastModificationTime: 1704153600,
  ),
];

class _Entries extends FakeEntryRepository {
  Object? error;
  Completer<List<HistorySummary>>? pending;
  final List<String> historyReads = [];

  @override
  Future<List<HistorySummary>> entryHistory(String uuid) async {
    historyReads.add(uuid);
    if (pending case final wait?) return wait.future;
    if (error case final failure?) throw failure;
    return super.entryHistory(uuid);
  }
}

Future<TestHarness> _mount(
  WidgetTester tester,
  _Entries entries,
  double width,
) async {
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
  await tester.tap(find.text('GitHub').first);
  await tester.pumpAndSettle();
  return h;
}

Future<void> _open(WidgetTester tester) async {
  final action = find.text('Entry history');
  expect(
    action,
    findsOneWidget,
    reason: 'shipping detail offers entry history',
  );
  await tester.ensureVisible(action);
  await tester.tap(action);
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 350));
  expect(find.byType(EntryHistorySheet), findsOneWidget);
}

Future<void> _close(WidgetTester tester, TestHarness h) async {
  await tester.pumpWidget(const SizedBox.shrink());
  await tester.pump();
  h.dispose();
  await tester.pump();
}

void main() {
  testWidgets(
    'existing history presentation clears metadata on first locked frame',
    (tester) async {
      final h = TestHarness()..session.currentLockState = LockEvent.unlocked;
      h.entries.histories['entry-1'] = _history;
      try {
        await tester.pumpFeatureWithHarness(
          Builder(
            builder: (context) => Scaffold(
              body: TextButton(
                onPressed: () => EntryHistorySheet.show(context, 'entry-1'),
                child: const Text('Open existing history'),
              ),
            ),
          ),
          h,
        );
        await tester.pumpAndSettle();
        await tester.tap(find.text('Open existing history'));
        await tester.pumpAndSettle();
        expect(find.text('newer prior title'), findsOneWidget);
        h.session.emitLockState(LockEvent.locked);
        await tester.pump();
        await tester.pump();
        expect(
          find.text('newer prior title'),
          findsNothing,
          reason: 'no history metadata remains visible while a lock transition is rendering',
        );
        expect(find.byType(EntryHistorySheet), findsNothing);
        expect(tester.takeException(), isNull);
      } finally {
        await _close(tester, h);
      }
    },
  );

  testWidgets('history detail timestamps fit a narrow pane without overflow', (
    tester,
  ) async {
    final h = TestHarness();
    h.entries.details['entry-1'] = const EntryDetail(
      uuid: 'entry-1',
      title: 'History fixture',
      username: '',
      hasPassword: false,
      url: '',
      notes: '',
      kind: EntryKindDto.secureNote,
      tags: [],
      customFields: [],
      attachments: [],
      creationTime: 1700000000,
      lastModificationTime: 1700100000,
    );
    try {
      await tester.pumpFeatureWithHarness(
        const Scaffold(
          body: Center(
            child: SizedBox(
              width: 320,
              child: EntryDetailPane(uuid: 'entry-1'),
            ),
          ),
        ),
        h,
      );
      await tester.pumpAndSettle();
      expect(
        tester.takeException(),
        isNull,
        reason: 'real timestamp labels and values fit the narrow detail pane',
      );
      expect(find.text('Created'), findsOneWidget);
      expect(find.text('Modified'), findsOneWidget);
    } finally {
      await _close(tester, h);
    }
  });

  for (final width in [390.0, 1280.0]) {
    testWidgets(
      'shipping history shows API order in ${width == 390 ? 'bottom sheet' : 'side panel'}',
      (tester) async {
        final entries = _Entries()..histories['entry-1'] = _history;
        final h = await _mount(tester, entries, width);
        try {
          await _open(tester);
          expect(entries.historyReads, ['entry-1']);
          expect(find.text('newer prior title'), findsOneWidget);
          expect(find.text('older prior user'), findsOneWidget);
          expect(
            tester.getTopLeft(find.text('newer prior title')).dy,
            lessThan(tester.getTopLeft(find.text('older prior title')).dy),
          );
          expect(find.text('v2'), findsOneWidget);
          expect(find.text('v1'), findsOneWidget);
          if (width == 390) {
            expect(find.byType(BottomSheet), findsOneWidget);
            expect(find.byType(Dialog), findsNothing);
          } else {
            expect(find.byType(Dialog), findsOneWidget);
            expect(find.byType(BottomSheet), findsNothing);
            final bounds = tester.getRect(find.byType(EntryHistorySheet));
            expect(bounds.right, closeTo(width, 1));
            expect(bounds.width, lessThan(width / 2));
          }
          await tester.tap(find.byTooltip('Close'));
          await tester.pumpAndSettle();
          expect(find.byType(EntryHistorySheet), findsNothing);
          expect(tester.takeException(), isNull);
        } finally {
          await _close(tester, h);
        }
      },
    );

    testWidgets(
      'lock removes pending history and ignores late read at $width',
      (tester) async {
        final pending = Completer<List<HistorySummary>>();
        final entries = _Entries()..pending = pending;
        final h = await _mount(tester, entries, width);
        try {
          await _open(tester);
          expect(
            find.descendant(
              of: find.byType(EntryHistorySheet),
              matching: find.byType(CircularProgressIndicator),
            ),
            findsOneWidget,
          );
          h.session.emitLockState(LockEvent.locked);
          await tester.pump();
          await tester.pump(const Duration(milliseconds: 350));
          expect(find.byType(UnlockScreen), findsOneWidget);
          expect(find.byType(EntryHistorySheet), findsNothing);
          pending.complete(_history);
          await tester.pump();
          await tester.pump(const Duration(milliseconds: 350));
          expect(find.text('newer prior title'), findsNothing);
          expect(tester.takeException(), isNull);
        } finally {
          if (!pending.isCompleted) pending.complete([]);
          await _close(tester, h);
        }
      },
    );
  }

  testWidgets(
    'history failure is distinct from empty and retry reloads same entry',
    (tester) async {
      final entries = _Entries()
        ..error = Exception('history-diagnostic-marker');
      final h = await _mount(tester, entries, 390);
      try {
        await _open(tester);
        expect(find.text('An error occurred'), findsOneWidget);
        expect(find.text('No history yet'), findsNothing);
        expect(find.textContaining('history-diagnostic-marker'), findsNothing);
        entries.error = null;
        entries.histories['entry-1'] = _history;
        await tester.tap(find.widgetWithText(TextButton, 'Retry'));
        await tester.pumpAndSettle();
        expect(entries.historyReads, ['entry-1', 'entry-1']);
        expect(find.text('newer prior title'), findsOneWidget);
        expect(find.text('An error occurred'), findsNothing);
        expect(tester.takeException(), isNull);
      } finally {
        await _close(tester, h);
      }
    },
  );

  testWidgets('empty history keeps heading and close action', (tester) async {
    final h = await _mount(tester, _Entries(), 390);
    try {
      await _open(tester);
      expect(find.text('No history yet'), findsOneWidget);
      expect(
        find.descendant(
          of: find.byType(EntryHistorySheet),
          matching: find.text('Entry history'),
        ),
        findsOneWidget,
      );
      await tester.tap(find.byTooltip('Close'));
      await tester.pumpAndSettle();
      expect(find.byType(EntryHistorySheet), findsNothing);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets(
    'compact history drag expands before scrolling through long history',
    (tester) async {
      final entries = _Entries()
        ..histories['entry-1'] = [
          for (var i = 0; i < 30; i++)
            HistorySummary(title: 'historical-$i', username: 'user-$i'),
        ];
      final h = await _mount(tester, entries, 390);
      try {
        await _open(tester);
        final before = tester.getSize(find.byType(EntryHistorySheet)).height;
        final scrollable = find.descendant(
          of: find.byType(EntryHistorySheet),
          matching: find.byType(Scrollable),
        );
        await tester.drag(scrollable, const Offset(0, -350));
        await tester.pumpAndSettle();
        expect(
          tester.getSize(find.byType(EntryHistorySheet)).height,
          greaterThan(before),
        );
        await tester.scrollUntilVisible(
          find.text('historical-29'),
          300,
          scrollable: scrollable,
        );
        expect(find.text('historical-29'), findsOneWidget);
        expect(tester.takeException(), isNull);
      } finally {
        await _close(tester, h);
      }
    },
  );

  testWidgets('existing nonempty history summaries render directly in order', (
    tester,
  ) async {
    final h = TestHarness()..entries.histories['entry-1'] = _history;
    try {
      await tester.pumpFeatureWithHarness(
        const Scaffold(body: EntryHistorySheet(uuid: 'entry-1')),
        h,
      );
      await tester.pumpAndSettle();
      expect(find.text('Entry history'), findsOneWidget);
      expect(find.text('newer prior title'), findsOneWidget);
      expect(find.text('older prior user'), findsOneWidget);
      expect(
        tester.getTopLeft(find.text('newer prior title')).dy,
        lessThan(tester.getTopLeft(find.text('older prior title')).dy),
      );
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });
}
