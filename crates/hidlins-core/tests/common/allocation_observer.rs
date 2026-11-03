//! Observe initialized bytes while their allocation is still owned by the allocator.
//! This is isolated to regression-test executables, never production code.

#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

struct ObservingAllocator;

#[global_allocator]
static ALLOCATOR: ObservingAllocator = ObservingAllocator;
static WATCHED: AtomicPtr<u8> = AtomicPtr::new(ptr::null_mut());
static LENGTH: AtomicUsize = AtomicUsize::new(0);
static DEALLOCATED: AtomicBool = AtomicBool::new(false);
static ERASED: AtomicBool = AtomicBool::new(false);
static SERIAL: Mutex<()> = Mutex::new(());

// SAFETY: allocation requests are forwarded unchanged to System. The observer
// reads only the initialized prefix captured from a live owned String/Vec,
// inside dealloc and BEFORE System releases that same allocation. Atomics and
// the watch guard serialize observations; the allocator never allocates or panics.
unsafe impl GlobalAlloc for ObservingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the GlobalAlloc caller supplied this valid layout.
        unsafe { System.alloc(layout) }
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
            // SAFETY: the watch was created from the initialized buffer at
            // this allocation's base; ownership reaches this dealloc call
            // before it is freed. Never inspect freed or spare-capacity bytes.
            let bytes = unsafe { std::slice::from_raw_parts(pointer, length) };
            ERASED.store(bytes.iter().all(|byte| *byte == 0), Ordering::Release);
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
        DEALLOCATED.store(false, Ordering::Release);
        ERASED.store(false, Ordering::Release);
        WATCHED.store(bytes.as_ptr().cast_mut(), Ordering::Release);
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
        WATCHED.store(ptr::null_mut(), Ordering::Release);
    }
}
