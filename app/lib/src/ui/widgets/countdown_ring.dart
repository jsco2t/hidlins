import 'dart:math';

import 'package:flutter/material.dart';

import '../tokens.dart';

class CountdownRing extends StatelessWidget {
  const CountdownRing({
    super.key,
    required this.remainingSecs,
    required this.periodSecs,
    this.size = 40,
    this.strokeWidth = 3,
  });

  final int remainingSecs;
  final int periodSecs;
  final double size;
  final double strokeWidth;

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final fraction = periodSecs > 0
        ? remainingSecs.clamp(0, periodSecs) / periodSecs
        : 0.0;
    final isUrgent = remainingSecs < HidlinsCountdown.amberThresholdSecs;
    final color = isUrgent ? Colors.amber : colorScheme.primary;

    return SizedBox(
      width: size,
      height: size,
      child: CustomPaint(
        painter: _RingPainter(
          fraction: fraction,
          color: color,
          backgroundColor: colorScheme.surfaceContainerHighest,
          strokeWidth: strokeWidth,
        ),
        child: Center(
          child: Text(
            '$remainingSecs',
            style: Theme.of(context).textTheme.labelSmall?.copyWith(
              color: color,
              fontWeight: FontWeight.bold,
            ),
          ),
        ),
      ),
    );
  }
}

class _RingPainter extends CustomPainter {
  _RingPainter({
    required this.fraction,
    required this.color,
    required this.backgroundColor,
    required this.strokeWidth,
  });

  final double fraction;
  final Color color;
  final Color backgroundColor;
  final double strokeWidth;

  @override
  void paint(Canvas canvas, Size size) {
    final center = Offset(size.width / 2, size.height / 2);
    final radius = (size.shortestSide - strokeWidth) / 2;

    final bgPaint = Paint()
      ..color = backgroundColor
      ..strokeWidth = strokeWidth
      ..style = PaintingStyle.stroke;
    canvas.drawCircle(center, radius, bgPaint);

    final fgPaint = Paint()
      ..color = color
      ..strokeWidth = strokeWidth
      ..style = PaintingStyle.stroke
      ..strokeCap = StrokeCap.round;
    canvas.drawArc(
      Rect.fromCircle(center: center, radius: radius),
      -pi / 2,
      2 * pi * fraction,
      false,
      fgPaint,
    );
  }

  @override
  bool shouldRepaint(_RingPainter oldDelegate) =>
      fraction != oldDelegate.fraction ||
      color != oldDelegate.color ||
      backgroundColor != oldDelegate.backgroundColor;
}
