use super::*;
use crate::test_support;
use std::fmt::Write as _;

fn history_note(version: usize) -> String {
    let mut text = format!("BEGIN-{version}\n");
    for index in 0..240 {
        write!(&mut text, "漢é{index:03} ").unwrap();
    }
    writeln!(&mut text, "\nTAIL-{version}").unwrap();
    text
}

fn history_app(pinned: bool) -> (TempDir, App, Uuid) {
    let (directory, mut app) = populated_app();
    unlock(&mut app, PASSWORD);
    let vault = app.vault_mut().unwrap();
    let root = vault.root_group_uuid();
    let uuid = vault
        .add_entry(
            root,
            EntryBuilder::credential("version-0")
                .password("HISTORY-PASSWORD-CANARY")
                .notes(history_note(0))
                .build(),
        )
        .unwrap();
    for version in 1..=3 {
        vault
            .update_entry(uuid, |entry| {
                entry.set_title(format!("version-{version}"));
                entry.set_notes(history_note(version));
                Ok(())
            })
            .unwrap();
    }
    let original: Vec<_> = vault
        .get_entry(uuid)
        .unwrap()
        .history()
        .iter()
        .map(|view| view.title().to_string())
        .collect();
    assert_eq!(
        original,
        ["version-2", "version-1", "version-0"],
        "actual core history contract"
    );
    app.jump_to_entry(uuid);
    if pinned {
        app.handle_event(&key('p'));
        app.tabs.jump_to(2);
    }
    app.handle_event(&key('H'));
    assert!(matches!(app.overlay, Some(Overlay::History(_))));
    (directory, app, uuid)
}

fn history_state(app: &App) -> &HistoryState {
    let Some(Overlay::History(state)) = app.overlay.as_ref() else {
        panic!("history open")
    };
    state
}

fn tail(app: &App) -> String {
    let state = history_state(app);
    state.snapshots[state.selected]
        .notes
        .lines()
        .last()
        .unwrap()
        .to_string()
}

fn bottom(app: &mut App, width: u16, height: u16) -> test_support::RenderedFrame {
    test_support::render_at(app, width, height, Instant::now());
    for _ in 0..100 {
        app.handle_event(&key_code(KeyCode::PageDown));
    }
    test_support::render_at(app, width, height, Instant::now())
}

#[test]
fn history_overlay_retains_actual_core_newest_first_order() {
    let (_directory, app, _) = history_app(false);
    let state = history_state(&app);
    let titles: Vec<_> = state
        .snapshots
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(titles, ["version-2", "version-1", "version-0"]);
    assert_eq!(state.selected, 0);
}

#[test]
fn page_keys_reach_wrapped_tail_without_switching_version_or_exposing_password() {
    for pinned in [false, true] {
        let (_directory, mut app, uuid) = history_app(pinned);
        let expected = tail(&app);
        let before = app.save_count;
        let selected = history_state(&app).selected;
        let rendered = bottom(&mut app, 60, 24);
        rendered.assert_contains_all(&[&expected]);
        assert_eq!(history_state(&app).selected, selected);
        assert!(!rendered.transcript.contains("HISTORY-PASSWORD-CANARY"));
        app.handle_event(&key_code(KeyCode::PageUp));
        let previous = test_support::render_at(&app, 60, 24, Instant::now());
        assert!(
            !previous.transcript.contains(&expected),
            "page up leaves the bottom"
        );
        assert_eq!(history_state(&app).selected, selected);
        app.handle_event(&key_code(KeyCode::Esc));
        assert!(app.overlay.is_none());
        assert_eq!(app.save_count, before);
        assert_eq!(
            app.vault().unwrap().get_entry(uuid).unwrap().title(),
            "version-3"
        );
    }
}

#[test]
fn actual_version_change_resets_detail_to_start() {
    let (_directory, mut app, _) = history_app(false);
    let expected = tail(&app);
    bottom(&mut app, 60, 24).assert_contains_all(&[&expected]);
    app.handle_event(&key_code(KeyCode::Down));
    assert_eq!(history_state(&app).selected, 1);
    let frame = test_support::render_at(&app, 60, 24, Instant::now());
    frame.assert_contains_all(&["Title: version-1", "BEGIN-1"]);
    app.handle_event(&key_code(KeyCode::Up));
    assert_eq!(history_state(&app).selected, 0);
    let title = history_state(&app).snapshots[0].title.clone();
    test_support::render_at(&app, 60, 24, Instant::now()).assert_contains_all(&[&title]);
}

#[test]
fn version_boundary_keeps_the_existing_reading_position() {
    let (_directory, mut app, _) = history_app(false);
    for _ in 0..10 {
        app.handle_event(&key('j'));
    }
    let selected = history_state(&app).selected;
    let expected = tail(&app);
    bottom(&mut app, 60, 24).assert_contains_all(&[&expected]);
    app.handle_event(&key('j'));
    assert_eq!(history_state(&app).selected, selected);
    test_support::render_at(&app, 60, 24, Instant::now()).assert_contains_all(&[&expected]);
}

#[test]
fn narrow_unicode_history_resizes_and_reverse_page_moves_immediately() {
    let (_directory, mut app, _) = history_app(false);
    let expected = tail(&app);
    bottom(&mut app, 36, 20).assert_contains_all(&[&expected]);
    let before = test_support::render_at(&app, 100, 30, Instant::now());
    before.assert_contains_all(&[&expected]);
    app.handle_event(&key_code(KeyCode::PageUp));
    let after = test_support::render_at(&app, 100, 30, Instant::now());
    assert!(!after.transcript.contains(&expected));
    bottom(&mut app, 36, 20).assert_contains_all(&[&expected]);
}

#[test]
fn empty_history_page_keys_and_cancel_preserve_vault() {
    let (_directory, mut app) = populated_app();
    unlock(&mut app, PASSWORD);
    let uuid = entry_uuid(&app, "Root1");
    app.jump_to_entry(uuid);
    app.handle_event(&key('H'));
    let before = app.save_count;
    for code in [
        KeyCode::PageDown,
        KeyCode::PageUp,
        KeyCode::Down,
        KeyCode::Up,
    ] {
        app.handle_event(&key_code(code));
    }
    test_support::render_at(&app, 60, 24, Instant::now())
        .assert_contains_all(&["No prior versions."]);
    app.handle_event(&key_code(KeyCode::Esc));
    assert!(app.overlay.is_none());
    assert_eq!(app.save_count, before);
    assert_eq!(
        app.vault().unwrap().get_entry(uuid).unwrap().password(),
        "s3cr3t"
    );
}
