import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'repository_providers.dart';

final activityThrottleProvider = StateNotifierProvider<ActivityThrottle, int>((
  ref,
) {
  final repo = ref.watch(sessionRepositoryProvider);
  return ActivityThrottle(repo.reportActivity);
});

class ActivityThrottle extends StateNotifier<int> {
  ActivityThrottle(this._bridgeReportActivity) : super(0);

  final void Function() _bridgeReportActivity;
  final _stopwatch = Stopwatch();
  static const _minInterval = Duration(seconds: 1);

  void reportActivity() {
    if (_stopwatch.isRunning && _stopwatch.elapsed < _minInterval) return;
    _stopwatch
      ..reset()
      ..start();
    state++;
    _bridgeReportActivity();
  }
}
