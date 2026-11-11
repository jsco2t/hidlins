import 'dart:async';

import 'package:app/src/data/failures.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/data/repositories.dart';
import 'package:app/src/features/generator/generator_page.dart';
import 'package:app/src/providers/providers.dart';
import 'package:app/src/ui/widgets/password_text.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import '../helpers/test_helpers.dart';

class _Generator implements GeneratorRepository {
  final calls = <({Object options, Completer<GeneratedSecret> result})>[];
  String? used;

  Future<GeneratedSecret> _enqueue(Object options) {
    final result = Completer<GeneratedSecret>();
    calls.add((options: options, result: result));
    return result.future;
  }

  @override
  Future<GeneratedSecret> generatePassword(PasswordOptionsDto options) =>
      _enqueue(options);

  @override
  Future<GeneratedSecret> generatePassphrase(PassphraseOptionsDto options) =>
      _enqueue(options);

  void finish(int index, String value) => calls[index].result.complete(
    GeneratedSecret(value: value, entropyBits: 80),
  );
}

Finder _button(String tooltip) => find.byWidgetPredicate(
  (widget) => widget is IconButton && widget.tooltip == tooltip,
);

Finder get _use => find.widgetWithText(FilledButton, 'Use');

Future<void> _tap(WidgetTester tester, Finder finder) async {
  await tester.ensureVisible(finder);
  await tester.tap(finder);
  await tester.pump();
}

Future<_Generator> _mount(WidgetTester tester) async {
  final repo = _Generator();
  await tester.pumpApp(
    ProviderScope(
      overrides: [generatorRepositoryProvider.overrideWithValue(repo)],
      child: GeneratorPage(onUse: (value) => repo.used = value),
    ),
  );
  await tester.pump();
  expect(repo.calls, hasLength(1));
  addTearDown(() async {
    await tester.pumpWidget(const SizedBox.shrink());
    for (final call in repo.calls) {
      if (!call.result.isCompleted) {
        call.result.complete(
          const GeneratedSecret(value: 'cleanup-marker', entropyBits: 80),
        );
      }
    }
    await tester.pump();
  });
  return repo;
}

void _expectUnavailable(WidgetTester tester) {
  expect(tester.widget<IconButton>(_button('Copy password')).onPressed, isNull);
  expect(tester.widget<FilledButton>(_use).onPressed, isNull);
  expect(find.byType(PasswordText), findsNothing);
}

