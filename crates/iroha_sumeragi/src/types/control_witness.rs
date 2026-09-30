//! Fixed-capacity application control bytes carried and signed independently of transactions.

use crate::bytes::{ByteDomain, ByteSequence, InlineBytes};

/// Maximum occupied bytes in one signed application control witness.
pub const MAX_CONTROL_WITNESS_BYTES: usize = 2048;

/// Semantic identity of application control bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlDomain {}
impl ByteDomain for ControlDomain {
    const NAME: &'static str = "ControlWitness";
    const FRAME: &'static str = "iroha_sumeragi::ControlWitness";
}
/// Opaque application control witness, encoded as its occupied canonical byte sequence.
/// Application validation determines exact demand and context, including empty witnesses.
pub type ControlWitness = ByteSequence<InlineBytes<MAX_CONTROL_WITNESS_BYTES, ControlDomain>>;

#[cfg(test)]
mod tests {
    use super::*;
    use norito::codec::{DecodeAll as _, Encode as _};
    use norito::core as ncore;
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
