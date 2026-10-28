import 'package:flutter/gestures.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/group_tree.dart';

import '../helpers/feature_test_helpers.dart';

void main() {
  for (final selected in <String?>[null, 'root', 'work']) {
    testWidgets(
      'All entries always selects the unfiltered tree from $selected',
      (tester) async {
        final selections = <String?>[];
        String? createdIn;
        await tester.pumpFeature(
          GroupTree(
            root: GroupNode(
              uuid: 'root',
              name: 'Root',
              entryCount: BigInt.two,
              children: [
                GroupNode(
                  uuid: 'work',
                  name: 'Work',
                  entryCount: BigInt.one,
                  children: [],
                ),
              ],
            ),
            selectedGroupUuid: selected,
            onGroupSelected: selections.add,
            onNewEntry: (uuid) => createdIn = uuid,
          ),
        );
        await tester.pumpAndSettle();
        if (selected == null) {
          expect(
            tester
                .getSemantics(find.text('All entries'))
                .flagsCollection
                .isSelected
                .toBoolOrNull(),
            isTrue,
          );
        }
        await tester.tap(find.text('All entries'));
        expect(selections.last, isNull);
        await tester.tapAt(
          tester.getCenter(find.text('All entries')),
          buttons: kSecondaryMouseButton,
        );
        await tester.pumpAndSettle();
        expect(selections.last, isNull);
        await tester.tap(find.text('New entry'));
        await tester.pumpAndSettle();
        expect(createdIn, 'root');
      },
    );
  }

  testWidgets('group context menu creates an entry in that group', (
    tester,
  ) async {
    String? targetGroup;
    await tester.pumpFeature(
      GroupTree(
        root: GroupNode(
          uuid: 'root',
          name: 'Root',
          entryCount: BigInt.one,
          children: [
            GroupNode(
              uuid: 'work',
              name: 'Work',
              entryCount: BigInt.one,
              children: [],
            ),
          ],
        ),
        selectedGroupUuid: null,
        onGroupSelected: (_) {},
        onNewEntry: (uuid) => targetGroup = uuid,
      ),
    );
    await tester.pumpAndSettle();

    await tester.tapAt(
      tester.getCenter(find.text('Work')),
      buttons: kSecondaryMouseButton,
    );
    await tester.pumpAndSettle();

    expect(find.text('New entry'), findsOneWidget);
    await tester.tap(find.text('New entry'));
    await tester.pump();
    expect(targetGroup, 'work');
  });
}
