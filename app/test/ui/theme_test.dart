import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/ui/theme.dart';

void main() {
  group('hidlinsLightTheme', () {
    test('builds without assertion errors', () {
      final theme = hidlinsLightTheme();
      expect(theme.brightness, Brightness.light);
      expect(theme.colorScheme.brightness, Brightness.light);
      expect(theme.useMaterial3, isTrue);
    });
  });

  group('hidlinsDarkTheme', () {
    test('builds without assertion errors', () {
      final theme = hidlinsDarkTheme();
      expect(theme.brightness, Brightness.dark);
      expect(theme.colorScheme.brightness, Brightness.dark);
      expect(theme.useMaterial3, isTrue);
    });
  });

  group('resolveThemeMode', () {
    test('returns system for null', () {
      expect(resolveThemeMode(null), ThemeMode.system);
    });

    test('returns light for "light"', () {
      expect(resolveThemeMode('light'), ThemeMode.light);
    });

    test('returns dark for "dark"', () {
      expect(resolveThemeMode('dark'), ThemeMode.dark);
    });

    test('returns system for unknown string', () {
      expect(resolveThemeMode('auto'), ThemeMode.system);
      expect(resolveThemeMode(''), ThemeMode.system);
    });
  });
}
