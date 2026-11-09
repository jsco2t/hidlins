import 'dart:async';

import 'package:app/src/app.dart';
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/group_tree.dart';
import 'package:app/src/features/lock/unlock_screen.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import '../fakes/fake_repositories.dart';
import '../helpers/feature_test_helpers.dart';

class _Group {
  _Group(this.uuid, this.name, [List<_Group>? children])
    : children = children ?? [];
  final String uuid;
  String name;
  final List<_Group> children;
}

class _Entries extends FakeEntryRepository {
  final root = _Group('root-uuid', 'Root', [
    _Group('work-uuid', 'Work', [_Group('child-uuid', 'Child')]),
    _Group('empty-uuid', 'Empty'),
    _Group('dest-uuid', 'Destination'),
  ]);
  final calls = <(String, String, Object?)>[];
  final recycled = <String>{};
  Object? groupError;
  int _serial = 0;
  final _entries = const [
    EntrySummary(
      uuid: 'entry-1',
      title: 'subject',
      username: 'user',
      url: '',
      kind: EntryKindDto.credential,
      hasTotp: false,
      hasAttachments: false,
      isExpired: false,
      groupUuid: 'work-uuid',
      tags: [],
    ),
    EntrySummary(
      uuid: 'entry-2',
      title: 'unrelated',
      username: '',
      url: '',
      kind: EntryKindDto.secureNote,
      hasTotp: false,
      hasAttachments: false,
      isExpired: false,
      groupUuid: 'root-uuid',
      tags: [],
    ),
  ];

  _Entries() {
    details['entry-1'] = const EntryDetail(
      uuid: 'entry-1',
      title: 'subject',
      username: 'user',
      hasPassword: true,
      url: '',
      notes: 'retained notes',
      kind: EntryKindDto.credential,
      tags: [],
      customFields: [],
      attachments: [],
    );
    _refresh();
  }

  _Group? _find(String uuid, [_Group? group]) {
    final current = group ?? root;
    if (current.uuid == uuid) return current;
    for (final child in current.children) {
      final found = _find(uuid, child);
      if (found != null) return found;
    }
    return null;
  }

  _Group _remove(String uuid, [_Group? group]) {
    final current = group ?? root;
    final index = current.children.indexWhere((g) => g.uuid == uuid);
    if (index >= 0) return current.children.removeAt(index);
    for (final child in current.children) {
      if (_find(uuid, child) != null) return _remove(uuid, child);
    }
    throw StateError('fixture group missing');
  }

  GroupNode _node(_Group group) => GroupNode(
    uuid: group.uuid,
    name: group.name,
    entryCount: BigInt.from(
      _entries.where((e) => e.groupUuid == group.uuid).length,
    ),
    children: group.children.map(_node).toList(),
  );

  void _refresh() {
    tree = VaultTree(
      root: _node(root),
      entries: _entries.where((e) => _find(e.groupUuid) != null).toList(),
    );
  }

  void _record(String action, String uuid, Object? value) {
    calls.add((action, uuid, value));
    if (groupError case final error?) throw error;
  }

  void _changed() {
    _refresh();
    onMutation?.call();
  }

  @override
  Future<void> createGroup(String parentUuid, String name) async {
    _record('create', parentUuid, name);
    _find(parentUuid)!.children.add(_Group('new-${_serial++}', name));
    _changed();
  }

  @override
  Future<void> renameGroup(String uuid, String name) async {
    _record('rename', uuid, name);
    _find(uuid)!.name = name;
    _changed();
  }

  @override
  Future<void> moveGroup(String uuid, String parentUuid) async {
    _record('move', uuid, parentUuid);
    final moved = _remove(uuid);
    _find(parentUuid)!.children.add(moved);
    _changed();
  }

  @override
  Future<void> deleteGroup(String uuid, GroupDeleteBehavior behavior) async {
    _record('delete', uuid, behavior);
    final group = _find(uuid)!;
    if (behavior == GroupDeleteBehavior.refuse &&
        (group.children.isNotEmpty ||
            _entries.any((e) => e.groupUuid == uuid))) {
      throw Exception('fixture group not empty');
    }
    void retain(_Group node) {
      recycled.add(node.uuid);
      for (final child in node.children) {
        retain(child);
      }
    }

    retain(_remove(uuid));
    _changed();
  }
}

