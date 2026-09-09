//! Secret-safe local-network sync prompts.

use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use tui_input::Input;

use crate::overlay::{centered, clear};
use crate::theme::Theme;
use crate::widgets::password_input::PasswordInput;

pub(crate) const HINTS: &str = "Tab: next field   Enter: continue   Esc: cancel";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalSyncAction {
    StartServer,
    PairExisting,
    Import,
}

pub(crate) struct LocalSyncState {
    pub(crate) action: LocalSyncAction,
    pub(crate) name: Input,
    pub(crate) peer_name: Input,
    pub(crate) master: PasswordInput,
    pub(crate) focused: usize,
    pub(crate) error: Option<String>,
}

impl LocalSyncState {
    pub(crate) fn new(action: LocalSyncAction) -> Self {
        Self {
            action,
            name: Input::default(),
            peer_name: Input::new("Nearby Hidlins".to_string()),
            master: PasswordInput::new(),
            focused: 0,
            error: None,
        }
    }

    pub(crate) fn field_count(&self) -> usize {
        match self.action {
            LocalSyncAction::StartServer => 1,
            LocalSyncAction::PairExisting => 2,
            LocalSyncAction::Import => 3,
        }
    }

    pub(crate) fn next(&mut self) {
        self.focused = (self.focused + 1) % self.field_count();
    }

    pub(crate) fn previous(&mut self) {
        self.focused = (self.focused + self.field_count() - 1) % self.field_count();
    }
}

pub(crate) fn render(state: &LocalSyncState, frame: &mut Frame, theme: &Theme) {
    let (title, subtitle) = match state.action {
        LocalSyncAction::StartServer => (
            "Start sync server",
            "Runs only while this TUI is open. Private and link-local interfaces only.",
        ),
        LocalSyncAction::PairExisting => (
            "Pair existing vault",
            "Secure discovery finds a nearby server; compare the SAS on both devices.",
        ),
        LocalSyncAction::Import => (
            "Import paired vault",
            "Creates a local encrypted vault only after bilateral SAS confirmation.",
        ),
    };
    let height = match state.action {
        LocalSyncAction::StartServer => 9,
        LocalSyncAction::PairExisting => 11,
        LocalSyncAction::Import => 13,
    };
    let area = centered(frame, 70, height);
    clear(frame, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_style(theme.header());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);
    frame.render_widget(Paragraph::new(subtitle).style(theme.muted()), rows[0]);

    let mut lines = Vec::new();
    if state.action == LocalSyncAction::Import {
        lines.push(Line::from(vec![
            Span::styled(
                if state.focused == 0 { "> " } else { "  " },
                theme.selected(),
            ),
            Span::raw("Vault name: "),
            Span::raw(state.name.value()),
        ]));
    }
    if state.action != LocalSyncAction::StartServer {
        let index = usize::from(state.action == LocalSyncAction::Import);
        lines.push(Line::from(vec![
            Span::styled(
                if state.focused == index { "> " } else { "  " },
                theme.selected(),
            ),
            Span::raw("Peer name: "),
            Span::raw(state.peer_name.value()),
        ]));
    }
    let master_index = state.field_count() - 1;
    lines.push(Line::from(vec![
        Span::styled(
            if state.focused == master_index {
                "> "
            } else {
                "  "
            },
            theme.selected(),
        ),
        Span::raw("Master password: "),
        Span::raw("*".repeat(state.master.len_chars())),
    ]));
    frame.render_widget(Paragraph::new(lines), rows[1]);
    if let Some(error) = state.error.as_deref() {
        frame.render_widget(Paragraph::new(error).style(theme.error()), rows[2]);
    }
    frame.render_widget(Paragraph::new(HINTS).style(theme.muted()), rows[3]);
}

pub(crate) struct SasState {
    pub(crate) code: String,
    pub(crate) peer_name: String,
    pub(crate) error: Option<String>,
}

pub(crate) fn render_sas(state: &SasState, frame: &mut Frame, theme: &Theme) {
    let area = centered(frame, 62, 10);
    clear(frame, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Confirm pairing")
        .title_style(theme.header());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let body = vec![
        Line::from("Compare this six-digit SAS on both devices:"),
        Line::from(""),
        Line::from(Span::styled(&state.code, theme.selected())),
        Line::from(""),
        Line::from(format!("Peer label: {}", state.peer_name)),
        Line::from("y: codes match and pair   n / Esc: reject"),
        Line::from(state.error.as_deref().unwrap_or("")),
    ];
    frame.render_widget(Paragraph::new(body), inner);
}

pub(crate) struct PeersState {
    pub(crate) names: Vec<String>,
    pub(crate) selected: usize,
}

pub(crate) fn render_peers(state: &PeersState, frame: &mut Frame, theme: &Theme) {
    let area = centered(frame, 62, 12);
    clear(frame, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Sync peers")
        .title_style(theme.header());
    let inner = block.inner(area);
    let mut lines = vec![Line::from("Peer identifiers are intentionally opaque.")];
    if state.names.is_empty() {
        lines.push(Line::from("No paired peers."));
    } else {
        lines.extend(state.names.iter().enumerate().map(|(index, name)| {
            Line::from(format!(
                "{} peer-{index}: {name}",
                if state.selected == index { ">" } else { " " }
            ))
        }));
    }
    lines.push(Line::from("↑/↓: select   r: Revoke peer   Esc: close"));
    frame.render_widget(Paragraph::new(lines), inner);
}
