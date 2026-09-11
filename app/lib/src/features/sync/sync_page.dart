import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../data/failures.dart';
import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../ui/tokens.dart';
import 'sync_controller.dart';

class SyncPage extends ConsumerStatefulWidget {
  const SyncPage({super.key, this.desktopOverride});

  final bool? desktopOverride;

  @override
  ConsumerState<SyncPage> createState() => _SyncPageState();
}

class _SyncPageState extends ConsumerState<SyncPage> {
  final _manualEndpoint = TextEditingController();
  late final SyncController _controller;
  String? _shownConflict;
  String? _shownPairing;

  bool get _isDesktop =>
      widget.desktopOverride ??
      (!kIsWeb &&
          (defaultTargetPlatform == TargetPlatform.macOS ||
              defaultTargetPlatform == TargetPlatform.linux ||
              defaultTargetPlatform == TargetPlatform.windows));

  @override
  void initState() {
    super.initState();
    _controller = ref.read(syncControllerProvider.notifier);
  }

  @override
  void dispose() {
    _manualEndpoint.clear();
    _manualEndpoint.dispose();
    unawaited(_controller.cancelForegroundOperations(updateState: false));
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    ref.listen(syncControllerProvider, (previous, next) {
      final previousValue = previous?.valueOrNull;
      final value = next.valueOrNull;
      final failure = value?.failure;
      final outcome = value?.lastOutcome;
      final prompt = value?.prompt;
      if (prompt != null && _shownPairing != prompt.transactionHandle) {
        _shownPairing = prompt.transactionHandle;
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted) unawaited(_showPairing(prompt));
        });
      }
      if (failure case SyncConflict(:final backupPath)) {
        if (_shownConflict != backupPath) {
          _shownConflict = backupPath;
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (mounted) unawaited(_showConflict(backupPath));
          });
        }
      } else if (previousValue?.inFlight == true &&
          value?.inFlight == false &&
          outcome != null) {
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (!mounted) return;
          ScaffoldMessenger.of(context)
              .showSnackBar(SnackBar(content: Text(_outcomeMessage(outcome))));
        });
      }
    });

    final value = ref.watch(syncControllerProvider);
    final l10n = AppLocalizations.of(context)!;
    return Scaffold(
      body: SafeArea(
        child: value.when(
          loading: () => const Center(child: CircularProgressIndicator()),
          error: (_, _) => Center(child: Text(l10n.errorGeneric)),
          data: (state) => _SyncContent(
            state: state,
            isDesktop: _isDesktop,
            manualEndpoint: _manualEndpoint,
            onPair: () => _withPermissionRationale(
              () => ref.read(syncControllerProvider.notifier).pairThisVault(),
            ),
            onStartServer: () =>
                ref.read(syncControllerProvider.notifier).startServer(),
            onUseManual: () => ref
                .read(syncControllerProvider.notifier)
                .useManualEndpoint(_manualEndpoint.text),
            onRetryDiscovery: () => _withPermissionRationale(
              () => ref
                  .read(syncControllerProvider.notifier)
                  .discover(DiscoveryKind.pairing),
            ),
            onOpenSettings: () => ref
                .read(syncControllerProvider.notifier)
                .openDiscoverySettings(),
          ),
        ),
      ),
    );
  }

  Future<void> _withPermissionRationale(Future<void> Function() action) async {
    if (_isDesktop) {
      await action();
      return;
    }
    final l10n = AppLocalizations.of(context)!;
    final proceed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(l10n.syncPermissionTitle),
        content: Text(l10n.syncPermissionRationale),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: Text(l10n.actionCancel),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: Text(l10n.actionConfirm),
          ),
        ],
      ),
    );
    if (proceed == true) await action();
  }

  Future<void> _showPairing(PairingPrompt prompt) async {
    final l10n = AppLocalizations.of(context)!;
    final peerName = TextEditingController(text: 'Nearby Hidlins');
    final accepted = await showDialog<bool>(
      context: context,
      barrierDismissible: false,
      builder: (context) => PopScope(
        canPop: false,
        child: AlertDialog(
          title: Text(l10n.syncCompareCode),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Semantics(
                label: '${l10n.syncCompareCode}: ${prompt.sas}',
                child: SelectableText(
                  prompt.sas,
                  key: const ValueKey('pairing-sas'),
                  textAlign: TextAlign.center,
                  style: Theme.of(context).textTheme.headlineMedium,
                ),
              ),
              const SizedBox(height: HidlinsSpacing.md),
              Text(l10n.syncCompareCodeHelp),
              const SizedBox(height: HidlinsSpacing.md),
              TextField(
                controller: peerName,
                decoration: InputDecoration(
                  labelText: l10n.syncPeerName,
                  border: const OutlineInputBorder(),
                ),
              ),
            ],
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(context, false),
              child: Text(l10n.syncRejectPairing),
            ),
            FilledButton(
              onPressed: () => Navigator.pop(context, true),
              child: Text(l10n.syncConfirmMatches),
            ),
          ],
        ),
      ),
    );
    final name = peerName.text;
    peerName.clear();
    peerName.dispose();
    if (!mounted) return;
    await ref
        .read(syncControllerProvider.notifier)
        .confirmPairing(accepted: accepted == true, peerName: name);
    _shownPairing = null;
  }

  String _outcomeMessage(SyncOutcomeDto outcome) {
    final l10n = AppLocalizations.of(context)!;
    return switch (outcome) {
      SyncOutcomeDto.alreadyInSync => l10n.syncOutcomeAlreadyCurrent,
      SyncOutcomeDto.pushed => l10n.syncOutcomePushed,
      SyncOutcomeDto.fastReplaced => l10n.syncOutcomeFastReplaced,
      SyncOutcomeDto.merged => l10n.syncOutcomeMerged,
      SyncOutcomeDto.unknown => l10n.syncStatusSuccess,
    };
  }

  Future<void> _showConflict(String backupPath) async {
    final l10n = AppLocalizations.of(context)!;
    await showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (context) => PopScope(
        canPop: false,
        child: AlertDialog(
          title: Text(l10n.syncConflictTitle),
          content: Text(l10n.syncConflictMessage(backupPath)),
          actions: [
            FilledButton(
              onPressed: () {
                ref.read(syncControllerProvider.notifier).clearFailure();
                _shownConflict = null;
                Navigator.of(context).pop();
              },
              child: Text(l10n.actionClose),
            ),
          ],
        ),
      ),
    );
  }
}

