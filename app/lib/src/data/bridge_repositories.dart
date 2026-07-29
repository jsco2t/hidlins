import 'dart:async';

import '../bridge/api/session.dart' as bridge_api;
import '../bridge/dto.dart' as bridge;
import '../bridge/error.dart';
import 'failures.dart';
import 'models.dart';
import 'repositories.dart';

Future<T> _bridgeFuture<T>(Future<T> Function() call) async {
  try {
    return await call();
  } on HidlinsApiError catch (error) {
    throw mapApiError(error);
  }
}

T _bridgeSync<T>(T Function() call) {
  try {
    return call();
  } on HidlinsApiError catch (error) {
    throw mapApiError(error);
  }
}

Stream<T> _bridgeStream<T>(Stream<T> Function() call) async* {
  try {
    yield* call();
  } on HidlinsApiError catch (error) {
    throw mapApiError(error);
  }
}

VaultSummary _vaultSummary(bridge.VaultSummary value) {
  return VaultSummary(
    name: value.name,
    path: value.path,
    hasKeyfile: value.hasKeyfile,
    hasSync: value.hasSync,
  );
}

AttachmentMeta _attachmentMeta(bridge.AttachmentMeta value) {
  return AttachmentMeta(name: value.name, sizeBytes: value.sizeBytes.toInt());
}

EntryDetail _entryDetail(bridge.EntryDetail value) {
  return EntryDetail(
    uuid: value.uuid,
    title: value.title,
    username: value.username,
    hasPassword: value.hasPassword,
    url: value.url,
    notes: value.notes,
    kind: value.kind,
    tags: value.tags,
    customFields: [
      for (final field in value.customFields)
        CustomFieldDto(name: field.name, isProtected: field.isProtected),
    ],
    attachments: [
      for (final attachment in value.attachments) _attachmentMeta(attachment),
    ],
    creationTime: value.creationTime,
    lastModificationTime: value.lastModificationTime,
    expiryTime: value.expiryTime,
  );
}

bridge.CustomFieldInputDto _customField(CustomFieldInputDto value) {
  return bridge.CustomFieldInputDto(
    name: value.name,
    value: value.value,
    protected: value.protected,
  );
}

bridge.EntryDraftDto _entryDraft(EntryDraftDto value) {
  return bridge.EntryDraftDto(
    kind: value.kind,
    title: value.title,
    username: value.username,
    password: value.password,
    url: value.url,
    notes: value.notes,
    tags: value.tags,
    customFields: value.customFields.map(_customField).toList(),
    totpUri: value.totpUri,
  );
}

bridge.EntryEditDto _entryEdit(EntryEditDto value) {
  return bridge.EntryEditDto(
    title: value.title,
    username: value.username,
    password: value.password,
    url: value.url,
    notes: value.notes,
    tags: value.tags,
    customFields: value.customFields?.map(_customField).toList(),
    totpUri: value.totpUri,
  );
}

bridge.RevealField _revealField(RevealField value) {
  return switch (value) {
    RevealFieldPassword() => const bridge.RevealField.password(),
    RevealFieldCustom(:final name) => bridge.RevealField.customField(name),
    RevealFieldTotpUri() => const bridge.RevealField.totpUri(),
  };
}

bridge.CopyField _copyField(CopyField value) {
  return switch (value) {
    CopyFieldUsername() => const bridge.CopyField.username(),
    CopyFieldPassword() => const bridge.CopyField.password(),
    CopyFieldTotpCode() => const bridge.CopyField.totpCode(),
    CopyFieldCustom(:final name) => bridge.CopyField.customField(name),
  };
}

bridge.S3ConfigDto _s3Config(S3ConfigDto value) {
  return bridge.S3ConfigDto(
    bucket: value.bucket,
    key: value.key,
    region: value.region,
    endpoint: value.endpoint,
    pathStyle: value.pathStyle,
    accessKeyId: value.accessKeyId,
    secretAccessKey: value.secretAccessKey,
  );
}

final class BridgeSessionRepository implements SessionRepository {
  const BridgeSessionRepository(this._session);

  final bridge_api.AppSession _session;

