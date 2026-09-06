import 'package:material_ui/material_ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/data/failures.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entry_detail.dart';
import 'package:app/src/l10n/app_localizations.dart';
import 'package:app/src/l10n/hidlins_localizations.dart';
import 'package:app/src/platform/platform_result.dart';
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
    TargetPlatform platform = TargetPlatform.macOS,
  }) async {
    final harness = TestHarness();
    harness.entries.details[uuid] = detail;

    await tester.pumpWidget(
      ProviderScope(
        overrides: harness.overrides,
        child: MaterialApp(
          theme: hidlinsLightTheme().copyWith(platform: platform),
          localizationsDelegates: hidlinsLocalizationsDelegates,
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
    testWidgets('desktop attachment exposes an accessible Save as action', (
      tester,
    ) async {
      final harness = await pumpDetail(tester, detail: detailWithAttachments);
      addTearDown(harness.dispose);

      expect(find.byTooltip('Save backup.txt as…'), findsOneWidget);
      expect(find.text('Saved copies are unencrypted.'), findsOneWidget);
    });

    testWidgets('desktop Save as forwards UUID, key, and chosen path', (
      tester,
    ) async {
      final harness = await pumpDetail(tester, detail: detailWithAttachments);
      addTearDown(harness.dispose);
      harness.attachmentExport.results.add(
        const PlatformSuccess('/chosen/backup.txt'),
      );

      await tester.tap(find.byTooltip('Save backup.txt as…'));
      await tester.pumpAndSettle();

      expect(harness.attachmentExport.suggestedNames, ['backup.txt']);
      expect(harness.entries.saveAttachmentCalled, isTrue);
      expect(harness.entries.lastSaveAttachmentUuid, uuid);
      expect(harness.entries.lastSaveAttachmentKey, 'backup.txt');
      expect(harness.entries.lastSaveAttachmentDest, '/chosen/backup.txt');
      expect(
        find.text('Saved an unencrypted copy of backup.txt'),
        findsOneWidget,
      );
    });

    testWidgets('unsafe attachment name is sanitized before native dispatch', (
      tester,
    ) async {
      const unsafeDetail = EntryDetail(
        uuid: uuid,
        title: 'Test Entry',
        username: '',
        hasPassword: false,
        url: '',
        notes: '',
        kind: EntryKindDto.credential,
        tags: [],
        customFields: [],
        attachments: [
          AttachmentMeta(name: '../folder\\secret\n.txt', sizeBytes: 12),
        ],
      );
      final harness = await pumpDetail(tester, detail: unsafeDetail);
      addTearDown(harness.dispose);
      harness.attachmentExport.results.add(const PlatformCanceled());

      await tester.tap(find.byIcon(Icons.save_alt));
      await tester.pumpAndSettle();

      expect(
        harness.attachmentExport.suggestedNames.single,
        '..foldersecret.txt',
      );
      expect(harness.entries.saveAttachmentCalled, isFalse);
    });

    testWidgets('Save as cancellation is a silent no-op', (tester) async {
      final harness = await pumpDetail(tester, detail: detailWithAttachments);
      addTearDown(harness.dispose);
      harness.attachmentExport.results.add(const PlatformCanceled());

      await tester.tap(find.byTooltip('Save backup.txt as…'));
      await tester.pumpAndSettle();

      expect(harness.entries.saveAttachmentCalled, isFalse);
      expect(find.text('Could not save attachment'), findsNothing);
    });

    testWidgets('native and repository export failures are safe', (
      tester,
    ) async {
      final harness = await pumpDetail(tester, detail: detailWithAttachments);
      addTearDown(harness.dispose);
      harness.attachmentExport.results.add(
        const PlatformFailure('native-dialog-error'),
      );

      await tester.tap(find.byTooltip('Save backup.txt as…'));
      await tester.pumpAndSettle();
      expect(find.text('Could not save attachment'), findsOneWidget);
      expect(harness.entries.saveAttachmentCalled, isFalse);

      harness.attachmentExport.results.add(
        const PlatformSuccess('/chosen/backup.txt'),
      );
      harness.entries.saveAttachmentError = const IoFailure('save attachment');
      await tester.tap(find.byTooltip('Save backup.txt as…'));
      await tester.pumpAndSettle();
      expect(find.text('Could not save attachment'), findsOneWidget);
      expect(harness.entries.saveAttachmentCalled, isTrue);
    });

    for (final platform in [TargetPlatform.android, TargetPlatform.iOS]) {
      testWidgets('$platform keeps attachment export hidden', (tester) async {
        final harness = await pumpDetail(
          tester,
          detail: detailWithAttachments,
          platform: platform,
        );
        addTearDown(harness.dispose);

        expect(find.byIcon(Icons.save_alt), findsNothing);
        expect(find.text('Saved copies are unencrypted.'), findsNothing);
        expect(find.byIcon(Icons.close), findsOneWidget);
      });
    }

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
