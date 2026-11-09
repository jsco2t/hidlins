import 'package:app/src/app.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entries_page.dart';
import 'package:app/src/platform/keyfile_access.dart';
import 'package:app/src/platform/platform_result.dart';
import 'package:app/src/platform/vault_import.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import '../fakes/fake_platform_capabilities.dart';
import '../fakes/fake_repositories.dart';
import '../helpers/feature_test_helpers.dart';

const _reference = KeyfileReference(
  reference: 'fixture-reference',
  displayName: 'fixture.key',
);

class _Session extends FakeSessionRepository {
  KeyfileRef? suppliedKeyfile;
  @override
  Future<VaultTree> unlock(
    String name,
    String masterPassword, {
    KeyfileRef? keyfile,
  }) {
    suppliedKeyfile = keyfile;
    return super.unlock(name, masterPassword, keyfile: keyfile);
  }
}

Future<void> _mount(
  WidgetTester tester,
  TestHarness h,
  _Session session,
  FakeKeyfileAccessCapability keys,
) async {
  final imports = FakeVaultImportCapability(const [
    PlatformSuccess(
      ImportedVault(sourceReference: '/fixture.kdbx', displayName: 'imported'),
    ),
  ]);
  await tester.pumpWidget(
    HidlinsApp(
      overrides: [
        sessionRepositoryProvider.overrideWithValue(session),
        entryRepositoryProvider.overrideWithValue(h.entries),
        secretsRepositoryProvider.overrideWithValue(h.secrets),
        searchRepositoryProvider.overrideWithValue(h.search),
        totpRepositoryProvider.overrideWithValue(h.totp),
        generatorRepositoryProvider.overrideWithValue(h.generator),
        syncRepositoryProvider.overrideWithValue(h.sync),
        prefsRepositoryProvider.overrideWithValue(h.prefs),
        keyfileAccessCapabilityProvider.overrideWithValue(keys),
        vaultImportCapabilityProvider.overrideWithValue(imports),
        attachmentExportCapabilityProvider.overrideWithValue(
          h.attachmentExport,
        ),
      ],
    ),
  );
  await tester.pumpAndSettle();
}

Future<void> _close(
  WidgetTester tester,
  TestHarness h,
  _Session session,
) async {
  await tester.pumpWidget(const SizedBox.shrink());
  await tester.pump();
  session.dispose();
  h.dispose();
  await tester.pump();
}

void main() {
  testWidgets(
    'shipping first-run import offers keyfile despite false registry metadata',
    (tester) async {
      final h = TestHarness();
      final session = _Session();
      final keys = FakeKeyfileAccessCapability(
        pickResults: const [PlatformSuccess(_reference)],
        resolveResults: const [PlatformSuccess('/fixture.key')],
        releaseResults: const [PlatformSuccess(platformUnit)],
      );
      try {
        await _mount(tester, h, session, keys);
        await tester.tap(find.text('Import a .kdbx file'));
        await tester.pumpAndSettle();
        expect(session.registerExistingVaultCalled, isTrue);
        expect(session.vaults.single.hasKeyfile, isFalse);
        final select = find.widgetWithText(TextButton, 'Select keyfile');
        expect(select, findsOneWidget);
        await tester.tap(select);
        await tester.pumpAndSettle();
        await tester.enterText(
          find.byType(TextField),
          'keyfile-fixture-marker',
        );
        await tester.tap(find.text('Unlock'));
        await tester.pumpAndSettle();
        expect(session.suppliedKeyfile, const KeyfileRef.path('/fixture.key'));
        expect(keys.resolved, [_reference]);
        expect(keys.released, [_reference]);
        expect(find.byType(EntriesPage), findsOneWidget);
      } finally {
        await _close(tester, h, session);
      }
    },
  );

  testWidgets(
    'unknown keyfile metadata does not prevent recovery from keyfile permission failure',
    (tester) async {
      final h = TestHarness();
      final session = _Session()
        ..vaults = const [
          VaultSummary(
            name: 'imported',
            path: '/fixture.kdbx',
            hasKeyfile: false,
            hasSync: false,
          ),
        ];
      final keys = FakeKeyfileAccessCapability(
        pickResults: const [PlatformSuccess(_reference)],
        resolveResults: const [PlatformDenied()],
        releaseResults: const [PlatformSuccess(platformUnit)],
      );
      try {
        await _mount(tester, h, session, keys);
        final select = find.widgetWithText(TextButton, 'Select keyfile');
        expect(select, findsOneWidget);
        await tester.tap(select);
        await tester.pumpAndSettle();
        await tester.enterText(
          find.byType(TextField),
          'keyfile-fixture-marker',
        );
        await tester.tap(find.text('Unlock'));
        await tester.pumpAndSettle();
        expect(session.unlockCalled, isFalse);
        expect(
          tester.widget<TextField>(find.byType(TextField)).controller!.text,
          isEmpty,
        );
        expect(keys.released, [_reference]);
        expect(
          tester
              .widget<FilledButton>(find.widgetWithText(FilledButton, 'Unlock'))
              .onPressed,
          isNotNull,
        );
      } finally {
        await _close(tester, h, session);
      }
    },
  );

  for (final hasKeyfile in [false, true]) {
    testWidgets(
      'existing ${hasKeyfile ? 'keyed' : 'password-only'} vault retains unlock behavior',
      (tester) async {
        final h = TestHarness();
        final session = _Session()
          ..vaults = [
            VaultSummary(
              name: 'existing',
              path: '/fixture.kdbx',
              hasKeyfile: hasKeyfile,
              hasSync: false,
            ),
          ];
        final keys = FakeKeyfileAccessCapability(
          pickResults: const [PlatformSuccess(_reference)],
          resolveResults: const [PlatformSuccess('/fixture.key')],
          releaseResults: const [PlatformSuccess(platformUnit)],
        );
        try {
          await _mount(tester, h, session, keys);
          if (hasKeyfile) {
            await tester.tap(find.widgetWithText(TextButton, 'Select keyfile'));
            await tester.pumpAndSettle();
          }
          await tester.enterText(
            find.byType(TextField),
            'keyfile-fixture-marker',
          );
          await tester.tap(find.text('Unlock'));
          await tester.pumpAndSettle();
          expect(
            session.suppliedKeyfile,
            hasKeyfile ? const KeyfileRef.path('/fixture.key') : isNull,
          );
          expect(find.byType(EntriesPage), findsOneWidget);
        } finally {
          await _close(tester, h, session);
        }
      },
    );
  }
}
