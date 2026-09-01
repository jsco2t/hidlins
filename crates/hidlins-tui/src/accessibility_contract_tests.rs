//! Semantic accessibility contracts over actual Ratatui buffers.
//!
//! These intentionally do not claim AT-SPI or speech-engine coverage. They pin
//! the text Hidlins emits for a terminal/screen reader to consume.

use std::time::Instant;

use crossterm::event::KeyCode;

use crate::screens::startup_modal::HIDLINS_STARTUP_ART;
use crate::test_support::{
    configured_app, key, key_alt, key_code, key_ctrl, onboarding_app, populated_app, render_at,
    type_text, unlock, MASTER_PASSWORD, SECRET_CANARY,
};
use crate::theme::Theme;

#[test]
fn compact_accessible_startup_keeps_the_complete_semantic_password_form() {
    let (_dir, mut app) = populated_app();
    app.theme = Theme::accessible();

    let prompt = render_at(&app, 40, 12, Instant::now());
    prompt.assert_contains_in_order(&[
        "HIDLINS",
        "Unlock vault: personal",
        "Focused field: Master password",
        "Password (masked):",
        "Enter: Unlock",
        "Ctrl+Q: Exit",
    ]);
    prompt.assert_excludes(&[".--------.", MASTER_PASSWORD, SECRET_CANARY]);
    prompt.assert_no_terminal_controls();
}

#[test]
fn normal_supported_startup_sizes_render_every_exact_art_row_in_order() {
    let (_dir, app) = configured_app(&["personal"]);
    let expected: Vec<_> = HIDLINS_STARTUP_ART.lines().collect();
    assert_eq!(expected.len(), 13);
    for (width, height) in [(80, 24), (60, 16)] {
        let frame = render_at(&app, width, height, Instant::now());
        frame.assert_contains_in_order(&expected);
        frame.assert_contains_all(&[
            "HIDLINS",
            "Unlock vault: personal",
            "Enter: Unlock",
            "Ctrl+Q: Exit",
        ]);
    }
}

#[test]
fn accessible_and_compact_startup_forms_preserve_semantics_without_art() {
    let (_empty_dir, mut empty, _) = onboarding_app();
    empty.theme = Theme::accessible();
    let onboarding = render_at(&empty, 80, 24, Instant::now());
    onboarding.assert_contains_in_order(&[
        "HIDLINS",
        "Please select a vault file:",
        "Focused field: Vault path",
        "Enter: Open vault",
        "Esc or Ctrl+Q: Exit",
    ]);
    onboarding.assert_excludes(&[".--------."]);

    type_text(&mut empty, "/definitely/missing/path-canary.kdbx");
    empty.handle_event(&key_code(KeyCode::Enter));
    let invalid = render_at(&empty, 40, 12, Instant::now());
    invalid.assert_contains_in_order(&[
        "HIDLINS",
        "Please select a vault file:",
        "Focused field: Vault path",
        "Vault file does not exist or cannot be",
        "accessed.",
        "Enter: Open vault",
        "Esc or Ctrl+Q: Exit",
    ]);
    invalid.assert_excludes(&[".--------."]);

    let (_multi_dir, mut multiple) = configured_app(&["alpha", "beta", "gamma"]);
    multiple.theme = Theme::accessible();
    multiple.handle_event(&key('j'));
    let picker = render_at(&multiple, 40, 12, Instant::now());
    picker.assert_contains_in_order(&[
        "HIDLINS",
        "Please select a vault:",
        "Selected: beta",
        "Enter: Continue",
        "Ctrl+Q: Exit",
    ]);
    picker.assert_excludes(&[".--------."]);
    multiple.handle_event(&key_code(KeyCode::Enter));
    let prompt = render_at(&multiple, 40, 12, Instant::now());
    prompt.assert_contains_in_order(&[
        "HIDLINS",
        "Unlock vault: beta",
        "Focused field: Master password",
        "Password (masked):",
        "Enter: Unlock",
        "Esc: Back to vault list",
        "Ctrl+Q: Exit",
    ]);
    prompt.assert_excludes(&[".--------."]);
}

#[test]
fn long_vault_lists_scroll_the_textual_selection_into_view() {
    let names: Vec<_> = (0..12).map(|index| format!("vault-{index:02}")).collect();
    let refs: Vec<_> = names.iter().map(String::as_str).collect();
    let (_dir, mut app) = configured_app(&refs);
    app.theme = Theme::accessible();
    for _ in 0..11 {
        app.handle_event(&key('j'));
    }
    let frame = render_at(&app, 40, 12, Instant::now());
    frame.assert_contains_all(&["Please select a vault:", "Selected: vault-11"]);
    frame.assert_no_terminal_controls();
}

#[test]
fn every_startup_transition_frame_is_control_free_and_password_safe() {
    let (_onboarding_dir, mut onboarding, vault_path) = onboarding_app();
    let mut frames = vec![render_at(&onboarding, 60, 16, Instant::now())];
    type_text(&mut onboarding, "/missing/control-contract.kdbx");
    onboarding.handle_event(&key_code(KeyCode::Enter));
    frames.push(render_at(&onboarding, 40, 12, Instant::now()));
    for _ in 0.."/missing/control-contract.kdbx".chars().count() {
        onboarding.handle_event(&key_code(KeyCode::Backspace));
    }
    type_text(&mut onboarding, &vault_path.to_string_lossy());
    onboarding.handle_event(&key_code(KeyCode::Enter));
    type_text(&mut onboarding, MASTER_PASSWORD);
    frames.push(render_at(&onboarding, 60, 16, Instant::now()));

    let (_multiple_dir, mut multiple) = configured_app(&["alpha", "beta"]);
    frames.push(render_at(&multiple, 60, 16, Instant::now()));
    multiple.handle_event(&key('j'));
    multiple.handle_event(&key_code(KeyCode::Enter));
    type_text(&mut multiple, MASTER_PASSWORD);
    frames.push(render_at(&multiple, 40, 12, Instant::now()));
    multiple.handle_event(&key_code(KeyCode::Esc));
    frames.push(render_at(&multiple, 40, 12, Instant::now()));

    for frame in frames {
        frame.assert_no_terminal_controls();
        frame.assert_excludes(&[MASTER_PASSWORD, SECRET_CANARY]);
    }
}

#[test]
fn password_prompts_and_entry_details_never_emit_secret_canaries() {
    let (_dir, mut app) = populated_app();
    type_text(&mut app, MASTER_PASSWORD);
    let prompt = render_at(&app, 80, 24, Instant::now());
    prompt.assert_contains_all(&["Unlock vault: personal", "Master password", "••••"]);
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
