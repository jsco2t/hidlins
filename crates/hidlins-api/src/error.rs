use hidlins_core::VaultError;
use hidlins_genpw::GenError;
use hidlins_security::SecurityError;
use hidlins_sync::SyncError;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum HidlinsApiError {
    #[error("authentication failed")]
    AuthenticationFailed,

    #[error("vault is locked")]
    VaultLocked,

    #[error("vault is busy syncing")]
    VaultBusySyncing,

    #[error("vault is held by another process")]
    VaultContended { holder_pid: Option<u32> },

    #[error("path already exists: {path}")]
    PathExists { path: String },

    #[error("file not found: {path}")]
    FileNotFound { path: String },

    #[error("keyfile is required")]
    KeyfileRequired,

    #[error("platform capability is unsupported: {capability}")]
    UnsupportedPlatform { capability: String },

    #[error("vault registry changed concurrently; reload and retry")]
    RegistryChanged,

    #[error("invalid vault format")]
    InvalidFormat,

    #[error("vault registry is malformed")]
    RegistryMalformed,

    #[error("sync is not configured")]
    SyncNotConfigured,

    #[error("local sync configuration is invalid")]
    LocalSyncConfiguration,

    #[error("local-network permission was denied or restricted")]
    SyncPermissionDenied,

    #[error("no local sync server was found")]
    SyncNotFound,

    #[error("the local sync peer identity did not match")]
    SyncKeyMismatch,

    #[error("the local sync peer is revoked")]
    SyncRevoked,

    #[error("local sync is busy")]
    SyncBusy,

    #[error("the authoritative vault advanced concurrently")]
    SyncConflict,

    #[error("local sync was canceled")]
    SyncCanceled,

    #[error("the local sync server is offline")]
    SyncOffline,

    #[error("sync endpoint unreachable")]
    SyncRemoteUnreachable { endpoint: Option<String> },

    #[error("sync authentication failed")]
    SyncAuthFailed,

    #[error("sync conflict cannot be auto-resolved; backup at {backup_path}")]
    SyncConflictUnresolvable { backup_path: String },

    #[error("invalid input: {field}: {reason}")]
    InvalidInput { field: String, reason: String },

    #[error("I/O error: {context}")]
    Io { context: String },

    #[error("internal error: {context}")]
    Internal { context: String },
}

impl From<VaultError> for HidlinsApiError {
    fn from(err: VaultError) -> Self {
        match err {
            VaultError::AuthenticationFailed => Self::AuthenticationFailed,
            VaultError::Contended { holder } => Self::VaultContended { holder_pid: holder },
            VaultError::PathExists { path } => Self::PathExists {
                path: path.display().to_string(),
            },
            VaultError::FileNotFound { path } => Self::FileNotFound {
                path: path.display().to_string(),
            },
            VaultError::RegistryChanged => Self::RegistryChanged,
            VaultError::InvalidFormat { .. } => Self::InvalidFormat,
            VaultError::RegistryMalformed { .. } => Self::RegistryMalformed,
            VaultError::HomeUnresolvable => Self::Io {
                context: "HOME is not set or not resolvable".to_string(),
            },
            VaultError::Io { source, path } => Self::Io {
                // Keep the portable error category for actionable diagnostics,
                // but never forward the OS/source message: a source supplied by
                // a dependency can contain secret-bearing input.
                context: format!("on {} ({:?})", path.display(), source.kind()),
            },
            VaultError::WriteFailed { .. } => Self::Io {
                context: "KDBX write failed".to_string(),
            },
            VaultError::RegistrySerializationFailed { .. } => Self::Io {
                context: "registry serialization failed".to_string(),
            },
            VaultError::NotRegistered { name } => Self::FileNotFound { path: name },
            VaultError::AlreadyRegistered { name } => Self::PathExists { path: name },
            VaultError::NoRecoveryNotConfirmed => Self::InvalidInput {
                field: "confirmed_no_recovery".to_string(),
                reason: "no-recovery warning must be confirmed before vault creation".to_string(),
            },
            VaultError::EntryNotFound { uuid } => Self::InvalidInput {
                field: "uuid".to_string(),
                reason: format!("entry not found: {uuid}"),
            },
            VaultError::GroupNotFound { uuid } => Self::InvalidInput {
                field: "group".to_string(),
                reason: format!("group not found: {uuid}"),
            },
            VaultError::EntryHasNoTotp { uuid } => Self::InvalidInput {
                field: "uuid".to_string(),
                reason: format!("entry has no TOTP: {uuid}"),
            },
            VaultError::AttachmentTooLarge { actual, limit } => Self::InvalidInput {
                field: "attachment".to_string(),
                reason: format!("{actual} bytes exceeds limit of {limit} bytes"),
            },
            VaultError::AttachmentNotFound { .. } => Self::InvalidInput {
                field: "attachment".to_string(),
                reason: "attachment not found".to_string(),
            },
            VaultError::InvalidOtpUri { .. } => Self::InvalidInput {
                field: "totp_uri".to_string(),
                reason: "invalid otpauth URI".to_string(),
            },
            VaultError::InvalidAttachmentCap => Self::InvalidInput {
                field: "attachment_cap".to_string(),
                reason: "invalid attachment cap".to_string(),
            },
            VaultError::GroupNotEmpty { uuid } => Self::InvalidInput {
                field: "group".to_string(),
                reason: format!("group is not empty: {uuid}"),
            },
            VaultError::InvalidTag { .. } => Self::InvalidInput {
                field: "tag".to_string(),
                reason: "tag contains the forbidden ';' delimiter".to_string(),
            },
            VaultError::CannotModifyRoot => Self::InvalidInput {
                field: "group".to_string(),
                reason: "cannot move or delete the root group".to_string(),
            },
            VaultError::InvalidGroupTarget { reason } => Self::InvalidInput {
                field: "group".to_string(),
                reason: reason.to_string(),
            },
            VaultError::DatabaseIdentityMismatch { expected, found } => Self::Internal {
                context: format!("database identity mismatch: {found} vs {expected}"),
            },
            _ => Self::Internal {
                context: "unrecognized vault error".to_string(),
            },
        }
    }
}

