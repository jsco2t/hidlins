import 'dart:io';

import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/ui/theme.dart';
import 'package:app/src/l10n/app_localizations.dart';
import 'package:app/src/l10n/hidlins_localizations.dart';

const goldenSizeCompact = Size(400, 800);
const goldenSizeMedium = Size(720, 800);
const goldenSizeExpanded = Size(1200, 800);

bool get _isLinux => Platform.isLinux;

Future<void> expectGolden(
  WidgetTester tester,
  Widget child, {
  required String name,
  Size size = goldenSizeExpanded,
  Brightness brightness = Brightness.light,
  bool settle = true,
}) async {
  if (!_isLinux) {
    markTestSkipped('Golden tests run on Linux only (deterministic rendering)');
    return;
  }

  final theme = brightness == Brightness.dark
      ? hidlinsDarkTheme()
      : hidlinsLightTheme();

  await tester.binding.setSurfaceSize(size);
  addTearDown(() => tester.binding.setSurfaceSize(null));

  await tester.pumpWidget(
    MaterialApp(
      theme: theme,
      debugShowCheckedModeBanner: false,
      localizationsDelegates: hidlinsLocalizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: child,
    ),
  );
  if (settle) {
    await tester.pumpAndSettle();
  } else {
    // Indeterminate progress indicators never settle. Advance by a fixed
    // interval so their captured phase remains reproducible.
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
  }

  await expectLater(
    find.byType(MaterialApp),
    matchesGoldenFile('goldens/$name.png'),
  );
}
