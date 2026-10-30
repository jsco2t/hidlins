import 'package:flutter/services.dart';
import 'package:flutter/gestures.dart';
import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entry_list.dart';

import '../helpers/feature_test_helpers.dart';

void main() {
  final entries = [
    EntrySummary(
      uuid: 'entry-1',
      title: 'GitHub',
      username: 'octocat',
      url: 'https://github.com',
      kind: EntryKindDto.credential,
      hasTotp: false,
      hasAttachments: false,
      isExpired: false,
      groupUuid: 'root',
      tags: const [],
    ),
    EntrySummary(
      uuid: 'entry-2',
      title: 'Expired Account',
      username: 'old-user',
      url: '',
      kind: EntryKindDto.credential,
      hasTotp: false,
      hasAttachments: false,
      isExpired: true,
      groupUuid: 'root',
      tags: const [],
    ),
    EntrySummary(
      uuid: 'entry-3',
      title: 'Bank',
      username: 'banker',
      url: 'https://bank.test',
      kind: EntryKindDto.credential,
      hasTotp: false,
      hasAttachments: false,
      isExpired: false,
      groupUuid: 'root',
      tags: const [],
    ),
  ];

  group('EntryList', () {
    for (final selected in [false, true]) {
      testWidgets('wide row actions obey selection visibility $selected', (
        tester,
      ) async {
        String? copied;
        await tester.pumpFeature(
          EntryList(
            entries: [entries.first],
            selectedUuid: selected ? 'entry-1' : null,
            onEntrySelected: (_) {},
            onCopyUsername: (uuid) => copied = uuid,
            onCopyPassword: (uuid) => copied = uuid,
          ),
        );
        await tester.pumpAndSettle();
        final action = find.byIcon(Icons.copy);
        expect(action.hitTestable(), selected ? findsOneWidget : findsNothing);
        expect(
          find.bySemanticsLabel('Password'),
          selected ? findsOneWidget : findsNothing,
        );
        if (selected) {
          await tester.tap(action);
          expect(copied, 'entry-1');
        } else {
          for (var i = 0; i < 6; i++) {
            await tester.sendKeyEvent(LogicalKeyboardKey.tab);
            final focused = FocusManager.instance.primaryFocus?.context;
            var focusedButton = false;
            focused?.visitAncestorElements((element) {
              if (element.widget is IconButton) focusedButton = true;
              return true;
            });
            expect(
              focusedButton,
              isFalse,
              reason: 'Hidden quick actions must not receive keyboard focus',
            );
          }
        }
      });
    }

    testWidgets('narrow list retains titles without reserving quick actions', (
      tester,
    ) async {
      await tester.pumpFeature(
        Align(
          child: SizedBox(
            width: 320,
            child: EntryList(
              entries: [entries.first],
              selectedUuid: 'entry-1',
              onEntrySelected: (_) {},
              onCopyUsername: (_) {},
              onCopyPassword: (_) {},
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('GitHub'), findsOneWidget);
      expect(find.byType(IconButton), findsNothing);
    });

    testWidgets('renders entry titles and usernames', (tester) async {
      await tester.pumpFeature(
        EntryList(
          entries: entries,
          selectedUuid: null,
          onEntrySelected: (_) {},
          onCopyUsername: (_) {},
          onCopyPassword: (_) {},
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('GitHub'), findsOneWidget);
      expect(find.text('octocat'), findsOneWidget);
      expect(find.text('Expired Account'), findsOneWidget);
    });

    testWidgets('shows expired badge on expired entries', (tester) async {
      await tester.pumpFeature(
        EntryList(
          entries: entries,
          selectedUuid: null,
          onEntrySelected: (_) {},
          onCopyUsername: (_) {},
          onCopyPassword: (_) {},
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('Expired'), findsOneWidget);
    });

    testWidgets('calls onEntrySelected when tapped', (tester) async {
      String? selected;
      await tester.pumpFeature(
        EntryList(
          entries: entries,
          selectedUuid: null,
          onEntrySelected: (uuid) => selected = uuid,
          onCopyUsername: (_) {},
          onCopyPassword: (_) {},
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.text('GitHub'));
      expect(selected, 'entry-1');
    });

    testWidgets('shows empty state when no entries', (tester) async {
      await tester.pumpFeature(
        EntryList(
          entries: const [],
          selectedUuid: null,
          onEntrySelected: (_) {},
          onCopyUsername: (_) {},
          onCopyPassword: (_) {},
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('No entries yet'), findsOneWidget);
    });

    testWidgets('right click opens the entry context menu', (tester) async {
      await tester.pumpFeature(
        EntryList(
          entries: entries,
          selectedUuid: null,
          onEntrySelected: (_) {},
          onCopyUsername: (_) {},
          onCopyPassword: (_) {},
          onEditEntry: (_) {},
          onDeleteEntry: (_) {},
        ),
      );
      await tester.pumpAndSettle();

      await tester.tapAt(
        tester.getCenter(find.text('GitHub')),
        buttons: kSecondaryMouseButton,
      );
      await tester.pumpAndSettle();

      expect(find.text('Edit'), findsOneWidget);
      expect(find.text('Delete'), findsOneWidget);
    });

    testWidgets('reduced motion removes the hover fade duration', (
      tester,
    ) async {
      await tester.pumpFeature(
        MediaQuery(
          data: const MediaQueryData(disableAnimations: true),
          child: EntryList(
            entries: entries,
            selectedUuid: null,
            onEntrySelected: (_) {},
            onCopyUsername: (_) {},
            onCopyPassword: (_) {},
          ),
        ),
      );
      await tester.pump();

      expect(
        tester
            .widgetList<AnimatedOpacity>(find.byType(AnimatedOpacity))
            .every((widget) => widget.duration == Duration.zero),
        isTrue,
      );
    });
  });

  group('EntryList keyboard navigation', () {
    testWidgets('arrow down selects first entry when none selected', (
      tester,
    ) async {
      String? selected;
      await tester.pumpFeature(
        EntryList(
          entries: entries,
          selectedUuid: null,
          onEntrySelected: (uuid) => selected = uuid,
          onCopyUsername: (_) {},
          onCopyPassword: (_) {},
        ),
      );
      await tester.pumpAndSettle();

      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      expect(selected, 'entry-1');
    });

    testWidgets('arrow down moves to next entry', (tester) async {
      String? selected;
      await tester.pumpFeature(
        EntryList(
          entries: entries,
          selectedUuid: 'entry-1',
          onEntrySelected: (uuid) => selected = uuid,
          onCopyUsername: (_) {},
          onCopyPassword: (_) {},
        ),
      );
      await tester.pumpAndSettle();

      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      expect(selected, 'entry-2');
    });

    testWidgets('arrow down clamps at last entry', (tester) async {
      String? selected;
      await tester.pumpFeature(
        EntryList(
          entries: entries,
          selectedUuid: 'entry-3',
          onEntrySelected: (uuid) => selected = uuid,
          onCopyUsername: (_) {},
          onCopyPassword: (_) {},
        ),
      );
      await tester.pumpAndSettle();

      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      expect(selected, 'entry-3');
    });

    testWidgets('arrow up selects last entry when none selected', (
      tester,
    ) async {
      String? selected;
      await tester.pumpFeature(
        EntryList(
          entries: entries,
          selectedUuid: null,
          onEntrySelected: (uuid) => selected = uuid,
          onCopyUsername: (_) {},
          onCopyPassword: (_) {},
        ),
      );
      await tester.pumpAndSettle();

      await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
      expect(selected, 'entry-3');
    });

    testWidgets('arrow up moves to previous entry', (tester) async {
      String? selected;
      await tester.pumpFeature(
        EntryList(
          entries: entries,
          selectedUuid: 'entry-2',
          onEntrySelected: (uuid) => selected = uuid,
          onCopyUsername: (_) {},
          onCopyPassword: (_) {},
        ),
      );
      await tester.pumpAndSettle();

      await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
      expect(selected, 'entry-1');
    });

    testWidgets('arrow up clamps at first entry', (tester) async {
      String? selected;
      await tester.pumpFeature(
        EntryList(
          entries: entries,
          selectedUuid: 'entry-1',
          onEntrySelected: (uuid) => selected = uuid,
          onCopyUsername: (_) {},
          onCopyPassword: (_) {},
        ),
      );
      await tester.pumpAndSettle();

      await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
      expect(selected, 'entry-1');
    });

    testWidgets('enter re-selects the current entry', (tester) async {
      String? selected;
      await tester.pumpFeature(
        EntryList(
          entries: entries,
          selectedUuid: 'entry-2',
          onEntrySelected: (uuid) => selected = uuid,
          onCopyUsername: (_) {},
          onCopyPassword: (_) {},
        ),
      );
      await tester.pumpAndSettle();

      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      expect(selected, 'entry-2');
    });

    testWidgets('enter does nothing when no entry selected', (tester) async {
      String? selected;
      await tester.pumpFeature(
        EntryList(
          entries: entries,
          selectedUuid: null,
          onEntrySelected: (uuid) => selected = uuid,
          onCopyUsername: (_) {},
          onCopyPassword: (_) {},
        ),
      );
      await tester.pumpAndSettle();

      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      expect(selected, isNull);
    });
  });
}
