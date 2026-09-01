//! Rendered, in-process user journeys over the real `App` event/state/render
//! seams. These replace deterministic paths previously duplicated in manual
//! terminal documents; emulator-specific behavior remains an external smoke.

use std::time::Instant;

use crossterm::event::KeyCode;
use hidlins_core::{RegisteredVault, VaultRegistry};

use crate::app::{Focus, Phase, TreeMode};
use crate::overlay::Overlay;
use crate::test_support::{
    configured_app, key, key_alt, key_code, key_ctrl, mouse_down, onboarding_app, populated_app,
    render_at, type_text, unlock, MASTER_PASSWORD, SECRET_CANARY,
};
use crate::widgets::which_key::WHICH_KEY_DELAY;

#[test]
fn first_run_journey_authenticates_before_save_and_reloads_as_configured() {
    let (_dir, mut app, vault_path) = onboarding_app();
    let registry_path = app.paths.vaults_toml();
    let empty = render_at(&app, 60, 16, Instant::now());
    empty.assert_contains_in_order(&[
        "HIDLINS",
        "Please select a vault file:",
        "Focused field: Vault path",
        "Enter: Open vault",
        "Esc or Ctrl+Q: Exit",
    ]);

    type_text(&mut app, &vault_path.to_string_lossy());
    app.handle_event(&key_code(KeyCode::Enter));
    assert!(matches!(app.phase, Phase::UnlockPrompt { .. }));
    assert!(!registry_path.exists(), "path selection must not register");
    let password = render_at(&app, 60, 16, Instant::now());
    password.assert_contains_in_order(&[
        "HIDLINS",
        "Unlock vault: first-vault",
        "Focused field: Master password",
        "Password (masked):",
        "Enter: Unlock",
        "Esc: Back to vault path",
        "Ctrl+Q: Exit",
    ]);
    password.assert_excludes(&[
        vault_path.to_string_lossy().as_ref(),
        MASTER_PASSWORD,
        SECRET_CANARY,
    ]);
    password.assert_no_terminal_controls();

    type_text(&mut app, "wrong-password");
    app.handle_event(&key_code(KeyCode::Enter));
    let retry = render_at(&app, 60, 16, Instant::now());
    retry.assert_contains_all(&["Authentication failed (1/3)", "Password (masked):"]);
    retry.assert_excludes(&["wrong-password", MASTER_PASSWORD, SECRET_CANARY]);
    assert!(
        !registry_path.exists(),
        "failed authentication must not register"
    );

    type_text(&mut app, MASTER_PASSWORD);
    app.handle_event(&key_code(KeyCode::Enter));
    assert!(matches!(app.phase, Phase::Workspace));
    let loaded = VaultRegistry::load(app.paths.clone()).expect("reload saved registry");
    let registered = loaded.get("first-vault").expect("registered after auth");
    assert_eq!(registered.path, vault_path.canonicalize().unwrap());

    let reloaded = crate::app::App::from_registry(
        loaded,
        app.paths.clone(),
        hidlins_security::AutoLockConfig::default(),
    )
    .expect("restart from saved registry");
    assert!(matches!(reloaded.phase, Phase::UnlockPrompt { .. }));
}

#[test]
fn first_run_registration_conflict_is_a_reviewable_recoverable_frame() {
    let (dir, mut app, vault_path) = onboarding_app();
    type_text(&mut app, &vault_path.to_string_lossy());
    app.handle_event(&key_code(KeyCode::Enter));

    let mut external = VaultRegistry::load(app.paths.clone()).expect("empty registry");
    external
        .register_and_save(RegisteredVault {
            name: "first-vault".to_string(),
            path: dir.path().join("registered-elsewhere.kdbx"),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            keyfile_path: None,
            extra: toml::Table::new(),
        })
        .expect("concurrent registration");

    type_text(&mut app, MASTER_PASSWORD);
    app.handle_event(&key_code(KeyCode::Enter));
    assert!(matches!(app.phase, Phase::UnlockPrompt { .. }));
    let recovered = render_at(&app, 60, 16, Instant::now());
    recovered.assert_contains_in_order(&[
        "HIDLINS",
        "Unlock vault: first-vault",
        "Focused field: Master password",
        "Could not register vault:",
        "Enter: Unlock",
    ]);
    recovered.assert_excludes(&[MASTER_PASSWORD, SECRET_CANARY]);
    recovered.assert_no_terminal_controls();
    assert_eq!(app.registry().list().count(), 1);
}

