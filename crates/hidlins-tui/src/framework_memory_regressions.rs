//! Actual Ratatui ownership and rendering checks, using synthetic payloads.

use std::time::{Duration, Instant};

use hidlins_core::Zeroizing;
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::Rect;
use ratatui::text::Span;
use ratatui::widgets::{Paragraph, Widget, Wrap};
use ratatui::{Terminal, TerminalOptions, Viewport};

use crate::allocation_observer::DeallocationWatch;

fn long_cluster() -> Zeroizing<String> {
    Zeroizing::new(format!("x{}", "\u{301}".repeat(32)))
}

fn heap_cell() -> Cell {
    let text = long_cluster();
    let mut cell = Cell::EMPTY;
    cell.set_symbol(text.as_str());
    assert_eq!(cell.symbol(), text.as_str());
    cell
}

#[test]
fn heap_symbol_drop_erases_before_release() {
    let cell = heap_cell();
    let watch = DeallocationWatch::new(cell.symbol().as_bytes());
    drop(cell);
    watch.assert_erased();
}

#[test]
fn heap_symbol_reset_erases_before_release() {
    let mut cell = heap_cell();
    let watch = DeallocationWatch::new(cell.symbol().as_bytes());
    cell.reset();
    assert_eq!(cell.symbol(), " ");
    watch.assert_erased();
}

#[test]
fn heap_symbol_replacement_erases_before_release() {
    let mut cell = heap_cell();
    let watch = DeallocationWatch::new(cell.symbol().as_bytes());
    cell.set_symbol("replacement");
    assert_eq!(cell.symbol(), "replacement");
    watch.assert_erased();
}

#[test]
fn cloned_heap_symbol_erases_its_own_allocation() {
    let cell = heap_cell();
    let cloned = cell.clone();
    assert_ne!(cell.symbol().as_ptr(), cloned.symbol().as_ptr());
    let watch = DeallocationWatch::new(cloned.symbol().as_bytes());
    drop(cloned);
    watch.assert_erased();
    assert_eq!(cell.symbol(), long_cluster().as_str());
}

#[test]
fn shrinking_buffer_erases_removed_heap_symbol() {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
    buffer[(0, 0)] = heap_cell();
    let watch = DeallocationWatch::new(buffer[(0, 0)].symbol().as_bytes());
    buffer.resize(Rect::new(0, 0, 0, 0));
    assert!(buffer.content.is_empty());
    watch.assert_erased();
}

#[test]
fn grown_buffer_preserves_and_then_erases_heap_symbol() {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
    buffer[(0, 0)] = heap_cell();
    let watch = DeallocationWatch::new(buffer[(0, 0)].symbol().as_bytes());
    buffer.resize(Rect::new(0, 0, 16, 2));
    assert_eq!(buffer[(0, 0)].symbol(), long_cluster().as_str());
    drop(buffer);
    watch.assert_erased();
}

#[test]
fn merged_buffer_preserves_and_then_erases_heap_symbol() {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
    buffer[(0, 0)] = heap_cell();
    let mut other = Buffer::empty(Rect::new(1, 0, 1, 1));
    other[(1, 0)].set_symbol("other");
    let watch = DeallocationWatch::new(buffer[(0, 0)].symbol().as_bytes());
    buffer.merge(&other);
    assert_eq!(buffer[(0, 0)].symbol(), long_cluster().as_str());
    assert_eq!(buffer[(1, 0)].symbol(), "other");
    drop(buffer);
    watch.assert_erased();
}

#[test]
fn const_cell_and_unicode_render_contracts_remain_available() {
    const CELL: Cell = Cell::new("é");
    const EMPTY: Cell = Cell::EMPTY;
    assert_eq!(CELL.symbol(), "é");
    assert_eq!(EMPTY.symbol(), " ");
    let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 1));
    Span::raw("aé🔒").render(buffer.area, &mut buffer);
    assert_eq!(buffer[(0, 0)].symbol(), "a");
    assert_eq!(buffer[(1, 0)].symbol(), "é");
    assert_eq!(buffer[(2, 0)].symbol(), "🔒");
}

fn percentile(samples: &mut [Duration], percentage: usize) -> Duration {
    samples.sort_unstable();
    samples[(samples.len() - 1) * percentage / 100]
}

#[test]
#[ignore = "serial release measurement: make bench-tui-render"]
fn render_workload_performance_baseline() {
    let text = Zeroizing::new("synthetic secret note é🔒 ".repeat(240));
    let mut terminal = Terminal::with_options(
        CrosstermBackend::new(std::io::sink()),
        TerminalOptions {
            viewport: Viewport::Fixed(Rect::new(0, 0, 120, 40)),
        },
    )
    .unwrap();
    for redraw in [true, false] {
        let mut samples = Vec::with_capacity(400);
        for index in 0..420 {
            let before = Instant::now();
            if redraw {
                terminal.clear().unwrap();
            }
            terminal
                .draw(|frame| {
                    frame.render_widget(
                        Paragraph::new(text.as_str()).wrap(Wrap { trim: false }),
                        frame.area(),
                    );
                })
                .unwrap();
            if index >= 20 {
                samples.push(before.elapsed());
            }
        }
        let median = percentile(&mut samples, 50);
        let p95 = percentile(&mut samples, 95);
        println!(
            "tui-render redraw={redraw} size=120x40 samples={} median_us={} p95_us={}",
            samples.len(),
            median.as_micros(),
            p95.as_micros()
        );
        assert!(
            p95 < Duration::from_millis(50),
            "retain existing 20-FPS render budget: {p95:?}"
        );
    }
}
