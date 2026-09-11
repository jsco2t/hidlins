import 'dart:async';

import 'package:material_ui/material_ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/failures.dart';
import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../platform/keyfile_access.dart';
import '../../platform/platform_result.dart';
import '../../platform/vault_import.dart';
import '../../providers/providers.dart';
import '../../ui/activity_capture.dart';
import '../../ui/tokens.dart';
import '../../ui/widgets/brand_mark.dart';
import '../vaults/connect_sync_dialog.dart';
import '../vaults/create_vault_dialog.dart';
import '../vaults/first_run_page.dart';

enum _UnlockState { idle, unlocking, syncing }

class UnlockScreen extends ConsumerStatefulWidget {
  const UnlockScreen({super.key});

  @override
  ConsumerState<UnlockScreen> createState() => _UnlockScreenState();
}

class _UnlockScreenState extends ConsumerState<UnlockScreen> {
  final _passwordController = TextEditingController();
  final _passwordFocusNode = FocusNode();
  _UnlockState _state = _UnlockState.idle;
  String? _errorMessage;
  String? _selectedVault;
  KeyfileReference? _keyfileReference;
  bool _obscurePassword = true;
  bool _showOfflineWarning = false;

  @override
  void dispose() {
    _passwordController.dispose();
    _passwordFocusNode.dispose();
    super.dispose();
  }

  Future<void> _unlock() async {
    final l10n = AppLocalizations.of(context)!;
    final password = _passwordController.text;
    if (password.isEmpty || _selectedVault == null) return;

    setState(() {
      _state = _UnlockState.unlocking;
      _errorMessage = null;
      _showOfflineWarning = false;
    });

    KeyfileRef? keyfile;
    final reference = _keyfileReference;
    try {
      if (reference != null) {
        final resolved = await ref
            .read(keyfileAccessCapabilityProvider)
            .resolve(reference);
        if (resolved case PlatformSuccess<String>(:final value)) {
          keyfile = KeyfileRef.path(value);
        } else {
          throw const _KeyfileAccessException();
        }
      }
      final repo = ref.read(sessionRepositoryProvider);
      await repo.unlock(_selectedVault!, password, keyfile: keyfile);
      _passwordController.clear();
      // Local use is available as soon as unlock returns. Discovery and the
      // once-per-process automatic attempt continue independently; offline or
      // permission failures are delivered through the sync event stream and
      // never turn a valid unlock into an error.
      unawaited(_startStartupSync());
    } on AppFailure catch (e) {
      if (!mounted) return;
      setState(() {
        _state = _UnlockState.idle;
        _errorMessage = switch (e) {
          BadCredentials() => l10n.lockScreenWrongPassword,
          VaultContended(:final holderPid) => l10n.lockScreenVaultContended(
            holderPid?.toString() ?? '?',
          ),
          KeyfileNeeded() => l10n.lockScreenKeyfileSelect,
          VaultIsBusy() => l10n.syncStatusInProgress,
          _ => l10n.errorGeneric,
        };
      });
    } on Exception {
      if (!mounted) return;
      setState(() {
        _state = _UnlockState.idle;
        _errorMessage = l10n.errorGeneric;
      });
    } finally {
      if (reference != null) {
        await ref.read(keyfileAccessCapabilityProvider).release(reference);
      }
    }
  }

