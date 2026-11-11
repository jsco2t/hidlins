import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entry_detail.dart';
import 'package:app/src/features/entries/entry_edit.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:app/src/providers/providers.dart';

import '../helpers/feature_test_helpers.dart';

const _detail = EntryDetail(
  uuid: 'entry-1',
  title: 'Detail target',
  username: 'user',
  hasPassword: false,
  url: '',
  notes: '',
  kind: EntryKindDto.credential,
  tags: [],
  customFields: [CustomFieldDto(name: 'token', isProtected: true)],
  attachments: [AttachmentMeta(name: 'file.txt', sizeBytes: 4)],
);

TestHarness _harness() => TestHarness()
  ..session.currentLockState = LockEvent.unlocked
  ..entries.details['entry-1'] = _detail;

Finder _button(String tooltip) =>
    find.byWidgetPredicate((w) => w is IconButton && w.tooltip == tooltip);

void main() {
  testWidgets('refreshing details remain readable but cannot be edited', (
    tester,
  ) async {
    final h = _harness();
    addTearDown(h.dispose);
    await tester.pumpFeatureWithHarness(
      const Scaffold(body: EntryDetailPane(uuid: 'entry-1')),
      h,
    );
    await tester.pumpAndSettle();
    final container = ProviderScope.containerOf(
      tester.element(find.byType(EntryDetailPane)),
    );
    final gate = h.entries.detailCompleter = Completer<EntryDetail>();
    container.invalidate(entryDetailProvider('entry-1'));
    await tester.pump();
    await tester.pump();
    expect(find.text('Detail target'), findsOneWidget);
    expect(tester.widget<IconButton>(_button('Edit')).onPressed, isNull);
    gate.complete(_detail);
    await tester.pumpAndSettle();
    expect(tester.widget<IconButton>(_button('Edit')).onPressed, isNotNull);
  });

  testWidgets('detail mutations stay disabled until sync status is known', (
    tester,
  ) async {
    final h = _harness()..sync.statusCompleter = Completer<SyncStatusDto>();
    addTearDown(h.dispose);
    await tester.pumpFeatureWithHarness(
      const Scaffold(body: EntryDetailPane(uuid: 'entry-1')),
      h,
    );
    await tester.pump();
    await tester.pump();
    expect(tester.widget<IconButton>(_button('Edit')).onPressed, isNull);
    await tester.scrollUntilVisible(
      _button('Remove'),
      150,
      scrollable: find.byType(Scrollable).first,
    );
    expect(tester.widget<IconButton>(_button('Remove')).onPressed, isNull);
    h.sync.statusCompleter!.complete(
      const SyncStatusDto(configured: false, inFlight: false),
    );
    await tester.pumpAndSettle();
    await tester.scrollUntilVisible(
      _button('Edit'),
      -150,
      scrollable: find.byType(Scrollable).first,
    );
    expect(tester.widget<IconButton>(_button('Edit')).onPressed, isNotNull);
  });

  testWidgets('detail has an edit action on compact and expanded layouts', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    for (final size in [const Size(400, 800), const Size(1200, 800)]) {
      tester.view.physicalSize = size;
      final h = _harness();
      addTearDown(h.dispose);
      await tester.pumpFeatureWithHarness(
        const Scaffold(body: EntryDetailPane(uuid: 'entry-1')),
        h,
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byTooltip('Edit'));
      await tester.pumpAndSettle();
      expect(find.byType(EntryEditDialog), findsOneWidget);
      Navigator.of(tester.element(find.byType(EntryEditDialog))).pop();
      await tester.pumpAndSettle();
    }
  });

  testWidgets('attachment selection cannot submit after sync starts', (
    tester,
  ) async {
    final h = _harness();
    addTearDown(h.dispose);
    final picker = Completer<String?>();
    await tester.pumpFeatureWithHarness(
      Scaffold(
        body: EntryDetailPane(uuid: 'entry-1', onPickFile: () => picker.future),
      ),
      h,
    );
    await tester.pumpAndSettle();
    await tester.scrollUntilVisible(
      find.text('Attach file'),
      150,
      scrollable: find.byType(Scrollable).first,
    );
    await tester.tap(find.text('Attach file'));
    h.sync.syncController.add(const bridge.SyncEvent.started(automatic: true));
    await tester.pump();
    picker.complete('/fixture/file.txt');
    await tester.pumpAndSettle();
    expect(h.entries.addAttachmentCalled, isFalse);
    expect(tester.widget<IconButton>(_button('Remove')).onPressed, isNull);
  });

  testWidgets('detach confirmation rechecks sync eligibility', (tester) async {
    final h = _harness();
    addTearDown(h.dispose);
    await tester.pumpFeatureWithHarness(
      const Scaffold(body: EntryDetailPane(uuid: 'entry-1')),
      h,
    );
    await tester.pumpAndSettle();
    await tester.scrollUntilVisible(
      find.byTooltip('Remove'),
      150,
      scrollable: find.byType(Scrollable).first,
    );
    await tester.tap(find.byTooltip('Remove'));
    await tester.pumpAndSettle();
    h.sync.syncController.add(const bridge.SyncEvent.started(automatic: true));
    await tester.pump();
    await tester.tap(find.text('Confirm'));
    await tester.pumpAndSettle();
    expect(h.entries.removeAttachmentCalled, isFalse);
  });

  testWidgets('protected field reveal failures give safe feedback', (
    tester,
  ) async {
    final h = _harness()..secrets.revealError = Exception('secret-canary');
    addTearDown(h.dispose);
    await tester.pumpFeatureWithHarness(
      const Scaffold(body: EntryDetailPane(uuid: 'entry-1')),
      h,
    );
    await tester.pumpAndSettle();
    await tester.scrollUntilVisible(
      find.byTooltip('Reveal'),
      150,
      scrollable: find.byType(Scrollable).first,
    );
    await tester.tap(find.byTooltip('Reveal'));
    await tester.pumpAndSettle();
    expect(find.text('Could not reveal field'), findsOneWidget);
    expect(find.textContaining('secret-canary'), findsNothing);
  });
}
