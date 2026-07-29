mod common;

use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use hidlins_api::api::session::AppSession;
use hidlins_api::dto::{
    EntryDraftDto, EntryEditDto, EntryKindDto, GroupDeleteBehaviorDto, KeyfileRef, UiPrefs,
};
use hidlins_api::error::HidlinsApiError;

use common::{create_test_vault, register_vault, TestEnv};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn credential_draft(title: &str) -> EntryDraftDto {
    EntryDraftDto {
        kind: EntryKindDto::Credential,
        title: title.to_string(),
        username: Some("user".to_string()),
        password: Some("pass".to_string()),
        url: Some("https://example.com".to_string()),
        notes: None,
        tags: Vec::new(),
        custom_fields: Vec::new(),
        totp_uri: None,
    }
}

fn totp_draft(title: &str) -> EntryDraftDto {
    EntryDraftDto {
        kind: EntryKindDto::Totp,
        title: title.to_string(),
        username: None,
        password: None,
        url: None,
        notes: None,
        tags: Vec::new(),
        custom_fields: Vec::new(),
        totp_uri: Some(
            "otpauth://totp/test:user@example.com?secret=JBSWY3DPEHPK3PXP\
             &issuer=test&algorithm=SHA1&digits=6&period=30"
                .to_string(),
        ),
    }
}

fn empty_edit() -> EntryEditDto {
    EntryEditDto {
        title: None,
        username: None,
        password: None,
        url: None,
        notes: None,
        tags: None,
        custom_fields: None,
        totp_uri: None,
    }
}

/// Unlock a session against a vault that has already been created and
/// registered.  Returns the root group UUID for entry/group operations.
fn unlock_session(session: &AppSession, name: &str, password: &str) -> String {
    let tree = session
        .unlock(name.to_string(), password.to_string(), None)
        .expect("unlock should succeed");
    tree.root.uuid
}

// ---------------------------------------------------------------------------
// 1. create_vault_requires_no_recovery_confirmation
// ---------------------------------------------------------------------------

#[test]
fn create_vault_requires_no_recovery_confirmation() {
    let env = TestEnv::new();
    env.paths().ensure_exists().expect("ensure dir");
    let session = AppSession::for_test(env.paths_clone()).expect("session");

    let result = session.create_vault(
        "unconfirmed".to_string(),
        "test-password".to_string(),
        None,
        None,
        false,
    );

    assert!(
        matches!(
            result,
            Err(HidlinsApiError::InvalidInput {
                ref field,
                ..
            }) if field == "confirmed_no_recovery"
        ),
        "create_vault with confirmed_no_recovery=false must return InvalidInput, got: {result:?}"
    );

    // Verify no file was created on disk.
    let vault_path = env.paths().state_dir().join("unconfirmed.kdbx");
    assert!(
        !vault_path.exists(),
        "no vault file should be created when confirmation is refused"
    );
}

// ---------------------------------------------------------------------------
// 2. create_vault_with_keyfile_succeeds
// ---------------------------------------------------------------------------

#[test]
fn create_vault_with_keyfile_succeeds() {
    let env = TestEnv::new();
    env.paths().ensure_exists().expect("ensure dir");

    // Write a keyfile to disk for KeyfileRef::Path.
    let keyfile_path = env.paths().state_dir().join("my.key");
    std::fs::write(&keyfile_path, b"keyfile-material-32-bytes-xxxxx").expect("write keyfile");

    let session = AppSession::for_test(env.paths_clone()).expect("session");
    let summary = session
        .create_vault(
            "kf-vault".to_string(),
            "test-password".to_string(),
            None,
            Some(KeyfileRef::Path(
                keyfile_path.to_string_lossy().into_owned(),
            )),
            true,
        )
        .expect("create_vault with keyfile should succeed");

    assert_eq!(summary.name, "kf-vault", "vault name matches");
    assert!(summary.has_keyfile, "has_keyfile must be true");

    // The .kdbx must exist on disk.
    let vault_file = env.paths().state_dir().join("kf-vault.kdbx");
    assert!(vault_file.exists(), "vault file must exist on disk");
}

