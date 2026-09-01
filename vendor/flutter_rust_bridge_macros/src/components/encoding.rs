pub(crate) fn encode_lower(input: impl AsRef<[u8]>) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";

    let input = input.as_ref();
    let mut encoded = String::with_capacity(input.len().saturating_mul(2));
    for byte in input {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::encode_lower;

    #[test]
    fn matches_macro_metadata_wire_format() {
        assert_eq!(encode_lower("#[frb(sync)]"), "235b6672622873796e63295d");
        assert_eq!(encode_lower(""), "");
    }
}
