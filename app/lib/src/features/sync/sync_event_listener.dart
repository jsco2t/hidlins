import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../bridge/dto.dart' as bridge;
import '../../data/failures.dart';
import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../providers/providers.dart';
import '../../ui/tokens.dart';

class SyncEventListener extends ConsumerStatefulWidget {
  const SyncEventListener({super.key, required this.child});

  final Widget child;

  @override
  ConsumerState<SyncEventListener> createState() => _SyncEventListenerState();
}

class _SyncEventListenerState extends ConsumerState<SyncEventListener> {
  bool _conflictDialogShowing = false;

  @override
  Widget build(BuildContext context) {
    final showUnlockWarning = ref.watch(unlockSyncWarningProvider);
    ref.listen(syncEventsProvider, (_, next) {
      final event = next.valueOrNull;
      if (event != null) _present(event);
    });
    return Column(
      children: [
        if (showUnlockWarning)
          MaterialBanner(
            content: Text(
              AppLocalizations.of(context)!.lockScreenOfflineWarning,
            ),
            leading: const Icon(Icons.cloud_off),
            actions: [
              TextButton(
                onPressed: () {
                  ref.read(unlockSyncWarningProvider.notifier).clear();
                },
                child: Text(AppLocalizations.of(context)!.actionClose),
              ),
            ],
          ),
        Expanded(child: widget.child),
      ],
    );
  }

  void _present(SyncEvent event) {
    if (!mounted) return;
    switch (event) {
      case SyncEvent_Started() || SyncEvent_Activity():
        return;
      case SyncEvent_Done(:final field0):
        _showMessage(_outcomeMessage(field0));
      case SyncEvent_Failed(:final field0):
        final failure = mapApiError(field0);
        if (failure is VaultIsLocked) {
          return;
        } else if (failure case SyncConflict(:final backupPath)) {
          unawaited(_showConflict(backupPath));
        } else {
          _showMessage(_failureMessage(failure));
        }
    }
  }

  String _outcomeMessage(bridge.SyncOutcomeDto outcome) {
    final l10n = AppLocalizations.of(context)!;
    return switch (outcome) {
      bridge.SyncOutcomeDto_AlreadyInSync() => l10n.syncOutcomeAlreadyInSync,
      bridge.SyncOutcomeDto_Pushed() => l10n.syncOutcomePushed,
      bridge.SyncOutcomeDto_FastReplaced() => l10n.syncOutcomeFastReplaced,
      bridge.SyncOutcomeDto_Merged() => l10n.syncOutcomeMerged,
      bridge.SyncOutcomeDto_Unknown() => l10n.syncOutcomeUnknown,
    };
  }

  String _failureMessage(AppFailure failure) {
    final l10n = AppLocalizations.of(context)!;
    return switch (failure) {
      SyncUnreachable(:final endpoint) => l10n.syncErrorUnreachable(
        endpoint ?? l10n.syncEndpointDefault,
      ),
      SyncAuthFailure() => l10n.syncErrorAuthFailed,
      SyncDuplicate(:final existingVault) => l10n.syncErrorDuplicate(
        existingVault,
      ),
      _ => l10n.syncFailureGeneric,
    };
  }

  void _showMessage(String message) {
    final messenger = ScaffoldMessenger.of(context);
    messenger
      ..hideCurrentSnackBar()
      ..showSnackBar(SnackBar(content: Text(message)));
  }

  Future<void> _showConflict(String backupPath) async {
    if (_conflictDialogShowing) return;
    _conflictDialogShowing = true;
    final l10n = AppLocalizations.of(context)!;
    await showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (dialogContext) => AlertDialog(
        title: Text(l10n.syncConflictTitle),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(l10n.syncConflictMessage),
            const SizedBox(height: HidlinsSpacing.md),
            Text(
              l10n.syncConflictBackupLabel,
              style: Theme.of(dialogContext).textTheme.labelMedium,
            ),
            const SizedBox(height: HidlinsSpacing.xs),
            SelectableText(backupPath),
          ],
        ),
        actions: [
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: Text(l10n.actionClose),
          ),
        ],
      ),
    );
    _conflictDialogShowing = false;
  }
}
