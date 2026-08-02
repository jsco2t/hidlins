// The single expression of "a desktop-only door was reached on mobile":
// the mobile counterpart stubs (paired with the desktop symbols the
// committed frb glue references unconditionally) all report through this
// helper so the diagnostic stays uniform and greppable. Reaching one is a
// UI-layer bug — the mobile UI never exposes these operations — hence
// `Internal` rather than a user-facing category.
#[cfg(not(feature = "desktop"))]
pub(crate) fn desktop_only(operation: &str, detail: &str) -> crate::error::HidlinsApiError {
    crate::error::HidlinsApiError::Internal {
        context: format!("{operation} is desktop-only in the alpha ({detail})"),
    }
}

pub mod bootstrap;
pub mod entries;
pub mod genpw;
pub mod prefs;
pub mod search;
pub mod secrets;
pub mod session;
pub mod sync;
pub mod totp;
pub mod vaults;
