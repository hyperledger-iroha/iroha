//! Quadratic, fully linked BLAKE2b G-function constraint composition.
//!
//! The schedule is fixed by RFC 7693 sections 2.1 and 3.1:
//! <https://www.rfc-editor.org/rfc/rfc7693#section-3.1>.
//! Six wrapping additions and four XOR/right-rotations are constrained by the
//! shared bitwise ARX gadget. Every operand is linked bit for bit to its preceding
//! register value or explicit input, and all four final registers are linked to
//! explicit outputs. The selector activates the entire G invocation. Inactive
//! operation words are zero, and the links force inactive inputs/outputs to zero.
//!
//! This is an unintegrated algebraic building block. Its direct witness contains
//! 2,572 field cells, plus one selector, and 3,746 numerator constraints. It cannot
//! be placed in one row under the current 512-column trace limit. The ten ARX
//! operations could occupy ten scheduled rows, with at most 194 operand/output/
//! carry cells per operation, but register and message binding would need a
//! separately constrained layout. No production trace limit is changed here.
//!
//! TODO: Commit and degree-prove the operation/register layout, connect the full
//! twelve-round compression schedule, IV, message schedule, byte counters,
//! final-block flag and complete hash output. This G relation alone proves no
//! full compression function or SMT path and does not replace witness replay.

use super::{
    arx64_air::{
        self, ADD_CONSTRAINT_COUNT, Add64Witness, BitWord64, Blake2bRotation, WORD_BITS,
        XOR_ROTATE_CONSTRAINT_COUNT, XorRotate64Witness,
    },
    transfer_integer_air::IntegerAirField,
};

/// Wrapping additions in the fixed G schedule, splitting each three-word sum in two.
pub const ADDITION_COUNT: usize = 6;
/// XOR/right-rotation operations in the fixed G schedule.
pub const XOR_ROTATION_COUNT: usize = 4;
/// ARX operation rows required before adding a register/message binding layout.
pub const OPERATION_ROW_COUNT: usize = ADDITION_COUNT + XOR_ROTATION_COUNT;
/// Complete operand and final-output word equalities in this composition.
pub const LINKED_WORD_COUNT: usize = 24;
/// Direct witness cells, excluding the one shared selector and any trace bookkeeping.
pub const WITNESS_CELL_COUNT: usize =
    10 * WORD_BITS + ADDITION_COUNT * (3 * WORD_BITS + 2) + XOR_ROTATION_COUNT * 3 * WORD_BITS;
/// All operation numerators plus complete bitwise operand/output linkage.
pub const CONSTRAINT_COUNT: usize = ADDITION_COUNT * ADD_CONSTRAINT_COUNT
    + XOR_ROTATION_COUNT * XOR_ROTATE_CONSTRAINT_COUNT
    + LINKED_WORD_COUNT * WORD_BITS;
/// Maximum numerator degree in every witness or selector variable.
pub const MAX_CONSTRAINT_DEGREE: usize = 2;

const ROTATIONS: [Blake2bRotation; XOR_ROTATION_COUNT] = [
    Blake2bRotation::Ror32,
    Blake2bRotation::Ror24,
    Blake2bRotation::Ror16,
    Blake2bRotation::Ror63,
];

/// Four exact bitwise registers participating in one G invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GRegisters<F = u64> {
    /// First register.
    pub a: BitWord64<F>,
    /// Second register.
    pub b: BitWord64<F>,
    /// Third register.
    pub c: BitWord64<F>,
    /// Fourth register.
    pub d: BitWord64<F>,
}

impl GRegisters<u64> {
    /// Decompose all four integers without reducing them into the base field.
    #[must_use]
    pub fn from_integers([a, b, c, d]: [u64; 4]) -> Self {
        Self {
            a: BitWord64::from_integer(a),
            b: BitWord64::from_integer(b),
            c: BitWord64::from_integer(c),
            d: BitWord64::from_integer(d),
        }
    }
}

impl<F: IntegerAirField> GRegisters<F> {
    /// Canonical zero registers for an inactive invocation.
    #[must_use]
    pub fn inactive() -> Self {
        Self {
            a: BitWord64::zero(),
            b: BitWord64::zero(),
            c: BitWord64::zero(),
            d: BitWord64::zero(),
        }
    }
}