  Future<void> _startStartupSync() async {
    try {
      await ref.read(syncRepositoryProvider).startStartupSync();
    } on Object {
      // Rust emits the terminal, secret-free automatic-sync failure whenever
      // the attempt can be scheduled. A concurrent lock/pause can invalidate
      // this UI-owned continuation and is intentionally nonfatal to unlock.
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final colorScheme = Theme.of(context).colorScheme;
    final vaults = ref.watch(vaultListProvider);
    final isLoading = _state != _UnlockState.idle;

    if (vaults.value?.isEmpty == true) {
      return Scaffold(body: FirstRunPage(onChoice: _handleFirstRunChoice));
    }

    return Scaffold(
      body: LayoutBuilder(
        builder: (context, constraints) => SingleChildScrollView(
          padding: const EdgeInsets.all(HidlinsSpacing.xl),
          child: ConstrainedBox(
            constraints: BoxConstraints(minHeight: constraints.maxHeight),
            child: Center(
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 400),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    const BrandMark(size: 88),
                    const SizedBox(height: HidlinsSpacing.lg),
                    Text(
                      l10n.lockScreenTitle,
                      style: Theme.of(context).textTheme.headlineMedium,
                    ),
                    const SizedBox(height: HidlinsSpacing.xl),

                    // Vault selector
                    vaults.when(
                      data: (list) {
                        if (list.isEmpty) {
                          return const SizedBox.shrink();
                        }
                        _selectedVault ??= list.first.name;
                        return Padding(
                          padding: const EdgeInsets.only(
                            bottom: HidlinsSpacing.md,
                          ),
                          child: DropdownButtonFormField<String>(
                            initialValue: _selectedVault,
                            decoration: InputDecoration(
                              labelText: l10n.lockScreenSelectVault,
                              border: const OutlineInputBorder(),
                            ),
                            items: list
                                .map(
                                  (v) => DropdownMenuItem(
                                    value: v.name,
                                    child: Text(v.name),
                                  ),
                                )
                                .toList(),
                            onChanged: isLoading
                                ? null
                                : (v) => setState(() {
                                    _selectedVault = v;
                                    _keyfileReference = null;
                                  }),
                          ),
                        );
                      },
                      loading: () => const Padding(
                        padding: EdgeInsets.only(bottom: HidlinsSpacing.md),
                        child: LinearProgressIndicator(),
                      ),
                      error: (_, _) => const SizedBox.shrink(),
                    ),

                    // Password field
                    Semantics(
                      label: l10n.lockScreenPasswordLabel,
                      child: TextField(
                        controller: _passwordController,
                        focusNode: _passwordFocusNode,
                        obscureText: _obscurePassword,
                        autocorrect: false,
                        enableSuggestions: false,
                        autofillHints: null,
                        enabled: !isLoading,
                        decoration: InputDecoration(
                          labelText: l10n.lockScreenPasswordLabel,
                          hintText: l10n.lockScreenPasswordHint,
                          border: const OutlineInputBorder(),
                          errorText: _errorMessage,
                          suffixIcon: IconButton(
                            icon: Icon(
                              _obscurePassword
                                  ? Icons.visibility
                                  : Icons.visibility_off,
                            ),
                            tooltip: _obscurePassword
                                ? l10n.actionReveal
                                : l10n.actionHide,
                            onPressed: isLoading
                                ? null
                                : () => setState(
                                    () => _obscurePassword = !_obscurePassword,
                                  ),
                          ),
                        ),
                        onChanged: (_) =>
                            ActivityCapture.reportTextInput(context),
                        onSubmitted: isLoading ? null : (_) => _unlock(),
                      ),
                    ),

                    // Keyfile row (shown when vault has keyfile)
                    if (_selectedVault != null)
                      vaults.whenData((list) {
                            final vault = list.where(
                              (v) => v.name == _selectedVault,
                            );
                            if (vault.isEmpty || !vault.first.hasKeyfile) {
                              return const SizedBox.shrink();
                            }
                            return Padding(
                              padding: const EdgeInsets.only(
                                top: HidlinsSpacing.md,
                              ),
                              child: Row(
                                children: [
                                  Icon(
                                    Icons.vpn_key,
                                    size: 20,
                                    color: colorScheme.onSurfaceVariant,
                                  ),
                                  const SizedBox(width: HidlinsSpacing.sm),
                                  Expanded(
                                    child: Text(
                                      _keyfileReference?.displayName ??
                                          l10n.lockScreenKeyfileSelect,
                                      style: Theme.of(context)
                                          .textTheme
                                          .bodyMedium
                                          ?.copyWith(
                                            color: _keyfileReference != null
                                                ? null
                                                : colorScheme.onSurfaceVariant,
                                          ),
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ),
                                  TextButton(
                                    onPressed: isLoading
                                        ? null
                                        : _selectKeyfile,
                                    child: Text(l10n.lockScreenKeyfileSelect),
                                  ),
                                ],
                              ),
                            );
                          }).value ??
                          const SizedBox.shrink(),

                    const SizedBox(height: HidlinsSpacing.lg),

                    // Unlock button / progress
                    SizedBox(
                      width: double.infinity,
                      child: FilledButton(
                        onPressed: isLoading ? null : _unlock,
                        child: isLoading
                            ? Row(
                                mainAxisSize: MainAxisSize.min,
                                children: [
                                  const SizedBox(
                                    width: 16,
                                    height: 16,
                                    child: CircularProgressIndicator(
                                      strokeWidth: 2,
                                    ),
                                  ),
                                  const SizedBox(width: HidlinsSpacing.sm),
                                  Text(
                                    _state == _UnlockState.syncing
                                        ? l10n.lockScreenSyncing
                                        : l10n.lockScreenUnlocking,
                                  ),
                                ],
                              )
                            : Text(l10n.lockScreenUnlockButton),
                      ),
                    ),

                    // Offline warning banner
                    if (_showOfflineWarning)
                      Padding(
                        padding: const EdgeInsets.only(top: HidlinsSpacing.md),
                        child: Card(
                          color: colorScheme.errorContainer,
                          child: Padding(
                            padding: const EdgeInsets.all(HidlinsSpacing.md),
                            child: Row(
                              children: [
                                Icon(
                                  Icons.cloud_off,
                                  color: colorScheme.onErrorContainer,
                                ),
                                const SizedBox(width: HidlinsSpacing.sm),
                                Expanded(
                                  child: Text(
                                    l10n.lockScreenOfflineWarning,
                                    style: TextStyle(
                                      color: colorScheme.onErrorContainer,
                                    ),
                                  ),
                                ),
                              ],
                            ),
                          ),
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  Future<void> _selectKeyfile() async {
    final result = await ref
        .read(keyfileAccessCapabilityProvider)
        .pickReference();
    if (!mounted) return;
    if (result case PlatformSuccess<KeyfileReference>(:final value)) {
      setState(() {
        _keyfileReference = value;
        _errorMessage = null;
      });
    } else if (result is PlatformFailure<KeyfileReference> ||
        result is PlatformStale<KeyfileReference>) {
      setState(
        () => _errorMessage = AppLocalizations.of(context)!.errorGeneric,
      );
    }
  }

  Future<void> _handleFirstRunChoice(FirstRunChoice choice) async {
    switch (choice) {
      case FirstRunChoice.create:
        await Navigator.of(context).push<bool>(
          MaterialPageRoute(builder: (_) => const CreateVaultDialog()),
        );
      case FirstRunChoice.connectSync:
        await Navigator.of(context).push<bool>(
          MaterialPageRoute(builder: (_) => const ConnectSyncDialog()),
        );
      case FirstRunChoice.import_:
        final picked = await ref
            .read(vaultImportCapabilityProvider)
            .pickVault();
        if (picked case PlatformSuccess<ImportedVault>(:final value)) {
          try {
            await ref
                .read(sessionRepositoryProvider)
                .registerExistingVault(
                  name: value.displayName,
                  kdbxPath: value.sourceReference,
                );
          } on Object {
            if (mounted) {
              setState(
                () =>
                    _errorMessage = AppLocalizations.of(context)!.errorGeneric,
              );
            }
          }
        }
    }
    if (mounted) ref.invalidate(vaultListProvider);
  }
}

final class _KeyfileAccessException implements Exception {
  const _KeyfileAccessException();
}
