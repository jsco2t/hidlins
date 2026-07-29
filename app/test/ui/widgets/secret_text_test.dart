import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/ui/widgets/secret_text.dart';

import '../../helpers/test_helpers.dart';

void main() {
  group('SecretText', () {
    testWidgets('shows masked text by default', (tester) async {
      await tester.pumpApp(SecretText(onReveal: () async => 'hunter2'));
      expect(find.text('••••••••'), findsOneWidget);
      expect(find.text('hunter2'), findsNothing);
    });

    testWidgets('masked value is wrapped in ExcludeSemantics', (tester) async {
      await tester.pumpApp(SecretText(onReveal: () async => 'hunter2'));
      final secretTextFinder = find.byType(SecretText);
      expect(secretTextFinder, findsOneWidget);
      expect(
        find.descendant(
          of: secretTextFinder,
          matching: find.byType(ExcludeSemantics),
        ),
        findsOneWidget,
      );
    });

    testWidgets('revealed text is plain Text, not SelectableText', (
      tester,
    ) async {
      final key = GlobalKey<SecretTextState>();
      await tester.pumpApp(
        SecretText(key: key, onReveal: () async => 'hunter2'),
      );
      await key.currentState!.reveal();
      await tester.pump();

      expect(find.text('hunter2'), findsOneWidget);
      expect(find.byType(SelectableText), findsNothing);
    });

    testWidgets('reveal shows the secret value', (tester) async {
      final key = GlobalKey<SecretTextState>();
      await tester.pumpApp(
        SecretText(key: key, onReveal: () async => 'hunter2'),
      );
      expect(find.text('hunter2'), findsNothing);

      await key.currentState!.reveal();
      await tester.pump();

      expect(find.text('hunter2'), findsOneWidget);
      expect(find.text('••••••••'), findsNothing);
    });

    testWidgets('hide clears the revealed value', (tester) async {
      final key = GlobalKey<SecretTextState>();
      await tester.pumpApp(
        SecretText(key: key, onReveal: () async => 'hunter2'),
      );
      await key.currentState!.reveal();
      await tester.pump();
      expect(find.text('hunter2'), findsOneWidget);

      key.currentState!.hide();
      await tester.pump();
      expect(find.text('hunter2'), findsNothing);
      expect(find.text('••••••••'), findsOneWidget);
    });

    testWidgets('ValueNotifier is nulled and disposed on unmount', (
      tester,
    ) async {
      final key = GlobalKey<SecretTextState>();
      await tester.pumpApp(
        SecretText(key: key, onReveal: () async => 'hunter2'),
      );
      await key.currentState!.reveal();
      await tester.pump();

      await tester.pumpApp(const SizedBox.shrink());
      expect(key.currentState, isNull);
    });

    testWidgets('reveal then navigate away clears value from tree', (
      tester,
    ) async {
      final key = GlobalKey<SecretTextState>();
      await tester.pumpApp(
        SecretText(key: key, onReveal: () async => 'TopSecret'),
      );
      await key.currentState!.reveal();
      await tester.pump();
      expect(find.text('TopSecret'), findsOneWidget);

      await tester.pumpApp(const Text('Other screen'));
      expect(find.text('TopSecret'), findsNothing);
    });

    testWidgets('unmount during pending reveal does not update state', (
      tester,
    ) async {
      final completer = Completer<String?>();
      final key = GlobalKey<SecretTextState>();
      await tester.pumpApp(
        SecretText(key: key, onReveal: () => completer.future),
      );

      // Start reveal but don't complete it yet.
      final revealFuture = key.currentState!.reveal();

      // Unmount while the future is still pending.
      await tester.pumpApp(const SizedBox.shrink());
      expect(key.currentState, isNull);

      // Complete the future — the mounted guard prevents use-after-dispose.
      completer.complete('should-not-appear');
      await revealFuture;
      await tester.pump();
      expect(find.text('should-not-appear'), findsNothing);
    });

    testWidgets('reveal swallows bridge errors gracefully', (tester) async {
      final key = GlobalKey<SecretTextState>();
      await tester.pumpApp(
        SecretText(
          key: key,
          onReveal: () async => throw Exception('bridge failed'),
        ),
      );
      await key.currentState!.reveal();
      await tester.pump();

      // Should still show masked — no crash, no revealed text.
      expect(find.text('••••••••'), findsOneWidget);
    });
  });
}
