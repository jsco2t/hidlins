use super::*;
use crate::overlay::edit::{Col, EditField};
use crate::test_support;
use std::fmt::Write as _;

fn editor_app(notes: &str, custom_count: usize) -> (TempDir, App, Uuid) {
    let (directory, mut app) = populated_app();
    unlock(&mut app, PASSWORD);
    let vault = app.vault_mut().unwrap();
    let root = vault.root_group_uuid();
    let mut draft = EntryBuilder::credential("Viewport entry")
        .password("EDITOR-PASSWORD-CANARY")
        .notes(notes);
    for index in 0..custom_count {
        draft = draft.custom_field(
            format!("custom-{index:02}"),
            format!("protected-{index:02}"),
            true,
        );
    }
    let uuid = vault.add_entry(root, draft.build()).unwrap();
    app.jump_to_entry(uuid);
    app.handle_event(&key('e'));
    assert!(matches!(app.overlay, Some(Overlay::Edit(_))));
    (directory, app, uuid)
}

fn edit_state(app: &App) -> &EditState {
    let Some(Overlay::Edit(state)) = app.overlay.as_ref() else {
        panic!("edit open")
    };
    state
}

fn focus(app: &mut App, target: EditField) {
    for _ in 0..100 {
        if edit_state(app).focused() == target {
            return;
        }
        app.handle_event(&key_code(KeyCode::Tab));
    }
    panic!("fixture must reach the requested field");
}

fn many_notes() -> String {
    let mut notes = String::new();
    for index in 0..80 {
        writeln!(&mut notes, "line-{index:02}").unwrap();
    }
    notes.push_str("TARGET-NOTE");
    notes
}

#[test]
fn tab_and_backtab_keep_late_fields_visible_without_exposing_protected_values() {
    let (_directory, mut app, _) = editor_app(&many_notes(), 20);
    focus(&mut app, EditField::Custom(19, Col::Name));
    let name = edit_state(&app).custom[19].name.value().to_string();
    let secret = edit_state(&app).custom[19].value.as_str().to_string();
    let frame = test_support::render_at(&app, 80, 24, Instant::now());
    frame.assert_contains_all(&[&name, "> name:"]);
    assert!(!frame.transcript.contains(&secret));
    app.handle_event(&key_code(KeyCode::Tab));
    let frame = test_support::render_at(&app, 80, 24, Instant::now());
    frame.assert_contains_all(&["> value:"]);
    assert!(!frame.transcript.contains(&secret));
    app.handle_event(&key_ctrl('r'));
    test_support::render_at(&app, 80, 24, Instant::now()).assert_contains_all(&[&secret]);
    app.handle_event(&key_code(KeyCode::Tab));
    test_support::render_at(&app, 80, 24, Instant::now())
        .assert_contains_all(&["> [+ add custom field]"]);
    app.handle_event(&key_code(KeyCode::BackTab));
    assert_eq!(
        edit_state(&app).focused(),
        EditField::Custom(19, Col::Value)
    );
    test_support::render_at(&app, 80, 24, Instant::now()).assert_contains_all(&["> value:"]);
}

#[test]
fn notes_cursor_follows_lines_and_resize_without_mutating_the_buffer() {
    let notes = many_notes();
    let (_directory, mut app, _) = editor_app(&notes, 2);
    focus(&mut app, EditField::Notes);
    for (width, height) in [(80, 24), (40, 14), (100, 30)] {
        test_support::render_at(&app, width, height, Instant::now())
            .assert_contains_all(&["TARGET-NOTE▏"]);
    }
    app.handle_event(&key_code(KeyCode::Up));
    test_support::render_at(&app, 40, 14, Instant::now()).assert_contains_all(&["line-79▏"]);
    app.handle_event(&key_code(KeyCode::Down));
    test_support::render_at(&app, 40, 14, Instant::now()).assert_contains_all(&["TARGET-▏NOTE"]);
    assert_eq!(edit_state(&app).notes.value().as_str(), notes);
}

#[test]
fn long_wrapped_single_line_caret_remains_visible_including_unicode_and_literal_carets() {
    for text in [
        "word ".repeat(100),
        "x".repeat(800),
        "漢字 é\u{301} 🔒 user▏ ".repeat(80),
    ] {
        let (_directory, mut app, _) = editor_app(&text, 0);
        focus(&mut app, EditField::Notes);
        app.handle_event(&key('§'));
        for width in [40, 72, 90] {
            let content = editor_content(&app, width, 16);
            assert!(content.contains("§▏"),"actual end caret is visible across row boundaries, independently of earlier user caret glyphs");
        }
        for _ in 0..25 {
            app.handle_event(&key_code(KeyCode::Left));
        }
        app.handle_event(&key('¶'));
        let content = editor_content(&app, 44, 16);
        assert!(
            content.contains("¶▏"),
            "actual middle caret is visible through full-word wrapping"
        );
    }
}

