#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DecodeError {
    OddLength,
    InvalidCharacter { byte: u8, index: usize },
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
    use super::{decode, DecodeError};

    #[test]
    fn decodes_all_nibbles_and_mixed_case() {
        assert_eq!(
            decode("0123456789aBcDeF").unwrap(),
            [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]
        );
    }

    #[test]
    fn rejects_malformed_input() {
        assert_eq!(decode("0"), Err(DecodeError::OddLength));
        assert_eq!(
            decode("00xz"),
            Err(DecodeError::InvalidCharacter { byte: b'x', index: 2 })
        );
    }
}
