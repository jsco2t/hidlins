import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../providers/providers.dart';
import '../../ui/activity_capture.dart';
import '../../ui/tokens.dart';
import '../generator/generator_page.dart';

class EntryEditDialog extends ConsumerStatefulWidget {
  const EntryEditDialog({super.key, this.detail, this.groupUuid});

  final EntryDetail? detail;
  final String? groupUuid;

  bool get isCreate => detail == null;

  @override
  ConsumerState<EntryEditDialog> createState() => _EntryEditDialogState();
}

class _EntryEditDialogState extends ConsumerState<EntryEditDialog> {
  final _formKey = GlobalKey<FormState>();
  late final TextEditingController _titleCtrl;
  late final TextEditingController _usernameCtrl;
  late final TextEditingController _passwordCtrl;
  late final TextEditingController _urlCtrl;
  late final TextEditingController _notesCtrl;
  late final TextEditingController _totpUriCtrl;
  late final TextEditingController _tagsCtrl;

  EntryKindDto _kind = EntryKindDto.credential;
  final List<_CustomFieldEntry> _customFields = [];
  bool _loadingCustomFields = false;
  bool _customFieldLoadFailed = false;
  bool _staleAfterSync = false;
  bool _dirty = false;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    final d = widget.detail;
    _titleCtrl = TextEditingController(text: d?.title ?? '');
    _usernameCtrl = TextEditingController(text: d?.username ?? '');
    _passwordCtrl = TextEditingController();
    _urlCtrl = TextEditingController(text: d?.url ?? '');
    _notesCtrl = TextEditingController(text: d?.notes ?? '');
    _totpUriCtrl = TextEditingController();
    _tagsCtrl = TextEditingController(text: d?.tags.join(', ') ?? '');
    if (d != null) {
      _kind = d.kind;
      if (d.customFields.isNotEmpty) {
        _loadingCustomFields = true;
        unawaited(_loadExistingCustomFields(d));
      }
    }
  }

  @override
  void dispose() {
    _titleCtrl.dispose();
    _usernameCtrl.dispose();
    _passwordCtrl.dispose();
    _urlCtrl.dispose();
    _notesCtrl.dispose();
    _totpUriCtrl.dispose();
    _tagsCtrl.dispose();
    for (final cf in _customFields) {
      cf.nameCtrl.dispose();
      cf.valueCtrl.dispose();
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final isCreate = widget.isCreate;
    final syncState = ref.watch(syncUiStateProvider);
    final syncUnavailable = !syncState.hasValue;
    final syncInFlight = syncState.valueOrNull?.inFlight ?? false;
    ref.listen(syncEventsProvider, (_, next) {
      if (next.valueOrNull is! SyncEvent_Done || !mounted || _saving) return;
      setState(() => _staleAfterSync = true);
    });
    final saveBlocked =
        _saving ||
        _loadingCustomFields ||
        _customFieldLoadFailed ||
        syncUnavailable ||
        syncInFlight ||
        _staleAfterSync;
    final blockedMessage = _staleAfterSync
        ? l10n.entryEditReloadAfterSync
        : syncUnavailable || syncInFlight
        ? l10n.syncActionsDisabled
        : _customFieldLoadFailed
        ? l10n.errorRevealFailed
        : null;

    return PopScope(
      canPop: !_dirty,
      onPopInvokedWithResult: (didPop, _) async {
        if (didPop) return;
        await _showDiscardDialog();
      },
      child: Scaffold(
        appBar: AppBar(
          title: Text(isCreate ? l10n.entryCreateTitle : l10n.entryEditTitle),
          actions: [
            Tooltip(
              message: blockedMessage ?? l10n.actionSave,
              child: FilledButton(
                onPressed: saveBlocked ? null : _save,
                child: Text(l10n.actionSave),
              ),
            ),
            const SizedBox(width: HidlinsSpacing.md),
          ],
        ),
        body: Column(
          children: [
            if (blockedMessage != null)
              Container(
                width: double.infinity,
                color: Theme.of(context).colorScheme.tertiaryContainer,
                padding: const EdgeInsets.symmetric(
                  horizontal: HidlinsSpacing.md,
                  vertical: HidlinsSpacing.sm,
                ),
                child: Row(
                  children: [
                    Expanded(child: Text(blockedMessage)),
                    if (_customFieldLoadFailed && !_staleAfterSync)
                      TextButton(
                        onPressed: () => unawaited(
                          _loadExistingCustomFields(widget.detail!),
                        ),
                        child: Text(l10n.actionRetry),
                      ),
                  ],
                ),
              ),
            Expanded(
              child: _saving || _loadingCustomFields
                  ? const Center(child: CircularProgressIndicator())
                  : SingleChildScrollView(
                      padding: const EdgeInsets.all(HidlinsSpacing.md),
                      child: Form(
                        key: _formKey,
                        onChanged: () {
                          ActivityCapture.reportTextInput(context);
                          setState(() => _dirty = true);
                        },
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: [
                            // Entry kind selector (create only)
                            if (isCreate)
                              Padding(
                                padding: const EdgeInsets.only(
                                  bottom: HidlinsSpacing.md,
                                ),
                                child: SegmentedButton<EntryKindDto>(
                                  segments: [
                                    ButtonSegment(
                                      value: EntryKindDto.credential,
                                      label: Text(l10n.entryEditKindCredential),
                                    ),
                                    ButtonSegment(
                                      value: EntryKindDto.secureNote,
                                      label: Text(l10n.entryEditKindSecureNote),
                                    ),
                                    ButtonSegment(
                                      value: EntryKindDto.totp,
                                      label: Text(l10n.entryEditKindTotp),
                                    ),
                                  ],
                                  selected: {_kind},
                                  onSelectionChanged: (s) =>
                                      setState(() => _kind = s.first),
                                ),
                              ),

                            // Title
                            TextFormField(
                              controller: _titleCtrl,
                              decoration: InputDecoration(
                                labelText: l10n.entryEditTitleLabel,
                                border: const OutlineInputBorder(),
                              ),
                              validator: (v) =>
                                  (v == null || v.isEmpty) ? 'Required' : null,
                            ),
                            const SizedBox(height: HidlinsSpacing.md),

                            // Username
                            if (_kind == EntryKindDto.credential) ...[
                              TextFormField(
                                controller: _usernameCtrl,
                                decoration: InputDecoration(
                                  labelText: l10n.entryEditUsernameLabel,
                                  border: const OutlineInputBorder(),
                                ),
                              ),
                              const SizedBox(height: HidlinsSpacing.md),
                            ],

                            // Password
                            if (_kind == EntryKindDto.credential) ...[
                              TextFormField(
                                controller: _passwordCtrl,
                                obscureText: true,
                                decoration: InputDecoration(
                                  labelText: l10n.entryEditPasswordLabel,
                                  border: const OutlineInputBorder(),
                                  suffixIcon: IconButton(
                                    icon: const Icon(Icons.password),
                                    tooltip: l10n.actionGenerate,
                                    onPressed: _openGenerator,
                                  ),
                                ),
                              ),
                              const SizedBox(height: HidlinsSpacing.md),
                            ],

                            // URL
                            TextFormField(
                              controller: _urlCtrl,
                              decoration: InputDecoration(
                                labelText: l10n.entryEditUrlLabel,
                                border: const OutlineInputBorder(),
                              ),
                              keyboardType: TextInputType.url,
                            ),
                            const SizedBox(height: HidlinsSpacing.md),

                            // Notes
                            TextFormField(
                              controller: _notesCtrl,
                              decoration: InputDecoration(
                                labelText: l10n.entryEditNotesLabel,
                                border: const OutlineInputBorder(),
                              ),
                              maxLines: 4,
                            ),
                            const SizedBox(height: HidlinsSpacing.md),

                            // TOTP URI
                            if (_kind == EntryKindDto.totp) ...[
                              TextFormField(
                                controller: _totpUriCtrl,
                                decoration: InputDecoration(
                                  labelText: l10n.entryEditTotpUriLabel,
                                  border: const OutlineInputBorder(),
                                ),
                              ),
                              const SizedBox(height: HidlinsSpacing.md),
                            ],

                            // Tags
                            TextFormField(
                              controller: _tagsCtrl,
                              decoration: InputDecoration(
                                labelText: l10n.entryEditTagsLabel,
                                border: const OutlineInputBorder(),
                              ),
                            ),
                            const SizedBox(height: HidlinsSpacing.md),

                            // Custom fields
                            ..._buildCustomFields(l10n),

                            TextButton.icon(
                              onPressed: _addCustomField,
                              icon: const Icon(Icons.add),
                              label: Text(l10n.entryEditAddCustomField),
                            ),
                          ],
                        ),
                      ),
                    ),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _loadExistingCustomFields(EntryDetail detail) async {
    if (!_loadingCustomFields && mounted) {
      setState(() {
        _loadingCustomFields = true;
        _customFieldLoadFailed = false;
      });
    } else {
      _loadingCustomFields = true;
      _customFieldLoadFailed = false;
    }

    final loaded = <_CustomFieldEntry>[];
    try {
      final repo = ref.read(secretsRepositoryProvider);
      for (final field in detail.customFields) {
        final value = await repo.revealField(
          detail.uuid,
          RevealFieldCustom(field.name),
        );
        if (!mounted) {
          for (final field in loaded) {
            field.dispose();
          }
          return;
        }
        loaded.add(
          _CustomFieldEntry(
            name: field.name,
            value: value,
            protected: field.isProtected,
          ),
        );
      }
    } on Object {
      for (final field in loaded) {
        field.dispose();
      }
      if (!mounted) return;
      setState(() {
        _loadingCustomFields = false;
        _customFieldLoadFailed = true;
      });
      return;
    }

    if (!mounted) {
      for (final field in loaded) {
        field.dispose();
      }
      return;
    }
    setState(() {
      for (final field in _customFields) {
        field.dispose();
      }
      _customFields
        ..clear()
        ..addAll(loaded);
      _loadingCustomFields = false;
      _customFieldLoadFailed = false;
    });
  }

  List<Widget> _buildCustomFields(AppLocalizations l10n) {
    return _customFields.asMap().entries.map((entry) {
      final i = entry.key;
      final cf = entry.value;
      return Padding(
        padding: const EdgeInsets.only(bottom: HidlinsSpacing.md),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              child: TextFormField(
                controller: cf.nameCtrl,
                decoration: InputDecoration(
                  labelText: l10n.entryEditCustomFieldName,
                  border: const OutlineInputBorder(),
                  isDense: true,
                ),
              ),
            ),
            const SizedBox(width: HidlinsSpacing.sm),
            Expanded(
              child: TextFormField(
                controller: cf.valueCtrl,
                obscureText: cf.protected,
                autocorrect: !cf.protected,
                enableSuggestions: !cf.protected,
                autofillHints: cf.protected ? null : const <String>[],
                decoration: InputDecoration(
                  labelText: l10n.entryEditCustomFieldValue,
                  border: const OutlineInputBorder(),
                  isDense: true,
                ),
              ),
            ),
            const SizedBox(width: HidlinsSpacing.xs),
            Tooltip(
              message: l10n.entryEditCustomFieldProtected,
              child: IconButton(
                icon: Icon(
                  cf.protected ? Icons.lock : Icons.lock_open,
                  size: 18,
                ),
                onPressed: () => setState(() => cf.protected = !cf.protected),
                visualDensity: VisualDensity.compact,
              ),
            ),
            IconButton(
              icon: const Icon(Icons.close, size: 18),
              tooltip: l10n.actionRemove,
              onPressed: () => setState(() {
                _customFields.removeAt(i);
                _dirty = true;
              }),
              visualDensity: VisualDensity.compact,
            ),
          ],
        ),
      );
    }).toList();
  }

  void _addCustomField() {
    setState(() {
      _customFields.add(_CustomFieldEntry());
      _dirty = true;
    });
  }

  Future<void> _openGenerator() async {
    await Navigator.of(context).push<void>(
      MaterialPageRoute(
        builder: (generatorContext) => GeneratorPage(
          onUse: (value) {
            if (!mounted) return;
            setState(() {
              _passwordCtrl.text = value;
              _dirty = true;
            });
            Navigator.of(generatorContext).pop();
          },
        ),
      ),
    );
  }

  Future<void> _save() async {
    final syncState = ref.read(syncUiStateProvider);
    if (_loadingCustomFields ||
        _customFieldLoadFailed ||
        _staleAfterSync ||
        !syncState.hasValue ||
        (syncState.valueOrNull?.inFlight ?? false)) {
      return;
    }
    if (!_formKey.currentState!.validate()) return;

    setState(() => _saving = true);

    final tags = _tagsCtrl.text
        .split(',')
        .map((t) => t.trim())
        .where((t) => t.isNotEmpty)
        .toList();

    final customFields = _customFields
        .where((cf) => cf.nameCtrl.text.isNotEmpty)
        .map(
          (cf) => CustomFieldInputDto(
            name: cf.nameCtrl.text,
            value: cf.valueCtrl.text,
            protected: cf.protected,
          ),
        )
        .toList();

    try {
      final repo = ref.read(entryRepositoryProvider);
      if (widget.isCreate) {
        final draft = EntryDraftDto(
          kind: _kind,
          title: _titleCtrl.text,
          username: _usernameCtrl.text.isEmpty ? null : _usernameCtrl.text,
          password: _passwordCtrl.text.isEmpty ? null : _passwordCtrl.text,
          url: _urlCtrl.text.isEmpty ? null : _urlCtrl.text,
          notes: _notesCtrl.text.isEmpty ? null : _notesCtrl.text,
          tags: tags,
          customFields: customFields,
          totpUri: _totpUriCtrl.text.isEmpty ? null : _totpUriCtrl.text,
        );
        _clearSecretControllers();
        await repo.createEntry(widget.groupUuid ?? 'root-uuid', draft);
      } else {
        final edit = EntryEditDto(
          title: _titleCtrl.text,
          username: _usernameCtrl.text,
          password: _passwordCtrl.text.isEmpty ? null : _passwordCtrl.text,
          url: _urlCtrl.text,
          notes: _notesCtrl.text,
          tags: tags,
          customFields: customFields,
          totpUri: _totpUriCtrl.text.isEmpty ? null : _totpUriCtrl.text,
        );
        _clearSecretControllers();
        await repo.updateEntry(widget.detail!.uuid, edit);
      }
      if (mounted) Navigator.of(context).pop(true);
    } on Exception {
      if (mounted) setState(() => _saving = false);
    }
  }

  void _clearSecretControllers() {
    _passwordCtrl.clear();
    _totpUriCtrl.clear();
    for (final field in _customFields.where((field) => field.protected)) {
      field.valueCtrl.clear();
    }
  }

  Future<void> _showDiscardDialog() async {
    final l10n = AppLocalizations.of(context)!;
    final result = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(l10n.entryEditUnsavedTitle),
        content: Text(l10n.entryEditUnsavedMessage),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(l10n.actionCancel),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(l10n.entryEditDiscard),
          ),
        ],
      ),
    );
    if (result == true && mounted) {
      Navigator.of(context).pop();
    }
  }
}

class _CustomFieldEntry {
  _CustomFieldEntry({
    String name = '',
    String value = '',
    this.protected = false,
  }) : nameCtrl = TextEditingController(text: name),
       valueCtrl = TextEditingController(text: value);

  final TextEditingController nameCtrl;
  final TextEditingController valueCtrl;
  bool protected;

  void dispose() {
    nameCtrl.dispose();
    valueCtrl.dispose();
  }
}
