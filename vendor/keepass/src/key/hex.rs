use std::fmt;

/// Error returned when a hexadecimal key contains malformed input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HexDecodeError {
    /// Hexadecimal input must contain two characters per decoded byte.
    OddLength,
    /// A byte outside the ASCII hexadecimal ranges was encountered.
    InvalidCharacter { byte: u8, index: usize },
}

impl fmt::Display for HexDecodeError {
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

impl std::error::Error for HexDecodeError {}

pub(super) fn decode(input: impl AsRef<[u8]>) -> Result<Vec<u8>, HexDecodeError> {
    let input = input.as_ref();
    if input.len() % 2 != 0 {
        return Err(HexDecodeError::OddLength);
    }

    let mut decoded = Vec::with_capacity(input.len() / 2);
    for (pair_index, pair) in input.chunks_exact(2).enumerate() {
        let index = pair_index * 2;
        let high = nibble(pair[0]).ok_or(HexDecodeError::InvalidCharacter {
            byte: pair[0],
            index,
        })?;
        let low = nibble(pair[1]).ok_or(HexDecodeError::InvalidCharacter {
            byte: pair[1],
            index: index + 1,
        })?;
        decoded.push((high << 4) | low);
    }
    Ok(decoded)
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
    use super::{decode, HexDecodeError};

    #[test]
    fn decodes_mixed_case_and_all_nibbles() {
        assert_eq!(
            decode(b"0123456789aBcDeF").expect("valid hexadecimal"),
            [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]
        );
    }

    #[test]
    fn rejects_odd_length() {
        assert_eq!(decode(b"0"), Err(HexDecodeError::OddLength));
    }

    #[test]
    fn reports_invalid_character_index() {
        assert_eq!(
            decode(b"00xz"),
            Err(HexDecodeError::InvalidCharacter {
                byte: b'x',
                index: 2,
            })
        );
    }
}