Future<TestHarness> _mount(
  WidgetTester tester,
  _Entries entries, {
  double width = 1280,
  TestHarness? harness,
}) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = Size(width, 1100);
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final h = harness ?? TestHarness();
  h.session.currentLockState = LockEvent.unlocked;
  registerTestVault(h);
  await tester.pumpWidget(
    HidlinsApp(
      overrides: [
        sessionRepositoryProvider.overrideWithValue(h.session),
        entryRepositoryProvider.overrideWithValue(entries),
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
  return h;
}

Future<void> _close(WidgetTester tester, TestHarness h) async {
  await tester.pumpWidget(const SizedBox.shrink());
  await tester.pump();
  h.dispose();
  await tester.pump();
}

Future<void> _openGroups(WidgetTester tester) async {
  expect(find.text('Groups'), findsOneWidget, reason: 'shipping group access');
  await tester.tap(find.text('Groups'));
  await tester.pumpAndSettle();
  expect(find.byType(GroupTree), findsOneWidget);
}

Future<void> _menu(WidgetTester tester, String group) async {
  final target = find.byTooltip('Actions for $group');
  expect(target, findsOneWidget);
  await tester.ensureVisible(target);
  await tester.tap(target);
  await tester.pumpAndSettle();
}

Future<void> _action(WidgetTester tester, String group, String action) async {
  await _menu(tester, group);
  await tester.tap(find.widgetWithText(MenuItemButton, action));
  await tester.pumpAndSettle();
  expect(find.widgetWithText(AlertDialog, action), findsOneWidget);
}

Future<void> _name(WidgetTester tester, String name, String confirm) async {
  await tester.enterText(
    find.widgetWithText(TextFormField, 'Group name'),
    name,
  );
  await tester.tap(find.widgetWithText(FilledButton, confirm));
  await tester.pumpAndSettle();
}

Future<void> _destination(WidgetTester tester, String uuid) async {
  await tester.tap(find.byType(DropdownButtonFormField<String>));
  await tester.pumpAndSettle();
  final choice = find.byWidgetPredicate(
    (w) => w is DropdownMenuItem<String> && w.value == uuid,
  );
  expect(choice, findsWidgets);
  await tester.tap(choice.last);
  await tester.pumpAndSettle();
}

Future<void> _recurse(WidgetTester tester) async {
  await tester.tap(find.byType(DropdownButtonFormField<GroupDeleteBehavior>));
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
}

void main() {
  for (final width in [390.0, 1280.0]) {
    testWidgets('shipping group CRUD refreshes at width $width', (
      tester,
    ) async {
      final entries = _Entries();
      final h = await _mount(tester, entries, width: width);
      try {
        await _openGroups(tester);
        await _action(tester, 'All entries', 'New group');
        await _name(tester, 'Created', 'Create');
        expect(entries.calls.last, ('create', 'root-uuid', 'Created'));
        expect(find.text('Created'), findsOneWidget);
        await _action(tester, 'Created', 'Rename group');
        await _name(tester, 'Renamed', 'Save');
        expect(entries.calls.last, ('rename', 'new-0', 'Renamed'));
        expect(find.text('Created'), findsNothing);
        expect(find.text('Renamed'), findsOneWidget);
        await _action(tester, 'Renamed', 'Move group');
        await _destination(tester, 'dest-uuid');
        await tester.tap(find.widgetWithText(FilledButton, 'Move'));
        await tester.pumpAndSettle();
        expect(entries.calls.last, ('move', 'new-0', 'dest-uuid'));
        expect(entries._find('dest-uuid')!.children.single.uuid, 'new-0');
        await _action(tester, 'Renamed', 'Delete group');
        expect(
          tester
              .widget<DropdownButtonFormField<GroupDeleteBehavior>>(
                find.byType(DropdownButtonFormField<GroupDeleteBehavior>),
              )
              .initialValue,
          GroupDeleteBehavior.refuse,
        );
        await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
        await tester.pumpAndSettle();
        expect(entries.calls.last, (
          'delete',
          'new-0',
          GroupDeleteBehavior.refuse,
        ));
        expect(find.text('Renamed'), findsNothing);
        expect(entries.tree.entries, hasLength(2));
        expect(tester.takeException(), isNull);
      } finally {
        await _close(tester, h);
      }
    });
  }

  testWidgets('nonempty delete defaults safe and requires explicit recursion', (
    tester,
  ) async {
    final entries = _Entries();
    final h = await _mount(tester, entries);
    try {
      await _openGroups(tester);
      await _action(tester, 'Work', 'Delete group');
      final delete = find.widgetWithText(FilledButton, 'Delete');
      expect(tester.widget<FilledButton>(delete).onPressed, isNull);
      expect(entries.calls, isEmpty);
      await _recurse(tester);
      expect(tester.widget<FilledButton>(delete).onPressed, isNotNull);
      await tester.tap(delete);
      await tester.pumpAndSettle();
      expect(entries.calls.single, (
        'delete',
        'work-uuid',
        GroupDeleteBehavior.recurse,
      ));
      expect(entries.recycled, containsAll(['work-uuid', 'child-uuid']));
      expect(entries.details['entry-1']!.notes, 'retained notes');
      expect(find.text('Work'), findsNothing);
      expect(find.text('Child'), findsNothing);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets('root supports naming but cannot be moved or deleted', (
    tester,
  ) async {
    final entries = _Entries();
    final h = await _mount(tester, entries);
    try {
      await _openGroups(tester);
      await _menu(tester, 'All entries');
      expect(find.widgetWithText(MenuItemButton, 'New group'), findsOneWidget);
      expect(
        find.widgetWithText(MenuItemButton, 'Rename group'),
        findsOneWidget,
      );
      expect(find.widgetWithText(MenuItemButton, 'Move group'), findsNothing);
      expect(find.widgetWithText(MenuItemButton, 'Delete group'), findsNothing);
      await tester.tap(find.widgetWithText(MenuItemButton, 'Rename group'));
      await tester.pumpAndSettle();
      await _name(tester, 'Named root', 'Save');
      expect(entries.tree.root.uuid, 'root-uuid');
      expect(entries.tree.root.name, 'Named root');
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets('move excludes self descendants and existing parent', (
    tester,
  ) async {
    final entries = _Entries();
    final h = await _mount(tester, entries);
    try {
      await _openGroups(tester);
      await _action(tester, 'Work', 'Move group');
      expect(
        tester
            .widget<FilledButton>(find.widgetWithText(FilledButton, 'Move'))
            .onPressed,
        isNull,
      );
      await tester.tap(find.byType(DropdownButtonFormField<String>));
      await tester.pumpAndSettle();
      for (final uuid in ['root-uuid', 'work-uuid', 'child-uuid']) {
        expect(
          find.byWidgetPredicate(
            (w) => w is DropdownMenuItem<String> && w.value == uuid,
          ),
          findsNothing,
        );
      }
      expect(entries.calls, isEmpty);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets('empty group name cannot create a group', (tester) async {
    final entries = _Entries();
    final h = await _mount(tester, entries);
    try {
      await _openGroups(tester);
      await _action(tester, 'All entries', 'New group');
      await _name(tester, '   ', 'Create');
      expect(find.text('Required'), findsOneWidget);
      expect(entries.calls, isEmpty);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  for (final action in [
    'New group',
    'Rename group',
    'Move group',
    'Delete group',
  ]) {
    testWidgets('$action cancellation has no side effects', (tester) async {
      final entries = _Entries();
      final h = await _mount(tester, entries);
      try {
        await _openGroups(tester);
        await _action(tester, 'Work', action);
        await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
        await tester.pumpAndSettle();
        expect(find.byType(AlertDialog), findsNothing);
        expect(entries.calls, isEmpty);
        expect(entries.tree.entries, hasLength(2));
        expect(tester.takeException(), isNull);
      } finally {
        await _close(tester, h);
      }
    });
  }

  testWidgets('unknown sync status disables group mutation menus', (
    tester,
  ) async {
    final gate = Completer<SyncStatusDto>();
    final pending = TestHarness()..sync.statusCompleter = gate;
    final entries = _Entries();
    final h = await _mount(tester, entries, harness: pending);
    try {
      await _openGroups(tester);
      expect(find.byTooltip('Actions for Work'), findsNothing);
      expect(entries.calls, isEmpty);
      expect(tester.takeException(), isNull);
    } finally {
      gate.complete(const SyncStatusDto(configured: false, inFlight: false));
      await tester.pump();
      await _close(tester, h);
    }
  });

  testWidgets('captured mutation callback is revoked when sync starts', (
    tester,
  ) async {
    final entries = _Entries();
    final h = await _mount(tester, entries);
    try {
      await _openGroups(tester);
      await _menu(tester, 'Work');
      final callback = tester
          .widget<MenuItemButton>(
            find.widgetWithText(MenuItemButton, 'Rename group'),
          )
          .onPressed!;
      h.sync.syncController.add(
        const bridge.SyncEvent.started(automatic: false),
      );
      await tester.pump();
      await tester.pump();
      callback();
      await tester.pump(const Duration(milliseconds: 400));
      await tester.pump();
      expect(find.byType(AlertDialog), findsNothing);
      expect(entries.calls, isEmpty);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets('completed sync revokes an old deletion confirmation', (
    tester,
  ) async {
    final entries = _Entries();
    final h = await _mount(tester, entries);
    try {
      await _openGroups(tester);
      await _action(tester, 'Empty', 'Delete group');
      h.sync.syncController.add(
        const bridge.SyncEvent.done(bridge.SyncOutcomeDto.alreadyInSync()),
      );
      await tester.pump();
      await tester.pump();
      await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
      await tester.pumpAndSettle();
      expect(entries.calls, isEmpty);
      expect(entries._find('empty-uuid'), isNotNull);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets('lock hides group dialog metadata without mutating', (
    tester,
  ) async {
    final entries = _Entries();
    final h = await _mount(tester, entries);
    try {
      await _openGroups(tester);
      await _action(tester, 'Work', 'Rename group');
      h.session.lockController.add(LockEvent.locked);
      await tester.pump();
      await tester.pump();
      expect(find.byType(AlertDialog), findsNothing);
      await tester.pumpAndSettle();
      expect(find.byType(UnlockScreen), findsOneWidget);
      expect(find.text('Work'), findsNothing);
      expect(entries.calls, isEmpty);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets('repeated captured callbacks open one group dialog', (
    tester,
  ) async {
    final entries = _Entries();
    final h = await _mount(tester, entries);
    try {
      await _openGroups(tester);
      await _menu(tester, 'Work');
      final callback = tester
          .widget<MenuItemButton>(
            find.widgetWithText(MenuItemButton, 'Rename group'),
          )
          .onPressed!;
      callback();
      callback();
      await tester.pumpAndSettle();
      expect(find.widgetWithText(AlertDialog, 'Rename group'), findsOneWidget);
      expect(entries.calls, isEmpty);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets('failure preserves the group and hides diagnostics', (
    tester,
  ) async {
    final entries = _Entries()..groupError = Exception('raw-marker');
    final h = await _mount(tester, entries);
    try {
      await _openGroups(tester);
      await _action(tester, 'Work', 'Rename group');
      await _name(tester, 'Changed', 'Save');
      expect(entries.calls, hasLength(1));
      expect(find.text('Work'), findsOneWidget);
      expect(find.text('Changed'), findsNothing);
      expect(find.textContaining('raw-marker'), findsNothing);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets('deleted selected group returns to visible unfiltered entries', (
    tester,
  ) async {
    final entries = _Entries();
    final h = await _mount(tester, entries, width: 390);
    try {
      await _openGroups(tester);
      await tester.tap(find.text('Work'));
      await tester.pumpAndSettle();
      expect(find.text('subject'), findsOneWidget);
      expect(find.text('unrelated'), findsNothing);
      await tester.tap(find.text('subject'));
      await tester.pumpAndSettle();
      await _openGroups(tester);
      await _action(tester, 'Work', 'Delete group');
      await _recurse(tester);
      await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
      await tester.pumpAndSettle();
      await tester.pageBack();
      await tester.pumpAndSettle();
      expect(find.text('unrelated'), findsOneWidget);
      expect(find.text('subject'), findsNothing);
      expect(find.byTooltip('Edit'), findsNothing);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets('All entries count includes visible descendants', (tester) async {
    final h = await tester.pumpFeature(
      GroupTree(
        root: GroupNode(
          uuid: 'root',
          name: 'Root',
          entryCount: BigInt.zero,
          children: [
            GroupNode(
              uuid: 'a',
              name: 'A',
              entryCount: BigInt.one,
              children: [],
            ),
            GroupNode(
              uuid: 'b',
              name: 'B',
              entryCount: BigInt.one,
              children: [],
            ),
          ],
        ),
        selectedGroupUuid: null,
        onGroupSelected: (_) {},
      ),
    );
    try {
      await tester.pumpAndSettle();
      final row = find.ancestor(
        of: find.text('All entries'),
        matching: find.byType(Row),
      );
      expect(
        find.descendant(of: row.first, matching: find.text('2')),
        findsOneWidget,
      );
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets(
    'existing group selection contract remains direct and unfiltered',
    (tester) async {
      final selected = <String?>[];
      final entries = _Entries();
      final h = await tester.pumpFeature(
        GroupTree(
          root: entries.tree.root,
          selectedGroupUuid: 'work-uuid',
          onGroupSelected: selected.add,
        ),
      );
      try {
        await tester.pumpAndSettle();
        await tester.tap(find.text('Work'));
        await tester.tap(find.text('All entries'));
        expect(selected, ['work-uuid', null]);
        expect(tester.takeException(), isNull);
      } finally {
        await _close(tester, h);
      }
    },
  );
}