// ---------------------------------------------------------------------------
// 3. create_vault_duplicate_name_rejected
// ---------------------------------------------------------------------------

#[test]
fn create_vault_duplicate_name_rejected() {
    let env = TestEnv::new();
    env.paths().ensure_exists().expect("ensure dir");
    let session = AppSession::for_test(env.paths_clone()).expect("session");

    session
        .create_vault(
            "dup-vault".to_string(),
            "test-password".to_string(),
            None,
            None,
            true,
        )
        .expect("first create_vault should succeed");

    let result = session.create_vault(
        "dup-vault".to_string(),
        "test-password".to_string(),
        None,
        None,
        true,
    );

    assert!(
        matches!(result, Err(HidlinsApiError::PathExists { .. })),
        "duplicate vault name must return PathExists, got: {result:?}"
    );
}

// ---------------------------------------------------------------------------
// 4. list_vaults_returns_registered_entries
// ---------------------------------------------------------------------------

#[test]
fn list_vaults_returns_registered_entries() {
    let env = TestEnv::new();
    // Create and register both vaults before constructing the session,
    // because the session snapshot loads the registry once at construction.
    let path_a = create_test_vault(&env, "vault-a", "pass-a");
    let path_b = create_test_vault(&env, "vault-b", "pass-b");
    register_vault(&env, "vault-a", &path_a);
    register_vault(&env, "vault-b", &path_b);

    let session = AppSession::for_test(env.paths_clone()).expect("session");
    let vaults = session.list_vaults().expect("list_vaults should succeed");
    let names: Vec<&str> = vaults.iter().map(|v| v.name.as_str()).collect();

    assert!(
        names.contains(&"vault-a"),
        "vault-a must appear in list_vaults"
    );
    assert!(
        names.contains(&"vault-b"),
        "vault-b must appear in list_vaults"
    );
}

// ---------------------------------------------------------------------------
// 5. register_existing_vault_with_keyfile_metadata
// ---------------------------------------------------------------------------

#[test]
fn register_existing_vault_with_keyfile_metadata() {
    let env = TestEnv::new();
    env.paths().ensure_exists().expect("ensure dir");

    // Create a vault file directly (not via the session), so the registry
    // is free of this name and register_existing_vault can claim it.
    let vault_path = env.paths().state_dir().join("external.kdbx");
    let master = hidlins_core::MasterPassword::new("test-pass".to_string());
    let _vault = hidlins_core::Vault::create(
        &vault_path,
        &master,
        None,
        common::fast_kdf(),
        hidlins_core::NoRecoveryConfirmed::yes(),
    )
    .expect("create vault file");

    let keyfile_path = env.paths().state_dir().join("external.key");
    std::fs::write(&keyfile_path, b"keyfile-contents").expect("write keyfile");

    let session = AppSession::for_test(env.paths_clone()).expect("session");
    let summary = session
        .register_existing_vault(
            "external-vault".to_string(),
            vault_path.to_string_lossy().into_owned(),
            Some(KeyfileRef::Path(
                keyfile_path.to_string_lossy().into_owned(),
            )),
        )
        .expect("register_existing_vault should succeed");

    assert_eq!(summary.name, "external-vault", "registered name matches");
    assert!(
        summary.has_keyfile,
        "has_keyfile must be true after registering with a keyfile path"
    );
}

// ---------------------------------------------------------------------------
// 6. deregister_vault_removes_from_registry
// ---------------------------------------------------------------------------

#[test]
fn deregister_vault_removes_from_registry() {
    let env = TestEnv::new();
    env.paths().ensure_exists().expect("ensure dir");

    // Use session.create_vault so the in-memory registry is up to date.
    let session = AppSession::for_test(env.paths_clone()).expect("session");
    session
        .create_vault(
            "ephemeral".to_string(),
            "test-pass".to_string(),
            None,
            None,
            true,
        )
        .expect("create vault");

    let before = session.list_vaults().expect("list before deregister");
    assert!(
        before.iter().any(|v| v.name == "ephemeral"),
        "vault must be listed before deregister"
    );

    session
        .deregister_vault("ephemeral".to_string(), false)
        .expect("deregister should succeed");

    let after = session.list_vaults().expect("list after deregister");
    assert!(
        !after.iter().any(|v| v.name == "ephemeral"),
        "vault must be absent after deregister"
    );
}

