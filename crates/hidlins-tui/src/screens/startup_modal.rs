//! Shared startup modal for first-vault onboarding and configured-vault unlock.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, OnboardingOrigin, Phase, UnlockOrigin, MAX_UNLOCK_ATTEMPTS};

/// The requested decorative mark. Keep these twelve rows byte-for-byte.
pub(crate) const HIDLINS_STARTUP_ART: &str = r"         ▄▄▄▄▄▄▄
       ▄██▀▀▀▀▀██▄
       ██       ██
       ██       ██
  ╔════██═══════██════╗
  ║  ┌─────────────┐  ║
  ║  │o  HIDLINS  o│  ║
  ║  └─────────────┘  ║
  ║       ▄▄▄▄▄       ║
  ║         █         ║
  ║       ▄▄█▄▄       ║
  ╚═══════════════════╝";

const ART_WIDTH: u16 = 23;
const ART_HEIGHT: u16 = 12;
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
        Phase::VaultOnboarding { input, origin } => {
            render_onboarding(app, frame, form_area, input, *origin);
        }
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

fn render_onboarding(
    app: &App,
    frame: &mut Frame,
    area: Rect,
    input: &tui_input::Input,
    origin: OnboardingOrigin,
) {
    let [heading, prompt, field, status, actions] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Min(3),
    ])
    .areas(area);

    render_heading(app, frame, heading);
    frame.render_widget(Paragraph::new("Please select a vault file:"), prompt);
    render_path_field(app, frame, field, input);
    render_status(app, frame, status, None);
    let action_lines = match origin {
        OnboardingOrigin::FirstRun => vec![
            Line::from("Enter: Open vault"),
            Line::from("F3: Import paired vault"),
            Line::from("Esc or Ctrl+Q: Exit"),
        ],
        OnboardingOrigin::VaultList { .. } => vec![
            Line::from("Enter: Open vault"),
            Line::from("F3: Import paired vault"),
            Line::from("Esc: Back   Ctrl+Q: Exit"),
        ],
    };
    frame.render_widget(Paragraph::new(action_lines), actions);
}

fn render_vault_list(app: &App, frame: &mut Frame, area: Rect) {
    let [heading, prompt, list_area, status, actions] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(2),
        Constraint::Length(2),
        Constraint::Length(3),
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
                format!("▶ {name}")
            } else {
                format!("  {name}")
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
            Line::from("j/k: Select  Enter: Open"),
            Line::from("a: Add existing vault"),
            Line::from("F3: Import paired vault  Ctrl+Q: Exit"),
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
    let [heading, prompt, field, status, actions] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Length(action_height),
    ])
    .areas(area);

    render_heading(app, frame, heading);
    frame.render_widget(
        Paragraph::new(format!("Unlock vault: {}", reviewable(origin.vault_name()))),
        prompt,
    );
    render_password_field(app, frame, field, input);
    render_status(app, frame, status, Some(attempts));

    let back = match origin {
        UnlockOrigin::Direct { .. } => "Esc: Choose a vault",
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

fn render_path_field(app: &App, frame: &mut Frame, area: Rect, input: &tui_input::Input) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" ▶ Vault path — active ")
        .border_style(app.theme.selected());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 {
        return;
    }

    let scroll = input.visual_scroll(inner.width as usize);
    let value = if input.value().is_empty() {
        "[type an existing KDBX path]".to_string()
    } else {
        reviewable(input.value())
    };
    frame.render_widget(
        Paragraph::new(value).scroll((0, u16::try_from(scroll).unwrap_or(u16::MAX))),
        inner,
    );
    let cursor = input
        .visual_cursor()
        .saturating_sub(scroll)
        .min(inner.width.saturating_sub(1) as usize);
    frame.set_cursor_position((inner.x + u16::try_from(cursor).unwrap_or(0), inner.y));
}

fn render_password_field(
    app: &App,
    frame: &mut Frame,
    area: Rect,
    input: &crate::widgets::password_input::PasswordInput,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" ▶ Master password — active ")
        .border_style(app.theme.selected());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 {
        return;
    }
    input.render(frame, inner, &app.theme);
    let cursor = input
        .cursor_chars()
        .min(inner.width.saturating_sub(1) as usize);
    frame.set_cursor_position((inner.x + u16::try_from(cursor).unwrap_or(0), inner.y));
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
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use unicode_width::UnicodeWidthStr;

    use crate::test_support::{configured_app, onboarding_app, type_text};

    #[test]
    fn decorative_art_is_twelve_rows_with_the_declared_display_geometry() {
        let rows: Vec<_> = HIDLINS_STARTUP_ART.lines().collect();
        assert_eq!(rows.len(), ART_HEIGHT as usize);
        assert_eq!(
            rows.iter().map(|row| UnicodeWidthStr::width(*row)).max(),
            Some(ART_WIDTH as usize)
        );
    }

    #[test]
    fn reviewable_text_removes_terminal_controls_without_changing_normal_text() {
        assert_eq!(reviewable("alpha\u{1b}[31m\u{85}beta"), "alpha�[31m�beta");
        assert_eq!(reviewable("Personal vault"), "Personal vault");
    }

    #[test]
    fn active_startup_fields_move_the_terminal_cursor_with_typed_input() {
        let (_dir, mut onboarding, _) = onboarding_app();
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
        terminal
            .draw(|frame| onboarding.render(frame, std::time::Instant::now()))
            .unwrap();
        let path_start = terminal.get_cursor_position().unwrap();
        type_text(&mut onboarding, "abc");
        terminal
            .draw(|frame| onboarding.render(frame, std::time::Instant::now()))
            .unwrap();
        let path_typed = terminal.get_cursor_position().unwrap();
        assert_eq!(path_typed.y, path_start.y);
        assert_eq!(path_typed.x, path_start.x + 3);

        let (_dir, mut configured) = configured_app(&["personal"]);
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
        terminal
            .draw(|frame| configured.render(frame, std::time::Instant::now()))
            .unwrap();
        let password_start = terminal.get_cursor_position().unwrap();
        type_text(&mut configured, "abc");
        terminal
            .draw(|frame| configured.render(frame, std::time::Instant::now()))
            .unwrap();
        let password_typed = terminal.get_cursor_position().unwrap();
        assert_eq!(password_typed.y, password_start.y);
        assert_eq!(password_typed.x, password_start.x + 3);
    }
}
