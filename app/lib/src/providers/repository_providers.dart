import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../bridge/api/session.dart' as bridge;
import '../data/bridge_repositories.dart';
import '../data/repositories.dart';

final appSessionProvider = Provider<bridge.AppSession>((_) {
  throw UnimplementedError('appSessionProvider must be overridden');
});

final sessionRepositoryProvider = Provider<SessionRepository>((ref) {
  return BridgeSessionRepository(ref.watch(appSessionProvider));
});

final entryRepositoryProvider = Provider<EntryRepository>((ref) {
  return BridgeEntryRepository(ref.watch(appSessionProvider));
});

final secretsRepositoryProvider = Provider<SecretsRepository>((ref) {
  return BridgeSecretsRepository(ref.watch(appSessionProvider));
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
