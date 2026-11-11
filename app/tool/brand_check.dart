import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

Never _fail(String message) => throw StateError(message);

Future<String> _sha256(String path) async {
  for (final command in [
    ('sha256sum', <String>[path]),
    ('shasum', <String>['-a', '256', path]),
  ]) {
    try {
      final result = await Process.run(command.$1, command.$2);
      if (result.exitCode == 0) {
        return (result.stdout as String).trim().split(RegExp(r'\s+')).first;
      }
    } on ProcessException {
      // Try the portable fallback.
    }
  }
  _fail('neither sha256sum nor shasum is available');
}

int _paeth(int a, int b, int c) {
  final p = a + b - c;
  final pa = (p - a).abs();
  final pb = (p - b).abs();
  final pc = (p - c).abs();
  if (pa <= pb && pa <= pc) return a;
  if (pb <= pc) return b;
  return c;
}

({int width, int height, List<int>? alpha}) _decodePng(String path) {
  final bytes = File(path).readAsBytesSync();
  const signature = <int>[137, 80, 78, 71, 13, 10, 26, 10];
  if (bytes.length < 33 ||
      List.generate(8, (i) => i).any((i) => bytes[i] != signature[i])) {
    _fail('$path is not a PNG');
  }
  final data = ByteData.sublistView(bytes);
  var offset = 8;
  var width = 0;
  var height = 0;
  var bitDepth = 0;
  var colorType = 0;
  var interlace = 0;
  final compressed = BytesBuilder(copy: false);
  while (offset + 12 <= bytes.length) {
    final length = data.getUint32(offset);
    final type = ascii.decode(bytes.sublist(offset + 4, offset + 8));
    final start = offset + 8;
    if (type == 'IHDR') {
      width = data.getUint32(start);
      height = data.getUint32(start + 4);
      bitDepth = bytes[start + 8];
      colorType = bytes[start + 9];
      interlace = bytes[start + 12];
    } else if (type == 'IDAT') {
      compressed.add(bytes.sublist(start, start + length));
    }
    offset = start + length + 4;
    if (type == 'IEND') break;
  }
  if (width <= 0 || height <= 0) _fail('$path has no valid IHDR');
  if (bitDepth != 8 || interlace != 0) {
    _fail('$path must be non-interlaced 8-bit PNG');
  }
  final channels = switch (colorType) {
    6 => 4,
    4 => 2,
    2 => 3,
    _ => 0,
  };
  if (channels == 0) _fail('$path uses unsupported PNG color type $colorType');
  if (colorType != 6 && colorType != 4) {
    return (width: width, height: height, alpha: null);
  }
  final raw = ZLibDecoder().convert(compressed.takeBytes());
  final stride = width * channels;
  final prior = Uint8List(stride);
  final row = Uint8List(stride);
  final alpha = <int>[];
  var rawOffset = 0;
  for (var y = 0; y < height; y++) {
    final filter = raw[rawOffset++];
    for (var x = 0; x < stride; x++) {
      final encoded = raw[rawOffset++];
      final left = x >= channels ? row[x - channels] : 0;
      final up = prior[x];
      final upLeft = x >= channels ? prior[x - channels] : 0;
      row[x] = switch (filter) {
        0 => encoded,
        1 => (encoded + left) & 0xff,
        2 => (encoded + up) & 0xff,
        3 => (encoded + ((left + up) ~/ 2)) & 0xff,
        4 => (encoded + _paeth(left, up, upLeft)) & 0xff,
        _ => _fail('$path uses invalid PNG filter $filter'),
      };
    }
    for (var x = channels - 1; x < stride; x += channels) {
      alpha.add(row[x]);
    }
    prior.setAll(0, row);
  }
  return (width: width, height: height, alpha: alpha);
}

void _requireText(String path, String text) {
  if (!File(path).readAsStringSync().contains(text)) {
    _fail('$path is missing required brand reference: $text');
  }
}

