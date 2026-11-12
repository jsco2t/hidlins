//! Test-only observations of initialized fields while their owners are alive.
#![allow(missing_docs)]

use std::{
    cell::RefCell,
    collections::BTreeMap,
    ops::{Deref, DerefMut},
};
use zeroize::Zeroize;

thread_local! {
    static OBSERVATIONS: RefCell<BTreeMap<&'static str, (usize, usize)>> = const { RefCell::new(BTreeMap::new()) };
}

pub fn reset() {
    OBSERVATIONS.with(|observations| observations.borrow_mut().clear());
}

pub fn record_erased(category: &'static str, erased: bool) {
    OBSERVATIONS.with(|observations| {
        let mut observations = observations.borrow_mut();
        let counts = observations.entry(category).or_default();
        counts.0 += 1;
        counts.1 += usize::from(!erased);
    });
}

pub fn record_bytes(category: &'static str, bytes: &[u8]) {
    record_erased(category, bytes.iter().all(|byte| *byte == 0));
}

pub fn count(category: &str) -> usize {
    OBSERVATIONS.with(|observations| {
        observations
            .borrow()
            .get(category)
            .map_or(0, |counts| counts.0)
    })
}

pub fn assert_erased(required: &[&str]) {
    OBSERVATIONS.with(|observations| {
        let observations = observations.borrow();
        for (category, (_, residual)) in observations.iter() {
            assert_eq!(*residual, 0, "initialized bytes remained at {category}");
        }
        for category in required {
            assert!(
                observations
                    .get(category)
                    .is_some_and(|counts| counts.0 > 0),
                "missing actual erasure observation: {category}"
            );
        }
    });
}

/// The real outer Zeroizing guard performs erasure. This wrapper only observes
/// the same initialized bytes during destruction and never repairs a missing wipe.
pub struct Bytes<T: Zeroize + AsRef<[u8]>> {
    category: &'static str,
    inner: T,
}

impl<T: Zeroize + AsRef<[u8]>> Bytes<T> {
    pub fn new(category: &'static str, inner: T) -> Self {
        Self { category, inner }
    }
}

impl<T: Zeroize + AsRef<[u8]>> Zeroize for Bytes<T> {
    fn zeroize(&mut self) {
        self.inner.zeroize();
    }
}

impl<T: Zeroize + AsRef<[u8]>> Deref for Bytes<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<T: Zeroize + AsRef<[u8]>> DerefMut for Bytes<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T: Zeroize + AsRef<[u8]>> Drop for Bytes<T> {
    fn drop(&mut self) {
        record_bytes(self.category, self.inner.as_ref());
    }
}
