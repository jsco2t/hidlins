import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import 'package:app/src/app.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/platform/lifecycle.dart';
import 'package:app/src/providers/providers.dart';

import 'fakes/fake_repositories.dart';
import 'helpers/feature_test_helpers.dart';

class _Session extends FakeSessionRepository {
  bool hold = false;
  final pending = <Completer<LockEvent>>[];

  @override
  Future<LockEvent> reportLifecycleState(LifecycleStateDto state) {
    lifecycleStates.add(state);
    if (!hold) return Future.value(currentLockState);
    final completion = Completer<LockEvent>();
    pending.add(completion);
    return completion.future;
  }

  void finishPending() {
    for (final completion in pending) {
      if (!completion.isCompleted) completion.complete(currentLockState);
    }
  }
}

class _Fixture {
  final session = _Session();
  final calls = <Map<Object?, Object?>>[];

  void signal(WidgetTester tester, String state, String token) {
    tester.binding.channelBuffers.push(
      MethodChannelLifecycle.channelName,
      const StandardMethodCodec().encodeMethodCall(
        MethodCall('reportState', {'state': state, 'observationToken': token}),
      ),
      (_) {},
    );
  }

  Iterable<Map<Object?, Object?>> get requests =>
      calls.where((call) => call['requestObservation'] == true);
  Iterable<Map<Object?, Object?>> get acknowledgements =>
      calls.where((call) => call['acknowledged'] == true);
}

Future<void> _frames(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump();
}

Future<_Fixture> _open(WidgetTester tester, {bool hold = false}) async {
  final fixture = _Fixture()..session.hold = hold;
  final harness = TestHarness();
  const channel = MethodChannel(MethodChannelLifecycle.channelName);
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(channel, (
    call,
  ) async {
    fixture.calls.add(Map<Object?, Object?>.from(call.arguments as Map));
    return {'status': 'success', 'value': null};
  });
  addTearDown(() async {
    await tester.pumpWidget(const SizedBox.shrink());
    fixture.session.finishPending();
    await _frames(tester);
    fixture.session.dispose();
    harness.dispose();
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      channel,
      null,
    );
  });
  await tester.pumpWidget(
    HidlinsApp(
      overrides: [
        sessionRepositoryProvider.overrideWithValue(fixture.session),
        ...harness.overrides.skip(1),
      ],
    ),
  );
  await _frames(tester);
  expect(fixture.session.lifecycleStates, [LifecycleStateDto.resumed]);
  fixture.calls.clear();
  return fixture;
}

void main() {
  testWidgets(
    'native resumed observation token crosses the acknowledgement boundary',
    (tester) async {
      final fixture = await _open(tester);
      fixture.signal(tester, 'resumed', 'native-resume-C');
      await _frames(tester);
      expect(fixture.acknowledgements.single['state'], 'resumed');
      expect(fixture.acknowledgements.single['lockState'], 'locked');
      expect(
        fixture.acknowledgements.single['observationToken'],
        'native-resume-C',
      );
    },
    variant: const TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.iOS,
    }),
  );

  testWidgets(
    'paused observation acknowledgement retains its native token',
    (tester) async {
      final fixture = await _open(tester);
      fixture.signal(tester, 'paused', 'native-pause-B');
      await _frames(tester);
      expect(fixture.acknowledgements.single['state'], 'paused');
      expect(
        fixture.acknowledgements.single['observationToken'],
        'native-pause-B',
      );
    },
    variant: const TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.iOS,
    }),
  );

  testWidgets(
    'startup observation request does not wait for Rust reconciliation',
    (tester) async {
      final fixture = await _open(tester, hold: true);
      expect(fixture.session.pending, hasLength(1));
      await tester.pump(const Duration(milliseconds: 600));
      expect(fixture.requests, isNotEmpty);
      expect(fixture.acknowledgements, isEmpty);
      expect(fixture.session.pending.single.isCompleted, isFalse);
    },
    variant: const TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.iOS,
    }),
  );

  testWidgets(
    'receiving a native observation stops bootstrap retries while Rust is busy',
    (tester) async {
      final fixture = await _open(tester, hold: true);
      await tester.pump(const Duration(milliseconds: 600));
      expect(fixture.requests, isNotEmpty);
      fixture.signal(tester, 'paused', 'current-native-observation');
      await _frames(tester);
      final count = fixture.requests.length;
      await tester.pump(const Duration(seconds: 2));
      expect(fixture.requests, hasLength(count));
      expect(fixture.acknowledgements, isEmpty);
    },
    variant: const TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.iOS,
    }),
  );

  testWidgets(
    'disposal cancels bootstrap retries and denies late acknowledgements',
    (tester) async {
      final fixture = await _open(tester, hold: true);
      fixture.signal(tester, 'paused', 'native-before-disposal');
      await _frames(tester);
      await tester.pumpWidget(const SizedBox.shrink());
      fixture.session.finishPending();
      await tester.pump(const Duration(seconds: 2));
      expect(fixture.calls, isEmpty);
      expect(tester.takeException(), isNull);
    },
    variant: const TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.iOS,
    }),
  );
}
