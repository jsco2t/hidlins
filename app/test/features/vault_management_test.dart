import 'dart:async';

import 'package:material_ui/material_ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/data/failures.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/vaults/change_password_dialog.dart';
import 'package:app/src/features/vaults/connect_sync_dialog.dart';
import 'package:app/src/features/vaults/create_vault_dialog.dart';
import 'package:app/src/features/vaults/deregister_vault_dialog.dart';
import 'package:app/src/features/vaults/vault_management_page.dart';
import 'package:app/src/l10n/app_localizations.dart';
import 'package:app/src/l10n/hidlins_localizations.dart';
import 'package:app/src/platform/keyfile_access.dart';
import 'package:app/src/platform/platform_result.dart';
import 'package:app/src/platform/vault_import.dart';

import '../helpers/feature_test_helpers.dart';
import '../fakes/fake_repositories.dart';

import 'package:app/src/providers/providers.dart';
import 'package:app/src/ui/theme.dart';

void main() {
  for (final disposeDuringResolve in [false, true]) {
    testWidgets(
      'create clears before keyfile resolution; dispose=$disposeDuringResolve',
      (tester) async {
        final harness = TestHarness();
        final keys = DelayedKeyfileAccess();
        final createGate = harness.session.createVaultCompleter =
            Completer<void>();
        addTearDown(harness.dispose);
        addTearDown(() {
          if (!keys.resolveGate.isCompleted) {
            keys.resolveGate.complete(const PlatformSuccess('/keyfile'));
          }
          if (!createGate.isCompleted) createGate.complete();
        });
        await tester.pumpWidget(
          ProviderScope(
            overrides: [
              sessionRepositoryProvider.overrideWithValue(harness.session),
              keyfileAccessCapabilityProvider.overrideWithValue(keys),
            ],
            child: MaterialApp(
              theme: hidlinsLightTheme(),
              localizationsDelegates: hidlinsLocalizationsDelegates,
              supportedLocales: AppLocalizations.supportedLocales,
              home: const CreateVaultDialog(),
            ),
          ),
        );
        await tester.pumpAndSettle();
        await tester.tap(find.text('Select keyfile'));
        await tester.pumpAndSettle();
        await tester.enterText(
          find.widgetWithText(TextFormField, 'Vault name'),
          'new-vault',
        );
        await tester.enterText(
          find.widgetWithText(TextFormField, 'Master password'),
          'create-canary',
        );
        await tester.enterText(
          find.widgetWithText(TextFormField, 'Confirm password'),
          'create-canary',
        );
        final password = tester
            .widget<TextFormField>(
              find.widgetWithText(TextFormField, 'Master password'),
            )
            .controller!;
        final confirmation = tester
            .widget<TextFormField>(
              find.widgetWithText(TextFormField, 'Confirm password'),
            )
            .controller!;
        final consent = find.widgetWithText(
          TextFormField,
          'Type CONFIRM to acknowledge',
        );
        await tester.ensureVisible(consent);
        await tester.enterText(consent, 'CONFIRM');
        final create = find.widgetWithText(FilledButton, 'Create');
        await tester.ensureVisible(create);
        await tester.tap(create);
        await tester.pump();
        expect(password.text, isEmpty);
        expect(confirmation.text, isEmpty);
        if (disposeDuringResolve) {
          await tester.pumpWidget(const SizedBox.shrink());
        }
        keys.resolveGate.complete(const PlatformSuccess('/keyfile'));
        await tester.pump();
        if (!disposeDuringResolve) {
          expect(harness.session.lastCreatePassword, 'create-canary');
          harness.session.createVaultError = const PathAlreadyExists(
            '/vaults/new-vault.kdbx',
          );
        }
        createGate.complete();
        await tester.pumpAndSettle();
        expect(keys.releaseCalls, 1);
        expect(tester.takeException(), isNull);
        if (disposeDuringResolve) {
          expect(harness.session.createVaultCalled, isFalse);
        } else {
          expect(password.text, isEmpty);
          expect(confirmation.text, isEmpty);
          expect(find.textContaining('create-canary'), findsNothing);
          await tester.enterText(
            find.widgetWithText(TextFormField, 'Master password'),
            'retry-canary',
          );
        }
      },
    );
  }

  for (final disposeDuringChange in [false, true]) {
    testWidgets(
      'change clears before delayed failure; dispose=$disposeDuringChange',
      (tester) async {
        final harness = TestHarness();
        final gate = harness.session.changePasswordCompleter =
            Completer<void>();
        harness.session.changePasswordError = const BadCredentials();
        addTearDown(harness.dispose);
        addTearDown(() {
          if (!gate.isCompleted) {
            gate.complete();
          }
        });
        await tester.pumpFeatureWithHarness(
          const ChangePasswordDialog(),
          harness,
        );
        await tester.pumpAndSettle();
        final fields = <String, String>{
          'Current password': 'old-canary',
          'New password': 'new-canary',
          'Confirm new password': 'new-canary',
        };
        final controllers = <TextEditingController>[];
        for (final entry in fields.entries) {
          final field = find.widgetWithText(TextFormField, entry.key);
          await tester.enterText(field, entry.value);
          controllers.add(tester.widget<TextFormField>(field).controller!);
        }
        await tester.tap(find.text('Confirm'));
        await tester.pump();
        expect(controllers.map((c) => c.text), everyElement(isEmpty));
        expect(harness.session.lastCurrentPassword, 'old-canary');
        expect(harness.session.lastNewPassword, 'new-canary');
        if (disposeDuringChange) {
          await tester.pumpWidget(const SizedBox.shrink());
        }
        gate.complete();
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        if (!disposeDuringChange) {
          expect(controllers.map((c) => c.text), everyElement(isEmpty));
          expect(find.text('Wrong password or keyfile'), findsOneWidget);
        }
      },
    );
  }

  testWidgets('local create resolves and releases an optional keyfile', (
    tester,
  ) async {
    final harness = TestHarness();
    addTearDown(harness.dispose);
    const reference = KeyfileReference(
      reference: 'bookmark-1',
      displayName: 'personal.key',
    );
    harness.keyfiles.pickResults.add(const PlatformSuccess(reference));
    harness.keyfiles.resolveResults.add(
      const PlatformSuccess('/provider/personal.key'),
    );
    harness.keyfiles.releaseResults.add(const PlatformSuccess(platformUnit));
    await tester.pumpFeatureWithHarness(const CreateVaultDialog(), harness);
    await tester.pumpAndSettle();

    await tester.tap(find.text('Select keyfile'));
    await tester.pumpAndSettle();
    expect(find.text('personal.key'), findsOneWidget);
    await tester.enterText(
      find.widgetWithText(TextFormField, 'Vault name'),
      'personal',
    );
    await tester.enterText(
      find.widgetWithText(TextFormField, 'Master password'),
      'strong-password',
    );
    await tester.enterText(
      find.widgetWithText(TextFormField, 'Confirm password'),
      'strong-password',
    );
    final noRecoveryConfirmation = find.widgetWithText(
      TextFormField,
      'Type CONFIRM to acknowledge',
    );
    await tester.ensureVisible(noRecoveryConfirmation);
    await tester.pumpAndSettle();
    await tester.enterText(noRecoveryConfirmation, 'CONFIRM');
    await tester.ensureVisible(find.widgetWithText(FilledButton, 'Create'));
    await tester.tap(find.widgetWithText(FilledButton, 'Create'));
    await tester.pumpAndSettle();

    expect(harness.session.createVaultCalled, isTrue);
    expect(harness.keyfiles.resolved, [reference]);
    expect(harness.keyfiles.released, [reference]);
    expect(harness.session.vaults.single.hasKeyfile, isTrue);
  });

  group('DeregisterVaultDialog', () {
    testWidgets('deregister without delete passes deleteFile: false', (
      tester,
    ) async {
      final harness = await tester.pumpFeature(
        const DeregisterVaultDialog(vaultName: 'test-vault'),
      );
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      expect(find.text('Remove vault'), findsOneWidget);
      expect(find.text('Remove this vault from the registry?'), findsOneWidget);

      await tester.tap(find.text('Delete'));
      await tester.pumpAndSettle();

      expect(harness.session.deregisterCalled, isTrue);
      expect(harness.session.lastDeregisterName, 'test-vault');
      expect(harness.session.lastDeregisterDeleteFile, isFalse);
    });

    testWidgets('deregister with delete requires second confirmation', (
      tester,
    ) async {
      final harness = await tester.pumpFeature(
        const DeregisterVaultDialog(vaultName: 'test-vault'),
      );
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      await tester.tap(find.text('Also delete the vault file'));
      await tester.pumpAndSettle();

      final deleteButton = tester.widget<FilledButton>(
        find.widgetWithText(FilledButton, 'Delete'),
      );
      expect(deleteButton.onPressed, isNull);

      expect(
        find.text('Are you sure? This permanently deletes the vault file.'),
        findsOneWidget,
      );

      await tester.tap(find.text('Confirm'));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Delete'));
      await tester.pumpAndSettle();

      expect(harness.session.deregisterCalled, isTrue);
      expect(harness.session.lastDeregisterDeleteFile, isTrue);
    });

    testWidgets('unchecking delete resets second confirmation', (tester) async {
      final harness = await tester.pumpFeature(
        const DeregisterVaultDialog(vaultName: 'test-vault'),
      );
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      await tester.tap(find.text('Also delete the vault file'));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Confirm'));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Also delete the vault file'));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Also delete the vault file'));
      await tester.pumpAndSettle();

      final deleteButton = tester.widget<FilledButton>(
        find.widgetWithText(FilledButton, 'Delete'),
      );
      expect(deleteButton.onPressed, isNull);
    });
  });

  group('ConnectSyncDialog', () {
    Future<void> fillConnectForm(
      WidgetTester tester, {
      String name = 'my-vault',
      String password = 'pass123',
    }) async {
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Vault name'),
        name,
      );
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Master password'),
        password,
      );
    }

    testWidgets('pair-and-import compares SAS and clears password', (
      tester,
    ) async {
      final harness = await tester.pumpFeature(
        const ConnectSyncDialog(mobileOverride: false),
      );
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      expect(find.text('Import paired vault'), findsWidgets);

      await fillConnectForm(tester);

      await tester.scrollUntilVisible(
        find.widgetWithText(FilledButton, 'Import paired vault'),
        100,
        scrollable: find.byType(Scrollable).first,
      );
      await tester.tap(
        find.widgetWithText(FilledButton, 'Import paired vault'),
      );
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 10));

      expect(harness.sync.beginPairImportCalled, isTrue);
      expect(find.text('Compare this code on both devices'), findsOneWidget);
      expect(find.text('123 456'), findsOneWidget);
      expect(find.text('pass123'), findsNothing);
      await tester.tap(find.text('Codes match'));
      await tester.pumpAndSettle();
      expect(find.text('123 456'), findsNothing);
    });

    testWidgets('wrong password shows error', (tester) async {
      final harness = await tester.pumpFeature(
        const ConnectSyncDialog(mobileOverride: false),
      );
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      harness.sync.beginPairImportError = const BadCredentials();

      await fillConnectForm(tester, password: 'wrong');

      await tester.scrollUntilVisible(
        find.widgetWithText(FilledButton, 'Import paired vault'),
        100,
        scrollable: find.byType(Scrollable).first,
      );
      await tester.tap(
        find.widgetWithText(FilledButton, 'Import paired vault'),
      );
      await tester.pumpAndSettle();

      expect(find.text('Wrong password or keyfile'), findsOneWidget);
    });

    testWidgets('existing local target fails without showing SAS', (
      tester,
    ) async {
      final harness = await tester.pumpFeature(
        const ConnectSyncDialog(mobileOverride: false),
      );
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      harness.sync.beginPairImportError = const PathAlreadyExists(
        '/vaults/dup.kdbx',
      );

      await fillConnectForm(tester, name: 'dup');

      await tester.scrollUntilVisible(
        find.widgetWithText(FilledButton, 'Import paired vault'),
        100,
        scrollable: find.byType(Scrollable).first,
      );
      await tester.tap(
        find.widgetWithText(FilledButton, 'Import paired vault'),
      );
      await tester.pumpAndSettle();

      expect(
        find.text('A vault with this name already exists'),
        findsOneWidget,
      );
      expect(find.text('123 456'), findsNothing);
    });

    testWidgets('validation prevents empty required fields', (tester) async {
      final harness = await tester.pumpFeature(
        const ConnectSyncDialog(mobileOverride: false),
      );
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      await tester.scrollUntilVisible(
        find.widgetWithText(FilledButton, 'Import paired vault'),
        100,
        scrollable: find.byType(Scrollable).first,
      );
      await tester.tap(
        find.widgetWithText(FilledButton, 'Import paired vault'),
      );
      await tester.pumpAndSettle();

      expect(harness.sync.beginPairImportCalled, isFalse);
      expect(find.text('Required'), findsWidgets);
    });

    testWidgets('mobile import explains permission before pairing', (
      tester,
    ) async {
      final harness = await tester.pumpFeature(
        const ConnectSyncDialog(mobileOverride: true),
      );
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();
      await fillConnectForm(tester);
      await tester.scrollUntilVisible(
        find.widgetWithText(FilledButton, 'Import paired vault'),
        100,
        scrollable: find.byType(Scrollable).first,
      );

      await tester.tap(
        find.widgetWithText(FilledButton, 'Import paired vault'),
      );
      await tester.pumpAndSettle();
      expect(find.text('Allow local network access'), findsOneWidget);
      expect(
        find.textContaining('It never uses this permission for internet sync'),
        findsOneWidget,
      );
      expect(harness.sync.beginPairImportCalled, isFalse);

      await tester.tap(find.text('Confirm'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 10));
      expect(harness.sync.beginPairImportCalled, isTrue);
      expect(find.text('123 456'), findsOneWidget);
      await tester.tap(find.text('Reject'));
      await tester.pumpAndSettle();
      expect(find.text('123 456'), findsNothing);
    });
  });

  group('ChangePasswordDialog', () {
    testWidgets('successful change calls repo', (tester) async {
      final harness = await tester.pumpFeature(const ChangePasswordDialog());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      expect(find.text('Change master password'), findsOneWidget);

      await tester.enterText(
        find.widgetWithText(TextFormField, 'Current password'),
        'old-pass',
      );
      await tester.enterText(
        find.widgetWithText(TextFormField, 'New password'),
        'new-pass',
      );
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Confirm new password'),
        'new-pass',
      );

      await tester.tap(find.text('Confirm'));
      await tester.pumpAndSettle();

      expect(harness.session.changePasswordCalled, isTrue);
    });

    testWidgets('wrong current password shows error', (tester) async {
      final harness = await tester.pumpFeature(const ChangePasswordDialog());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      harness.session.changePasswordError = const BadCredentials();

      await tester.enterText(
        find.widgetWithText(TextFormField, 'Current password'),
        'wrong',
      );
      await tester.enterText(
        find.widgetWithText(TextFormField, 'New password'),
        'new-pass',
      );
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Confirm new password'),
        'new-pass',
      );

      await tester.tap(find.text('Confirm'));
      await tester.pumpAndSettle();

      expect(find.text('Wrong password or keyfile'), findsOneWidget);
    });

    testWidgets('registry write failure shows remediation', (tester) async {
      final harness = await tester.pumpFeature(const ChangePasswordDialog());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      harness.session.changePasswordError = const InternalFailure(
        'registry changed',
      );

      await tester.enterText(
        find.widgetWithText(TextFormField, 'Current password'),
        'current',
      );
      await tester.enterText(
        find.widgetWithText(TextFormField, 'New password'),
        'new-pass',
      );
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Confirm new password'),
        'new-pass',
      );

      await tester.tap(find.text('Confirm'));
      await tester.pumpAndSettle();

      expect(
        find.text(
          'Password changed. Your local sync identity was re-protected.',
        ),
        findsWidgets,
      );
    });

    testWidgets('mismatched passwords fail validation', (tester) async {
      final harness = await tester.pumpFeature(const ChangePasswordDialog());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      await tester.enterText(
        find.widgetWithText(TextFormField, 'Current password'),
        'current',
      );
      await tester.enterText(
        find.widgetWithText(TextFormField, 'New password'),
        'new-pass',
      );
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Confirm new password'),
        'different',
      );

      await tester.tap(find.text('Confirm'));
      await tester.pumpAndSettle();

      expect(harness.session.changePasswordCalled, isFalse);
      expect(find.text('Passwords do not match'), findsOneWidget);
    });
  });

  group('VaultManagementPage', () {
    testWidgets('import registers only the Hidlins-owned copied path', (
      tester,
    ) async {
      final harness = TestHarness();
      addTearDown(harness.dispose);
      harness.vaultImport.results.add(
        const PlatformSuccess(
          ImportedVault(
            sourceReference:
                '/Library/Application Support/Hidlins/Vaults/imported.kdbx',
            displayName: 'imported',
          ),
        ),
      );
      await tester.pumpFeatureWithHarness(const VaultManagementPage(), harness);
      await tester.pumpAndSettle();

      await tester.tap(find.byIcon(Icons.file_open_outlined));
      await tester.pumpAndSettle();

      expect(harness.vaultImport.calls, 1);
      expect(harness.session.registerExistingVaultCalled, isTrue);
      expect(
        harness.session.lastRegisteredVaultPath,
        '/Library/Application Support/Hidlins/Vaults/imported.kdbx',
      );
    });

    testWidgets('shows vault list with sync badge', (tester) async {
      final harness = TestHarness();
      addTearDown(harness.dispose);

      harness.session.vaults = [
        const VaultSummary(
          name: 'personal',
          path: '/v/personal.kdbx',
          hasKeyfile: false,
          hasSync: true,
        ),
        const VaultSummary(
          name: 'work',
          path: '/v/work.kdbx',
          hasKeyfile: true,
          hasSync: false,
        ),
      ];

      await tester.pumpWidget(
        ProviderScope(
          overrides: harness.overrides,
          child: MaterialApp(
            localizationsDelegates: hidlinsLocalizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            home: const VaultManagementPage(),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('personal'), findsOneWidget);
      expect(find.text('work'), findsOneWidget);
    });

    testWidgets('empty state shown with no vaults', (tester) async {
      final harness = await tester.pumpFeature(const VaultManagementPage());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      expect(find.text('No vaults registered'), findsOneWidget);
    });
  });
}
