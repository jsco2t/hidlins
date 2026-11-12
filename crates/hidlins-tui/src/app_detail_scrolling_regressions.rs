use super::*;
use crate::test_support;
use std::fmt::Write as _;

fn note_words() -> String {
    let mut text = String::new();
    for index in 0..600 {
        write!(&mut text, "w{index:03} ").unwrap();
    }
    text.push_str("\nEND-MARKER\n");
    text
}

fn detail_app(notes: &str, pinned: bool) -> (TempDir, App) {
    let (directory, mut app) = populated_app();
    unlock(&mut app, PASSWORD);
    let vault = app.vault_mut().unwrap();
    let root = vault.root_group_uuid();
    let uuid = vault
        .add_entry(
            root,
            EntryBuilder::credential("Wrapped detail")
                .password("private-fixture-password")
                .notes(notes)
                .build(),
        )
        .unwrap();
    app.jump_to_entry(uuid);
    if pinned {
        app.handle_event(&key('p'));
        app.tabs.jump_to(2);
        assert!(matches!(app.tabs.active_tab(), Tab::Pinned(value) if value == uuid));
    }
    (directory, app)
}

fn scroll_to_bottom(app: &mut App, width: u16, height: u16) -> test_support::RenderedFrame {
    test_support::render_at(app, width, height, Instant::now());
    for _ in 0..600 {
        app.handle_event(&key('j'));
    }
    test_support::render_at(app, width, height, Instant::now())
}

#[test]
fn secrets_keyboard_reaches_wrapped_note_tail_and_history_footer() {
    let (_directory, mut app) = detail_app(&note_words(), false);
    let rendered = scroll_to_bottom(&mut app, 40, 12);
    rendered.assert_contains_all(&["END-MARKER", "History: 0"]);
    assert!(!rendered.transcript.contains("private-fixture-password"));
}

#[test]
fn pinned_keyboard_reaches_wrapped_note_tail_and_history_footer() {
    let (_directory, mut app) = detail_app(&note_words(), true);
    scroll_to_bottom(&mut app, 40, 12).assert_contains_all(&["END-MARKER", "History: 0"]);
}

#[test]
fn long_words_and_unicode_graphemes_use_actual_rendered_rows_without_mouse() {
    for body in ["x".repeat(1600), "漢字 é\u{301} 🔒 ".repeat(160)] {
        let note = format!("{body}\nEND-MARKER\n");
        let (_directory, mut app) = detail_app(&note, false);
        app.mouse_enabled = false;
        scroll_to_bottom(&mut app, 40, 12).assert_contains_all(&["END-MARKER", "History: 0"]);
    }
}

fn check_resize(pinned: bool) {
    let (_directory, mut app) = detail_app(&note_words(), pinned);
    scroll_to_bottom(&mut app, 40, 12).assert_contains_all(&["END-MARKER"]);
    let before = test_support::render_at(&app, 100, 30, Instant::now());
    before.assert_contains_all(&["END-MARKER", "History: 0"]);
    app.handle_event(&key('k'));
    let after = test_support::render_at(&app, 100, 30, Instant::now());
    let body = |text: &str| text.lines().skip(2).take(25).collect::<String>();
    assert_ne!(
        body(&before.transcript),
        body(&after.transcript),
        "one reverse motion immediately leaves the visible bottom after resize"
    );
    scroll_to_bottom(&mut app, 36, 12).assert_contains_all(&["END-MARKER", "History: 0"]);
}

#[test]
fn secrets_resize_clamps_both_rendering_and_reverse_navigation() {
    check_resize(false);
}

#[test]
fn pinned_resize_clamps_both_rendering_and_reverse_navigation() {
    check_resize(true);
}

#[test]
fn short_content_and_explicit_reveal_keep_their_existing_contracts() {
    let (_directory, mut app) = detail_app("one line", false);
    let masked = test_support::render_at(&app, 100, 30, Instant::now());
    masked.assert_contains_all(&["Wrapped detail", "one line", "Password:", "History: 0"]);
    assert!(!masked.transcript.contains("private-fixture-password"));
    app.handle_event(&key(' '));
    test_support::render_at(&app, 100, 30, Instant::now())
        .assert_contains_all(&["private-fixture-password", "one line"]);
}
