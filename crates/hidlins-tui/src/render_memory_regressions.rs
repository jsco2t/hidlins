//! Observe actual first-party render payloads before their owners release them.

use std::cell::RefCell;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hidlins_core::{EntryKind, Zeroizing};
use ratatui::text::Line;
use tempfile::TempDir;

use crate::allocation_observer::DeallocationWatch;
use crate::app::App;
use crate::overlay::edit::{Col, EditField, EditState};
use crate::overlay::generate::GenState;
use crate::overlay::Overlay;
use crate::test_support;
use crate::theme::Theme;
use crate::widgets::entry_detail::{detail_lines, CustomFieldData, DetailData};
use crate::widgets::text_area::TextArea;

const CANARY: &str = "synthetic-render-secret-7Q2F";
const TOTP_CANARY: &str = "428591";

struct WatchedRender {
    tag: &'static str,
    needle: &'static str,
    allocation: Option<DeallocationWatch>,
}

thread_local! {
    static WATCH: RefCell<Option<WatchedRender>> = const { RefCell::new(None) };
}

/// Test-only producer hook: record a live initialized span at its allocation
/// base. Fixtures select a first-line/source buffer, never an interior slice.
pub(crate) fn capture(tag: &str, lines: &[Line<'_>]) {
    WATCH.with(|watch| {
        let mut watch = watch.borrow_mut();
        let Some(watch) = watch.as_mut() else { return };
        if watch.tag != tag || watch.allocation.is_some() {
            return;
        }
        if let Some(span) = lines
            .iter()
            .flat_map(|line| &line.spans)
            .find(|span| span.content.contains(watch.needle))
        {
            watch.allocation = Some(DeallocationWatch::new(span.content.as_bytes()));
        }
    });
}

struct Observation;

impl Observation {
    fn new(tag: &'static str, needle: &'static str) -> Self {
        WATCH.with(|watch| {
            assert!(watch.borrow().is_none());
            *watch.borrow_mut() = Some(WatchedRender {
                tag,
                needle,
                allocation: None,
            });
        });
        Self
    }

    fn assert_erased(self) {
        let watch = WATCH.with(|watch| watch.borrow_mut().take().unwrap());
        watch
            .allocation
            .expect("producer must expose the secret payload")
            .assert_erased();
        drop(self);
    }
}

impl Drop for Observation {
    fn drop(&mut self) {
        WATCH.with(|watch| {
            watch.borrow_mut().take();
        });
    }
}

fn inspect_detail(data: DetailData, needle: &'static str) {
    let observation = Observation::new("detail", needle);
    let theme = Theme::from_env_parts(None, false, Some("xterm-256color"), Some("truecolor"));
    let lines = detail_lines(&data, true, &theme);
    assert!(lines
        .iter()
        .flat_map(|line| &line.spans)
        .any(|span| span.content.contains(needle)));
    drop(lines);
    drop(data);
    observation.assert_erased();
}

#[test]
fn detail_reveal_password_payload_erases_before_release() {
    inspect_detail(
        DetailData {
            password: CANARY.to_string().into(),
            ..DetailData::default()
        },
        CANARY,
    );
}

#[test]
fn detail_custom_payload_erases_before_release() {
    inspect_detail(
        DetailData {
            custom_fields: vec![CustomFieldData {
                name: "API key".to_string(),
                value: CANARY.to_string().into(),
                protected: true,
            }],
            ..DetailData::default()
        },
        CANARY,
    );
}

#[test]
fn detail_note_payload_erases_before_release() {
    inspect_detail(
        DetailData {
            notes: CANARY.to_string().into(),
            ..DetailData::default()
        },
        CANARY,
    );
}

#[test]
fn detail_current_totp_payload_erases_before_release() {
    fn owned_text<T: From<String>>(value: &str) -> T {
        T::from(value.to_string())
    }
    inspect_detail(
        DetailData {
            totp: Some((owned_text(TOTP_CANARY), 25)),
            ..DetailData::default()
        },
        TOTP_CANARY,
    );
}

fn editing() -> (TempDir, App) {
    let (dir, mut app) = test_support::populated_app();
    test_support::unlock(&mut app);
    let id = app.vault().unwrap().entry_uuids_by_title("Root1")[0];
    app.tree.select(id);
    app.handle_event(&test_support::key('e'));
    assert!(matches!(app.overlay, Some(Overlay::Edit(_))));
    (dir, app)
}

fn editor(app: &mut App) -> &mut EditState {
    let Some(Overlay::Edit(state)) = app.overlay.as_mut() else {
        panic!("expected Edit")
    };
    state
}

fn inspect_app(app: App, tag: &'static str) {
    let observation = Observation::new(tag, CANARY);
    test_support::render_at(&app, 110, 32, Instant::now());
    drop(app);
    observation.assert_erased();
}

#[test]
fn edit_revealed_password_payload_erases_before_release() {
    let (_dir, mut app) = editing();
    editor(&mut app).password.set_value(CANARY);
    editor(&mut app).reveal_password = true;
    inspect_app(app, "edit");
}

fn custom_payload(protected: bool) {
    let (_dir, mut app) = editing();
    let state = editor(&mut app);
    state.add_custom_row();
    state.custom[0].name = tui_input::Input::new("API key".to_string());
    state.custom[0].value.set_value(CANARY);
    state.custom[0].protected = protected;
    state.reveal_password = true;
    state.focus = state
        .fields()
        .iter()
        .position(|field| *field == EditField::Custom(0, Col::Value))
        .unwrap();
    inspect_app(app, "edit");
}

#[test]
fn edit_protected_custom_payload_erases_before_release() {
    custom_payload(true);
}

#[test]
fn edit_unprotected_custom_payload_erases_before_release() {
    custom_payload(false);
}

#[test]
fn edit_totp_uri_caret_payload_erases_before_release() {
    let (_dir, mut app) = editing();
    let state = editor(&mut app);
    state.kind = EntryKind::Totp;
    state.is_new = true;
    state.totp_uri.set_value(&format!(
        "otpauth://totp/fixture?secret={CANARY}&issuer=fixture"
    ));
    for _ in 0..3 {
        state
            .totp_uri
            .on_key(&KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    }
    state.focus = state
        .fields()
        .iter()
        .position(|field| *field == EditField::TotpUri)
        .unwrap();
    inspect_app(app, "edit");
}

#[test]
fn edit_note_caret_payload_erases_before_release() {
    let (_dir, mut app) = editing();
    let state = editor(&mut app);
    state.notes = TextArea::from_str(&format!("{CANARY} trailing"));
    for _ in 0..3 {
        state
            .notes
            .on_key(&KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    }
    state.focus = state
        .fields()
        .iter()
        .position(|field| *field == EditField::Notes)
        .unwrap();
    inspect_app(app, "edit");
}

#[test]
fn generated_preview_payload_erases_before_release() {
    let (_dir, mut app) = editing();
    let mut generation = GenState::new();
    generation.length = CANARY.len();
    generation.preview = Zeroizing::new(CANARY.to_string());
    editor(&mut app).generating = Some(generation);
    inspect_app(app, "generate");
}

#[test]
fn historical_note_payload_erases_before_release() {
    let (_dir, mut app) = editing();
    editor(&mut app).notes = TextArea::from_str(CANARY);
    app.handle_event(&test_support::key_ctrl('s'));
    app.handle_event(&test_support::key('e'));
    editor(&mut app).notes = TextArea::from_str("newer note");
    app.handle_event(&test_support::key_ctrl('s'));
    app.handle_event(&test_support::key('H'));
    let Some(Overlay::History(state)) = app.overlay.as_ref() else {
        panic!("expected History")
    };
    let index = state
        .snapshots
        .iter()
        .position(|snapshot| snapshot.notes.as_str() == CANARY)
        .unwrap();
    for _ in 0..index {
        app.handle_event(&test_support::key_code(KeyCode::Down));
    }
    let Some(Overlay::History(state)) = app.overlay.as_ref() else {
        unreachable!()
    };
    assert_eq!(state.snapshots[state.selected].notes.as_str(), CANARY);
    inspect_app(app, "history");
}

#[test]
fn masked_edit_does_not_copy_secret_into_visible_payload() {
    let (_dir, mut app) = editing();
    editor(&mut app).password.set_value(CANARY);
    assert!(!editor(&mut app).reveal_password);
    test_support::render_at(&app, 110, 32, Instant::now()).assert_excludes(&[CANARY]);
}