// ---------------------------------------------------------------------------
// 7. entry_crud_creates_history_snapshots
// ---------------------------------------------------------------------------

#[test]
fn entry_crud_creates_history_snapshots() {
    let env = TestEnv::new();
    let name = "history-vault";
    let password = "test-pass";
    let vault_path = create_test_vault(&env, name, password);
    register_vault(&env, name, &vault_path);

    let session = AppSession::for_test(env.paths_clone()).expect("session");
    let root = unlock_session(&session, name, password);

    let uuid = session
        .create_entry(root.clone(), credential_draft("Original Title"))
        .expect("create_entry should succeed");

    // History should be empty right after creation.
    let history_before = session
        .entry_history(uuid.clone())
        .expect("entry_history should succeed");
    assert!(
        history_before.is_empty(),
        "history should be empty immediately after creation"
    );

    // Update the entry to generate a history snapshot.
    let mut edit = empty_edit();
    edit.title = Some("Updated Title".to_string());
    session
        .update_entry(uuid.clone(), edit)
        .expect("update_entry should succeed");

    let history_after = session
        .entry_history(uuid.clone())
        .expect("entry_history after update");
    assert!(
        !history_after.is_empty(),
        "history must have at least one entry after update"
    );
    assert_eq!(
        history_after[0].title, "Original Title",
        "history snapshot preserves the previous title"
    );
}

// ---------------------------------------------------------------------------
// 8. purge_entry_is_permanent
// ---------------------------------------------------------------------------

#[test]
fn purge_entry_is_permanent() {
    let env = TestEnv::new();
    let name = "purge-vault";
    let password = "test-pass";
    let vault_path = create_test_vault(&env, name, password);
    register_vault(&env, name, &vault_path);

    let session = AppSession::for_test(env.paths_clone()).expect("session");
    let root = unlock_session(&session, name, password);

    let uuid = session
        .create_entry(root.clone(), credential_draft("Doomed Entry"))
        .expect("create_entry should succeed");

    // Soft-delete moves to recycle bin.
    session
        .delete_entry(uuid.clone())
        .expect("delete_entry should succeed");

    // Purge permanently removes from the database.
    session
        .purge_entry(uuid.clone())
        .expect("purge_entry should succeed");

    // entry_detail must now fail — the entry no longer exists anywhere.
    let result = session.entry_detail(uuid.clone());
    assert!(
        result.is_err(),
        "entry_detail must fail after purge, got: {result:?}"
    );
}

// ---------------------------------------------------------------------------
// 9. group_crud_create_rename_move_delete
// ---------------------------------------------------------------------------

