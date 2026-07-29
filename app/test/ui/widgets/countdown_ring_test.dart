import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/ui/widgets/countdown_ring.dart';

import '../../helpers/test_helpers.dart';

void main() {
  group('CountdownRing', () {
    testWidgets('renders remaining seconds', (tester) async {
      await tester.pumpApp(
        const CountdownRing(remainingSecs: 15, periodSecs: 30),
      );
      expect(find.text('15'), findsOneWidget);
    });

    testWidgets('uses primary color when above amber threshold', (
      tester,
    ) async {
      await tester.pumpApp(
        const CountdownRing(remainingSecs: 20, periodSecs: 30),
      );
      // The text should use the primary color (not amber).
      final textWidget = tester.widget<Text>(find.text('20'));
      final color = textWidget.style?.color;
      expect(color, isNot(Colors.amber));
    });

    testWidgets('uses amber color below threshold', (tester) async {
      await tester.pumpApp(
        const CountdownRing(remainingSecs: 3, periodSecs: 30),
      );
      final textWidget = tester.widget<Text>(find.text('3'));
      expect(textWidget.style?.color, Colors.amber);
    });

    testWidgets('handles zero period gracefully', (tester) async {
      await tester.pumpApp(
        const CountdownRing(remainingSecs: 0, periodSecs: 0),
      );
      expect(find.text('0'), findsOneWidget);
    });
  });
}