Future<void> main() async {
  final manifest = jsonDecode(
    File('assets/branding/manifest.json').readAsStringSync(),
  ) as Map<String, dynamic>;
  final assets = (manifest['assets'] as List).cast<Map<String, dynamic>>();
  if (assets.isEmpty) _fail('brand manifest has no assets');

  const flutterTemplateHashes = {
    'c7c0c0189145e4e32a401c61c9bdc615754b0264e7afae24e834bb81049eaf81',
    '6a7c8f0d703e3682108f9662f813302236240d3f8f638bb391e32bfb96055fef',
    'e14aa40904929bf313fded22cf7e7ffcbf1d1aac4263b5ef1be8bfce650397aa',
    '4d470bf22d5c17d84edc5f82516d1ba8a1c09559cd761cefb792f86d9f52b540',
    '3c34e1f298d0c9ea3455d46db6b7759c8211a49e9ec6e44b635fc5c87dfb4180',
    '7770183009e914112de7d8ef1d235a6a30c5834424858e0d2f8253f6b8d31926',
    '6232e5815af17e25e0268b2fec7aea9e068cc92ec709e9605c2b31df4ff2a313',
    '93ae7d494fad0fb30cbf3ae746a39c4bc7a0f8bbf87fbb587a3f3c01f3c5ce20',
  };
  final manifestPaths = assets.map((asset) => asset['path'] as String).toSet();

  for (final asset in assets) {
    final path = asset['path'] as String;
    if (!File(path).existsSync()) _fail('manifest asset is missing: $path');
    final actualHash = await _sha256(path);
    if (actualHash != asset['sha256']) _fail('hash mismatch: $path');
    if (flutterTemplateHashes.contains(actualHash)) {
      _fail('Flutter template icon remains: $path');
    }
    if (path.endsWith('.png')) {
      final png = _decodePng(path);
      if (png.width != asset['width'] || png.height != asset['height']) {
        _fail('dimension mismatch: $path');
      }
      if (asset['alphaPolicy'] == 'opaque-required' &&
          png.alpha != null &&
          png.alpha!.any((value) => value != 255)) {
        _fail('iOS icon is not fully opaque: $path');
      }
      if (asset['alphaPolicy'] == 'transparent-safe-zone') {
        final alpha =
            png.alpha ?? _fail('adaptive foreground lacks alpha: $path');
        if (!alpha.contains(0) || !alpha.any((value) => value != 0)) {
          _fail('adaptive foreground transparency is invalid: $path');
        }
        var minX = png.width;
        var minY = png.height;
        var maxX = -1;
        var maxY = -1;
        for (var i = 0; i < alpha.length; i++) {
          if (alpha[i] == 0) continue;
          final x = i % png.width;
          final y = i ~/ png.width;
          if (x < minX) minX = x;
          if (y < minY) minY = y;
          if (x > maxX) maxX = x;
          if (y > maxY) maxY = y;
        }
        final safeInset = (png.width * 0.17).floor();
        if (minX < safeInset ||
            minY < safeInset ||
            maxX >= png.width - safeInset ||
            maxY >= png.height - safeInset) {
          _fail('adaptive foreground escapes the 66% safe zone: $path');
        }
      }
    }
  }

  for (final catalog in [
    'ios/Runner/Assets.xcassets/AppIcon.appiconset/Contents.json',
    'ios/Runner/Assets.xcassets/LaunchImage.imageset/Contents.json',
    'macos/Runner/Assets.xcassets/AppIcon.appiconset/Contents.json',
  ]) {
    final json =
        jsonDecode(File(catalog).readAsStringSync()) as Map<String, dynamic>;
    for (final image in (json['images'] as List).cast<Map<String, dynamic>>()) {
      final filename = image['filename'] as String?;
      final assetPath = filename == null
          ? null
          : '${File(catalog).parent.path}/$filename';
      if (assetPath == null || !File(assetPath).existsSync()) {
        _fail('incomplete Xcode icon catalog: $catalog');
      }
      if (!manifestPaths.contains(assetPath)) {
        _fail(
          'Xcode catalog asset is not tracked in the brand manifest: $assetPath',
        );
      }
    }
  }

  _requireText(
    'android/app/src/main/AndroidManifest.xml',
    '@mipmap/ic_launcher',
  );
  _requireText(
    'android/app/src/main/AndroidManifest.xml',
    '@mipmap/ic_launcher_round',
  );
  _requireText(
    'android/app/src/main/res/mipmap-anydpi-v26/ic_launcher.xml',
    '@drawable/ic_launcher_foreground',
  );
  _requireText(
    'android/app/src/main/res/mipmap-anydpi-v26/ic_launcher.xml',
    '@color/ic_launcher_background',
  );
  for (final path in [
    'android/app/src/main/res/mipmap-anydpi-v26/ic_launcher_round.xml',
    'android/app/src/main/res/mipmap-anydpi-v33/ic_launcher.xml',
    'android/app/src/main/res/mipmap-anydpi-v33/ic_launcher_round.xml',
  ]) {
    _requireText(path, '@drawable/ic_launcher_foreground');
    _requireText(path, '@color/ic_launcher_background');
  }
  for (final path in [
    'android/app/src/main/res/mipmap-anydpi-v33/ic_launcher.xml',
    'android/app/src/main/res/mipmap-anydpi-v33/ic_launcher_round.xml',
  ]) {
    _requireText(path, '<monochrome');
  }
  _requireText('pubspec.yaml', 'assets/branding/runtime/hidlins-navy-256.png');
  _requireText(
    'pubspec.yaml',
    'assets/branding/runtime/hidlins-vintage-dark-256.png',
  );
  _requireText('linux/CMakeLists.txt', 'share/icons/hicolor/16x16/apps');
  _requireText('linux/CMakeLists.txt', 'share/icons/hicolor/32x32/apps');
  _requireText('linux/CMakeLists.txt', 'share/icons/hicolor/256x256/apps');
  _requireText(
    'linux/runner/my_application.cc',
    'gtk_window_set_icon_from_file',
  );
  _requireText(
    'lib/src/features/lock/unlock_screen.dart',
    'BrandMark(size: 88)',
  );
  _requireText(
    'lib/src/features/vaults/first_run_page.dart',
    'BrandMark(size: 88)',
  );
  _requireText(
    'lib/src/features/settings/settings_page.dart',
    'applicationIcon: const BrandMark(size: 64)',
  );

  stdout.writeln(
    '  OK: ${assets.length} brand assets match manifest, dimensions, opacity, safe-zone, catalogs, and platform references',
  );
}