#[test]
fn group_crud_create_rename_move_delete() {
    let env = TestEnv::new();
    let name = "group-vault";
    let password = "test-pass";
    let vault_path = create_test_vault(&env, name, password);
    register_vault(&env, name, &vault_path);

    let session = AppSession::for_test(env.paths_clone()).expect("session");
    let root = unlock_session(&session, name, password);

    // Create a group under root.
    let group_uuid = session
        .create_group(root.clone(), "Work".to_string())
        .expect("create_group should succeed");

    let tree = session.vault_tree().expect("vault_tree after create");
    assert!(
        tree.root
            .children
            .iter()
            .any(|g| g.uuid == group_uuid && g.name == "Work"),
        "created group must appear in the tree"
    );

    // Rename the group.
    session
        .rename_group(group_uuid.clone(), "Office".to_string())
        .expect("rename_group should succeed");

    let tree = session.vault_tree().expect("vault_tree after rename");
    assert!(
        tree.root
            .children
            .iter()
            .any(|g| g.uuid == group_uuid && g.name == "Office"),
        "group must have the new name after rename"
    );

    // Create a second group and move the first under it.
    let parent_uuid = session
        .create_group(root.clone(), "Projects".to_string())
        .expect("create second group");

    session
        .move_group(group_uuid.clone(), parent_uuid.clone())
        .expect("move_group should succeed");

    let tree = session.vault_tree().expect("vault_tree after move");
    let projects = tree
        .root
        .children
        .iter()
        .find(|g| g.uuid == parent_uuid)
        .expect("Projects group must exist");
    assert!(
        projects
            .children
            .iter()
            .any(|g| g.uuid == group_uuid && g.name == "Office"),
        "Office must be a child of Projects after move"
    );

    // Delete the (now empty) Office group.
    session
        .delete_group(group_uuid.clone(), GroupDeleteBehaviorDto::Refuse)
        .expect("delete_group should succeed on an empty group");

    let tree = session.vault_tree().expect("vault_tree after delete");
    let projects = tree
        .root
        .children
        .iter()
        .find(|g| g.uuid == parent_uuid)
        .expect("Projects group must still exist");
    assert!(
        !projects.children.iter().any(|g| g.uuid == group_uuid),
        "deleted group must not appear in the tree"
    );
}

// ---------------------------------------------------------------------------
// 10. list_tags_returns_all_tags
// ---------------------------------------------------------------------------

#[test]
fn list_tags_returns_all_tags() {
    let env = TestEnv::new();
    let name = "tags-vault";
    let password = "test-pass";
    let vault_path = create_test_vault(&env, name, password);
    register_vault(&env, name, &vault_path);

    let session = AppSession::for_test(env.paths_clone()).expect("session");
    let root = unlock_session(&session, name, password);

    let mut draft_a = credential_draft("Entry A");
    draft_a.tags = vec!["work".to_string(), "finance".to_string()];
    session
        .create_entry(root.clone(), draft_a)
        .expect("create entry A");

    let mut draft_b = credential_draft("Entry B");
    draft_b.tags = vec!["personal".to_string(), "finance".to_string()];
    session
        .create_entry(root.clone(), draft_b)
        .expect("create entry B");

    let tags = session.list_tags().expect("list_tags should succeed");
    assert!(
        tags.contains(&"work".to_string()),
        "tags must contain 'work'"
    );
    assert!(
        tags.contains(&"finance".to_string()),
        "tags must contain 'finance'"
    );
    assert!(
        tags.contains(&"personal".to_string()),
        "tags must contain 'personal'"
    );
}

// ---------------------------------------------------------------------------
// 11. set_and_clear_expiration
// ---------------------------------------------------------------------------

#[test]
fn set_and_clear_expiration() {
    let env = TestEnv::new();
    let name = "expiry-vault";
    let password = "test-pass";
    let vault_path = create_test_vault(&env, name, password);
    register_vault(&env, name, &vault_path);

    let session = AppSession::for_test(env.paths_clone()).expect("session");
    let root = unlock_session(&session, name, password);

    let uuid = session
        .create_entry(root.clone(), credential_draft("Expiring Entry"))
        .expect("create_entry should succeed");

    // Set expiration to a known epoch (2030-01-01T00:00:00Z = 1893456000).
    let epoch: i64 = 1_893_456_000;
    session
        .set_expiration(uuid.clone(), epoch)
        .expect("set_expiration should succeed");

    let detail = session
        .entry_detail(uuid.clone())
        .expect("entry_detail after set_expiration");
    assert_eq!(
        detail.expiry_time,
        Some(epoch),
        "expiry_time must match the epoch set"
    );

    // Clear expiration.
    session
        .clear_expiration(uuid.clone())
        .expect("clear_expiration should succeed");

    let detail = session
        .entry_detail(uuid.clone())
        .expect("entry_detail after clear_expiration");
    assert_eq!(
        detail.expiry_time, None,
        "expiry_time must be None after clearing"
    );
}

// ---------------------------------------------------------------------------
// 12. totp_cache_populated_on_detail_invalidated_on_update
// ---------------------------------------------------------------------------

