import 'package:flutter/material.dart';

import '../typography.dart';

class SecretText extends StatefulWidget {
  const SecretText({super.key, required this.onReveal, this.maskLength = 8});

  final Future<String?> Function() onReveal;
  final int maskLength;

  @override
  State<SecretText> createState() => SecretTextState();
}

class SecretTextState extends State<SecretText> {
  final ValueNotifier<String?> _revealed = ValueNotifier<String?>(null);

  bool get isRevealed => _revealed.value != null;

  Future<void> reveal() async {
    String? value;
    try {
      value = await widget.onReveal();
    } on Exception {
      return;
    }
    if (!mounted) return;
    _revealed.value = value;
  }

  void hide() {
    _revealed.value = null;
  }

  @override
  void dispose() {
    _revealed.value = null;
    _revealed.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ValueListenableBuilder<String?>(
      valueListenable: _revealed,
      builder: (context, value, _) {
        if (value != null) {
          return Text(value, style: monospaceTextStyle(context));
        }
        return ExcludeSemantics(
          child: Text(
            '•' * widget.maskLength,
            style: monospaceTextStyle(context),
          ),
        );
      },
    );
  }
}
