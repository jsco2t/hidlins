import 'package:flutter/material.dart';

const _seedColor = Color(0xFF3E6B5C);

ThemeData hidlinsLightTheme() => _buildTheme(Brightness.light);
ThemeData hidlinsDarkTheme() => _buildTheme(Brightness.dark);

ThemeData _buildTheme(Brightness brightness) {
  final colorScheme = ColorScheme.fromSeed(
    seedColor: _seedColor,
    brightness: brightness,
  );
  return ThemeData(
    useMaterial3: true,
    colorScheme: colorScheme,
    visualDensity: VisualDensity.adaptivePlatformDensity,
  );
}

ThemeMode resolveThemeMode(String? pref) {
  return switch (pref) {
    'light' => ThemeMode.light,
    'dark' => ThemeMode.dark,
    _ => ThemeMode.system,
  };
}