impl From<SyncError> for HidlinsApiError {
    fn from(err: SyncError) -> Self {
        match err {
            SyncError::LocalNetwork(hidlins_sync::client::LanError::Unreachable) => {
                Self::SyncOffline
            }
            SyncError::LocalNetwork(hidlins_sync::client::LanError::Authentication) => {
                Self::SyncKeyMismatch
            }
            SyncError::LocalNetwork(hidlins_sync::client::LanError::StaleVersion) => {
                Self::SyncConflict
            }
            SyncError::LocalNetwork(hidlins_sync::client::LanError::Busy) => Self::SyncBusy,
            SyncError::LocalNetwork(hidlins_sync::client::LanError::Cancelled) => {
                Self::SyncCanceled
            }
            SyncError::LocalNetwork(hidlins_sync::client::LanError::TimedOut) => Self::SyncOffline,
            SyncError::LocalNetwork(
                hidlins_sync::client::LanError::Protocol | hidlins_sync::client::LanError::Internal,
            ) => Self::Internal {
                context: "local sync protocol failed".to_string(),
            },
            SyncError::LocalConfig(_) => Self::LocalSyncConfiguration,
            SyncError::NotConfigured => Self::SyncNotConfigured,
            SyncError::RemoteUnreachable { endpoint, .. } => Self::SyncRemoteUnreachable {
                endpoint: Some(endpoint),
            },
            SyncError::ConditionalCommitExhausted { .. } => Self::Internal {
                context: "conditional commit exhausted; retry sync".to_string(),
            },
            SyncError::MasterPasswordMismatch => Self::AuthenticationFailed,
            SyncError::Unresolvable { backup_path, .. } => Self::SyncConflictUnresolvable {
                backup_path: backup_path.display().to_string(),
            },
            SyncError::BackupFailed { .. } => Self::Io {
                context: "pre-merge backup creation failed".to_string(),
            },
            SyncError::Merge(_) => Self::Internal {
                context: "merge engine error".to_string(),
            },
            SyncError::Vault(error) => Self::from(error),
            SyncError::VaultIo { path, .. } => Self::Io {
                context: format!("vault I/O during sync: {}", path.display()),
            },
            _ => Self::Internal {
                context: "unrecognized sync error".to_string(),
            },
        }
    }
}
impl From<SecurityError> for HidlinsApiError {
    fn from(err: SecurityError) -> Self {
        match err {
            SecurityError::ClipboardUnavailable(_) => Self::Io {
                context: "clipboard unavailable".to_string(),
            },
            SecurityError::ClipboardIo { .. } => Self::Io {
                context: "clipboard operation failed".to_string(),
            },
            SecurityError::EventSourceStart { name, .. } => Self::Io {
                context: format!("OS event source '{name}' failed"),
            },
            SecurityError::EventChannelClosed { name } => Self::Io {
                context: format!("OS event source '{name}' channel closed"),
            },
            SecurityError::InvalidVaultLockConfig { .. } => Self::InvalidInput {
                field: "lock_config".to_string(),
                reason: "invalid per-vault lock configuration".to_string(),
            },
            SecurityError::InvalidAutoLockConfig { .. } => Self::InvalidInput {
                field: "auto_lock_config".to_string(),
                reason: "invalid auto-lock configuration".to_string(),
            },
            _ => Self::Internal {
                context: "unrecognized security error".to_string(),
            },
        }
    }
}