  @override
  Future<VaultTree> unlock(
    String name,
    String masterPassword, {
    KeyfileRef? keyfile,
  }) {
    return _bridgeFuture(
      () => _session.unlock(
        name: name,
        masterPassword: masterPassword,
        keyfile: keyfile,
      ),
    );
  }

  @override
  Future<void> lockNow() => _bridgeFuture(_session.lockNow);

  @override
  Future<void> shutdown() => _bridgeFuture(_session.shutdown);

  @override
  Stream<LockEvent> lockEvents() => _bridgeStream(_session.lockEvents);

  @override
  BigInt droppedLockEvents() => _bridgeSync(_session.droppedLockEvents);

  @override
  void reportActivity() => _bridgeSync(_session.reportActivity);

  @override
  void reportLifecycleState(LifecycleStateDto state) {
    _bridgeSync(() => _session.reportLifecycleState(state: state));
  }

  @override
  Future<List<VaultSummary>> listVaults() async {
    final values = await _bridgeFuture(_session.listVaults);
    return values.map(_vaultSummary).toList();
  }

  @override
  Future<VaultSummary> createVault({
    required String name,
    required String masterPassword,
    String? fileName,
    KeyfileRef? keyfile,
    required bool confirmedNoRecovery,
  }) async {
    final value = await _bridgeFuture(
      () => _session.createVault(
        name: name,
        masterPassword: masterPassword,
        fileName: fileName,
        keyfile: keyfile,
        confirmedNoRecovery: confirmedNoRecovery,
      ),
    );
    return _vaultSummary(value);
  }

  @override
  Future<void> deregisterVault(String name, {required bool deleteFile}) {
    return _bridgeFuture(
      () => _session.deregisterVault(name: name, deleteFile: deleteFile),
    );
  }

  @override
  Future<void> changeMasterPassword(String current, String newPassword) {
    return _bridgeFuture(
      () => _session.changeMasterPassword(
        current: current,
        newPassword: newPassword,
      ),
    );
  }

  @override
  Future<VaultSummary> bootstrapFromRemote({
    required String name,
    required S3ConfigDto config,
    required String masterPassword,
    KeyfileRef? keyfile,
  }) async {
    final value = await _bridgeFuture(
      () => _session.bootstrapVaultFromRemote(
        name: name,
        cfg: _s3Config(config),
        masterPassword: masterPassword,
        keyfile: keyfile,
      ),
    );
    return _vaultSummary(value);
  }
}

final class BridgeEntryRepository implements EntryRepository {
  BridgeEntryRepository(this._session);

  final bridge_api.AppSession _session;

  @override
  void Function()? onMutation;

  Future<T> _mutation<T>(Future<T> Function() call) async {
    final value = await _bridgeFuture(call);
    onMutation?.call();
    return value;
  }

  @override
  Future<VaultTree> vaultTree() => _bridgeFuture(_session.vaultTree);

  @override
  Future<EntryDetail> entryDetail(String uuid) async {
    final value = await _bridgeFuture(() => _session.entryDetail(uuid: uuid));
    return _entryDetail(value);
  }

  @override
  Future<String> createEntry(String groupUuid, EntryDraftDto draft) {
    return _mutation(
      () => _session.createEntry(group: groupUuid, draft: _entryDraft(draft)),
    );
  }

  @override
  Future<void> updateEntry(String uuid, EntryEditDto edit) {
    return _mutation(
      () => _session.updateEntry(uuid: uuid, edit: _entryEdit(edit)),
    );
  }

  @override
  Future<void> deleteEntry(String uuid) {
    return _mutation(() => _session.deleteEntry(uuid: uuid));
  }

  @override
  Future<void> purgeEntry(String uuid) {
    return _mutation(() => _session.purgeEntry(uuid: uuid));
  }

  @override
  Future<void> moveEntry(String uuid, String groupUuid) {
    return _mutation(() => _session.moveEntry(uuid: uuid, group: groupUuid));
  }

  @override
  Future<List<HistorySummary>> entryHistory(String uuid) async {
    final values = await _bridgeFuture(() => _session.entryHistory(uuid: uuid));
    return [
      for (final value in values)
        HistorySummary(
          title: value.title,
          username: value.username,
          lastModificationTime: value.lastModificationTime,
        ),
    ];
  }

