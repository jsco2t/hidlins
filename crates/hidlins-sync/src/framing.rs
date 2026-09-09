//! Fixed transport preface and bounded Noise record framing.

/// Size of the cleartext protocol preface.
pub const PREFACE_LENGTH: usize = 12;
/// Largest Noise ciphertext record representable by the two-byte prefix.
pub const MAX_NOISE_FRAME: usize = u16::MAX as usize;

const MAGIC: &[u8; 8] = b"HIDLINS\0";
const MAJOR: u8 = 1;

/// Fixed connection mode encoded in the V1 preface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PrefaceMode {
    /// Noise XX first-contact pairing.
    Pairing = 1,
    /// Noise IK pinned-peer synchronization.
    Trusted = 2,
}

/// A validated protocol V1 preface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Preface {
    mode: PrefaceMode,
}

impl Preface {
    /// Construct the single supported protocol version for a fixed mode.
    #[must_use]
    pub const fn new(mode: PrefaceMode) -> Self {
        Self { mode }
    }

    /// Return the authenticated session mode.
    #[must_use]
    pub const fn mode(self) -> PrefaceMode {
        self.mode
    }

    /// Encode the exact twelve-byte V1 preface.
    #[must_use]
    pub const fn encode(self) -> [u8; PREFACE_LENGTH] {
        let mut bytes = [0_u8; PREFACE_LENGTH];
        bytes[0] = b'H';
        bytes[1] = b'I';
        bytes[2] = b'D';
        bytes[3] = b'L';
        bytes[4] = b'I';
        bytes[5] = b'N';
        bytes[6] = b'S';
        bytes[7] = 0;
        bytes[8] = MAJOR;
        bytes[9] = self.mode as u8;
        bytes
    }

    /// Decode an exact V1 preface, rejecting extensions and reserved bits.
    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        if bytes.len() != PREFACE_LENGTH || &bytes[..8] != MAGIC || bytes[8] != MAJOR {
            return Err(FramingError::InvalidPreface);
        }
        if bytes[10] != 0 || bytes[11] != 0 {
            return Err(FramingError::InvalidPreface);
        }
        let mode = match bytes[9] {
            1 => PrefaceMode::Pairing,
            2 => PrefaceMode::Trusted,
            _ => return Err(FramingError::InvalidPreface),
        };
        Ok(Self { mode })
    }
}

/// Secret-free framing failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum FramingError {
    /// The connection preface is not the exact supported V1 form.
    #[error("invalid connection preface")]
    InvalidPreface,
    /// A frame length is zero or exceeds the fixed transport bound.
    #[error("invalid encrypted record length")]
    InvalidLength,
    /// The supplied bytes end before the complete record.
    #[error("truncated encrypted record")]
    Truncated,
    /// Bytes remain after the one expected record.
    #[error("trailing encrypted record data")]
    TrailingData,
}

/// Prefix one nonempty bounded Noise ciphertext with its big-endian length.
pub fn encode_noise_frame(ciphertext: &[u8]) -> Result<Vec<u8>, FramingError> {
    let length = u16::try_from(ciphertext.len()).map_err(|_| FramingError::InvalidLength)?;
    if length == 0 {
        return Err(FramingError::InvalidLength);
    }
    let mut frame = Vec::with_capacity(2 + ciphertext.len());
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(ciphertext);
    Ok(frame)
}

/// Decode exactly one bounded Noise record without allocating payload storage.
pub fn decode_noise_frame(frame: &[u8]) -> Result<&[u8], FramingError> {
    let prefix: [u8; 2] = frame
        .get(..2)
        .ok_or(FramingError::Truncated)?
        .try_into()
        .map_err(|_| FramingError::Truncated)?;
    let length = usize::from(u16::from_be_bytes(prefix));
    if length == 0 {
        return Err(FramingError::InvalidLength);
    }
    let expected = 2_usize
        .checked_add(length)
        .ok_or(FramingError::InvalidLength)?;
    match frame.len().cmp(&expected) {
        std::cmp::Ordering::Less => Err(FramingError::Truncated),
        std::cmp::Ordering::Greater => Err(FramingError::TrailingData),
        std::cmp::Ordering::Equal => Ok(&frame[2..]),
    }
}
