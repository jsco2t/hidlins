import 'package:app/src/app.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entries_page.dart';
import 'package:app/src/features/lock/unlock_screen.dart';
import 'package:app/src/features/settings/settings_page.dart';
import 'package:app/src/features/vaults/connect_sync_dialog.dart';
import 'package:app/src/features/vaults/create_vault_dialog.dart';
import 'package:app/src/features/vaults/vault_management_page.dart';
import 'package:app/src/platform/platform_result.dart';
import 'package:app/src/platform/vault_import.dart';
import 'package:app/src/providers/providers.dart';
import 'package:app/src/router.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import '../helpers/feature_test_helpers.dart';

Future<TestHarness> _mount(
  WidgetTester tester, {
  bool locked = true,
  double width = 1280,
}) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = Size(width, 1000);
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final h = TestHarness();
  h.session.currentLockState = locked ? LockEvent.locked : LockEvent.unlocked;
  h.session.vaults = [
    for (final name in ['a', 'b'])
      VaultSummary(
        name: name,
        path: '/$name.kdbx',
        hasKeyfile: false,
        hasSync: false,
      ),
  ];
  await tester.pumpWidget(HidlinsApp(overrides: h.overrides));
  await tester.pumpAndSettle();
  return h;
}

ProviderContainer _container(WidgetTester tester) =>
    ProviderScope.containerOf(tester.element(find.byType(Scaffold).first));

Future<void> _close(WidgetTester tester, TestHarness h) async {
  await tester.pumpWidget(const SizedBox.shrink());
  await tester.pump();
  h.dispose();
  await tester.pump();
}

Future<void> _open(WidgetTester tester) async {
  final action = find.text('Vaults');
  expect(action, findsOneWidget, reason: 'shipping screen offers management');
  await tester.ensureVisible(action);
  await tester.tap(action);
  await tester.pumpAndSettle();
  expect(find.byType(VaultManagementPage), findsOneWidget);
}

Future<void> _menu(WidgetTester tester, String name, String action) async {
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
  await tester.tap(find.text(action));
  await tester.pumpAndSettle();
}

