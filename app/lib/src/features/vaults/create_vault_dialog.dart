import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/failures.dart';
import '../../l10n/app_localizations.dart';
import '../../providers/providers.dart';
import '../../ui/activity_capture.dart';
import '../../ui/tokens.dart';

class CreateVaultDialog extends ConsumerStatefulWidget {
  const CreateVaultDialog({super.key});

  @override
  ConsumerState<CreateVaultDialog> createState() => _CreateVaultDialogState();
}

class _CreateVaultDialogState extends ConsumerState<CreateVaultDialog> {
  final _formKey = GlobalKey<FormState>();
  final _nameCtrl = TextEditingController();
  final _passwordCtrl = TextEditingController();
  final _confirmCtrl = TextEditingController();
  final _confirmPhraseCtrl = TextEditingController();
  bool _saving = false;
  String? _error;

  @override
  void dispose() {
    _nameCtrl.dispose();
    _passwordCtrl.dispose();
    _confirmCtrl.dispose();
    _confirmPhraseCtrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;

    return Scaffold(
      appBar: AppBar(title: Text(l10n.vaultCreateTitle)),
      body: _saving
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
                        validator: (v) => (v == null || v.isEmpty)
                            ? l10n.validatorRequired
                            : null,
                      ),
                      const SizedBox(height: HidlinsSpacing.md),
                      TextFormField(
                        controller: _passwordCtrl,
                        obscureText: true,
                        decoration: InputDecoration(
                          labelText: l10n.vaultCreatePasswordLabel,
                          border: const OutlineInputBorder(),
                        ),
                        validator: (v) => (v == null || v.isEmpty)
                            ? l10n.validatorRequired
                            : null,
                      ),
                      const SizedBox(height: HidlinsSpacing.md),
                      TextFormField(
                        controller: _confirmCtrl,
                        obscureText: true,
                        decoration: InputDecoration(
                          labelText: l10n.vaultCreateConfirmPasswordLabel,
                          border: const OutlineInputBorder(),
                        ),
                        validator: (v) {
                          if (v != _passwordCtrl.text) {
                            return l10n.vaultPasswordMismatch;
                          }
                          return null;
                        },
                      ),
                      const SizedBox(height: HidlinsSpacing.lg),

                      // No-recovery warning
                      Card(
                        color: Theme.of(context).colorScheme.errorContainer,
                        child: Padding(
                          padding: const EdgeInsets.all(HidlinsSpacing.md),
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Row(
                                children: [
                                  Icon(
                                    Icons.warning,
                                    color: Theme.of(
                                      context,
                                    ).colorScheme.onErrorContainer,
                                  ),
                                  const SizedBox(width: HidlinsSpacing.sm),
                                  Expanded(
                                    child: Text(
                                      l10n.vaultCreateNoRecoveryWarning,
                                      style: TextStyle(
                                        color: Theme.of(
                                          context,
                                        ).colorScheme.onErrorContainer,
                                      ),
                                    ),
                                  ),
                                ],
                              ),
                              const SizedBox(height: HidlinsSpacing.md),
                              TextFormField(
                                controller: _confirmPhraseCtrl,
                                decoration: InputDecoration(
                                  labelText: l10n.vaultCreateNoRecoveryConfirm,
                                  border: const OutlineInputBorder(),
                                ),
                                validator: (v) {
                                  if (v != l10n.vaultCreateNoRecoveryPhrase) {
                                    return l10n.vaultCreateNoRecoveryConfirm;
                                  }
                                  return null;
                                },
                              ),
                            ],
                          ),
                        ),
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
                        onPressed: _create,
                        child: Text(l10n.actionCreate),
                      ),
                    ],
                  ),
                ),
              ),
            ),
    );
  }

  Future<void> _create() async {
    if (!_formKey.currentState!.validate()) return;

    setState(() {
      _saving = true;
      _error = null;
    });

    final masterPassword = _passwordCtrl.text;
    _passwordCtrl.clear();
    _confirmCtrl.clear();
    try {
      final repo = ref.read(sessionRepositoryProvider);
      await repo.createVault(
        name: _nameCtrl.text,
        masterPassword: masterPassword,
        confirmedNoRecovery: true,
      );
      if (mounted) Navigator.of(context).pop(true);
    } on AppFailure catch (e) {
      setState(() {
        _saving = false;
        _error = switch (e) {
          PathAlreadyExists() => 'A vault with this name already exists',
          InvalidInputFailure(:final reason) => reason,
          _ => AppLocalizations.of(context)!.errorGeneric,
        };
      });
    } on Exception {
      setState(() {
        _saving = false;
        _error = AppLocalizations.of(context)!.errorGeneric;
      });
    }
  }
}
