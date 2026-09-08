//! Quadratic bit constraints for the 64-bit operations used by BLAKE2b.
//!
//! Addition uses two 32-bit integer equations, with a Boolean carry between
//! halves and a Boolean discarded carry. This proves wrapping modulo `2^64`,
//! rather than reduction modulo Goldilocks. XOR and rotation operate on exact
//! Boolean bits. Every inactive word and carry is constrained to zero.
//!
//! The operation schedule follows RFC 7693 section 3.1:
//! <https://www.rfc-editor.org/rfc/rfc7693#section-3.1>.
//! TODO: commit these operation rows and constrain the BLAKE2b schedule, register
//! reads/writes, IV, message words, byte counters, final-block flag, output bytes
//! and Iroha hash marker before using them to prove complete SMT hash relations.
//! This module is a prerequisite and does not replace transfer witness replay.

use super::transfer_integer_air::IntegerAirField;

/// Bits in a BLAKE2b word, ordered least significant first.
pub const WORD_BITS: usize = 64;
/// Number of constraint numerators for one wrapping addition.
pub const ADD_CONSTRAINT_COUNT: usize = 197;
/// Number of constraint numerators for one XOR followed by a rotation.
pub const XOR_ROTATE_CONSTRAINT_COUNT: usize = 257;
/// Maximum degree of either operation's constraint numerators.
pub const MAX_CONSTRAINT_DEGREE: usize = 2;

/// A complete word decomposition; no reduction of a whole `u64` into a field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BitWord64<F = u64> {
    /// Least-significant-first bit openings, whose Booleanity is checked by AIR.
    pub bits: [F; WORD_BITS],
}

impl BitWord64<u64> {
    /// Decompose all 64 integer bits, including values above the field modulus.
    #[must_use]
    pub fn from_integer(word: u64) -> Self {
        Self {
            bits: core::array::from_fn(|bit| (word >> bit) & 1),
        }
    }
}

impl<F: IntegerAirField> BitWord64<F> {
    /// Canonical zero word for inactive operation rows.
    #[must_use]
    pub fn zero() -> Self {
        Self {
            bits: [F::ZERO; WORD_BITS],
        }
    }

    /// Exact 56+8-bit packed representation when the bits are Boolean.
    #[must_use]
    pub fn packed_limbs(&self) -> [F; 2] {
        [pack(&self.bits[..56]), pack(&self.bits[56..])]
    }
}

/// Witness for `left + right = output (mod 2^64)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Add64Witness<F = u64> {
    /// First 64-bit operand.
    pub left: BitWord64<F>,
    /// Second 64-bit operand.
    pub right: BitWord64<F>,
    /// Low 64 bits of the integer sum.
    pub output: BitWord64<F>,
    /// Carry from the low 32-bit sum into the high half.
    pub carry_32: F,
    /// Discarded carry out of the high 32-bit sum.
    pub carry_64: F,
}

impl Add64Witness<u64> {
    /// Generate the exact sum and both carries using native integer arithmetic.
    #[must_use]
    pub fn from_operands(left: u64, right: u64) -> Self {
        let wide_sum = u128::from(left) + u128::from(right);
        let low_sum = (left & u64::from(u32::MAX)) + (right & u64::from(u32::MAX));
        Self {
            left: BitWord64::from_integer(left),
            right: BitWord64::from_integer(right),
            output: BitWord64::from_integer(wide_sum as u64),
            carry_32: low_sum >> 32,
            carry_64: (wide_sum >> 64) as u64,
        }
    }
}

impl<F: IntegerAirField> Add64Witness<F> {
    /// Canonical witness for an inactive addition row.
    #[must_use]
    pub fn inactive() -> Self {
        Self {
            left: BitWord64::zero(),
            right: BitWord64::zero(),
            output: BitWord64::zero(),
            carry_32: F::ZERO,
            carry_64: F::ZERO,
        }
    }
}

/// The four fixed right rotations in the BLAKE2b G function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blake2bRotation {
    /// First rotation: 32 bits.
    Ror32,
    /// Second rotation: 24 bits.
    Ror24,
    /// Third rotation: 16 bits.
    Ror16,
    /// Fourth rotation: 63 bits.
    Ror63,
}

