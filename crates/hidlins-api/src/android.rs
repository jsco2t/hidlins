//! Android-only JNI surface: one-time `rustls-platform-verifier` init.
//!
//! On Android, certificate verification for every TLS connection the sync
//! stack makes (`hidlins-sync` → `ureq` → `rustls` +
//! `rustls-platform-verifier`) is delegated to the OS trust store through
//! JNI. The verifier crate requires a **one-time initialization with an
//! Android `Context`** before the first TLS use; without it, any TLS
//! attempt fails (design review A9 — the failure mode is an immediate,
//! clearly-worded panic inside the verifier, contained by the sync
//! worker's `catch_unwind` / frb's panic capture, never a hang).
//!
//! Kotlin cannot call a Rust crate API directly, so this module exports the
//! single JNI symbol the app's `Application` subclass invokes at startup,
//! immediately after `System.loadLibrary("hidlins_api")`:
//!
//! ```kotlin
//! object HidlinsNative {
//!     // @JvmStatic makes this a static native method, so the second JNI
//!     // argument really is the declaring class (matching `JClass` below).
//!     // Without it Kotlin binds an instance method and the argument is
//!     // the singleton instance — ABI-compatible here because the
//!     // parameter is unused (`JClass` is a transparent `JObject`
//!     // wrapper), but declare it static so signature and reality agree.
//!     @JvmStatic
//!     external fun initVerifier(context: Context): Boolean
//! }
//! // in Application.onCreate(), BEFORE any Flutter engine starts:
//! System.loadLibrary("hidlins_api")
//! check(HidlinsNative.initVerifier(this)) { "hidlins: verifier init failed" }
//! ```
//!
//! Failure contract: on error this function drains any pending Java
//! exception (describing it to logcat first — that pending throwable is
//! the *real* diagnostic), throws an `IllegalStateException` with a
//! secret-free summary, and returns `JNI_FALSE`. Per JNI semantics the
//! thrown exception is raised at the Kotlin call site the moment this
//! function returns, so **the exception is the authoritative failure
//! signal** — the boolean is observable only on success paths. Callers
//! should let the exception propagate (fail-fast at startup): an app with
//! a dead TLS stack must not boot silently.
//!
//! This is the boundary crate's ONLY hand-written FFI-export module. The
//! `#[allow(unsafe_code)]` on the export below is a documented,
//! narrowly-scoped exception to the workspace `deny(unsafe_code)` lint —
//! the `#[no_mangle]` attribute is what trips the lint; the module
//! contains no `unsafe` blocks (all JNI interaction goes through the
//! `jni` crate's safe wrappers).
//!
//! The verifier's Kotlin counterpart classes (`org.rustls.platformverifier`)
//! ship as a Maven artifact vendored in-tree at
//! `vendor/rustls-platform-verifier-android/maven/`; the Gradle side must
//! declare that repository (with an `exclusiveContent` filter so the
//! `rustls` group can never resolve from a network repo) and keep the
//! classes from R8 minification (see the spike memo
//! `android-verifier-spike.md` for the T8.x recipe).

use jni::objects::{JClass, JObject};
use jni::sys::{jboolean, JNI_FALSE, JNI_TRUE};
use jni::JNIEnv;

/// JNI entry point bound to `app.hidlins.HidlinsNative.initVerifier(Context)`.
///
/// Idempotent from the caller's perspective: the verifier stores its context
/// in a process-global `OnceCell`, so repeated calls are harmless (the first
/// successful initialization wins; a failed one leaves the cell empty and
/// retryable).
///
/// # Safety contract (JNI, not Rust `unsafe`)
///
/// The JVM guarantees `env`, `_class`, and `context` are valid for the
/// duration of this call (standard JNI invariants). `context` must be an
/// `android.content.Context`; passing any other object type makes the
/// verifier's own JNI lookups fail, which surfaces through the drained
/// pending exception + thrown `IllegalStateException` described in the
/// module docs — not undefined behavior. A panic must never unwind across
/// an `extern` boundary (that aborts the process), hence the
/// `catch_unwind`.
#[allow(unsafe_code)] // `#[no_mangle]` export — the module-doc'd lint exception
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
            throw_init_failure(&mut env, &format!("init failed: {e}"));
            JNI_FALSE
        }
        Err(payload) => {
            let detail = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "non-string panic payload".to_string());
            throw_init_failure(&mut env, &format!("init panicked: {detail}"));
            JNI_FALSE
        }
    }
}

/// Drain any pending Java exception, then throw an `IllegalStateException`.
///
/// The drain is load-bearing, not cosmetic: `init_with_env`'s error path
/// (e.g. `context` is not a real `Context`) leaves the original Java
/// exception **pending** on this thread, and calling further JNI functions
/// (`FindClass` inside `throw_new`) with a pending exception is illegal
/// under the JNI spec — CheckJNI (on by default on emulators and
/// debuggable builds) aborts the process. `exception_describe` writes the
/// original throwable to logcat first so the real diagnostic survives the
/// drain; the remaining `let _ =` swallows are genuinely best-effort (if
/// throwing itself fails, the `JNI_FALSE` return is the only signal left).
fn throw_init_failure(env: &mut JNIEnv<'_>, message: &str) {
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
    let _ = env.throw_new(
        "java/lang/IllegalStateException",
        format!("hidlins: rustls-platform-verifier {message}"),
    );
}
