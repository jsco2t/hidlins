//! Observe initialized bytes while their allocation is still owned by the allocator.
//! This is isolated to regression-test executables, never production code.

#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

struct ObservingAllocator;

#[global_allocator]
static ALLOCATOR: ObservingAllocator = ObservingAllocator;
static WATCHED: AtomicPtr<u8> = AtomicPtr::new(ptr::null_mut());
static LENGTH: AtomicUsize = AtomicUsize::new(0);
static OFFSET: AtomicUsize = AtomicUsize::new(0);
static NEXT_SIZE: AtomicUsize = AtomicUsize::new(0);
static DEALLOCATED: AtomicBool = AtomicBool::new(false);
static ERASED: AtomicBool = AtomicBool::new(false);
static SERIAL: Mutex<()> = Mutex::new(());

thread_local! {
    static ARM_NEXT: Cell<bool> = const { Cell::new(false) };
}

// SAFETY: allocation requests are forwarded unchanged to System. The observer
// reads only the initialized prefix captured from a live owned String/Vec,
// inside dealloc and BEFORE System releases that same allocation. Atomics and
// the watch guard serialize observations; the allocator never allocates or panics.
unsafe impl GlobalAlloc for ObservingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the GlobalAlloc caller supplied this valid layout.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null()
            && layout.size() == NEXT_SIZE.load(Ordering::Acquire)
            && ARM_NEXT
                .try_with(|armed| armed.replace(false))
                .unwrap_or(false)
        {
            WATCHED.store(pointer, Ordering::Release);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if WATCHED
            .compare_exchange(
                pointer,
                ptr::null_mut(),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            let length = LENGTH.load(Ordering::Acquire);
            let offset = OFFSET.load(Ordering::Acquire);
            if offset
                .checked_add(length)
                .is_some_and(|end| end <= layout.size())
            {
                // SAFETY: the watch captured an initialized symbol subrange of
                // this allocation (or the explicit next-allocation contract).
                // Read only before System frees it, never padding/spare bytes.
                let bytes = unsafe { std::slice::from_raw_parts(pointer.add(offset), length) };
                ERASED.store(bytes.iter().all(|byte| *byte == 0), Ordering::Release);
            }
            DEALLOCATED.store(true, Ordering::Release);
        }
        // SAFETY: the original pointer/layout pair is forwarded exactly once.
        unsafe { System.dealloc(pointer, layout) }
    }
}

pub struct DeallocationWatch {
    _serial: MutexGuard<'static, ()>,
}

impl DeallocationWatch {
    /// The slice must cover initialized bytes at the base of an owned buffer.
    /// Observations inspect it only during its actual deallocation callback.
    pub fn new(bytes: &[u8]) -> Self {
        assert!(
            !bytes.is_empty(),
            "the fixture must own a nonempty allocation"
        );
        let serial = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        LENGTH.store(bytes.len(), Ordering::Release);
        OFFSET.store(0, Ordering::Release);
        DEALLOCATED.store(false, Ordering::Release);
        ERASED.store(false, Ordering::Release);
        WATCHED.store(bytes.as_ptr().cast_mut(), Ordering::Release);
        Self { _serial: serial }
    }

    /// Watch an initialized inline symbol inside a live Cell vector allocation.
    /// A non-base slice cannot match dealloc and fails the release assertion.
    pub fn in_cells(cells: &[ratatui::buffer::Cell], index: usize) -> Self {
        let bytes = cells[index].symbol().as_bytes();
        let base = cells.as_ptr().cast::<u8>();
        let offset = (bytes.as_ptr() as usize)
            .checked_sub(base as usize)
            .unwrap();
        assert!(!bytes.is_empty());
        assert!(offset.checked_add(bytes.len()).unwrap() <= std::mem::size_of_val(cells));
        let watch = Self::new(bytes);
        OFFSET.store(offset, Ordering::Release);
        WATCHED.store(base.cast_mut(), Ordering::Release);
        watch
    }

    /// Watch the next matching allocation on this thread.
    ///
    /// # Safety
    /// The caller must guarantee its first `initialized` bytes are initialized
    /// before any release, and that the intended allocation is the next one
    /// with this exact size. Used only for an inspected `CompactString` append.
    unsafe fn next(size: usize, initialized: usize) -> Self {
        assert!(initialized > 0 && initialized <= size);
        let serial = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        LENGTH.store(initialized, Ordering::Release);
        OFFSET.store(0, Ordering::Release);
        DEALLOCATED.store(false, Ordering::Release);
        ERASED.store(false, Ordering::Release);
        WATCHED.store(ptr::null_mut(), Ordering::Release);
        NEXT_SIZE.store(size, Ordering::Release);
        ARM_NEXT.with(|armed| armed.set(true));
        Self { _serial: serial }
    }

    pub fn assert_erased(self) {
        assert!(
            DEALLOCATED.load(Ordering::Acquire),
            "fixture owner must free its buffer"
        );
        assert!(
            ERASED.load(Ordering::Acquire),
            "sensitive bytes must be erased before deallocation"
        );
        drop(self);
    }
}

impl Drop for DeallocationWatch {
    fn drop(&mut self) {
        ARM_NEXT.with(|armed| armed.set(false));
        NEXT_SIZE.store(0, Ordering::Release);
        WATCHED.store(ptr::null_mut(), Ordering::Release);
    }
}

