import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/app.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/providers/providers.dart';
import 'fakes/fake_repositories.dart';

void main() {
  group('ActivityCapture', () {
    testWidgets('pointer event triggers onActivity', (tester) async {
      var count = 0;
      await tester.pumpWidget(
        MaterialApp(
          home: ActivityCapture(
            onActivity: () => count++,
            child: const SizedBox.expand(),
          ),
        ),
      );
      await tester.tapAt(const Offset(100, 100));
      expect(count, greaterThan(0));
    });

    testWidgets('keyboard event triggers onActivity', (tester) async {
      var count = 0;
      await tester.pumpWidget(
        MaterialApp(
          home: ActivityCapture(
            onActivity: () => count++,
            child: const SizedBox.expand(),
          ),
        ),
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
      expect(count, greaterThan(0));
    });

    testWidgets('text input reports activity via reportTextInput', (
      tester,
    ) async {
      var activityCount = 0;
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: ActivityCapture(
              onActivity: () => activityCount++,
              child: Builder(
                builder: (context) => TextField(
                  onChanged: (_) => ActivityCapture.reportTextInput(context),
                ),
              ),
            ),
          ),
        ),
      );

      await tester.tap(find.byType(TextField));
      await tester.pump();
      await tester.enterText(find.byType(TextField), 'hello');
      await tester.pump();
      expect(activityCount, greaterThan(0));
    });

    testWidgets('unlock text input reports through the session repository', (
      tester,
    ) async {
      final session = FakeSessionRepository()
        ..vaults = [
          const VaultSummary(
            name: 'personal',
            path: '/vaults/personal.kdbx',
            hasKeyfile: false,
            hasSync: false,
          ),
        ];
      addTearDown(session.dispose);

      await tester.pumpWidget(
        HidlinsApp(
          overrides: [sessionRepositoryProvider.overrideWithValue(session)],
        ),
      );
      await tester.pumpAndSettle();

      await tester.enterText(find.byType(TextField), 'master password');
      await tester.pump();

      expect(session.activityCount, 1);
    });

    testWidgets('lifecycle state change is forwarded', (tester) async {
      final states = <AppLifecycleState>[];
      await tester.pumpWidget(
        MaterialApp(
          home: ActivityCapture(
            onActivity: () {},
            onLifecycleStateChange: states.add,
            child: const SizedBox.expand(),
          ),
        ),
      );

      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
      expect(states, contains(AppLifecycleState.inactive));

      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
      expect(states, contains(AppLifecycleState.resumed));
    });

    testWidgets('lifecycle listener forwards states verbatim', (tester) async {
      final states = <AppLifecycleState>[];
      await tester.pumpWidget(
        MaterialApp(
          home: ActivityCapture(
            onActivity: () {},
            onLifecycleStateChange: states.add,
            child: const SizedBox.expand(),
          ),
        ),
      );

      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.hidden);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.paused);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.hidden);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);

      expect(states, [
        AppLifecycleState.inactive,
        AppLifecycleState.hidden,
        AppLifecycleState.paused,
        AppLifecycleState.hidden,
        AppLifecycleState.inactive,
        AppLifecycleState.resumed,
      ]);
    });
  });
}
