import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/ui/widgets/empty_state.dart';

import '../../helpers/test_helpers.dart';

void main() {
  group('EmptyState', () {
    testWidgets('renders icon and title', (tester) async {
      await tester.pumpApp(
        const EmptyState(icon: Icons.inbox, title: 'No entries'),
      );
      expect(find.byIcon(Icons.inbox), findsOneWidget);
      expect(find.text('No entries'), findsOneWidget);
    });

    testWidgets('renders subtitle when provided', (tester) async {
      await tester.pumpApp(
        const EmptyState(
          icon: Icons.inbox,
          title: 'No entries',
          subtitle: 'Add one',
        ),
      );
      expect(find.text('Add one'), findsOneWidget);
    });

    testWidgets('renders action widget when provided', (tester) async {
      await tester.pumpApp(
        EmptyState(
          icon: Icons.inbox,
          title: 'Empty',
          action: ElevatedButton(onPressed: () {}, child: const Text('Add')),
        ),
      );
      expect(find.widgetWithText(ElevatedButton, 'Add'), findsOneWidget);
    });

    testWidgets('renders without subtitle or action', (tester) async {
      await tester.pumpApp(
        const EmptyState(icon: Icons.search, title: 'No results'),
      );
      expect(find.text('No results'), findsOneWidget);
    });
  });
}
