import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/group_tree.dart';
import '../helpers/feature_test_helpers.dart';

void main() {
  testWidgets('right-click group menu filters and starts entry creation', (
    tester,
  ) async {
    String? selected;
    String? createGroup;
    final root = GroupNode(
      uuid: 'root',
      name: 'Root',
      children: [
        GroupNode(
          uuid: 'work',
          name: 'Work',
          children: const [],
          entryCount: BigInt.one,
        ),
      ],
      entryCount: BigInt.one,
    );
    await tester.pumpFeature(
      GroupTree(
        root: root,
        selectedGroupUuid: null,
        onGroupSelected: (uuid) => selected = uuid,
        onCreateEntry: (uuid) => createGroup = uuid,
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(
      find.text('Work'),
      kind: PointerDeviceKind.mouse,
      buttons: kSecondaryMouseButton,
    );
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(MenuItemButton, 'Show entries').last);
    await tester.pumpAndSettle();
    expect(selected, 'work');
    expect(find.widgetWithText(MenuItemButton, 'Show entries'), findsNothing);

    await tester.tap(
      find.text('Work'),
      kind: PointerDeviceKind.mouse,
      buttons: kSecondaryMouseButton,
    );
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(MenuItemButton, 'New entry').last);
    await tester.pumpAndSettle();
    expect(createGroup, 'work');
    expect(find.widgetWithText(MenuItemButton, 'New entry'), findsNothing);
  });

  testWidgets('All entries root action selects the unfiltered state', (
    tester,
  ) async {
    String? selected = 'work';
    final root = GroupNode(
      uuid: 'root',
      name: 'Root',
      children: const [],
      entryCount: BigInt.one,
    );
    await tester.pumpFeature(
      GroupTree(
        root: root,
        selectedGroupUuid: selected,
        onGroupSelected: (uuid) => selected = uuid,
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(
      find.text('All entries'),
      kind: PointerDeviceKind.mouse,
      buttons: kSecondaryMouseButton,
    );
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(MenuItemButton, 'Show entries').last);
    await tester.pumpAndSettle();

    expect(selected, isNull);
  });
}
