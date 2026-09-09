//! Android-only JNI surface for single-use clipboard transfers.

use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::jint;
use jni::JNIEnv;

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
