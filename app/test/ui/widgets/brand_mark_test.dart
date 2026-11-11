import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/ui/widgets/brand_mark.dart';

void main() {
  testWidgets('uses navy on light surfaces and is decorative', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: ThemeData(brightness: Brightness.light),
        home: const BrandMark(),
      ),
    );

    expect(find.byKey(const ValueKey(BrandMark.navyAsset)), findsOneWidget);
    expect(
      find.ancestor(
        of: find.byType(Image),
        matching: find.byType(ExcludeSemantics),
      ),
      findsOneWidget,
    );
  });

  testWidgets('uses vintage treatment on dark surfaces', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: ThemeData(brightness: Brightness.dark),
        home: const BrandMark(),
      ),
    );

    expect(
      find.byKey(const ValueKey(BrandMark.vintageDarkAsset)),
      findsOneWidget,
    );
  });

  for (final variant in [
    (name: 'navy', brightness: Brightness.light, asset: BrandMark.navyAsset),
    (
      name: 'vintage_dark',
      brightness: Brightness.dark,
      asset: BrandMark.vintageDarkAsset,
    ),
  ]) {
    testWidgets('${variant.name} native-size raster golden', (tester) async {
      await tester.binding.setSurfaceSize(const Size.square(256));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final image = AssetImage(variant.asset);
      await tester.pumpWidget(
        MaterialApp(
          theme: ThemeData(brightness: variant.brightness),
          home: Center(
            child: Image(
              image: image,
              width: 256,
              height: 256,
              filterQuality: FilterQuality.none,
            ),
          ),
        ),
      );
      await tester.runAsync(
        () => precacheImage(image, tester.element(find.byType(Image))),
      );
      await tester.pumpAndSettle();

      await expectLater(
        find.byType(Image),
        matchesGoldenFile('goldens/brand_mark_${variant.name}.png'),
      );
    });
  }
}
