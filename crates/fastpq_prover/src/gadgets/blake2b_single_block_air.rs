//! Bounded BLAKE2b-256 message framing and full marked Iroha hash relation.
//!
//! This composition handles exactly one final block and message lengths 0..=128
//! bytes. A committed Boolean prefix selects every present byte; its sum equals
//! the committed byte length. Eight Boolean length bits bind that exact count to
//! the full compression counter. Absent bytes are zero, all message bits bind to
//! their little-endian compression positions, digest initialization is fixed to
//! BLAKE2b-256, and all 32 marked output bytes are constrained.
//!
//! Initialization and final-block framing follow RFC 7693 section 3.3:
//! <https://www.rfc-editor.org/rfc/rfc7693#section-3.3>.
//! The current SMT value, leaf and internal-node payloads fit this bound. A key
//! hash includes a variable-length canonical key and may exceed it. Such inputs
//! are explicitly rejected by the constructor; no truncation or multi-block
//! fallback is provided by this gadget.
//!
//! This remains unintegrated with production proofs. The direct witness has
//! 254,602 cells plus one selector and 407,774 degree-at-most-two numerators,
//! requiring 976 ARX operations before a real register layout. Production's
//! 512-column limit is unchanged. Prefix/length/message auxiliaries must be
//! committed before AIR aggregation and independently degree-proved.
//!
//! TODO: Build and commit the narrow trace layout, add all required quotient and
//! degree checks, bind exact caller payload/domain bytes, and connect complete SMT
//! path/root relations. Multi-block key framing/chaining needs its own constraints.
//! This module neither authenticates callers nor replaces full witness replay.

use super::{
    blake2b_compression_air::{self, Blake2bCompressionWitness, INITIALIZATION_VECTOR},
    iroha_hash_output_air::{self, IrohaHashOutput},
    transfer_integer_air::IntegerAirField,
};

/// Exact maximum input size for this single-final-block relation.
pub const MAX_MESSAGE_BYTES: usize = 128;
/// Bits needed to represent every permitted byte count, including 128.
pub const LENGTH_BITS: usize = 8;
/// Fixed BLAKE2b parameter word for unkeyed sequential 32-byte output.
pub const DIGEST_PARAMETER_WORD: u64 = 0x0101_0020;
/// Framing numerators before compression and output-conversion constraints.
pub const FRAMING_CONSTRAINT_COUNT: usize = 1
    + MAX_MESSAGE_BYTES
    + (MAX_MESSAGE_BYTES - 1)
    + 1
    + LENGTH_BITS
    + 1
    + 2 * MAX_MESSAGE_BYTES * 8
    + 8 * 64
    + 2 * 64
    + 1;
/// Complete framing, compression and marked-output numerator count.
pub const CONSTRAINT_COUNT: usize = FRAMING_CONSTRAINT_COUNT
    + blake2b_compression_air::CONSTRAINT_COUNT
    + iroha_hash_output_air::CONSTRAINT_COUNT;
/// Direct field cells, excluding the shared selector.
pub const WITNESS_CELL_COUNT: usize = blake2b_compression_air::WITNESS_CELL_COUNT
    + MAX_MESSAGE_BYTES * 8
    + MAX_MESSAGE_BYTES
    + 1
    + LENGTH_BITS
    + iroha_hash_output_air::DIGEST_BITS;
/// ARX operation rows before adding a complete committed register layout.
pub const OPERATION_ROW_COUNT: usize = blake2b_compression_air::OPERATION_ROW_COUNT;
/// Maximum degree in arbitrary committed selector, prefix and witness variables.
pub const MAX_CONSTRAINT_DEGREE: usize = 2;

/// Complete bounded message, framing auxiliaries, compression and marked output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SingleBlockHashWitness<F = u64> {
    /// Exact message length, bound to both the prefix count and length bits.
    pub byte_len: F,
    /// Little-endian Boolean decomposition of the length.
    pub length_bits: [F; LENGTH_BITS],
    /// One for present bytes, followed by zeros; all zero when inactive.
    pub present: [F; MAX_MESSAGE_BYTES],
    /// Eight little-endian bits for each byte, forced to zero beyond the prefix.
    pub message_bits: [[F; 8]; MAX_MESSAGE_BYTES],
    /// Complete single final compression invocation, including all raw output words.
    pub compression: Box<Blake2bCompressionWitness<F>>,
    /// All marked Iroha hash words, with byte 31 bit zero forced to one when active.
    pub output: IrohaHashOutput<F>,
}

