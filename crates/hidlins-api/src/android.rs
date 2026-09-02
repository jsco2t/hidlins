//! Android-only JNI surface for one-time platform TLS verifier initialization.
//!
//! `rustls-platform-verifier` needs the process `Context` before the first TLS
//! request. Android calls this one export from `Application.onCreate`, after
//! loading `libhidlins_api.so` and before a Flutter engine can issue sync work.
//! All JNI access uses safe `jni` wrappers. The one unsafe-code allowance is
//! scoped to the exported symbol attribute, and panics are contained so none
//! can unwind across the JVM boundary.

use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::{jboolean, jint, JNI_FALSE, JNI_TRUE};
use jni::JNIEnv;

/// JNI entry point bound to `app.hidlins.HidlinsNative.initVerifier(Context)`.
///
/// The verifier's internal `OnceCell` makes successful initialization
/// idempotent. A failed initialization leaves the cell retryable.
///
/// # JNI safety contract
///
/// The JVM supplies valid local references for the duration of the call.
/// `context` must be an `android.content.Context`. Errors and panics are
/// converted to a secret-free `IllegalStateException`; they never unwind.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
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
        Ok(Err(_)) => {
            throw_init_failure(&mut env, "initialization failed");
            JNI_FALSE
        }
        Err(_) => {
            throw_init_failure(&mut env, "initialization panicked");
            JNI_FALSE
        }
    }
}

/// Consume a Rust-owned, single-use clipboard ticket and deliver one mutable
/// byte array to the Android mechanism callback. The secret is never returned
/// through Dart or as a JNI `String`; Android wipes the array before return.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_hidlins_HidlinsNative_consumeClipboard<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    transfer_id: JString<'local>,
    receiver: JObject<'local>,
) -> jint {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if transfer_id.is_null() || receiver.is_null() {
            return Err(1);
        }
        let id: String = env.get_string(&transfer_id).map_err(|_| 1)?.into();
        let value = crate::api::secrets::consume_clipboard_transfer(&id).map_err(|_| 2)?;
        let bytes = env.byte_array_from_slice(value.as_bytes()).map_err(|_| 3)?;
        let accepted = env
            .call_method(
                receiver,
                "receive",
                "([B)Z",
                &[JValue::Object(bytes.as_ref())],
            )
            .and_then(|returned| returned.z())
            .map_err(|_| 3)?;
        if accepted {
            Ok(0)
        } else {
            Err(3)
        }
    }));
    match result {
        Ok(Ok(status)) => status,
        Ok(Err(status)) => {
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
            }
            status
        }
        Err(_) => {
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
            }
            3
        }
    }
}

/// Clear a verifier JNI exception before throwing the stable app exception.
/// Calling another JNI function with an exception pending can abort under
/// CheckJNI, so the clear is a correctness requirement rather than cleanup.
fn throw_init_failure(env: &mut JNIEnv<'_>, message: &str) {
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
    let _ = env.throw_new(
        "java/lang/IllegalStateException",
        format!("hidlins: rustls platform verifier {message}"),
    );
}
