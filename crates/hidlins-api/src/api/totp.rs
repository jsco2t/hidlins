use hidlins_core::Uuid;

use super::session::AppSession;
use crate::dto::TotpCode;
use crate::error::HidlinsApiError;

impl AppSession {
    /// Compute the current TOTP code for a cached entry.
    ///
    /// This method reads the TOTP cache via a `RwLock` read guard and
    /// NEVER takes the session mutex (threading rule 3). The cache is
    /// populated by [`Self::entry_detail`] when the entry kind is TOTP.
    ///
    /// Returns `Err(Internal)` on cache miss — callers must open the
    /// entry detail before requesting TOTP codes.
    #[flutter_rust_bridge::frb(sync)]
    pub fn totp_now(&self, uuid: String) -> Result<TotpCode, HidlinsApiError> {
        let id = uuid
            .parse::<Uuid>()
            .map_err(|_| HidlinsApiError::InvalidInput {
                field: "uuid".to_string(),
                reason: "invalid UUID".to_string(),
            })?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());

        let cache = match self.totp_cache.read() {
            Ok(guard) => guard,
            Err(poison) => {
                // RwLock poisoned — clear everything and report miss.
                // Clearing requires a write guard; drop the read attempt,
                // take a write guard, clear, then report miss.
                drop(poison);
                match self.totp_cache.write() {
                    Ok(mut w) => w.clear(),
                    Err(wp) => wp.into_inner().clear(),
                }
                return Err(HidlinsApiError::Internal {
                    context: "totp not cached".to_string(),
                });
            }
        };

        let snapshot = cache.get(&id).ok_or_else(|| HidlinsApiError::Internal {
            context: "totp not cached".to_string(),
        })?;

        let code = snapshot.totp.code_at(now);
        let remaining_secs = snapshot.totp.remaining_at(now);
        let period = snapshot.totp.period();

        Ok(TotpCode {
            code,
            remaining_secs,
            period,
        })
    }
}