class _SyncContent extends ConsumerWidget {
  const _SyncContent({
    required this.state,
    required this.isDesktop,
    required this.manualEndpoint,
    required this.onPair,
    required this.onStartServer,
    required this.onUseManual,
    required this.onRetryDiscovery,
    required this.onOpenSettings,
  });

  final SyncViewState state;
  final bool isDesktop;
  final TextEditingController manualEndpoint;
  final Future<void> Function() onPair;
  final Future<void> Function() onStartServer;
  final Future<void> Function() onUseManual;
  final Future<void> Function() onRetryDiscovery;
  final Future<void> Function() onOpenSettings;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    final notifier = ref.read(syncControllerProvider.notifier);
    return ListView(
      padding: const EdgeInsets.all(HidlinsSpacing.lg),
      children: [
        Text(
          l10n.syncConfigureTitle,
          style: Theme.of(context).textTheme.headlineSmall,
        ),
        const SizedBox(height: HidlinsSpacing.sm),
        Text(l10n.syncLocalOnlyDescription),
        const SizedBox(height: HidlinsSpacing.md),
        _StatusCard(state: state),
        if (state.working || state.inFlight) ...[
          const SizedBox(height: HidlinsSpacing.md),
          Card(
            child: ListTile(
              leading: const SizedBox.square(
                dimension: 24,
                child: CircularProgressIndicator(strokeWidth: 2),
              ),
              title: Text(
                state.inFlight
                    ? l10n.syncStatusInProgress
                    : l10n.syncDiscovering,
              ),
              subtitle: Text(l10n.syncWorkspaceAvailable),
              trailing: TextButton(
                onPressed: notifier.cancelForegroundOperations,
                child: Text(l10n.syncCancelForeground),
              ),
            ),
          ),
        ],
        if (state.failure != null) ...[
          const SizedBox(height: HidlinsSpacing.md),
          _FailureCard(
            failure: state.failure!,
            onRetry: onRetryDiscovery,
            onOpenSettings: onOpenSettings,
          ),
        ],
        const SizedBox(height: HidlinsSpacing.md),
        Wrap(
          spacing: HidlinsSpacing.sm,
          runSpacing: HidlinsSpacing.sm,
          children: [
            if (state.status.role != LocalSyncRole.server &&
                !state.status.paired)
              FilledButton.icon(
                onPressed: state.working ? null : onPair,
                icon: const Icon(Icons.link),
                label: Text(l10n.syncPairVault),
              ),
            if (state.status.role == LocalSyncRole.client &&
                state.status.paired)
              FilledButton.icon(
                onPressed: state.inFlight ? null : notifier.syncNow,
                icon: const Icon(Icons.sync),
                label: Text(l10n.syncNow),
              ),
            if (isDesktop && state.status.role != LocalSyncRole.client)
              OutlinedButton.icon(
                onPressed: state.status.serverRunning
                    ? notifier.stopServer
                    : onStartServer,
                icon: Icon(
                  state.status.serverRunning ? Icons.stop_circle : Icons.dns,
                ),
                label: Text(
                  state.status.serverRunning
                      ? l10n.syncStopServer
                      : state.status.configured
                      ? l10n.syncStartServer
                      : l10n.syncConfigureServer,
                ),
              ),
            if (isDesktop && state.status.serverRunning)
              OutlinedButton.icon(
                onPressed: notifier.openPairingWindow,
                icon: const Icon(Icons.timer),
                label: Text(l10n.syncAllowPairing),
              ),
          ],
        ),
        if (state.pairingSecondsRemaining > 0) ...[
          const SizedBox(height: HidlinsSpacing.sm),
          Semantics(
            liveRegion: true,
            child: Text(
              l10n.syncPairingRemaining(state.pairingSecondsRemaining),
              key: const ValueKey('pairing-countdown'),
            ),
          ),
        ],
        const SizedBox(height: HidlinsSpacing.lg),
        ExpansionTile(
          title: Text(l10n.syncManualEndpoint),
          subtitle: Text(l10n.syncManualPolicy),
          childrenPadding: const EdgeInsets.only(bottom: HidlinsSpacing.md),
          children: [
            TextField(
              controller: manualEndpoint,
              autocorrect: false,
              enableSuggestions: false,
              decoration: InputDecoration(
                labelText: l10n.syncManualEndpointHint,
                border: const OutlineInputBorder(),
              ),
            ),
            const SizedBox(height: HidlinsSpacing.sm),
            Align(
              alignment: Alignment.centerLeft,
              child: OutlinedButton(
                onPressed: onUseManual,
                child: Text(l10n.actionConfirm),
              ),
            ),
          ],
        ),
        if (state.status.configured)
          ExpansionTile(
            initiallyExpanded: state.peers.isNotEmpty,
            title: Text(l10n.syncManagePeers),
            children: [
              if (state.peers.isEmpty)
                ListTile(title: Text(l10n.syncNoDevices))
              else
                for (final peer in state.peers) _PeerTile(peer: peer),
            ],
          ),
      ],
    );
  }
}

