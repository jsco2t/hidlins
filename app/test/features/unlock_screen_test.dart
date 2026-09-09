import 'package:material_ui/material_ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/data/failures.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/lock/unlock_screen.dart';
import 'package:app/src/l10n/app_localizations.dart';
import 'package:app/src/l10n/hidlins_localizations.dart';
import 'package:app/src/providers/providers.dart';
import 'package:app/src/ui/theme.dart';
import 'package:app/src/ui/widgets/brand_mark.dart';

import '../fakes/fake_repositories.dart';

void main() {
  const testVault = VaultSummary(
    name: 'test',
    path: '/test.kdbx',
    hasKeyfile: false,
    hasSync: false,
  );

  Widget buildApp(FakeSessionRepository session, [FakeSyncRepository? sync]) {
    return ProviderScope(
      overrides: [
        sessionRepositoryProvider.overrideWithValue(session),
        syncRepositoryProvider.overrideWithValue(sync ?? FakeSyncRepository()),
        vaultListProvider.overrideWith((_) async => session.vaults),
      ],
      child: MaterialApp(
        theme: hidlinsLightTheme(),
        localizationsDelegates: hidlinsLocalizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: const UnlockScreen(),
      ),
    );
  }

  group('UnlockScreen', () {
    testWidgets('shows password field and unlock button in idle state', (
      tester,
    ) async {
      final session = FakeSessionRepository()..vaults = [testVault];
      addTearDown(session.dispose);
      await tester.pumpWidget(buildApp(session));
      await tester.pumpAndSettle();

      expect(find.text('Vault locked'), findsOneWidget);
      expect(find.text('Unlock'), findsOneWidget);
      expect(find.byType(BrandMark), findsOneWidget);
    });

    testWidgets('shows error on bad password', (tester) async {
      final session = FakeSessionRepository()
        ..vaults = [testVault]
        ..unlockError = const BadCredentials();
      addTearDown(session.dispose);
      await tester.pumpWidget(buildApp(session));
      await tester.pumpAndSettle();

      final passwordField = find.byWidgetPredicate(
        (w) => w is TextField && w.obscureText == true,
      );
      expect(passwordField, findsOneWidget);
      await tester.enterText(passwordField, 'wrong');
      await tester.tap(find.text('Unlock'));
      await tester.pumpAndSettle();

      expect(find.text('Wrong password or keyfile'), findsOneWidget);
    });

    testWidgets('shows contended error with PID', (tester) async {
      final session = FakeSessionRepository()
        ..vaults = [testVault]
        ..unlockError = const VaultContended(holderPid: 12345);
      addTearDown(session.dispose);
      await tester.pumpWidget(buildApp(session));
      await tester.pumpAndSettle();

      final passwordField = find.byWidgetPredicate(
        (w) => w is TextField && w.obscureText == true,
      );
      await tester.enterText(passwordField, 'password');
      await tester.tap(find.text('Unlock'));
      await tester.pumpAndSettle();

      expect(
        find.text('Vault in use by another process (PID 12345)'),
        findsOneWidget,
      );
    });

    // LNS-REVIEW-003
    testWidgets('calls unlock with the entered password', (tester) async {
      final session = FakeSessionRepository()..vaults = [testVault];
      final sync = FakeSyncRepository();
      addTearDown(session.dispose);
      addTearDown(sync.dispose);
      await tester.pumpWidget(buildApp(session, sync));
      await tester.pumpAndSettle();

      final passwordField = find.byWidgetPredicate(
        (w) => w is TextField && w.obscureText == true,
      );
      await tester.enterText(passwordField, 'mypassword');
      await tester.tap(find.text('Unlock'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 100));

      expect(session.unlockCalled, isTrue);
      expect(session.lastUnlockPassword, 'mypassword');
      expect(sync.startupOrder, ['discover', 'sync']);
    });

    testWidgets('startup discovery failure stays nonfatal after unlock', (
      tester,
    ) async {
      final session = FakeSessionRepository()..vaults = [testVault];
      final sync = FakeSyncRepository()
        ..discoverError = const SyncUnreachable();
      addTearDown(session.dispose);
      addTearDown(sync.dispose);
      await tester.pumpWidget(buildApp(session, sync));
      await tester.pumpAndSettle();

      final passwordField = find.byWidgetPredicate(
        (widget) => widget is TextField && widget.obscureText,
      );
      await tester.enterText(passwordField, 'mypassword');
      await tester.tap(find.text('Unlock'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 100));

      expect(session.unlockCalled, isTrue);
      expect(sync.startupOrder, ['discover', 'sync']);
      expect(find.text('Something went wrong'), findsNothing);
    });

    testWidgets('password field label is visible', (tester) async {
      final session = FakeSessionRepository()..vaults = [testVault];
      addTearDown(session.dispose);
      await tester.pumpWidget(buildApp(session));
      await tester.pumpAndSettle();

      expect(find.text('Master password'), findsOneWidget);
    });
  });
}
