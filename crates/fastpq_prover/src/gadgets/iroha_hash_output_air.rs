//! Full-width conversion of a BLAKE2b-256 result into the Iroha hash encoding.
//!
//! Iroha preserves the first 32 digest bytes except for byte 31's least
//! significant bit, which is set to one. With little-endian BLAKE2b words this
//! is bit 248, not bit zero. Every other bit remains bound to its hash output.
//! The 255 remaining variable bits must be accounted for in commitment security.
//!
//! The input words must come from a separately constrained BLAKE2b computation
//! initialized for a **32-byte** digest. Truncating BLAKE2b-512 is not equivalent.
//! These relations enforce only output conversion, not initialization, message
//! framing, compression, hash chaining or authorization. The upstream hash AIR
//! must enforce activated Booleanity of every input bit and all eight final
//! compression words. This module has no proof-admission call site yet.
//!
//! TODO: Link these output bits to the complete committed BLAKE2b schedule and
//! every SMT leaf, sibling, internal-node and public-root relation before
//! replacing replay. Do not substitute a scalar projection of this digest.

use super::{arx64_air::BitWord64, transfer_integer_air::IntegerAirField};

/// Number of 64-bit words returned by BLAKE2b configured for a 32-byte digest.
pub const DIGEST_WORDS: usize = 4;
/// Exact number of output bits, including the fixed Iroha marker.
pub const DIGEST_BITS: usize = DIGEST_WORDS * 64;
/// Little-endian bit position corresponding to byte 31's least significant bit.
pub const MARKER_BIT: usize = 248;
/// Selector Booleanity, activated output bits and complete output linkage.
pub const CONSTRAINT_COUNT: usize = 1 + 2 * DIGEST_BITS;
/// Maximum numerator degree in the selector and input/output variables.
pub const MAX_CONSTRAINT_DEGREE: usize = 2;

/// All four output words, preserving the Iroha hash's complete byte encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrohaHashOutput<F = u64> {
    /// Little-endian digest words with byte 31's marker bit set on active rows.
    pub words: [BitWord64<F>; DIGEST_WORDS],
}

impl IrohaHashOutput<u64> {
    /// Generate the marker conversion from the first four final hash words.
    ///
    /// This constructor does not establish that the words came from BLAKE2b.
    #[must_use]
    pub fn from_blake2b_256_words(words: [u64; DIGEST_WORDS]) -> Self {
        let mut output = Self {
            words: words.map(BitWord64::from_integer),
        };
        output.words[MARKER_BIT / 64].bits[MARKER_BIT % 64] = 1;
        output
    }
}

impl<F: IntegerAirField> IrohaHashOutput<F> {
    /// Canonical zero output on an inactive hash invocation.
    #[must_use]
    pub fn inactive() -> Self {
        Self {
            words: [BitWord64::zero(); DIGEST_WORDS],
        }
    }
}

