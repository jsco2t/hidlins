#![deny(unsafe_code)]
//! Reusable application API over the vault, security, generation and sync crates.
//!
//! Applications own presentation and platform mechanisms. This crate owns
//! sessions, DTO conversion, secret lifetimes and operation orchestration.

#[cfg(target_os = "android")]
pub mod android;
pub mod api;
pub mod clipboard_port;
pub mod dto;
pub mod error;
pub mod event;
#[cfg(any(test, feature = "test-fixtures"))]
pub mod fixtures;
#[cfg(target_os = "ios")]
#[allow(unsafe_code)]
pub mod ios;
pub mod sync_port;
