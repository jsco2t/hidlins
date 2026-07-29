import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/ui/theme.dart';
import 'package:app/src/l10n/app_localizations.dart';

Widget wrapWithTheme(Widget child, {Brightness brightness = Brightness.light}) {
  final theme = brightness == Brightness.dark
      ? hidlinsDarkTheme()
      : hidlinsLightTheme();
  return MaterialApp(
    theme: theme,
    localizationsDelegates: AppLocalizations.localizationsDelegates,
    supportedLocales: AppLocalizations.supportedLocales,
    home: child,
  );
}

extension HidlinsTesterExtensions on WidgetTester {
  Future<void> pumpApp(
    Widget child, {
    Brightness brightness = Brightness.light,
  }) async {
    await pumpWidget(wrapWithTheme(child, brightness: brightness));
  }
}
