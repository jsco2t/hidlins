import 'package:app/src/app.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entries_page.dart';
import 'package:app/src/platform/keyfile_access.dart';
import 'package:app/src/platform/platform_result.dart';
import 'package:app/src/platform/vault_import.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import '../fakes/fake_repositories.dart';
import '../helpers/feature_test_helpers.dart';

class _Session extends FakeSessionRepository {
  KeyfileRef? usedKeyfile;

  @override
  Future<VaultTree> unlock(
    String name,
    String masterPassword, {
    KeyfileRef? keyfile,
  }) {
    usedKeyfile = keyfile;
    return super.unlock(name, masterPassword, keyfile: keyfile);
  }
}

const _reference = KeyfileReference(
  reference: 'former-vault-key',
  displayName: 'former-vault.key',
);

Future<void> _mount(
  WidgetTester tester,
  TestHarness h,
  _Session session,
) async {
  h.keyfiles.pickResults.add(const PlatformSuccess(_reference));
  h.keyfiles.resolveResults.add(const PlatformSuccess('/former-vault.key'));
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
        keyfileAccessCapabilityProvider.overrideWithValue(h.keyfiles),
        vaultImportCapabilityProvider.overrideWithValue(h.vaultImport),
        attachmentExportCapabilityProvider.overrideWithValue(
          h.attachmentExport,
        ),
      ],
    ),
  );
  await tester.pumpAndSettle();
  await tester.tap(find.widgetWithText(TextButton, 'Select keyfile'));
  await tester.pumpAndSettle();
  expect(find.text(_reference.displayName), findsOneWidget);
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

Future<void> _unlock(WidgetTester tester, _Session session) async {
  await tester.enterText(find.byType(TextField), 'fixture-password');
  await tester.tap(find.widgetWithText(FilledButton, 'Unlock'));
  await tester.pumpAndSettle();
  expect(session.unlockCalled, isTrue);
  expect(session.lastUnlockName, 'personal');
  expect(find.byType(EntriesPage), findsOneWidget);
}

VaultSummary _vault({required bool keyed, String path = '/old.kdbx'}) =>
    VaultSummary(
      name: 'personal',
      path: path,
      hasKeyfile: keyed,
      hasSync: false,
    );

void main() {
  for (final known in [false, true]) {
    testWidgets(
      'empty registry clears former keyfile before same-name replacement known=$known',
      (tester) async {
        final h = TestHarness();
        final session = _Session()..vaults = [_vault(keyed: known)];
        try {
          await _mount(tester, h, session);
          final container = ProviderScope.containerOf(
            tester.element(find.byType(Scaffold).first),
          );
          session.vaults.clear();
          container.invalidate(vaultListProvider);
          await tester.pumpAndSettle();
          expect(find.text('Create a new vault'), findsOneWidget);
          session.vaults.add(_vault(keyed: false, path: '/replacement.kdbx'));
          container.invalidate(vaultListProvider);
          await tester.pumpAndSettle();
          await _unlock(tester, session);
          expect(
            session.usedKeyfile,
            isNull,
            reason: 'replacement vault cannot inherit a removed vault keyfile',
          );
          expect(h.keyfiles.resolved, isEmpty);
          expect(h.keyfiles.released, isEmpty);
          expect(tester.takeException(), isNull);
        } finally {
          await _close(tester, h, session);
        }
      },
    );
  }

  testWidgets(
    'same registered vault refresh retains explicit optional keyfile',
    (tester) async {
      final h = TestHarness();
      final session = _Session()..vaults = [_vault(keyed: false)];
      try {
        await _mount(tester, h, session);
        final container = ProviderScope.containerOf(
          tester.element(find.byType(Scaffold).first),
        );
        container.invalidate(vaultListProvider);
        await tester.pumpAndSettle();
        expect(find.text(_reference.displayName), findsOneWidget);
        await _unlock(tester, session);
        expect(session.usedKeyfile, KeyfileRef.path('/former-vault.key'));
        expect(h.keyfiles.resolved, [_reference]);
        expect(h.keyfiles.released, [_reference]);
        expect(tester.takeException(), isNull);
      } finally {
        await _close(tester, h, session);
      }
    },
  );

  testWidgets(
    'shipping management remove/import same-name vault drops former keyfile',
    (tester) async {
      final h = TestHarness();
      final session = _Session()..vaults = [_vault(keyed: false)];
      try {
        await _mount(tester, h, session);
        await tester.tap(find.text('Vaults'));
        await tester.pumpAndSettle();
        final tile = find.ancestor(
          of: find.text('personal'),
          matching: find.byType(ListTile),
        );
        await tester.tap(
          find.descendant(
            of: tile,
            matching: find.byWidgetPredicate((w) => w is PopupMenuButton),
          ),
        );
        await tester.pumpAndSettle();
        await tester.tap(find.text('Remove vault'));
        await tester.pumpAndSettle();
        await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
        await tester.pumpAndSettle();
        expect(session.vaults, isEmpty);
        h.vaultImport.results.add(
          const PlatformSuccess(
            ImportedVault(
              sourceReference: '/replacement.kdbx',
              displayName: 'personal',
            ),
          ),
        );
        await tester.tap(find.byTooltip('Import a .kdbx file'));
        await tester.pumpAndSettle();
        expect(session.vaults.single.path, '/replacement.kdbx');
        await tester.tap(find.byTooltip('Back'));
        await tester.pumpAndSettle();
        await _unlock(tester, session);
        expect(
          session.usedKeyfile,
          isNull,
          reason:
              'shipping replacement does not submit the former vault keyfile',
        );
        expect(h.keyfiles.resolved, isEmpty);
        expect(h.keyfiles.released, isEmpty);
        expect(tester.takeException(), isNull);
      } finally {
        await _close(tester, h, session);
      }
    },
  );
}
