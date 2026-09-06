import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../bridge/api/session.dart' as bridge;
import '../data/bridge_repositories.dart';
import '../data/repositories.dart';
import '../platform/secure_clipboard.dart';
import '../platform/attachment_export.dart';
import '../platform/keyfile_access.dart';
import '../platform/vault_import.dart';

final appSessionProvider = Provider<bridge.AppSession>((_) {
  throw UnimplementedError('appSessionProvider must be overridden');
});

final sessionRepositoryProvider = Provider<SessionRepository>((ref) {
  return BridgeSessionRepository(ref.watch(appSessionProvider));
});

final entryRepositoryProvider = Provider<EntryRepository>((ref) {
  return BridgeEntryRepository(ref.watch(appSessionProvider));
});

final secureClipboardCapabilityProvider = Provider<SecureClipboardCapability>((
  _,
) {
  return const MethodChannelSecureClipboard();
});

final keyfileAccessCapabilityProvider = Provider<KeyfileAccessCapability>((_) {
  return const MethodChannelKeyfileAccess();
});

final vaultImportCapabilityProvider = Provider<VaultImportCapability>((_) {
  return const MethodChannelVaultImport();
});

final attachmentExportCapabilityProvider = Provider<AttachmentExportCapability>(
  (_) {
    return const MethodChannelAttachmentExport();
  },
);

final secretsRepositoryProvider = Provider<SecretsRepository>((ref) {
  final session = ref.watch(appSessionProvider);
  return switch (defaultTargetPlatform) {
    TargetPlatform.android ||
    TargetPlatform.iOS => BridgeSecretsRepository.mobile(
      session,
      ref.watch(secureClipboardCapabilityProvider),
    ),
    _ => BridgeSecretsRepository(session),
  };
});

final searchRepositoryProvider = Provider<SearchRepository>((ref) {
  return BridgeSearchRepository(ref.watch(appSessionProvider));
});

final totpRepositoryProvider = Provider<TotpRepository>((ref) {
  return BridgeTotpRepository(ref.watch(appSessionProvider));
});

final generatorRepositoryProvider = Provider<GeneratorRepository>((ref) {
  return BridgeGeneratorRepository(ref.watch(appSessionProvider));
});

final syncRepositoryProvider = Provider<SyncRepository>((ref) {
  return BridgeSyncRepository(ref.watch(appSessionProvider));
});

final prefsRepositoryProvider = Provider<PrefsRepository>((ref) {
  return BridgePrefsRepository(ref.watch(appSessionProvider));
});
