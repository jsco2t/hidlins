import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/failures.dart';
import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../providers/providers.dart';
import '../../ui/activity_capture.dart';
import '../../ui/tokens.dart';
import 'sync_status_button.dart' show triggerSyncNow;

class SyncPage extends ConsumerWidget {
  const SyncPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    final syncState = ref.watch(syncUiStateProvider);

    return ListView(
      padding: const EdgeInsets.all(HidlinsSpacing.lg),
      children: [
        Text(l10n.syncTitle, style: Theme.of(context).textTheme.headlineSmall),
        const SizedBox(height: HidlinsSpacing.lg),
        Card(
          child: syncState.when(
            loading: () => const Padding(
              padding: EdgeInsets.all(HidlinsSpacing.md),
              child: LinearProgressIndicator(),
            ),
            error: (_, _) => ListTile(
              leading: const Icon(Icons.sync_problem),
              title: Text(l10n.syncStatusFailed),
            ),
            data: (state) => ListTile(
              leading: state.inFlight
                  ? const SizedBox.square(
                      dimension: 24,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : Icon(
                      state.lastFailure != null
                          ? Icons.sync_problem
                          : state.configured
                          ? Icons.cloud_done_outlined
                          : Icons.cloud_off_outlined,
                    ),
              title: Text(_statusLabel(l10n, state)),
              trailing: FilledButton.icon(
                key: const Key('sync-now-button'),
                onPressed: state.configured && !state.inFlight
                    ? () => triggerSyncNow(context, ref)
                    : null,
                icon: const Icon(Icons.sync),
                label: Text(l10n.syncNow),
              ),
            ),
          ),
        ),
        const SizedBox(height: HidlinsSpacing.lg),
        const SyncConfigurationForm(),
      ],
    );
  }

  static String _statusLabel(AppLocalizations l10n, SyncUiState state) {
    if (state.inFlight) return l10n.syncStatusInProgress;
    if (state.lastFailure != null) return l10n.syncStatusFailed;
    if (!state.configured) return l10n.syncNotConfigured;
    return l10n.syncStatusIdle;
  }
}

class SyncConfigurationForm extends ConsumerStatefulWidget {
  const SyncConfigurationForm({super.key});

  @override
  ConsumerState<SyncConfigurationForm> createState() =>
      _SyncConfigurationFormState();
}

class _SyncConfigurationFormState extends ConsumerState<SyncConfigurationForm> {
  final _formKey = GlobalKey<FormState>();
  final _bucketController = TextEditingController();
  final _keyController = TextEditingController();
  final _regionController = TextEditingController();
  final _endpointController = TextEditingController();
  final _accessKeyController = TextEditingController();
  final _secretKeyController = TextEditingController();
  bool _pathStyle = false;
  bool _saving = false;
  String? _error;

