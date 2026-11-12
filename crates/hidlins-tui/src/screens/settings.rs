//! `settings` — the Settings/Sync tab body (T6.3 / T6.4).
//!
//! Two regions: local preferences/actions and a secret-free local-sync status.
//!
//! Key handling lives on `App` (`on_settings_key`); this module only renders.
//! Local role, trust, and sealed identity live in `vaults.toml`; the explicit
//! server request is process-local and is never persisted.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;

/// The editable settings rows, in display order. `App::SETTINGS_ROW_COUNT` must
/// match this slice's length (asserted in `app.rs` tests).
pub(crate) const ROW_LABELS: &[&str] = &[
    "Default sort",
    "Theme",
    "Auto-lock",
    "Configure as sync server",
    "Pair this vault",
    "Import paired vault",
    "Start sync server",
    "Allow pairing for 3 minutes",
    "Manage peers",
];

/// Render the Settings tab into `area`: the editable rows on top, and a
/// secret-free "Status" sub-view below that also acknowledges the two settings
/// configured *outside* this editor (the env-detected theme and per-vault
/// auto-lock) so a user who looks here for them isn't met with a void.
pub(crate) fn render(app: &App, frame: &mut Frame, area: Rect) {
    let [editor_area, status_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(8)]).areas(area);

    render_editor(app, frame, editor_area);
    render_status(app, frame, status_area);
}

fn render_editor(app: &App, frame: &mut Frame, area: Rect) {
    let values = [
        app.user_config.default_sort().label().to_string(),
        app.current_theme_name().to_string(),
        format!("{} min", app.current_auto_lock_seconds() / 60),
        String::new(),
        String::new(),
        String::new(),
        if app.sync_server_running() {
            "running"
        } else {
            "off"
        }
        .to_string(),
        app.pairing_countdown(),
        app.sync_peer_count().to_string(),
    ];

    let visible_rows = usize::from(area.height.saturating_sub(2)).max(1);
    let first = app
        .settings_index
        .saturating_add(1)
        .saturating_sub(visible_rows);
    let items: Vec<ListItem> = ROW_LABELS
        .iter()
        .enumerate()
        .skip(first)
        .map(|(i, label)| {
            let label = if i == 6 && app.sync_server_running() {
                "Stop sync server"
            } else {
                label
            };
            let selected = i == app.settings_index;
            let marker = if selected { "> " } else { "  " };
            let value = &values[i];
            let text = if value.is_empty() {
                format!("{marker}{label}")
            } else {
                format!("{marker}{label}: {value}")
            };
            let style = if selected {
                app.theme.selected()
            } else {
                app.theme.header()
            };
            ListItem::new(Line::from(text)).style(style)
        })
        .collect();

    frame.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(app.theme.border_focused())
                .title("Settings")
                .title_style(app.theme.header()),
        ),
        area,
    );
}

fn render_status(app: &App, frame: &mut Frame, area: Rect) {
    let target = app
        .sync_target_summary()
        .unwrap_or_else(|| "(local sync not configured)".to_string());
    let last = app.sync_status_line().unwrap_or("Last sync: —").to_string();

    let body = vec![
        // App settings that live outside this editor — surfaced read-only with a
        // pointer to where they're changed.
        Line::from(vec![
            Span::styled("Keymap preset: ", app.theme.muted()),
            Span::styled(app.keymap_preset_label(), app.theme.header()),
            Span::styled("  (config.toml [keymap])", app.theme.muted()),
        ]),
        Line::from(vec![
            Span::styled("Config file: ", app.theme.muted()),
            Span::styled(app.config_file_display(), app.theme.header()),
        ]),
        Line::from(""),
        // Sync target + last outcome (secret-free).
        Line::from(vec![
            Span::styled("Target: ", app.theme.muted()),
            Span::styled(target, app.theme.header()),
        ]),
        Line::from(Span::styled(last, app.theme.muted())),
    ];

    frame.render_widget(
        Paragraph::new(body).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(app.theme.border())
                .title("Local sync Status")
                .title_style(app.theme.header()),
        ),
        area,
    );
}
