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
mod totp_cache;
pub mod vaults;

#[cfg(all(test, not(feature = "desktop")))]
mod native_publication_regressions;

// Explicit native-test artifact only. Never expose fixture controls in a
// device, release, desktop, or ordinary simulator application.
#[cfg(all(
    hidlins_ios_publication_fixture,
    not(all(
        target_os = "ios",
        target_env = "sim",
        debug_assertions,
        not(feature = "desktop")
    ))
))]
compile_error!("iOS publication fixtures require a debug simulator build without desktop features");

#[cfg(hidlins_ios_publication_fixture)]
#[allow(unsafe_code)] // Locally audited test-only C buffer boundary, not core code.
mod ios_publication_fixture;
