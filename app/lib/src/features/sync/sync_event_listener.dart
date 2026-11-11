import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../data/failures.dart';
import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import 'sync_controller.dart';

/// One presentation owner for the shell, or a standalone Sync page.
class SyncEventListener extends ConsumerStatefulWidget {
  const SyncEventListener({super.key, required this.child});
  final Widget child;

  static bool isPresent(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<_SyncNoticeScope>() != null;

  @override
  ConsumerState<SyncEventListener> createState() => _SyncEventListenerState();
}

class _SyncEventListenerState extends ConsumerState<SyncEventListener> {
  int _handledRevision = 0;
  bool _offline = false;
  DialogRoute<void>? _conflict;

  @override
  void initState() {
    super.initState();
    ref.listenManual(syncControllerProvider, (_, next) {
      final value = next.asData?.value;
      if (value == null) return;
      if (value.terminalRevision == 0) {
        _handledRevision = 0;
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (!mounted) return;
          if (_offline) setState(() => _offline = false);
          _closeConflict();
        });
        return;
      }
      if (value.terminalRevision == _handledRevision) return;
      _handledRevision = value.terminalRevision;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted ||
            ref.read(syncControllerProvider).asData?.value.terminalRevision !=
                value.terminalRevision) {
          return;
        }
        _present(value);
      });
    }, fireImmediately: true);
  }

  @override
  void dispose() {
    // The root navigator may be locked while the shell is being removed.
    final route = _conflict;
    _conflict = null;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (route?.isActive == true) route!.navigator?.removeRoute(route);
    });
    super.dispose();
  }

  void _closeConflict() {
    final route = _conflict;
    _conflict = null;
    if (route?.isActive == true) route!.navigator?.removeRoute(route);
  }

  void _present(SyncViewState value) {
    final l10n = AppLocalizations.of(context)!;
    final failure = value.failure;
    if (failure is VaultIsLocked) return;
    setState(() => _offline = failure is SyncUnreachable);
    if (failure is SyncConflict) {
      if (_conflict?.isActive == true) return;
      final route = DialogRoute<void>(
        context: context,
        barrierDismissible: false,
        builder: (dialogContext) => PopScope(
          canPop: false,
          child: AlertDialog(
            title: Text(l10n.syncConflictTitle),
            content: Text(l10n.syncConflictMessage(failure.backupPath)),
            actions: [
              FilledButton(
                onPressed: () {
                  ref.read(syncControllerProvider.notifier).clearFailure();
                  Navigator.of(dialogContext).pop();
                  _conflict = null;
                },
                child: Text(l10n.actionClose),
              ),
            ],
          ),
        ),
      );
      _conflict = route;
      Navigator.of(context, rootNavigator: true).push(route);
      return;
    }
    final message = failure == null
        ? switch (value.lastOutcome) {
            SyncOutcomeDto.alreadyInSync => l10n.syncOutcomeAlreadyCurrent,
            SyncOutcomeDto.pushed => l10n.syncOutcomePushed,
            SyncOutcomeDto.fastReplaced => l10n.syncOutcomeFastReplaced,
            SyncOutcomeDto.merged => l10n.syncOutcomeMerged,
            _ => l10n.syncStatusSuccess,
          }
        : switch (failure) {
            SyncUnreachable() => l10n.syncOfflineNotice,
            SyncAuthFailure() => l10n.syncErrorAuthFailed,
            _ => l10n.errorGeneric,
          };
    ScaffoldMessenger.of(context)
      ..hideCurrentSnackBar()
      ..showSnackBar(SnackBar(content: Text(message)));
  }

  @override
  Widget build(BuildContext context) => _SyncNoticeScope(
    child: Column(
      children: [
        if (_offline)
          MaterialBanner(
            content: Text(AppLocalizations.of(context)!.syncOfflineNotice),
            actions: [
              TextButton(
                onPressed: () => setState(() => _offline = false),
                child: Text(AppLocalizations.of(context)!.actionClose),
              ),
            ],
          ),
        Expanded(child: widget.child),
      ],
    ),
  );
}

class _SyncNoticeScope extends InheritedWidget {
  const _SyncNoticeScope({required super.child});
  @override
  bool updateShouldNotify(_SyncNoticeScope oldWidget) => false;
}
