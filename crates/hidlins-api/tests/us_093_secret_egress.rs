mod common;

use std::sync::Arc;

use hidlins_api::api::session::AppSession;
use hidlins_api::clipboard_port::{ClipboardPort, RecordingClipboard};
use hidlins_api::dto::{CopyField, CustomFieldInputDto, EntryDraftDto, EntryKindDto, RevealField};
use hidlins_api::error::HidlinsApiError;
use hidlins_api::sync_port::SucceedingSyncEngine;

use common::{create_test_vault, register_vault, TestEnv};

const TOTP_URI: &str = "otpauth://totp/test:user@example.com?secret=JBSWY3DPEHPK3PXP&issuer=test&algorithm=SHA1&digits=6&period=30";

/// Create a session with a `RecordingClipboard` and a no-op sync engine,
/// unlock the vault, and return the session, clipboard handle, and root
/// group UUID (needed by `create_entry`).
fn unlocked_session_with_clipboard(
    env: &TestEnv,
    name: &str,
    password: &str,
) -> (AppSession, Arc<RecordingClipboard>, String) {
    let vault_path = create_test_vault(env, name, password);
    register_vault(env, name, &vault_path);

    let clipboard = Arc::new(RecordingClipboard::new());
    // Explicit coercion site: `Some(Arc<RecordingClipboard>)` does not
    // always unsize to `Option<Arc<dyn ClipboardPort>>` through inference.
    let clipboard_port: Arc<dyn ClipboardPort> = clipboard.clone();
    let sync_engine = Arc::new(SucceedingSyncEngine(
        hidlins_sync::SyncOutcome::AlreadyInSync,
    ));
    let session = AppSession::with_ports(env.paths_clone(), Some(clipboard_port), sync_engine)
        .expect("create session with ports");

    let tree = session
        .unlock(name.to_string(), password.to_string(), None)
        .expect("unlock");
    let root_uuid = tree.root.uuid.clone();

    (session, clipboard, root_uuid)
}

// ---------------------------------------------------------------------------
// reveal_field
// ---------------------------------------------------------------------------

#[test]
fn reveal_field_returns_password_for_password_variant() {
    let env = TestEnv::new();
    let (session, _clipboard, root_uuid) =
        unlocked_session_with_clipboard(&env, "reveal-pw", "test-pass");

    let draft = EntryDraftDto {
        kind: EntryKindDto::Credential,
        title: "Reveal PW Test".to_string(),
        username: Some("user".to_string()),
        password: Some("secret-password-42".to_string()),
        url: None,
        notes: None,
        tags: Vec::new(),
        custom_fields: Vec::new(),
        totp_uri: None,
    };
    let uuid = session
        .create_entry(root_uuid, draft)
        .expect("create entry");

    let revealed = session
        .reveal_field(uuid, RevealField::Password)
        .expect("reveal password");

    assert_eq!(revealed, "secret-password-42");
}

#[test]
fn reveal_field_returns_custom_field_value() {
    let env = TestEnv::new();
    let (session, _clipboard, root_uuid) =
        unlocked_session_with_clipboard(&env, "reveal-cf", "test-pass");

    let draft = EntryDraftDto {
        kind: EntryKindDto::Credential,
        title: "Custom Field Test".to_string(),
        username: None,
        password: None,
        url: None,
        notes: None,
        tags: Vec::new(),
        custom_fields: vec![CustomFieldInputDto {
            name: "pin".to_string(),
            value: "1234".to_string(),
            protected: true,
        }],
        totp_uri: None,
    };
    let uuid = session
        .create_entry(root_uuid, draft)
        .expect("create entry");

    let revealed = session
        .reveal_field(uuid, RevealField::CustomField("pin".to_string()))
        .expect("reveal custom field");

    assert_eq!(revealed, "1234");
}

// ---------------------------------------------------------------------------
// copy_entry_field -- clipboard injection via with_ports
// ---------------------------------------------------------------------------

#[test]
fn copy_entry_field_sends_username_to_clipboard_not_return() {
    let env = TestEnv::new();
    let (session, clipboard, root_uuid) =
        unlocked_session_with_clipboard(&env, "copy-user", "test-pass");

    let draft = EntryDraftDto {
        kind: EntryKindDto::Credential,
        title: "Copy Username".to_string(),
        username: Some("alice@example.com".to_string()),
        password: Some("pw".to_string()),
        url: None,
        notes: None,
        tags: Vec::new(),
        custom_fields: Vec::new(),
        totp_uri: None,
    };
    let uuid = session
        .create_entry(root_uuid, draft)
        .expect("create entry");

    let result = session.copy_entry_field(uuid, CopyField::Username);
    assert!(
        result.is_ok(),
        "copy_entry_field should return Ok(()), got: {result:?}"
    );

    assert_eq!(
        clipboard.last_copied().as_deref(),
        Some("alice@example.com"),
        "clipboard should contain the username"
    );
    assert!(
        clipboard.last_ttl().is_some_and(|d| !d.is_zero()),
        "auto-clear must be armed (non-zero TTL)"
    );
}