/// Explicit input/output registers and every intermediate ARX witness for one G.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Blake2bGWitness<F = u64> {
    /// Register values before the invocation.
    pub inputs: GRegisters<F>,
    /// Exact message words `x` and `y`, in that order.
    pub message: [BitWord64<F>; 2],
    /// Register values after the invocation.
    pub outputs: GRegisters<F>,
    /// Sums in order: `a+b`, `previous+x`, `c+d`, `a+b`, `previous+y`, `c+d`.
    pub additions: [Add64Witness<F>; ADDITION_COUNT],
    /// XOR/rotations in order: `d/32`, `b/24`, `d/16`, `b/63`.
    pub xor_rotations: [XorRotate64Witness<F>; XOR_ROTATION_COUNT],
}

impl Blake2bGWitness<u64> {
    /// Generate every intermediate from the fixed integer G schedule.
    ///
    /// Native witness generation is not a validity decision. Verification must
    /// enforce [`constraint_residues`] and bind these explicit inputs/outputs.
    #[must_use]
    pub fn from_inputs([a, b, c, d]: [u64; 4], [x, y]: [u64; 2]) -> Self {
        let a_sum_0 = a.wrapping_add(b);
        let a_0 = a_sum_0.wrapping_add(x);
        let d_0 = (d ^ a_0).rotate_right(32);
        let c_0 = c.wrapping_add(d_0);
        let b_0 = (b ^ c_0).rotate_right(24);
        let a_sum_1 = a_0.wrapping_add(b_0);
        let a_1 = a_sum_1.wrapping_add(y);
        let d_1 = (d_0 ^ a_1).rotate_right(16);
        let c_1 = c_0.wrapping_add(d_1);
        let b_1 = (b_0 ^ c_1).rotate_right(63);
        Self {
            inputs: GRegisters::from_integers([a, b, c, d]),
            message: [BitWord64::from_integer(x), BitWord64::from_integer(y)],
            outputs: GRegisters::from_integers([a_1, b_1, c_1, d_1]),
            additions: [
                Add64Witness::from_operands(a, b),
                Add64Witness::from_operands(a_sum_0, x),
                Add64Witness::from_operands(c, d_0),
                Add64Witness::from_operands(a_0, b_0),
                Add64Witness::from_operands(a_sum_1, y),
                Add64Witness::from_operands(c_0, d_1),
            ],
            xor_rotations: [
                XorRotate64Witness::from_operands(d, a_0, ROTATIONS[0]),
                XorRotate64Witness::from_operands(b, c_0, ROTATIONS[1]),
                XorRotate64Witness::from_operands(d_0, a_1, ROTATIONS[2]),
                XorRotate64Witness::from_operands(b_0, c_1, ROTATIONS[3]),
            ],
        }
    }
}

impl<F: IntegerAirField> Blake2bGWitness<F> {
    /// Canonical all-zero witness for an inactive invocation.
    #[must_use]
    pub fn inactive() -> Self {
        Self {
            inputs: GRegisters::inactive(),
            message: [BitWord64::zero(); 2],
            outputs: GRegisters::inactive(),
            additions: [Add64Witness::inactive(); ADDITION_COUNT],
            xor_rotations: [XorRotate64Witness::inactive(); XOR_ROTATION_COUNT],
        }
    }
}

