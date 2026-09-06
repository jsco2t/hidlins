import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/ui/theme.dart';
import 'package:app/src/l10n/app_localizations.dart';
import 'package:app/src/l10n/hidlins_localizations.dart';

Widget wrapWithTheme(
  Widget child, {
  Brightness brightness = Brightness.light,
  TargetPlatform? platform,
}) {
  final baseTheme = brightness == Brightness.dark
      ? hidlinsDarkTheme()
      : hidlinsLightTheme();
  final theme = platform == null
      ? baseTheme
      : baseTheme.copyWith(platform: platform);
  return MaterialApp(
    theme: theme,
    localizationsDelegates: hidlinsLocalizationsDelegates,
    supportedLocales: AppLocalizations.supportedLocales,
    home: child,
  );
}

extension HidlinsTesterExtensions on WidgetTester {
  Future<void> pumpApp(
    Widget child, {
    Brightness brightness = Brightness.light,
    TargetPlatform? platform,
  }) async {
    await pumpWidget(
      wrapWithTheme(child, brightness: brightness, platform: platform),
    );
  }
}