fn decode_word(bits: &[u64; 64]) -> u64 {
    bits.iter()
        .enumerate()
        .fold(0, |word, (index, &bit)| word | (bit << index))
}

impl SingleBlockHashWitness<u64> {
    /// Generate one exact final-block witness, rejecting every message over 128 bytes.
    ///
    /// No truncation or multi-block fallback is performed. Construction does not
    /// authenticate the message; admission must enforce the complete relation.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > MAX_MESSAGE_BYTES {
            return None;
        }
        let byte_len = bytes.len() as u64;
        let mut padded = [0_u8; MAX_MESSAGE_BYTES];
        padded[..bytes.len()].copy_from_slice(bytes);
        let message = core::array::from_fn(|index| {
            u64::from_le_bytes(
                padded[8 * index..8 * index + 8]
                    .try_into()
                    .expect("complete 8-byte word"),
            )
        });
        let mut chaining = INITIALIZATION_VECTOR;
        chaining[0] ^= DIGEST_PARAMETER_WORD;
        let compression = Box::new(Blake2bCompressionWitness::from_inputs(
            chaining,
            message,
            [byte_len, 0],
            true,
        ));
        let output = IrohaHashOutput::from_blake2b_256_words(core::array::from_fn(|index| {
            decode_word(&compression.output[index].bits)
        }));
        Some(Self {
            byte_len,
            length_bits: core::array::from_fn(|bit| (byte_len >> bit) & 1),
            present: core::array::from_fn(|index| u64::from(index < bytes.len())),
            message_bits: core::array::from_fn(|index| {
                core::array::from_fn(|bit| u64::from((padded[index] >> bit) & 1))
            }),
            compression,
            output,
        })
    }
}

impl<F: IntegerAirField> SingleBlockHashWitness<F> {
    /// Canonical zero message, framing, compression and output for an inactive row.
    #[must_use]
    pub fn inactive() -> Self {
        Self {
            byte_len: F::ZERO,
            length_bits: [F::ZERO; LENGTH_BITS],
            present: [F::ZERO; MAX_MESSAGE_BYTES],
            message_bits: [[F::ZERO; 8]; MAX_MESSAGE_BYTES],
            compression: Box::new(Blake2bCompressionWitness::inactive()),
            output: IrohaHashOutput::inactive(),
        }
    }
}

/// Evaluate exact length, presence, zero padding, message bits and compression framing.
///
/// Prefix bits obey `p_i (p_i - active) = 0` and
/// `p_(i+1) (active - p_i) = 0`. Their sum is therefore the exact bounded integer
/// byte count, without field wrap. Message bits obey `b (b - p_i) = 0`, making
/// present bytes Boolean and all absent bytes zero. The compression and hash
/// output polynomials must additionally be enforced by [`constraint_residues`].
#[must_use]
pub fn framing_residues<F: IntegerAirField>(
    active: F,
    witness: &SingleBlockHashWitness<F>,
) -> Vec<F> {
    let mut residues = Vec::with_capacity(FRAMING_CONSTRAINT_COUNT);
    residues.push(active.mul(active.sub(F::ONE)));
    let mut prefix_count = F::ZERO;
    for (index, &present) in witness.present.iter().enumerate() {
        residues.push(present.mul(present.sub(active)));
        prefix_count = prefix_count.add(present);
        if index > 0 {
            residues.push(present.mul(active.sub(witness.present[index - 1])));
        }
    }
    residues.push(witness.byte_len.sub(prefix_count));
    let mut bit_count = F::ZERO;
    let mut weight = F::ONE;
    for &bit in &witness.length_bits {
        residues.push(bit.mul(bit.sub(active)));
        bit_count = bit_count.add(bit.mul(weight));
        weight = weight.add(weight);
    }
    residues.push(witness.byte_len.sub(bit_count));
    for byte in 0..MAX_MESSAGE_BYTES {
        for bit in 0..8 {
            let value = witness.message_bits[byte][bit];
            residues.push(value.mul(value.sub(witness.present[byte])));
            residues
                .push(value.sub(witness.compression.message[byte / 8].bits[(byte % 8) * 8 + bit]));
        }
    }
    for word in 0..8 {
        let initialized =
            INITIALIZATION_VECTOR[word] ^ if word == 0 { DIGEST_PARAMETER_WORD } else { 0 };
        for bit in 0..64 {
            let expected = if (initialized >> bit) & 1 == 1 {
                active
            } else {
                F::ZERO
            };
            residues.push(witness.compression.chaining[word].bits[bit].sub(expected));
        }
    }
    for word in 0..2 {
        for bit in 0..64 {
            let expected = if word == 0 && bit < LENGTH_BITS {
                witness.length_bits[bit]
            } else {
                F::ZERO
            };
            residues.push(witness.compression.counter[word].bits[bit].sub(expected));
        }
    }
    residues.push(witness.compression.final_block.sub(active));
    residues
}

