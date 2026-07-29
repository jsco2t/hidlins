import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../bridge/dto.dart';
import '../providers/lock_state_provider.dart';
import 'lock_content.dart';

class LockGuard extends ConsumerWidget {
  const LockGuard({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final lockState = ref.watch(lockStateProvider);
    return lockState.when(
      data: (event) {
        if (event == LockEvent.locked) {
          return const Scaffold(body: LockContent(iconSize: 48));
        }
        return child;
      },
      loading: () => const Scaffold(body: LockContent(iconSize: 48)),
      error: (_, _) => const Scaffold(body: LockContent(iconSize: 48)),
    );
  }
}
