use std::time::Duration;

use crate::error::HidlinsApiError;

#[cfg_attr(not(feature = "desktop"), allow(dead_code))]
pub trait ClipboardPort: Send + Sync {
    fn copy_with_autoclear(&self, text: String, ttl: Duration) -> Result<(), HidlinsApiError>;
}

#[cfg(feature = "desktop")]
pub(crate) struct SystemClipboard {
    clipboard: std::sync::Mutex<hidlins_security::Clipboard>,
    active_guard: std::sync::Mutex<Option<hidlins_security::AutoClearGuard>>,
}

#[cfg(feature = "desktop")]
impl SystemClipboard {
    pub(crate) fn new() -> Result<Self, HidlinsApiError> {
        let clipboard = hidlins_security::Clipboard::new().map_err(|e| HidlinsApiError::Io {
            context: format!("clipboard init: {e}"),
        })?;
        Ok(Self {
            clipboard: std::sync::Mutex::new(clipboard),
            active_guard: std::sync::Mutex::new(None),
        })
    }
}

#[cfg(feature = "desktop")]
impl ClipboardPort for SystemClipboard {
    fn copy_with_autoclear(&self, text: String, ttl: Duration) -> Result<(), HidlinsApiError> {
        let mut cb = self.clipboard.lock().map_err(|_| HidlinsApiError::Io {
            context: "clipboard lock poisoned".to_string(),
        })?;
        let guard = cb
            .copy_with_autoclear(text, ttl)
            .map_err(|e| HidlinsApiError::Io {
                context: format!("clipboard: {e}"),
            })?;
        // Store the guard so its auto-clear timer fires after TTL.
        // Replacing the previous guard cancels the old timer (correct:
        // the old clipboard content was just overwritten).
        if let Ok(mut slot) = self.active_guard.lock() {
            *slot = Some(guard);
        }
        Ok(())
    }
}

#[cfg(any(test, feature = "test-fixtures"))]
impl Default for RecordingClipboard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(test, feature = "test-fixtures"))]
#[allow(dead_code)]
pub struct RecordingClipboard {
    pub(crate) copies: std::sync::Mutex<Vec<(String, Duration)>>,
}

#[cfg(any(test, feature = "test-fixtures"))]
#[allow(dead_code)]
impl RecordingClipboard {
    pub fn new() -> Self {
        Self {
            copies: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn last_copied(&self) -> Option<String> {
        self.copies
            .lock()
            .ok()?
            .last()
            .map(|(text, _)| text.clone())
    }

    pub fn last_ttl(&self) -> Option<Duration> {
        self.copies.lock().ok()?.last().map(|(_, ttl)| *ttl)
    }
}

#[cfg(any(test, feature = "test-fixtures"))]
impl ClipboardPort for RecordingClipboard {
    fn copy_with_autoclear(&self, text: String, ttl: Duration) -> Result<(), HidlinsApiError> {
        if let Ok(mut copies) = self.copies.lock() {
            copies.push((text, ttl));
        }
        Ok(())
    }
}
