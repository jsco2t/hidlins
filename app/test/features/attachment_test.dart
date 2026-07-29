import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/data/failures.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entry_detail.dart';
import 'package:app/src/l10n/app_localizations.dart';
import 'package:app/src/ui/theme.dart';
import '../helpers/feature_test_helpers.dart';

void main() {
  const uuid = 'test-entry';

  const detailWithAttachments = EntryDetail(
    uuid: uuid,
    title: 'Test Entry',
    username: 'user',
    hasPassword: false,
    url: '',
    notes: '',
    kind: EntryKindDto.credential,
    tags: [],
    customFields: [],
    attachments: [AttachmentMeta(name: 'backup.txt', sizeBytes: 128)],
  );

  const detailNoAttachments = EntryDetail(
    uuid: uuid,
    title: 'Test Entry',
    username: 'user',
    hasPassword: false,
    url: '',
    notes: '',
    kind: EntryKindDto.credential,
    tags: [],
    customFields: [],
    attachments: [],
  );

  Future<TestHarness> pumpDetail(
    WidgetTester tester, {
    required EntryDetail detail,
    FilePickerCallback? onPickFile,
  }) async {
    final harness = TestHarness();
    harness.entries.details[uuid] = detail;

    await tester.pumpWidget(
      ProviderScope(
        overrides: harness.overrides,
        child: MaterialApp(
          theme: hidlinsLightTheme(),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          home: Scaffold(
            body: EntryDetailPane(uuid: uuid, onPickFile: onPickFile),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    return harness;
  }

  group('Attachment attach/detach', () {
    testWidgets('attach passes source path to repo', (tester) async {
      final harness = await pumpDetail(
        tester,
        detail: detailNoAttachments,
        onPickFile: () async => '/path/to/document.pdf',
      );
      addTearDown(harness.dispose);

      await tester.tap(find.text('Attach file'));
      await tester.pumpAndSettle();

      expect(harness.entries.addAttachmentCalled, isTrue);
      expect(harness.entries.lastAttachmentPath, '/path/to/document.pdf');
    });

    testWidgets('attach shows error on size cap exceeded', (tester) async {
      final harness = await pumpDetail(
        tester,
        detail: detailNoAttachments,
        onPickFile: () async => '/path/to/huge-file.bin',
      );
      addTearDown(harness.dispose);

      harness.entries.attachmentError = const InvalidInputFailure(
        field: 'attachment',
        reason: '10 bytes exceeds limit of 5 bytes',
      );

      await tester.tap(find.text('Attach file'));
      await tester.pumpAndSettle();

      expect(find.text('File too large'), findsOneWidget);
    });

    testWidgets('attach cancelled by picker does not call repo', (
      tester,
    ) async {
      final harness = await pumpDetail(
        tester,
        detail: detailNoAttachments,
        onPickFile: () async => null,
      );
      addTearDown(harness.dispose);

      await tester.tap(find.text('Attach file'));
      await tester.pumpAndSettle();

      expect(harness.entries.addAttachmentCalled, isFalse);
    });

    testWidgets('detach requires confirmation dialog', (tester) async {
      final harness = await pumpDetail(tester, detail: detailWithAttachments);
      addTearDown(harness.dispose);

      expect(find.text('backup.txt'), findsOneWidget);

      await tester.tap(find.byIcon(Icons.close));
      await tester.pumpAndSettle();

      expect(find.text('Remove this attachment?'), findsOneWidget);

      await tester.tap(find.text('Cancel'));
      await tester.pumpAndSettle();

      expect(harness.entries.removeAttachmentCalled, isFalse);
    });

    testWidgets('detach confirmed calls repo with attachment key', (
      tester,
    ) async {
      final harness = await pumpDetail(tester, detail: detailWithAttachments);
      addTearDown(harness.dispose);

      await tester.tap(find.byIcon(Icons.close));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Confirm'));
      await tester.pumpAndSettle();

      expect(harness.entries.removeAttachmentCalled, isTrue);
      expect(harness.entries.lastRemovedAttachmentKey, 'backup.txt');
    });
  });
}
