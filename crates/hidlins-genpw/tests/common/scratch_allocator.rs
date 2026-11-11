//! Observe only initialized prefixes of known fixture scratch allocations.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
use std::sync::Mutex;

struct ScratchAllocator;
#[global_allocator]
static ALLOCATOR: ScratchAllocator = ScratchAllocator;
static SERIAL: Mutex<()> = Mutex::new(());
static SIZE_A: AtomicUsize = AtomicUsize::new(0);
static SIZE_B: AtomicUsize = AtomicUsize::new(0);
static PREFIX_A: AtomicUsize = AtomicUsize::new(0);
static PREFIX_B: AtomicUsize = AtomicUsize::new(0);
static PREFIX: AtomicUsize = AtomicUsize::new(0);
static WATCHED: AtomicPtr<u8> = AtomicPtr::new(ptr::null_mut());
static FREED: AtomicBool = AtomicBool::new(false);
static ERASED: AtomicBool = AtomicBool::new(false);
thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
}

// SAFETY: all requests are forwarded unchanged. Selection is restricted to the
// active test thread and unique fixture scratch layouts. Deallocation reads
// only the explicitly initialized prefix, before System releases the buffer.
unsafe impl GlobalAlloc for ScratchAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forward the valid allocation request.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() && ACTIVE.try_with(Cell::get).unwrap_or(false) {
            let prefix = if layout.size() == SIZE_A.load(Ordering::Acquire) {
                PREFIX_A.load(Ordering::Acquire)
            } else if layout.size() == SIZE_B.load(Ordering::Acquire) {
                PREFIX_B.load(Ordering::Acquire)
            } else {
                0
            };
            if prefix != 0 {
                PREFIX.store(prefix, Ordering::Release);
                WATCHED.store(pointer, Ordering::Release);
            }
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
            // SAFETY: each fixture documents this prefix as fully initialized;
            // the allocation is live until System.dealloc below.
            let bytes =
                unsafe { std::slice::from_raw_parts(pointer, PREFIX.load(Ordering::Acquire)) };
            ERASED.store(bytes.iter().all(|byte| *byte == 0), Ordering::Release);
            FREED.store(true, Ordering::Release);
        }
        // SAFETY: forward the original pointer/layout exactly once.
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

pub fn assert_erased(layouts: [(usize, usize); 2], action: impl FnOnce()) {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    SIZE_A.store(layouts[0].0, Ordering::Release);
    PREFIX_A.store(layouts[0].1, Ordering::Release);
    SIZE_B.store(layouts[1].0, Ordering::Release);
    PREFIX_B.store(layouts[1].1, Ordering::Release);
    FREED.store(false, Ordering::Release);
    ERASED.store(false, Ordering::Release);
    let scope = Scope;
    ACTIVE.with(|active| active.set(true));
    action();
    drop(scope);
    assert!(
        FREED.load(Ordering::Acquire),
        "known scratch allocation must be observed"
    );
    assert!(
        ERASED.load(Ordering::Acquire),
        "secret scratch must be erased before release"
    );
}
