//! Small encodings used by the sync wire formats.

/// Encode bytes as lowercase hexadecimal ASCII.
///
/// Hidlins only needs the fixed lowercase form used by SHA-256 and SigV4.
/// Keeping that narrow operation here avoids a direct dependency whose broader
/// encode/decode API is unnecessary at these call sites.
pub(crate) fn encode_lower(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";

    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::encode_lower;

    #[test]
    fn covers_every_nibble() {
        assert_eq!(
            encode_lower(&[
                0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0xf0, 0xde, 0xbc, 0x9a, 0x78, 0x56,
                0x34, 0x12,
            ]),
            "0123456789abcdeff0debc9a78563412"
        );
    }

    #[test]
    fn handles_empty_input() {
        assert_eq!(encode_lower(&[]), "");
    }
}
