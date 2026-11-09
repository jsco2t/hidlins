import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../data/failures.dart';
import '../../data/models.dart';
import '../../data/repositories.dart';
import '../sync/foreground_lifecycle.dart';
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
  bool _progress = false;
  _ImportAttempt? _attempt;
  String? _error;

  bool get _isMobile =>
      widget.mobileOverride ??
      (!kIsWeb &&
          (defaultTargetPlatform == TargetPlatform.iOS ||
              defaultTargetPlatform == TargetPlatform.android));

  @override
  void dispose() {
    final attempt = _attempt;
    if (attempt != null) unawaited(attempt.lease.release());
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
                    icon: _progress
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

  bool _isCurrent(_ImportAttempt attempt) =>
      mounted &&
      identical(_attempt, attempt) &&
      !attempt.retired &&
      attempt.lease.isCurrent;

  Future<void> _connect() async {
    if (_working || !_formKey.currentState!.validate()) return;
    final attempt = _ImportAttempt(
      ref.read(syncRepositoryProvider),
      name: _name.text.trim(),
      peerName: _peerName.text.trim(),
      endpoint: _manualEndpoint.text.trim(),
      password: _password.text,
    );
    _password.clear();
    _attempt = attempt;
    setState(() {
      _working = true;
      _error = null;
    });
    attempt.lease = ref
        .read(syncForegroundLifecycleProvider)
        .acquire(attempt, () => _prepareCleanup(attempt));
    try {
      if (_isMobile) {
        final allowed = await _showPermissionRationale(attempt);
        if (!_isCurrent(attempt) || !allowed) return;
      }
      await attempt.lease.ready;
      if (!_isCurrent(attempt)) return;
      setState(() => _progress = true);
      attempt.startedNative = true;
      final repository = attempt.repository;
      if (attempt.endpoint.isNotEmpty) {
        await attempt.lease.track(
          repository.setManualEndpoint(_parseEndpoint(attempt.endpoint)),
        );
      } else {
        final discovery = await attempt.lease.track(
          repository.discover(DiscoveryKind.pairing),
        );
        if (!_isCurrent(attempt)) return;
        _requireCandidate(discovery);
      }
      if (!_isCurrent(attempt)) return;
      final prompt = await attempt.lease.track(
        repository
            .beginPairImport(
              name: attempt.name,
              masterPassword: attempt.takePassword(),
            )
            .then((prompt) async {
              // Keep exact-handle rejection inside the tracked native call, so a
              // replacement import waits until the old transaction is gone.
              if (!_isCurrent(attempt)) {
                await _reject(repository, prompt);
              } else {
                attempt.prompt = prompt;
              }
              return prompt;
            }),
      );
      if (!_isCurrent(attempt)) return;
      setState(() => _progress = false);
      final accepted = await _showSas(attempt, prompt);
      if (!_isCurrent(attempt)) return;
      attempt.prompt = null; // Confirmation now owns this handle.
      setState(() => _progress = true);
      final imported = await attempt.lease.track(
        repository.confirmPairing(
          transactionHandle: prompt.transactionHandle,
          accepted: accepted,
          peerDisplayName: attempt.peerName,
        ),
      );
      if (!mounted || !_isCurrent(attempt)) return;
      if (accepted && imported != null && ModalRoute.of(context)!.isCurrent) {
        Navigator.of(context).pop(true);
      }
    } on AppFailure catch (failure) {
      if (_isCurrent(attempt)) {
        setState(() => _error = _failureMessage(failure));
      }
    } on Exception {
      if (_isCurrent(attempt)) {
        setState(() => _error = AppLocalizations.of(context)!.errorGeneric);
      }
    } finally {
      await attempt.lease.release();
      if (mounted && identical(_attempt, attempt)) {
        _attempt = null;
        setState(() {
          _working = false;
          _progress = false;
        });
      }
    }
  }

  ForegroundCleanup _prepareCleanup(_ImportAttempt attempt) {
    attempt.retired = true;
    attempt.password = null;
    final prompt = attempt.prompt;
    attempt.prompt = null;
    final modal = attempt.modal;
    attempt.modal = null;
    // dispose can run while Navigator is updating its history. Remove only
    // our own still-active route after that synchronous update has finished.
    scheduleMicrotask(() {
      final navigator = modal?.navigator;
      if (navigator != null && navigator.mounted && modal!.isActive) {
        navigator.removeRoute(modal, false);
      }
    });
    return (mayCancelNative) async {
      if (prompt != null) await _reject(attempt.repository, prompt);
      if (!mayCancelNative || !attempt.startedNative) return;
      try {
        await attempt.repository.cancelSync();
      } finally {
        await attempt.repository.stopDiscovery();
      }
    };
  }

  Future<void> _reject(SyncRepository repository, PairingPrompt prompt) async {
    await repository
        .confirmPairing(
          transactionHandle: prompt.transactionHandle,
          accepted: false,
          peerDisplayName: '',
        )
        .onError((_, _) => null);
  }

  Future<bool> _showOwnedDialog(
    _ImportAttempt attempt, {
    required WidgetBuilder builder,
    bool barrierDismissible = true,
  }) async {
    final route = DialogRoute<bool>(
      context: context,
      builder: builder,
      barrierDismissible: barrierDismissible,
    );
    attempt.modal = route;
    final result = await Navigator.of(context, rootNavigator: true).push(route);
    if (identical(attempt.modal, route)) attempt.modal = null;
    return result ?? false;
  }

  Future<bool> _showPermissionRationale(_ImportAttempt attempt) async {
    final l10n = AppLocalizations.of(context)!;
    return await _showOwnedDialog(
      attempt,
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
  }

  Future<bool> _showSas(_ImportAttempt attempt, PairingPrompt prompt) async {
    final l10n = AppLocalizations.of(context)!;
    return await _showOwnedDialog(
      attempt,
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
    );
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

// Dart/FRB strings cannot be zeroized here. Minimize their retained references:
// the text intake is cleared before any await and this copy is taken at handoff.
class _ImportAttempt {
  _ImportAttempt(
    this.repository, {
    required this.name,
    required this.peerName,
    required this.endpoint,
    required this.password,
  });
  final SyncRepository repository;
  final String name;
  final String peerName;
  final String endpoint;
  String? password;
  late final SyncForegroundLease lease;
  PairingPrompt? prompt;
  DialogRoute<bool>? modal;
  bool startedNative = false;
  bool retired = false;

  String takePassword() {
    final value = password!;
    password = null;
    return value;
  }
}
