//! Semantic accessibility contracts over actual Ratatui buffers.
//!
//! These intentionally do not claim AT-SPI or speech-engine coverage. They pin
//! the text Hidlins emits for a terminal/screen reader to consume.

use std::time::Instant;

use crossterm::event::KeyCode;

use crate::test_support::{
    key, key_alt, key_code, key_ctrl, populated_app, render_at, type_text, unlock, MASTER_PASSWORD,
    SECRET_CANARY,
};
use crate::theme::Theme;

#[test]
fn password_prompts_and_entry_details_never_emit_secret_canaries() {
    let (_dir, mut app) = populated_app();
    app.handle_event(&key_code(KeyCode::Enter));
    type_text(&mut app, MASTER_PASSWORD);
    let prompt = render_at(&app, 80, 24, Instant::now());
    prompt.assert_contains_all(&["Unlock personal", "Master password", "••••"]);
    prompt.assert_excludes(&[MASTER_PASSWORD, SECRET_CANARY]);

    app.handle_event(&key_code(KeyCode::Enter));
    app.handle_event(&key('j'));
    let detail = render_at(&app, 80, 24, Instant::now());
    detail.assert_contains_all(&["Root1", "Username", "Password", "••••"]);
    detail.assert_excludes(&[MASTER_PASSWORD, SECRET_CANARY]);

    app.handle_event(&key('/'));
    type_text(&mut app, "Root1");
    let search = render_at(&app, 100, 24, Instant::now());
    search.assert_contains_all(&["Search", "[ALL]", "Preview", "Root1"]);
    search.assert_excludes(&[MASTER_PASSWORD, SECRET_CANARY]);
}

#[test]
fn semantic_transcript_is_equivalent_across_color_and_accessible_themes() {
    let (_dir, mut app) = populated_app();
    unlock(&mut app);
    app.handle_event(&key('j'));
    app.read_only = true;

    app.theme = Theme::from_env_parts(None, false, Some("xterm-256color"), Some("truecolor"));
    let color = render_at(&app, 80, 24, Instant::now());
    app.theme = Theme::accessible();
    let accessible = render_at(&app, 80, 24, Instant::now());

    assert_eq!(
        color.transcript, accessible.transcript,
        "required semantics must not change when color is removed"
    );
    accessible.assert_contains_all(&["[1:Secrets]", "Root1", "RO", "Username", "Password"]);
}

#[test]
fn important_states_have_textual_carriers_in_review_order() {
    let (_dir, mut app) = populated_app();
    unlock(&mut app);
    app.read_only = true;
    app.handle_event(&key('l'));
    app.handle_event(&key('v'));
    app.handle_event(&key('j'));
    let visual = render_at(&app, 80, 24, Instant::now());
    visual.assert_contains_in_order(&["[1:Secrets]", "VISUAL", "selected", "RO"]);
    visual.assert_contains_all(&["(expired)", "▸"]);

    app.handle_event(&key_code(KeyCode::Esc));
    app.handle_event(&key_code(KeyCode::Esc));
    app.handle_event(&key('/'));
    let search = render_at(&app, 60, 16, Instant::now());
    search.assert_contains_in_order(&["Search", "[ALL]", "Results"]);

    app.handle_event(&key_code(KeyCode::Esc));
    app.handle_event(&key_alt('2'));
    let settings = render_at(&app, 80, 24, Instant::now());
    settings.assert_contains_in_order(&[
        "[2:Settings]",
        "Settings",
        "Default sort",
        "Theme",
        "Auto-lock",
        "Status",
    ]);

    app.handle_event(&key_alt('1'));
    app.handle_event(&key_ctrl('l'));
    let locked = render_at(&app, 40, 12, Instant::now());
    locked.assert_contains_in_order(&["Vault locked", "personal is locked", "Press any key"]);
}

#[test]
fn reviewable_buffers_never_contain_raw_terminal_controls() {
    let (_dir, mut app) = populated_app();
    let mut frames = vec![render_at(&app, 80, 24, Instant::now())];

    app.handle_event(&key_code(KeyCode::Enter));
    type_text(&mut app, MASTER_PASSWORD);
    frames.push(render_at(&app, 80, 24, Instant::now()));
    app.handle_event(&key_code(KeyCode::Enter));
    frames.push(render_at(&app, 80, 24, Instant::now()));
    app.handle_event(&key('?'));
    frames.push(render_at(&app, 60, 16, Instant::now()));
    app.handle_event(&key_code(KeyCode::Esc));
    app.handle_event(&key('/'));
    frames.push(render_at(&app, 60, 16, Instant::now()));
    app.handle_event(&key_code(KeyCode::Esc));
    app.handle_event(&key_ctrl('l'));
    frames.push(render_at(&app, 40, 12, Instant::now()));

    for frame in frames {
        frame.assert_no_terminal_controls();
        frame.assert_excludes(&[MASTER_PASSWORD, SECRET_CANARY]);
    }
}
