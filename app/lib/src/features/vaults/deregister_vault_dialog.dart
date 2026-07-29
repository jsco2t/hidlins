import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../l10n/app_localizations.dart';
import '../../providers/providers.dart';
import '../../ui/tokens.dart';

class DeregisterVaultDialog extends ConsumerStatefulWidget {
  const DeregisterVaultDialog({super.key, required this.vaultName});

  final String vaultName;

  @override
  ConsumerState<DeregisterVaultDialog> createState() =>
      _DeregisterVaultDialogState();
}

class _DeregisterVaultDialogState extends ConsumerState<DeregisterVaultDialog> {
  bool _deleteFile = false;
  bool _confirmedDeletion = false;
  bool _working = false;
  String? _error;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final colorScheme = Theme.of(context).colorScheme;

    return AlertDialog(
      title: Text(l10n.vaultDeregisterTitle),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(l10n.vaultDeregisterMessage),
          const SizedBox(height: HidlinsSpacing.md),
          CheckboxListTile(
            value: _deleteFile,
            onChanged: _working
                ? null
                : (v) => setState(() {
                    _deleteFile = v ?? false;
                    _confirmedDeletion = false;
                  }),
            title: Text(l10n.vaultDeregisterDeleteFile),
            controlAffinity: ListTileControlAffinity.leading,
            contentPadding: EdgeInsets.zero,
          ),
          if (_error != null)
            Padding(
              padding: const EdgeInsets.only(top: HidlinsSpacing.sm),
              child: Text(_error!, style: TextStyle(color: colorScheme.error)),
            ),
          if (_deleteFile && !_confirmedDeletion)
            Padding(
              padding: const EdgeInsets.only(top: HidlinsSpacing.sm),
              child: Card(
                color: colorScheme.errorContainer,
                child: Padding(
                  padding: const EdgeInsets.all(HidlinsSpacing.md),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          Icon(
                            Icons.warning,
                            color: colorScheme.onErrorContainer,
                          ),
                          const SizedBox(width: HidlinsSpacing.sm),
                          Expanded(
                            child: Text(
                              l10n.vaultDeregisterDeleteConfirm,
                              style: TextStyle(
                                color: colorScheme.onErrorContainer,
                              ),
                            ),
                          ),
                        ],
                      ),
                      const SizedBox(height: HidlinsSpacing.sm),
                      Align(
                        alignment: Alignment.centerRight,
                        child: FilledButton(
                          style: FilledButton.styleFrom(
                            backgroundColor: colorScheme.error,
                            foregroundColor: colorScheme.onError,
                          ),
                          onPressed: () =>
                              setState(() => _confirmedDeletion = true),
                          child: Text(l10n.actionConfirm),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: _working ? null : () => Navigator.of(context).pop(false),
          child: Text(l10n.actionCancel),
        ),
        if (_working)
          const SizedBox(
            width: 24,
            height: 24,
            child: CircularProgressIndicator(strokeWidth: 2),
          )
        else
          FilledButton(
            onPressed: (_deleteFile && !_confirmedDeletion)
                ? null
                : _deregister,
            child: Text(l10n.actionDelete),
          ),
      ],
    );
  }

  Future<void> _deregister() async {
    setState(() => _working = true);
    try {
      final repo = ref.read(sessionRepositoryProvider);
      await repo.deregisterVault(widget.vaultName, deleteFile: _deleteFile);
      if (mounted) Navigator.of(context).pop(true);
    } on Exception {
      if (mounted) {
        setState(() {
          _working = false;
          _error = AppLocalizations.of(context)!.errorGeneric;
        });
      }
    }
  }
}
