#[cfg(not(feature = "desktop"))]
pub(crate) fn desktop_only(operation: &str, detail: &str) -> crate::error::HidlinsApiError {
    let _ = detail;
    crate::error::HidlinsApiError::UnsupportedPlatform {
        capability: operation.to_string(),
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
