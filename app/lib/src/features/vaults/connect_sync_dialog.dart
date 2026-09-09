import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../data/failures.dart';
import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../providers/providers.dart';
import '../../ui/activity_capture.dart';
import '../../ui/tokens.dart';

class ConnectSyncDialog extends ConsumerStatefulWidget {
  const ConnectSyncDialog({super.key, this.mobileOverride});

  final bool? mobileOverride;

  @override
  ConsumerState<ConnectSyncDialog> createState() => _ConnectSyncDialogState();
}

class _ConnectSyncDialogState extends ConsumerState<ConnectSyncDialog> {
  final _formKey = GlobalKey<FormState>();
  final _name = TextEditingController();
  final _peerName = TextEditingController(text: 'This device');
  final _manualEndpoint = TextEditingController();
  final _password = TextEditingController();
  bool _working = false;
  String? _error;

  bool get _isMobile =>
      widget.mobileOverride ??
      (!kIsWeb &&
          (defaultTargetPlatform == TargetPlatform.iOS ||
              defaultTargetPlatform == TargetPlatform.android));

  @override
  void dispose() {
    _name.dispose();
    _peerName.dispose();
    _manualEndpoint.dispose();
    _password.clear();
    _password.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Scaffold(
      appBar: AppBar(title: Text(l10n.syncImportPairedVault)),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(HidlinsSpacing.lg),
        child: Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 480),
            child: Form(
              key: _formKey,
              onChanged: () => ActivityCapture.reportTextInput(context),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Text(l10n.syncLocalOnlyDescription),
                  const SizedBox(height: HidlinsSpacing.lg),
                  TextFormField(
                    controller: _name,
                    decoration: InputDecoration(
                      labelText: l10n.vaultCreateNameLabel,
                      border: const OutlineInputBorder(),
                    ),
                    validator: _required,
                  ),
                  const SizedBox(height: HidlinsSpacing.md),
                  TextFormField(
                    controller: _peerName,
                    decoration: InputDecoration(
                      labelText: l10n.syncPeerName,
                      border: const OutlineInputBorder(),
                    ),
                    validator: _required,
                  ),
                  const SizedBox(height: HidlinsSpacing.md),
                  TextFormField(
                    controller: _password,
                    obscureText: true,
                    autocorrect: false,
                    enableSuggestions: false,
                    decoration: InputDecoration(
                      labelText: l10n.vaultCreatePasswordLabel,
                      border: const OutlineInputBorder(),
                    ),
                    validator: _required,
                  ),
                  const SizedBox(height: HidlinsSpacing.md),
                  ExpansionTile(
                    tilePadding: EdgeInsets.zero,
                    title: Text(l10n.syncManualEndpoint),
                    subtitle: Text(l10n.syncManualPolicy),
                    children: [
                      TextFormField(
                        controller: _manualEndpoint,
                        autocorrect: false,
                        enableSuggestions: false,
                        decoration: InputDecoration(
                          labelText: l10n.syncManualEndpointHint,
                          border: const OutlineInputBorder(),
                        ),
                      ),
                    ],
                  ),
                  if (_error != null) ...[
                    const SizedBox(height: HidlinsSpacing.md),
                    Text(
                      _error!,
                      key: const ValueKey('pair-import-error'),
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.error,
                      ),
                    ),
                  ],
                  const SizedBox(height: HidlinsSpacing.lg),
                  FilledButton.icon(
                    onPressed: _working ? null : _connect,
                    icon: _working
                        ? const SizedBox.square(
                            dimension: 18,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          )
                        : const Icon(Icons.download),
                    label: Text(
                      _working
                          ? l10n.syncDiscovering
                          : l10n.syncImportPairedVault,
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }

  String? _required(String? value) => value == null || value.trim().isEmpty
      ? AppLocalizations.of(context)!.validatorRequired
      : null;

  Future<void> _connect() async {
    if (!_formKey.currentState!.validate()) return;
    if (_isMobile && !await _showPermissionRationale()) return;
    setState(() {
      _working = true;
      _error = null;
    });
    final repository = ref.read(syncRepositoryProvider);
    try {
      if (_manualEndpoint.text.trim().isNotEmpty) {
        await repository.setManualEndpoint(
          _parseEndpoint(_manualEndpoint.text),
        );
      } else {
        final discovery = await repository.discover(DiscoveryKind.pairing);
        _requireCandidate(discovery);
      }
      final prompt = await repository.beginPairImport(
        name: _name.text.trim(),
        masterPassword: _password.text,
      );
      _password.clear();
      if (!mounted) return;
      final accepted = await _showSas(prompt);
      if (!mounted) return;
      final imported = await repository.confirmPairing(
        transactionHandle: prompt.transactionHandle,
        accepted: accepted,
        peerDisplayName: _peerName.text.trim(),
      );
      if (accepted && imported != null && mounted) {
        Navigator.of(context).pop(true);
        return;
      }
      if (mounted) setState(() => _working = false);
    } on AppFailure catch (failure) {
      _password.clear();
      if (mounted) {
        setState(() {
          _working = false;
          _error = _failureMessage(failure);
        });
      }
    } on Exception {
      _password.clear();
      if (mounted) {
        setState(() {
          _working = false;
          _error = AppLocalizations.of(context)!.errorGeneric;
        });
      }
    }
  }

  Future<bool> _showPermissionRationale() async {
    final l10n = AppLocalizations.of(context)!;
    return await showDialog<bool>(
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
        ) ??
        false;
  }

  Future<bool> _showSas(PairingPrompt prompt) async {
    final l10n = AppLocalizations.of(context)!;
    return await showDialog<bool>(
          context: context,
          barrierDismissible: false,
          builder: (context) => PopScope(
            canPop: false,
            child: AlertDialog(
              title: Text(l10n.syncCompareCode),
              content: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Semantics(
                    label: '${l10n.syncCompareCode}: ${prompt.sas}',
                    child: SelectableText(
                      prompt.sas,
                      key: const ValueKey('pair-import-sas'),
                      style: Theme.of(context).textTheme.headlineMedium,
                    ),
                  ),
                  const SizedBox(height: HidlinsSpacing.md),
                  Text(l10n.syncCompareCodeHelp),
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
        ) ??
        false;
  }

  String _failureMessage(AppFailure failure) {
    final l10n = AppLocalizations.of(context)!;
    return switch (failure) {
      BadCredentials() => l10n.lockScreenWrongPassword,
      PathAlreadyExists() => l10n.errorVaultAlreadyExists,
      PlatformOperationFailure(:final state) when state == 'denied' =>
        l10n.syncPermissionDenied,
      PlatformOperationFailure() => l10n.syncPermissionRestricted,
      UnsupportedPlatformFailure() => l10n.syncPermissionUnsupported,
      InvalidInputFailure(:final reason) => reason,
      SyncUnreachable() => l10n.syncNoDevices,
      SyncAuthFailure() => l10n.syncErrorAuthFailed,
      _ => l10n.errorGeneric,
    };
  }
}

void _requireCandidate(LocalDiscoveryStatus status) {
  if (status.permission == LocalDiscoveryPermission.denied) {
    throw const PlatformOperationFailure(
      capability: 'local network',
      state: 'denied',
    );
  }
  if (status.permission == LocalDiscoveryPermission.restricted) {
    throw const PlatformOperationFailure(
      capability: 'local network',
      state: 'restricted',
    );
  }
  if (status.permission == LocalDiscoveryPermission.unavailable) {
    throw const UnsupportedPlatformFailure('local network discovery');
  }
  if (status.candidates.isEmpty) throw const SyncUnreachable();
}

LocalEndpoint _parseEndpoint(String input) {
  final value = input.trim();
  final separator = value.lastIndexOf(':');
  if (separator < 1) {
    throw const InvalidInputFailure(
      field: 'endpoint',
      reason: 'Use a private IP address followed by :port.',
    );
  }
  var address = value.substring(0, separator);
  var scope = 0;
  if (address.startsWith('[') && address.endsWith(']')) {
    address = address.substring(1, address.length - 1);
    final scopeSeparator = address.lastIndexOf('%');
    if (scopeSeparator >= 0) {
      scope = int.tryParse(address.substring(scopeSeparator + 1)) ?? -1;
      address = address.substring(0, scopeSeparator);
    }
  }
  final port = int.tryParse(value.substring(separator + 1));
  if (port == null || scope < 0) {
    throw const InvalidInputFailure(
      field: 'endpoint',
      reason: 'Use a private IP address followed by :port.',
    );
  }
  return LocalEndpoint(address: address, port: port, scopeId: scope);
}