#[test]
fn failed_save_keeps_error_and_focused_field_visible_and_vault_unchanged() {
    let (_directory, mut app, uuid) = editor_app(&many_notes(), 20);
    let Some(Overlay::Edit(state)) = app.overlay.as_mut() else {
        unreachable!()
    };
    state.title = tui_input::Input::new(String::new());
    focus(&mut app, EditField::CustomAdd);
    let before = app.save_count;
    app.handle_event(&key_ctrl('s'));
    let error = edit_state(&app)
        .error
        .as_deref()
        .expect("empty title rejected");
    assert!(error.contains("Title") || error.contains("title"));
    let frame = test_support::render_at(&app, 80, 24, Instant::now());
    frame.assert_contains_all(&[error, "[+ add custom field]"]);
    assert_eq!(app.save_count, before);
    assert_eq!(
        app.vault().unwrap().get_entry(uuid).unwrap().title(),
        "Viewport entry"
    );
}

#[test]
fn short_forms_keep_fields_caret_masking_cancel_and_navigation() {
    let (_directory, mut app, _) = editor_app("short", 0);
    test_support::render_at(&app, 80, 24, Instant::now())
        .assert_contains_all(&["Title:", "Viewport entry▏"]);
    focus(&mut app, EditField::Notes);
    test_support::render_at(&app, 80, 24, Instant::now()).assert_contains_all(&["short▏"]);
    app.handle_event(&key_code(KeyCode::Esc));
    assert!(app.overlay.is_none());
    let frame = test_support::render_at(&app, 80, 24, Instant::now());
    assert!(!frame.transcript.contains("EDITOR-PASSWORD-CANARY"));
}

// Read only the actual modal interior. Row joins tolerate legitimate wrapping
// between the typed marker and caret; no underlying vault detail can satisfy it.
fn editor_content(app: &App, width: u16, height: u16) -> String {
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
    let mut area = ratatui::layout::Rect::default();
    terminal
        .draw(|frame| {
            area = crate::overlay::centered(frame, 72, 22);
            app.render(frame, Instant::now());
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    for y in area.y.saturating_add(1)..area.bottom().saturating_sub(1) {
        let mut row = String::new();
        for x in area.x.saturating_add(1)..area.right().saturating_sub(1) {
            row.push_str(buffer[(x, y)].symbol());
        }
        text.push_str(row.trim());
    }
    text
}

#[test]
fn new_custom_row_and_removal_follow_the_live_focus() {
    let (_directory, mut app, _) = editor_app(&many_notes(), 20);
    focus(&mut app, EditField::CustomAdd);
    app.handle_event(&key_code(KeyCode::Enter));
    assert_eq!(edit_state(&app).focused(), EditField::Custom(20, Col::Name));
    test_support::render_at(&app, 72, 18, Instant::now()).assert_contains_all(&["> name:"]);
    app.handle_event(&key_ctrl('d'));
    assert_eq!(edit_state(&app).custom.len(), 20);
    assert_eq!(edit_state(&app).focused(), EditField::CustomAdd);
    test_support::render_at(&app, 72, 18, Instant::now())
        .assert_contains_all(&["> [+ add custom field]"]);
}

#[test]
fn masked_custom_cursor_motion_keeps_fixed_geometry_and_secret_hidden() {
    let (_directory, mut app, _) = editor_app(&many_notes(), 20);
    let Some(Overlay::Edit(state)) = app.overlay.as_mut() else {
        unreachable!()
    };
    state.custom[19].value = PasswordInput::with_value(&"SECRET-CURSOR-".repeat(100));
    focus(&mut app, EditField::Custom(19, Col::Value));
    let before = editor_content(&app, 60, 16);
    assert!(before.contains("> value:"));
    for _ in 0..150 {
        app.handle_event(&key_code(KeyCode::Left));
    }
    let after = editor_content(&app, 60, 16);
    assert_eq!(
        before, after,
        "fixed protected mask and viewport cannot disclose hidden cursor/length"
    );
    assert!(!after.contains("SECRET-CURSOR-"));
}

#[test]
fn add_totp_and_note_kinds_keep_their_actual_caret_visible() {
    for (kind, target) in [
        (EntryKind::Totp, EditField::TotpUri),
        (EntryKind::SecureNote, EditField::Notes),
    ] {
        let (_directory, mut app) = populated_app();
        unlock(&mut app, PASSWORD);
        app.handle_event(&key('a'));
        let Some(Overlay::Edit(state)) = app.overlay.as_mut() else {
            panic!("add editor")
        };
        // Fixture values use the same constructors as a loaded/typed form;
        // navigation below goes through actual Tab and character events.
        state.kind = kind;
        state.totp_uri = PasswordInput::with_value(&format!(
            "otpauth://totp/{}?secret=JBSWY3DPEHPK3PXP",
            "label".repeat(100)
        ));
        state.notes = crate::widgets::text_area::TextArea::from_str(&many_notes());
        focus(&mut app, target);
        app.handle_event(&key('§'));
        assert!(editor_content(&app, 44, 16).contains("§▏"));
    }
}

#[test]
fn tiny_editor_frames_remain_safe_with_focus_and_validation_error() {
    let (_directory, mut app, _) = editor_app(&many_notes(), 20);
    focus(&mut app, EditField::CustomAdd);
    let Some(Overlay::Edit(state)) = app.overlay.as_mut() else {
        unreachable!()
    };
    state.error = Some("Title is required".to_string());
    for (width, height) in [(0, 0), (1, 1), (8, 4), (40, 12)] {
        test_support::render_at(&app, width, height, Instant::now()).assert_no_terminal_controls();
    }
}
