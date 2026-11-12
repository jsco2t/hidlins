//! Shared test helpers for `hidlins-security`.
//!
//! `signals` serializes process-global signal tests; `clock` provides the
//! deterministic `FakeClock` used by idle auto-lock tests. Clipboard fixtures
//! live with their display-gated integration suite.

#![allow(dead_code)] // not all helpers used by every integration test

pub mod clock;
pub mod signals;
