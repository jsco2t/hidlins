import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../bridge/dto.dart';
import '../data/repositories.dart';
import 'repository_providers.dart';

final lockStateProvider = StreamProvider<LockEvent>((ref) {
  final repo = ref.watch(sessionRepositoryProvider);
  return failClosedLockEvents(repo);
});

const _droppedEventPollInterval = Duration(milliseconds: 250);
const _lockStreamRetryInterval = Duration(seconds: 1);

Stream<LockEvent> failClosedLockEvents(SessionRepository repo) {
  late final StreamController<LockEvent> controller;
  StreamSubscription<LockEvent>? subscription;
  Timer? droppedEventTimer;
  Timer? retryTimer;
  var stopped = false;
  var resubscribing = false;
  var lastDroppedEvents = BigInt.zero;
  LockEvent? lastEmittedEvent;

  void emit(LockEvent event) {
    if (!stopped && !controller.isClosed && lastEmittedEvent != event) {
      lastEmittedEvent = event;
      controller.add(event);
    }
  }

  void emitLocked() => emit(LockEvent.locked);

  late final void Function() subscribe;

  void scheduleResubscribe() {
    if (stopped || retryTimer != null) return;
    retryTimer = Timer(_lockStreamRetryInterval, () {
      retryTimer = null;
      subscribe();
    });
  }

  Future<void> resubscribeNow() async {
    if (stopped || resubscribing) return;
    resubscribing = true;
    final previous = subscription;
    subscription = null;
    await previous?.cancel();
    resubscribing = false;
    subscribe();
  }

  void failClosedAndResubscribe() {
    emitLocked();
    unawaited(resubscribeNow());
  }

  void checkDroppedEvents() {
    if (stopped) return;
    try {
      final current = repo.droppedLockEvents();
      if (current > lastDroppedEvents) {
        lastDroppedEvents = current;
        failClosedAndResubscribe();
      }
    } on Object {
      failClosedAndResubscribe();
    }
  }

  subscribe = () {
    if (stopped || subscription != null || resubscribing) return;
    try {
      lastDroppedEvents = repo.droppedLockEvents();
      subscription = repo.lockEvents().listen(
        (event) {
          checkDroppedEvents();
          if (!resubscribing && !stopped) {
            emit(event);
          }
        },
        onError: (Object _, StackTrace _) {
          failClosedAndResubscribe();
        },
        onDone: () {
          subscription = null;
          emitLocked();
          scheduleResubscribe();
        },
      );
    } on Object {
      emitLocked();
      scheduleResubscribe();
    }
  };

  controller = StreamController<LockEvent>(
    onListen: () {
      subscribe();
      droppedEventTimer = Timer.periodic(
        _droppedEventPollInterval,
        (_) => checkDroppedEvents(),
      );
    },
    onCancel: () async {
      stopped = true;
      droppedEventTimer?.cancel();
      retryTimer?.cancel();
      await subscription?.cancel();
    },
  );
  return controller.stream;
}
