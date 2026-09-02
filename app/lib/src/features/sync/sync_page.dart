import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../data/failures.dart';
import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../ui/tokens.dart';
import 'sync_controller.dart';

class SyncPage extends ConsumerStatefulWidget {
  const SyncPage({super.key});

  @override
  ConsumerState<SyncPage> createState() => _SyncPageState();
}

class _SyncPageState extends ConsumerState<SyncPage> {
  String? _shownConflict;

  @override
  Widget build(BuildContext context) {
    ref.listen(syncControllerProvider, (previous, next) {
      final previousValue = previous?.valueOrNull;
      final value = next.valueOrNull;
      final failure = value?.failure;
      final outcome = value?.lastOutcome;
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

    final state = ref.watch(syncControllerProvider);
    final l10n = AppLocalizations.of(context)!;
    return Scaffold(
      body: SafeArea(
        child: state.when(
          loading: () => const Center(child: CircularProgressIndicator()),
          error: (_, _) => Center(child: Text(l10n.errorGeneric)),
          data: (value) => value.configured
              ? _ConfiguredSync(state: value)
              : _SyncConfigurationForm(
                  failure: value.failure,
                  onSave: (config) => ref
                      .read(syncControllerProvider.notifier)
                      .configure(config),
                ),
        ),
      ),
    );
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

class _ConfiguredSync extends ConsumerWidget {
  const _ConfiguredSync({required this.state});

  final SyncViewState state;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    return ListView(
      padding: const EdgeInsets.all(HidlinsSpacing.lg),
      children: [
        Text(l10n.navSync, style: Theme.of(context).textTheme.headlineSmall),
        const SizedBox(height: HidlinsSpacing.md),
        if (state.inFlight)
          Card(
            child: ListTile(
              leading: const SizedBox.square(
                dimension: 24,
                child: CircularProgressIndicator(strokeWidth: 2),
              ),
              title: Text(l10n.syncStatusInProgress),
              subtitle: Text(l10n.syncWorkspaceAvailable),
            ),
          )
        else
          Card(
            child: ListTile(
              leading: Icon(
                state.failure == null ? Icons.cloud_done : Icons.cloud_off,
              ),
              title: Text(
                state.failure == null
                    ? _statusMessage(context, state.lastOutcome)
                    : _failureMessage(context, state.failure!),
                key: state.failure == null
                    ? const ValueKey('sync-status')
                    : const ValueKey('sync-error'),
              ),
            ),
          ),
        const SizedBox(height: HidlinsSpacing.md),
        Align(
          alignment: Alignment.centerLeft,
          child: FilledButton.icon(
            onPressed: state.inFlight
                ? null
                : () => ref.read(syncControllerProvider.notifier).syncNow(),
            icon: const Icon(Icons.sync),
            label: Text(l10n.syncNow),
          ),
        ),
      ],
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

String _failureMessage(BuildContext context, AppFailure failure) {
  final l10n = AppLocalizations.of(context)!;
  return switch (failure) {
    SyncUnreachable(:final endpoint) => l10n.syncErrorUnreachable(
      endpoint ?? l10n.navSync,
    ),
    SyncAuthFailure() => l10n.syncErrorAuthFailed,
    SyncNotReady() => l10n.syncNotConfigured,
    SyncConflict() => l10n.syncConflictTitle,
    _ => l10n.syncStatusFailed,
  };
}

class _SyncConfigurationForm extends StatefulWidget {
  const _SyncConfigurationForm({required this.failure, required this.onSave});

  final AppFailure? failure;
  final Future<void> Function(S3ConfigDto) onSave;

  @override
  State<_SyncConfigurationForm> createState() => _SyncConfigurationFormState();
}

class _SyncConfigurationFormState extends State<_SyncConfigurationForm> {
  final _formKey = GlobalKey<FormState>();
  final _bucket = TextEditingController();
  final _key = TextEditingController();
  final _region = TextEditingController(text: 'us-east-1');
  final _endpoint = TextEditingController();
  final _accessKey = TextEditingController();
  final _secretKey = TextEditingController();
  bool _pathStyle = false;
  bool _saving = false;

  @override
  void dispose() {
    _bucket.dispose();
    _key.dispose();
    _region.dispose();
    _endpoint.dispose();
    _accessKey.dispose();
    _secretKey.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Form(
      key: _formKey,
      child: ListView(
        padding: const EdgeInsets.all(HidlinsSpacing.lg),
        children: [
          Text(
            l10n.syncConfigureTitle,
            style: Theme.of(context).textTheme.headlineSmall,
          ),
          const SizedBox(height: HidlinsSpacing.md),
          for (final field in <(TextEditingController, String, bool)>[
            (_bucket, l10n.syncS3BucketLabel, false),
            (_key, l10n.syncObjectKeyLabel, false),
            (_region, l10n.syncRegionLabel, false),
            (_endpoint, l10n.syncEndpointLabel, false),
            (_accessKey, l10n.syncAccessKeyIdLabel, false),
            (_secretKey, l10n.syncSecretAccessKeyLabel, true),
          ]) ...[
            TextFormField(
              controller: field.$1,
              obscureText: field.$3,
              autocorrect: false,
              enableSuggestions: false,
              decoration: InputDecoration(
                labelText: field.$2,
                border: const OutlineInputBorder(),
              ),
              validator: (value) {
                if (field.$1 == _endpoint) return null;
                return value == null || value.trim().isEmpty
                    ? l10n.validatorRequired
                    : null;
              },
            ),
            const SizedBox(height: HidlinsSpacing.sm),
          ],
          SwitchListTile(
            value: _pathStyle,
            onChanged: (value) => setState(() => _pathStyle = value),
            title: Text(l10n.syncPathStyleLabel),
          ),
          if (widget.failure != null)
            Text(
              _failureMessage(context, widget.failure!),
              key: const ValueKey('sync-error'),
              style: TextStyle(color: Theme.of(context).colorScheme.error),
            ),
          const SizedBox(height: HidlinsSpacing.md),
          Align(
            alignment: Alignment.centerLeft,
            child: FilledButton(
              onPressed: _saving ? null : _save,
              child: Text(l10n.syncSaveConfiguration),
            ),
          ),
        ],
      ),
    );
  }

  Future<void> _save() async {
    if (!_formKey.currentState!.validate()) return;
    final config = S3ConfigDto(
      bucket: _bucket.text.trim(),
      key: _key.text.trim(),
      region: _region.text.trim(),
      endpoint: _endpoint.text.trim().isEmpty ? null : _endpoint.text.trim(),
      pathStyle: _pathStyle,
      accessKeyId: _accessKey.text.trim(),
      secretAccessKey: _secretKey.text,
    );
    _secretKey.clear();
    setState(() => _saving = true);
    await widget.onSave(config);
    if (mounted) setState(() => _saving = false);
  }
}
