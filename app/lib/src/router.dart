import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import 'data/models.dart';
import 'features/entries/entries_page.dart';
import 'features/entries/entry_edit.dart';
import 'features/generator/generator_page.dart';
import 'features/lock/unlock_screen.dart';
import 'features/search/search_page.dart';
import 'features/settings/settings_page.dart';
import 'features/sync/sync_controller.dart';
import 'features/sync/sync_page.dart';
import 'providers/lock_state_provider.dart';
import 'providers/providers.dart';
import 'ui/adaptive_scaffold.dart';
import 'ui/desktop_shortcuts.dart';
import 'ui/lock_guard.dart';
import 'ui/tokens.dart';
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
    errorPageBuilder: (context, state) {
      final l10n = AppLocalizations.of(context)!;
      return _materialPage(
        state,
        Scaffold(
          appBar: AppBar(title: Text(l10n.errorPageTitle)),
          body: Center(
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Text(l10n.errorPageMessage),
                const SizedBox(height: 16),
                TextButton(
                  onPressed: () => context.go('/'),
                  child: Text(l10n.actionHome),
                ),
              ],
            ),
          ),
        ),
      );
    },
    routes: [
      GoRoute(
        path: '/lock',
        pageBuilder: (context, state) =>
            _materialPage(state, const UnlockScreen()),
      ),
      ShellRoute(
        pageBuilder: (context, state, child) => _materialPage(
          state,
          _ShellWrapper(location: state.matchedLocation, child: child),
        ),
        routes: [
          GoRoute(
            path: '/entries',
            pageBuilder: (context, state) =>
                _materialPage(state, const LockGuard(child: EntriesPage())),
            routes: [
              GoRoute(
                path: ':uuid',
                pageBuilder: (context, state) => _materialPage(
                  state,
                  LockGuard(
                    child: EntriesPage(
                      initialUuid: state.pathParameters['uuid'],
                    ),
                  ),
                ),
              ),
            ],
          ),
          GoRoute(
            path: '/search',
            pageBuilder: (context, state) =>
                _materialPage(state, const LockGuard(child: SearchPage())),
          ),
          GoRoute(
            path: '/generator',
            pageBuilder: (context, state) =>
                _materialPage(state, const LockGuard(child: GeneratorPage())),
          ),
          GoRoute(
            path: '/sync',
            pageBuilder: (context, state) =>
                _materialPage(state, const LockGuard(child: SyncPage())),
          ),
          GoRoute(
            path: '/settings',
            pageBuilder: (context, state) =>
                _materialPage(state, const LockGuard(child: SettingsPage())),
          ),
        ],
      ),
    ],
  );
});

MaterialPage<void> _materialPage(GoRouterState state, Widget child) {
  return MaterialPage<void>(
    key: state.pageKey,
    name: state.name ?? state.path,
    arguments: <String, String>{
      ...state.pathParameters,
      ...state.uri.queryParameters,
    },
    restorationId: state.pageKey.value,
    child: child,
  );
}

class _ShellWrapper extends ConsumerWidget {
  const _ShellWrapper({required this.location, required this.child});

  final String location;
  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final index = _indexForLocation(location);
    final prefs = ref.watch(prefsProvider).valueOrNull;
    final selectedEntry = ref.watch(selectedEntryProvider);
    final compactDetail =
        location.startsWith('/entries') &&
        selectedEntry != null &&
        MediaQuery.sizeOf(context).width < HidlinsBreakpoints.medium;
    return PopScope(
      canPop: !compactDetail,
      onPopInvokedWithResult: (didPop, _) {
        if (!didPop && compactDetail) {
          ref.read(selectedEntryProvider.notifier).select(null);
        }
      },
      child: DesktopShortcuts(
        onSearch: () => context.go('/search'),
        onNewEntry: () => unawaited(_newEntry(context, ref)),
        onLock: () => unawaited(ref.read(sessionRepositoryProvider).lockNow()),
        onCopyPassword: () => unawaited(_copySelectedPassword(context, ref)),
        onGenerator: () => context.go('/generator'),
        onDismiss: () => unawaited(Navigator.of(context).maybePop()),
        child: AdaptiveScaffold(
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
        ),
      ),
    );
  }

  Future<void> _newEntry(BuildContext context, WidgetRef ref) async {
    final syncState = await ref.read(syncControllerProvider.future);
    if (syncState.inFlight) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(AppLocalizations.of(context)!.syncMutationsDisabled),
          ),
        );
      }
      return;
    }
    final tree = await ref.read(vaultTreeProvider.future);
    if (!context.mounted) return;
    await Navigator.of(context).push<void>(
      MaterialPageRoute(
        builder: (_) => EntryEditDialog(groupUuid: tree.root.uuid),
      ),
    );
  }

  Future<void> _copySelectedPassword(
    BuildContext context,
    WidgetRef ref,
  ) async {
    final uuid = ref.read(selectedEntryProvider);
    if (uuid == null) return;
    final l10n = AppLocalizations.of(context)!;
    try {
      await ref
          .read(secretsRepositoryProvider)
          .copyEntryField(uuid, CopyField.password);
      if (!context.mounted) return;
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(l10n.copiedSnackbar(30))));
    } on Exception {
      if (!context.mounted) return;
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(l10n.errorCopyFailed)));
    }
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
