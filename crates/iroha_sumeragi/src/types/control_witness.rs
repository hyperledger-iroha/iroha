//! Fixed-capacity application control bytes carried and signed independently of transactions.

use norito::core as ncore;
use std::{
    fmt,
    hash::{Hash, Hasher},
    io,
};

/// Maximum occupied bytes in one signed application control witness.
/// This bound is checked before copying or constructing any variable-size decode allocation.
pub const MAX_CONTROL_WITNESS_BYTES: usize = 2048;

/// Opaque application control witness with no heap backing or mutable allocation escape.
///
/// The sole canonical codec emits its occupied bytes as one byte sequence. The private unused
/// capacity is always zero and is never encoded. The application validates exact demand and
/// context; empty bytes do not by themselves prove that no witness was required.
#[derive(Clone, Copy)]
pub struct ControlWitness {
    bytes: [u8; MAX_CONTROL_WITNESS_BYTES],
    len: u16,
}

/// A control witness exceeded the fixed protocol capacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlWitnessError {
    /// Requested occupied byte length.
    pub length: usize,
}
impl fmt::Display for ControlWitnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "control witness length {} exceeds {}",
            self.length, MAX_CONTROL_WITNESS_BYTES
        )
    }
}
impl std::error::Error for ControlWitnessError {}

impl ControlWitness {
    /// Empty control bytes. Application validation must still check whether demand exists.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            bytes: [0; MAX_CONTROL_WITNESS_BYTES],
            len: 0,
        }
    }

    /// Copy bounded canonical application bytes without allocating.
    ///
    /// # Errors
    /// Rejects lengths above the protocol cap before copying any bytes.
    pub fn try_from_slice(bytes: &[u8]) -> Result<Self, ControlWitnessError> {
        if bytes.len() > MAX_CONTROL_WITNESS_BYTES {
            return Err(ControlWitnessError {
                length: bytes.len(),
            });
        }
        let mut witness = Self::empty();
        witness.bytes[..bytes.len()].copy_from_slice(bytes);
        witness.len = u16::try_from(bytes.len()).expect("protocol bound fits u16");
        Ok(witness)
    }
    /// Occupied canonical application bytes, without the unused capacity.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
    /// Whether no application control bytes are carried.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Number of occupied bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}
impl Default for ControlWitness {
    fn default() -> Self {
        Self::empty()
    }
}
impl fmt::Debug for ControlWitness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ControlWitness")
            .field(&self.as_slice())
            .finish()
    }
}
impl PartialEq for ControlWitness {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}
impl Eq for ControlWitness {}
impl Hash for ControlWitness {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}

// The application can count and stream its sole canonical frame directly into this owner.
// A failed write leaves it unchanged; neither replacement backing nor partial writes occur.
impl io::Write for ControlWitness {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let start = self.len();
        let end = start
            .checked_add(bytes.len())
            .filter(|end| *end <= MAX_CONTROL_WITNESS_BYTES)
            .ok_or_else(|| io::Error::from(io::ErrorKind::WriteZero))?;
        self.bytes[start..end].copy_from_slice(bytes);
        self.len = u16::try_from(end).expect("protocol bound fits u16");
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl ncore::SerializePayload for ControlWitness {
    fn serialize(&self, encoder: &mut ncore::Encoder<'_>) -> Result<(), ncore::Error> {
        ncore::write_seq_len(encoder, u64::from(self.len))?;
        encoder.write_all(self.as_slice())?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(8 + self.len())
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(8 + self.len())
    }
}
impl<'de> ncore::DecodeFromSlice<'de> for ControlWitness {
    fn decode_from_slice(bytes: &'de [u8]) -> Result<(Self, usize), ncore::Error> {
        let (length, prefix) = ncore::read_seq_len_slice(bytes)?;
        if length > MAX_CONTROL_WITNESS_BYTES {
            return Err(ncore::Error::FieldLengthExceeded {
                length: u64::try_from(length).unwrap_or(u64::MAX),
                limit: MAX_CONTROL_WITNESS_BYTES as u64,
            });
        }
        let used = prefix
            .checked_add(length)
            .ok_or(ncore::Error::LengthMismatch)?;
        let data = bytes
            .get(prefix..used)
            .ok_or(ncore::Error::LengthMismatch)?;
        let witness = Self::try_from_slice(data).map_err(|_| ncore::Error::LengthMismatch)?;
        ncore::note_payload_access(bytes, used);
        Ok((witness, used))
    }
}
impl<'de> ncore::DeserializePayload<'de> for ControlWitness {
    fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("canonical bounded control witness")
    }
    fn try_deserialize(archived: &'de ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let bytes = ncore::payload_slice_from_ptr(std::ptr::from_ref(archived).cast())?;
        <Self as ncore::DecodeFromSlice>::decode_from_slice(bytes).map(|(witness, _)| witness)
    }
}
impl norito::NoritoSchema for ControlWitness {
    fn nominal_name() -> String {
        "iroha_sumeragi::ControlWitness".to_owned()
    }
    fn static_frame_name() -> Option<&'static str> {
        Some("iroha_sumeragi::ControlWitness")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use norito::codec::{DecodeAll as _, Encode as _};
    use std::io::Write as _;

