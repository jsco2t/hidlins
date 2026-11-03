//! Test-only observation of fully initialized search-copy allocations.
//!
//! Fixtures use seventeen four-byte Unicode scalars: both their UTF-8 String
//! and exact-capacity Vec<char> occupy 68 initialized bytes. The observed
//! search calls allocate only those complete buffers with that layout.

#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
use std::sync::Mutex;

struct SearchAllocator;
#[global_allocator]
static ALLOCATOR: SearchAllocator = SearchAllocator;
static SERIAL: Mutex<()> = Mutex::new(());
static SIZE: AtomicUsize = AtomicUsize::new(0);
static ORDINAL: AtomicUsize = AtomicUsize::new(0);
static WATCHED: AtomicPtr<u8> = AtomicPtr::new(ptr::null_mut());
static FREED: AtomicBool = AtomicBool::new(false);
static ERASED: AtomicBool = AtomicBool::new(false);
thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
}

// SAFETY: requests are forwarded to System unchanged. The thread-local scope
// selects only the known complete fixture String/Vec<char> copies described
// above, excluding other test threads. Reads happen inside their dealloc call
// before System frees the allocation, with no reads of freed/spare bytes.
unsafe impl GlobalAlloc for SearchAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forward the valid GlobalAlloc request unchanged.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null()
            && layout.size() == SIZE.load(Ordering::Acquire)
            && ACTIVE.try_with(Cell::get).unwrap_or(false)
            && ORDINAL.load(Ordering::Acquire) != 0
            && ORDINAL.fetch_sub(1, Ordering::AcqRel) == 1
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
            // SAFETY: this is the selected fully initialized fixture copy,
            // still owned by the allocator until System.dealloc below.
            let bytes = unsafe { std::slice::from_raw_parts(pointer, layout.size()) };
            ERASED.store(bytes.iter().all(|byte| *byte == 0), Ordering::Release);
            FREED.store(true, Ordering::Release);
        }
        // SAFETY: forward the original pointer/layout pair exactly once.
        unsafe { System.dealloc(pointer, layout) }
    }
}

struct Scope;
impl Drop for Scope {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(false));
        WATCHED.store(ptr::null_mut(), Ordering::Release);
    }
}

pub fn assert_copy_erased(size: usize, ordinal: usize, action: impl FnOnce()) {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    SIZE.store(size, Ordering::Release);
    ORDINAL.store(ordinal, Ordering::Release);
    FREED.store(false, Ordering::Release);
    ERASED.store(false, Ordering::Release);
    let scope = Scope;
    ACTIVE.with(|active| active.set(true));
    action();
    drop(scope);
    assert!(
        FREED.load(Ordering::Acquire),
        "fixture must observe its known complete search-copy allocation"
    );
    assert!(
        ERASED.load(Ordering::Acquire),
        "search plaintext copy must be erased before release"
    );
}
