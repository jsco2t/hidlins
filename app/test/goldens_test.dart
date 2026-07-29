import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'helpers/golden.dart';

void main() {
  group('seed goldens', () {
    testWidgets('placeholder screen light', (tester) async {
      await expectGolden(
        tester,
        const _PlaceholderScreen(),
        name: 'placeholder_light',
        brightness: Brightness.light,
      );
    });

    testWidgets('placeholder screen dark', (tester) async {
      await expectGolden(
        tester,
        const _PlaceholderScreen(),
        name: 'placeholder_dark',
        brightness: Brightness.dark,
      );
    });
  });
}

class _PlaceholderScreen extends StatelessWidget {
  const _PlaceholderScreen();

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    return Scaffold(
      appBar: AppBar(title: const Text('Hidlins')),
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(Icons.lock, size: 48, color: colorScheme.primary),
            const SizedBox(height: 16),
            Text(
              'Vault locked',
              style: Theme.of(context).textTheme.headlineMedium,
            ),
          ],
        ),
      ),
    );
  }
}