impl Blake2bRotation {
    /// Number of positions rotated right by this fixed operation.
    #[must_use]
    pub const fn bits(self) -> u32 {
        match self {
            Self::Ror32 => 32,
            Self::Ror24 => 24,
            Self::Ror16 => 16,
            Self::Ror63 => 63,
        }
    }
}

/// Witness for `output = (left XOR right).rotate_right(rotation)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XorRotate64Witness<F = u64> {
    /// First word to XOR.
    pub left: BitWord64<F>,
    /// Second word to XOR.
    pub right: BitWord64<F>,
    /// Complete rotated output word.
    pub output: BitWord64<F>,
}

impl XorRotate64Witness<u64> {
    /// Generate an exact bit witness for one fixed BLAKE2b rotation.
    #[must_use]
    pub fn from_operands(left: u64, right: u64, rotation: Blake2bRotation) -> Self {
        Self {
            left: BitWord64::from_integer(left),
            right: BitWord64::from_integer(right),
            output: BitWord64::from_integer((left ^ right).rotate_right(rotation.bits())),
        }
    }
}

impl<F: IntegerAirField> XorRotate64Witness<F> {
    /// Canonical witness for an inactive XOR/rotation row.
    #[must_use]
    pub fn inactive() -> Self {
        Self {
            left: BitWord64::zero(),
            right: BitWord64::zero(),
            output: BitWord64::zero(),
        }
    }
}

fn pack<F: IntegerAirField>(bits: &[F]) -> F {
    bits.iter()
        .rev()
        .fold(F::ZERO, |value, &bit| value.add(value).add(bit))
}

fn bit_residues<F: IntegerAirField>(active: F, word: &BitWord64<F>, output: &mut [F]) {
    for (&bit, residue) in word.bits.iter().zip(output) {
        *residue = bit.mul(bit.sub(active));
    }
}

/// Evaluate every addition numerator at base or extension-field openings.
///
/// Order: active Booleanity; 192 activated input/output bit constraints; two
/// activated carry constraints; low/high 32-bit sum equations. Each integer
/// equation has absolute value below `2^33`, so vanishing in Goldilocks implies
/// exact integer equality after the Boolean constraints hold.
#[must_use]
pub fn add_residues<F: IntegerAirField>(
    active: F,
    witness: &Add64Witness<F>,
) -> [F; ADD_CONSTRAINT_COUNT] {
    let mut residues = [F::ZERO; ADD_CONSTRAINT_COUNT];
    residues[0] = active.mul(active.sub(F::ONE));
    for (index, word) in [&witness.left, &witness.right, &witness.output]
        .iter()
        .enumerate()
    {
        bit_residues(
            active,
            word,
            &mut residues[1 + index * 64..1 + (index + 1) * 64],
        );
    }
    residues[193] = witness.carry_32.mul(witness.carry_32.sub(active));
    residues[194] = witness.carry_64.mul(witness.carry_64.sub(active));
    let radix = F::from_u32(u32::MAX).add(F::ONE);
    residues[195] = pack(&witness.left.bits[..32])
        .add(pack(&witness.right.bits[..32]))
        .sub(pack(&witness.output.bits[..32]))
        .sub(radix.mul(witness.carry_32));
    residues[196] = pack(&witness.left.bits[32..])
        .add(pack(&witness.right.bits[32..]))
        .add(witness.carry_32)
        .sub(pack(&witness.output.bits[32..]))
        .sub(radix.mul(witness.carry_64));
    residues
}

