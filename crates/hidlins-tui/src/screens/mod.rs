//! Per-phase rendering. Each module exposes a `render(app, frame, …)` that
//! reads `&App` immutably and composes widgets; key handling lives on `App`.

pub(crate) mod lock_screen;
pub(crate) mod secrets;
pub(crate) mod settings;
pub(crate) mod startup_modal;
pub(crate) mod workspace;
