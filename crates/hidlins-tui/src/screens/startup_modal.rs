//! Shared startup modal for first-vault onboarding and configured-vault unlock.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Phase, UnlockOrigin, MAX_UNLOCK_ATTEMPTS};

/// The requested decorative mark. Keep these thirteen rows byte-for-byte.
pub(crate) const HIDLINS_STARTUP_ART: &str = r"        .--------.
      .'          '.
     /    .----.    \
    |    |      |    |
.---'----'------'----'---.
| o      HIDLINS      o  |
|                        |
|       HH     HH        |
|       HH     HH        |
|       HHHHHHHHH        |
|       HH     HH        |
| o     HH     HH     o  |
'------------------------'";

const ART_WIDTH: u16 = 26;
const ART_HEIGHT: u16 = 13;
const FULL_MODAL_HEIGHT: u16 = 15;
const FULL_MODAL_MAX_WIDTH: u16 = 76;
const COMPACT_MODAL_MAX_WIDTH: u16 = 56;
const COMPACT_MODAL_MAX_HEIGHT: u16 = 14;

pub(crate) fn render(app: &App, frame: &mut Frame) {
    let area = frame.area();
    let full = app.theme.name != "accessible" && area.width >= 60 && area.height >= 16;
    let (art_area, modal_area) = layout(area, full);

    frame.render_widget(Clear, modal_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Hidlins startup ");
    let form_area = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    if let Some(art_area) = art_area {
        frame.render_widget(Paragraph::new(HIDLINS_STARTUP_ART), art_area);
    }

    match &app.phase {
        Phase::VaultOnboarding { input } => render_onboarding(app, frame, form_area, input.value()),
        Phase::UnlockList => render_vault_list(app, frame, form_area),
        Phase::UnlockPrompt {
            origin,
            input,
            attempts,
        } => render_password(app, frame, form_area, origin, input, *attempts),
        Phase::LockScreen | Phase::Workspace => {}
    }
}

fn layout(area: Rect, full: bool) -> (Option<Rect>, Rect) {
    if full {
        let width = area.width.min(FULL_MODAL_MAX_WIDTH);
        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(FULL_MODAL_HEIGHT) / 2;
        let content = Rect::new(x, y, width, FULL_MODAL_HEIGHT);
        let [art_column, _, modal] = Layout::horizontal([
            Constraint::Length(ART_WIDTH),
            Constraint::Length(1),
            Constraint::Min(33),
        ])
        .areas(content);
        let art = Rect::new(
            art_column.x,
            art_column.y + (art_column.height - ART_HEIGHT) / 2,
            ART_WIDTH,
            ART_HEIGHT,
        );
        (Some(art), modal)
    } else {
        let width = area.width.min(COMPACT_MODAL_MAX_WIDTH);
        let height = area.height.min(COMPACT_MODAL_MAX_HEIGHT);
        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(height) / 2;
        (None, Rect::new(x, y, width, height))
    }
}

fn render_onboarding(app: &App, frame: &mut Frame, area: Rect, path: &str) {
    let [heading, prompt, label, field, status, actions] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Min(2),
    ])
    .areas(area);

    render_heading(app, frame, heading);
    frame.render_widget(Paragraph::new("Please select a vault file:"), prompt);
    frame.render_widget(
        Paragraph::new("Focused field: Vault path").style(app.theme.header()),
        label,
    );
    let value = if path.is_empty() {
        "Path: [type an existing KDBX path]".to_string()
    } else {
        format!("Path: {}", reviewable(path))
    };
    frame.render_widget(Paragraph::new(value), field);
    render_status(app, frame, status, None);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from("Enter: Open vault"),
            Line::from("Esc or Ctrl+Q: Exit"),
        ]),
        actions,
    );
}