impl From<GenError> for HidlinsApiError {
    fn from(err: GenError) -> Self {
        match err {
            GenError::InvalidLength => Self::InvalidInput {
                field: "length".to_string(),
                reason: "must be at least 1".to_string(),
            },
            GenError::NoClassesEnabled => Self::InvalidInput {
                field: "classes".to_string(),
                reason: "at least one character class must be enabled".to_string(),
            },
            GenError::LengthTooShort { length, classes } => Self::InvalidInput {
                field: "length".to_string(),
                reason: format!(
                    "length {length} cannot satisfy at-least-one-of-each for {classes} classes"
                ),
            },
            GenError::AlphabetEmpty => Self::InvalidInput {
                field: "classes".to_string(),
                reason: "alphabet is empty after ambiguous filter".to_string(),
            },
            GenError::InvalidWordCount => Self::InvalidInput {
                field: "words".to_string(),
                reason: "must be at least 1".to_string(),
            },
            GenError::Csprng(_) => Self::Internal {
                context: "OS CSPRNG failure".to_string(),
            },
            // GenError is not #[non_exhaustive] today, but defensive:
            #[allow(unreachable_patterns)]
            _ => Self::Internal {
                context: "unrecognized password-generation error".to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARKER_SECRET: &str = "MARKER-p@ss-CR3D3NTIAL";

    fn assert_no_secret(err: &HidlinsApiError) {
        let display = format!("{err}");
        let debug = format!("{err:?}");
        assert!(
            !display.contains(MARKER_SECRET),
            "Display contains secret: {display}"
        );
        assert!(
            !debug.contains(MARKER_SECRET),
            "Debug contains secret: {debug}"
        );
    }

    fn invalid_input(field: &str, reason: impl Into<String>) -> HidlinsApiError {
        HidlinsApiError::InvalidInput {
            field: field.to_string(),
            reason: reason.into(),
        }
    }

    fn assert_vault_mappings(cases: Vec<(VaultError, HidlinsApiError)>) {
        for (upstream, expected) in cases {
            let case = format!("{upstream:?}");
            assert_eq!(HidlinsApiError::from(upstream), expected, "case: {case}");
        }
    }

    #[test]
    fn vault_filesystem_registry_and_auth_errors_map_exactly() {
        use std::path::PathBuf;

        assert_vault_mappings(vec![
            (
                VaultError::HomeUnresolvable,
                HidlinsApiError::Io {
                    context: "HOME is not set or not resolvable".to_string(),
                },
            ),
            (
                VaultError::PathExists {
                    path: PathBuf::from("/test"),
                },
                HidlinsApiError::PathExists {
                    path: "/test".to_string(),
                },
            ),
            (
                VaultError::FileNotFound {
                    path: PathBuf::from("/test"),
                },
                HidlinsApiError::FileNotFound {
                    path: "/test".to_string(),
                },
            ),
            (
                VaultError::AuthenticationFailed,
                HidlinsApiError::AuthenticationFailed,
            ),
            (
                VaultError::Contended { holder: Some(1234) },
                HidlinsApiError::VaultContended {
                    holder_pid: Some(1234),
                },
            ),
            (
                VaultError::RegistryChanged,
                HidlinsApiError::RegistryChanged,
            ),
            (
                VaultError::RegistryMalformed { source: None },
                HidlinsApiError::RegistryMalformed,
            ),
            (
                VaultError::Io {
                    source: std::io::Error::other("source detail"),
                    path: PathBuf::from("/test"),
                },
                HidlinsApiError::Io {
                    context: "on /test (Other)".to_string(),
                },
            ),
            (
                VaultError::NotRegistered {
                    name: "test".to_string(),
                },
                HidlinsApiError::FileNotFound {
                    path: "test".to_string(),
                },
            ),
            (
                VaultError::AlreadyRegistered {
                    name: "test".to_string(),
                },
                HidlinsApiError::PathExists {
                    path: "test".to_string(),
                },
            ),
        ]);
    }

    #[test]
    fn vault_entry_and_group_errors_map_exactly() {
        use hidlins_core::Uuid;

        let nil = Uuid::nil();
        assert_vault_mappings(vec![
            (
                VaultError::NoRecoveryNotConfirmed,
                invalid_input(
                    "confirmed_no_recovery",
                    "no-recovery warning must be confirmed before vault creation",
                ),
            ),
            (
                VaultError::EntryNotFound { uuid: nil },
                invalid_input("uuid", format!("entry not found: {nil}")),
            ),
            (
                VaultError::GroupNotFound { uuid: nil },
                invalid_input("group", format!("group not found: {nil}")),
            ),
            (
                VaultError::EntryHasNoTotp { uuid: nil },
                invalid_input("uuid", format!("entry has no TOTP: {nil}")),
            ),
            (
                VaultError::AttachmentTooLarge {
                    actual: 10,
                    limit: 5,
                },
                invalid_input("attachment", "10 bytes exceeds limit of 5 bytes"),
            ),
            (
                VaultError::AttachmentNotFound {
                    name: "x".to_string(),
                },
                invalid_input("attachment", "attachment not found"),
            ),
            (
                VaultError::InvalidAttachmentCap,
                invalid_input("attachment_cap", "invalid attachment cap"),
            ),
            (
                VaultError::GroupNotEmpty { uuid: nil },
                invalid_input("group", format!("group is not empty: {nil}")),
            ),
            (
                VaultError::InvalidTag {
                    value: "a;b".to_string(),
                },
                invalid_input("tag", "tag contains the forbidden ';' delimiter"),
            ),
            (
                VaultError::CannotModifyRoot,
                invalid_input("group", "cannot move or delete the root group"),
            ),
            (
                VaultError::InvalidGroupTarget { reason: "test" },
                invalid_input("group", "test"),
            ),
            (
                VaultError::DatabaseIdentityMismatch {
                    expected: nil,
                    found: nil,
                },
                HidlinsApiError::Internal {
                    context: format!("database identity mismatch: {nil} vs {nil}"),
                },
            ),
        ]);

        let invalid_otp = hidlins_core::Totp::from_otpauth_uri("not-an-otpauth-uri")
            .expect_err("malformed URI should fail");
        assert_eq!(
            HidlinsApiError::from(invalid_otp),
            invalid_input("totp_uri", "invalid otpauth URI")
        );
    }

    #[test]
    fn constructible_sync_errors_map_to_exact_api_categories() {
        use std::path::PathBuf;

        let cases: Vec<(SyncError, HidlinsApiError)> = vec![
            (SyncError::NotConfigured, HidlinsApiError::SyncNotConfigured),
            (
                SyncError::RemoteUnreachable {
                    endpoint: "local-authority".to_string(),
                    source: Box::new(std::io::Error::other("timeout")),
                },
                HidlinsApiError::SyncRemoteUnreachable {
                    endpoint: Some("local-authority".to_string()),
                },
            ),
            (
                SyncError::ConditionalCommitExhausted { attempts: 3 },
                HidlinsApiError::Internal {
                    context: "conditional commit exhausted; retry sync".to_string(),
                },
            ),
            (
                SyncError::MasterPasswordMismatch,
                HidlinsApiError::AuthenticationFailed,
            ),
            (
                SyncError::Unresolvable {
                    reason: "same-second".to_string(),
                    backup_path: PathBuf::from("/test.bak"),
                },
                HidlinsApiError::SyncConflictUnresolvable {
                    backup_path: "/test.bak".to_string(),
                },
            ),
            (
                SyncError::BackupFailed {
                    source: std::io::Error::other("test"),
                },
                HidlinsApiError::Io {
                    context: "pre-merge backup creation failed".to_string(),
                },
            ),
            (
                SyncError::Merge(hidlins_sync::MergeError::Unresolvable {
                    reason: "conflict".to_string(),
                }),
                HidlinsApiError::Internal {
                    context: "merge engine error".to_string(),
                },
            ),
            (
                SyncError::Vault(VaultError::AuthenticationFailed),
                HidlinsApiError::AuthenticationFailed,
            ),
            (
                SyncError::VaultIo {
                    path: PathBuf::from("/test"),
                    source: std::io::Error::other("test"),
                },
                HidlinsApiError::Io {
                    context: "vault I/O during sync: /test".to_string(),
                },
            ),
        ];

        for (upstream, expected) in cases {
            let case = format!("{upstream:?}");
            assert_eq!(HidlinsApiError::from(upstream), expected, "case: {case}");
        }
    }

    #[test]
    fn every_current_security_error_maps_to_the_exact_api_category() {
        let cases = vec![
            (
                SecurityError::ClipboardUnavailable("no DISPLAY".to_string()),
                HidlinsApiError::Io {
                    context: "clipboard unavailable".to_string(),
                },
            ),
            (
                SecurityError::ClipboardIo {
                    detail: "test".to_string(),
                },
                HidlinsApiError::Io {
                    context: "clipboard operation failed".to_string(),
                },
            ),
            (
                SecurityError::EventSourceStart {
                    name: "test",
                    detail: "test".to_string(),
                },
                HidlinsApiError::Io {
                    context: "OS event source 'test' failed".to_string(),
                },
            ),
            (
                SecurityError::EventChannelClosed { name: "test" },
                HidlinsApiError::Io {
                    context: "OS event source 'test' channel closed".to_string(),
                },
            ),
            (
                SecurityError::InvalidVaultLockConfig {
                    detail: "test".to_string(),
                },
                HidlinsApiError::InvalidInput {
                    field: "lock_config".to_string(),
                    reason: "invalid per-vault lock configuration".to_string(),
                },
            ),
            (
                SecurityError::InvalidAutoLockConfig {
                    detail: "test".to_string(),
                },
                HidlinsApiError::InvalidInput {
                    field: "auto_lock_config".to_string(),
                    reason: "invalid auto-lock configuration".to_string(),
                },
            ),
        ];

        for (upstream, expected) in cases {
            let case = format!("{upstream:?}");
            assert_eq!(HidlinsApiError::from(upstream), expected, "case: {case}");
        }
    }

    #[test]
    fn every_constructible_generation_error_maps_to_the_exact_api_category() {
        let cases = vec![
            (
                GenError::InvalidLength,
                HidlinsApiError::InvalidInput {
                    field: "length".to_string(),
                    reason: "must be at least 1".to_string(),
                },
            ),
            (
                GenError::NoClassesEnabled,
                HidlinsApiError::InvalidInput {
                    field: "classes".to_string(),
                    reason: "at least one character class must be enabled".to_string(),
                },
            ),
            (
                GenError::LengthTooShort {
                    length: 2,
                    classes: 4,
                },
                HidlinsApiError::InvalidInput {
                    field: "length".to_string(),
                    reason: "length 2 cannot satisfy at-least-one-of-each for 4 classes"
                        .to_string(),
                },
            ),
            (
                GenError::AlphabetEmpty,
                HidlinsApiError::InvalidInput {
                    field: "classes".to_string(),
                    reason: "alphabet is empty after ambiguous filter".to_string(),
                },
            ),
            (
                GenError::InvalidWordCount,
                HidlinsApiError::InvalidInput {
                    field: "words".to_string(),
                    reason: "must be at least 1".to_string(),
                },
            ),
        ];

        for (upstream, expected) in cases {
            let case = format!("{upstream:?}");
            assert_eq!(HidlinsApiError::from(upstream), expected, "case: {case}");
        }
    }

    #[test]
    fn rendered_messages_never_contain_secret_material() {
        let marker_vault_err = VaultError::Io {
            source: std::io::Error::other(MARKER_SECRET),
            path: std::path::PathBuf::from("/vaults/test.kdbx"),
        };
        let api_err = HidlinsApiError::from(marker_vault_err);
        let display = format!("{api_err}");
        assert!(
            !display.contains(MARKER_SECRET),
            "VaultError::Io source error message leaked: {display}"
        );

        for marker_vault_err in [
            VaultError::AttachmentNotFound {
                name: MARKER_SECRET.to_string(),
            },
            VaultError::InvalidTag {
                value: MARKER_SECRET.to_string(),
            },
        ] {
            assert_no_secret(&HidlinsApiError::from(marker_vault_err));
        }

        for marker_security_err in [
            SecurityError::ClipboardUnavailable(MARKER_SECRET.to_string()),
            SecurityError::ClipboardIo {
                detail: MARKER_SECRET.to_string(),
            },
            SecurityError::EventSourceStart {
                name: "test",
                detail: MARKER_SECRET.to_string(),
            },
            SecurityError::InvalidVaultLockConfig {
                detail: MARKER_SECRET.to_string(),
            },
            SecurityError::InvalidAutoLockConfig {
                detail: MARKER_SECRET.to_string(),
            },
        ] {
            assert_no_secret(&HidlinsApiError::from(marker_security_err));
        }
    }

    #[test]
    fn authentication_failure_is_indistinct_between_password_and_keyfile() {
        let from_password = HidlinsApiError::from(VaultError::AuthenticationFailed);
        let from_sync_mismatch = HidlinsApiError::from(SyncError::MasterPasswordMismatch);
        assert!(matches!(
            from_password,
            HidlinsApiError::AuthenticationFailed
        ));
        assert!(matches!(
            from_sync_mismatch,
            HidlinsApiError::AuthenticationFailed
        ));
    }
}
