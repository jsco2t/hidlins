import 'dart:async';

import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entry_detail.dart';
import 'package:app/src/providers/providers.dart';
import 'package:app/src/ui/widgets/secret_text.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import '../helpers/feature_test_helpers.dart';

EntryDetail _detail(
  String uuid, {
  bool password = false,
  List<String> fields = const [],
  EntryKindDto kind = EntryKindDto.credential,
}) => EntryDetail(
  uuid: uuid,
  title: 'Entry $uuid',
  username: '',
  hasPassword: password,
  url: '',
  notes: '',
  kind: kind,
  tags: const [],
  customFields: [
    for (final name in fields) CustomFieldDto(name: name, isProtected: true),
  ],
  attachments: const [],
);

Future<({TestHarness h, ValueNotifier<String> selection, ProviderContainer c})>
_mount(WidgetTester tester, EntryDetail a, EntryDetail b) async {
  final h = TestHarness()..session.currentLockState = LockEvent.unlocked;
  h.entries.details.addAll({'a': a, 'b': b});
  final selection = ValueNotifier('a');
  addTearDown(h.dispose);
  addTearDown(selection.dispose);
  await tester.pumpFeatureWithHarness(
    ValueListenableBuilder<String>(
      valueListenable: selection,
      builder: (_, uuid, _) => Scaffold(body: EntryDetailPane(uuid: uuid)),
    ),
    h,
  );
  await tester.pumpAndSettle();
  final c = ProviderScope.containerOf(
    tester.element(find.byType(EntryDetailPane)),
  );
  // B must already be cached: a loading page would incidentally dispose A.
  await c.read(entryDetailProvider('b').future);
  return (h: h, selection: selection, c: c);
}

Finder _field(String name) =>
    find.ancestor(of: find.text(name), matching: find.byType(Row)).first;

