import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/ui/widgets/section_card.dart';

import '../../helpers/test_helpers.dart';

void main() {
  group('SectionCard', () {
    testWidgets('renders child content', (tester) async {
      await tester.pumpApp(const SectionCard(child: Text('Hello')));
      expect(find.text('Hello'), findsOneWidget);
    });

    testWidgets('renders title when provided', (tester) async {
      await tester.pumpApp(
        const SectionCard(title: 'Details', child: Text('Content')),
      );
      expect(find.text('Details'), findsOneWidget);
      expect(find.text('Content'), findsOneWidget);
    });

    testWidgets('omits title when null', (tester) async {
      await tester.pumpApp(const SectionCard(child: Text('Only content')));
      expect(find.text('Only content'), findsOneWidget);
      // Only 1 Text widget should exist (the content).
      expect(find.byType(Text), findsOneWidget);
    });
  });
}
