use super::*;
use crate::args::{dump_keys, DumpFormat};
use crate::command::{Keymap, Preset};
use crate::overlay::palette::build_palette_rows;
use crate::tabs::Tab;
use crate::test_support;

fn configured(contents: &str) -> (TempDir, App) {
    let (directory, mut app) = test_support::populated_app();
    let path = directory.path().join("reserved-fixture.toml");
    std::fs::write(&path, contents).unwrap();
    app.apply_args(&Args {
        config: Some(path),
        ..Args::default()
    })
    .unwrap();
    (directory, app)
}

fn default_metadata(app: &App, command: Command, name: &str, context: Contexts) -> bool {
    let expected = Keymap::preset(Preset::Vim).rendered_keys(command).unwrap();
    let dump = dump_keys(&app.keys, DumpFormat::Text);
    let dumped = dump
        .lines()
        .find(|line| line.split('\t').nth(1) == Some(name))
        .and_then(|line| line.split('\t').nth(2));
    let palette = build_palette_rows(app, context, name);
    app.keys.rendered_keys(command).as_deref() == Some(expected.as_str())
        && dumped == Some(expected.as_str())
        && palette
            .iter()
            .any(|row| row.spec.id == command && row.keys == expected)
}

fn invalid_digits(sequence: bool) {
    let mut accepted = Vec::new();
    for digit in '0'..='9' {
        let binding = if sequence {
            format!("{digit} x")
        } else {
            digit.to_string()
        };
        let (_directory, app) =
            configured(&format!("[keymap.bindings]\njump-to-tab = '{binding}'\n"));
        if app.startup_notices.is_empty()
            || !default_metadata(
                &app,
                Command::JumpToTab,
                "jump-to-tab",
                Contexts::SECRETS_TREE,
            )
        {
            accepted.push(digit);
        }
    }
    assert!(
        accepted.is_empty(),
        "all digit-leading overrides must warn and retain default metadata: {accepted:?}"
    );
    let (_directory, mut app) = configured("[keymap.bindings]\njump-to-tab = '2'\n");
    test_support::unlock(&mut app);
    app.handle_event(&test_support::key_alt('2'));
    assert!(matches!(app.tabs.active_tab(), Tab::Settings));
    app.handle_event(&test_support::key('g'));
    app.handle_event(&test_support::key('t'));
    assert!(matches!(app.tabs.active_tab(), Tab::Secrets));
    for character in ['2', 'g', 't'] {
        app.handle_event(&test_support::key(character));
    }
    assert!(matches!(app.tabs.active_tab(), Tab::Settings));
}

#[test]
fn digit_single_jump_keeps_default_keys_metadata_and_dispatch() {
    invalid_digits(false);
}

#[test]
fn digit_leading_sequence_keeps_default_keys_metadata_and_dispatch() {
    invalid_digits(true);
}

fn open_edit(app: &mut App) -> Uuid {
    test_support::unlock(app);
    let id = app.vault().unwrap().entry_uuids_by_title("Root1")[0];
    app.tree.select(id);
    app.handle_event(&test_support::key('e'));
    let Some(Overlay::Edit(state)) = app.overlay.as_mut() else {
        panic!("edit open")
    };
    state.focus = state
        .fields()
        .iter()
        .position(|field| *field == EditField::Password)
        .unwrap();
    id
}

fn has_generator(app: &App) -> bool {
    matches!(app.overlay.as_ref(),Some(Overlay::Edit(state)) if state.generating.is_some())
}