/// Evaluate every XOR/rotation numerator without branching on witness values.
///
/// The rotation is fixed by the operation schedule, never a prover-selected
/// integer. Order: active Booleanity, 192 activated bit constraints, then 64
/// exact XOR equations using `a XOR b = a + b - 2ab` on Boolean operands.
#[must_use]
pub fn xor_rotate_residues<F: IntegerAirField>(
    active: F,
    rotation: Blake2bRotation,
    witness: &XorRotate64Witness<F>,
) -> [F; XOR_ROTATE_CONSTRAINT_COUNT] {
    let mut residues = [F::ZERO; XOR_ROTATE_CONSTRAINT_COUNT];
    residues[0] = active.mul(active.sub(F::ONE));
    for (index, word) in [&witness.left, &witness.right, &witness.output]
        .iter()
        .enumerate()
    {
        bit_residues(
            active,
            word,
            &mut residues[1 + index * 64..1 + (index + 1) * 64],
        );
    }
    for bit in 0..64 {
        let source = (bit + rotation.bits() as usize) % 64;
        let left = witness.left.bits[source];
        let right = witness.right.bits[source];
        let product = left.mul(right);
        residues[193 + bit] =
            witness.output.bits[bit].sub(left.add(right).sub(product.add(product)));
    }
    residues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GOLDILOCKS_MODULUS_V1, GoldilocksFp4V1};

    fn zero<const N: usize>(residues: [u64; N]) -> bool {
        residues.iter().all(|&residue| residue == 0)
    }

    fn decode(word: &BitWord64) -> u64 {
        word.bits
            .iter()
            .enumerate()
            .fold(0, |value, (bit, &set)| value | (set << bit))
    }

    #[test]
    fn addition_proves_integer_wrap_and_both_carries_at_boundaries() {
        let edges = [
            0,
            1,
            u32::MAX as u64,
            1 << 32,
            (1 << 56) - 1,
            1 << 56,
            GOLDILOCKS_MODULUS_V1 - 1,
            GOLDILOCKS_MODULUS_V1,
            u64::MAX,
        ];
        for left in edges {
            for right in edges {
                let witness = Add64Witness::from_operands(left, right);
                assert!(zero(add_residues(1, &witness)), "{left}+{right}");
                assert_eq!(decode(&witness.output), left.wrapping_add(right));
                assert_eq!(witness.carry_64, u64::from(left.overflowing_add(right).1));
            }
        }
        let mut alias = Add64Witness::from_operands(GOLDILOCKS_MODULUS_V1, 0);
        alias.output = BitWord64::from_integer(0);
        assert!(
            !zero(add_residues(1, &alias)),
            "field alias must not be an integer sum"
        );
    }

    #[test]
    fn each_addition_bit_and_carry_is_constrained() {
        let original = Add64Witness::from_operands(u64::MAX, 1);
        for column in 0..194 {
            let mut changed = original;
            let value = match column {
                0..64 => &mut changed.left.bits[column],
                64..128 => &mut changed.right.bits[column - 64],
                128..192 => &mut changed.output.bits[column - 128],
                192 => &mut changed.carry_32,
                _ => &mut changed.carry_64,
            };
            *value ^= 1;
            assert!(
                !zero(add_residues(1, &changed)),
                "unconstrained column {column}"
            );
        }
        let mut non_boolean = original;
        non_boolean.left.bits[0] = 2;
        assert!(!zero(add_residues(1, &non_boolean)));
        assert!(!zero(add_residues(2, &original)));
    }

    #[test]
    fn xor_rotations_bind_every_bit_and_fixed_rotation() {
        let rotations = [
            Blake2bRotation::Ror32,
            Blake2bRotation::Ror24,
            Blake2bRotation::Ror16,
            Blake2bRotation::Ror63,
        ];
        for rotation in rotations {
            for bit in 0..64 {
                for (left, right) in [
                    (1 << bit, 0),
                    (0, 1 << bit),
                    (1 << bit, 1 << bit),
                    (u64::MAX, 1 << bit),
                ] {
                    let witness = XorRotate64Witness::from_operands(left, right, rotation);
                    assert!(zero(xor_rotate_residues(1, rotation, &witness)));
                    assert_eq!(
                        decode(&witness.output),
                        (left ^ right).rotate_right(rotation.bits())
                    );
                }
            }
            let original = XorRotate64Witness::from_operands(
                0x0123_4567_89ab_cdef,
                0xfedc_ba98_7654_3210,
                rotation,
            );
            for column in 0..192 {
                let mut changed = original;
                let value = match column {
                    0..64 => &mut changed.left.bits[column],
                    64..128 => &mut changed.right.bits[column - 64],
                    _ => &mut changed.output.bits[column - 128],
                };
                *value ^= 1;
                assert!(!zero(xor_rotate_residues(1, rotation, &changed)));
            }
            let asymmetric = XorRotate64Witness::from_operands(1, 0, rotation);
            for wrong in rotations.into_iter().filter(|&other| other != rotation) {
                assert!(!zero(xor_rotate_residues(1, wrong, &asymmetric)));
            }
        }
    }

    #[test]
    fn inactive_words_are_zero_and_exact_packed_limbs_preserve_all_bits() {
        assert!(zero(add_residues(0, &Add64Witness::inactive())));
        assert!(zero(xor_rotate_residues(
            0,
            Blake2bRotation::Ror63,
            &XorRotate64Witness::inactive()
        )));
        for bit in 0..64 {
            let mut add = Add64Witness::inactive();
            add.left.bits[bit] = 1;
            assert!(!zero(add_residues(0, &add)));
            let mut xor = XorRotate64Witness::inactive();
            xor.output.bits[bit] = 1;
            assert!(!zero(xor_rotate_residues(0, Blake2bRotation::Ror63, &xor)));
        }
        for value in [
            0,
            1,
            (1 << 56) - 1,
            1 << 56,
            GOLDILOCKS_MODULUS_V1,
            u64::MAX,
        ] {
            let [low, high] = BitWord64::from_integer(value).packed_limbs();
            assert_eq!(low | (high << 56), value);
            assert!(low < 1 << 56 && high < 256);
        }
    }

    #[test]
    fn base_and_extension_evaluators_agree_on_invalid_openings() {
        let embed = |word: BitWord64| BitWord64 {
            bits: word.bits.map(|v| GoldilocksFp4V1::from_base(v).unwrap()),
        };
        let mut add = Add64Witness::from_operands(u64::MAX, 123);
        add.output.bits[17] = 2;
        let extension = Add64Witness {
            left: embed(add.left),
            right: embed(add.right),
            output: embed(add.output),
            carry_32: GoldilocksFp4V1::from_base(add.carry_32).unwrap(),
            carry_64: GoldilocksFp4V1::from_base(add.carry_64).unwrap(),
        };
        assert_eq!(
            add_residues(1, &add).map(|v| GoldilocksFp4V1::from_base(v).unwrap()),
            add_residues(GoldilocksFp4V1::ONE, &extension)
        );
        let xor = XorRotate64Witness {
            left: add.left,
            right: add.right,
            output: add.output,
        };
        assert!(!zero(add_residues(1, &add)));
        assert!(!zero(xor_rotate_residues(1, Blake2bRotation::Ror24, &xor)));
        let extension = XorRotate64Witness {
            left: embed(xor.left),
            right: embed(xor.right),
            output: embed(xor.output),
        };
        assert_eq!(
            xor_rotate_residues(1, Blake2bRotation::Ror24, &xor)
                .map(|v| GoldilocksFp4V1::from_base(v).unwrap()),
            xor_rotate_residues(GoldilocksFp4V1::ONE, Blake2bRotation::Ror24, &extension)
        );
    }

    #[test]
    fn all_numerators_are_quadratic_on_arbitrary_affine_openings() {
        // A third finite difference vanishes for every quadratic, including
        // at non-Boolean openings where accidental selector gates add degree.
        fn third_difference<const N: usize>(samples: [[u64; N]; 4]) {
            for column in 0..N {
                let difference = IntegerAirField::sub(
                    samples[3][column],
                    IntegerAirField::mul(3_u64, samples[2][column]),
                );
                let difference = IntegerAirField::add(
                    difference,
                    IntegerAirField::mul(3_u64, samples[1][column]),
                );
                assert_eq!(
                    IntegerAirField::sub(difference, samples[0][column]),
                    0,
                    "cubic numerator at column {column}"
                );
            }
        }
        for seed in 1..=4 {
            let witness = |t: u64| {
                let word = |offset| BitWord64 {
                    bits: core::array::from_fn(|bit| {
                        (bit as u64 + offset + seed) * (t + 1) + seed * offset
                    }),
                };
                Add64Witness {
                    left: word(1),
                    right: word(3),
                    output: word(7),
                    carry_32: 11 + seed * t,
                    carry_64: 13 + (seed + 1) * t,
                }
            };
            third_difference(core::array::from_fn(|t| {
                add_residues(2 + t as u64, &witness(t as u64))
            }));
            for rotation in [
                Blake2bRotation::Ror32,
                Blake2bRotation::Ror24,
                Blake2bRotation::Ror16,
                Blake2bRotation::Ror63,
            ] {
                third_difference(core::array::from_fn(|t| {
                    let add = witness(t as u64);
                    xor_rotate_residues(
                        2 + t as u64,
                        rotation,
                        &XorRotate64Witness {
                            left: add.left,
                            right: add.right,
                            output: add.output,
                        },
                    )
                }));
            }
        }
        assert_eq!(MAX_CONSTRAINT_DEGREE, 2);
    }
}
