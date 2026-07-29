import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/dto.dart';
import 'package:app/src/app.dart';
import 'package:app/src/providers/lock_state_provider.dart';

void main() {
  group('router redirect', () {
    testWidgets('redirects to /lock when lock stream emits locked', (
      tester,
    ) async {
      await tester.pumpWidget(
        HidlinsApp(
          overrides: [
            lockStateProvider.overrideWith(
              (_) => Stream.value(LockEvent.locked),
            ),
          ],
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('Vault locked'), findsOneWidget);
    });

    testWidgets('shows entries when unlocked', (tester) async {
      await tester.pumpWidget(
        HidlinsApp(
          overrides: [
            lockStateProvider.overrideWith(
              (_) => Stream.value(LockEvent.unlocked),
            ),
          ],
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('Entries'), findsWidgets);
    });
  });
}
