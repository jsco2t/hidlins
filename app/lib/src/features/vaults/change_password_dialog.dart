import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/failures.dart';
import '../../l10n/app_localizations.dart';
import '../../providers/providers.dart';
import '../../ui/activity_capture.dart';
import '../../ui/tokens.dart';

class ChangePasswordDialog extends ConsumerStatefulWidget {
  const ChangePasswordDialog({super.key});

  @override
  ConsumerState<ChangePasswordDialog> createState() =>
      _ChangePasswordDialogState();
}

class _ChangePasswordDialogState extends ConsumerState<ChangePasswordDialog> {
  final _formKey = GlobalKey<FormState>();
  final _currentCtrl = TextEditingController();
  final _newCtrl = TextEditingController();
  final _confirmCtrl = TextEditingController();
  bool _working = false;
  String? _error;
  bool _registryRemediation = false;

  @override
  void dispose() {
    _currentCtrl.dispose();
    _newCtrl.dispose();
    _confirmCtrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final colorScheme = Theme.of(context).colorScheme;

    return Scaffold(
      appBar: AppBar(title: Text(l10n.vaultChangePasswordTitle)),
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
                        controller: _currentCtrl,
                        obscureText: true,
                        autocorrect: false,
                        enableSuggestions: false,
                        decoration: InputDecoration(
                          labelText: l10n.vaultChangePasswordCurrent,
                          border: const OutlineInputBorder(),
                        ),
                        validator: (v) => (v == null || v.isEmpty)
                            ? l10n.validatorRequired
                            : null,
                      ),
                      const SizedBox(height: HidlinsSpacing.md),
                      TextFormField(
                        controller: _newCtrl,
                        obscureText: true,
                        autocorrect: false,
                        enableSuggestions: false,
                        decoration: InputDecoration(
                          labelText: l10n.vaultChangePasswordNew,
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
                        autocorrect: false,
                        enableSuggestions: false,
                        decoration: InputDecoration(
                          labelText: l10n.vaultChangePasswordConfirm,
                          border: const OutlineInputBorder(),
                        ),
                        validator: (v) {
                          if (v != _newCtrl.text) {
                            return l10n.vaultPasswordMismatch;
                          }
                          return null;
                        },
                      ),
                      if (_error != null) ...[
                        const SizedBox(height: HidlinsSpacing.md),
                        Text(
                          _error!,
                          style: TextStyle(color: colorScheme.error),
                        ),
                      ],
                      if (_registryRemediation) ...[
                        const SizedBox(height: HidlinsSpacing.md),
                        Card(
                          color: colorScheme.errorContainer,
                          child: Padding(
                            padding: const EdgeInsets.all(HidlinsSpacing.md),
                            child: Row(
                              children: [
                                Icon(
                                  Icons.warning,
                                  color: colorScheme.onErrorContainer,
                                ),
                                const SizedBox(width: HidlinsSpacing.sm),
                                Expanded(
                                  child: Text(
                                    l10n.vaultChangePasswordReenterSync,
                                    style: TextStyle(
                                      color: colorScheme.onErrorContainer,
                                    ),
                                  ),
                                ),
                              ],
                            ),
                          ),
                        ),
                      ],
                      const SizedBox(height: HidlinsSpacing.lg),
                      FilledButton(
                        onPressed: _changePassword,
                        child: Text(l10n.actionConfirm),
                      ),
                    ],
                  ),
                ),
              ),
            ),
    );
  }

  Future<void> _changePassword() async {
    if (!_formKey.currentState!.validate()) return;

    setState(() {
      _working = true;
      _error = null;
      _registryRemediation = false;
    });

    try {
      final repo = ref.read(sessionRepositoryProvider);
      await repo.changeMasterPassword(_currentCtrl.text, _newCtrl.text);
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(
              AppLocalizations.of(context)!.vaultChangePasswordSuccess,
            ),
          ),
        );
        Navigator.of(context).pop(true);
      }
    } on AppFailure catch (e) {
      if (!mounted) return;
      final l10n = AppLocalizations.of(context)!;
      setState(() {
        _working = false;
        _error = switch (e) {
          BadCredentials() => l10n.lockScreenWrongPassword,
          InternalFailure(context: final ctx) when ctx == 'registry changed' =>
            () {
              _registryRemediation = true;
              return l10n.vaultChangePasswordReenterSync;
            }(),
          VaultIsBusy() => l10n.syncStatusInProgress,
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