#[test]
fn totp_cache_populated_on_detail_invalidated_on_update() {
    let env = TestEnv::new();
    let name = "totp-vault";
    let password = "test-pass";
    let vault_path = create_test_vault(&env, name, password);
    register_vault(&env, name, &vault_path);

    let session = AppSession::for_test(env.paths_clone()).expect("session");
    let root = unlock_session(&session, name, password);

    let uuid = session
        .create_entry(root.clone(), totp_draft("My TOTP"))
        .expect("create TOTP entry");

    // Before calling entry_detail, the TOTP cache is empty — totp_now must
    // fail with a cache miss.
    let miss = session.totp_now(uuid.clone());
    assert!(
        matches!(miss, Err(HidlinsApiError::Internal { .. })),
        "totp_now before entry_detail must report cache miss, got: {miss:?}"
    );

    // entry_detail populates the TOTP cache.
    let detail = session
        .entry_detail(uuid.clone())
        .expect("entry_detail should succeed");
    assert_eq!(detail.kind, EntryKindDto::Totp, "entry kind must be TOTP");

    // totp_now must now succeed.
    let code = session
        .totp_now(uuid.clone())
        .expect("totp_now should succeed after entry_detail");
    assert_eq!(code.period, 30, "TOTP period must be 30");
    assert!(!code.code.is_empty(), "TOTP code must not be empty");

    // Update the entry (any field) — this must invalidate the cache.
    let mut edit = empty_edit();
    edit.title = Some("Updated TOTP".to_string());
    session
        .update_entry(uuid.clone(), edit)
        .expect("update_entry should succeed");

    let result = session.totp_now(uuid.clone());
    assert!(
        matches!(result, Err(HidlinsApiError::Internal { .. })),
        "totp_now must fail after update_entry (cache invalidated), got: {result:?}"
    );
}

// ---------------------------------------------------------------------------
// 13. totp_now_does_not_take_session_mutex
// ---------------------------------------------------------------------------

#[test]
fn totp_now_does_not_take_session_mutex() {
    let env = TestEnv::new();
    let name = "totp-mutex-vault";
    let password = "test-pass";
    let vault_path = create_test_vault(&env, name, password);
    register_vault(&env, name, &vault_path);

    let session = AppSession::for_test(env.paths_clone()).expect("session");
    let root = unlock_session(&session, name, password);

    let uuid = session
        .create_entry(root.clone(), totp_draft("Mutex TOTP"))
        .expect("create TOTP entry");

    // Populate the TOTP cache (requires the session mutex).
    session
        .entry_detail(uuid.clone())
        .expect("entry_detail populates TOTP cache");

    let session = Arc::new(session);
    let session2 = Arc::clone(&session);

    // Hold the session mutex on the main thread.
    let _guard = session.hold_mutex_for_test();

    let uuid_clone = uuid.clone();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        // totp_now only touches the RwLock, not the session mutex.
        let result = session2.totp_now(uuid_clone);
        tx.send(result).expect("send totp_now result");
    });

    let result = rx
        .recv_timeout(Duration::from_millis(500))
        .expect("totp_now must not block when the session mutex is held");
    assert!(
        result.is_ok(),
        "totp_now should succeed while mutex is held, got: {result:?}"
    );
}

// ---------------------------------------------------------------------------
// 14. totp_cache_poison_recovers_by_clearing
// ---------------------------------------------------------------------------

