//! Android-only JNI surface: one-time `rustls-platform-verifier` init.
//!
//! On Android, certificate verification for every TLS connection the sync
//! stack makes (`hidlins-sync` → `ureq` → `rustls` +
//! `rustls-platform-verifier`) is delegated to the OS trust store through
//! JNI. The verifier crate requires a **one-time initialization with an
//! Android `Context`** before the first TLS use; without it, any TLS
//! attempt fails (design review A9).
//!
//! Kotlin cannot call a Rust crate API directly, so this module exports the
//! single JNI symbol the app's `Application` subclass invokes at startup,
//! immediately after `System.loadLibrary("hidlins_api")`:
//!
//! ```kotlin
//! object HidlinsNative {
//!     external fun initVerifier(context: Context): Boolean
//! }
//! // in Application.onCreate():
//! System.loadLibrary("hidlins_api")
//! check(HidlinsNative.initVerifier(this)) { "verifier init failed" }
//! ```
//!
//! This is the boundary crate's ONLY hand-written FFI-export module and a
//! documented, narrowly-scoped exception to the workspace `deny(unsafe_code)`
//! lint (the `#[no_mangle]` export attribute is what trips the lint — the
//! module body contains no `unsafe` blocks; all JNI interaction goes through
//! the `jni` crate's safe wrappers). Precedent: the `frb_generated` module's
//! scoped allowance in `lib.rs`.
//!
//! The verifier's Kotlin counterpart classes (`org.rustls.platformverifier`)
//! ship as a Maven artifact vendored in-tree at
//! `vendor/rustls-platform-verifier-android/maven/`; the Gradle side must
//! declare that repository and keep the classes from R8 minification (see
//! the spike memo `android-verifier-spike.md` for the T8.x recipe).

use jni::objects::{JClass, JObject};
use jni::sys::{jboolean, JNI_FALSE, JNI_TRUE};
use jni::JNIEnv;

/// JNI entry point bound to `app.hidlins.HidlinsNative.initVerifier(Context)`.
///
/// Idempotent from the caller's perspective: the verifier stores its context
/// in a process-global `OnceCell`, so repeated calls are harmless (the first
/// initialization wins). Returns `JNI_TRUE` on success; on failure it throws
/// an `IllegalStateException` carrying the (secret-free) error context and
/// returns `JNI_FALSE` so both Kotlin error-handling styles work.
///
/// # Safety contract (JNI, not Rust `unsafe`)
///
/// The JVM guarantees `env`, `_class`, and `context` are valid for the
/// duration of this call (standard JNI invariants). `context` must be an
/// `android.content.Context`; passing any other object type makes the
/// verifier's own JNI lookups fail, which surfaces as the `Err` branch
/// below — not undefined behavior. A panic must never unwind across an
/// `extern` boundary (that aborts the process), hence the `catch_unwind`.
#[no_mangle]
pub extern "system" fn Java_app_hidlins_HidlinsNative_initVerifier<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    context: JObject<'local>,
) -> jboolean {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rustls_platform_verifier::android::init_with_env(&mut env, context)
    }));

    match result {
        Ok(Ok(())) => JNI_TRUE,
        Ok(Err(e)) => {
            // Best-effort: if throwing itself fails there is nothing more
            // we can do — the JNI_FALSE return still signals the failure.
            let _ = env.throw_new(
                "java/lang/IllegalStateException",
                format!("hidlins: rustls-platform-verifier init failed: {e}"),
            );
            JNI_FALSE
        }
        Err(_) => {
            let _ = env.throw_new(
                "java/lang/IllegalStateException",
                "hidlins: rustls-platform-verifier init panicked",
            );
            JNI_FALSE
        }
    }
}
