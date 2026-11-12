//! Encoding oracle shared by integration tests.

/// Encode bytes as lowercase hexadecimal without relying on the production
/// helper, so integration assertions independently pin the wire representation.
pub fn encode_lower(bytes: &[u8]) -> String {
    bytes.iter().fold(
        String::with_capacity(bytes.len().saturating_mul(2)),
        |mut output, byte| {
            use std::fmt::Write as _;
            write!(output, "{byte:02x}").expect("writing to String cannot fail");
            output
        },
    )
}
