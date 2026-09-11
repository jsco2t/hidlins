import '../bridge/dto.dart';

export '../bridge/dto.dart'
    show
        AppInitConfig,
        EntryKindDto,
        EntrySummary,
        GroupNode,
        KeyfileRef,
        LifecycleStateDto,
        LockEvent,
        SyncEvent,
        SyncEvent_Started,
        SyncEvent_Activity,
        SyncEvent_Done,
        SyncEvent_Failed,
        VaultTree;

class VaultSummary {
  final String name;
  final String path;
  final bool hasKeyfile;
  final bool hasSync;

  const VaultSummary({
    required this.name,
    required this.path,
    required this.hasKeyfile,
    required this.hasSync,
  });
}

class EntryDetail {
  final String uuid;
  final String title;
  final String username;
  final bool hasPassword;
  final String url;
  final String notes;
  final EntryKindDto kind;
  final List<String> tags;
  final List<CustomFieldDto> customFields;
  final List<AttachmentMeta> attachments;
  final int? creationTime;
  final int? lastModificationTime;
  final int? expiryTime;

  const EntryDetail({
    required this.uuid,
    required this.title,
    required this.username,
    required this.hasPassword,
    required this.url,
    required this.notes,
    required this.kind,
    required this.tags,
    required this.customFields,
    required this.attachments,
    this.creationTime,
    this.lastModificationTime,
    this.expiryTime,
  });
}

class CustomFieldDto {
  final String name;
  final bool isProtected;

  const CustomFieldDto({required this.name, required this.isProtected});
}

class AttachmentMeta {
  final String name;
  final int sizeBytes;

  const AttachmentMeta({required this.name, required this.sizeBytes});
}

class HistorySummary {
  final String title;
  final String username;
  final int? lastModificationTime;

  const HistorySummary({
    required this.title,
    required this.username,
    this.lastModificationTime,
  });
}

class EntryDraftDto {
  final EntryKindDto kind;
  final String title;
  final String? username;
  final String? password;
  final String? url;
  final String? notes;
  final List<String> tags;
  final List<CustomFieldInputDto> customFields;
  final String? totpUri;

  const EntryDraftDto({
    required this.kind,
    required this.title,
    this.username,
    this.password,
    this.url,
    this.notes,
    this.tags = const [],
    this.customFields = const [],
    this.totpUri,
  });

  @override
  String toString() => 'EntryDraftDto(***)';
}

class EntryEditDto {
  final String? title;
  final String? username;
  final String? password;
  final String? url;
  final String? notes;
  final List<String>? tags;
  final List<CustomFieldInputDto>? customFields;
  final String? totpUri;

  const EntryEditDto({
    this.title,
    this.username,
    this.password,
    this.url,
    this.notes,
    this.tags,
    this.customFields,
    this.totpUri,
  });

  @override
  String toString() => 'EntryEditDto(***)';
}

class CustomFieldInputDto {
  final String name;
  final String value;
  final bool protected;

  const CustomFieldInputDto({
    required this.name,
    required this.value,
    this.protected = false,
  });

  @override
  String toString() => 'CustomFieldInputDto(***)';
}

enum SearchModeDto { substring, wildcard, fuzzy }

class SearchOptionsDto {
  final String query;
  final SearchModeDto mode;
  final bool includeRecycled;

  const SearchOptionsDto({
    required this.query,
    this.mode = SearchModeDto.substring,
    this.includeRecycled = false,
  });
}

enum MatchedFieldDto { title, username, url, notes, tags }

class SearchHit {
  final EntrySummary entry;
  final List<SearchFieldMatchDto> matches;

  const SearchHit({required this.entry, required this.matches});
}

class SearchFieldMatchDto {
  final MatchedFieldDto field;
  final List<(int, int)> ranges;

  const SearchFieldMatchDto({required this.field, required this.ranges});
}

class TotpCode {
  final String code;
  final int remainingSecs;
  final int period;

  const TotpCode({
    required this.code,
    required this.remainingSecs,
    required this.period,
  });

  @override
  String toString() => 'TotpCode(***)';
}

class PasswordOptionsDto {
  final int length;
  final bool lowercase;
  final bool uppercase;
  final bool digits;
  final bool symbols;
  final bool excludeAmbiguous;

  const PasswordOptionsDto({
    this.length = 20,
    this.lowercase = true,
    this.uppercase = true,
    this.digits = true,
    this.symbols = true,
    this.excludeAmbiguous = false,
  });
}

class PassphraseOptionsDto {
  final int words;
  final String separator;

  const PassphraseOptionsDto({this.words = 6, this.separator = '-'});
}

