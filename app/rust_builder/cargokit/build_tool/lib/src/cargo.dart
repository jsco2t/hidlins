/// This is copied from Cargokit (which is the official way to use it currently)
/// Details: https://fzyzcjy.github.io/flutter_rust_bridge/manual/integrate/builtin
///
/// HIDLINS PATCH (2026-07-24):
///   - CrateInfo now tracks both packageName (for `cargo -p`) and
///     libraryName (for artifact file lookup). libraryName reads [lib]
///     name when present, falling back to packageName with hyphens
///     normalized to underscores (Cargo's convention).

import 'dart:io';

import 'package:path/path.dart' as path;
import 'package:toml/toml.dart';

class ManifestException {
  ManifestException(this.message, {required this.fileName});

  final String? fileName;
  final String message;

  @override
  String toString() {
    if (fileName != null) {
      return 'Failed to parse package manifest at $fileName: $message';
    } else {
      return 'Failed to parse package manifest: $message';
    }
  }
}

class CrateInfo {
  CrateInfo({required this.packageName, required this.libraryName});

  /// The [package] name — used for `cargo build -p <name>`.
  final String packageName;

  /// The library artifact name — used for constructing file names
  /// like `lib<name>.so`. Derived from [lib] name if present,
  /// otherwise from packageName with hyphens normalized to underscores.
  final String libraryName;

  static CrateInfo parseManifest(String manifest, {final String? fileName}) {
    final toml = TomlDocument.parse(manifest);
    final package = toml.toMap()['package'];
    if (package == null) {
      throw ManifestException('Missing package section', fileName: fileName);
    }
    final pkgName = package['name'];
    if (pkgName == null) {
      throw ManifestException('Missing package name', fileName: fileName);
    }
    final lib = toml.toMap()['lib'];
    final libName = lib is Map ? lib['name'] as String? : null;
    final resolvedLibName =
        libName ?? (pkgName as String).replaceAll('-', '_');
    return CrateInfo(packageName: pkgName, libraryName: resolvedLibName);
  }

  static CrateInfo load(String manifestDir) {
    final manifestFile = File(path.join(manifestDir, 'Cargo.toml'));
    final manifest = manifestFile.readAsStringSync();
    return parseManifest(manifest, fileName: manifestFile.path);
  }
}