/// Evaluate the complete bounded BLAKE2b-256 and marked-output polynomial relation.
///
/// This adds all compression numerators and every marked output bit to the
/// framing relations. It does not substitute native hashing for constraints.
#[must_use]
pub fn constraint_residues<F: IntegerAirField>(
    active: F,
    witness: &SingleBlockHashWitness<F>,
) -> Vec<F> {
    let mut residues = Vec::with_capacity(CONSTRAINT_COUNT);
    residues.extend(framing_residues(active, witness));
    residues.extend(blake2b_compression_air::constraint_residues(
        active,
        &witness.compression,
    ));
    let raw_words = core::array::from_fn(|index| witness.compression.output[index]);
    residues.extend(iroha_hash_output_air::constraint_residues(
        active,
        &raw_words,
        &witness.output,
    ));
    residues
}

#[cfg(test)]
mod tests {
    use super::super::{
        arx64_air::{Add64Witness, BitWord64, XorRotate64Witness},
        blake2b_compression_air::{CompressionSteps, G_COUNT, Xor64Witness},
        blake2b_g_air::{Blake2bGWitness, GRegisters},
    };
    use super::*;
    use crate::GoldilocksFp4V1;

    fn marked_bytes(output: &IrohaHashOutput) -> [u8; 32] {
        core::array::from_fn(|byte| {
            (0..8).fold(0, |value, bit| {
                value | ((output.words[byte / 8].bits[(byte % 8) * 8 + bit] as u8) << bit)
            })
        })
    }

    fn framing_valid(active: u64, witness: &SingleBlockHashWitness) -> bool {
        let residues = framing_residues(active, witness);
        assert_eq!(residues.len(), FRAMING_CONSTRAINT_COUNT);
        residues.iter().all(|&value| value == 0)
    }

    fn valid(active: u64, witness: &SingleBlockHashWitness) -> bool {
        let residues = constraint_residues(active, witness);
        assert_eq!(residues.len(), CONSTRAINT_COUNT);
        residues.iter().all(|&value| value == 0)
    }

    #[test]
    fn complete_boundary_messages_match_native_hash_new() {
        for length in [0, 1, 7, 8, 31, 32, 63, 64, 81, 82, 127, 128] {
            let message: Vec<_> = (0..length)
                .map(|index| ((index * 73 + 19) % 256) as u8)
                .collect();
            let witness = SingleBlockHashWitness::from_bytes(&message).unwrap();
            assert_eq!(witness.byte_len, length as u64);
            assert!(valid(1, &witness), "length {length}");
            assert_eq!(
                marked_bytes(&witness.output),
                *iroha_crypto::Hash::new(&message).as_ref()
            );
        }
        assert!(SingleBlockHashWitness::from_bytes(&[0; 129]).is_none());
        assert!(SingleBlockHashWitness::from_bytes(&[0; 256]).is_none());
    }

    #[test]
    fn all_bounded_prefix_lengths_are_polynomially_exact() {
        let mut witness = SingleBlockHashWitness::from_bytes(&[]).unwrap();
        // Evaluate the framing stage over all lengths. Compression itself is
        // covered by the complete boundary vectors, not inferred from this test.
        for length in 0..=MAX_MESSAGE_BYTES {
            witness.byte_len = length as u64;
            witness.present = core::array::from_fn(|index| u64::from(index < length));
            witness.length_bits = core::array::from_fn(|bit| (length as u64 >> bit) & 1);
            witness.compression.counter[0] = BitWord64::from_integer(length as u64);
            assert!(framing_valid(1, &witness), "prefix length {length}");
        }
        witness.byte_len = 129;
        witness.length_bits = core::array::from_fn(|bit| (129 >> bit) & 1);
        witness.compression.counter[0] = BitWord64::from_integer(129);
        assert!(
            !framing_valid(1, &witness),
            "prefix count cannot encode a second block"
        );
    }