class GeneratedSecret {
  final String value;
  final double entropyBits;

  const GeneratedSecret({required this.value, required this.entropyBits});

  @override
  String toString() => 'GeneratedSecret(***)';
}

sealed class RevealField {
  const RevealField();
  static const password = RevealFieldPassword();
  static const totpUri = RevealFieldTotpUri();
}

class RevealFieldPassword extends RevealField {
  const RevealFieldPassword();
}

class RevealFieldCustom extends RevealField {
  final String name;
  const RevealFieldCustom(this.name);
}

class RevealFieldTotpUri extends RevealField {
  const RevealFieldTotpUri();
}

sealed class CopyField {
  const CopyField();
  static const username = CopyFieldUsername();
  static const password = CopyFieldPassword();
  static const totpCode = CopyFieldTotpCode();
}

class CopyFieldUsername extends CopyField {
  const CopyFieldUsername();
}

class CopyFieldPassword extends CopyField {
  const CopyFieldPassword();
}

class CopyFieldTotpCode extends CopyField {
  const CopyFieldTotpCode();
}

class CopyFieldCustom extends CopyField {
  final String name;
  const CopyFieldCustom(this.name);
}

class UiPrefs {
  final String? themeMode;
  final int? windowX;
  final int? windowY;
  final int? windowWidth;
  final int? windowHeight;
  final String? lastVault;
  final double? listPaneWidth;

  const UiPrefs({
    this.themeMode,
    this.windowX,
    this.windowY,
    this.windowWidth,
    this.windowHeight,
    this.lastVault,
    this.listPaneWidth,
  });

  UiPrefs copyWith({
    Object? themeMode = _sentinel,
    Object? windowX = _sentinel,
    Object? windowY = _sentinel,
    Object? windowWidth = _sentinel,
    Object? windowHeight = _sentinel,
    Object? lastVault = _sentinel,
    Object? listPaneWidth = _sentinel,
  }) {
    return UiPrefs(
      themeMode: themeMode == _sentinel ? this.themeMode : themeMode as String?,
      windowX: windowX == _sentinel ? this.windowX : windowX as int?,
      windowY: windowY == _sentinel ? this.windowY : windowY as int?,
      windowWidth: windowWidth == _sentinel
          ? this.windowWidth
          : windowWidth as int?,
      windowHeight: windowHeight == _sentinel
          ? this.windowHeight
          : windowHeight as int?,
      lastVault: lastVault == _sentinel ? this.lastVault : lastVault as String?,
      listPaneWidth: listPaneWidth == _sentinel
          ? this.listPaneWidth
          : listPaneWidth as double?,
    );
  }
}

const _sentinel = Object();

class SyncStatusDto {
  final bool configured;
  final bool inFlight;
  final SyncOutcomeDto? lastOutcome;
  final LocalSyncRole? role;
  final bool paired;
  final int activePeerCount;
  final bool serverEnabled;
  final bool serverRunning;
  final bool pairingOpen;

  const SyncStatusDto({
    required this.configured,
    required this.inFlight,
    this.lastOutcome,
    this.role,
    this.paired = false,
    this.activePeerCount = 0,
    this.serverEnabled = false,
    this.serverRunning = false,
    this.pairingOpen = false,
  });
}

enum SyncOutcomeDto { alreadyInSync, pushed, fastReplaced, merged, unknown }

enum LocalSyncRole { server, client }

enum DiscoveryKind { trusted, pairing }

enum LocalDiscoveryPermission {
  notDetermined,
  granted,
  denied,
  restricted,
  unavailable,
}

class LocalEndpoint {
  final String address;
  final int port;
  final int scopeId;

  const LocalEndpoint({
    required this.address,
    required this.port,
    this.scopeId = 0,
  });

  @override
  String toString() =>
      scopeId == 0 ? '$address:$port' : '[$address%$scopeId]:$port';
}

class LocalDiscoveryStatus {
  final LocalDiscoveryPermission permission;
  final List<LocalEndpoint> candidates;

  const LocalDiscoveryStatus({
    required this.permission,
    this.candidates = const [],
  });
}

class PairingPrompt {
  final String transactionHandle;
  final String sas;

  const PairingPrompt({required this.transactionHandle, required this.sas});

  @override
  String toString() => 'PairingPrompt(***)';
}

class SyncPeer {
  final String peerId;
  final String displayName;
  final bool revoked;

  const SyncPeer({
    required this.peerId,
    required this.displayName,
    required this.revoked,
  });
}

enum GroupDeleteBehavior { refuse, recurse }
