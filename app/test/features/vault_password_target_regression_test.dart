import 'package:app/src/data/failures.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/data/repositories.dart';
import 'package:app/src/features/vaults/change_password_dialog.dart';
import 'package:app/src/features/vaults/vault_management_page.dart';
import 'package:app/src/l10n/app_localizations.dart';
import 'package:app/src/l10n/hidlins_localizations.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

class _Session implements SessionRepository {
  String? selected;
  bool legacy = false;
  List<Object?>? credentials;

  // Dynamic dispatch keeps the pre-fix fixture valid before the named API is
  // added, without adding a stub to the production repository contract.
  @override
  dynamic noSuchMethod(Invocation invocation) {
    if (invocation.memberName == #listVaults) {
      return Future.value([
        for (final name in ['a', 'b'])
          VaultSummary(
            name: name,
            path: '/$name.kdbx',
            hasKeyfile: false,
            hasSync: false,
          ),
      ]);
    }
    if (invocation.memberName == #changeMasterPassword) {
      legacy = true;
      credentials = invocation.positionalArguments;
      return Future<void>.value();
    }
    if (invocation.memberName == #changeVaultMasterPassword) {
      selected = invocation.positionalArguments.first as String;
      credentials = invocation.positionalArguments.skip(1).toList();
      if (selected == 'b') {
        return Future<void>.error(
          const InvalidInputFailure(field: 'vault', reason: 'target mismatch'),
        );
      }
      return Future<void>.value();
    }
    return super.noSuchMethod(invocation);
  }
}

Future<List<TextEditingController>> _fill(WidgetTester tester) async {
  final controllers = <TextEditingController>[];
  for (final entry in {
    'Current password': 'shared-old',
    'New password': 'new-fixture',
    'Confirm new password': 'new-fixture',
  }.entries) {
    final field = find.widgetWithText(TextFormField, entry.key);
    await tester.enterText(field, entry.value);
    controllers.add(tester.widget<TextFormField>(field).controller!);
  }
  return controllers;
}

Future<void> _mount(WidgetTester tester, _Session session, Widget home) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: [sessionRepositoryProvider.overrideWithValue(session)],
      child: MaterialApp(
        localizationsDelegates: hidlinsLocalizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: home,
      ),
    ),
  );
  await tester.pumpAndSettle();
}

void main() {
  for (final name in ['a', 'b']) {
    testWidgets('management password action binds selected vault $name', (
      tester,
    ) async {
      final session = _Session();
      await _mount(tester, session, const VaultManagementPage());
      final tile = find.ancestor(
        of: find.text(name),
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
      final controllers = await _fill(tester);
      await tester.tap(find.widgetWithText(FilledButton, 'Confirm'));
      await tester.pump();
      expect(controllers.map((c) => c.text), everyElement(isEmpty));
      await tester.pumpAndSettle();
      expect(
        session.selected,
        name,
        reason: 'selected tile identity reaches repository',
      );
      expect(
        session.legacy,
        isFalse,
        reason: 'management must not rotate an implicit current vault',
      );
      expect(session.credentials, ['shared-old', 'new-fixture']);
      if (name == 'b') {
        expect(
          find.text('Unlock this vault before changing its password.'),
          findsOneWidget,
        );
        expect(find.byType(ChangePasswordDialog), findsOneWidget);
      } else {
        expect(find.byType(ChangePasswordDialog), findsNothing);
      }
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets('legacy current-vault dialog remains compatible', (tester) async {
    final session = _Session();
    await _mount(
      tester,
      session,
      Builder(
        builder: (context) => Scaffold(
          body: TextButton(
            onPressed: () => Navigator.of(context).push<void>(
              MaterialPageRoute(builder: (_) => const ChangePasswordDialog()),
            ),
            child: const Text('Open legacy dialog'),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Open legacy dialog'));
    await tester.pumpAndSettle();
    final controllers = await _fill(tester);
    await tester.tap(find.widgetWithText(FilledButton, 'Confirm'));
    await tester.pump();
    expect(controllers.map((c) => c.text), everyElement(isEmpty));
    await tester.pumpAndSettle();
    expect(session.legacy, isTrue);
    expect(session.selected, isNull);
    expect(session.credentials, ['shared-old', 'new-fixture']);
    expect(find.byType(ChangePasswordDialog), findsNothing);
    expect(tester.takeException(), isNull);
  });
}