    #[test]
    fn forged_counts_prefix_gaps_and_padding_fail_even_with_matching_message_words() {
        let mut witness = SingleBlockHashWitness::from_bytes(&[0; 17]).unwrap();
        witness.byte_len = 18;
        assert!(!framing_valid(1, &witness));
        witness.length_bits = core::array::from_fn(|bit| (18 >> bit) & 1);
        witness.compression.counter[0] = BitWord64::from_integer(18);
        assert!(
            !framing_valid(1, &witness),
            "correct length bits cannot forge prefix count"
        );
        witness.byte_len = 17;
        witness.length_bits = core::array::from_fn(|bit| (17 >> bit) & 1);
        witness.compression.counter[0] = BitWord64::from_integer(17);
        witness.present[5] = 0;
        witness.present[17] = 1;
        assert!(
            !framing_valid(1, &witness),
            "equal-cardinality hole must fail"
        );
        witness.present[5] = 1;
        witness.present[17] = 0;
        witness.present[0] = 2;
        assert!(!framing_valid(1, &witness));
        witness.present[0] = 1;
        for byte in 17..MAX_MESSAGE_BYTES {
            witness.message_bits[byte][7] = 1;
            witness.compression.message[byte / 8].bits[(byte % 8) * 8 + 7] = 1;
            assert!(!framing_valid(1, &witness), "nonzero padded byte {byte}");
            witness.message_bits[byte][7] = 0;
            witness.compression.message[byte / 8].bits[(byte % 8) * 8 + 7] = 0;
        }
        witness.length_bits[0] = 2;
        assert!(!framing_valid(1, &witness));
    }

    #[test]
    fn every_message_bit_is_bound_to_its_exact_little_endian_position() {
        let mut witness = SingleBlockHashWitness::from_bytes(&[0; MAX_MESSAGE_BYTES]).unwrap();
        for byte in 0..MAX_MESSAGE_BYTES {
            for bit in 0..8 {
                witness.message_bits[byte][bit] = 1;
                assert!(
                    !framing_valid(1, &witness),
                    "detached byte {byte}, bit {bit}"
                );
                witness.compression.message[byte / 8].bits[(byte % 8) * 8 + bit] = 1;
                assert!(
                    framing_valid(1, &witness),
                    "matching packed byte {byte}, bit {bit}"
                );
                witness.message_bits[byte][bit] = 2;
                witness.compression.message[byte / 8].bits[(byte % 8) * 8 + bit] = 2;
                assert!(
                    !framing_valid(1, &witness),
                    "non-Boolean byte {byte}, bit {bit}"
                );
                witness.message_bits[byte][bit] = 0;
                witness.compression.message[byte / 8].bits[(byte % 8) * 8 + bit] = 0;
            }
        }
    }

    #[test]
    fn iv_counter_and_final_flag_are_fixed_by_framing() {
        let mut witness = SingleBlockHashWitness::from_bytes(b"abc").unwrap();
        for word in 0..8 {
            witness.compression.chaining[word].bits[7] ^= 1;
            assert!(!framing_valid(1, &witness), "IV word {word}");
            witness.compression.chaining[word].bits[7] ^= 1;
        }
        for word in 0..2 {
            for bit in 0..64 {
                witness.compression.counter[word].bits[bit] ^= 1;
                assert!(
                    !framing_valid(1, &witness),
                    "counter word {word}, bit {bit}"
                );
                witness.compression.counter[word].bits[bit] ^= 1;
            }
        }
        witness.compression.final_block = 0;
        assert!(!framing_valid(1, &witness));
        witness.compression.final_block = 2;
        assert!(!framing_valid(1, &witness));
    }

    #[test]
    fn internally_valid_wrong_digest_parameters_or_counter_do_not_bypass_framing() {
        let mut witness = SingleBlockHashWitness::from_bytes(b"abc").unwrap();
        let message = witness
            .compression
            .message
            .map(|word| decode_word(&word.bits));
        for (parameter, counter, final_block) in [
            (0x0101_0040, 3, true),
            (DIGEST_PARAMETER_WORD, 4, true),
            (DIGEST_PARAMETER_WORD, 3, false),
        ] {
            let mut chaining = INITIALIZATION_VECTOR;
            chaining[0] ^= parameter;
            witness.compression = Box::new(Blake2bCompressionWitness::from_inputs(
                chaining,
                message,
                [counter, 0],
                final_block,
            ));
            witness.output =
                IrohaHashOutput::from_blake2b_256_words(core::array::from_fn(|index| {
                    decode_word(&witness.compression.output[index].bits)
                }));
            assert!(
                blake2b_compression_air::constraint_residues(1, &witness.compression)
                    .iter()
                    .all(|&value| value == 0)
            );
            assert!(
                !valid(1, &witness),
                "wrong parameter/counter/final tuple {parameter:x}/{counter}/{final_block}"
            );
        }
    }

