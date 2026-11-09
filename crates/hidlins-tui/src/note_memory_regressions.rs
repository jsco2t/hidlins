//! Observe actual secure-note owners before their initialized allocations are freed.

use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hidlins_core::{EntryBuilder, KdfParams, MasterPassword, NoRecoveryConfirmed, Uuid, Vault};
use tempfile::TempDir;

use crate::allocation_observer::DeallocationWatch;
use crate::app::{App, Phase};
use crate::overlay::edit::EditState;
use crate::overlay::history::HistoryState;
use crate::overlay::Overlay;
use crate::test_support::{self, MASTER_PASSWORD};
use crate::widgets::entry_detail;
use crate::widgets::text_area::TextArea;

const NOTE: &str = "synthetic recovery codes 🔒 1234-5678";

fn key(area: &mut TextArea, code: KeyCode) {
    assert!(area.on_key(&KeyEvent::new(code, KeyModifiers::NONE)));
}

fn add_note(vault: &mut Vault) -> Uuid {
    let id = vault
        .add_entry(
            vault.root_group_uuid(),
            EntryBuilder::secure_note("Note").notes(NOTE).build(),
        )
        .unwrap();
    vault
        .update_entry(id, |entry| {
            entry.set_title("Revised note");
            Ok(())
        })
        .unwrap();
    vault.save().unwrap();
    id
}

