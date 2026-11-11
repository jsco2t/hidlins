// Hidlins patch: coordinate byte-derived vendor invalidation across build frontends.
import 'dart:convert';
import 'dart:io';

import 'package:path/path.dart' as path;

import 'util.dart';

class VendorCache {
  static String build({
    required String sourceRoot,
    required String manifestDirectory,
    required String targetDirectory,
    required List<String> cargo,
    required String Function() compile,
  }) {
    final helper = path.join(sourceRoot, 'tools', 'dev', 'vendor_cache.py');
    final description = runCommand('python3', [
      helper,
      'describe',
      '--workspace',
      manifestDirectory,
      '--target-dir',
      targetDirectory,
      '--',
      ...cargo,
    ]);
    final data = jsonDecode(description.stdout.toString());
    if (data is! Map ||
        data['lock_paths'] is! List ||
        data['target_directory'] is! String ||
        data['build_directory'] is! String) {
      throw StateError('Invalid Cargo cache descriptor');
    }
    final locks = (data['lock_paths'] as List).cast<String>().toSet().toList()
      ..sort();
    if (locks.isEmpty ||
        locks.any((value) => !path.isAbsolute(value)) ||
        !path.isAbsolute(data['target_directory'] as String) ||
        !path.isAbsolute(data['build_directory'] as String)) {
      throw StateError('Cargo cache descriptor must contain absolute paths');
    }
    final held = <RandomAccessFile>[];
    try {
      // Keep acquisition, invalidation and compilation synchronous in this
      // build-tool isolate: POSIX record locks are owned by the process.
      for (final name in locks) {
        final file = File(name);
        file.parent.createSync(recursive: true);
        if (FileSystemEntity.typeSync(name, followLinks: false) ==
            FileSystemEntityType.link) {
          throw StateError('Cargo cache lock must not be a symbolic link');
        }
        final handle = file.openSync(mode: FileMode.append);
        try {
          handle.lockSync(FileLock.blockingExclusive);
        } catch (_) {
          handle.closeSync();
          rethrow;
        }
        held.add(handle);
      }
      runCommand('python3', [
        helper,
        'invalidate-locked',
        '--source-root',
        sourceRoot,
        '--target-dir',
        data['target_directory'] as String,
        '--build-dir',
        data['build_directory'] as String,
      ]);
      return compile();
    } finally {
      for (final handle in held.reversed) {
        try {
          handle.unlockSync();
        } finally {
          handle.closeSync();
        }
      }
    }
  }
}
