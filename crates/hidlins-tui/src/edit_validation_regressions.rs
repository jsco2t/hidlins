//! Real App save/keyboard/render boundaries for Edit safety.

use std::time::Instant;

use crossterm::event::KeyCode;
use hidlins_core::Uuid;
use tempfile::TempDir;
use tui_input::Input;

use crate::app::App;
use crate::overlay::edit::{Col, EditField, EditState};
use crate::overlay::Overlay;
use crate::test_support;

const CUSTOM_VALUE: &str = "protected-custom-canary";

fn editing() -> (TempDir, App, Uuid) {
    let (dir, mut app) = test_support::populated_app();
    test_support::unlock(&mut app);
    let id = app.vault().unwrap().entry_uuids_by_title("Root1")[0];
    app.tree.select(id);
    app.handle_event(&test_support::key('e'));
    assert!(matches!(app.overlay, Some(Overlay::Edit(_))));
    (dir, app, id)
}

fn state(app: &App) -> &EditState {
    let Some(Overlay::Edit(state)) = app.overlay.as_ref() else {
        panic!("expected Edit overlay");
    };
    state
}

fn state_mut(app: &mut App) -> &mut EditState {
    let Some(Overlay::Edit(state)) = app.overlay.as_mut() else {
        panic!("expected Edit overlay");
    };
    state
}

fn add_custom(state: &mut EditState, name: &str, protected: bool) {
    state.add_custom_row();
    let row = state.custom.last_mut().unwrap();
    row.name = Input::new(name.to_string());
    row.value.set_value(CUSTOM_VALUE);
    row.protected = protected;
    let index = state.custom.len() - 1;
    state.focus = state
        .fields()
        .iter()
        .position(|field| *field == EditField::Custom(index, Col::Value))
        .unwrap();
}

fn rejects_standard_custom_name(name: &str, is_new: bool) {
    let (dir, mut app, id) = editing();
    let before = std::fs::read(dir.path().join("personal.kdbx")).unwrap();
    let count = app.vault().unwrap().entry_summaries().len();
    if is_new {
        let root = app.vault().unwrap().root_group_uuid();
        app.overlay = Some(Overlay::Edit(Box::new(EditState::new_add(root))));
    }
    state_mut(&mut app).title = Input::new("Valid dedicated title".to_string());
    add_custom(state_mut(&mut app), name, false);
    app.handle_event(&test_support::key_ctrl('s'));
    assert!(
        std::fs::read(dir.path().join("personal.kdbx")).unwrap() == before,
        "a reserved custom name must not change ciphertext"
    );
    assert!(
        state(&app).error.is_some(),
        "standard field must be rejected"
    );
    assert_eq!(app.vault().unwrap().entry_summaries().len(), count);
    assert_eq!(
        std::fs::read(dir.path().join("personal.kdbx")).unwrap(),
        before
    );
    let vault = app.vault().unwrap();
    let entry = vault.get_entry(id).unwrap();
    assert_eq!(entry.title(), "Root1");
    assert_eq!(entry.username(), "alice");
    assert_eq!(entry.password(), test_support::SECRET_CANARY);
    assert_eq!(entry.url(), "");
    assert_eq!(entry.notes(), "");
}

#[test]
fn adding_rejects_every_standard_custom_field_without_mutation() {
    for name in ["Title", "UserName", "Password", "URL", "Notes"] {
        rejects_standard_custom_name(name, true);
    }
}

#[test]
fn editing_rejects_every_standard_custom_field_without_mutation() {
    for name in ["Title", "UserName", "Password", "URL", "Notes"] {
        rejects_standard_custom_name(name, false);
    }
}

#[test]
fn ordinary_custom_field_still_saves_with_protection() {
    let (_dir, mut app, id) = editing();
    add_custom(state_mut(&mut app), "API key", true);
    app.handle_event(&test_support::key_ctrl('s'));
    assert!(app.overlay.is_none());
    let vault = app.vault().unwrap();
    let entry = vault.get_entry(id).unwrap();
    assert_eq!(entry.custom_field("API key"), Some(CUSTOM_VALUE));
    assert_eq!(entry.custom_field_is_protected("API key"), Some(true));
    assert_eq!(entry.password(), test_support::SECRET_CANARY);
}

#[test]
fn failed_generation_enter_keeps_panel_and_original_password() {
    let (dir, mut app, id) = editing();
    let before = std::fs::read(dir.path().join("personal.kdbx")).unwrap();
    app.handle_event(&test_support::key_ctrl('g'));
    for class in ['l', 'u', 'd', 's'] {
        app.handle_event(&test_support::key(class));
    }
    assert!(state(&app).generating.as_ref().unwrap().error.is_some());
    app.handle_event(&test_support::key_code(KeyCode::Enter));
    assert_eq!(state(&app).password.as_str(), test_support::SECRET_CANARY);
    assert!(state(&app)
        .generating
        .as_ref()
        .is_some_and(|panel| panel.error.is_some()));
    assert_eq!(
        std::fs::read(dir.path().join("personal.kdbx")).unwrap(),
        before
    );
    app.handle_event(&test_support::key_code(KeyCode::Esc));
    app.handle_event(&test_support::key_ctrl('s'));
    assert!(app.overlay.is_none());
    assert_eq!(
        app.vault().unwrap().get_entry(id).unwrap().password(),
        test_support::SECRET_CANARY
    );
}

#[test]
fn successful_generation_still_accepts_and_masks_output() {
    let (_dir, mut app, _id) = editing();
    app.handle_event(&test_support::key_ctrl('g'));
    let preview = state(&app).generating.as_ref().unwrap().preview.clone();
    assert!(!preview.is_empty());
    app.handle_event(&test_support::key_code(KeyCode::Enter));
    assert!(state(&app).generating.is_none());
    assert_eq!(state(&app).password.as_str(), preview.as_str());
    assert!(!state(&app).reveal_password);
}

#[test]
fn cancelling_generation_preserves_original_password() {
    let (_dir, mut app, _id) = editing();
    app.handle_event(&test_support::key_ctrl('g'));
    app.handle_event(&test_support::key_code(KeyCode::Esc));
    assert!(state(&app).generating.is_none());
    assert_eq!(state(&app).password.as_str(), test_support::SECRET_CANARY);
}

#[test]
fn protected_custom_value_requires_explicit_reveal_and_can_be_hidden() {
    let (_dir, mut app, id) = editing();
    add_custom(state_mut(&mut app), "API key", true);
    assert!(!state(&app).reveal_password);
    test_support::render_at(&app, 100, 32, Instant::now()).assert_excludes(&[CUSTOM_VALUE]);
    assert!(state(&app).hints().contains("Ctrl+R"));
    app.handle_event(&test_support::key_ctrl('r'));
    test_support::render_at(&app, 100, 32, Instant::now()).assert_contains_all(&[CUSTOM_VALUE]);
    app.handle_event(&test_support::key_ctrl('r'));
    test_support::render_at(&app, 100, 32, Instant::now()).assert_excludes(&[CUSTOM_VALUE]);
    app.handle_event(&test_support::key_ctrl('s'));
    assert!(app.overlay.is_none());
    assert_eq!(
        app.vault()
            .unwrap()
            .get_entry(id)
            .unwrap()
            .custom_field_is_protected("API key"),
        Some(true)
    );
}