  @override
  Future<void> createGroup(String parentUuid, String name) {
    return _mutation(() async {
      await _session.createGroup(parent: parentUuid, name: name);
    });
  }

  @override
  Future<void> renameGroup(String uuid, String name) {
    return _mutation(() => _session.renameGroup(uuid: uuid, name: name));
  }

  @override
  Future<void> moveGroup(String uuid, String parentUuid) {
    return _mutation(() => _session.moveGroup(uuid: uuid, parent: parentUuid));
  }

  @override
  Future<void> deleteGroup(String uuid, GroupDeleteBehavior behavior) {
    final bridgeBehavior = switch (behavior) {
      GroupDeleteBehavior.refuse => bridge.GroupDeleteBehaviorDto.refuse,
      GroupDeleteBehavior.recurse => bridge.GroupDeleteBehaviorDto.recurse,
    };
    return _mutation(
      () => _session.deleteGroup(uuid: uuid, behavior: bridgeBehavior),
    );
  }

  @override
  Future<List<String>> listTags() => _bridgeFuture(_session.listTags);

  @override
  Future<void> setExpiration(String uuid, int epochSecs) {
    return _mutation(
      () => _session.setExpiration(uuid: uuid, epochSecs: epochSecs),
    );
  }

  @override
  Future<void> clearExpiration(String uuid) {
    return _mutation(() => _session.clearExpiration(uuid: uuid));
  }

  @override
  Future<List<AttachmentMeta>> listAttachments(String uuid) async {
    final values = await _bridgeFuture(
      () => _session.listAttachments(uuid: uuid),
    );
    return values.map(_attachmentMeta).toList();
  }

  @override
  Future<void> addAttachment(String uuid, String sourcePath) {
    return _mutation(
      () => _session.addAttachment(uuid: uuid, sourcePath: sourcePath),
    );
  }

  @override
  Future<void> removeAttachment(String uuid, String key) {
    return _mutation(() => _session.removeAttachment(uuid: uuid, key: key));
  }

  @override
  Future<void> saveAttachmentTo(String uuid, String key, String destPath) {
    return _bridgeFuture(
      () => _session.saveAttachmentTo(uuid: uuid, key: key, destPath: destPath),
    );
  }
}

final class BridgeSecretsRepository implements SecretsRepository {
  const BridgeSecretsRepository(this._session);

  final bridge_api.AppSession _session;

  @override
  Future<String> revealField(String uuid, RevealField field) {
    return _bridgeFuture(
      () => _session.revealField(uuid: uuid, field: _revealField(field)),
    );
  }

  @override
  Future<void> copyEntryField(String uuid, CopyField field) {
    return _bridgeFuture(
      () => _session.copyEntryField(uuid: uuid, field: _copyField(field)),
    );
  }
}

final class BridgeSearchRepository implements SearchRepository {
  const BridgeSearchRepository(this._session);

  final bridge_api.AppSession _session;

  @override
  Future<List<SearchHit>> search(SearchOptionsDto options) async {
    final bridgeMode = switch (options.mode) {
      SearchModeDto.substring => bridge.SearchModeDto.substring,
      SearchModeDto.wildcard => bridge.SearchModeDto.wildcard,
      SearchModeDto.fuzzy => bridge.SearchModeDto.fuzzy,
    };
    final values = await _bridgeFuture(
      () => _session.search(
        opts: bridge.SearchOptionsDto(
          query: options.query,
          mode: bridgeMode,
          scope: const bridge.SearchScopeDto.all(),
          includeRecycled: options.includeRecycled,
        ),
      ),
    );
    return [
      for (final value in values)
        SearchHit(
          entry: value.entry,
          matches: [
            for (final match in value.matches)
              SearchFieldMatchDto(
                field: MatchedFieldDto.values[match.field.index],
                ranges: match.ranges,
              ),
          ],
        ),
    ];
  }
}

final class BridgeTotpRepository implements TotpRepository {
  const BridgeTotpRepository(this._session);

  final bridge_api.AppSession _session;

  @override
  TotpCode? totpNow(String uuid) {
    try {
      final value = _session.totpNow(uuid: uuid);
      return TotpCode(
        code: value.code,
        remainingSecs: value.remainingSecs.toInt(),
        period: value.period.toInt(),
      );
    } on HidlinsApiError {
      return null;
    }
  }
}