/// Bind all output bits to separately constrained BLAKE2b-256 words.
///
/// Input words must already satisfy their upstream activated bit constraints.
/// The marker input bit is intentionally discarded; its output equals `active`.
/// All other output bits equal their input without a selector multiplication,
/// which also binds inactive inputs to zero. Each output bit satisfies
/// `bit * (bit - active) = 0`. Every returned numerator requires the applicable
/// row zerofier and independent composition challenge in the surrounding AIR.
#[must_use]
pub fn constraint_residues<F: IntegerAirField>(
    active: F,
    raw_words: &[BitWord64<F>; DIGEST_WORDS],
    output: &IrohaHashOutput<F>,
) -> Vec<F> {
    let mut residues = Vec::with_capacity(CONSTRAINT_COUNT);
    residues.push(active.mul(active.sub(F::ONE)));
    for bit in 0..DIGEST_BITS {
        let value = output.words[bit / 64].bits[bit % 64];
        residues.push(value.mul(value.sub(active)));
        let expected = if bit == MARKER_BIT {
            active
        } else {
            raw_words[bit / 64].bits[bit % 64]
        };
        residues.push(value.sub(expected));
    }
    residues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GoldilocksFp4V1;
    use iroha_crypto::Hash;

    fn words() -> [u64; DIGEST_WORDS] {
        [
            0,
            u64::MAX,
            crate::GOLDILOCKS_MODULUS_V1,
            0x80_23_45_67_89_ab_cd_ef,
        ]
    }

    fn bytes(output: &IrohaHashOutput) -> [u8; 32] {
        core::array::from_fn(|byte| {
            (0..8).fold(0_u8, |value, bit| {
                value | ((output.words[byte / 8].bits[(byte % 8) * 8 + bit] as u8) << bit)
            })
        })
    }

    #[test]
    fn conversion_matches_existing_iroha_marker_for_both_input_bit_values() {
        for marker in [0, 1] {
            let mut raw = words();
            raw[3] = (raw[3] & !(1 << 56)) | (marker << 56);
            let raw_bytes: [u8; 32] =
                core::array::from_fn(|byte| raw[byte / 8].to_le_bytes()[byte % 8]);
            let output = IrohaHashOutput::from_blake2b_256_words(raw);
            let expected: [u8; 32] = Hash::prehashed(raw_bytes).into();
            assert_eq!(bytes(&output), expected);
            assert_eq!(expected[31] & 1, 1);
            assert_eq!(expected[0], raw_bytes[0]);
            assert!(
                constraint_residues(1, &raw.map(BitWord64::from_integer), &output)
                    .into_iter()
                    .all(|value| value == 0)
            );
        }
    }

    #[test]
    fn every_output_bit_and_each_nondiscarded_input_bit_is_bound() {
        let raw = words().map(BitWord64::from_integer);
        let output = IrohaHashOutput::from_blake2b_256_words(words());
        for bit in 0..DIGEST_BITS {
            let mut changed_output = output;
            changed_output.words[bit / 64].bits[bit % 64] ^= 1;
            assert!(
                constraint_residues(1, &raw, &changed_output)
                    .into_iter()
                    .any(|value| value != 0),
                "output bit {bit}"
            );
            let mut changed_input = raw;
            changed_input[bit / 64].bits[bit % 64] ^= 1;
            let accepted = constraint_residues(1, &changed_input, &output)
                .into_iter()
                .all(|value| value == 0);
            assert_eq!(accepted, bit == MARKER_BIT, "input bit {bit}");
        }
    }

    #[test]
    fn inactive_output_and_nonboolean_selectors_are_rejected_when_malformed() {
        let raw = [BitWord64::<u64>::zero(); DIGEST_WORDS];
        let output = IrohaHashOutput::inactive();
        assert!(
            constraint_residues(0, &raw, &output)
                .into_iter()
                .all(|value| value == 0)
        );
        for bit in 0..DIGEST_BITS {
            let mut changed = output;
            changed.words[bit / 64].bits[bit % 64] = 1;
            assert!(
                constraint_residues(0, &raw, &changed)
                    .into_iter()
                    .any(|value| value != 0)
            );
        }
        assert!(
            constraint_residues(2, &raw, &output)
                .into_iter()
                .any(|value| value != 0)
        );
    }

    #[test]
    fn extension_evaluation_matches_base_residues_including_invalid_bits() {
        let raw = words().map(BitWord64::from_integer);
        let mut output = IrohaHashOutput::from_blake2b_256_words(words());
        output.words[2].bits[7] = 2;
        let lift = |word: BitWord64<u64>| BitWord64 {
            bits: word
                .bits
                .map(|bit| GoldilocksFp4V1::from_base(bit).unwrap()),
        };
        let base = constraint_residues(1, &raw, &output);
        let extension = constraint_residues(
            GoldilocksFp4V1::ONE,
            &raw.map(lift),
            &IrohaHashOutput {
                words: output.words.map(lift),
            },
        );
        assert_eq!(base.len(), CONSTRAINT_COUNT);
        assert!(base.iter().any(|&value| value != 0));
        assert_eq!(
            extension,
            base.into_iter()
                .map(|value| GoldilocksFp4V1::from_base(value).unwrap())
                .collect::<Vec<_>>()
        );
        assert_eq!(MAX_CONSTRAINT_DEGREE, 2);
    }
    #[derive(Clone, Copy)]
    struct Degree(usize);

    impl IntegerAirField for Degree {
        const ZERO: Self = Self(0);
        const ONE: Self = Self(0);

        fn from_u32(_: u32) -> Self {
            Self(0)
        }

        fn add(self, other: Self) -> Self {
            Self(self.0.max(other.0))
        }

        fn sub(self, other: Self) -> Self {
            Self(self.0.max(other.0))
        }

        fn mul(self, other: Self) -> Self {
            Self(self.0 + other.0)
        }
    }

    #[test]
    fn every_output_conversion_numerator_is_quadratic() {
        // Treat the selector, every raw digest bit and every encoded output bit
        // as independent degree-one variables. Add/sub preserve the maximum
        // degree and multiplication adds degrees, without relying on Booleanity.
        let raw_words = [BitWord64 {
            bits: [Degree(1); 64],
        }; DIGEST_WORDS];
        let output = IrohaHashOutput { words: raw_words };
        let residues = constraint_residues(Degree(1), &raw_words, &output);
        assert_eq!(residues.len(), 513);
        assert_eq!(residues.len(), CONSTRAINT_COUNT);
        assert!(residues.iter().all(|degree| degree.0 <= 2));
        assert_eq!(residues.iter().map(|degree| degree.0).max(), Some(2));
        assert_eq!(
            residues.iter().map(|degree| degree.0).max(),
            Some(MAX_CONSTRAINT_DEGREE)
        );
    }
}