  @override
  void dispose() {
    _bucketController.dispose();
    _keyController.dispose();
    _regionController.dispose();
    _endpointController.dispose();
    _accessKeyController.dispose();
    _secretKeyController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final syncInFlight =
        ref.watch(syncUiStateProvider).valueOrNull?.inFlight ?? false;
    final enabled = !_saving && !syncInFlight;
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(HidlinsSpacing.lg),
        child: Form(
          key: _formKey,
          onChanged: () => ActivityCapture.reportTextInput(context),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                l10n.syncConfigureTitle,
                style: Theme.of(context).textTheme.titleMedium,
              ),
              const SizedBox(height: HidlinsSpacing.md),
              TextFormField(
                controller: _bucketController,
                enabled: enabled,
                decoration: _decoration(l10n.syncS3BucketLabel),
                validator: (value) => _required(value, l10n),
              ),
              const SizedBox(height: HidlinsSpacing.md),
              TextFormField(
                controller: _keyController,
                enabled: enabled,
                decoration: _decoration(l10n.syncObjectKeyLabel),
                validator: (value) => _required(value, l10n),
              ),
              const SizedBox(height: HidlinsSpacing.md),
              TextFormField(
                controller: _regionController,
                enabled: enabled,
                decoration: _decoration(l10n.syncRegionLabel),
                validator: (value) => _required(value, l10n),
              ),
              const SizedBox(height: HidlinsSpacing.md),
              TextFormField(
                controller: _endpointController,
                enabled: enabled,
                decoration: _decoration(l10n.syncEndpointLabel),
              ),
              CheckboxListTile(
                value: _pathStyle,
                onChanged: !enabled
                    ? null
                    : (value) => setState(() => _pathStyle = value ?? false),
                title: Text(l10n.syncPathStyleLabel),
                controlAffinity: ListTileControlAffinity.leading,
                contentPadding: EdgeInsets.zero,
              ),
              TextFormField(
                initialValue: l10n.syncCredentialSourceEncrypted,
                readOnly: true,
                decoration: _decoration(l10n.syncCredentialSourceLabel),
              ),
              const SizedBox(height: HidlinsSpacing.md),
              TextFormField(
                controller: _accessKeyController,
                enabled: enabled,
                autocorrect: false,
                decoration: _decoration(l10n.syncAccessKeyIdLabel),
                validator: (value) => _required(value, l10n),
              ),
              const SizedBox(height: HidlinsSpacing.md),
              TextFormField(
                key: const Key('sync-secret-key-field'),
                controller: _secretKeyController,
                enabled: enabled,
                obscureText: true,
                autocorrect: false,
                enableSuggestions: false,
                decoration: _decoration(l10n.syncSecretAccessKeyLabel),
                validator: (value) => _required(value, l10n),
              ),
              if (_error != null) ...[
                const SizedBox(height: HidlinsSpacing.md),
                Text(
                  _error!,
                  style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                    color: Theme.of(context).colorScheme.error,
                  ),
                ),
              ],
              const SizedBox(height: HidlinsSpacing.lg),
              Align(
                alignment: Alignment.centerRight,
                child: FilledButton(
                  key: const Key('save-sync-config'),
                  onPressed: enabled ? _save : null,
                  child: _saving
                      ? const SizedBox.square(
                          dimension: 18,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : Text(l10n.actionSave),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  InputDecoration _decoration(String label) {
    return InputDecoration(
      labelText: label,
      border: const OutlineInputBorder(),
    );
  }

  String? _required(String? value, AppLocalizations l10n) {
    return value == null || value.isEmpty ? l10n.validatorRequired : null;
  }

  Future<void> _save() async {
    if (!_formKey.currentState!.validate()) return;
    final config = S3ConfigDto(
      bucket: _bucketController.text,
      key: _keyController.text,
      region: _regionController.text,
      endpoint: _endpointController.text.isEmpty
          ? null
          : _endpointController.text,
      pathStyle: _pathStyle,
      accessKeyId: _accessKeyController.text,
      secretAccessKey: _secretKeyController.text,
    );
    setState(() {
      _saving = true;
      _error = null;
    });
    final operation = ref
        .read(syncUiStateProvider.notifier)
        .configureSync(config);
    _secretKeyController.clear();
    try {
      await operation;
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(AppLocalizations.of(context)!.syncConfigureSuccess),
        ),
      );
      setState(() => _saving = false);
    } on AppFailure catch (failure) {
      if (!mounted) return;
      setState(() {
        _saving = false;
        _error = _failureMessage(failure);
      });
    } on Object {
      if (!mounted) return;
      setState(() {
        _saving = false;
        _error = AppLocalizations.of(context)!.errorGeneric;
      });
    }
  }

  String _failureMessage(AppFailure failure) {
    final l10n = AppLocalizations.of(context)!;
    return switch (failure) {
      SyncDuplicate(:final existingVault) => l10n.syncErrorDuplicate(
        existingVault,
      ),
      SyncUnreachable(:final endpoint) => l10n.syncErrorUnreachable(
        endpoint ?? l10n.syncEndpointDefault,
      ),
      SyncAuthFailure() => l10n.syncErrorAuthFailed,
      _ => l10n.errorGeneric,
    };
  }
}
