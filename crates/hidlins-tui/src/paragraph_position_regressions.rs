//! Real-render oracles for the borrowed exact wrapped-span position query.
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};

const MARKER: Color = Color::Rgb(11, 123, 237);

fn caret_oracle(
    lines: Vec<Line<'static>>,
    line: usize,
    span: usize,
    byte: usize,
    width: u16,
    trim: bool,
) {
    let source = lines[line].spans[span].content.as_ref();
    let ordinal = source
        .match_indices('▏')
        .take_while(|(offset, _)| *offset <= byte)
        .count()
        - 1;
    let query = Paragraph::new(lines.clone()).wrap(Wrap { trim });
    let found = query.wrapped_row_for_span(width, line, span, byte);
    let mut oracle_lines = lines;
    for row in &mut oracle_lines {
        for part in &mut row.spans {
            part.style.bg = Some(Color::Reset);
        }
    }
    oracle_lines[line].spans[span].style.bg = Some(MARKER);
    let mut buffer = Buffer::empty(Rect::new(0, 0, width, 256));
    Paragraph::new(oracle_lines)
        .wrap(Wrap { trim })
        .render(buffer.area, &mut buffer);
    let actual = buffer
        .area
        .positions()
        .filter(|position| {
            let cell = &buffer[(position.x, position.y)];
            cell.bg == MARKER && cell.symbol().contains('▏')
        })
        .nth(ordinal)
        .map(|position| usize::from(position.y));
    assert!(
        actual.is_some(),
        "bounded synthetic oracle must actually paint the requested caret"
    );
    assert_eq!(
        found, actual,
        "exact actual row width={width} trim={trim} byte={byte}"
    );
}

#[test]
fn words_unicode_literal_carets_and_combining_bytes_match_actual_painted_rows() {
    for text in [
        "ab ▏ longwordmore abc",
        "aaaaaaaaaa▏bbbbbbbbbb",
        "漢字 é\u{301} 🔒 ▏\u{301} tail",
        "user▏ previous literal verylongword middle▏later",
    ] {
        for (offset, _) in text.match_indices('▏') {
            for width in 1..=20 {
                for trim in [false, true] {
                    let lines = vec![Line::from(vec![Span::raw(text)])];
                    caret_oracle(lines.clone(), 0, 0, offset, width, trim);
                    caret_oracle(lines, 0, 0, offset + 1, width, trim);
                }
            }
        }
    }
    let text = "prefix ▏\u{301} suffix";
    for width in 1..=12 {
        caret_oracle(
            vec![Line::from(text)],
            0,
            0,
            text.find('▏').unwrap() + 3,
            width,
            false,
        );
    }
}

#[test]
fn duplicate_static_spans_lines_and_original_styles_do_not_confuse_identity() {
    for width in 1..=10 {
        let reversed = Style::default().add_modifier(Modifier::REVERSED);
        let lines = vec![
            Line::from(vec![
                Span::styled("▏", reversed),
                Span::raw(" verylongword "),
                Span::styled("▏", reversed),
            ]),
            Line::from(vec![
                Span::styled("▏", reversed),
                Span::styled("▏", reversed),
            ]),
        ];
        for (line, span) in [(0, 0), (0, 2), (1, 0), (1, 1)] {
            caret_oracle(lines.clone(), line, span, 0, width, false);
        }
    }
}

#[test]
fn whitespace_boundaries_use_the_actual_emitted_or_discarded_cell() {
    for width in 1..=8 {
        for trim in [false, true] {
            for (prefix, suffix) in [("a", "b"), ("aaaa", "b"), ("", "tail"), ("abcdefg", "")] {
                let lines = vec![Line::from(vec![
                    Span::raw(prefix),
                    Span::raw(" "),
                    Span::raw(suffix),
                ])];
                let query = Paragraph::new(lines.clone()).wrap(Wrap { trim });
                let mut oracle = lines;
                oracle[0].spans[1].style.bg = Some(MARKER);
                let mut buffer = Buffer::empty(Rect::new(0, 0, width, 32));
                Paragraph::new(oracle)
                    .wrap(Wrap { trim })
                    .render(buffer.area, &mut buffer);
                let actual = buffer
                    .area
                    .positions()
                    .find(|p| buffer[(p.x, p.y)].bg == MARKER)
                    .map(|p| usize::from(p.y));
                assert_eq!(
                    query.wrapped_row_for_span(width, 0, 1, 0),
                    actual,
                    "width{width} trim{trim} prefix{prefix}"
                );
            }
        }
    }
}

#[test]
fn filtered_zero_width_overwide_empty_and_invalid_targets_are_explicit_none() {
    for text in ["\t", "\u{301}", "漢", ""] {
        let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
        assert_eq!(paragraph.wrapped_row_for_span(1, 0, 0, 0), None);
    }
    let paragraph = Paragraph::new("abc").wrap(Wrap { trim: false });
    for (width, line, span, byte) in [
        (0, 0, 0, 0),
        (2, 1, 0, 0),
        (2, 0, 1, 0),
        (2, 0, 0, 3),
        (2, 0, 0, usize::MAX),
    ] {
        assert_eq!(
            paragraph.wrapped_row_for_span(width, line, span, byte),
            None
        );
    }
    assert_eq!(Paragraph::new("a").wrapped_row_for_span(1, 0, 0, 0), None);
}

#[test]
#[ignore = "serial release editor measurement: make bench-tui-edit"]
fn editor_render_budget() {
    use crate::overlay::edit::{CustomRow, EditField, EditState};
    use crate::widgets::password_input::PasswordInput;
    use std::time::{Duration, Instant};
    let mut state = EditState::new_add(uuid::Uuid::nil());
    state.title = tui_input::Input::new("Synthetic editor workload".to_string());
    state.notes = crate::widgets::text_area::TextArea::from_str(&format!(
        "{}{}",
        "logical note\n".repeat(80),
        "word é🔒 ".repeat(600)
    ));
    for index in 0..20 {
        state.custom.push(CustomRow {
            name: tui_input::Input::new(format!("field-{index}")),
            value: PasswordInput::with_value("synthetic-value"),
            protected: true,
        });
    }
    state.error = Some("Synthetic validation message".to_string());
    let theme =
        crate::theme::Theme::from_env_parts(None, false, Some("xterm-256color"), Some("truecolor"));
    let keys = crate::command::Keymap::preset(crate::command::Preset::Vim);
    let mut terminal = ratatui::Terminal::with_options(
        ratatui::backend::CrosstermBackend::new(std::io::sink()),
        ratatui::TerminalOptions {
            viewport: ratatui::Viewport::Fixed(Rect::new(0, 0, 100, 30)),
        },
    )
    .unwrap();
    for target in [EditField::Notes, EditField::CustomAdd] {
        state.focus = state
            .fields()
            .iter()
            .position(|field| *field == target)
            .unwrap();
        let mut samples = Vec::with_capacity(100);
        for index in 0..120 {
            let before = Instant::now();
            terminal
                .draw(|frame| crate::overlay::edit::render(&state, frame, &theme, &keys))
                .unwrap();
            if index >= 20 {
                samples.push(before.elapsed());
            }
        }
        samples.sort_unstable();
        let p95 = samples[94];
        eprintln!(
            "tui-edit target={target:?} samples=100 median_us={} p95_us={}",
            samples[49].as_micros(),
            p95.as_micros()
        );
        assert!(
            p95 < Duration::from_millis(50),
            "retain existing 20-FPS frame budget"
        );
    }
}