fn render_vault_list(app: &App, frame: &mut Frame, area: Rect) {
    let [heading, prompt, list_area, status, actions] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(2),
        Constraint::Length(2),
        Constraint::Length(2),
    ])
    .areas(area);

    render_heading(app, frame, heading);
    frame.render_widget(Paragraph::new("Please select a vault:"), prompt);
    let items: Vec<ListItem> = app
        .registry()
        .list()
        .enumerate()
        .map(|(index, vault)| {
            let name = reviewable(&vault.name);
            let selected = index == app.list_index;
            let text = if selected {
                format!("Selected: {name}")
            } else {
                format!("Vault: {name}")
            };
            let style = if selected {
                app.theme.selected()
            } else {
                Style::default()
            };
            ListItem::new(Line::from(text)).style(style)
        })
        .collect();
    let mut state = ListState::default().with_selected(Some(app.list_index));
    frame.render_stateful_widget(List::new(items), list_area, &mut state);
    render_status(app, frame, status, None);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from("Up/Down or j/k: Select"),
            Line::from("Enter: Continue   Ctrl+Q: Exit"),
        ]),
        actions,
    );
}

fn render_password(
    app: &App,
    frame: &mut Frame,
    area: Rect,
    origin: &UnlockOrigin,
    input: &crate::widgets::password_input::PasswordInput,
    attempts: u8,
) {
    let action_height = 3;
    let [heading, prompt, label, field, status, actions] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Length(action_height),
    ])
    .areas(area);

    render_heading(app, frame, heading);
    frame.render_widget(
        Paragraph::new(format!("Unlock vault: {}", reviewable(origin.vault_name()))),
        prompt,
    );
    frame.render_widget(
        Paragraph::new("Focused field: Master password").style(app.theme.header()),
        label,
    );
    frame.render_widget(Paragraph::new("Password (masked):"), field);
    let prefix_width = u16::try_from("Password (masked): ".chars().count())
        .expect("password label width fits in u16");
    if field.width > prefix_width {
        input.render(
            frame,
            Rect::new(
                field.x + prefix_width,
                field.y,
                field.width - prefix_width,
                field.height,
            ),
            &app.theme,
        );
    }
    render_status(app, frame, status, Some(attempts));

    let back = match origin {
        UnlockOrigin::Direct { .. } => "Esc: Exit",
        UnlockOrigin::VaultList { .. } => "Esc: Back to vault list",
        UnlockOrigin::Onboarding { .. } => "Esc: Back to vault path",
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from("Enter: Unlock"),
            Line::from(back),
            Line::from("Ctrl+Q: Exit"),
        ]),
        actions,
    );
}

fn render_heading(app: &App, frame: &mut Frame, area: Rect) {
    frame.render_widget(Paragraph::new("HIDLINS").style(app.theme.header()), area);
}

fn render_status(app: &App, frame: &mut Frame, area: Rect, attempts: Option<u8>) {
    let text = match attempts {
        Some(attempts) if attempts > 0 => {
            format!("Authentication failed ({attempts}/{MAX_UNLOCK_ATTEMPTS})")
        }
        _ => app
            .status
            .as_deref()
            .map_or_else(|| "Status: Ready".to_string(), reviewable),
    };
    let style = if attempts.is_some_and(|attempts| attempts > 0) {
        app.theme.error()
    } else if app.status.is_some() {
        app.theme.warning()
    } else {
        Style::default()
    };
    frame.render_widget(
        Paragraph::new(text).style(style).wrap(Wrap { trim: false }),
        area,
    );
}

fn reviewable(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_control() {
                '�'
            } else {
                character
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decorative_art_is_thirteen_rows_with_the_declared_geometry() {
        let rows: Vec<_> = HIDLINS_STARTUP_ART.lines().collect();
        assert_eq!(rows.len(), ART_HEIGHT as usize);
        assert_eq!(
            rows.iter().map(|row| row.len()).max(),
            Some(ART_WIDTH as usize)
        );
    }

    #[test]
    fn reviewable_text_removes_terminal_controls_without_changing_normal_text() {
        assert_eq!(reviewable("alpha\u{1b}[31m\u{85}beta"), "alpha�[31m�beta");
        assert_eq!(reviewable("Personal vault"), "Personal vault");
    }
}
