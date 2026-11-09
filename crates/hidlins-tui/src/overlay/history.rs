//! `Overlay::History` — a read-only viewer for an entry's KDBX history (T5.4).
//!
//! When opened, the App snapshots the selected entry's `history()` into owned
//! [`HistorySnapshot`]s (so the overlay holds no `Vault` borrow). Passwords are
//! **not** stored in the clear — only a "set / empty" indicator — keeping the
//! viewer free of password copies. Notes can still contain sensitive content
//! and retain zeroizing ownership. `↑`/`↓` browse versions; `PgUp`/`PgDn`
//! page the selected version's detail; `Esc` closes.

use std::cell::Cell;

use hidlins_core::{HistoryView, Zeroizing};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use super::{centered, clear};
use crate::theme::Theme;

/// One historical version, with sensitive notes erased when the snapshot drops.
pub(crate) struct HistorySnapshot {
    pub(crate) modified: String,
    pub(crate) title: String,
    pub(crate) username: String,
    pub(crate) url: String,
    pub(crate) notes: Zeroizing<String>,
    pub(crate) has_password: bool,
}

/// History-overlay state: the live entry's title, its version snapshots
/// (newest first), and the highlighted version.
pub(crate) struct HistoryState {
    pub(crate) entry_title: String,
    pub(crate) snapshots: Vec<HistorySnapshot>,
    pub(crate) selected: usize,
    detail_scroll: u16,
    detail_viewport: Cell<Rect>,
}

impl HistoryState {
    /// Snapshot the history views, retaining the core's newest-first order.
    pub(crate) fn from_views(entry_title: String, views: &[HistoryView<'_>]) -> Self {
        let snapshots: Vec<HistorySnapshot> = views
            .iter()
            .map(|v| HistorySnapshot {
                modified: v
                    .last_modification_time()
                    .map_or_else(|| "(unknown)".to_string(), crate::util::format_ts),
                title: v.title().to_string(),
                username: v.username().to_string(),
                url: v.url().to_string(),
                notes: Zeroizing::new(v.notes().to_string()),
                has_password: !v.password().is_empty(),
            })
            .collect();
        Self {
            entry_title,
            snapshots,
            selected: 0,
            detail_scroll: 0,
            detail_viewport: Cell::new(Rect::new(0, 0, u16::MAX, 0)),
        }
    }

    pub(crate) fn select_next(&mut self) {
        if !self.snapshots.is_empty() {
            let next = self
                .selected
                .saturating_add(1)
                .min(self.snapshots.len() - 1);
            if next != self.selected {
                self.detail_scroll = 0;
                self.selected = next;
            }
        }
    }

    pub(crate) fn select_prev(&mut self) {
        let previous = self.selected.saturating_sub(1);
        if previous != self.selected {
            self.detail_scroll = 0;
            self.selected = previous;
        }
    }

    pub(crate) fn page_detail(&mut self, down: bool) {
        let Some(snapshot) = self.snapshots.get(self.selected) else {
            return;
        };
        let viewport = self.detail_viewport.get();
        let paragraph = detail_paragraph(snapshot);
        let limit = scroll_limit(&paragraph, viewport);
        let current = self.detail_scroll.min(limit);
        let page = viewport.height.max(1);
        self.detail_scroll = if down {
            current.saturating_add(page).min(limit)
        } else {
            current.saturating_sub(page)
        };
    }
}

pub(crate) fn render(state: &HistoryState, frame: &mut Frame, theme: &Theme) {
    let area = centered(frame, 70, 18);
    clear(frame, area);

    let title = if state.entry_title.is_empty() {
        "History".to_string()
    } else {
        format!("History — {}", state.entry_title)
    };
    let outer = Block::default().borders(Borders::ALL).title(title);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    if state.snapshots.is_empty() {
        frame.render_widget(
            Paragraph::new("No prior versions.").style(theme.muted()),
            inner,
        );
        return;
    }

    let [list_area, detail_area] =
        Layout::horizontal([Constraint::Percentage(40), Constraint::Min(1)]).areas(inner);
    state.detail_viewport.set(detail_area);

    let items: Vec<ListItem> = state
        .snapshots
        .iter()
        .map(|s| ListItem::new(Line::from(s.modified.clone())))
        .collect();
    let list = List::new(items)
        .block(Block::default().borders(Borders::RIGHT).title("Versions"))
        .highlight_style(theme.selected())
        .highlight_symbol("> ");
    let mut list_state = ListState::default();
    list_state.select(Some(state.selected));
    frame.render_stateful_widget(list, list_area, &mut list_state);

    let paragraph = detail_paragraph(&state.snapshots[state.selected]);
    let scroll = state
        .detail_scroll
        .min(scroll_limit(&paragraph, detail_area));
    frame.render_widget(paragraph.scroll((scroll, 0)), detail_area);
}

fn scroll_limit(paragraph: &Paragraph<'_>, viewport: Rect) -> u16 {
    u16::try_from(
        paragraph
            .line_count(viewport.width)
            .saturating_sub(usize::from(viewport.height)),
    )
    .unwrap_or(u16::MAX)
}

fn detail_paragraph(snap: &HistorySnapshot) -> Paragraph<'_> {
    let mut lines: Vec<Line> = vec![
        Line::from(format!("Modified: {}", snap.modified)),
        Line::from(format!("Title: {}", snap.title)),
    ];
    if !snap.username.is_empty() {
        lines.push(Line::from(format!("Username: {}", snap.username)));
    }
    if snap.has_password {
        lines.push(Line::from("Password: •••••••• (set)".to_string()));
    }
    if !snap.url.is_empty() {
        lines.push(Line::from(format!("URL: {}", snap.url)));
    }
    if !snap.notes.is_empty() {
        lines.push(Line::from("Notes:".to_string()));
        for nl in snap.notes.lines() {
            lines.push(Line::from(vec![Span::raw("  "), Span::raw(nl)]));
        }
    }
    #[cfg(test)]
    crate::render_memory_regressions::capture("history", &lines);
    Paragraph::new(lines).wrap(Wrap { trim: false })
}