fn note_vault() -> (TempDir, Vault, Uuid) {
    let dir = tempfile::tempdir().unwrap();
    let mut vault = Vault::create(
        &dir.path().join("note.kdbx"),
        &MasterPassword::new(MASTER_PASSWORD.to_string()),
        None,
        KdfParams {
            memory_kib: 1_024,
            iterations: 1,
            parallelism: 1,
        },
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    let id = add_note(&mut vault);
    (dir, vault, id)
}

fn edit_app() -> (TempDir, App, Uuid) {
    let (dir, mut app) = test_support::configured_app(&["personal"]);
    let mut vault = Vault::open(
        &dir.path().join("personal.kdbx"),
        &MasterPassword::new(MASTER_PASSWORD.to_string()),
        None,
    )
    .unwrap();
    let id = add_note(&mut vault);
    drop(vault);
    test_support::unlock(&mut app);
    app.tree.select(id);
    app.handle_event(&test_support::key('e'));
    assert!(matches!(app.overlay, Some(Overlay::Edit(_))));
    (dir, app, id)
}

fn editor_bytes(app: &App) -> &[u8] {
    let Some(Overlay::Edit(state)) = app.overlay.as_ref() else {
        panic!("expected note editor");
    };
    state.notes.lines()[0].as_bytes()
}

#[test]
fn note_editor_drop_erases_line() {
    let area = TextArea::from_str(NOTE);
    let watch = DeallocationWatch::new(area.lines()[0].as_bytes());
    drop(area);
    watch.assert_erased();
}

#[test]
fn cloned_note_editor_erases_its_own_line() {
    let area = TextArea::from_str(NOTE);
    let cloned = area.clone();
    let watch = DeallocationWatch::new(cloned.lines()[0].as_bytes());
    drop(cloned);
    watch.assert_erased();
    assert_eq!(area.value().as_str(), NOTE);
}

#[test]
fn note_line_growth_erases_previous_allocation() {
    let mut area = TextArea::from_str(NOTE);
    let watch = DeallocationWatch::new(area.lines()[0].as_bytes());
    for _ in 0..64 {
        key(&mut area, KeyCode::Char('界'));
    }
    assert_eq!(area.value().as_str(), format!("{NOTE}{}", "界".repeat(64)));
    watch.assert_erased();
}

#[test]
fn merging_note_lines_erases_removed_line_allocation() {
    let mut area = TextArea::from_str(&format!("prefix\n{NOTE}"));
    for _ in NOTE.chars() {
        key(&mut area, KeyCode::Left);
    }
    let watch = DeallocationWatch::new(area.lines()[1].as_bytes());
    key(&mut area, KeyCode::Backspace);
    assert_eq!(area.value().as_str(), format!("prefix{NOTE}"));
    watch.assert_erased();
}

#[test]
fn split_note_tail_keeps_erasing_ownership() {
    let mut area = TextArea::from_str(&format!("prefix{NOTE}"));
    for _ in NOTE.chars() {
        key(&mut area, KeyCode::Left);
    }
    key(&mut area, KeyCode::Enter);
    assert_eq!(area.value().as_str(), format!("prefix\n{NOTE}"));
    let watch = DeallocationWatch::new(area.lines()[1].as_bytes());
    drop(area);
    watch.assert_erased();
}

#[test]
fn committed_edit_snapshot_erases_note_copy() {
    let (_dir, vault, id) = note_vault();
    let view = vault.get_entry(id).unwrap();
    let state = EditState::from_entry(id, &view);
    let values = state.snapshot().unwrap();
    assert_eq!(values.notes.as_str(), NOTE);
    let watch = DeallocationWatch::new(values.notes.as_bytes());
    drop(values);
    watch.assert_erased();
}

#[test]
fn live_detail_snapshot_erases_note_copy() {
    let (_dir, vault, id) = note_vault();
    let data = entry_detail::detail_data_for(&vault, id, Utc::now()).unwrap();
    assert_eq!(data.notes.as_str(), NOTE);
    let watch = DeallocationWatch::new(data.notes.as_bytes());
    drop(data);
    watch.assert_erased();
}

#[test]
fn history_snapshot_erases_note_copy() {
    let (_dir, vault, id) = note_vault();
    let view = vault.get_entry(id).unwrap();
    let history = HistoryState::from_views("Note".to_string(), &view.history());
    assert_eq!(history.snapshots.len(), 1);
    assert_eq!(history.snapshots[0].notes.as_str(), NOTE);
    let watch = DeallocationWatch::new(history.snapshots[0].notes.as_bytes());
    drop(history);
    watch.assert_erased();
}

#[test]
fn cancelling_actual_edit_erases_note_without_changing_entry() {
    let (_dir, mut app, id) = edit_app();
    let watch = DeallocationWatch::new(editor_bytes(&app));
    app.handle_event(&test_support::key_code(KeyCode::Esc));
    assert!(app.overlay.is_none());
    assert_eq!(app.vault().unwrap().get_entry(id).unwrap().notes(), NOTE);
    watch.assert_erased();
}

#[test]
fn locking_actual_edit_erases_note_before_returning_to_unlock() {
    let (_dir, mut app, _id) = edit_app();
    let watch = DeallocationWatch::new(editor_bytes(&app));
    app.handle_event(&test_support::key_ctrl('l'));
    assert!(matches!(app.phase, Phase::LockScreen));
    assert!(app.overlay.is_none());
    assert!(app.vault().is_none());
    watch.assert_erased();
}

#[test]
fn saving_actual_edit_erases_note_and_preserves_content() {
    let (_dir, mut app, id) = edit_app();
    let watch = DeallocationWatch::new(editor_bytes(&app));
    app.handle_event(&test_support::key_ctrl('s'));
    assert!(app.overlay.is_none());
    assert_eq!(app.vault().unwrap().get_entry(id).unwrap().notes(), NOTE);
    watch.assert_erased();
}

#[test]
fn unicode_split_merge_and_cursor_keep_note_content() {
    let mut area = TextArea::from_str("aé🔒");
    key(&mut area, KeyCode::Left);
    key(&mut area, KeyCode::Enter);
    assert_eq!(area.value().as_str(), "aé\n🔒");
    assert_eq!(area.cursor(), (1, 0));
    key(&mut area, KeyCode::Backspace);
    assert_eq!(area.value().as_str(), "aé🔒");
    assert_eq!(area.cursor(), (0, 2));
}

#[test]
fn view_and_history_producers_preserve_note_values() {
    let (_dir, vault, id) = note_vault();
    let view = vault.get_entry(id).unwrap();
    assert_eq!(view.notes(), NOTE);
    let state = EditState::from_entry(id, &view);
    assert_eq!(state.snapshot().unwrap().notes.as_str(), NOTE);
    let history = HistoryState::from_views("Note".to_string(), &view.history());
    assert_eq!(history.snapshots[0].notes.as_str(), NOTE);
    assert_eq!(history.snapshots[0].title, "Note");
}
