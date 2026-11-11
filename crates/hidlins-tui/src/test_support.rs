//! Shared test-only fixture and semantic render helpers for TUI journeys.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use chrono::{TimeZone, Utc};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use hidlins_core::{
    EntryBuilder, HidlinsPaths, KdfParams, MasterPassword, NoRecoveryConfirmed, RegisteredVault,
    Vault, VaultRegistry,
};
use hidlins_security::AutoLockConfig;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tempfile::TempDir;

use crate::app::{App, Phase};
use crate::theme::Theme;

pub(crate) const MASTER_PASSWORD: &str = "master-canary-7C2F91";
pub(crate) const SECRET_CANARY: &str = "entry-canary-4E8A63";

fn fast_kdf() -> KdfParams {
    KdfParams {
        memory_kib: 1_024,
        iterations: 1,
        parallelism: 1,
    }
}

pub(crate) fn populated_app() -> (TempDir, App) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("personal.kdbx");
    let mut vault = Vault::create(
        &path,
        &MasterPassword::new(MASTER_PASSWORD.to_string()),
        None,
        fast_kdf(),
        NoRecoveryConfirmed::yes(),
    )
    .expect("create fixture vault");
    let root = vault.root_group_uuid();
    let group = vault.create_group(root, "Personal").expect("create group");
    let expired = vault
        .add_entry(
            group,
            EntryBuilder::credential("Old")
                .username("old-user")
                .password("expired-secret")
                .build(),
        )
        .expect("add expired entry");
    vault
        .set_expiration(expired, Utc.timestamp_opt(1_000_000, 0).unwrap())
        .expect("expire entry");
    vault
        .add_entry(
            group,
            EntryBuilder::credential("Alpha")
                .username("alpha-user")
                .password("alpha-secret")
                .build(),
        )
        .expect("add alpha entry");
    vault
        .add_entry(
            root,
            EntryBuilder::credential("Root1")
                .username("alice")
                .password(SECRET_CANARY)
                .build(),
        )
        .expect("add root entry");
    vault.save().expect("save fixture vault");
    drop(vault);

    let paths = HidlinsPaths::with_state_dir(dir.path().join("state"));
    let mut registry = VaultRegistry::with_paths(paths.clone());
    registry
        .register(RegisteredVault {
            name: "personal".to_string(),
            path,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            keyfile_path: None,
            extra: toml::Table::new(),
        })
        .expect("register fixture vault");
    registry.save().expect("save fixture registry");
    let mut app = App::from_registry(
        registry,
        paths,
        AutoLockConfig {
            idle_timeout: Duration::from_secs(300),
        },
    )
    .expect("build fixture app");
    app.theme = Theme::from_env_parts(None, false, Some("xterm-256color"), Some("truecolor"));
    (dir, app)
}

/// Build an empty-registry app beside one real, unregistered fast-KDF vault.
pub(crate) fn onboarding_app() -> (TempDir, App, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let vault_path = dir.path().join("first-vault.kdbx");
    drop(
        Vault::create(
            &vault_path,
            &MasterPassword::new(MASTER_PASSWORD.to_string()),
            None,
            fast_kdf(),
            NoRecoveryConfirmed::yes(),
        )
        .expect("create onboarding fixture vault"),
    );
    let paths = HidlinsPaths::with_state_dir(dir.path().join("state"));
    let registry = VaultRegistry::with_paths(paths.clone());
    let mut app = App::from_registry(registry, paths, AutoLockConfig::default())
        .expect("build onboarding app");
    app.theme = Theme::from_env_parts(None, false, Some("xterm-256color"), Some("truecolor"));
    (dir, app, vault_path)
}

/// Build a configured app with one fast-KDF vault for each registry-order name.
pub(crate) fn configured_app(names: &[&str]) -> (TempDir, App) {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = HidlinsPaths::with_state_dir(dir.path().join("state"));
    let mut registry = VaultRegistry::with_paths(paths.clone());
    for name in names {
        let path = dir.path().join(format!("{name}.kdbx"));
        drop(
            Vault::create(
                &path,
                &MasterPassword::new(MASTER_PASSWORD.to_string()),
                None,
                fast_kdf(),
                NoRecoveryConfirmed::yes(),
            )
            .expect("create configured fixture vault"),
        );
        registry
            .register(RegisteredVault {
                name: (*name).to_string(),
                path,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                keyfile_path: None,
                extra: toml::Table::new(),
            })
            .expect("register configured fixture vault");
    }
    registry.save().expect("save configured fixture registry");
    let mut app = App::from_registry(registry, paths, AutoLockConfig::default())
        .expect("build configured app");
    app.theme = Theme::from_env_parts(None, false, Some("xterm-256color"), Some("truecolor"));
    (dir, app)
}

pub(crate) fn key(c: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
}

pub(crate) fn key_code(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

pub(crate) fn key_ctrl(c: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL))
}

pub(crate) fn key_alt(c: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT))
}

pub(crate) fn mouse_down(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

pub(crate) fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        app.handle_event(&key(c));
    }
}

pub(crate) fn unlock(app: &mut App) {
    if matches!(app.phase, Phase::UnlockList) {
        app.handle_event(&key_code(KeyCode::Enter));
    }
    type_text(app, MASTER_PASSWORD);
    app.handle_event(&key_code(KeyCode::Enter));
    assert!(matches!(app.phase, Phase::Workspace));
}

pub(crate) struct RenderedFrame {
    pub(crate) transcript: String,
    raw: String,
}

impl RenderedFrame {
    pub(crate) fn assert_contains_all(&self, needles: &[&str]) {
        for needle in needles {
            assert!(
                self.transcript.contains(needle),
                "semantic transcript does not contain {needle:?}:\n{}",
                self.transcript
            );
        }
    }

    pub(crate) fn assert_contains_in_order(&self, needles: &[&str]) {
        let mut offset = 0;
        for needle in needles {
            let remainder = &self.transcript[offset..];
            let relative = remainder.find(needle).unwrap_or_else(|| {
                panic!(
                    "semantic transcript does not contain {needle:?} after byte {offset}:\n{}",
                    self.transcript
                )
            });
            offset += relative + needle.len();
        }
    }

    pub(crate) fn assert_excludes(&self, needles: &[&str]) {
        for needle in needles {
            assert!(
                !self.transcript.contains(needle),
                "semantic transcript leaked forbidden text {needle:?}:\n{}",
                self.transcript
            );
        }
    }

    pub(crate) fn assert_no_terminal_controls(&self) {
        for (index, c) in self.raw.char_indices() {
            assert_ne!(c, '\u{1b}', "raw ESC at byte {index}");
            assert!(
                c == '\n' || !c.is_control(),
                "raw control character U+{:04X} at byte {index}",
                c as u32
            );
        }
    }
}

pub(crate) fn render_at(app: &App, width: u16, height: u16, now: Instant) -> RenderedFrame {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test backend");
    terminal
        .draw(|frame| app.render(frame, now))
        .expect("render app");
    let buffer = terminal.backend().buffer();
    let mut raw = String::new();
    let mut rows = Vec::with_capacity(height as usize);
    for y in 0..height {
        let mut row = String::new();
        for x in 0..width {
            let cell = buffer.cell((x, y)).expect("cell inside backend bounds");
            row.push_str(cell.symbol());
        }
        raw.push_str(&row);
        raw.push('\n');
        rows.push(row.trim_end().to_string());
    }
    while rows.last().is_some_and(String::is_empty) {
        rows.pop();
    }
    RenderedFrame {
        transcript: rows.join("\n"),
        raw,
    }
}
