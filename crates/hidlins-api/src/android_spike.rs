//! SPIKE-ONLY (T6.2, branch `spike/android-verifier`): TLS probe export.
//!
//! Drives a real `hidlins-sync` HTTP HEAD through the production TLS stack
//! (`ureq` + `rustls` + `rustls-platform-verifier`) so the scratch harness
//! app can prove, on an emulator, that certificate verification works
//! end-to-end through the JNI-initialized platform verifier — and that a
//! *missing* init produces a clear error rather than a hang.
//!
//! This module is spike scaffolding: it reaches into `hidlins_sync::s3::http`
//! (documented as not-for-external-consumers) because the probe must exercise
//! the exact production client, not a lookalike. It is committed only on the
//! spike branch and MUST NOT be merged to `flutter-app`.

use jni::objects::{JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;

use hidlins_sync::s3::http::{HttpBackend, HttpClient};

/// JNI entry point bound to `app.hidlins.spike.SpikeNative.probeTls(String)`.
///
/// Returns a human-readable one-line outcome:
/// - `"OK: HTTP <status> ..."` — TLS handshake + platform certificate
///   verification succeeded (any HTTP status, incl. 403, proves the TLS
///   layer: S3 rejects the unsigned request *after* the handshake).
/// - `"ERR: ..."` — transport/TLS failure (this is the expected shape for
///   the missing-init negative control).
/// - `"PANIC: ..."` — the TLS stack panicked (captured so it can never
///   unwind across the JNI boundary and abort the process).
#[no_mangle]
pub extern "system" fn Java_app_hidlins_spike_SpikeNative_probeTls<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    url: JString<'local>,
) -> jstring {
    let url: String = match env.get_string(&url) {
        Ok(s) => s.into(),
        Err(e) => {
            return make_jstring(&mut env, &format!("ERR: reading url argument: {e}"));
        }
    };

    // A panic hook captures the formatted panic (message + location)
    // regardless of payload type — the raw payload of std's formatted
    // panics is not always a downcastable String, and the memo needs the
    // exact missing-init failure text.
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|info| {
        if let Ok(mut slot) = LAST_PANIC.lock() {
            *slot = Some(info.to_string());
        }
    }));
    let outcome = std::panic::catch_unwind(|| probe(&url));
    std::panic::set_hook(prev_hook);

    let message = match outcome {
        Ok(Ok(status)) => format!(
            "OK: HTTP {status} — TLS handshake + platform certificate verification succeeded"
        ),
        Ok(Err(e)) => format!("ERR: {e}"),
        Err(payload) => {
            let hook_msg = LAST_PANIC
                .lock()
                .ok()
                .and_then(|mut s| s.take())
                .unwrap_or_else(|| panic_message(&payload));
            format!("PANIC: {hook_msg}")
        }
    };
    make_jstring(&mut env, &message)
}

static LAST_PANIC: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn probe(url: &str) -> Result<u16, String> {
    let client = HttpClient::new("hidlins-android-spike/0.1").map_err(|e| e.to_string())?;
    let response = client
        .request("HEAD", url, &[], &[])
        .map_err(|e| e.to_string())?;
    Ok(response.status)
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

fn make_jstring(env: &mut JNIEnv<'_>, s: &str) -> jstring {
    env.new_string(s)
        .map(jni::objects::JString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}
