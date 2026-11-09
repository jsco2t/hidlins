//! Observe the production input's live allocation before the allocator releases it.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::allocation_observer::DeallocationWatch;
use crate::widgets::password_input::PasswordInput;

const SEED: &str = "synthetic-password-🔒";

fn key(input: &mut PasswordInput, code: KeyCode) {
    input.on_key(&KeyEvent::new(code, KeyModifiers::NONE));
}

#[test]
fn final_input_drop_erases_owned_bytes() {
    let input = PasswordInput::with_value(SEED);
    let watch = DeallocationWatch::new(input.as_str().as_bytes());
    drop(input);
    watch.assert_erased();
}

#[test]
fn replacing_input_erases_previous_bytes() {
    let mut input = PasswordInput::with_value(SEED);
    let watch = DeallocationWatch::new(input.as_str().as_bytes());
    input.set_value("replacement");
    assert_eq!(input.as_str(), "replacement");
    watch.assert_erased();
}

#[test]
fn taking_input_preserves_protected_ownership() {
    let input = PasswordInput::with_value(SEED);
    let watch = DeallocationWatch::new(input.as_str().as_bytes());
    let taken = input.take();
    assert_eq!(taken.as_str(), SEED);
    drop(taken);
    watch.assert_erased();
}

#[test]
fn appending_across_capacity_erases_previous_allocation() {
    let mut input = PasswordInput::with_value(SEED);
    let watch = DeallocationWatch::new(input.as_str().as_bytes());
    for _ in 0..64 {
        key(&mut input, KeyCode::Char('é'));
    }
    assert_eq!(input.as_str(), format!("{SEED}{}", "é".repeat(64)));
    watch.assert_erased();
}

#[test]
fn growth_after_deletion_erases_original_initialized_tail() {
    let mut input = PasswordInput::with_value(SEED);
    // Capture the original initialized prefix, including the character that
    // Backspace will remove, while it is still part of the live allocation.
    let watch = DeallocationWatch::new(input.as_str().as_bytes());
    key(&mut input, KeyCode::Backspace);
    for _ in 0..64 {
        key(&mut input, KeyCode::Char('x'));
    }
    assert_eq!(
        input.as_str(),
        format!("synthetic-password-{}", "x".repeat(64))
    );
    watch.assert_erased();
}

#[test]
fn middle_unicode_insertion_erases_previous_allocation() {
    let mut input = PasswordInput::with_value(SEED);
    let watch = DeallocationWatch::new(input.as_str().as_bytes());
    key(&mut input, KeyCode::Left);
    for _ in 0..64 {
        key(&mut input, KeyCode::Char('界'));
    }
    assert_eq!(
        input.as_str(),
        format!("synthetic-password-{}🔒", "界".repeat(64))
    );
    assert_eq!(input.cursor_chars(), "synthetic-password-".len() + 64);
    watch.assert_erased();
}
