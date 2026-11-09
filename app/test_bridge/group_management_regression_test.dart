import 'dart:io';

import 'package:app/src/app.dart';
import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/frb_generated.dart';
import 'package:app/src/data/bridge_repositories.dart';
import 'package:app/src/data/models.dart' show GroupDeleteBehavior;
import 'package:app/src/providers/providers.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

Future<void> _drive(WidgetTester tester, bool Function() done) async {
  final elapsed = Stopwatch()..start();
  while (!done() && elapsed.elapsed < const Duration(seconds: 10)) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 1)),
    );
    await tester.pump(const Duration(milliseconds: 20));
  }
  expect(done(), isTrue, reason: 'native/UI operation completed');
}

bridge.GroupNode? _find(bridge.GroupNode root, String uuid) {
  if (root.uuid == uuid) return root;
  for (final child in root.children) {
    final found = _find(child, uuid);
    if (found != null) return found;
  }
  return null;
}

Future<void> _action(WidgetTester tester, String group, String action) async {
  final menu = find.byTooltip('Actions for $group');
  await _drive(tester, () => menu.evaluate().isNotEmpty);
  await tester.ensureVisible(menu);
  await tester.tap(menu);
  await tester.pumpAndSettle();
  await tester.tap(find.widgetWithText(MenuItemButton, action));
  await tester.pumpAndSettle();
  expect(find.widgetWithText(AlertDialog, action), findsOneWidget);
}

Future<void> _destination(WidgetTester tester, String uuid) async {
  await tester.tap(find.byType(DropdownButtonFormField<String>));
  await tester.pumpAndSettle();
  await tester.tap(
    find
        .byWidgetPredicate(
          (w) => w is DropdownMenuItem<String> && w.value == uuid,
        )
        .last,
  );
  await tester.pumpAndSettle();
}

