import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/ui/widgets/password_text.dart';

import '../../helpers/test_helpers.dart';

void main() {
  group('classifyChar', () {
    final table = {
      'a': CharClass.letter,
      'Z': CharClass.letter,
      'm': CharClass.letter,
      '0': CharClass.digit,
      '5': CharClass.digit,
      '9': CharClass.digit,
      '!': CharClass.symbol,
      '@': CharClass.symbol,
      '#': CharClass.symbol,
      '-': CharClass.symbol,
      ' ': CharClass.symbol,
    };

    for (final entry in table.entries) {
      test('"${entry.key}" is ${entry.value}', () {
        expect(classifyChar(entry.key), entry.value);
      });
    }
  });

  group('PasswordText', () {
    testWidgets('renders the password widget', (tester) async {
      await tester.pumpApp(const PasswordText(password: 'aB3!'));
      expect(find.byType(PasswordText), findsOneWidget);
      expect(find.byType(RichText), findsWidgets);
    });

    testWidgets('renders empty password without error', (tester) async {
      await tester.pumpApp(const PasswordText(password: ''));
      expect(find.byType(PasswordText), findsOneWidget);
    });
  });
}