#[cfg(test)]
mod framework_memory_regressions {
    use super::DeallocationWatch;
    use hidlins_core::Zeroizing;
    use ratatui::backend::CrosstermBackend;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::text::Span;
    use ratatui::widgets::Widget;
    use ratatui::{Terminal, TerminalOptions, Viewport};

    fn inline_buffer() -> Buffer {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        buffer[(0, 0)].set_symbol("é");
        buffer
    }

    #[test]
    fn inline_symbol_erased_when_vector_drops() {
        let buffer = inline_buffer();
        let watch = DeallocationWatch::in_cells(&buffer.content, 0);
        drop(buffer);
        watch.assert_erased();
    }

    #[test]
    fn inline_reset_preserves_empty_cell_contract() {
        let mut buffer = inline_buffer();
        // Reset installs Option::None. Its inactive payload is not guaranteed
        // initialized, so never inspect the former inline bytes afterward.
        // Erasure ownership is covered by direct inline Drop and heap reset.
        buffer.reset();
        assert_eq!(buffer[(0, 0)].symbol(), " ");
    }

    #[test]
    fn inline_clone_erases_independent_owner() {
        let buffer = inline_buffer();
        let cloned = buffer.clone();
        assert_ne!(buffer.content.as_ptr(), cloned.content.as_ptr());
        let watch = DeallocationWatch::in_cells(&cloned.content, 0);
        drop(cloned);
        watch.assert_erased();
        assert_eq!(buffer[(0, 0)].symbol(), "é");
    }

    #[test]
    fn inline_growth_erases_old_vector_before_release() {
        let mut buffer = inline_buffer();
        let watch = DeallocationWatch::in_cells(&buffer.content, 0);
        buffer.resize(Rect::new(0, 0, 64, 2));
        assert_eq!(buffer[(0, 0)].symbol(), "é");
        watch.assert_erased();
    }

    #[test]
    fn inline_merge_erases_old_vector_before_release() {
        let mut buffer = inline_buffer();
        let other = Buffer::with_lines(["other"]);
        let watch = DeallocationWatch::in_cells(&buffer.content, 0);
        buffer.merge(&other);
        assert_eq!(buffer[(0, 0)].symbol(), "o");
        watch.assert_erased();
    }

    #[test]
    fn inline_shrink_erases_removed_symbol_before_vector_release() {
        let mut buffer = inline_buffer();
        let watch = DeallocationWatch::in_cells(&buffer.content, 0);
        buffer.resize(Rect::new(0, 0, 0, 0));
        drop(buffer);
        watch.assert_erased();
    }

    #[test]
    fn zero_width_append_erases_replaced_heap_allocation() {
        // One 65-byte grapheme then a separate zero-width space exercises
        // Span::render -> Cell::append_symbol and forces 65 -> 68 byte growth.
        let text = Zeroizing::new(format!("x{}\u{200b}", "\u{301}".repeat(32)));
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        // SAFETY: inspected CompactString::new allocates exactly 65 bytes for
        // this first grapheme and initializes all 65 before append can release
        // it. Span rendering performs no other allocation of that size.
        let watch = unsafe { DeallocationWatch::next(65, 65) };
        Span::raw(text.as_str()).render(buffer.area, &mut buffer);
        assert_eq!(buffer[(0, 0)].symbol(), text.as_str());
        watch.assert_erased();
    }

    fn terminal() -> Terminal<CrosstermBackend<std::io::Sink>> {
        Terminal::with_options(
            CrosstermBackend::new(std::io::sink()),
            TerminalOptions {
                viewport: Viewport::Fixed(Rect::new(0, 0, 2, 1)),
            },
        )
        .unwrap()
    }

    #[test]
    fn actual_terminal_clear_preserves_empty_display_contract() {
        let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(2, 1)).unwrap();
        let frame = terminal
            .draw(|frame| frame.render_widget("é", frame.area()))
            .unwrap();
        assert_eq!(frame.buffer[(0, 0)].symbol(), "é");
        // Clear resets to Option::None; do not read its inactive payload.
        terminal.clear().unwrap();
        terminal.backend().assert_buffer_lines(["  "]);
    }

    #[test]
    fn actual_terminal_second_draw_erases_previous_heap_payload() {
        let mut terminal = terminal();
        let text = Zeroizing::new(format!("x{}", "\u{301}".repeat(32)));
        let frame = terminal
            .draw(|frame| frame.render_widget(text.as_str(), frame.area()))
            .unwrap();
        let watch = DeallocationWatch::new(frame.buffer[(0, 0)].symbol().as_bytes());
        terminal
            .draw(|frame| frame.render_widget("ok", frame.area()))
            .unwrap();
        watch.assert_erased();
    }

    #[test]
    fn actual_terminal_resize_erases_previous_inline_storage() {
        let mut terminal = terminal();
        let frame = terminal
            .draw(|frame| frame.render_widget("é", frame.area()))
            .unwrap();
        let watch = DeallocationWatch::in_cells(&frame.buffer.content, 0);
        terminal.resize(Rect::new(0, 0, 64, 2)).unwrap();
        watch.assert_erased();
    }
}