void main() {
  testWidgets('latest options retain output after older completion', (
    tester,
  ) async {
    final repo = await _mount(tester);
    await _tap(tester, find.text('Lowercase'));
    expect((repo.calls[1].options as PasswordOptionsDto).lowercase, isFalse);
    repo.finish(1, 'new-options-marker');
    await tester.pump();
    repo.finish(0, 'old-options-marker');
    await tester.pumpAndSettle();
    expect(
      tester.widget<PasswordText>(find.byType(PasswordText)).password,
      'new-options-marker',
    );
  });

  testWidgets('older success cannot finish busy state of latest request', (
    tester,
  ) async {
    final repo = await _mount(tester);
    await _tap(tester, find.text('Lowercase'));
    repo.finish(0, 'old-options-marker');
    await tester.pump();
    expect(tester.widget<IconButton>(_button('Regenerate')).onPressed, isNull);
    _expectUnavailable(tester);
    repo.finish(1, 'new-options-marker');
    await tester.pumpAndSettle();
  });

  testWidgets('older failure cannot finish busy state of latest request', (
    tester,
  ) async {
    final repo = await _mount(tester);
    await _tap(tester, find.text('Lowercase'));
    repo.calls[0].result.completeError(Exception('old-error-private-marker'));
    await tester.pump();
    expect(tester.widget<IconButton>(_button('Regenerate')).onPressed, isNull);
    expect(find.text('An error occurred'), findsNothing);
    expect(find.textContaining('old-error-private-marker'), findsNothing);
    repo.finish(1, 'new-options-marker');
    await tester.pumpAndSettle();
  });

  testWidgets('regeneration revokes old Copy and Use until current success', (
    tester,
  ) async {
    final repo = await _mount(tester);
    repo.finish(0, 'previous-marker');
    await tester.pumpAndSettle();
    await _tap(tester, _button('Regenerate'));
    _expectUnavailable(tester);
    expect(repo.used, isNull);
    repo.finish(1, 'current-marker');
    await tester.pumpAndSettle();
    await _tap(tester, _use);
    expect(repo.used, 'current-marker');
  });

  testWidgets(
    'latest failure clears old output and shows safe localized error',
    (tester) async {
      final repo = await _mount(tester);
      repo.finish(0, 'previous-marker');
      await tester.pumpAndSettle();
      await _tap(tester, find.text('Lowercase'));
      repo.calls[1].result.completeError(Exception('sensitive-error-marker'));
      await tester.pumpAndSettle();
      _expectUnavailable(tester);
      expect(find.text('An error occurred'), findsOneWidget);
      expect(find.textContaining('sensitive-error-marker'), findsNothing);
      expect(
        tester.widget<IconButton>(_button('Regenerate')).onPressed,
        isNotNull,
      );
    },
  );

  testWidgets('empty character classes cannot leave an old actionable result', (
    tester,
  ) async {
    final repo = await _mount(tester);
    repo.finish(0, 'initial-valid-marker');
    await tester.pumpAndSettle();
    for (final label in ['Lowercase', 'Uppercase', 'Digits']) {
      await _tap(tester, find.text(label));
      repo.finish(repo.calls.length - 1, 'intermediate-valid-marker');
      await tester.pumpAndSettle();
    }
    await _tap(tester, find.text('Symbols'));
    final options = repo.calls.last.options as PasswordOptionsDto;
    expect([
      options.lowercase,
      options.uppercase,
      options.digits,
      options.symbols,
    ], everyElement(isFalse));
    repo.calls.last.result.completeError(
      const InvalidInputFailure(
        field: 'classes',
        reason: 'fixture-private-marker',
      ),
    );
    await tester.pumpAndSettle();
    _expectUnavailable(tester);
    expect(find.text('An error occurred'), findsOneWidget);
    expect(find.textContaining('fixture-private-marker'), findsNothing);
  });

  testWidgets('older success cannot resurrect output after latest failure', (
    tester,
  ) async {
    final repo = await _mount(tester);
    await _tap(tester, find.text('Lowercase'));
    repo.calls[1].result.completeError(Exception('controlled failure'));
    await tester.pump();
    repo.finish(0, 'stale-after-failure-marker');
    await tester.pumpAndSettle();
    _expectUnavailable(tester);
    expect(find.text('An error occurred'), findsOneWidget);
  });

  testWidgets('Diceware output survives late random completion', (
    tester,
  ) async {
    final repo = await _mount(tester);
    await _tap(tester, find.text('Diceware'));
    expect(repo.calls[1].options, isA<PassphraseOptionsDto>());
    repo.finish(1, 'current-diceware-marker');
    await tester.pump();
    repo.finish(0, 'stale-random-marker');
    await tester.pumpAndSettle();
    expect(
      tester.widget<PasswordText>(find.byType(PasswordText)).password,
      'current-diceware-marker',
    );
    await _tap(tester, _use);
    expect(repo.used, 'current-diceware-marker');
  });

  testWidgets('current retry recovers after failure without retaining error', (
    tester,
  ) async {
    final repo = await _mount(tester);
    repo.calls[0].result.completeError(Exception('controlled failure'));
    await tester.pumpAndSettle();
    expect(find.text('An error occurred'), findsOneWidget);
    await _tap(tester, _button('Regenerate'));
    expect(find.text('An error occurred'), findsNothing);
    repo.finish(1, 'recovered-marker');
    await tester.pumpAndSettle();
    await _tap(tester, _use);
    expect(repo.used, 'recovered-marker');
  });

  testWidgets('ordinary generation preserves entropy and Use', (tester) async {
    final repo = await _mount(tester);
    repo.finish(0, 'ordinary-marker');
    await tester.pumpAndSettle();
    expect(find.text('80.0 bits'), findsOneWidget);
    expect(find.text('An error occurred'), findsNothing);
    await _tap(tester, _use);
    expect(repo.used, 'ordinary-marker');
  });

  testWidgets('disposal rejects pending completion without an exception', (
    tester,
  ) async {
    final repo = await _mount(tester);
    await tester.pumpWidget(const SizedBox.shrink());
    repo.finish(0, 'disposed-marker');
    await tester.pump();
    expect(find.byType(PasswordText), findsNothing);
    expect(tester.takeException(), isNull);
  });
}
