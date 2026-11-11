import 'package:flutter/gestures.dart';
import 'package:flutter/foundation.dart';
import 'package:material_ui/material_ui.dart';
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

    testWidgets('iOS compact body stays inside unsafe screen insets', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.iOS;
      tester.view.physicalSize = const Size(390, 844);
      tester.view.devicePixelRatio = 1.0;
      tester.view.padding = const FakeViewPadding(
        top: 47,
        left: 3,
        right: 3,
        bottom: 34,
      );
      addTearDown(() => debugDefaultTargetPlatformOverride = null);
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPadding);

      await tester.pumpApp(
        AdaptiveScaffold(
          selectedIndex: 0,
          onDestinationSelected: (_) {},
          body: const ColoredBox(key: ValueKey('safe-body'), color: Colors.red),
        ),
      );
      debugDefaultTargetPlatformOverride = null;

      final body = tester.getRect(find.byKey(const ValueKey('safe-body')));
      expect(body.top, greaterThanOrEqualTo(47));
      expect(body.left, greaterThanOrEqualTo(3));
      expect(body.right, lessThanOrEqualTo(387));
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

    testWidgets(
      'expanded route body fills space when no secondary pane exists',
      (tester) async {
        tester.view.physicalSize = const Size(1200, 800);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        await tester.pumpApp(
          AdaptiveScaffold(
            selectedIndex: 0,
            onDestinationSelected: (_) {},
            body: const ColoredBox(
              key: ValueKey('route-body'),
              color: Colors.red,
            ),
          ),
        );

        expect(
          tester.getSize(find.byKey(const ValueKey('route-body'))).width,
          greaterThan(800),
        );
        expect(
          find.byWidgetPredicate(
            (widget) =>
                widget is MouseRegion &&
                widget.cursor == SystemMouseCursors.resizeColumn,
          ),
          findsNothing,
        );
      },
    );

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
