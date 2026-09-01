//! Rendered, in-process user journeys over the real `App` event/state/render
//! seams. These replace deterministic paths previously duplicated in manual
//! terminal documents; emulator-specific behavior remains an external smoke.

use std::time::Instant;

use crossterm::event::KeyCode;

use crate::app::{Focus, Phase, TreeMode};
use crate::overlay::Overlay;
use crate::test_support::{
    key, key_alt, key_code, key_ctrl, mouse_down, populated_app, render_at, type_text, unlock,
    MASTER_PASSWORD, SECRET_CANARY,
};
use crate::widgets::which_key::WHICH_KEY_DELAY;

#[test]
fn unlock_browse_search_and_lock_journey_renders_each_state() {
    let (_dir, mut app) = populated_app();

    let unlock_list = render_at(&app, 80, 24, Instant::now());
    unlock_list.assert_contains_all(&["Hidlins — select a vault", "Vaults", "personal"]);

    app.handle_event(&key_code(KeyCode::Enter));
    type_text(&mut app, MASTER_PASSWORD);
    let prompt = render_at(&app, 80, 24, Instant::now());
    prompt.assert_contains_all(&["Unlock personal", "Master password", "••••"]);
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
