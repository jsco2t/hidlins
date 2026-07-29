import 'package:flutter/material.dart';

class EntryAvatar extends StatelessWidget {
  const EntryAvatar({super.key, required this.title, this.size = 40});

  final String title;
  final double size;

  @override
  Widget build(BuildContext context) {
    final initial = title.isNotEmpty ? title[0].toUpperCase() : '?';
    final colorScheme = Theme.of(context).colorScheme;
    final tint = _tintForTitle(title, colorScheme);
    return SizedBox(
      width: size,
      height: size,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: tint.withValues(alpha: 0.15),
          shape: BoxShape.circle,
        ),
        child: Center(
          child: Text(
            initial,
            style: Theme.of(context).textTheme.titleMedium?.copyWith(
              color: tint,
              fontWeight: FontWeight.w600,
            ),
          ),
        ),
      ),
    );
  }
}

Color _tintForTitle(String title, ColorScheme colorScheme) {
  if (title.isEmpty) return colorScheme.primary;
  final hash = title.hashCode.abs();
  const tints = [
    Color(0xFF3E6B5C),
    Color(0xFF5C6B3E),
    Color(0xFF3E5C6B),
    Color(0xFF6B3E5C),
    Color(0xFF6B5C3E),
    Color(0xFF3E6B6B),
  ];
  return tints[hash % tints.length];
}
