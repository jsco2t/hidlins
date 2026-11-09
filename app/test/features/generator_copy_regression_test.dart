import 'dart:async';

import 'package:app/src/data/failures.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/data/repositories.dart';
import 'package:app/src/features/generator/generator_page.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import '../helpers/test_helpers.dart';

class _Generator implements GeneratorRepository {
  final calls = <Completer<GeneratedSecret>>[];
  Future<GeneratedSecret> _next() {
    final result = Completer<GeneratedSecret>();
    calls.add(result);
    return result.future;
  }

  @override
  Future<GeneratedSecret> generatePassword(PasswordOptionsDto options) =>
      _next();
  @override
  Future<GeneratedSecret> generatePassphrase(PassphraseOptionsDto options) =>
      _next();
}

class _Secrets implements SecretsRepository {
  final copied = <GeneratedSecret>[];
  int entryCopies = 0;
  Object? error;
  Completer<void>? pending;

  // This future interface method exists on the test double before the fix so
  // the old widget compiles and fails on its actual invalid-entry dispatch.
  // ignore: annotate_overrides
  Future<void> copyGeneratedSecret(GeneratedSecret result) async {
    copied.add(result);
    if (error case final failure?) throw failure;
    await pending?.future;
  }

  @override
  Future<void> copyEntryField(String uuid, CopyField field) async {
    entryCopies++;
    throw const InvalidInputFailure(field: 'uuid', reason: 'invalid UUID');
  }

  @override
  Future<String> revealField(String uuid, RevealField field) =>
      throw StateError('generated copying must not reveal an entry');
}

Future<({_Generator generator, _Secrets secrets})> _mount(
  WidgetTester tester,
) async {
  final generator = _Generator();
  final secrets = _Secrets();
  await tester.pumpApp(
    ProviderScope(
      overrides: [
        generatorRepositoryProvider.overrideWithValue(generator),
        secretsRepositoryProvider.overrideWithValue(secrets),
      ],
      child: const GeneratorPage(),
    ),
  );
  await tester.pump();
  addTearDown(() async {
    await tester.pumpWidget(const SizedBox.shrink());
    for (final call in generator.calls) {
      if (!call.isCompleted) {
        call.complete(
          const GeneratedSecret(value: 'cleanup-marker', entropyBits: 80),
        );
      }
    }
    if (secrets.pending case final pending? when !pending.isCompleted) {
      pending.complete();
    }
    await tester.pump();
  });
  return (generator: generator, secrets: secrets);
}

Future<void> _copy(WidgetTester tester) async {
  final button = find.byTooltip('Copy password');
  await tester.ensureVisible(button);
  await tester.tap(button);
  await tester.pump();
}

void main() {
  testWidgets(
    'Copy sends the exact displayed random result instead of an entry UUID',
    (tester) async {
      final f = await _mount(tester);
      const result = GeneratedSecret(
        value: 'random-copy-marker',
        entropyBits: 80,
      );
      f.generator.calls[0].complete(result);
      await tester.pumpAndSettle();
      await _copy(tester);
      expect(f.secrets.copied, hasLength(1));
      expect(f.secrets.copied.single, same(result));
      expect(f.secrets.entryCopies, 0);
      expect(find.textContaining('Copied'), findsOneWidget);
    },
  );

  testWidgets('late random completion cannot change copied Diceware result', (
    tester,
  ) async {
    final f = await _mount(tester);
    await tester.tap(find.text('Diceware'));
    await tester.pump();
    const current = GeneratedSecret(
      value: 'diceware-copy-marker',
      entropyBits: 80,
    );
    f.generator.calls[1].complete(current);
    await tester.pump();
    f.generator.calls[0].complete(
      const GeneratedSecret(value: 'old-random-marker', entropyBits: 80),
    );
    await tester.pumpAndSettle();
    await _copy(tester);
    expect(f.secrets.copied, hasLength(1));
    expect(f.secrets.copied.single, same(current));
    expect(f.secrets.entryCopies, 0);
  });

  testWidgets(
    'another independent generation cannot replace the displayed copy',
    (tester) async {
      final f = await _mount(tester);
      const displayed = GeneratedSecret(
        value: 'displayed-copy-marker',
        entropyBits: 80,
      );
      f.generator.calls[0].complete(displayed);
      await tester.pumpAndSettle();
      final independent = f.generator.generatePassword(
        const PasswordOptionsDto(),
      );
      f.generator.calls[1].complete(
        const GeneratedSecret(value: 'independent-marker', entropyBits: 80),
      );
      await independent;
      await _copy(tester);
      expect(f.secrets.copied, hasLength(1));
      expect(f.secrets.copied.single, same(displayed));
    },
  );

  testWidgets(
    'copy failure displays safe feedback without secret or native error text',
    (tester) async {
      final f = await _mount(tester);
      f.secrets.error = Exception('private-copy-error-marker');
      f.generator.calls[0].complete(
        const GeneratedSecret(
          value: 'copy-error-secret-marker',
          entropyBits: 80,
        ),
      );
      await tester.pumpAndSettle();
      await _copy(tester);
      expect(f.secrets.copied, hasLength(1));
      expect(find.text('Copy failed'), findsOneWidget);
      expect(find.textContaining('private-copy-error-marker'), findsNothing);
    },
  );

  testWidgets('pending generated copy does not update a disposed page', (
    tester,
  ) async {
    final f = await _mount(tester);
    final pending = f.secrets.pending = Completer<void>();
    f.generator.calls[0].complete(
      const GeneratedSecret(value: 'pending-copy-marker', entropyBits: 80),
    );
    await tester.pumpAndSettle();
    await _copy(tester);
    expect(f.secrets.copied, hasLength(1));
    await tester.pumpWidget(const SizedBox.shrink());
    pending.complete();
    await tester.pump();
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'regeneration keeps generated Copy unavailable until current output',
    (tester) async {
      final f = await _mount(tester);
      f.generator.calls[0].complete(
        const GeneratedSecret(value: 'previous-marker', entropyBits: 80),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byTooltip('Regenerate'));
      await tester.pump();
      final button = tester.widget<IconButton>(
        find.byWidgetPredicate(
          (w) => w is IconButton && w.tooltip == 'Copy password',
        ),
      );
      expect(button.onPressed, isNull);
      expect(f.secrets.copied, isEmpty);
      expect(f.secrets.entryCopies, 0);
    },
  );
}
