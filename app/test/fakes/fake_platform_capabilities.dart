import 'dart:collection';

import 'package:app/src/platform/app_paths.dart';
import 'package:app/src/platform/attachment_export.dart';
import 'package:app/src/platform/keyfile_access.dart';
import 'package:app/src/platform/lifecycle.dart';
import 'package:app/src/platform/platform_result.dart';
import 'package:app/src/platform/secure_clipboard.dart';
import 'package:app/src/platform/vault_import.dart';

PlatformResult<T> _take<T>(Queue<PlatformResult<T>> results) {
  if (results.isEmpty) return const PlatformUnsupported();
  return results.removeFirst();
}

final class FakeLifecycleCapability implements LifecycleCapability {
  FakeLifecycleCapability(Iterable<PlatformResult<PlatformUnit>> results)
    : results = Queue.of(results);

  final Queue<PlatformResult<PlatformUnit>> results;
  final List<AppLifecycleSignal> reports = [];

  @override
  Future<PlatformResult<PlatformUnit>> report(AppLifecycleSignal state) async {
    reports.add(state);
    return _take(results);
  }
}

final class FakeSecureClipboardCapability implements SecureClipboardCapability {
  FakeSecureClipboardCapability(Iterable<PlatformResult<PlatformUnit>> results)
    : results = Queue.of(results);

  final Queue<PlatformResult<PlatformUnit>> results;
  final List<PreparedClipboardTransfer> copies = [];

  @override
  Future<PlatformResult<PlatformUnit>> copyPrepared(
    PreparedClipboardTransfer transfer,
  ) async {
    copies.add(transfer);
    return _take(results);
  }
}

final class FakeAppPathsCapability implements AppPathsCapability {
  FakeAppPathsCapability(Iterable<PlatformResult<String>> results)
    : results = Queue.of(results);

  final Queue<PlatformResult<String>> results;
  var calls = 0;

  @override
  Future<PlatformResult<String>> applicationSupportPath() async {
    calls += 1;
    return _take(results);
  }
}

final class FakeVaultImportCapability implements VaultImportCapability {
  FakeVaultImportCapability(Iterable<PlatformResult<ImportedVault>> results)
    : results = Queue.of(results);

  final Queue<PlatformResult<ImportedVault>> results;
  var calls = 0;

  @override
  Future<PlatformResult<ImportedVault>> pickVault() async {
    calls += 1;
    return _take(results);
  }
}

final class FakeAttachmentExportCapability
    implements AttachmentExportCapability {
  FakeAttachmentExportCapability(Iterable<PlatformResult<String>> results)
    : results = Queue.of(results);

  final Queue<PlatformResult<String>> results;
  final List<String> suggestedNames = [];

  @override
  Future<PlatformResult<String>> chooseDestination(String suggestedName) async {
    suggestedNames.add(suggestedName);
    return _take(results);
  }
}

final class FakeKeyfileAccessCapability implements KeyfileAccessCapability {
  FakeKeyfileAccessCapability({
    Iterable<PlatformResult<KeyfileReference>> pickResults = const [],
    Iterable<PlatformResult<String>> resolveResults = const [],
    Iterable<PlatformResult<PlatformUnit>> releaseResults = const [],
  }) : pickResults = Queue.of(pickResults),
       resolveResults = Queue.of(resolveResults),
       releaseResults = Queue.of(releaseResults);

  final Queue<PlatformResult<KeyfileReference>> pickResults;
  final Queue<PlatformResult<String>> resolveResults;
  final Queue<PlatformResult<PlatformUnit>> releaseResults;
  final List<KeyfileReference> resolved = [];
  final List<KeyfileReference> released = [];

  @override
  Future<PlatformResult<KeyfileReference>> pickReference() async {
    return _take(pickResults);
  }

  @override
  Future<PlatformResult<String>> resolve(KeyfileReference reference) async {
    resolved.add(reference);
    return _take(resolveResults);
  }

  @override
  Future<PlatformResult<PlatformUnit>> release(
    KeyfileReference reference,
  ) async {
    released.add(reference);
    return _take(releaseResults);
  }
}
