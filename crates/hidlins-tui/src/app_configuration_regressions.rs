// Exercise effective configuration through App without changing the process environment.

use super::*;
use crate::args::{dump_keys, DumpFormat};
use crate::clipboard::RecordingClipboard;
use crate::overlay::palette::build_palette_rows;
use crate::tabs::Tab;
use crate::test_support;
use crate::theme::{EnvParts, ThemeDef, ThemePatch};
use crate::user_config::ThemeCfg;

const SENTINEL: &str = "synthetic-recovery-secret-9Q3F";

fn configured(contents: &str) -> (TempDir, App) {
    let (dir, mut app) = test_support::populated_app();
    let path = dir.path().join("ui-config.toml");
    std::fs::write(&path, contents).unwrap();
    app.apply_args(&Args {
        config: Some(path),
        ..Args::default()
    })
    .unwrap();
    (dir, app)
}

fn open_edit(app: &mut App) {
    test_support::unlock(app);
    let id = app.vault().unwrap().entry_uuids_by_title("Root1")[0];
    app.tree.select(id);
    app.handle_event(&test_support::key('e'));
    let Some(Overlay::Edit(state)) = app.overlay.as_mut() else {
        panic!("expected Edit");
    };
    state.focus = state
        .fields()
        .iter()
        .position(|field| *field == EditField::Password)
        .unwrap();
}

fn has_generator(app: &App) -> bool {
    matches!(app.overlay.as_ref(), Some(Overlay::Edit(state)) if state.generating.is_some())
}

fn assert_metadata(app: &App, cmd: Command, name: &str, expected: &str, context: Contexts) {
    assert_eq!(app.keys.rendered_keys(cmd).as_deref(), Some(expected));
    let dump = dump_keys(&app.keys, DumpFormat::Text);
    let row = dump
        .lines()
        .find(|line| line.split('\t').nth(1) == Some(name))
        .unwrap();
    assert_eq!(row.split('\t').nth(2), Some(expected));
    let rows = build_palette_rows(app, context, name);
    assert_eq!(
        rows.iter().find(|row| row.spec.id == cmd).unwrap().keys,
        expected
    );
}

#[test]
fn unsupported_non_tab_sequence_keeps_default_dispatch_and_metadata() {
    let (_dir, mut app) = configured("[keymap.bindings]\ncopy-password = 'x y'\n");
    assert!(!app.startup_notices.is_empty());
    assert_metadata(
        &app,
        Command::CopyPassword,
        "copy-password",
        "c",
        Contexts::SECRETS_TREE,
    );
    test_support::unlock(&mut app);
    let id = app.vault().unwrap().entry_uuids_by_title("Root1")[0];
    app.tree.select(id);
    let sink = RecordingClipboard::new();
    let log = sink.log.clone();
    app.clipboard = Box::new(sink);
    app.handle_event(&test_support::key('c'));
    assert_eq!(log.borrow().copies, 1);
    assert_eq!(
        log.borrow().last.as_ref().unwrap().0,
        test_support::SECRET_CANARY
    );
}

fn counted_jump(binding: &str, keys: &[char]) {
    let (_dir, mut app) = configured(&format!("[keymap.bindings]\njump-to-tab = '{binding}'\n"));
    assert!(app.startup_notices.is_empty());
    test_support::unlock(&mut app);
    for &key in keys {
        app.handle_event(&test_support::key(key));
    }
    assert!(matches!(app.tabs.active_tab(), Tab::Settings));
    // The uncounted default next-tab sequence remains functional.
    app.handle_event(&test_support::key('g'));
    app.handle_event(&test_support::key('t'));
    assert!(matches!(app.tabs.active_tab(), Tab::Secrets));
}

#[test]
fn rebinding_default_tab_sequence_retains_count_parameter() {
    counted_jump("g t", &['2', 'g', 't']);
}

#[test]
fn custom_tab_sequence_retains_count_parameter() {
    counted_jump("x y", &['2', 'x', 'y']);
}

#[test]
fn documented_single_key_jump_accepts_its_count_prefix() {
    counted_jump("z", &['2', 'z']);
}

