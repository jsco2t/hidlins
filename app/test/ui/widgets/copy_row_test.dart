import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/ui/widgets/copy_row.dart';

import '../../helpers/test_helpers.dart';

void main() {
  group('CopyRow', () {
    testWidgets('displays label and value', (tester) async {
      await tester.pumpApp(
        CopyRow(label: 'Username', value: 'alice', onCopy: () {}),
      );
      expect(find.text('Username'), findsOneWidget);
      expect(find.text('alice'), findsOneWidget);
    });

    testWidgets('copy button calls onCopy when hovered and tapped', (
      tester,
    ) async {
      var copied = false;
      await tester.pumpApp(
        CopyRow(label: 'Password', value: '***', onCopy: () => copied = true),
      );

      // Hover to make the button fully opaque.
      final gesture = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await gesture.addPointer(location: Offset.zero);
      addTearDown(gesture.removePointer);
      await gesture.moveTo(tester.getCenter(find.byType(CopyRow)));
      await tester.pumpAndSettle();

      await tester.tap(find.byIcon(Icons.copy));
      expect(copied, isTrue);
    });

    testWidgets('copy icon is hidden when not hovered', (tester) async {
      await tester.pumpApp(
        CopyRow(label: 'URL', value: 'https://x', onCopy: () {}),
      );
      await tester.pumpAndSettle();
      final opacity = tester.widget<AnimatedOpacity>(
        find.byType(AnimatedOpacity),
      );
      expect(opacity.opacity, 0.0);
    });
  });
}