#[test]
fn configured_single_and_multiple_vault_startup_journeys_are_complete() {
    let (_single_dir, mut single) = configured_app(&["personal"]);
    let direct = render_at(&single, 40, 12, Instant::now());
    direct.assert_contains_in_order(&[
        "HIDLINS",
        "Unlock vault: personal",
        "Enter: Unlock",
        "Esc: Exit",
        "Ctrl+Q: Exit",
    ]);
    unlock(&mut single);
    assert!(matches!(single.phase, Phase::Workspace));

    let (_multi_dir, mut multiple) = configured_app(&["alpha", "beta", "gamma"]);
    let initial = render_at(&multiple, 40, 12, Instant::now());
    initial.assert_contains_in_order(&[
        "HIDLINS",
        "Please select a vault:",
        "Selected: alpha",
        "Enter: Continue",
        "Ctrl+Q: Exit",
    ]);
    multiple.handle_event(&key('j'));
    multiple.handle_event(&key_code(KeyCode::Enter));
    let beta_prompt = render_at(&multiple, 40, 12, Instant::now());
    beta_prompt.assert_contains_in_order(&[
        "Unlock vault: beta",
        "Esc: Back to vault list",
        "Ctrl+Q: Exit",
    ]);
    multiple.handle_event(&key_code(KeyCode::Esc));
    let restored = render_at(&multiple, 40, 12, Instant::now());
    restored.assert_contains_all(&["Please select a vault:", "Selected: beta"]);
    multiple.handle_event(&key('j'));
    multiple.handle_event(&key_code(KeyCode::Enter));
    type_text(&mut multiple, MASTER_PASSWORD);
    multiple.handle_event(&key_code(KeyCode::Enter));
    assert!(matches!(multiple.phase, Phase::Workspace));
    assert_eq!(multiple.selected_vault.as_deref(), Some("gamma"));

    let (_exit_dir, mut exit) = configured_app(&["alpha", "beta"]);
    exit.handle_event(&key_ctrl('q'));
    assert!(exit.should_quit, "Ctrl+Q exits the picker");
}

#[test]
fn unlock_browse_search_and_lock_journey_renders_each_state() {
    let (_dir, mut app) = populated_app();

    type_text(&mut app, MASTER_PASSWORD);
    let prompt = render_at(&app, 80, 24, Instant::now());
    prompt.assert_contains_all(&["Unlock vault: personal", "Master password", "••••"]);
    prompt.assert_excludes(&[MASTER_PASSWORD, SECRET_CANARY]);

    app.handle_event(&key_code(KeyCode::Enter));
    assert!(matches!(app.phase, Phase::Workspace));
    app.handle_event(&key('j')); // Personal group -> loose Root1 entry.
    let workspace = render_at(&app, 80, 24, Instant::now());
    workspace.assert_contains_all(&["[1:Secrets]", "Root1", "Username", "alice", "Password"]);
    workspace.assert_excludes(&[MASTER_PASSWORD, SECRET_CANARY]);

    app.handle_event(&key('/'));
    type_text(&mut app, "Root1");
    let search = render_at(&app, 80, 24, Instant::now());
    search.assert_contains_all(&["Search", "[ALL]", "Results", "Root1"]);
    search.assert_excludes(&[MASTER_PASSWORD, SECRET_CANARY]);

    app.handle_event(&key_code(KeyCode::Esc));
    app.handle_event(&key_ctrl('l'));
    assert!(matches!(app.phase, Phase::LockScreen));
    let locked = render_at(&app, 80, 24, Instant::now());
    locked.assert_contains_all(&["Vault locked", "personal is locked", "Press any key"]);
    locked.assert_excludes(&[MASTER_PASSWORD, SECRET_CANARY]);
}

