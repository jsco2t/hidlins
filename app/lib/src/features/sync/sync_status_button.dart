import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/failures.dart';
import '../../l10n/app_localizations.dart';
import '../../providers/providers.dart';

Future<void> triggerSyncNow(BuildContext context, WidgetRef ref) async {
  try {
    await ref.read(syncUiStateProvider.notifier).syncNow();
  } on AppFailure catch (failure) {
    if (failure is! VaultIsBusy && failure is! VaultIsLocked) return;
    if (!context.mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(AppLocalizations.of(context)!.syncFailureGeneric)),
    );
  } on Object {
    if (!context.mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(AppLocalizations.of(context)!.syncFailureGeneric)),
    );
  }
}

class SyncStatusButton extends ConsumerWidget {
  const SyncStatusButton({super.key, required this.onNotConfigured});

  final VoidCallback onNotConfigured;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    final state = ref.watch(syncUiStateProvider);
    final value = state.valueOrNull;
    final inFlight = value?.inFlight ?? state.isLoading;
    final hasFailure = value?.lastFailure != null;
    final configured = value?.configured ?? false;

    final tooltip = inFlight
        ? l10n.syncStatusInProgress
        : hasFailure
        ? l10n.syncStatusFailed
        : configured
        ? l10n.syncNow
        : l10n.syncNotConfigured;

    return Tooltip(
      message: tooltip,
      child: IconButton(
        key: const Key('sync-status-button'),
        onPressed: inFlight
            ? null
            : configured
            ? () => triggerSyncNow(context, ref)
            : onNotConfigured,
        icon: inFlight
            ? const SizedBox.square(
                dimension: 20,
                child: CircularProgressIndicator(strokeWidth: 2),
              )
            : Icon(
                hasFailure
                    ? Icons.sync_problem
                    : configured
                    ? Icons.cloud_done_outlined
                    : Icons.cloud_off_outlined,
              ),
      ),
    );
  }
}
