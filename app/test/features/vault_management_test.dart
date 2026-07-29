import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/data/failures.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/vaults/change_password_dialog.dart';
import 'package:app/src/features/vaults/connect_sync_dialog.dart';
import 'package:app/src/features/vaults/deregister_vault_dialog.dart';
import 'package:app/src/features/vaults/vault_management_page.dart';
import 'package:app/src/l10n/app_localizations.dart';
import '../helpers/feature_test_helpers.dart';

void main() {
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
      String bucket = 'my-bucket',
      String key = 'vault.kdbx',
      String region = 'us-east-1',
      String accessKey = 'AKID',
      String secretKey = 'secret',
      String password = 'pass123',
    }) async {
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Vault name'),
        name,
      );
      await tester.enterText(
        find.widgetWithText(TextFormField, 'S3 Bucket'),
        bucket,
      );
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Object key'),
        key,
      );
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Region'),
        region,
      );
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Access key ID'),
        accessKey,
      );
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Secret access key'),
        secretKey,
      );
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Master password'),
        password,
      );
    }

    testWidgets('successful bootstrap calls repo with S3 config', (
      tester,
    ) async {
      final harness = await tester.pumpFeature(const ConnectSyncDialog());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      expect(find.text('Connect to sync'), findsOneWidget);

      await fillConnectForm(tester);

      await tester.scrollUntilVisible(
        find.text('Confirm'),
        100,
        scrollable: find.byType(Scrollable).first,
      );
      await tester.tap(find.text('Confirm'));
      await tester.pumpAndSettle();

      expect(harness.session.bootstrapCalled, isTrue);
    });

    testWidgets('wrong password shows error', (tester) async {
      final harness = await tester.pumpFeature(const ConnectSyncDialog());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      harness.session.bootstrapError = const BadCredentials();

      await fillConnectForm(tester, password: 'wrong');

      await tester.scrollUntilVisible(
        find.text('Confirm'),
        100,
        scrollable: find.byType(Scrollable).first,
      );
      await tester.tap(find.text('Confirm'));
      await tester.pumpAndSettle();

      expect(find.text('Wrong password or keyfile'), findsOneWidget);
    });

    testWidgets('duplicate target shows error with vault name', (tester) async {
      final harness = await tester.pumpFeature(const ConnectSyncDialog());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      harness.session.bootstrapError = const SyncDuplicate('existing-vault');

      await fillConnectForm(tester, name: 'dup');

      await tester.scrollUntilVisible(
        find.text('Confirm'),
        100,
        scrollable: find.byType(Scrollable).first,
      );
      await tester.tap(find.text('Confirm'));
      await tester.pumpAndSettle();

      expect(find.textContaining('Duplicate sync target'), findsOneWidget);
      expect(find.textContaining('existing-vault'), findsOneWidget);
    });

    testWidgets('validation prevents empty required fields', (tester) async {
      final harness = await tester.pumpFeature(const ConnectSyncDialog());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      await tester.scrollUntilVisible(
        find.text('Confirm'),
        100,
        scrollable: find.byType(Scrollable).first,
      );
      await tester.tap(find.text('Confirm'));
      await tester.pumpAndSettle();

      expect(harness.session.bootstrapCalled, isFalse);
      expect(find.text('Required'), findsWidgets);
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
        find.text('Password changed. Re-enter your S3 sync credentials.'),
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
            localizationsDelegates: AppLocalizations.localizationsDelegates,
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
