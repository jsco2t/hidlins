import 'package:material_ui/material_ui.dart';

import '../tokens.dart';

class CopyRow extends StatefulWidget {
  const CopyRow({
    super.key,
    required this.label,
    required this.value,
    required this.onCopy,
    this.copyTooltip,
  });

  final String label;
  final String value;
  final VoidCallback onCopy;
  final String? copyTooltip;

  @override
  State<CopyRow> createState() => _CopyRowState();
}

class _CopyRowState extends State<CopyRow> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final usesHover = switch (Theme.of(context).platform) {
      TargetPlatform.linux ||
      TargetPlatform.macOS ||
      TargetPlatform.windows => true,
      _ => false,
    };
    final showCopyAction = !usesHover || _hovered;
    return MouseRegion(
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: Padding(
        padding: const EdgeInsets.symmetric(
          horizontal: HidlinsSpacing.md,
          vertical: HidlinsSpacing.sm,
        ),
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    widget.label,
                    style: Theme.of(context).textTheme.labelSmall
                        ?.copyWith(color: colorScheme.onSurfaceVariant),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    widget.value,
                    style: Theme.of(context).textTheme.bodyMedium,
                  ),
                ],
              ),
            ),
            IgnorePointer(
              ignoring: !showCopyAction,
              child: AnimatedOpacity(
                opacity: showCopyAction ? 1.0 : 0.0,
                duration: const Duration(milliseconds: 150),
                child: Semantics(
                  label: widget.copyTooltip,
                  button: true,
                  onTap: widget.onCopy,
                  excludeSemantics: true,
                  child: IconButton(
                    icon: const Icon(Icons.copy, size: 18),
                    tooltip: widget.copyTooltip,
                    onPressed: widget.onCopy,
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
