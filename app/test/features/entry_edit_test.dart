import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entry_edit.dart';
import '../helpers/feature_test_helpers.dart';

void main() {
  group('EntryEditDialog', () {
    testWidgets('create mode shows kind selector and title field', (
      tester,
    ) async {
      final harness = await tester.pumpFeature(
        const EntryEditDialog(groupUuid: 'root'),
      );
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      expect(find.text('New entry'), findsOneWidget);
      expect(find.text('Credential'), findsOneWidget);
      expect(find.text('Secure note'), findsOneWidget);
      expect(find.text('TOTP'), findsOneWidget);
    });

    testWidgets('validates title is required', (tester) async {
      final harness = await tester.pumpFeature(
        const EntryEditDialog(groupUuid: 'root'),
      );
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();

      expect(find.text('Required'), findsOneWidget);
      expect(harness.entries.createEntryCalled, isFalse);
    });

    testWidgets('calls createEntry with form data', (tester) async {
      final harness = await tester.pumpFeature(
        const EntryEditDialog(groupUuid: 'test-group'),
      );
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      final titleField = find.widgetWithText(TextFormField, 'Title');
      await tester.enterText(titleField, 'My Entry');
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();

      expect(harness.entries.createEntryCalled, isTrue);
      expect(harness.entries.lastCreateGroup, 'test-group');
      expect(harness.entries.lastDraft?.title, 'My Entry');
    });

    testWidgets('unsaved changes guard shows dialog', (tester) async {
      final harness = await tester.pumpFeature(
        Scaffold(
          body: Builder(
            builder: (context) => ElevatedButton(
              onPressed: () => Navigator.of(context).push(
                MaterialPageRoute<void>(
                  builder: (_) => const EntryEditDialog(groupUuid: 'root'),
                ),
              ),
              child: const Text('Open'),
            ),
          ),
        ),
      );
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      await tester.tap(find.text('Open'));
      await tester.pumpAndSettle();

      final titleField = find.widgetWithText(TextFormField, 'Title');
      await tester.enterText(titleField, 'Modified');
      await tester.pumpAndSettle();

      // The form is now dirty, back navigation should trigger guard
      expect(find.text('New entry'), findsOneWidget);
    });

    testWidgets('edit mode loads existing detail', (tester) async {
      final detail = EntryDetail(
        uuid: 'existing',
        title: 'Existing Entry',
        username: 'user',
        hasPassword: true,
        url: 'https://example.com',
        notes: 'Some notes',
        kind: EntryKindDto.credential,
        tags: const ['tag1', 'tag2'],
        customFields: const [],
        attachments: const [],
      );

      final harness = await tester.pumpFeature(EntryEditDialog(detail: detail));
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      expect(find.text('Edit entry'), findsOneWidget);
      expect(find.text('Existing Entry'), findsOneWidget);
    });

    testWidgets('edit mode preserves existing custom fields on save', (
      tester,
    ) async {
      final detail = EntryDetail(
        uuid: 'existing',
        title: 'Existing Entry',
        username: 'user',
        hasPassword: true,
        url: 'https://example.com',
        notes: 'Some notes',
        kind: EntryKindDto.credential,
        tags: const [],
        customFields: const [
          CustomFieldDto(name: 'account-id', isProtected: false),
          CustomFieldDto(name: 'api-token', isProtected: true),
        ],
        attachments: const [],
      );
      final harness = TestHarness()
        ..secrets.revealResultsByField.addAll({
          'account-id': 'account-123',
          'api-token': 'token-456',
        });
      addTearDown(harness.dispose);

      await tester.pumpFeature(
        EntryEditDialog(detail: detail),
        testHarness: harness,
      );
      await tester.pumpAndSettle();
      final protectedValueField = tester.widget<EditableText>(
        find.byWidgetPredicate(
          (widget) =>
              widget is EditableText &&
              widget.controller.text == 'token-456' &&
              widget.obscureText,
        ),
      );
      expect(protectedValueField.autocorrect, isFalse);
      expect(protectedValueField.enableSuggestions, isFalse);

      await tester.tap(find.widgetWithText(FilledButton, 'Save'));
      await tester.pumpAndSettle();

      expect(harness.entries.updateEntryCalled, isTrue);
      final customFields = harness.entries.lastEdit?.customFields;
      expect(customFields, hasLength(2));
      expect(customFields?.map((field) => field.name), [
        'account-id',
        'api-token',
      ]);
      expect(customFields?.map((field) => field.protected), [false, true]);
      expect(customFields?.map((field) => field.value), [
        'account-123',
        'token-456',
      ]);
    });

    testWidgets('keeps save disabled until sync status is known', (
      tester,
    ) async {
      final statusCompleter = Completer<SyncStatusDto>();
      final detail = EntryDetail(
        uuid: 'existing',
        title: 'Existing Entry',
        username: '',
        hasPassword: false,
        url: '',
        notes: '',
        kind: EntryKindDto.secureNote,
        tags: const [],
        customFields: const [],
        attachments: const [],
      );
      final harness = TestHarness()
        ..session.currentLockState = LockEvent.unlocked
        ..sync.statusCompleter = statusCompleter;
      addTearDown(harness.dispose);

      await tester.pumpFeature(
        EntryEditDialog(detail: detail),
        testHarness: harness,
      );
      await tester.pump();

      expect(
        tester
            .widget<FilledButton>(find.widgetWithText(FilledButton, 'Save'))
            .onPressed,
        isNull,
      );

      statusCompleter.complete(
        const SyncStatusDto(configured: false, inFlight: false),
      );
      await tester.pump();
      await tester.pump();
      expect(
        tester
            .widget<FilledButton>(find.widgetWithText(FilledButton, 'Save'))
            .onPressed,
        isNotNull,
      );
    });

    testWidgets('generator use fills the edit password field', (tester) async {
      final detail = EntryDetail(
        uuid: 'existing',
        title: 'Existing Entry',
        username: 'user',
        hasPassword: true,
        url: '',
        notes: '',
        kind: EntryKindDto.credential,
        tags: const [],
        customFields: const [],
        attachments: const [],
      );
      final harness = TestHarness();
      addTearDown(harness.dispose);

      await tester.pumpFeature(
        EntryEditDialog(detail: detail),
        testHarness: harness,
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byTooltip('Generate'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Use'));
      await tester.pumpAndSettle();

      final passwordField = tester.widget<TextFormField>(
        find.widgetWithText(TextFormField, 'Password'),
      );
      expect(passwordField.controller?.text, 'xK9#mP2!qL');
    });

    testWidgets('stops loading custom fields after the editor is disposed', (
      tester,
    ) async {
      final revealCompleter = Completer<String>();
      final detail = EntryDetail(
        uuid: 'existing',
        title: 'Existing Entry',
        username: '',
        hasPassword: false,
        url: '',
        notes: '',
        kind: EntryKindDto.secureNote,
        tags: const [],
        customFields: const [
          CustomFieldDto(name: 'first-secret', isProtected: true),
          CustomFieldDto(name: 'second-secret', isProtected: true),
        ],
        attachments: const [],
      );
      final harness = TestHarness()..secrets.revealCompleter = revealCompleter;
      addTearDown(harness.dispose);

      await tester.pumpFeature(
        EntryEditDialog(detail: detail),
        testHarness: harness,
      );
      await tester.pump();
      unawaited(
        Navigator.of(
          tester.element(find.byType(EntryEditDialog)),
        ).pushReplacement(MaterialPageRoute<void>(builder: (_) => Container())),
      );
      await tester.pumpAndSettle();
      revealCompleter.complete('revealed-after-dispose');
      await tester.pump();
      await tester.pump();

      expect(harness.secrets.revealCallCount, 1);
    });

    testWidgets('edit mode blocks save when custom fields cannot be loaded', (
      tester,
    ) async {
      final detail = EntryDetail(
        uuid: 'existing',
        title: 'Existing Entry',
        username: '',
        hasPassword: false,
        url: '',
        notes: '',
        kind: EntryKindDto.secureNote,
        tags: const [],
        customFields: const [
          CustomFieldDto(name: 'protected', isProtected: true),
        ],
        attachments: const [],
      );
      final harness = TestHarness()
        ..secrets.revealError = StateError('reveal failed');
      addTearDown(harness.dispose);

      await tester.pumpFeature(
        EntryEditDialog(detail: detail),
        testHarness: harness,
      );
      await tester.pumpAndSettle();

      expect(find.text('Could not reveal field'), findsOneWidget);
      expect(
        tester
            .widget<FilledButton>(find.widgetWithText(FilledButton, 'Save'))
            .onPressed,
        isNull,
      );
      expect(harness.entries.updateEntryCalled, isFalse);
    });

    testWidgets('an open editor stays blocked after sync changes the vault', (
      tester,
    ) async {
      final detail = EntryDetail(
        uuid: 'existing',
        title: 'Existing Entry',
        username: '',
        hasPassword: false,
        url: '',
        notes: '',
        kind: EntryKindDto.secureNote,
        tags: const [],
        customFields: const [],
        attachments: const [],
      );
      final harness = TestHarness()
        ..session.currentLockState = LockEvent.unlocked
        ..sync.status = const SyncStatusDto(configured: true, inFlight: false);
      addTearDown(harness.dispose);

      await tester.pumpFeature(
        EntryEditDialog(detail: detail),
        testHarness: harness,
      );
      await tester.pumpAndSettle();

      harness.sync.syncController.add(const SyncEvent.started());
      await tester.pump();
      await tester.pump();
      expect(
        tester
            .widget<FilledButton>(find.widgetWithText(FilledButton, 'Save'))
            .onPressed,
        isNull,
      );

      harness.sync.syncController.add(
        const SyncEvent.done(bridge.SyncOutcomeDto.fastReplaced()),
      );
      await tester.pump();
      await tester.pump();

      expect(
        find.text(
          'The vault changed during sync. Close and reopen this editor before saving.',
        ),
        findsOneWidget,
      );
      expect(
        tester
            .widget<FilledButton>(find.widgetWithText(FilledButton, 'Save'))
            .onPressed,
        isNull,
      );
    });
  });
}
