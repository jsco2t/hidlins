use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DecodeError {
    OddLength,
    InvalidCharacter { byte: u8, index: usize },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OddLength => formatter.write_str("odd number of hexadecimal digits"),
            Self::InvalidCharacter { byte, index } => write!(
                formatter,
                "invalid hexadecimal character 0x{byte:02x} at index {index}"
            ),
        }
    }
}

pub(crate) fn encode(input: impl AsRef<[u8]>) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let input = input.as_ref();
    let mut output = String::with_capacity(input.len().saturating_mul(2));
    for byte in input {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

pub(crate) fn decode(input: impl AsRef<[u8]>) -> Result<Vec<u8>, DecodeError> {
    let input = input.as_ref();
    if input.len() % 2 != 0 {
        return Err(DecodeError::OddLength);
    }
    input
        .chunks_exact(2)
        .enumerate()
        .map(|(pair_index, pair)| {
            let index = pair_index * 2;
            let high = nibble(pair[0]).ok_or(DecodeError::InvalidCharacter {
                byte: pair[0],
                index,
            })?;
            let low = nibble(pair[1]).ok_or(DecodeError::InvalidCharacter {
                byte: pair[1],
                index: index + 1,
            })?;
            Ok((high << 4) | low)
        })
        .collect()
}

const fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{decode, encode, DecodeError};

    #[test]
    fn codec_covers_all_nibbles_and_mixed_case() {
        assert_eq!(encode([0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]), "0123456789abcdef");
        assert_eq!(decode("0123456789aBcDeF").unwrap(), [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]);
    }

    #[test]
    fn decoder_rejects_malformed_input() {
        assert_eq!(decode("0"), Err(DecodeError::OddLength));
        assert_eq!(
            decode("00xz"),
            Err(DecodeError::InvalidCharacter { byte: b'x', index: 2 })
        );
    }
}
