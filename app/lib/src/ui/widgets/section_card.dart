import 'package:flutter/material.dart';

import '../tokens.dart';

class SectionCard extends StatelessWidget {
  const SectionCard({super.key, this.title, required this.child});

  final String? title;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Card(
      color: Theme.of(context).colorScheme.surfaceContainerLow,
      margin: const EdgeInsets.symmetric(
        horizontal: HidlinsSpacing.md,
        vertical: HidlinsSpacing.sm,
      ),
      child: Padding(
        padding: const EdgeInsets.all(HidlinsSpacing.md),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            if (title != null) ...[
              Text(title!, style: Theme.of(context).textTheme.titleSmall),
              const SizedBox(height: HidlinsSpacing.sm),
            ],
            child,
          ],
        ),
      ),
    );
  }
}