Future<void> _revealField(WidgetTester tester, String name) async {
  final reveal = find.descendant(
    of: _field(name),
    matching: find.byTooltip('Reveal'),
  );
  await tester.ensureVisible(reveal);
  await tester.tap(reveal);
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('cached entry switch clears an already revealed password', (
    tester,
  ) async {
    final f = await _mount(
      tester,
      _detail('a', password: true),
      _detail('b', password: true),
    );
    f.h.secrets.revealResult = 'a-password-marker';
    await tester.tap(find.byTooltip('Reveal'));
    await tester.pumpAndSettle();
    expect(find.text('a-password-marker'), findsOneWidget);
    f.selection.value = 'b';
    await tester.pumpAndSettle();
    expect(find.text('Entry b'), findsOneWidget);
    expect(find.text('a-password-marker'), findsNothing);
    expect(find.text('••••••••'), findsOneWidget);
    f.h.secrets.revealResult = 'b-password-marker';
    await tester.tap(find.byTooltip('Reveal'));
    await tester.pumpAndSettle();
    expect(f.h.secrets.lastRevealUuid, 'b');
    expect(find.text('b-password-marker'), findsOneWidget);
  });

  testWidgets('delayed old password cannot publish or update disposed parent', (
    tester,
  ) async {
    final f = await _mount(
      tester,
      _detail('a', password: true),
      _detail('b', password: true),
    );
    final pending = f.h.secrets.revealCompleter = Completer<String>();
    await tester.tap(find.byTooltip('Reveal'));
    await tester.pump();
    expect(f.h.secrets.lastRevealUuid, 'a');
    f.selection.value = 'b';
    await tester.pumpAndSettle();
    pending.complete('late-a-password-marker');
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    expect(find.text('Entry b'), findsOneWidget);
    expect(find.text('late-a-password-marker'), findsNothing);
    expect(find.text('••••••••'), findsOneWidget);
  });

  testWidgets('cached entry switch clears revealed protected custom field', (
    tester,
  ) async {
    final f = await _mount(
      tester,
      _detail('a', fields: ['token']),
      _detail('b', fields: ['token']),
    );
    f.h.secrets.revealResultsByField['token'] = 'a-token-marker';
    await _revealField(tester, 'token');
    expect(find.text('a-token-marker'), findsOneWidget);
    f.selection.value = 'b';
    await tester.pumpAndSettle();
    expect(find.text('a-token-marker'), findsNothing);
    expect(find.text('••••••••'), findsOneWidget);
    f.h.secrets.revealResultsByField['token'] = 'b-token-marker';
    await _revealField(tester, 'token');
    expect(f.h.secrets.lastRevealUuid, 'b');
    expect(find.text('b-token-marker'), findsOneWidget);
  });

  testWidgets('same-entry field reorder cannot relabel a revealed secret', (
    tester,
  ) async {
    final f = await _mount(
      tester,
      _detail('a', fields: ['alpha', 'beta']),
      _detail('b'),
    );
    f.h.secrets.revealResultsByField['alpha'] = 'alpha-secret-marker';
    await _revealField(tester, 'alpha');
    expect(find.text('alpha-secret-marker'), findsOneWidget);
    f.h.entries.details['a'] = _detail('a', fields: ['beta', 'alpha']);
    f.c.invalidate(entryDetailProvider('a'));
    await tester.pumpAndSettle();
    expect(find.text('alpha-secret-marker'), findsNothing);
    expect(find.text('••••••••'), findsNWidgets(2));
    f.h.secrets.revealResultsByField['beta'] = 'beta-secret-marker';
    await _revealField(tester, 'beta');
    expect(
      find.descendant(
        of: _field('beta'),
        matching: find.text('beta-secret-marker'),
      ),
      findsOneWidget,
    );
  });

  testWidgets('same-entry refreshed snapshot revokes revealed password', (
    tester,
  ) async {
    final f = await _mount(tester, _detail('a', password: true), _detail('b'));
    f.h.secrets.revealResult = 'before-edit-marker';
    await tester.tap(find.byTooltip('Reveal'));
    await tester.pumpAndSettle();
    expect(find.text('before-edit-marker'), findsOneWidget);
    f.h.entries.details['a'] = _detail('a', password: true);
    f.c.invalidate(entryDetailProvider('a'));
    await tester.pumpAndSettle();
    expect(find.text('before-edit-marker'), findsNothing);
    expect(find.text('••••••••'), findsOneWidget);
  });

  testWidgets('replaced custom field rejects its pending reveal', (
    tester,
  ) async {
    final f = await _mount(
      tester,
      _detail('a', fields: ['alpha']),
      _detail('b'),
    );
    final pending = f.h.secrets.revealCompleter = Completer<String>();
    await tester.tap(find.byTooltip('Reveal'));
    await tester.pump();
    f.h.entries.details['a'] = _detail('a', fields: ['beta']);
    f.c.invalidate(entryDetailProvider('a'));
    await tester.pumpAndSettle();
    pending.complete('late-alpha-marker');
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    expect(find.text('beta'), findsOneWidget);
    expect(find.text('late-alpha-marker'), findsNothing);
    expect(find.text('••••••••'), findsOneWidget);
  });

  testWidgets('cached TOTP entry switch updates before the next timer tick', (
    tester,
  ) async {
    final f = await _mount(
      tester,
      _detail('a', kind: EntryKindDto.totp),
      _detail('b', kind: EntryKindDto.totp),
    );
    f.h.totp.codes.addAll({
      'a': const TotpCode(code: '123456', remainingSecs: 25, period: 30),
      'b': const TotpCode(code: '654321', remainingSecs: 25, period: 30),
    });
    await tester.pump(const Duration(seconds: 1));
    expect(find.text('123 456'), findsOneWidget);
    f.selection.value = 'b';
    await tester.pump();
    expect(find.text('123 456'), findsNothing);
    expect(find.text('654 321'), findsOneWidget);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('unrelated rebuild retains reveal for the same snapshot', (
    tester,
  ) async {
    final f = await _mount(tester, _detail('a', password: true), _detail('b'));
    await tester.tap(find.byTooltip('Reveal'));
    await tester.pumpAndSettle();
    final state = tester.state<SecretTextState>(find.byType(SecretText));
    // A fresh callback/build is not a changed entry or data snapshot.
    tester.element(find.byType(EntryDetailPane)).markNeedsBuild();
    await tester.pumpAndSettle();
    expect(tester.state<SecretTextState>(find.byType(SecretText)), same(state));
    expect(find.text('revealed-secret'), findsOneWidget);
    expect(f.h.secrets.revealCallCount, 1);
  });
}