final class BridgeGeneratorRepository implements GeneratorRepository {
  const BridgeGeneratorRepository(this._session);

  final bridge_api.AppSession _session;

  @override
  Future<GeneratedSecret> generatePassword(PasswordOptionsDto options) async {
    final value = await _bridgeFuture(
      () => _session.generatePassword(
        opts: bridge.PasswordOptionsDto(
          length: BigInt.from(options.length),
          lowercase: options.lowercase,
          uppercase: options.uppercase,
          digits: options.digits,
          symbols: options.symbols,
          excludeAmbiguous: options.excludeAmbiguous,
        ),
      ),
    );
    return GeneratedSecret(value: value.value, entropyBits: value.entropyBits);
  }

  @override
  Future<GeneratedSecret> generatePassphrase(
    PassphraseOptionsDto options,
  ) async {
    final value = await _bridgeFuture(
      () => _session.generatePassphrase(
        opts: bridge.PassphraseOptionsDto(
          words: BigInt.from(options.words),
          separator: options.separator,
        ),
      ),
    );
    return GeneratedSecret(value: value.value, entropyBits: value.entropyBits);
  }
}

final class BridgeSyncRepository implements SyncRepository {
  const BridgeSyncRepository(this._session);

  final bridge_api.AppSession _session;

  @override
  Future<SyncStatusDto> syncStatus() async {
    final value = await _bridgeFuture(_session.syncStatus);
    return SyncStatusDto(
      configured: value.configured,
      inFlight: value.inFlight,
      lastOutcome: _syncOutcome(value.lastOutcome),
    );
  }

  SyncOutcomeDto? _syncOutcome(bridge.SyncOutcomeDto? value) {
    return switch (value) {
      null => null,
      bridge.SyncOutcomeDto_AlreadyInSync() => SyncOutcomeDto.alreadyInSync,
      bridge.SyncOutcomeDto_Pushed() => SyncOutcomeDto.pushed,
      bridge.SyncOutcomeDto_FastReplaced() => SyncOutcomeDto.fastReplaced,
      bridge.SyncOutcomeDto_Merged() => SyncOutcomeDto.merged,
      bridge.SyncOutcomeDto_Unknown() => SyncOutcomeDto.unknown,
    };
  }

  @override
  Future<void> configureSync(S3ConfigDto config) {
    return _bridgeFuture(() => _session.configureSync(cfg: _s3Config(config)));
  }

  @override
  Future<void> syncNow() async {
    await _bridgeFuture(_session.syncNow);
  }

  @override
  Future<void> clearSyncConfig(String name) {
    return _bridgeFuture(() => _session.clearSyncConfig(name: name));
  }

  @override
  Stream<SyncEvent> syncEvents() => _bridgeStream(_session.syncEvents);

  @override
  Stream<ClipboardEvent> clipboardEvents() {
    return _bridgeStream(_session.clipboardEvents).map(
      (event) => ClipboardEvent(
        remainingSecs: event.remainingSecs,
        cleared: event.cleared,
      ),
    );
  }
}

final class BridgePrefsRepository implements PrefsRepository {
  const BridgePrefsRepository(this._session);

  final bridge_api.AppSession _session;

  @override
  Future<UiPrefs> getPrefs() async {
    final value = await _bridgeFuture(_session.getPrefs);
    return UiPrefs(
      themeMode: value.themeMode,
      windowX: value.windowX,
      windowY: value.windowY,
      windowWidth: value.windowWidth,
      windowHeight: value.windowHeight,
      lastVault: value.lastVault,
      listPaneWidth: value.listPaneWidth,
    );
  }

  @override
  Future<void> setPrefs(UiPrefs prefs) {
    return _bridgeFuture(
      () => _session.setPrefs(
        prefs: bridge.UiPrefs(
          themeMode: prefs.themeMode,
          windowX: prefs.windowX,
          windowY: prefs.windowY,
          windowWidth: prefs.windowWidth,
          windowHeight: prefs.windowHeight,
          lastVault: prefs.lastVault,
          listPaneWidth: prefs.listPaneWidth,
        ),
      ),
    );
  }
}
