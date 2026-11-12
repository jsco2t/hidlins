//! Force std lowercase growth to move, inspecting initialized bytes before free.

#![allow(unsafe_code)]

mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use common::{fast_kdf, master, TestEnv};
use hidlins_core::{EntryBuilder, NoRecoveryConfirmed, SearchMode, SearchOptions, Vault};

struct MovingAllocator;
#[global_allocator]
static ALLOCATOR: MovingAllocator = MovingAllocator;
static SERIAL: Mutex<()> = Mutex::new(());
static MOVES: AtomicUsize = AtomicUsize::new(0);
static LEAKED: AtomicBool = AtomicBool::new(false);
thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
}

// SAFETY: all ordinary requests are forwarded unchanged. During the scoped
// fixture, the only growing 34-byte allocation is the lowercase String for
// seventeen U+0130 scalars. Eleven mappings write 33 bytes; the next 'i'
// fills byte 34 before the combining dot requests growth. Thus every byte
// inspected is initialized and still allocated. The replacement is allocated
// before the original is released, forcing a move without freed-memory reads.
unsafe impl GlobalAlloc for MovingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forward the caller's valid layout unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: forward the caller's live pointer/layout pair unchanged.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if layout.size() == 34 && new_size > 34 && ACTIVE.try_with(Cell::get).unwrap_or(false) {
            // SAFETY: realloc's caller guarantees a valid new size, retaining
            // the original layout's valid alignment.
            let replacement_layout =
                unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
            // SAFETY: the replacement layout is valid as above.
            let replacement = unsafe { System.alloc(replacement_layout) };
            if replacement.is_null() {
                return replacement;
            }
            // SAFETY: the complete initialized fixture prefix is still live.
            let bytes = unsafe { std::slice::from_raw_parts(pointer, 34) };
            LEAKED.fetch_or(bytes.iter().any(|byte| *byte != 0), Ordering::AcqRel);
            MOVES.fetch_add(1, Ordering::AcqRel);
            // SAFETY: the distinct new allocation has enough room, and the
            // old 34 bytes remain valid until the following deallocation.
            unsafe { ptr::copy_nonoverlapping(pointer, replacement, 34) };
            // SAFETY: release the original exactly once after copying it.
            unsafe { System.dealloc(pointer, layout) };
            replacement
        } else {
            // SAFETY: forward the caller's valid realloc request unchanged.
            unsafe { System.realloc(pointer, layout, new_size) }
        }
    }
}

struct Scope;
impl Drop for Scope {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(false));
    }
}

fn growth_probe(action: impl FnOnce()) -> (usize, bool) {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    MOVES.store(0, Ordering::Release);
    LEAKED.store(false, Ordering::Release);
    let scope = Scope;
    ACTIVE.with(|active| active.set(true));
    action();
    drop(scope);
    (
        MOVES.load(Ordering::Acquire),
        LEAKED.load(Ordering::Acquire),
    )
}

#[test]
fn probe_detects_plain_std_lowercase_growth() {
    let source = "İ".repeat(17);
    let (moves, leaked) = growth_probe(|| {
        assert_eq!(source.to_lowercase(), "i\u{307}".repeat(17));
    });
    assert!(
        moves > 0 && leaked,
        "the fixture must force the known std growth path"
    );
}

fn note_search(mode: SearchMode, growing_query: bool) {
    let env = TestEnv::new();
    let body = "İ".repeat(17);
    let mut vault = Vault::create(
        &env.tempdir().join("lowercase-growth.kdbx"),
        &master("fixture"),
        None,
        fast_kdf(),
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    let id = vault
        .add_entry(
            vault.root_group_uuid(),
            EntryBuilder::secure_note("").notes(body.as_str()).build(),
        )
        .unwrap();
    let options =
        SearchOptions::new(if growing_query { body } else { "z".to_owned() }).with_mode(mode);
    let (_, leaked) = growth_probe(|| {
        let hits = vault.search(options).unwrap();
        if growing_query {
            assert!(hits.iter().any(|hit| hit.uuid == id));
        } else {
            assert!(hits.is_empty());
        }
    });
    assert!(
        !leaked,
        "normalization must not release plaintext through a moving reallocation"
    );
}

#[test]
fn substring_note_growth_never_releases_plaintext() {
    note_search(SearchMode::Substring, false);
}

#[test]
fn wildcard_note_growth_never_releases_plaintext() {
    note_search(SearchMode::Wildcard, false);
}

#[test]
fn growing_query_normalization_never_releases_plaintext() {
    note_search(SearchMode::Substring, true);
}

#[test]
fn all_utf8_growing_mappings_keep_std_contextual_case_equivalence() {
    let env = TestEnv::new();
    let mut vault = Vault::create(
        &env.tempdir().join("case-context.kdbx"),
        &master("fixture"),
        None,
        fast_kdf(),
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    let growing = (0..=0x0010_ffff)
        .filter_map(char::from_u32)
        .filter(|scalar| {
            scalar.to_lowercase().map(char::len_utf8).sum::<usize>() > scalar.len_utf8()
        });
    for scalar in growing {
        for text in [
            format!("{scalar}"),
            format!("{scalar}Σ"),
            format!("Σ{scalar}"),
            format!("AΣ{scalar}"),
            format!("{scalar}ΣA"),
            format!("{scalar}\u{301}Σ"),
            format!("AΣ\u{301}{scalar}"),
            format!("{scalar}Σ\u{301}!"),
            format!("Σ-{scalar}"),
            format!("{scalar} Σ"),
            format!("ΟΣΣ{scalar}Σ"),
            format!("\u{345}Σ{scalar}"),
        ] {
            let original = format!("oracle-{text}");
            let expected = original.to_lowercase();
            let root = vault.root_group_uuid();
            let original_id = vault
                .add_entry(root, EntryBuilder::credential(&original).build())
                .unwrap();
            let expected_id = vault
                .add_entry(root, EntryBuilder::credential(&expected).build())
                .unwrap();
            for query in [&original, &expected] {
                let hits = vault
                    .search(SearchOptions::new(query).wildcard(true))
                    .unwrap();
                assert!(hits.iter().any(|hit| hit.uuid == original_id));
                assert!(hits.iter().any(|hit| hit.uuid == expected_id));
            }
        }
    }
}
