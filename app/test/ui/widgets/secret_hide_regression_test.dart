import 'dart:async';

import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:app/src/ui/widgets/secret_text.dart';

import '../../helpers/test_helpers.dart';

final class _Pending {
  final key = GlobalKey<SecretTextState>();
  final older = Completer<String?>();
  final newer = Completer<String?>();
  int calls = 0;

  Widget get widget => SecretText(
    key: key,
    onReveal: () => calls++ == 0 ? older.future : newer.future,
  );
}

void main() {
  testWidgets('older reveal cannot undo Hide after a newer reveal', (
    tester,
  ) async {
    final pending = _Pending();
    await tester.pumpApp(pending.widget);
    final older = pending.key.currentState!.reveal();
    final newer = pending.key.currentState!.reveal();
    pending.newer.complete('new-secret-marker');
    await newer;
    await tester.pump();
    expect(find.text('new-secret-marker'), findsOneWidget);
    pending.key.currentState!.hide();
    pending.older.complete('old-secret-marker');
    await older;
    await tester.pump();
    expect(find.text('old-secret-marker'), findsNothing);
    expect(find.text('new-secret-marker'), findsNothing);
    expect(find.text('••••••••'), findsOneWidget);
    expect(pending.key.currentState!.isRevealed, isFalse);
  });

  testWidgets('Hide cancels a pending reveal before it publishes', (
    tester,
  ) async {
    final pending = _Pending();
    await tester.pumpApp(pending.widget);
    final older = pending.key.currentState!.reveal();
    pending.key.currentState!.hide();
    pending.older.complete('old-secret-marker');
    await older;
    await tester.pump();
    expect(find.text('old-secret-marker'), findsNothing);
    expect(find.text('••••••••'), findsOneWidget);
  });

  testWidgets('the latest reveal request retains its result', (tester) async {
    final pending = _Pending();
    await tester.pumpApp(pending.widget);
    final older = pending.key.currentState!.reveal();
    final newer = pending.key.currentState!.reveal();
    pending.newer.complete('new-secret-marker');
    await newer;
    pending.older.complete('old-secret-marker');
    await older;
    await tester.pump();
    expect(find.text('new-secret-marker'), findsOneWidget);
    expect(find.text('old-secret-marker'), findsNothing);
  });

  testWidgets('a failed newer request cannot authorize an older completion', (
    tester,
  ) async {
    final pending = _Pending();
    await tester.pumpApp(pending.widget);
    final older = pending.key.currentState!.reveal();
    final newer = pending.key.currentState!.reveal();
    pending.newer.completeError(Exception('controlled reveal failure'));
    await newer;
    pending.older.complete('old-secret-marker');
    await older;
    await tester.pump();
    expect(find.text('old-secret-marker'), findsNothing);
    expect(find.text('••••••••'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('ordinary reveal and explicit Hide remain usable', (
    tester,
  ) async {
    final key = GlobalKey<SecretTextState>();
    await tester.pumpApp(
      SecretText(key: key, onReveal: () async => 'current-marker'),
    );
    await key.currentState!.reveal();
    await tester.pump();
    expect(find.text('current-marker'), findsOneWidget);
    key.currentState!.hide();
    await tester.pump();
    expect(find.text('current-marker'), findsNothing);
    expect(find.text('••••••••'), findsOneWidget);
  });

  testWidgets('disposal keeps a pending completion out of the widget tree', (
    tester,
  ) async {
    final pending = _Pending();
    await tester.pumpApp(pending.widget);
    final older = pending.key.currentState!.reveal();
    await tester.pumpApp(const SizedBox.shrink());
    pending.older.complete('old-secret-marker');
    await older;
    await tester.pump();
    expect(find.text('old-secret-marker'), findsNothing);
    expect(tester.takeException(), isNull);
  });
}
