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

class _Request {
  _Request(this.state);
  final LifecycleStateDto state;
  final completion = Completer<LockEvent>();
}

class _Session extends FakeSessionRepository {
  final requests = <_Request>[];

  @override
  Future<LockEvent> reportLifecycleState(LifecycleStateDto state) {
    final request = _Request(state);
    requests.add(request);
    return request.completion.future;
  }

  void finishPending() {
    for (final request in requests) {
      if (!request.completion.isCompleted) {
        request.completion.complete(currentLockState);
      }
    }
  }
}

class _Fixture {
  final session = _Session();
  final acknowledgements = <Map<Object?, Object?>>[];

  void signal(WidgetTester tester, String state) {
    tester.binding.channelBuffers.push(
      MethodChannelLifecycle.channelName,
      const StandardMethodCodec().encodeMethodCall(
        MethodCall('reportState', {'state': state}),
      ),
      (_) {},
    );
  }
}

Future<void> _frames(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump();
}

Future<_Fixture> _open(WidgetTester tester, {bool finishInitial = true}) async {
  final fixture = _Fixture();
  final harness = TestHarness();
  const channel = MethodChannel(MethodChannelLifecycle.channelName);
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(channel, (
    call,
  ) async {
    fixture.acknowledgements.add(
      Map<Object?, Object?>.from(call.arguments as Map),
    );
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
  expect(fixture.session.requests.map((r) => r.state), [
    LifecycleStateDto.resumed,
  ]);
  if (finishInitial) {
    fixture.session.finishPending();
    await _frames(tester);
    expect(fixture.acknowledgements, hasLength(1));
    fixture.session.requests.clear();
    fixture.acknowledgements.clear();
  }
  return fixture;
}

void main() {
  testWidgets(
    'pause reconciliation completes before later resume reaches Rust',
    (tester) async {
      final fixture = await _open(tester);
      fixture.signal(tester, 'paused');
      fixture.signal(tester, 'resumed');
      await _frames(tester);
      expect(fixture.session.requests.map((r) => r.state), [
        LifecycleStateDto.paused,
      ]);
      fixture.session.requests.single.completion.complete(LockEvent.locked);
      await _frames(tester);
      expect(fixture.session.requests.map((r) => r.state), [
        LifecycleStateDto.paused,
        LifecycleStateDto.resumed,
      ]);
      fixture.session.requests.last.completion.complete(LockEvent.locked);
      await _frames(tester);
      expect(fixture.acknowledgements.map((ack) => ack['state']), ['resumed']);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.android),
  );

  testWidgets(
    'outdated resumed reply cannot remove cover after pause arrives',
    (tester) async {
      final fixture = await _open(tester, finishInitial: false);
      fixture.signal(tester, 'paused');
      await _frames(tester);
      fixture.session.requests.first.completion.complete(LockEvent.locked);
      await _frames(tester);
      expect(
        fixture.acknowledgements,
        isEmpty,
        reason: 'native shield must not receive stale resumed acknowledgement',
      );
      expect(fixture.session.requests.last.state, LifecycleStateDto.paused);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.android),
  );

  testWidgets(
    'acknowledgement waits for Rust completion and rendered lock state',
    (tester) async {
      final fixture = await _open(tester);
      fixture.session.emitLockState(LockEvent.unlocked);
      await _frames(tester);
      fixture.signal(tester, 'paused');
      await _frames(tester);
      expect(fixture.acknowledgements, isEmpty);
      fixture.session.requests.single.completion.complete(LockEvent.locked);
      await _frames(tester);
      expect(fixture.acknowledgements, isEmpty);
      fixture.session.emitLockState(LockEvent.locked);
      await _frames(tester);
      expect(fixture.acknowledgements.single, containsPair('state', 'paused'));
      expect(
        fixture.acknowledgements.single,
        containsPair('lockState', 'locked'),
      );
    },
    variant: TargetPlatformVariant.only(TargetPlatform.android),
  );

  testWidgets('disposed app never acknowledges a late Rust result', (
    tester,
  ) async {
    final fixture = await _open(tester);
    fixture.signal(tester, 'paused');
    await _frames(tester);
    await tester.pumpWidget(const SizedBox.shrink());
    fixture.session.requests.single.completion.complete(LockEvent.locked);
    await _frames(tester);
    expect(fixture.acknowledgements, isEmpty);
    expect(tester.takeException(), isNull);
  }, variant: TargetPlatformVariant.only(TargetPlatform.android));
}
