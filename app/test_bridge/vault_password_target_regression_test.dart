import 'dart:io';

import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/frb_generated.dart';
import 'package:app/src/features/vaults/change_password_dialog.dart';
import 'package:app/src/features/vaults/vault_management_page.dart';
import 'package:app/src/l10n/app_localizations.dart';
import 'package:app/src/l10n/hidlins_localizations.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

Future<void> _drive(WidgetTester tester, bool Function() completed) async {
  final elapsed = Stopwatch()..start();
  while (!completed() && elapsed.elapsed < const Duration(seconds: 10)) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 1)),
    );
    await tester.pump(const Duration(milliseconds: 20));
  }
  expect(completed(), isTrue, reason: 'native/UI operation completed');
}

void main() {
  setUpAll(() async {
    final path = Platform.environment['HIDLINS_API_LIB'];
    expect(path, isNotNull, reason: 'run make app-test-vault-password-target');
    expect(File(path!).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(path));
  });

  for (final selected in ['b', 'a']) {
    testWidgets('real management password change targets selected $selected', (
      tester,
    ) async {
      final directory = Directory.systemTemp.createTempSync('hidlins-target-');
      final session = (await tester.runAsync(
        () => api.initApp(cfg: bridge.AppInitConfig(stateDir: directory.path)),
      ))!;
      try {
        final a = (await tester.runAsync(
          () => session.createVault(
            name: 'a',
            masterPassword: 'shared-old',
            confirmedNoRecovery: true,
          ),
        ))!;
        final b = (await tester.runAsync(
          () => session.createVault(
            name: 'b',
            masterPassword: 'shared-old',
            confirmedNoRecovery: true,
          ),
        ))!;
        await tester.runAsync(
          () => session.unlock(name: 'a', masterPassword: 'shared-old'),
        );
        final beforeA = File(a.path).readAsBytesSync();
        final beforeB = File(b.path).readAsBytesSync();
        await tester.pumpWidget(
          ProviderScope(
            overrides: [appSessionProvider.overrideWithValue(session)],
            child: MaterialApp(
              localizationsDelegates: hidlinsLocalizationsDelegates,
              supportedLocales: AppLocalizations.supportedLocales,
              home: const VaultManagementPage(),
            ),
          ),
        );
        await _drive(tester, () => find.text(selected).evaluate().isNotEmpty);
        final tile = find.ancestor(
          of: find.text(selected),
          matching: find.byType(ListTile),
        );
        await tester.tap(
          find.descendant(
            of: tile,
            matching: find.byWidgetPredicate((w) => w is PopupMenuButton),
          ),
        );
        await tester.pumpAndSettle();
        await tester.tap(find.text('Change master password'));
        await tester.pumpAndSettle();
        for (final entry in {
          'Current password': 'shared-old',
          'New password': 'selected-new',
          'Confirm new password': 'selected-new',
        }.entries) {
          await tester.enterText(
            find.widgetWithText(TextFormField, entry.key),
            entry.value,
          );
        }
        var completed = false;
        Object? callbackError;
        final confirm = tester.widget<FilledButton>(
          find.widgetWithText(FilledButton, 'Confirm'),
        );
        final operation =
            Function.apply(confirm.onPressed!, const []) as Future<void>;
        final observed = operation.then<void>(
          (_) {
            completed = true;
          },
          onError: (Object error) {
            callbackError = error;
            completed = true;
          },
        );
        await _drive(tester, () => completed);
        await observed;
        await tester.pumpAndSettle();
        expect(callbackError, isNull);
        expect(
          File(b.path).readAsBytesSync(),
          beforeB,
          reason: 'other vault remains byte-identical',
        );
        if (selected == 'b') {
          expect(
            File(a.path).readAsBytesSync(),
            beforeA,
            reason:
                'selecting B cannot rotate unlocked A with reused credentials',
          );
          expect(
            find.text('Unlock this vault before changing its password.'),
            findsOneWidget,
          );
          expect(find.byType(ChangePasswordDialog), findsOneWidget);
        } else {
          expect(File(a.path).readAsBytesSync(), isNot(beforeA));
          expect(find.byType(ChangePasswordDialog), findsNothing);
        }
        await tester.runAsync(session.lockNow);
        await tester.runAsync(
          () => session.unlock(
            name: 'a',
            masterPassword: selected == 'a' ? 'selected-new' : 'shared-old',
          ),
        );
        await tester.runAsync(session.lockNow);
        await tester.runAsync(
          () => session.unlock(name: 'b', masterPassword: 'shared-old'),
        );
        expect(tester.takeException(), isNull);
      } finally {
        await tester.pumpWidget(const SizedBox.shrink());
        await tester.pump();
        await tester.runAsync(session.shutdown);
        await tester.pump();
        directory.deleteSync(recursive: true);
      }
    });
  }
}
