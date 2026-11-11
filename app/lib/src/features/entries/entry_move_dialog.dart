import 'package:material_ui/material_ui.dart';

import '../../data/models.dart';
import '../../l10n/app_localizations.dart';

class EntryMoveDialog extends StatefulWidget {
  const EntryMoveDialog({super.key, required this.root, this.currentGroupUuid});

  final GroupNode root;
  final String? currentGroupUuid;

  @override
  State<EntryMoveDialog> createState() => _EntryMoveDialogState();
}

class _EntryMoveDialogState extends State<EntryMoveDialog> {
  String? _destination;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final destinations = _items(widget.root, widget.root.name).toList();
    return AlertDialog(
      title: Text(l10n.entryMoveTitle),
      content: SizedBox(
        width: 400,
        child: destinations.isEmpty
            ? Text(l10n.entryMoveNoDestinations)
            : DropdownButtonFormField<String>(
                isExpanded: true,
                initialValue: _destination,
                decoration: InputDecoration(
                  labelText: l10n.entryMoveDestinationLabel,
                ),
                items: destinations,
                onChanged: (value) => setState(() => _destination = value),
              ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.actionCancel),
        ),
        FilledButton(
          onPressed: _destination == null
              ? null
              : () => Navigator.of(context).pop(_destination),
          child: Text(l10n.actionMove),
        ),
      ],
    );
  }

  Iterable<DropdownMenuItem<String>> _items(GroupNode node, String path) sync* {
    if (node.uuid != widget.currentGroupUuid) {
      yield DropdownMenuItem(value: node.uuid, child: Text(path));
    }
    for (final child in node.children) {
      yield* _items(child, '$path / ${child.name}');
    }
  }
}