class _StatusCard extends StatelessWidget {
  const _StatusCard({required this.state});

  final SyncViewState state;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final status = state.status;
    final title = switch (status.role) {
      LocalSyncRole.server => l10n.syncRoleServer,
      LocalSyncRole.client when status.paired => l10n.syncRoleClient,
      _ => l10n.syncNotConfigured,
    };
    final subtitle = status.serverRunning
        ? l10n.syncServerRunning
        : _statusMessage(context, state.lastOutcome);
    return Card(
      child: ListTile(
        leading: Icon(
          status.serverRunning
              ? Icons.lan
              : status.paired
              ? Icons.devices
              : Icons.link_off,
        ),
        title: Text(title, key: const ValueKey('sync-status')),
        subtitle: Text(subtitle),
      ),
    );
  }

  static String _statusMessage(BuildContext context, SyncOutcomeDto? outcome) {
    final l10n = AppLocalizations.of(context)!;
    return switch (outcome) {
      null => l10n.syncStatusIdle,
      SyncOutcomeDto.alreadyInSync => l10n.syncOutcomeAlreadyCurrent,
      SyncOutcomeDto.pushed => l10n.syncOutcomePushed,
      SyncOutcomeDto.fastReplaced => l10n.syncOutcomeFastReplaced,
      SyncOutcomeDto.merged => l10n.syncOutcomeMerged,
      SyncOutcomeDto.unknown => l10n.syncStatusSuccess,
    };
  }
}

