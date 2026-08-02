import 'package:flutter/widgets.dart';

abstract final class HidlinsSpacing {
  static const double xs = 4;
  static const double sm = 8;
  static const double md = 16;
  static const double lg = 24;
  static const double xl = 32;
}

abstract final class HidlinsMotion {
  static const Duration fast = Duration(milliseconds: 150);
  static const Duration standard = Duration(milliseconds: 250);
  static const Duration slow = Duration(milliseconds: 300);
  static const Curve curve = Curves.easeOutCubic;
}

abstract final class HidlinsBreakpoints {
  static const double compact = 600;
  static const double medium = 840;
}

abstract final class HidlinsCountdown {
  static const int amberThresholdSecs = 5;
}

Duration hidlinsMotionDuration(BuildContext context, Duration duration) {
  return MediaQuery.disableAnimationsOf(context) ? Duration.zero : duration;
}