    #[test]
    fn fixed_control_bytes_encode_only_occupied_canonical_sequence() {
        for length in [0, 1, 127, 128, MAX_CONTROL_WITNESS_BYTES] {
            let bytes = vec![0x5A; length];
            let witness = ControlWitness::try_from_slice(&bytes).unwrap();
            assert_eq!(witness.len(), length);
            assert_eq!(witness.is_empty(), length == 0);
            assert_eq!(witness.as_slice(), bytes);
            assert_eq!(witness.encode(), bytes.encode());
            assert_eq!(
                ControlWitness::decode_all(&mut witness.encode().as_slice()).unwrap(),
                witness
            );
            let frame = norito::encode_canonical(&witness).unwrap();
            assert_eq!(
                norito::decode_canonical::<ControlWitness>(&frame).unwrap(),
                witness
            );
        }
        assert_eq!(ControlWitness::default(), ControlWitness::empty());
    }
    #[test]
    fn control_writer_streams_without_growth_and_refuses_without_partial_write() {
        let mut witness = ControlWitness::empty();
        witness.write_all(b"first").unwrap();
        let before = witness;
        assert!(witness.write_all(&[7; MAX_CONTROL_WITNESS_BYTES]).is_err());
        assert_eq!(witness, before);
        witness.write_all(b" second").unwrap();
        witness.flush().unwrap();
        assert_eq!(witness.as_slice(), b"first second");
        assert!(ControlWitness::try_from_slice(&[0; MAX_CONTROL_WITNESS_BYTES + 1]).is_err());
    }
    #[test]
    fn decoder_rejects_advertised_overflow_without_heap_backing_or_missing_bytes() {
        use ncore::DecodeFromSlice as _;
        for length in [(MAX_CONTROL_WITNESS_BYTES + 1) as u64, u64::MAX] {
            assert!(ControlWitness::decode_from_slice(&length.to_le_bytes()).is_err());
        }
        let mut truncated = 3_u64.to_le_bytes().to_vec();
        truncated.extend_from_slice(&[1, 2]);
        assert!(ControlWitness::decode_from_slice(&truncated).is_err());
        let mut exact = 2_u64.to_le_bytes().to_vec();
        exact.extend_from_slice(&[1, 2]);
        // Norito conservatively charges sequence bytes even though this type's backing is
        // inline. Preserve both cumulative element and allocation limits, rather than skipping
        // accounting through an inspection-only length parser.
        assert!(
            ncore::with_decode_limits(norito::DecodeLimits::new(2, 16, 2, 0, 8), || {
                ControlWitness::decode_from_slice(&exact)
            })
            .is_err()
        );
        let witness = ncore::with_decode_limits(norito::DecodeLimits::new(2, 16, 2, 2, 8), || {
            ControlWitness::decode_from_slice(&exact)
        })
        .unwrap();
        assert_eq!(witness.0.as_slice(), &[1, 2]);
        assert_eq!(witness.1, exact.len());
    }
}