    #[test]
    fn complete_output_and_inactive_hash_remain_bound() {
        let mut witness = SingleBlockHashWitness::from_bytes(b"abc").unwrap();
        for bit in [0, 63, 127, 191, iroha_hash_output_air::MARKER_BIT, 255] {
            witness.output.words[bit / 64].bits[bit % 64] ^= 1;
            assert!(!valid(1, &witness), "marked output bit {bit}");
            witness.output.words[bit / 64].bits[bit % 64] ^= 1;
        }
        let mut inactive = SingleBlockHashWitness::<u64>::inactive();
        assert!(valid(0, &inactive));
        assert!(!valid(1, &inactive));
        inactive.present[0] = 1;
        assert!(!framing_valid(0, &inactive));
        inactive.present[0] = 0;
        inactive.message_bits[127][7] = 1;
        assert!(!framing_valid(0, &inactive));
    }

    #[test]
    fn framing_base_and_fp4_evaluators_agree_on_invalid_openings() {
        let mut base = SingleBlockHashWitness::from_bytes(b"abc").unwrap();
        base.present[1] = 2;
        base.message_bits[19][3] = 4;
        base.compression.chaining[6].bits[37] = 2;
        let embed = |value| GoldilocksFp4V1::from_base(value).unwrap();
        let word = |value: BitWord64| BitWord64 {
            bits: value.bits.map(embed),
        };
        let mut extension = SingleBlockHashWitness::<GoldilocksFp4V1>::inactive();
        extension.byte_len = embed(base.byte_len);
        extension.length_bits = base.length_bits.map(embed);
        extension.present = base.present.map(embed);
        extension.message_bits = base.message_bits.map(|bits| bits.map(embed));
        extension.compression.chaining = base.compression.chaining.map(word);
        extension.compression.message = base.compression.message.map(word);
        extension.compression.counter = base.compression.counter.map(word);
        extension.compression.final_block = embed(base.compression.final_block);
        // This test compares only framing, whose complete inputs are lifted.
        // Full compression Fp4 agreement is tested in that module itself.
        let expected = framing_residues(1, &base);
        assert!(expected.iter().any(|&value| value != 0));
        assert_eq!(
            expected.into_iter().map(embed).collect::<Vec<_>>(),
            framing_residues(GoldilocksFp4V1::ONE, &extension)
        );
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
    fn combined_relation_is_quadratic_with_explicit_resource_cost() {
        let word = BitWord64 {
            bits: [Degree(1); 64],
        };
        let registers = GRegisters {
            a: word,
            b: word,
            c: word,
            d: word,
        };
        let g = Blake2bGWitness {
            inputs: registers,
            message: [word; 2],
            outputs: registers,
            additions: [Add64Witness {
                left: word,
                right: word,
                output: word,
                carry_32: Degree(1),
                carry_64: Degree(1),
            }; 6],
            xor_rotations: [XorRotate64Witness {
                left: word,
                right: word,
                output: word,
            }; 4],
        };
        let witness = SingleBlockHashWitness {
            byte_len: Degree(1),
            length_bits: [Degree(1); LENGTH_BITS],
            present: [Degree(1); MAX_MESSAGE_BYTES],
            message_bits: [[Degree(1); 8]; MAX_MESSAGE_BYTES],
            compression: Box::new(Blake2bCompressionWitness {
                chaining: [word; 8],
                message: [word; 16],
                counter: [word; 2],
                final_block: Degree(1),
                initialized: Box::new([word; 16]),
                steps: CompressionSteps::new((0..G_COUNT).map(|_| g).collect()).unwrap(),
                feed_forward: Box::new(
                    [Xor64Witness {
                        left: word,
                        right: word,
                        output: word,
                    }; 16],
                ),
                output: [word; 8],
            }),
            output: IrohaHashOutput { words: [word; 4] },
        };
        let residues = constraint_residues(Degree(1), &witness);
        assert_eq!(residues.len(), CONSTRAINT_COUNT);
        assert_eq!(
            residues.iter().map(|value| value.0).max(),
            Some(MAX_CONSTRAINT_DEGREE)
        );
        assert_eq!(FRAMING_CONSTRAINT_COUNT, 2955);
        assert_eq!(CONSTRAINT_COUNT, 407_774);
        assert_eq!(WITNESS_CELL_COUNT, 254_602);
        assert_eq!(OPERATION_ROW_COUNT, 976);
        assert!(core::mem::size_of::<SingleBlockHashWitness<GoldilocksFp4V1>>() < 64 * 1024);
    }
}
