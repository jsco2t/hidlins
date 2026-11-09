import 'dart:async';

import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/data/models.dart';
import 'package:app/src/features/sync/sync_controller.dart';
import 'package:app/src/features/sync/sync_page.dart';
import 'package:app/src/providers/lock_state_provider.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import '../fakes/fake_repositories.dart';
import '../helpers/test_helpers.dart';

class _Sync extends FakeSyncRepository {
  final decisions = <(String, bool)>[];
  @override
  Future<VaultSummary?> confirmPairing({
    required String transactionHandle,
    required bool accepted,
    required String peerDisplayName,
  }) async {
    decisions.add((transactionHandle, accepted));
    return null;
  }

  void request(String handle, String sas) => syncController.add(
    bridge.SyncEvent.pairingRequested(
      bridge.PairingPromptDto(transactionHandle: handle, sas: sas),
    ),
  );
}

class _Fixture {
  final sync = _Sync()
    ..status = const SyncStatusDto(configured: true, inFlight: false);
  final session = FakeSessionRepository()
    ..currentLockState = LockEvent.unlocked;
  late final container = ProviderContainer(
    overrides: [
      syncRepositoryProvider.overrideWithValue(sync),
      sessionRepositoryProvider.overrideWithValue(session),
    ],
  );
  final navigator = GlobalKey<NavigatorState>();
  late MaterialPageRoute<void> route;

  Future<void> mount(WidgetTester tester) async {
    await tester.pumpApp(
      UncontrolledProviderScope(
        container: container,
        child: Navigator(
          key: navigator,
          onGenerateRoute: (_) => MaterialPageRoute<void>(
            builder: (_) => const Scaffold(body: Text('entries-marker')),
          ),
        ),
      ),
    );
    final lock = container.read(lockStateProvider.future);
    await tester.pump();
    await lock;
    final loaded = container.read(syncControllerProvider.future);
    await tester.pump();
    await loaded;
  }

  Future<void> open(WidgetTester tester) async {
    route = MaterialPageRoute<void>(
      builder: (_) => const SyncPage(desktopOverride: true),
    );
    unawaited(navigator.currentState!.push(route));
    await frames(tester);
  }

  Future<void> close(WidgetTester tester) async {
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump();
    container.dispose();
    sync.dispose();
    session.dispose();
    await tester.pump();
  }
}

Future<void> frames(WidgetTester tester) async {
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 350));
  await tester.pump();
}

Finder get _sas => find.byKey(const ValueKey('pairing-sas'));
Finder get _accept => find.widgetWithText(FilledButton, 'Codes match');

void main() {
  testWidgets(
    'current inbound prompt is shown when navigating from Entries to Sync',
    (tester) async {
      final f = _Fixture();
      try {
        await f.mount(tester);
        f.sync.request('off-page', 'ABC-DEF');
        await tester.pump();
        expect(
          f.container
              .read(syncControllerProvider)
              .requireValue
              .prompt
              ?.transactionHandle,
          'off-page',
        );
        await f.open(tester);
        expect(_sas, findsOneWidget);
        expect(tester.widget<SelectableText>(_sas).data, 'ABC-DEF');
        await tester.tap(find.text('Reject'));
        await frames(tester);
        expect(f.sync.decisions, [('off-page', false)]);
      } finally {
        await f.close(tester);
      }
    },
  );

  testWidgets(
    'future prompt appears once and harmless events do not duplicate it',
    (tester) async {
      final f = _Fixture();
      try {
        await f.mount(tester);
        await f.open(tester);
        f.sync.request('future', 'ABC-DEF');
        await frames(tester);
        expect(_sas, findsOneWidget);
        f.sync.syncController.add(
          const bridge.SyncEvent.started(automatic: true),
        );
        await frames(tester);
        expect(_sas, findsOneWidget);
        await tester.tap(_accept);
        await frames(tester);
        expect(f.sync.decisions, [('future', true)]);
      } finally {
        await f.close(tester);
      }
    },
  );

  testWidgets('replacement prompt has a single modal bound to its own SAS', (
    tester,
  ) async {
    final f = _Fixture();
    try {
      await f.mount(tester);
      await f.open(tester);
      f.sync.request('first', 'ABC-DEF');
      await frames(tester);
      f.sync.request('second', 'GHI-JKL');
      await frames(tester);
      expect(_sas, findsOneWidget);
      expect(tester.widget<SelectableText>(_sas).data, 'GHI-JKL');
      await tester.tap(_accept);
      await frames(tester);
      expect(f.sync.decisions.where((decision) => decision.$2), [
        ('second', true),
      ]);
    } finally {
      await f.close(tester);
    }
  });

  testWidgets(
    'stale first-modal callback cannot approve the newer transaction',
    (tester) async {
      final f = _Fixture();
      try {
        await f.mount(tester);
        await f.open(tester);
        f.sync.request('first', 'ABC-DEF');
        await frames(tester);
        final oldAccept = tester.widget<FilledButton>(_accept).onPressed!;
        f.sync.request('second', 'GHI-JKL');
        await frames(tester);
        oldAccept();
        await frames(tester);
        expect(f.sync.decisions.where((decision) => decision.$2), isEmpty);
        expect(_sas, findsOneWidget);
        expect(tester.widget<SelectableText>(_sas).data, 'GHI-JKL');
        expect(tester.takeException(), isNull);
      } finally {
        await f.close(tester);
      }
    },
  );

  testWidgets('foreground cancellation dismisses its stale SAS modal', (
    tester,
  ) async {
    final f = _Fixture();
    try {
      await f.mount(tester);
      await f.open(tester);
      f.sync.request('cancelled', 'ABC-DEF');
      await frames(tester);
      final cancellation = f.container
          .read(syncControllerProvider.notifier)
          .cancelForegroundOperations();
      await frames(tester);
      await cancellation;
      expect(_sas, findsNothing);
      expect(f.sync.decisions, [('cancelled', false)]);
      expect(tester.takeException(), isNull);
    } finally {
      await f.close(tester);
    }
  });

  testWidgets('removing Sync parent removes only its owned SAS route', (
    tester,
  ) async {
    final f = _Fixture();
    try {
      await f.mount(tester);
      await f.open(tester);
      f.sync.request('removed', 'ABC-DEF');
      await frames(tester);
      f.navigator.currentState!.removeRoute(f.route);
      await frames(tester);
      expect(_sas, findsNothing);
      expect(find.text('entries-marker'), findsOneWidget);
      expect(f.sync.decisions, [('removed', false)]);
      expect(tester.takeException(), isNull);
    } finally {
      await f.close(tester);
    }
  });
}
