import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/failures.dart';
import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../providers/providers.dart';
import '../../ui/activity_capture.dart';
import '../../ui/tokens.dart';

class ConnectSyncDialog extends ConsumerStatefulWidget {
  const ConnectSyncDialog({super.key});

  @override
  ConsumerState<ConnectSyncDialog> createState() => _ConnectSyncDialogState();
}

class _ConnectSyncDialogState extends ConsumerState<ConnectSyncDialog> {
  final _formKey = GlobalKey<FormState>();
  final _nameCtrl = TextEditingController();
  final _bucketCtrl = TextEditingController();
  final _keyCtrl = TextEditingController();
  final _regionCtrl = TextEditingController();
  final _endpointCtrl = TextEditingController();
  final _accessKeyCtrl = TextEditingController();
  final _secretKeyCtrl = TextEditingController();
  final _passwordCtrl = TextEditingController();
  bool _pathStyle = false;
  bool _working = false;
  String? _error;

  @override
  void dispose() {
    _nameCtrl.dispose();
    _bucketCtrl.dispose();
    _keyCtrl.dispose();
    _regionCtrl.dispose();
    _endpointCtrl.dispose();
    _accessKeyCtrl.dispose();
    _secretKeyCtrl.dispose();
    _passwordCtrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;

    return Scaffold(
      appBar: AppBar(title: Text(l10n.vaultConnectSyncTitle)),
      body: _working
          ? const Center(child: CircularProgressIndicator())
          : SingleChildScrollView(
              padding: const EdgeInsets.all(HidlinsSpacing.lg),
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 400),
                child: Form(
                  key: _formKey,
                  onChanged: () => ActivityCapture.reportTextInput(context),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      TextFormField(
                        controller: _nameCtrl,
                        decoration: InputDecoration(
                          labelText: l10n.vaultCreateNameLabel,
                          border: const OutlineInputBorder(),
                        ),
                        validator: (v) => _requiredValidator(v, l10n),
                      ),
                      const SizedBox(height: HidlinsSpacing.md),
                      TextFormField(
                        controller: _bucketCtrl,
                        decoration: InputDecoration(
                          labelText: l10n.syncS3BucketLabel,
                          border: const OutlineInputBorder(),
                        ),
                        validator: (v) => _requiredValidator(v, l10n),
                      ),
                      const SizedBox(height: HidlinsSpacing.md),
                      TextFormField(
                        controller: _keyCtrl,
                        decoration: InputDecoration(
                          labelText: l10n.syncObjectKeyLabel,
                          border: const OutlineInputBorder(),
                        ),
                        validator: (v) => _requiredValidator(v, l10n),
                      ),
                      const SizedBox(height: HidlinsSpacing.md),
                      TextFormField(
                        controller: _regionCtrl,
                        decoration: InputDecoration(
                          labelText: l10n.syncRegionLabel,
                          border: const OutlineInputBorder(),
                        ),
                        validator: (v) => _requiredValidator(v, l10n),
                      ),
                      const SizedBox(height: HidlinsSpacing.md),
                      TextFormField(
                        controller: _endpointCtrl,
                        decoration: InputDecoration(
                          labelText: l10n.syncEndpointLabel,
                          border: const OutlineInputBorder(),
                        ),
                      ),
                      const SizedBox(height: HidlinsSpacing.sm),
                      CheckboxListTile(
                        value: _pathStyle,
                        onChanged: (v) =>
                            setState(() => _pathStyle = v ?? false),
                        title: Text(l10n.syncPathStyleLabel),
                        controlAffinity: ListTileControlAffinity.leading,
                        contentPadding: EdgeInsets.zero,
                      ),
                      const SizedBox(height: HidlinsSpacing.md),
                      TextFormField(
                        controller: _accessKeyCtrl,
                        decoration: InputDecoration(
                          labelText: l10n.syncAccessKeyIdLabel,
                          border: const OutlineInputBorder(),
                        ),
                        validator: (v) => _requiredValidator(v, l10n),
                      ),
                      const SizedBox(height: HidlinsSpacing.md),
                      TextFormField(
                        controller: _secretKeyCtrl,
                        obscureText: true,
                        autocorrect: false,
                        enableSuggestions: false,
                        decoration: InputDecoration(
                          labelText: l10n.syncSecretAccessKeyLabel,
                          border: const OutlineInputBorder(),
                        ),
                        validator: (v) => _requiredValidator(v, l10n),
                      ),
                      const SizedBox(height: HidlinsSpacing.lg),
                      TextFormField(
                        controller: _passwordCtrl,
                        obscureText: true,
                        autocorrect: false,
                        enableSuggestions: false,
                        decoration: InputDecoration(
                          labelText: l10n.vaultCreatePasswordLabel,
                          border: const OutlineInputBorder(),
                        ),
                        validator: (v) => _requiredValidator(v, l10n),
                      ),
                      if (_error != null) ...[
                        const SizedBox(height: HidlinsSpacing.md),
                        Text(
                          _error!,
                          style: TextStyle(
                            color: Theme.of(context).colorScheme.error,
                          ),
                        ),
                      ],
                      const SizedBox(height: HidlinsSpacing.lg),
                      FilledButton(
                        onPressed: _connect,
                        child: Text(l10n.actionConfirm),
                      ),
                    ],
                  ),
                ),
              ),
            ),
    );
  }

  static String? _requiredValidator(String? v, AppLocalizations l10n) =>
      (v == null || v.isEmpty) ? l10n.validatorRequired : null;

  Future<void> _connect() async {
    if (!_formKey.currentState!.validate()) return;

    setState(() {
      _working = true;
      _error = null;
    });

    try {
      final repo = ref.read(sessionRepositoryProvider);
      await repo.bootstrapFromRemote(
        name: _nameCtrl.text,
        config: S3ConfigDto(
          bucket: _bucketCtrl.text,
          key: _keyCtrl.text,
          region: _regionCtrl.text,
          endpoint: _endpointCtrl.text.isEmpty ? null : _endpointCtrl.text,
          pathStyle: _pathStyle,
          accessKeyId: _accessKeyCtrl.text,
          secretAccessKey: _secretKeyCtrl.text,
        ),
        masterPassword: _passwordCtrl.text,
      );
      if (mounted) Navigator.of(context).pop(true);
    } on AppFailure catch (e) {
      if (!mounted) return;
      final l10n = AppLocalizations.of(context)!;
      setState(() {
        _working = false;
        _error = switch (e) {
          BadCredentials() => l10n.lockScreenWrongPassword,
          SyncDuplicate(:final existingVault) => l10n.syncErrorDuplicate(
            existingVault,
          ),
          PathAlreadyExists() => l10n.errorVaultAlreadyExists,
          SyncUnreachable(:final endpoint) => l10n.syncErrorUnreachable(
            endpoint ?? 'remote',
          ),
          SyncAuthFailure() => l10n.syncErrorAuthFailed,
          _ => l10n.errorGeneric,
        };
      });
    } on Exception {
      if (!mounted) return;
      setState(() {
        _working = false;
        _error = AppLocalizations.of(context)!.errorGeneric;
      });
    }
  }
}
