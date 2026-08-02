import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/ui/adaptive_scaffold.dart';

import 'helpers/test_helpers.dart';

void main() {
  group('AdaptiveScaffold', () {
    Widget buildScaffold({
      int selected = 0,
      double? initialWidth,
      ValueChanged<double>? onWidthChanged,
    }) {
      return AdaptiveScaffold(
        selectedIndex: selected,
        onDestinationSelected: (_) {},
        body: const Text('Body'),
        secondaryBody: const Text('Detail'),
        initialListPaneWidth: initialWidth,
        onListPaneWidthChanged: onWidthChanged,
      );
    }

    testWidgets('compact layout shows NavigationBar', (tester) async {
      tester.view.physicalSize = const Size(400, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpApp(buildScaffold());
      expect(find.byType(NavigationBar), findsOneWidget);
      expect(find.byType(NavigationRail), findsNothing);
    });

    testWidgets('medium layout shows NavigationRail', (tester) async {
      tester.view.physicalSize = const Size(720, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpApp(buildScaffold());
      expect(find.byType(NavigationRail), findsOneWidget);
      expect(find.byType(NavigationBar), findsNothing);
    });

    testWidgets('expanded layout shows extended NavigationRail', (
      tester,
    ) async {
      tester.view.physicalSize = const Size(1200, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpApp(buildScaffold());
      expect(find.byType(NavigationRail), findsOneWidget);
      expect(find.byType(NavigationBar), findsNothing);
      // Extended rail shows labels.
      expect(find.text('Entries'), findsOneWidget);
    });

    testWidgets('expanded single-pane body fills available workspace', (
      tester,
    ) async {
      tester.view.physicalSize = const Size(1200, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpApp(
        AdaptiveScaffold(
          selectedIndex: 0,
          onDestinationSelected: (_) {},
          body: const ColoredBox(
            key: Key('single-pane-body'),
            color: Colors.red,
          ),
        ),
      );

      expect(
        find.byWidgetPredicate(
          (w) =>
              w is MouseRegion && w.cursor == SystemMouseCursors.resizeColumn,
        ),
        findsNothing,
      );
      expect(
        tester.getSize(find.byKey(const Key('single-pane-body'))).width,
        greaterThan(800),
      );
    });

    testWidgets('body content preserved across resize', (tester) async {
      tester.view.physicalSize = const Size(1200, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpApp(buildScaffold());
      expect(find.text('Body'), findsOneWidget);

      // Resize to compact.
      tester.view.physicalSize = const Size(400, 800);
      await tester.pump();
      expect(find.text('Body'), findsOneWidget);
    });

    testWidgets('divider drag fires onListPaneWidthChanged', (tester) async {
      tester.view.physicalSize = const Size(1200, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      double? reportedWidth;
      await tester.pumpApp(
        buildScaffold(onWidthChanged: (w) => reportedWidth = w),
      );

      final divider = find.byWidgetPredicate(
        (w) => w is MouseRegion && w.cursor == SystemMouseCursors.resizeColumn,
      );
      expect(divider, findsOneWidget);

      final gesture = await tester.createGesture(kind: PointerDeviceKind.mouse);
      final center = tester.getCenter(divider);
      await gesture.down(center);
      await gesture.moveBy(const Offset(50, 0));
      await gesture.up();
      await tester.pump();

      expect(reportedWidth, isNotNull);
      expect(reportedWidth, greaterThan(320));
    });

    testWidgets('initialListPaneWidth sets starting width', (tester) async {
      tester.view.physicalSize = const Size(1200, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpApp(buildScaffold(initialWidth: 400));

      final sizedBoxes = tester.widgetList<SizedBox>(find.byType(SizedBox));
      final paneBox = sizedBoxes.where((sb) => sb.width == 400).toList();
      expect(paneBox, isNotEmpty);
    });
  });
}
