import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../providers/providers.dart';
import '../../ui/activity_capture.dart';
import '../../ui/tokens.dart';

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
    if (d != null) _kind = d.kind;
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
            FilledButton(
              onPressed: _saving ? null : _save,
              child: Text(l10n.actionSave),
            ),
            const SizedBox(width: HidlinsSpacing.md),
          ],
        ),
        body: _saving
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
    );
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
    // T4.6 will provide the generator dialog; for now this is a placeholder
  }

  Future<void> _save() async {
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
        await repo.createEntry(
          widget.groupUuid ?? 'root-uuid',
          EntryDraftDto(
            kind: _kind,
            title: _titleCtrl.text,
            username: _usernameCtrl.text.isEmpty ? null : _usernameCtrl.text,
            password: _passwordCtrl.text.isEmpty ? null : _passwordCtrl.text,
            url: _urlCtrl.text.isEmpty ? null : _urlCtrl.text,
            notes: _notesCtrl.text.isEmpty ? null : _notesCtrl.text,
            tags: tags,
            customFields: customFields,
            totpUri: _totpUriCtrl.text.isEmpty ? null : _totpUriCtrl.text,
          ),
        );
      } else {
        await repo.updateEntry(
          widget.detail!.uuid,
          EntryEditDto(
            title: _titleCtrl.text,
            username: _usernameCtrl.text,
            password: _passwordCtrl.text.isEmpty ? null : _passwordCtrl.text,
            url: _urlCtrl.text,
            notes: _notesCtrl.text,
            tags: tags,
            customFields: customFields,
            totpUri: _totpUriCtrl.text.isEmpty ? null : _totpUriCtrl.text,
          ),
        );
      }
      if (mounted) Navigator.of(context).pop(true);
    } on Exception {
      setState(() => _saving = false);
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
  final TextEditingController nameCtrl = TextEditingController();
  final TextEditingController valueCtrl = TextEditingController();
  bool protected = false;
}