#[test]
fn totp_cache_poison_recovers_by_clearing() {
    let env = TestEnv::new();
    let name = "totp-poison-vault";
    let password = "test-pass";
    let vault_path = create_test_vault(&env, name, password);
    register_vault(&env, name, &vault_path);

    let session = AppSession::for_test(env.paths_clone()).expect("session");
    let root = unlock_session(&session, name, password);

    let uuid = session
        .create_entry(root.clone(), totp_draft("Poison TOTP"))
        .expect("create TOTP entry");

    // Populate the cache.
    session
        .entry_detail(uuid.clone())
        .expect("entry_detail populates cache");

    // Poison the RwLock via the test helper.
    session.poison_totp_cache_for_test();

    // totp_now must recover gracefully: clear the poisoned cache and
    // report a cache miss — it must NOT panic.
    let result = session.totp_now(uuid.clone());
    assert!(
        matches!(result, Err(HidlinsApiError::Internal { .. })),
        "poisoned totp_now must return Internal error (cache miss), got: {result:?}"
    );

    // RwLock poisoning is permanent — once poisoned, every subsequent
    // read() returns Err(PoisonError). The recovery contract is: no
    // panic, graceful error, and the session remains usable for other
    // operations. Calling totp_now again must still return an error
    // (not panic).
    let result2 = session.totp_now(uuid.clone());
    assert!(
        matches!(result2, Err(HidlinsApiError::Internal { .. })),
        "repeated totp_now on poisoned cache must still return Internal, got: {result2:?}"
    );

    // entry_detail must still succeed (it writes through the poison).
    session
        .entry_detail(uuid.clone())
        .expect("entry_detail must still work after cache poison");
}

// ---------------------------------------------------------------------------
// 15. prefs_get_set_round_trip_through_session
// ---------------------------------------------------------------------------

#[test]
fn prefs_get_set_round_trip_through_session() {
    let env = TestEnv::new();
    env.paths().ensure_exists().expect("ensure dir");
    let session = AppSession::for_test(env.paths_clone()).expect("session");

    let prefs = UiPrefs {
        theme_mode: Some("dark".to_string()),
        window_x: Some(100),
        window_y: Some(200),
        window_width: Some(1024),
        window_height: Some(768),
        last_vault: Some("my-vault".to_string()),
        list_pane_width: Some(384.5),
    };

    session
        .set_prefs(prefs.clone())
        .expect("set_prefs should succeed");

    let got = session.get_prefs().expect("get_prefs should succeed");

    assert_eq!(
        got.theme_mode.as_deref(),
        Some("dark"),
        "theme_mode must round-trip"
    );
    assert_eq!(got.window_x, Some(100), "window_x must round-trip");
    assert_eq!(got.window_y, Some(200), "window_y must round-trip");
    assert_eq!(got.window_width, Some(1024), "window_width must round-trip");
    assert_eq!(
        got.window_height,
        Some(768),
        "window_height must round-trip"
    );
    assert_eq!(
        got.last_vault.as_deref(),
        Some("my-vault"),
        "last_vault must round-trip"
    );
    assert_eq!(
        got.list_pane_width,
        Some(384.5),
        "list_pane_width must round-trip"
    );
}

// ---------------------------------------------------------------------------
// 16. corrupt_prefs_file_yields_defaults_not_error
// ---------------------------------------------------------------------------

#[test]
fn corrupt_prefs_file_yields_defaults_not_error() {
    let env = TestEnv::new();
    env.paths().ensure_exists().expect("ensure dir");

    // Write corrupt data to the prefs file.
    let prefs_path = env.paths().state_dir().join("ui-prefs.toml");
    std::fs::write(&prefs_path, b"\x00\xff GARBAGE not valid TOML {{{")
        .expect("write corrupt prefs");

    let session = AppSession::for_test(env.paths_clone()).expect("session");
    let got = session
        .get_prefs()
        .expect("get_prefs must succeed even with corrupt file");

    // All fields must be None (defaults).
    assert!(
        got.theme_mode.is_none(),
        "corrupt prefs: theme_mode must default to None"
    );
    assert!(
        got.window_x.is_none(),
        "corrupt prefs: window_x must default to None"
    );
    assert!(
        got.window_y.is_none(),
        "corrupt prefs: window_y must default to None"
    );
    assert!(
        got.window_width.is_none(),
        "corrupt prefs: window_width must default to None"
    );
    assert!(
        got.window_height.is_none(),
        "corrupt prefs: window_height must default to None"
    );
    assert!(
        got.last_vault.is_none(),
        "corrupt prefs: last_vault must default to None"
    );
    assert!(
        got.list_pane_width.is_none(),
        "corrupt prefs: list_pane_width must default to None"
    );
}
