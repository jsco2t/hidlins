import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/ui/theme.dart';
import 'package:app/src/l10n/app_localizations.dart';

const goldenSizeCompact = Size(400, 800);
const goldenSizeMedium = Size(720, 800);
const goldenSizeExpanded = Size(1200, 800);

bool get _canRenderGoldens =>
    Platform.isLinux ||
    Platform.environment['HIDLINS_ALLOW_NON_LINUX_GOLDENS'] == '1';

Future<void> expectGolden(
  WidgetTester tester,
  Widget child, {
  required String name,
  Size size = goldenSizeExpanded,
  Brightness brightness = Brightness.light,
}) async {
  if (!_canRenderGoldens) {
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
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: child,
    ),
  );
  await tester.pumpAndSettle();

  await expectLater(
    find.byType(MaterialApp),
    matchesGoldenFile('goldens/$name.png'),
  );
}
