import 'package:material_ui/material_ui.dart';

/// The decorative Hidlins mark, selected for the current surface brightness.
///
/// The visible product name remains adjacent text, so announcing the image
/// would duplicate information for assistive technology.
class BrandMark extends StatelessWidget {
  const BrandMark({super.key, this.size = 64});

  static const navyAsset = 'assets/branding/runtime/hidlins-navy-256.png';
  static const vintageDarkAsset =
      'assets/branding/runtime/hidlins-vintage-dark-256.png';

  final double size;

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    return ExcludeSemantics(
      child: Image.asset(
        isDark ? vintageDarkAsset : navyAsset,
        key: ValueKey(isDark ? vintageDarkAsset : navyAsset),
        width: size,
        height: size,
        filterQuality: FilterQuality.high,
      ),
    );
  }
}
