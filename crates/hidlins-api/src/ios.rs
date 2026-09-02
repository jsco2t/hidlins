//! iOS-only one-shot clipboard handoff.
//!
//! Dart receives only an opaque ticket. Swift presents that ticket here and
//! receives the protected bytes only inside a synchronous callback whose
//! lifetime is bounded by this function call. The registry removes the value
//! before calling native code, preserving one-shot semantics on every path.

use std::ffi::c_void;

/// Native callback which must synchronously copy the bytes into `UIPasteboard`.
pub type ClipboardReceiver =
    extern "C" fn(bytes: *const u8, len: usize, context: *mut c_void) -> bool;

/// Consume one prepared transfer and synchronously deliver it to iOS.
///
/// Status codes are deliberately small and payload-free:
/// 0 success, 1 invalid ABI input, 2 missing/expired/consumed ticket,
/// 3 native pasteboard rejection, 4 contained panic.
///
/// # Safety
///
/// `ticket` must reference `ticket_len` readable bytes for this call.
/// `context` must satisfy the receiver's contract. The receiver must not retain
/// `bytes`, which is valid only until it returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hidlins_ios_consume_clipboard(
    ticket: *const u8,
    ticket_len: usize,
    receiver: Option<ClipboardReceiver>,
    context: *mut c_void,
) -> i32 {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if ticket.is_null() || ticket_len == 0 || receiver.is_none() || context.is_null() {
            return 1;
        }

        // SAFETY: upheld by this function's caller contract and used only for
        // the duration of this invocation.
        let ticket_bytes = unsafe { std::slice::from_raw_parts(ticket, ticket_len) };
        let Ok(ticket_id) = std::str::from_utf8(ticket_bytes) else {
            return 1;
        };
        let Ok(secret) = crate::api::secrets::consume_clipboard_transfer(ticket_id) else {
            return 2;
        };
        let accepted = receiver.expect("receiver checked above")(
            secret.as_bytes().as_ptr(),
            secret.len(),
            context,
        );
        if accepted {
            0
        } else {
            3
        }
    }));
    result.unwrap_or(4)
}