fn rejected_generator(config: &str) {
    let (_directory, mut app) = configured(config);
    let rejected = !app.startup_notices.is_empty();
    let id = open_edit(&mut app);
    let metadata = default_metadata(&app, Command::Generate, "generate", Contexts::EDIT);
    let default = Keymap::preset(Preset::Vim)
        .rendered_keys(Command::Generate)
        .unwrap();
    let hints = app
        .overlay
        .as_ref()
        .unwrap()
        .hints(&app.keys)
        .contains(&default);
    let visible = test_support::render_at(&app, 110, 32, Instant::now())
        .transcript
        .contains(&default);
    app.handle_event(&test_support::key_ctrl('g'));
    let generated = has_generator(&app);
    if generated {
        app.handle_event(&test_support::key_code(KeyCode::Esc));
    }
    let Some(Overlay::Edit(state)) = app.overlay.as_mut() else {
        panic!("edit remains")
    };
    state.title = tui_input::Input::new("Saved via real Ctrl+S".to_string());
    let before = app.save_count;
    app.handle_event(&test_support::key_ctrl('s'));
    let saved = app.overlay.is_none()
        && app.save_count == before + 1
        && app.vault().unwrap().get_entry(id).unwrap().title() == "Saved via real Ctrl+S";
    assert!(rejected && metadata && hints && visible && generated && saved,
        "reserved override must retain default generation and real save: rejected={rejected} metadata={metadata} hints={hints} visible={visible} generated={generated} saved={saved}");
}

#[test]
fn generate_ctrl_s_warns_and_keeps_default_generation_and_actual_save() {
    rejected_generator("[keymap.bindings]\ngenerate = 'ctrl+s'\n");
}

#[test]
fn generate_ctrl_alt_s_matches_the_same_actual_save_predicate() {
    rejected_generator("[keymap.bindings]\ngenerate = 'ctrl+alt+s'\n");
}

#[test]
fn generate_escape_cannot_bypass_hardcoded_cancel_after_registry_unbind() {
    rejected_generator("[keymap.bindings]\ncancel = []\ngenerate = 'esc'\n");
}

#[test]
fn counted_sequence_can_still_contain_a_digit_after_its_prefix() {
    let (_directory, mut app) = configured("[keymap.bindings]\njump-to-tab = 'x 2'\n");
    assert!(app.startup_notices.is_empty());
    test_support::unlock(&mut app);
    for character in ['2', 'x', '2'] {
        app.handle_event(&test_support::key(character));
    }
    assert!(matches!(app.tabs.active_tab(), Tab::Settings));
}

#[test]
fn modified_digit_jump_still_accepts_a_count() {
    let (_directory, mut app) = configured("[keymap.bindings]\njump-to-tab = 'ctrl+2'\n");
    assert!(app.startup_notices.is_empty());
    test_support::unlock(&mut app);
    app.handle_event(&test_support::key('2'));
    app.handle_event(&test_support::key_ctrl('2'));
    assert!(matches!(app.tabs.active_tab(), Tab::Settings));
}

#[test]
fn other_control_alt_generator_binding_remains_supported() {
    let (_directory, mut app) = configured("[keymap.bindings]\ngenerate = 'ctrl+alt+g'\n");
    assert!(app.startup_notices.is_empty());
    open_edit(&mut app);
    app.handle_event(&Event::Key(KeyEvent::new(
        KeyCode::Char('g'),
        KeyModifiers::CONTROL | KeyModifiers::ALT,
    )));
    assert!(has_generator(&app));
    app.handle_event(&test_support::key_code(KeyCode::Esc));
    app.handle_event(&test_support::key_code(KeyCode::Esc));
    assert!(
        app.overlay.is_none(),
        "actual Escape cancel remains available"
    );
}

#[test]
fn default_metadata_oracles_and_editor_generation_are_valid_controls() {
    let (_directory, mut app) = configured("");
    assert!(app.startup_notices.is_empty());
    assert!(default_metadata(
        &app,
        Command::JumpToTab,
        "jump-to-tab",
        Contexts::SECRETS_TREE
    ));
    open_edit(&mut app);
    assert!(default_metadata(
        &app,
        Command::Generate,
        "generate",
        Contexts::EDIT
    ));
    let expected = Keymap::preset(Preset::Vim)
        .rendered_keys(Command::Generate)
        .unwrap();
    assert!(app
        .overlay
        .as_ref()
        .unwrap()
        .hints(&app.keys)
        .contains(&expected));
    assert!(test_support::render_at(&app, 110, 32, Instant::now())
        .transcript
        .contains(&expected));
    app.handle_event(&test_support::key_ctrl('g'));
    assert!(has_generator(&app));
}
