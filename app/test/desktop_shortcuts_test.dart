import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import 'package:app/src/ui/desktop_shortcuts.dart';

import 'helpers/test_helpers.dart';

void main() {
  testWidgets('desktop shortcut map invokes every documented action', (
    tester,
  ) async {
    final invoked = <String>[];
    await tester.pumpApp(
      DesktopShortcuts(
        onSearch: () => invoked.add('search'),
        onNewEntry: () => invoked.add('new'),
        onLock: () => invoked.add('lock'),
        onCopyPassword: () => invoked.add('copy'),
        onGenerator: () => invoked.add('generator'),
        onDismiss: () => invoked.add('dismiss'),
        child: const Scaffold(body: Text('workspace')),
      ),
    );

    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyF);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyC);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyN);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyL);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyG);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);

    expect(invoked, ['search', 'copy', 'new', 'lock', 'generator', 'dismiss']);
  });

  testWidgets('unmodified shortcuts do not steal text input', (tester) async {
    final controller = TextEditingController();
    addTearDown(controller.dispose);
    var lockCount = 0;
    await tester.pumpApp(
      DesktopShortcuts(
        onSearch: () {},
        onNewEntry: () {},
        onLock: () => lockCount++,
        onCopyPassword: () {},
        onGenerator: () {},
        onDismiss: () {},
        child: Scaffold(
          body: TextField(controller: controller, autofocus: true),
        ),
      ),
    );
    await tester.pump();

    await tester.enterText(find.byType(TextField), 'login');

    expect(controller.text, 'login');
    expect(lockCount, 0);
  });
}
