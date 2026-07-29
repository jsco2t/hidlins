import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'data/models.dart';
import 'features/entries/entries_page.dart';
import 'features/generator/generator_page.dart';
import 'features/lock/unlock_screen.dart';
import 'features/search/search_page.dart';
import 'features/settings/settings_page.dart';
import 'providers/lock_state_provider.dart';
import 'providers/providers.dart';
import 'ui/adaptive_scaffold.dart';
import 'ui/lock_guard.dart';
import 'l10n/app_localizations.dart';

class _LockNotifier extends ChangeNotifier {
  bool _locked = true;
  bool get locked => _locked;

  void update(bool locked) {
    _locked = locked;
    notifyListeners();
  }
}

final _lockNotifierProvider = Provider<_LockNotifier>((ref) {
  final notifier = _LockNotifier();
  ref.listen(lockStateProvider, (_, next) {
    notifier.update(
      next.whenOrNull(data: (e) => e == LockEvent.locked) ?? true,
    );
  }, fireImmediately: true);
  ref.onDispose(notifier.dispose);
  return notifier;
});

final routerProvider = Provider<GoRouter>((ref) {
  final lockNotifier = ref.watch(_lockNotifierProvider);
  return GoRouter(
    initialLocation: '/lock',
    refreshListenable: lockNotifier,
    redirect: (context, state) {
      final isLocked = lockNotifier.locked;
      final goingToLock = state.matchedLocation == '/lock';
      if (isLocked && !goingToLock) return '/lock';
      if (!isLocked && goingToLock) return '/entries';
      return null;
    },
    routes: [
      GoRoute(path: '/lock', builder: (context, state) => const UnlockScreen()),
      ShellRoute(
        builder: (context, state, child) {
          return _ShellWrapper(location: state.matchedLocation, child: child);
        },
        routes: [
          GoRoute(
            path: '/entries',
            builder: (context, state) => const LockGuard(child: EntriesPage()),
            routes: [
              GoRoute(
                path: ':uuid',
                builder: (context, state) => LockGuard(
                  child: EntriesPage(initialUuid: state.pathParameters['uuid']),
                ),
              ),
            ],
          ),
          GoRoute(
            path: '/search',
            builder: (context, state) => const LockGuard(child: SearchPage()),
          ),
          GoRoute(
            path: '/generator',
            builder: (context, state) =>
                const LockGuard(child: GeneratorPage()),
          ),
          GoRoute(
            path: '/sync',
            builder: (context, state) => LockGuard(
              child: Builder(
                builder: (context) =>
                    Center(child: Text(AppLocalizations.of(context)!.navSync)),
              ),
            ),
          ),
          GoRoute(
            path: '/settings',
            builder: (context, state) => const LockGuard(child: SettingsPage()),
          ),
        ],
      ),
    ],
  );
});

class _ShellWrapper extends ConsumerWidget {
  const _ShellWrapper({required this.location, required this.child});

  final String location;
  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final index = _indexForLocation(location);
    final prefs = ref.watch(prefsProvider).valueOrNull;
    return AdaptiveScaffold(
      selectedIndex: index,
      onDestinationSelected: (i) {
        final path = _locationForIndex(i);
        context.go(path);
      },
      body: child,
      initialListPaneWidth: prefs?.listPaneWidth,
      onListPaneWidthChanged: (width) {
        unawaited(ref.read(prefsProvider.notifier).setListPaneWidth(width));
      },
    );
  }

  static int _indexForLocation(String location) {
    if (location.startsWith('/entries')) return 0;
    if (location.startsWith('/search')) return 1;
    if (location.startsWith('/generator')) return 2;
    if (location.startsWith('/sync')) return 3;
    if (location.startsWith('/settings')) return 4;
    return 0;
  }

  static String _locationForIndex(int index) {
    return switch (index) {
      0 => '/entries',
      1 => '/search',
      2 => '/generator',
      3 => '/sync',
      4 => '/settings',
      _ => '/entries',
    };
  }
}
