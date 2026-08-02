import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/features/generator/generator_page.dart';
import 'package:app/src/ui/widgets/password_text.dart';
import '../helpers/feature_test_helpers.dart';

void main() {
  group('GeneratorPage', () {
    testWidgets('generates and displays password on init', (tester) async {
      final harness = await tester.pumpFeature(const GeneratorPage());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      expect(harness.generator.generatePasswordCalled, isTrue);
      expect(find.byType(PasswordText), findsOneWidget);
    });

    testWidgets('regenerates on option change', (tester) async {
      final harness = await tester.pumpFeature(const GeneratorPage());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      harness.generator.generatePasswordCalled = false;
      final slider = find.byType(Slider);
      expect(slider, findsOneWidget);

      await tester.drag(slider, const Offset(50, 0));
      await tester.pumpAndSettle();

      expect(harness.generator.generatePasswordCalled, isTrue);
    });

    testWidgets('shows entropy meter', (tester) async {
      final harness = await tester.pumpFeature(const GeneratorPage());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      expect(find.byType(LinearProgressIndicator), findsOneWidget);
      expect(find.text('65.0 bits'), findsOneWidget);
    });

    testWidgets('switches to diceware mode', (tester) async {
      final harness = await tester.pumpFeature(const GeneratorPage());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      await tester.tap(find.text('Diceware'));
      await tester.pumpAndSettle();

      expect(harness.generator.generatePassphraseCalled, isTrue);
      expect(find.text('Words'), findsOneWidget);
    });

    testWidgets('onUse callback returns generated value', (tester) async {
      String? usedValue;
      final harness = await tester.pumpFeature(
        GeneratorPage(onUse: (v) => usedValue = v),
      );
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      await tester.tap(find.text('Use'));
      await tester.pumpAndSettle();

      expect(usedValue, harness.generator.passwordResult.value);
    });

    testWidgets('reduced motion removes the regenerate rotation', (
      tester,
    ) async {
      final harness = await tester.pumpFeature(
        const MediaQuery(
          data: MediaQueryData(disableAnimations: true),
          child: GeneratorPage(),
        ),
      );
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('generator-rotation')), findsNothing);
      expect(find.byTooltip('Regenerate'), findsOneWidget);
    });
  });
}