#[test]
fn discoverability_to_search_journey_is_reviewable_at_supported_floor() {
    let (_dir, mut app) = populated_app();
    unlock(&mut app);

    app.handle_event(&key('g'));
    let which_key = render_at(&app, 60, 16, Instant::now() + WHICH_KEY_DELAY);
    which_key.assert_contains_all(&["keys", "next tab", "previous tab"]);

    app.handle_event(&key_code(KeyCode::Esc));
    app.handle_event(&key('?'));
    assert!(matches!(app.overlay, Some(Overlay::Palette(_))));
    let palette = render_at(&app, 60, 16, Instant::now());
    palette.assert_contains_all(&["Commands", "type to filter", "quit", "lock now"]);

    type_text(&mut app, "search");
    let filtered = render_at(&app, 60, 16, Instant::now());
    filtered.assert_contains_all(&["Commands", "search"]);
    app.handle_event(&key_code(KeyCode::Enter));
    assert!(matches!(app.overlay, Some(Overlay::Search(_))));
    let search = render_at(&app, 60, 16, Instant::now());
    search.assert_contains_all(&["Search", "[ALL]", "Results"]);
    search.assert_excludes(&[MASTER_PASSWORD, SECRET_CANARY]);
}

#[test]
fn visual_readonly_and_mouse_journeys_keep_text_and_keyboard_parity() {
    let (_dir, mut app) = populated_app();
    unlock(&mut app);
    app.handle_event(&key('l')); // Expand Personal.
    app.handle_event(&key('v'));
    app.handle_event(&key('j'));
    assert!(matches!(app.tree_mode, TreeMode::Visual { .. }));
    assert!(!app.marks.is_empty());
    let visual = render_at(&app, 80, 24, Instant::now());
    visual.assert_contains_all(&["VISUAL", "selected", "▸"]);

    app.handle_event(&key_code(KeyCode::Esc));
    app.handle_event(&key_code(KeyCode::Esc));
    app.read_only = true;
    app.handle_event(&key('d'));
    let read_only = render_at(&app, 80, 24, Instant::now());
    read_only.assert_contains_all(&["RO", "Read-only"]);
    assert_eq!(
        app.save_count, 0,
        "read-only journey performs no vault write"
    );

    let (_dir, mut mouse_app) = populated_app();
    unlock(&mut mouse_app);
    let _ = render_at(&mouse_app, 80, 24, Instant::now());
    let before = mouse_app.tree.selected();
    mouse_app.handle_mouse_event(mouse_down(3, 4));
    assert_ne!(
        mouse_app.tree.selected(),
        before,
        "mouse accelerates row selection"
    );
    assert!(matches!(mouse_app.focus, Focus::Tree));
    let after_mouse = render_at(&mouse_app, 80, 24, Instant::now());
    after_mouse.assert_contains_all(&["Root1", "Username", "alice"]);

    let selected_by_mouse = mouse_app.tree.selected();
    mouse_app.mouse_enabled = false;
    mouse_app.handle_mouse_event(mouse_down(3, 3));
    assert_eq!(
        mouse_app.tree.selected(),
        selected_by_mouse,
        "--no-mouse posture leaves the keyboard state unchanged"
    );

    mouse_app.handle_event(&key_alt('2'));
    let settings = render_at(&mouse_app, 80, 24, Instant::now());
    settings.assert_contains_all(&[
        "[2:Settings]",
        "Default sort",
        "Theme",
        "Auto-lock",
        "Configure sync target",
    ]);
}
