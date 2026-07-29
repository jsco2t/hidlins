import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

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
  });
}