void main() {
  for (final width in [390.0, 1280.0]) {
    for (final locked in [true, false]) {
      testWidgets('management navigation and back at $width locked=$locked', (
        tester,
      ) async {
        final h = await _mount(tester, width: width, locked: locked);
        try {
          if (!locked) {
            _container(tester).read(routerProvider).go('/settings');
            await tester.pumpAndSettle();
          } else {
            await tester.enterText(
              find.byType(TextField),
              'unsubmitted-master-marker',
            );
          }
          await _open(tester);
          expect(find.text('a'), findsOneWidget);
          expect(find.text('b'), findsOneWidget);
          expect(find.byType(EntriesPage), findsNothing);
          await tester.tap(find.byTooltip('Back'));
          await tester.pumpAndSettle();
          if (locked) {
            expect(find.byType(UnlockScreen), findsOneWidget);
            expect(
              tester.widget<TextField>(find.byType(TextField)).controller!.text,
              isEmpty,
            );
          } else {
            expect(find.byType(SettingsPage), findsOneWidget);
          }
          expect(h.session.lockNowCalled, isFalse);
          await _open(tester);
          if (!locked) {
            expect(find.byTooltip('Lock'), findsOneWidget);
            await tester.tap(find.byTooltip('Lock'));
            await tester.pumpAndSettle();
            expect(h.session.lockNowCalled, isTrue);
            expect(find.byType(VaultManagementPage), findsOneWidget);
            await tester.tap(find.byTooltip('Back'));
            await tester.pumpAndSettle();
            expect(find.byType(UnlockScreen), findsOneWidget);
            expect(find.byType(SettingsPage), findsNothing);
            expect(find.byType(EntriesPage), findsNothing);
          }
          expect(tester.takeException(), isNull);
        } finally {
          await _close(tester, h);
        }
      });
    }
  }

  testWidgets('only exact public metadata route bypasses locked redirect', (
    tester,
  ) async {
    final h = await _mount(tester);
    try {
      final router = _container(tester).read(routerProvider);
      for (final path in [
        '/entries',
        '/entries/entry-1',
        '/search',
        '/generator',
        '/sync',
        '/settings',
        '/vaults/private',
      ]) {
        router.go(path);
        await tester.pumpAndSettle();
        expect(find.byType(UnlockScreen), findsOneWidget, reason: path);
        expect(find.byType(EntriesPage), findsNothing);
        expect(find.byType(VaultManagementPage), findsNothing);
      }
      router.go('/vaults');
      await tester.pumpAndSettle();
      expect(find.byType(VaultManagementPage), findsOneWidget);
      expect(h.session.unlockCalled, isFalse);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets('registry refresh replaces a removed selection before unlock', (
    tester,
  ) async {
    final h = await _mount(tester);
    try {
      final container = _container(tester);
      h.session.vaults.removeWhere((v) => v.name == 'a');
      container.invalidate(vaultListProvider);
      await tester.pumpAndSettle();
      expect(
        tester.takeException(),
        isNull,
        reason: 'removed dropdown selection cannot survive registry refresh',
      );
      final field = find.byType(DropdownButtonFormField<String>);
      expect(tester.state<FormFieldState<String>>(field).value, 'b');
      await tester.enterText(find.byType(TextField), 'remaining-vault-marker');
      await tester.tap(find.widgetWithText(FilledButton, 'Unlock'));
      await tester.pumpAndSettle();
      expect(h.session.lastUnlockName, 'b');
      expect(find.byType(EntriesPage), findsOneWidget);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets(
    'nonempty locked registry reaches existing management workflows',
    (tester) async {
      final h = await _mount(tester);
      try {
        await _open(tester);
        await tester.tap(find.byTooltip('Create vault'));
        await tester.pumpAndSettle();
        expect(find.byType(CreateVaultDialog), findsOneWidget);
        await tester.tap(find.byTooltip('Back'));
        await tester.pumpAndSettle();
        expect(h.session.createVaultCalled, isFalse);
        await tester.tap(find.byTooltip('Import paired vault'));
        await tester.pumpAndSettle();
        expect(find.byType(ConnectSyncDialog), findsOneWidget);
        await tester.tap(find.byTooltip('Back'));
        await tester.pumpAndSettle();
        h.vaultImport.results.add(
          const PlatformSuccess(
            ImportedVault(
              sourceReference: '/imported.kdbx',
              displayName: 'imported',
            ),
          ),
        );
        await tester.tap(find.byTooltip('Import a .kdbx file'));
        await tester.pumpAndSettle();
        expect(h.session.lastRegisteredVaultPath, '/imported.kdbx');
        expect(find.text('imported'), findsOneWidget);
        await _menu(tester, 'b', 'Remove vault');
        await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
        await tester.pumpAndSettle();
        expect(h.session.deregisterCalled, isFalse);
        await _menu(tester, 'b', 'Remove vault');
        await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
        await tester.pumpAndSettle();
        expect(h.session.lastDeregisterName, 'b');
        expect(h.session.lastDeregisterDeleteFile, isFalse);
        expect(find.text('b'), findsNothing);
        expect(h.session.unlockCalled, isFalse);
        expect(tester.takeException(), isNull);
      } finally {
        await _close(tester, h);
      }
    },
  );

  testWidgets('empty registry keeps the first-run choices', (tester) async {
    final h = TestHarness();
    try {
      await tester.pumpWidget(HidlinsApp(overrides: h.overrides));
      await tester.pumpAndSettle();
      expect(find.text('Create a new vault'), findsOneWidget);
      expect(find.text('Import a .kdbx file'), findsOneWidget);
      expect(find.text('Import paired vault'), findsOneWidget);
      expect(find.byType(VaultManagementPage), findsNothing);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });
}
