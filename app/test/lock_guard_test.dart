import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/dto.dart';
import 'package:app/src/providers/lock_state_provider.dart';
import 'package:app/src/ui/lock_guard.dart';
import 'package:app/src/l10n/app_localizations.dart';
import 'package:app/src/ui/theme.dart';

void main() {
  Widget buildApp({required LockEvent event, required Widget child}) {
    return ProviderScope(
      overrides: [lockStateProvider.overrideWith((_) => Stream.value(event))],
      child: MaterialApp(
        theme: hidlinsLightTheme(),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: LockGuard(child: child),
      ),
    );
  }

  group('LockGuard', () {
    testWidgets('shows lock icon when locked', (tester) async {
      await tester.pumpWidget(
        buildApp(event: LockEvent.locked, child: const Text('Secret content')),
      );
      await tester.pumpAndSettle();
      expect(find.byIcon(Icons.lock), findsOneWidget);
      expect(find.text('Secret content'), findsNothing);
    });

    testWidgets('shows child when unlocked', (tester) async {
      await tester.pumpWidget(
        buildApp(
          event: LockEvent.unlocked,
          child: const Text('Secret content'),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('Secret content'), findsOneWidget);
    });
  });
}
