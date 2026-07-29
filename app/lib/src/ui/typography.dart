import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

TextStyle monospaceTextStyle(BuildContext context, {double? fontSize}) {
  final fallback = _monospaceFallback();
  return TextStyle(
    fontFamilyFallback: fallback,
    fontSize: fontSize,
    color: Theme.of(context).colorScheme.onSurface,
  );
}

List<String> _monospaceFallback() {
  return switch (defaultTargetPlatform) {
    TargetPlatform.iOS || TargetPlatform.macOS => const ['SF Mono', 'Menlo'],
    TargetPlatform.android => const ['Roboto Mono', 'monospace'],
    _ => const ['DejaVu Sans Mono', 'monospace'],
  };
}
