import 'package:flutter/material.dart';

import '../typography.dart';

class PasswordText extends StatelessWidget {
  const PasswordText({super.key, required this.password, this.fontSize});

  final String password;
  final double? fontSize;

  @override
  Widget build(BuildContext context) {
    final baseStyle = monospaceTextStyle(context, fontSize: fontSize);
    final colorScheme = Theme.of(context).colorScheme;
    return ExcludeSemantics(
      child: RichText(
        text: TextSpan(
          children: password.runes.map((rune) {
            final char = String.fromCharCode(rune);
            return TextSpan(
              text: char,
              style: baseStyle.copyWith(
                color: _colorForClass(classifyChar(char), colorScheme),
              ),
            );
          }).toList(),
        ),
      ),
    );
  }
}

Color _colorForClass(CharClass cls, ColorScheme colorScheme) {
  return switch (cls) {
    CharClass.digit => colorScheme.tertiary,
    CharClass.letter => colorScheme.onSurface,
    CharClass.symbol => colorScheme.error,
  };
}

enum CharClass { letter, digit, symbol }

CharClass classifyChar(String char) {
  final code = char.codeUnitAt(0);
  if (code >= 0x30 && code <= 0x39) return CharClass.digit;
  if ((code >= 0x41 && code <= 0x5A) || (code >= 0x61 && code <= 0x7A)) {
    return CharClass.letter;
  }
  return CharClass.symbol;
}
