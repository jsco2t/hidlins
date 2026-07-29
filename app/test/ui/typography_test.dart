import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/ui/typography.dart';

void main() {
  group('monospaceTextStyle', () {
    testWidgets('uses Apple monospace fallback on macOS', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;

      late TextStyle style;
      await tester.pumpWidget(
        MaterialApp(
          home: Builder(
            builder: (context) {
              style = monospaceTextStyle(context);
              return const SizedBox.shrink();
            },
          ),
        ),
      );
      expect(style.fontFamilyFallback, contains('SF Mono'));
      expect(style.fontFamilyFallback, contains('Menlo'));

      debugDefaultTargetPlatformOverride = null;
    });

    testWidgets('uses Apple monospace fallback on iOS', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.iOS;

      late TextStyle style;
      await tester.pumpWidget(
        MaterialApp(
          home: Builder(
            builder: (context) {
              style = monospaceTextStyle(context);
              return const SizedBox.shrink();
            },
          ),
        ),
      );
      expect(style.fontFamilyFallback, contains('SF Mono'));

      debugDefaultTargetPlatformOverride = null;
    });

    testWidgets('uses Android monospace fallback on Android', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.android;

      late TextStyle style;
      await tester.pumpWidget(
        MaterialApp(
          home: Builder(
            builder: (context) {
              style = monospaceTextStyle(context);
              return const SizedBox.shrink();
            },
          ),
        ),
      );
      expect(style.fontFamilyFallback, contains('Roboto Mono'));
      expect(style.fontFamilyFallback, contains('monospace'));

      debugDefaultTargetPlatformOverride = null;
    });

    testWidgets('uses Linux monospace fallback on Linux', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.linux;

      late TextStyle style;
      await tester.pumpWidget(
        MaterialApp(
          home: Builder(
            builder: (context) {
              style = monospaceTextStyle(context);
              return const SizedBox.shrink();
            },
          ),
        ),
      );
      expect(style.fontFamilyFallback, contains('DejaVu Sans Mono'));
      expect(style.fontFamilyFallback, contains('monospace'));

      debugDefaultTargetPlatformOverride = null;
    });

    testWidgets('respects custom fontSize', (tester) async {
      late TextStyle style;
      await tester.pumpWidget(
        MaterialApp(
          home: Builder(
            builder: (context) {
              style = monospaceTextStyle(context, fontSize: 20);
              return const SizedBox.shrink();
            },
          ),
        ),
      );
      expect(style.fontSize, 20);
    });
  });
}