#[test]
fn copy_entry_field_sends_password_to_clipboard() {
    let env = TestEnv::new();
    let (session, clipboard, root_uuid) =
        unlocked_session_with_clipboard(&env, "copy-pw", "test-pass");

    let draft = EntryDraftDto {
        kind: EntryKindDto::Credential,
        title: "Copy Password".to_string(),
        username: Some("user".to_string()),
        password: Some("copy-me-password".to_string()),
        url: None,
        notes: None,
        tags: Vec::new(),
        custom_fields: Vec::new(),
        totp_uri: None,
    };
    let uuid = session
        .create_entry(root_uuid, draft)
        .expect("create entry");

    session
        .copy_entry_field(uuid, CopyField::Password)
        .expect("copy password");

    assert_eq!(
        clipboard.last_copied().as_deref(),
        Some("copy-me-password"),
        "clipboard should contain the password"
    );
    assert!(
        clipboard.last_ttl().is_some_and(|d| !d.is_zero()),
        "auto-clear must be armed (non-zero TTL)"
    );
}

#[test]
fn copy_entry_field_sends_custom_field_to_clipboard() {
    let env = TestEnv::new();
    let (session, clipboard, root_uuid) =
        unlocked_session_with_clipboard(&env, "copy-cf", "test-pass");

    let draft = EntryDraftDto {
        kind: EntryKindDto::Credential,
        title: "Copy Custom".to_string(),
        username: None,
        password: None,
        url: None,
        notes: None,
        tags: Vec::new(),
        custom_fields: vec![CustomFieldInputDto {
            name: "recovery-key".to_string(),
            value: "ABCD-EFGH-1234".to_string(),
            protected: true,
        }],
        totp_uri: None,
    };
    let uuid = session
        .create_entry(root_uuid, draft)
        .expect("create entry");

    session
        .copy_entry_field(uuid, CopyField::CustomField("recovery-key".to_string()))
        .expect("copy custom field");

    assert_eq!(
        clipboard.last_copied().as_deref(),
        Some("ABCD-EFGH-1234"),
        "clipboard should contain the custom field value"
    );
    assert!(
        clipboard.last_ttl().is_some_and(|d| !d.is_zero()),
        "auto-clear must be armed (non-zero TTL)"
    );
}

#[test]
fn copy_entry_field_sends_totp_code_to_clipboard_not_uri() {
    let env = TestEnv::new();
    let (session, clipboard, root_uuid) =
        unlocked_session_with_clipboard(&env, "copy-totp", "test-pass");

    let draft = EntryDraftDto {
        kind: EntryKindDto::Totp,
        title: "TOTP Entry".to_string(),
        username: Some("user@example.com".to_string()),
        password: None,
        url: None,
        notes: None,
        tags: Vec::new(),
        custom_fields: Vec::new(),
        totp_uri: Some(TOTP_URI.to_string()),
    };
    let uuid = session
        .create_entry(root_uuid, draft)
        .expect("create TOTP entry");

    // Populate the TOTP cache by fetching entry detail.
    let _detail = session
        .entry_detail(uuid.clone())
        .expect("entry_detail should populate TOTP cache");

    session
        .copy_entry_field(uuid, CopyField::TotpCode)
        .expect("copy TOTP code");

    let copied = clipboard
        .last_copied()
        .expect("clipboard should have a value");

    // The code must be a 6-digit numeric string, NOT the otpauth URI.
    assert!(
        copied.len() == 6 && copied.chars().all(|c| c.is_ascii_digit()),
        "expected a 6-digit numeric TOTP code, got: {copied}"
    );
    assert!(
        !copied.starts_with("otpauth://"),
        "clipboard must contain the computed TOTP code, not the otpauth URI"
    );
    assert!(
        clipboard.last_ttl().is_some_and(|d| !d.is_zero()),
        "auto-clear must be armed for TOTP copy (non-zero TTL)"
    );
}

// ---------------------------------------------------------------------------
// Error path — zeroize is structural (ZeroizeOnDrop on DTOs); verifying
// drop-time scrubbing from outside the crate is not possible without
// unsafe pointer inspection. This test confirms the error path returns
// the expected error and does not panic.
// ---------------------------------------------------------------------------

#[test]
fn reveal_on_locked_vault_returns_vault_locked() {
    let env = TestEnv::new();
    let name = "locked-vault";
    let password = "test-pass";
    let vault_path = create_test_vault(&env, name, password);
    register_vault(&env, name, &vault_path);

    let session = AppSession::for_test(env.paths_clone()).expect("create session");
    // Do NOT unlock -- vault stays locked.

    let fake_uuid = "00000000-0000-0000-0000-000000000001".to_string();

    let result = session.reveal_field(fake_uuid, RevealField::Password);
    assert!(
        matches!(result, Err(HidlinsApiError::VaultLocked)),
        "reveal_field on locked vault must return VaultLocked: {result:?}"
    );
}
