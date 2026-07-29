import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/ui/widgets/entry_avatar.dart';

import '../../helpers/test_helpers.dart';

void main() {
  group('EntryAvatar', () {
    testWidgets('displays uppercase initial', (tester) async {
      await tester.pumpApp(const EntryAvatar(title: 'github'));
      expect(find.text('G'), findsOneWidget);
    });

    testWidgets('shows ? for empty title', (tester) async {
      await tester.pumpApp(const EntryAvatar(title: ''));
      expect(find.text('?'), findsOneWidget);
    });

    testWidgets('same title produces same tint', (tester) async {
      // Render twice to verify deterministic tinting.
      await tester.pumpApp(
        const Row(
          children: [
            EntryAvatar(title: 'MyBank'),
            EntryAvatar(title: 'MyBank'),
          ],
        ),
      );
      final boxes = tester
          .widgetList<DecoratedBox>(find.byType(DecoratedBox))
          .toList();
      final color1 = (boxes[0].decoration as BoxDecoration).color;
      final color2 = (boxes[1].decoration as BoxDecoration).color;
      expect(color1, equals(color2));
    });

    testWidgets('respects custom size', (tester) async {
      await tester.pumpApp(const EntryAvatar(title: 'X', size: 60));
      final sizedBox = tester.widget<SizedBox>(find.byType(SizedBox));
      expect(sizedBox.width, 60);
      expect(sizedBox.height, 60);
    });
  });
}
