import 'package:flutter/gestures.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/group_tree.dart';

import '../helpers/feature_test_helpers.dart';

void main() {
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