/// Evaluate the complete G schedule as polynomial numerators at arbitrary openings.
///
/// Order: six addition constraint sets, four fixed-rotation constraint sets,
/// twenty operand-word links in operation order, then four output-word links.
/// All links are unconditional linear differences. Activated ARX Booleanity
/// therefore forces external inputs and outputs to zero on inactive rows too.
/// Every numerator must be combined and divided by its applicable row zerofier;
/// a caller must also bind the registers/message to the surrounding computation.
#[must_use]
pub fn constraint_residues<F: IntegerAirField>(active: F, witness: &Blake2bGWitness<F>) -> Vec<F> {
    let mut residues = Vec::with_capacity(CONSTRAINT_COUNT);
    for addition in &witness.additions {
        residues.extend(arx64_air::add_residues(active, addition));
    }
    for (rotation, xor) in ROTATIONS.into_iter().zip(&witness.xor_rotations) {
        residues.extend(arx64_air::xor_rotate_residues(active, rotation, xor));
    }
    let add = &witness.additions;
    let xor = &witness.xor_rotations;
    let links = [
        (&add[0].left, &witness.inputs.a),
        (&add[0].right, &witness.inputs.b),
        (&add[1].left, &add[0].output),
        (&add[1].right, &witness.message[0]),
        (&xor[0].left, &witness.inputs.d),
        (&xor[0].right, &add[1].output),
        (&add[2].left, &witness.inputs.c),
        (&add[2].right, &xor[0].output),
        (&xor[1].left, &witness.inputs.b),
        (&xor[1].right, &add[2].output),
        (&add[3].left, &add[1].output),
        (&add[3].right, &xor[1].output),
        (&add[4].left, &add[3].output),
        (&add[4].right, &witness.message[1]),
        (&xor[2].left, &xor[0].output),
        (&xor[2].right, &add[4].output),
        (&add[5].left, &add[2].output),
        (&add[5].right, &xor[2].output),
        (&xor[3].left, &xor[1].output),
        (&xor[3].right, &add[5].output),
        (&witness.outputs.a, &add[4].output),
        (&witness.outputs.b, &xor[3].output),
        (&witness.outputs.c, &add[5].output),
        (&witness.outputs.d, &xor[2].output),
    ];
    for (left, right) in links {
        residues.extend(
            left.bits
                .iter()
                .zip(right.bits)
                .map(|(&left, right)| left.sub(right)),
        );
    }
    residues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GOLDILOCKS_MODULUS_V1, GoldilocksFp4V1};

    fn decode(word: BitWord64) -> u64 {
        word.bits
            .iter()
            .enumerate()
            .fold(0, |value, (bit, &set)| value | (set << bit))
    }

    fn decode_registers(registers: GRegisters) -> [u64; 4] {
        [registers.a, registers.b, registers.c, registers.d].map(decode)
    }

    // Independent integer reference: one u128 three-word sum per half, and an
    // explicit shift/or rotation instead of the witness generator's operations.
    fn reference(mut words: [u64; 4], message: [u64; 2]) -> [u64; 4] {
        let mask = u128::from(u64::MAX);
        let rotate = |word: u64, count| (word >> count) | (word << (64 - count));
        for (word, [d_rotation, b_rotation]) in message.into_iter().zip([[32, 24], [16, 63]]) {
            words[0] =
                ((u128::from(words[0]) + u128::from(words[1]) + u128::from(word)) & mask) as u64;
            words[3] = rotate(words[3] ^ words[0], d_rotation);
            words[2] = ((u128::from(words[2]) + u128::from(words[3])) & mask) as u64;
            words[1] = rotate(words[1] ^ words[2], b_rotation);
        }
        words
    }

    fn valid(active: u64, witness: &Blake2bGWitness) -> bool {
        let residues = constraint_residues(active, witness);
        assert_eq!(residues.len(), CONSTRAINT_COUNT);
        residues.iter().all(|&value| value == 0)
    }

    fn map_word<F: Copy>(word: BitWord64, map: &mut impl FnMut(u64) -> F) -> BitWord64<F> {
        BitWord64 {
            bits: word.bits.map(map),
        }
    }

    fn map_registers<F: Copy>(
        registers: GRegisters,
        map: &mut impl FnMut(u64) -> F,
    ) -> GRegisters<F> {
        GRegisters {
            a: map_word(registers.a, map),
            b: map_word(registers.b, map),
            c: map_word(registers.c, map),
            d: map_word(registers.d, map),
        }
    }

    fn map_witness<F: Copy>(
        witness: Blake2bGWitness,
        mut map: impl FnMut(u64) -> F,
    ) -> Blake2bGWitness<F> {
        Blake2bGWitness {
            inputs: map_registers(witness.inputs, &mut map),
            message: witness.message.map(|word| map_word(word, &mut map)),
            outputs: map_registers(witness.outputs, &mut map),
            additions: witness.additions.map(|add| Add64Witness {
                left: map_word(add.left, &mut map),
                right: map_word(add.right, &mut map),
                output: map_word(add.output, &mut map),
                carry_32: map(add.carry_32),
                carry_64: map(add.carry_64),
            }),
            xor_rotations: witness.xor_rotations.map(|xor| XorRotate64Witness {
                left: map_word(xor.left, &mut map),
                right: map_word(xor.right, &mut map),
                output: map_word(xor.output, &mut map),
            }),
        }
    }

    fn sample() -> Blake2bGWitness {
        Blake2bGWitness::from_inputs(
            [u64::MAX, GOLDILOCKS_MODULUS_V1, 0x0123_4567_89ab_cdef, 1],
            [0xfedc_ba98_7654_3210, 1 << 32],
        )
    }

    #[test]
    fn boundary_and_seeded_vectors_match_independent_integer_g() {
        let edges = [
            0,
            1,
            u64::from(u32::MAX),
            1 << 32,
            (1 << 56) - 1,
            1 << 56,
            GOLDILOCKS_MODULUS_V1 - 1,
            GOLDILOCKS_MODULUS_V1,
            u64::MAX,
        ];
        for left in edges {
            for right in edges {
                let inputs = [left, right, !left, left.rotate_left(17)];
                let message = [right, !right];
                let witness = Blake2bGWitness::from_inputs(inputs, message);
                assert_eq!(decode_registers(witness.inputs), inputs);
                assert_eq!(witness.message.map(decode), message);
                assert_eq!(
                    decode_registers(witness.outputs),
                    reference(inputs, message)
                );
                assert!(valid(1, &witness), "boundary {inputs:?}, {message:?}");
            }
        }
        let mut seed = 0x7654_3210_fedc_ba98_u64;
        for _ in 0..128 {
            let values: [u64; 6] = core::array::from_fn(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                seed
            });
            let inputs = [values[0], values[1], values[2], values[3]];
            let message = [values[4], values[5]];
            let witness = Blake2bGWitness::from_inputs(inputs, message);
            assert_eq!(
                decode_registers(witness.outputs),
                reference(inputs, message)
            );
            assert!(valid(1, &witness));
        }
    }

    #[test]
    fn every_intermediate_input_output_bit_and_carry_is_constrained() {
        let original = sample();
        assert!(valid(1, &original));
        for cell in 0..WITNESS_CELL_COUNT {
            let mut index = 0;
            let changed = map_witness(original, |value| {
                let next = if index == cell { value ^ 1 } else { value };
                index += 1;
                next
            });
            assert_eq!(index, WITNESS_CELL_COUNT);
            assert!(!valid(1, &changed), "unconstrained witness cell {cell}");
        }
    }

    #[test]
    fn locally_valid_replacement_operations_fail_complete_register_links() {
        let original = sample();
        for index in 0..ADDITION_COUNT {
            let mut changed = original;
            let old = changed.additions[index];
            changed.additions[index] =
                Add64Witness::from_operands(decode(old.left).wrapping_add(1), decode(old.right));
            assert!(
                arx64_air::add_residues(1, &changed.additions[index])
                    .iter()
                    .all(|&value| value == 0)
            );
            assert!(!valid(1, &changed), "detached addition {index}");
        }
        for (index, rotation) in ROTATIONS.into_iter().enumerate() {
            let mut changed = original;
            let old = changed.xor_rotations[index];
            changed.xor_rotations[index] = XorRotate64Witness::from_operands(
                decode(old.left) ^ 1,
                decode(old.right),
                rotation,
            );
            assert!(
                arx64_air::xor_rotate_residues(1, rotation, &changed.xor_rotations[index])
                    .iter()
                    .all(|&value| value == 0)
            );
            assert!(!valid(1, &changed), "detached XOR/rotation {index}");
        }
    }

    #[test]
    fn inactive_invocation_requires_every_cell_to_be_zero() {
        let inactive = Blake2bGWitness::inactive();
        assert!(valid(0, &inactive));
        assert!(
            valid(1, &inactive),
            "all-zero inputs have an all-zero G output"
        );
        assert!(!valid(2, &inactive), "non-Boolean selector must fail");
        for cell in 0..WITNESS_CELL_COUNT {
            let mut index = 0;
            let changed = map_witness(inactive, |value| {
                let next = if index == cell { 1 } else { value };
                index += 1;
                next
            });
            assert!(!valid(0, &changed), "inactive witness cell {cell}");
        }
    }

    #[test]
    fn base_and_extension_evaluators_agree_on_non_boolean_openings() {
        let mut cell = 0;
        let invalid = map_witness(sample(), |value| {
            cell += 1;
            value + (cell % 7) as u64
        });
        let extension = map_witness(invalid, |value| GoldilocksFp4V1::from_base(value).unwrap());
        let base = constraint_residues(2, &invalid);
        assert!(base.iter().any(|&value| value != 0));
        assert_eq!(
            base.into_iter()
                .map(|value| GoldilocksFp4V1::from_base(value).unwrap())
                .collect::<Vec<_>>(),
            constraint_residues(GoldilocksFp4V1::from_base(2).unwrap(), &extension)
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
    fn all_operation_numerators_are_quadratic_and_all_links_are_linear() {
        let witness = map_witness(sample(), |_| Degree(1));
        let residues = constraint_residues(Degree(1), &witness);
        assert_eq!(residues.len(), CONSTRAINT_COUNT);
        assert_eq!(
            residues.iter().map(|value| value.0).max(),
            Some(MAX_CONSTRAINT_DEGREE)
        );
        assert!(
            residues[CONSTRAINT_COUNT - LINKED_WORD_COUNT * WORD_BITS..]
                .iter()
                .all(|value| value.0 == 1)
        );
        assert_eq!(WITNESS_CELL_COUNT, 2572);
        assert_eq!(CONSTRAINT_COUNT, 3746);
        assert_eq!(OPERATION_ROW_COUNT, 10);
    }
}
