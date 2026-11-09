import 'dart:async';

import 'package:app/src/data/models.dart';
import 'package:app/src/features/sync/sync_controller.dart';
import 'package:app/src/providers/lock_state_provider.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'connect_import_lifetime_regression_test.dart'
    show ImportFixture, advanceImport;

void main() {
  for (final stage in ['reject', 'cancel', 'stop']) {
    testWidgets('new dialog waits for old dialog $stage cleanup', (
      tester,
    ) async {
      final f = ImportFixture();
      try {
        await f.mount(tester);
        final old = f.submit(tester);
        await advanceImport(tester);
        final gate = Completer<void>();
        switch (stage) {
          case 'reject':
            f.sync.rejectionGate = gate;
          case 'cancel':
            f.sync.cancellationGate = gate;
          case 'stop':
            f.sync.stopGate = gate;
        }
        final before = f.sync.discoveries;
        await f.remove(tester);
        await f.open(tester);
        final newer = f.submit(tester);
        await tester.pump();
        expect(f.sync.discoveries, before);
        gate.complete();
        await advanceImport(tester);
        expect(await old, isNull);
        expect(f.sync.discoveries, before + 1);
        await tester.tap(find.text('Reject'));
        await advanceImport(tester);
        expect(await newer, isNull);
      } finally {
        await f.close(tester);
      }
    });
  }

  testWidgets(
    'controller takeover cancels dialog and delayed old disposal is harmless',
    (tester) async {
      final f = ImportFixture(unlocked: true);
      try {
        await f.mount(tester);
        final lock = f.container.read(lockStateProvider.future);
        await tester.pump();
        await lock;
        final loaded = f.container.read(syncControllerProvider.future);
        await tester.pump();
        await loaded;
        final gate = f.sync.importGate = Completer<PairingPrompt>();
        final old = f.submit(tester);
        await tester.pump();
        expect(f.sync.imports, 1);
        final controller = f.container.read(syncControllerProvider.notifier);
        final newer = controller.useManualEndpoint('192.168.1.44:42873');
        await tester.pump();
        expect(f.sync.cancellations, 1);
        expect(f.sync.effects.where((effect) => effect == 'manual'), isEmpty);
        gate.complete(
          const PairingPrompt(
            transactionHandle: 'retired-import',
            sas: 'ABC-DEF',
          ),
        );
        await tester.pump();
        await tester.pump();
        await newer;
        expect(await old, isNull);
        expect(f.sync.rejected, ['retired-import']);
        expect(f.sync.route, '192.168.1.44');
        final before = (f.sync.cancellations, f.sync.stops);
        await f.remove(tester);
        await tester.pump();
        expect((f.sync.cancellations, f.sync.stops), before);
      } finally {
        await f.close(tester);
      }
    },
  );

  testWidgets(
    'cancel ack is prompt but replacement waits old candidate publication',
    (tester) async {
      final f = ImportFixture(unlocked: true);
      try {
        await f.mount(tester);
        final lock = f.container.read(lockStateProvider.future);
        await tester.pump();
        await lock;
        final loaded = f.container.read(syncControllerProvider.future);
        await tester.pump();
        await loaded;
        final discovery = f.sync.discoveryGate =
            Completer<LocalDiscoveryStatus>();
        final publish = f.sync.publicationGate = Completer<void>();
        final controller = f.container.read(syncControllerProvider.notifier);
        final old = controller.discover(DiscoveryKind.trusted);
        await tester.pump();
        var acknowledged = false;
        final cancellation = controller.cancelForegroundOperations().then(
          (_) => acknowledged = true,
        );
        await tester.pump();
        expect(acknowledged, isTrue);
        final newer = controller.useManualEndpoint('192.168.1.55:42873');
        await tester.pump();
        expect(f.sync.effects.where((effect) => effect == 'manual'), isEmpty);
        discovery.complete(f.sync.discovery);
        await tester.pump();
        expect(f.sync.effects.where((effect) => effect == 'manual'), isEmpty);
        publish.complete();
        await tester.pump();
        await tester.pump();
        await cancellation;
        await old;
        await newer;
        expect(f.sync.route, '192.168.1.55');
        expect(
          f.container
              .read(syncControllerProvider)
              .requireValue
              .discovery
              .candidates
              .single
              .address,
          '192.168.1.55',
        );
      } finally {
        await f.close(tester);
      }
    },
  );

  testWidgets(
    'same controller actions share ownership and retain both completions',
    (tester) async {
      final f = ImportFixture(unlocked: true);
      try {
        await f.mount(tester);
        final lock = f.container.read(lockStateProvider.future);
        await tester.pump();
        await lock;
        final loaded = f.container.read(syncControllerProvider.future);
        await tester.pump();
        await loaded;
        final controller = f.container.read(syncControllerProvider.notifier);
        final discovery = f.sync.discoveryGate =
            Completer<LocalDiscoveryStatus>();
        final first = controller.discover(DiscoveryKind.trusted);
        await tester.pump();
        await controller.useManualEndpoint('192.168.1.66:42873');
        expect(f.sync.cancellations, 0);
        discovery.complete(f.sync.discovery);
        await first;
        expect(
          f.container
              .read(syncControllerProvider)
              .requireValue
              .discovery
              .candidates,
          isNotEmpty,
        );
      } finally {
        await f.close(tester);
      }
    },
  );
}