void main() {
  setUpAll(() async {
    final path = Platform.environment['HIDLINS_API_LIB'];
    expect(path, isNotNull, reason: 'run make app-test-group-management');
    expect(File(path!).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(path));
  });

  testWidgets(
    'shipping group operations preserve UUIDs and recycled vault data',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(390, 1100);
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final directory = Directory.systemTemp.createTempSync('hidlins-groups-');
      final session = (await tester.runAsync(
        () => api.initApp(cfg: bridge.AppInitConfig(stateDir: directory.path)),
      ))!;
      final repo = BridgeEntryRepository(session);
      var notifications = 0;
      Future<void> commit(String label, int expected) async {
        await tester.tap(find.widgetWithText(FilledButton, label));
        await _drive(
          tester,
          () =>
              notifications == expected &&
              find.byType(AlertDialog).evaluate().isEmpty,
        );
      }

      try {
        await tester.runAsync(() async {
          await session.createVault(
            name: 'fixture',
            masterPassword: 'groups-master-fixture',
            confirmedNoRecovery: true,
          );
          await session.unlock(
            name: 'fixture',
            masterPassword: 'groups-master-fixture',
          );
        });
        final root = (await tester.runAsync(session.vaultTree))!.root.uuid;
        final work = (await tester.runAsync(
          () => session.createGroup(parent: root, name: 'Work'),
        ))!;
        final child = (await tester.runAsync(
          () => session.createGroup(parent: work, name: 'Child'),
        ))!;
        final destination = (await tester.runAsync(
          () => session.createGroup(parent: root, name: 'Destination'),
        ))!;
        final subject = (await tester.runAsync(
          () => session.createEntry(
            group: work,
            draft: const bridge.EntryDraftDto(
              kind: bridge.EntryKindDto.credential,
              title: 'prior subject',
              username: 'retained user',
              password: 'group-secret-marker',
              notes: 'retained notes',
              tags: [],
              customFields: [],
            ),
          ),
        ))!;
        await tester.runAsync(
          () => session.updateEntry(
            uuid: subject,
            edit: const bridge.EntryEditDto(title: 'subject'),
          ),
        );
        final unrelated = (await tester.runAsync(
          () => session.createEntry(
            group: root,
            draft: const bridge.EntryDraftDto(
              kind: bridge.EntryKindDto.secureNote,
              title: 'unrelated',
              notes: 'unrelated notes',
              tags: [],
              customFields: [],
            ),
          ),
        ))!;
        expect(
          (await tester.runAsync(() => session.entryHistory(uuid: subject)))!
              .single
              .title,
          'prior subject',
        );
        await tester.pumpWidget(
          HidlinsApp(
            overrides: [
              appSessionProvider.overrideWithValue(session),
              entryRepositoryProvider.overrideWithValue(repo),
            ],
          ),
        );
        await _drive(tester, () => find.text('subject').evaluate().isNotEmpty);
        final notify = repo.onMutation!;
        repo.onMutation = () {
          notifications++;
          notify();
        };
        await tester.tap(find.text('subject').first);
        await _drive(
          tester,
          () => find.byTooltip('Edit').evaluate().isNotEmpty,
        );
        await tester.pump(const Duration(milliseconds: 350));
        final groups = find.text('Groups');
        expect(groups, findsOneWidget, reason: 'shipping compact group access');
        await tester.ensureVisible(groups);
        await tester.tap(groups);
        await tester.pumpAndSettle();

        await _action(tester, 'All entries', 'New group');
        await tester.enterText(
          find.widgetWithText(TextFormField, 'Group name'),
          'Created',
        );
        await commit('Create', 1);
        final created = (await tester.runAsync(session.vaultTree))!
            .root
            .children
            .singleWhere((g) => g.name == 'Created')
            .uuid;
        expect([root, work, child, destination], isNot(contains(created)));
        await _action(tester, 'Created', 'Rename group');
        await tester.enterText(
          find.widgetWithText(TextFormField, 'Group name'),
          'Renamed',
        );
        await commit('Save', 2);
        expect(
          _find(
            (await tester.runAsync(session.vaultTree))!.root,
            created,
          )!.name,
          'Renamed',
        );
        await _action(tester, 'Renamed', 'Move group');
        await _destination(tester, destination);
        await commit('Move', 3);
        expect(
          _find(
            (await tester.runAsync(session.vaultTree))!.root,
            destination,
          )!.children.single.uuid,
          created,
        );
        await _action(tester, 'Renamed', 'Delete group');
        expect(
          tester
              .widget<DropdownButtonFormField<GroupDeleteBehavior>>(
                find.byType(DropdownButtonFormField<GroupDeleteBehavior>),
              )
              .initialValue,
          GroupDeleteBehavior.refuse,
        );
        await commit('Delete', 4);
        await _drive(tester, () => find.text('Renamed').evaluate().isEmpty);
        expect(
          _find((await tester.runAsync(session.vaultTree))!.root, created),
          isNull,
        );

        await _action(tester, 'Work', 'Move group');
        await _destination(tester, destination);
        await commit('Move', 5);
        final moved = (await tester.runAsync(session.vaultTree))!;
        final movedWork = _find(moved.root, destination)!.children.single;
        expect(movedWork.uuid, work);
        expect(movedWork.children.single.uuid, child);
        expect(
          moved.entries.singleWhere((e) => e.uuid == subject).groupUuid,
          work,
        );
        await _action(tester, 'Work', 'Delete group');
        expect(
          tester
              .widget<FilledButton>(find.widgetWithText(FilledButton, 'Delete'))
              .onPressed,
          isNull,
          reason: 'default Refuse cannot delete a nonempty group',
        );
        await tester.tap(
          find.byType(DropdownButtonFormField<GroupDeleteBehavior>),
        );
        await tester.pumpAndSettle();
        await tester.tap(
          find
              .byWidgetPredicate(
                (w) =>
                    w is DropdownMenuItem<GroupDeleteBehavior> &&
                    w.value == GroupDeleteBehavior.recurse,
              )
              .last,
        );
        await tester.pumpAndSettle();
        await commit('Delete', 6);
        await _drive(tester, () => find.text('Work').evaluate().isEmpty);
        expect(
          _find((await tester.runAsync(session.vaultTree))!.root, work),
          isNull,
        );
        await tester.pageBack();
        await _drive(
          tester,
          () =>
              find.text('unrelated').evaluate().isNotEmpty &&
              find.byTooltip('Edit').evaluate().isEmpty,
        );
        expect(find.text('subject'), findsNothing);
        expect(find.text('group-secret-marker'), findsNothing);
        final normal = (await tester.runAsync(
          () => session.search(
            opts: const bridge.SearchOptionsDto(
              query: 'subject',
              mode: bridge.SearchModeDto.substring,
              scope: bridge.SearchScopeDto.all(),
              includeRecycled: false,
            ),
          ),
        ))!;
        expect(normal, isEmpty);
        final recycled = (await tester.runAsync(
          () => session.search(
            opts: const bridge.SearchOptionsDto(
              query: 'subject',
              mode: bridge.SearchModeDto.substring,
              scope: bridge.SearchScopeDto.all(),
              includeRecycled: true,
            ),
          ),
        ))!;
        expect(recycled.single.entry.uuid, subject);
        expect(
          (await tester.runAsync(() => session.entryDetail(uuid: subject)))!
              .notes,
          'retained notes',
        );
        expect(
          (await tester.runAsync(() => session.entryHistory(uuid: subject)))!
              .single
              .title,
          'prior subject',
        );
        expect(
          await tester.runAsync(
            () => session.revealField(
              uuid: subject,
              field: const bridge.RevealField.password(),
            ),
          ),
          'group-secret-marker',
        );
        await tester.runAsync(session.lockNow);
        await tester.runAsync(
          () => session.unlock(
            name: 'fixture',
            masterPassword: 'groups-master-fixture',
          ),
        );
        final reopened = (await tester.runAsync(session.vaultTree))!;
        expect(reopened.root.uuid, root);
        expect(reopened.entries.single.uuid, unrelated);
        expect(_find(reopened.root, work), isNull);
        expect(
          (await tester.runAsync(() => session.entryDetail(uuid: subject)))!
              .notes,
          'retained notes',
        );
        expect(tester.takeException(), isNull);
      } finally {
        repo.onMutation = null;
        await tester.pumpWidget(const SizedBox.shrink());
        await tester.pump();
        await tester.runAsync(session.shutdown);
        await tester.pump();
        directory.deleteSync(recursive: true);
      }
    },
  );
}