class _FailureCard extends StatelessWidget {
  const _FailureCard({
    required this.failure,
    required this.onRetry,
    required this.onOpenSettings,
  });

  final AppFailure failure;
  final Future<void> Function() onRetry;
  final Future<void> Function() onOpenSettings;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final permissionFailure = failure is PlatformOperationFailure;
    return Card(
      color: Theme.of(context).colorScheme.errorContainer,
      child: ListTile(
        leading: const Icon(Icons.warning_amber),
        title: Text(
          _failureMessage(context, failure),
          key: const ValueKey('sync-error'),
        ),
        subtitle: permissionFailure
            ? Wrap(
                spacing: HidlinsSpacing.sm,
                children: [
                  TextButton(
                    onPressed: onRetry,
                    child: Text(l10n.syncRetryDiscovery),
                  ),
                  TextButton(
                    onPressed: onOpenSettings,
                    child: Text(l10n.syncOpenSettings),
                  ),
                ],
              )
            : null,
      ),
    );
  }
}

class _PeerTile extends ConsumerWidget {
  const _PeerTile({required this.peer});

  final SyncPeer peer;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    return ListTile(
      enabled: !peer.revoked,
      leading: Icon(peer.revoked ? Icons.block : Icons.devices),
      title: Text(peer.displayName),
      trailing: Wrap(
        children: [
          IconButton(
            tooltip: l10n.syncRenamePeer,
            onPressed: peer.revoked ? null : () => _rename(context, ref),
            icon: const Icon(Icons.edit),
          ),
          IconButton(
            tooltip: l10n.syncRevokePeer,
            onPressed: peer.revoked ? null : () => _revoke(context, ref),
            icon: const Icon(Icons.link_off),
          ),
        ],
      ),
    );
  }

  Future<void> _rename(BuildContext context, WidgetRef ref) async {
    final l10n = AppLocalizations.of(context)!;
    final controller = TextEditingController(text: peer.displayName);
    final value = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(l10n.syncRenamePeer),
        content: TextField(
          controller: controller,
          decoration: InputDecoration(labelText: l10n.syncPeerName),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: Text(l10n.actionCancel),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, controller.text),
            child: Text(l10n.actionSave),
          ),
        ],
      ),
    );
    await WidgetsBinding.instance.endOfFrame;
    controller.clear();
    controller.dispose();
    if (value != null && value.trim().isNotEmpty) {
      await ref
          .read(syncControllerProvider.notifier)
          .renamePeer(peer.peerId, value.trim());
    }
  }

  Future<void> _revoke(BuildContext context, WidgetRef ref) async {
    final l10n = AppLocalizations.of(context)!;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(l10n.syncRevokePeer),
        content: Text(l10n.syncRevokeConfirm),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: Text(l10n.actionCancel),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: Text(l10n.syncRevokePeer),
          ),
        ],
      ),
    );
    if (confirmed == true) {
      await ref.read(syncControllerProvider.notifier).revokePeer(peer.peerId);
    }
  }
}

String _failureMessage(BuildContext context, AppFailure failure) {
  final l10n = AppLocalizations.of(context)!;
  return switch (failure) {
    PlatformOperationFailure(:final state) when state == 'denied' =>
      l10n.syncPermissionDenied,
    PlatformOperationFailure() => l10n.syncPermissionRestricted,
    UnsupportedPlatformFailure() => l10n.syncPermissionUnsupported,
    InvalidInputFailure(:final reason) => reason,
    SyncUnreachable() => l10n.syncNoDevices,
    SyncAuthFailure() => l10n.syncErrorAuthFailed,
    SyncNotReady() => l10n.syncNotConfigured,
    SyncConflict() => l10n.syncConflictTitle,
    _ => l10n.syncStatusFailed,
  };
}