#[test]
fn rebound_generator_dispatch_and_visible_hints_agree() {
    let (_dir, mut app) = configured("[keymap.bindings]\ngenerate = 'alt+g'\n");
    assert!(app.startup_notices.is_empty());
    open_edit(&mut app);
    assert_metadata(&app, Command::Generate, "generate", "Alt+g", Contexts::EDIT);
    let rendered = test_support::render_at(&app, 110, 32, Instant::now());
    rendered.assert_contains_all(&["Alt+g"]);
    rendered.assert_excludes(&["Ctrl+G", "Ctrl+g"]);
    app.handle_event(&test_support::key_ctrl('g'));
    assert!(!has_generator(&app));
    app.handle_event(&test_support::key_alt('g'));
    assert!(has_generator(&app));
}

#[test]
fn unbound_generator_does_not_dispatch_removed_default() {
    let (_dir, mut app) = configured("[keymap.bindings]\ngenerate = []\n");
    open_edit(&mut app);
    assert!(app.keys.rendered_keys(Command::Generate).is_none());
    app.handle_event(&test_support::key_ctrl('g'));
    assert!(!has_generator(&app));
    test_support::render_at(&app, 110, 32, Instant::now()).assert_excludes(&["Ctrl+G", "Ctrl+g"]);
}

#[test]
fn generator_remains_unavailable_for_note_kind() {
    let (_dir, mut app) = configured("[keymap.bindings]\ngenerate = 'alt+g'\n");
    open_edit(&mut app);
    let Some(Overlay::Edit(state)) = app.overlay.as_mut() else {
        unreachable!()
    };
    state.kind = EntryKind::SecureNote;
    app.handle_event(&test_support::key_alt('g'));
    assert!(!has_generator(&app));
}

#[test]
fn unknown_keymap_key_stays_out_of_startup_warning_and_render() {
    let (_dir, mut app) = configured(&format!("[keymap.bindings]\n'{SENTINEL}' = 'c'\n"));
    assert!(!app.startup_notices.is_empty());
    assert!(app
        .startup_notices
        .iter()
        .all(|notice| !notice.contains(SENTINEL)));
    test_support::unlock(&mut app);
    let rendered = test_support::render_at(&app, 110, 32, Instant::now());
    rendered.assert_excludes(&[SENTINEL]);
    rendered.assert_contains_all(&["unknown keymap slot"]);
}

#[test]
fn unknown_theme_slot_warning_does_not_echo_quoted_key() {
    let patch: ThemePatch = toml::from_str(&format!("'{SENTINEL}' = '#112233'\n")).unwrap();
    let mut def = ThemeDef::builtin("default-dark").unwrap();
    let warnings = patch.apply(&mut def);
    assert!(!warnings.is_empty());
    assert!(warnings
        .iter()
        .all(|warning| !warning.message.contains(SENTINEL)));
}

#[test]
fn unknown_theme_selection_warning_does_not_echo_value() {
    let env = EnvParts {
        theme_override: None,
        no_color: false,
        term: Some("xterm-256color"),
        colorterm: Some("truecolor"),
        colorfgbg: None,
    };
    let (_, warnings) = theme::resolve_theme(env, &ThemeCfg::default(), Some(SENTINEL), &[]);
    assert!(!warnings.is_empty());
    assert!(warnings
        .iter()
        .all(|warning| !warning.message.contains(SENTINEL)));
}

#[test]
fn invalid_known_theme_color_warning_already_hides_value() {
    let patch: ThemePatch = toml::from_str(&format!("accent = '{SENTINEL}'\n")).unwrap();
    let mut def = ThemeDef::builtin("default-dark").unwrap();
    let warnings = patch.apply(&mut def);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].message.contains("invalid color"));
    assert!(!warnings[0].message.contains(SENTINEL));
}

#[test]
fn valid_configuration_keeps_default_tab_and_generator_controls() {
    let (_dir, mut app) = configured("[behavior]\nconfirm-quit = true\n");
    assert!(app.startup_notices.is_empty());
    open_edit(&mut app);
    app.handle_event(&test_support::key_ctrl('g'));
    assert!(has_generator(&app));
}
