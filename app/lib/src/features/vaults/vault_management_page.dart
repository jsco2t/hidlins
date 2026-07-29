import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../providers/providers.dart';
import '../../ui/tokens.dart';
import 'change_password_dialog.dart';
import 'connect_sync_dialog.dart';
import 'create_vault_dialog.dart';
import 'deregister_vault_dialog.dart';

class VaultManagementPage extends ConsumerWidget {
  const VaultManagementPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    final vaults = ref.watch(vaultListProvider);

    return Scaffold(
      appBar: AppBar(title: Text(l10n.navVaults)),
      body: vaults.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (_, _) => Center(child: Text(l10n.errorGeneric)),
        data: (list) => _VaultList(vaults: list),
      ),
      floatingActionButton: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          FloatingActionButton.small(
            heroTag: 'connect-sync',
            onPressed: () => _openConnectSync(context, ref),
            tooltip: l10n.vaultConnectSyncTitle,
            child: const Icon(Icons.sync),
          ),
          const SizedBox(height: HidlinsSpacing.sm),
          FloatingActionButton(
            heroTag: 'create-vault',
            onPressed: () => _openCreate(context, ref),
            tooltip: l10n.vaultCreateTitle,
            child: const Icon(Icons.add),
          ),
        ],
      ),
    );
  }

  Future<void> _openCreate(BuildContext context, WidgetRef ref) async {
    final result = await Navigator.of(
      context,
    ).push<bool>(MaterialPageRoute(builder: (_) => const CreateVaultDialog()));
    if (result == true) ref.invalidate(vaultListProvider);
  }

  Future<void> _openConnectSync(BuildContext context, WidgetRef ref) async {
    final result = await Navigator.of(
      context,
    ).push<bool>(MaterialPageRoute(builder: (_) => const ConnectSyncDialog()));
    if (result == true) ref.invalidate(vaultListProvider);
  }
}

class _VaultList extends ConsumerWidget {
  const _VaultList({required this.vaults});

  final List<VaultSummary> vaults;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;

    if (vaults.isEmpty) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              l10n.emptyStateNoVaults,
              style: Theme.of(context).textTheme.bodyLarge,
            ),
            const SizedBox(height: HidlinsSpacing.sm),
            Text(
              l10n.emptyStateNoVaultsSubtitle,
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ),
          ],
        ),
      );
    }

    return ListView.builder(
      padding: const EdgeInsets.all(HidlinsSpacing.md),
      itemCount: vaults.length,
      itemBuilder: (context, index) => _VaultTile(vault: vaults[index]),
    );
  }
}

class _VaultTile extends ConsumerWidget {
  const _VaultTile({required this.vault});

  final VaultSummary vault;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    final colorScheme = Theme.of(context).colorScheme;

    return Card(
      child: ListTile(
        leading: Icon(Icons.shield_outlined, color: colorScheme.primary),
        title: Text(vault.name),
        subtitle: Row(
          children: [
            if (vault.hasSync)
              Padding(
                padding: const EdgeInsets.only(right: HidlinsSpacing.sm),
                child: Icon(
                  Icons.sync,
                  size: 16,
                  color: colorScheme.onSurfaceVariant,
                  semanticLabel: l10n.navSync,
                ),
              ),
            if (vault.hasKeyfile)
              Icon(
                Icons.vpn_key,
                size: 16,
                color: colorScheme.onSurfaceVariant,
                semanticLabel: l10n.lockScreenKeyfileLabel,
              ),
          ],
        ),
        trailing: PopupMenuButton<_VaultAction>(
          onSelected: (action) => _onAction(context, ref, action),
          itemBuilder: (_) => [
            PopupMenuItem(
              value: _VaultAction.changePassword,
              child: Text(l10n.vaultChangePasswordTitle),
            ),
            PopupMenuItem(
              value: _VaultAction.deregister,
              child: Text(l10n.vaultDeregisterTitle),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _onAction(
    BuildContext context,
    WidgetRef ref,
    _VaultAction action,
  ) async {
    switch (action) {
      case _VaultAction.changePassword:
        final result = await Navigator.of(context).push<bool>(
          MaterialPageRoute(builder: (_) => const ChangePasswordDialog()),
        );
        if (result == true) ref.invalidate(vaultListProvider);
      case _VaultAction.deregister:
        final result = await showDialog<bool>(
          context: context,
          builder: (_) => DeregisterVaultDialog(vaultName: vault.name),
        );
        if (result == true) ref.invalidate(vaultListProvider);
    }
  }
}

enum _VaultAction { changePassword, deregister }
