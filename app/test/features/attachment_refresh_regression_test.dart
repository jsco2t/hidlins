import 'package:app/src/data/failures.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entry_detail.dart';
import 'package:app/src/l10n/app_localizations.dart';
import 'package:app/src/l10n/hidlins_localizations.dart';
import 'package:app/src/providers/providers.dart';
import 'package:app/src/ui/theme.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import '../fakes/fake_repositories.dart';
import '../helpers/feature_test_helpers.dart';

EntryDetail _detail(String uuid, Iterable<String> attachments) => EntryDetail(
  uuid: uuid,
  title: 'Attachment target $uuid',
  username: '',
  hasPassword: false,
  url: '',
  notes: '',
  kind: EntryKindDto.credential,
  tags: const [],
  customFields: const [],
  attachments: [
    for (final name in attachments) AttachmentMeta(name: name, sizeBytes: 4),
  ],
);

class _Repository extends FakeEntryRepository {
  _Repository() {
    details['entry'] = _detail('entry', ['old.txt']);
    details['other'] = _detail('other', ['other.txt']);
  }

  @override
  Future<void> addAttachment(String uuid, String sourcePath) async {
    if (attachmentError case final failure?) throw failure;
    details[uuid] = _detail(uuid, [
      ...details[uuid]!.attachments.map((a) => a.name),
      'added.txt',
    ]);
    await super.addAttachment(uuid, sourcePath);
  }

  @override
  Future<void> removeAttachment(String uuid, String key) async {
    details[uuid] = _detail(uuid, [
      for (final attachment in details[uuid]!.attachments)
        if (attachment.name != key) attachment.name,
    ]);
    await super.removeAttachment(uuid, key);
  }
}

Future<ProviderContainer> _mount(
  WidgetTester tester,
  _Repository repo, {
  bool watchTree = false,
}) async {
  final h = TestHarness()..session.currentLockState = LockEvent.unlocked;
  addTearDown(h.dispose);
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        entryRepositoryProvider.overrideWithValue(repo),
        sessionRepositoryProvider.overrideWithValue(h.session),
        secretsRepositoryProvider.overrideWithValue(h.secrets),
        syncRepositoryProvider.overrideWithValue(h.sync),
        prefsRepositoryProvider.overrideWithValue(h.prefs),
      ],
      child: MaterialApp(
        theme: hidlinsLightTheme(),
        localizationsDelegates: hidlinsLocalizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: Consumer(
          builder: (_, ref, _) {
            if (watchTree) ref.watch(vaultTreeProvider);
            return Scaffold(
              body: EntryDetailPane(
                uuid: 'entry',
                onPickFile: () async => '/fixture/added.txt',
              ),
            );
          },
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
  return ProviderScope.containerOf(
    tester.element(find.byType(EntryDetailPane)),
  );
}

void main() {
  testWidgets('detail-only Attach action publishes the added attachment', (
    tester,
  ) async {
    final repo = _Repository();
    await _mount(tester, repo);
    expect(find.text('old.txt'), findsOneWidget);
    await tester.tap(find.text('Attach file'));
    await tester.pumpAndSettle();
    expect(repo.addAttachmentCalled, isTrue);
    expect(
      repo.details['entry']!.attachments.map((a) => a.name),
      contains('added.txt'),
    );
    expect(find.text('added.txt'), findsOneWidget);
    expect(find.text('old.txt'), findsOneWidget);
  });

  testWidgets('Remove confirmation refreshes a detail alongside the tree', (
    tester,
  ) async {
    final repo = _Repository();
    final c = await _mount(tester, repo, watchTree: true);
    await tester.tap(find.byTooltip('Remove'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Confirm'));
    await tester.pumpAndSettle();
    expect(repo.removeAttachmentCalled, isTrue);
    expect(repo.details['entry']!.attachments, isEmpty);
    expect(find.text('old.txt'), findsNothing);
    expect(
      (await c.read(entryDetailProvider('entry').future)).attachments,
      isEmpty,
    );
  });

  test(
    'local mutation invalidates previously cached detail providers',
    () async {
      final repo = _Repository();
      final sync = FakeSyncRepository();
      addTearDown(sync.dispose);
      final c = ProviderContainer(
        overrides: [
          entryRepositoryProvider.overrideWithValue(repo),
          syncRepositoryProvider.overrideWithValue(sync),
        ],
      );
      addTearDown(c.dispose);
      final before = await c.read(entryDetailProvider('entry').future);
      await c.read(entryDetailProvider('other').future);
      await repo.addAttachment('entry', '/fixture/added.txt');
      final after = await c.read(entryDetailProvider('entry').future);
      expect(after.attachments.map((a) => a.name), ['old.txt', 'added.txt']);
      expect(after, isNot(same(before)));
      expect(
        (await c.read(entryDetailProvider('other').future))
            .attachments
            .single
            .name,
        'other.txt',
      );
    },
  );

  testWidgets('failed Attach keeps existing details and safe error feedback', (
    tester,
  ) async {
    final repo = _Repository()
      ..attachmentError = const InvalidInputFailure(
        field: 'attachment',
        reason: 'private-path-marker',
      );
    await _mount(tester, repo, watchTree: true);
    await tester.tap(find.text('Attach file'));
    await tester.pumpAndSettle();
    expect(find.text('old.txt'), findsOneWidget);
    expect(find.text('added.txt'), findsNothing);
    expect(find.text('File too large'), findsOneWidget);
    expect(find.textContaining('private-path-marker'), findsNothing);
  });
}
