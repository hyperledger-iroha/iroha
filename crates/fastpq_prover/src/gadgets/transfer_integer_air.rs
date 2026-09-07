//! Quadratic AIR relations for exact unsigned transfer-row arithmetic.
//!
//! A Goldilocks element cannot represent every `u64` injectively. This gadget
//! instead binds the trace's two 56+8-bit packed limbs to 64 Boolean bits, then
//! checks addition in two 32-bit limbs with a Boolean carry and no final carry.
//! Each bounded limb equation has magnitude below `2^33`, so vanishing in the
//! Goldilocks field implies the corresponding integer equality without wrap.
//!
//! All returned values are polynomial constraint numerators, not host-side
//! acceptance decisions. They must be committed, independently combined and
//! divided by the row zerofier by the proof backend. Openings must have canonical
//! field encodings. Every packed value limb beyond index one must additionally
//! be constrained to zero on transfer rows by the surrounding variable-width AIR.
//!
//! TODO: Bind the inferred amount and direction to authenticated transfer pairs,
//! identities, ordering, cardinality, complete hash relations and state roots.
//! These row relations do not replace full witness replay or source anchoring.

use crate::{GoldilocksFp4V1, field};

/// Number of auxiliary columns, excluding existing old/new packed values.
pub const AUXILIARY_COLUMN_COUNT: usize = 196;
/// Number of fixed row-local constraint numerators returned by this gadget.
pub const CONSTRAINT_COUNT: usize = 205;
/// Maximum algebraic degree of a numerator in the trace-column variables.
pub const MAX_CONSTRAINT_DEGREE: usize = 2;
const WORD_BITS: usize = 64;
const PACKED_LOW_BITS: usize = 56;
const ARITHMETIC_LIMB_BITS: usize = 32;
const AMOUNT_LOW_COLUMN: usize = 192;
const AMOUNT_HIGH_COLUMN: usize = 193;
const CARRY_COLUMN: usize = 194;
const DEBIT_COLUMN: usize = 195;

/// Field operations needed to evaluate the same polynomials at base or extension points.
///
/// Implementations must obey the Goldilocks field laws, embed `u32` constants
/// injectively and use canonical representations. In particular, raw `u64`
/// openings must be checked by the decoder before this adapter is used.
pub trait IntegerAirField: Copy {
    /// Additive identity.
    const ZERO: Self;
    /// Multiplicative identity.
    const ONE: Self;
    /// Embed an unsigned 32-bit integer as a field constant.
    fn from_u32(value: u32) -> Self;
    /// Field addition.
    fn add(self, other: Self) -> Self;
    /// Field subtraction.
    fn sub(self, other: Self) -> Self;
    /// Field multiplication.
    fn mul(self, other: Self) -> Self;
}

impl IntegerAirField for u64 {
    const ZERO: Self = 0;
    const ONE: Self = 1;

    fn from_u32(value: u32) -> Self {
        Self::from(value)
    }

    fn add(self, other: Self) -> Self {
        field::add_base(self, other)
    }

    fn sub(self, other: Self) -> Self {
        field::sub_base(self, other)
    }

    fn mul(self, other: Self) -> Self {
        field::mul_base(self, other)
    }
}

impl IntegerAirField for GoldilocksFp4V1 {
    const ZERO: Self = Self::ZERO;
    const ONE: Self = Self::ONE;

    fn from_u32(value: u32) -> Self {
        Self::from_base(u64::from(value)).expect("u32 is a canonical Goldilocks constant")
    }

    fn add(self, other: Self) -> Self {
        Self::add(self, other)
    }

    fn sub(self, other: Self) -> Self {
        Self::sub(self, other)
    }

    fn mul(self, other: Self) -> Self {
        Self::mul(self, other)
    }
}

/// Two 56+8-bit packed columns and their unsigned little-endian bit decomposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unsigned64Witness<F = u64> {
    /// First seven bytes followed by the final byte, each in a separate field element.
    pub packed: [F; 2],
    /// Least-significant bit first; AIR evaluation does not assume Booleanity.
    pub bits: [F; WORD_BITS],
}

impl Unsigned64Witness<u64> {
    /// Generate a witness from an integer without reducing that integer modulo the field.
    #[must_use]
    pub fn from_integer(value: u64) -> Self {
        Self {
            packed: [
                value & ((1_u64 << PACKED_LOW_BITS) - 1),
                value >> PACKED_LOW_BITS,
            ],
            bits: core::array::from_fn(|bit| (value >> bit) & 1),
        }
    }
}

/// Witness columns for one transfer row, evaluated independently of native transcripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransferIntegerWitness<F = u64> {
    /// Balance immediately before this row's update.
    pub before: Unsigned64Witness<F>,
    /// Balance immediately after this row's update.
    pub after: Unsigned64Witness<F>,
    /// Unsigned absolute difference; correspondence with a transfer pair is separate.
    pub amount: Unsigned64Witness<F>,
    /// Carry from the low 32-bit addition into the high 32-bit addition.
    pub carry_32: F,
    /// One for `after + amount = before`, zero for `before + amount = after`.
    pub is_debit: F,
}

impl TransferIntegerWitness<u64> {
    /// Generate the exact integer witness for either direction of a balance change.
    ///
    /// Zero changes use credit direction. The amount is inferred, not authenticated.
    #[must_use]
    pub fn from_balances(before: u64, after: u64) -> Self {
        let is_debit = before > after;
        let amount = before.abs_diff(after);
        let low_addend = if is_debit { after } else { before } & u64::from(u32::MAX);
        let carry_32 = (low_addend + (amount & u64::from(u32::MAX))) >> ARITHMETIC_LIMB_BITS;
        Self {
            before: Unsigned64Witness::from_integer(before),
            after: Unsigned64Witness::from_integer(after),
            amount: Unsigned64Witness::from_integer(amount),
            carry_32,
            is_debit: u64::from(is_debit),
        }
    }
}

impl<F: IntegerAirField> TransferIntegerWitness<F> {
    /// Canonical auxiliary witness for metadata and padding rows.
    #[must_use]
    pub fn inactive() -> Self {
        let zero = Unsigned64Witness {
            packed: [F::ZERO; 2],
            bits: [F::ZERO; WORD_BITS],
        };
        Self {
            before: zero,
            after: zero,
            amount: zero,
            carry_32: F::ZERO,
            is_debit: F::ZERO,
        }
    }

    /// Reconstruct the row view from existing packed balances and auxiliary openings.
    #[must_use]
    pub fn from_auxiliary(
        before_packed: [F; 2],
        after_packed: [F; 2],
        auxiliary: &[F; AUXILIARY_COLUMN_COUNT],
    ) -> Self {
        Self {
            before: Unsigned64Witness {
                packed: before_packed,
                bits: core::array::from_fn(|bit| auxiliary[bit]),
            },
            after: Unsigned64Witness {
                packed: after_packed,
                bits: core::array::from_fn(|bit| auxiliary[WORD_BITS + bit]),
            },
            amount: Unsigned64Witness {
                packed: [auxiliary[AMOUNT_LOW_COLUMN], auxiliary[AMOUNT_HIGH_COLUMN]],
                bits: core::array::from_fn(|bit| auxiliary[2 * WORD_BITS + bit]),
            },
            carry_32: auxiliary[CARRY_COLUMN],
            is_debit: auxiliary[DEBIT_COLUMN],
        }
    }

    /// Return the auxiliary values in the order defined by [`auxiliary_column_names`].
    #[must_use]
    pub fn auxiliary_values(&self) -> [F; AUXILIARY_COLUMN_COUNT] {
        core::array::from_fn(|column| match column {
            0..64 => self.before.bits[column],
            64..128 => self.after.bits[column - WORD_BITS],
            128..192 => self.amount.bits[column - 2 * WORD_BITS],
            AMOUNT_LOW_COLUMN => self.amount.packed[0],
            AMOUNT_HIGH_COLUMN => self.amount.packed[1],
            CARRY_COLUMN => self.carry_32,
            DEBIT_COLUMN => self.is_debit,
            _ => unreachable!("auxiliary column has a fixed bound"),
        })
    }
}

/// Canonical order of the 196 transfer-only auxiliary trace columns.
#[must_use]
pub fn auxiliary_column_names() -> [String; AUXILIARY_COLUMN_COUNT] {
    core::array::from_fn(|column| match column {
        0..64 => format!("transfer_old_bit_{column}"),
        64..128 => format!("transfer_new_bit_{}", column - WORD_BITS),
        128..192 => format!("transfer_amount_bit_{}", column - 2 * WORD_BITS),
        AMOUNT_LOW_COLUMN => "transfer_amount_limb_0".to_owned(),
        AMOUNT_HIGH_COLUMN => "transfer_amount_limb_1".to_owned(),
        CARRY_COLUMN => "transfer_carry_32".to_owned(),
        DEBIT_COLUMN => "transfer_is_debit".to_owned(),
        _ => unreachable!("auxiliary column has a fixed bound"),
    })
}

fn reconstruct_bits<F: IntegerAirField>(bits: &[F]) -> F {
    let mut value = F::ZERO;
    let mut weight = F::ONE;
    for &bit in bits {
        value = value.add(bit.mul(weight));
        weight = weight.add(weight);
    }
    value
}

/// Evaluate 205 quadratic row-local numerators without native integer tests or replay.
///
/// Order: selector Booleanity; old/new exact byte lengths; then, for each of
/// before/after/amount, 64 activated-bit constraints and two packed bindings;
/// activated carry/direction constraints; low/high 32-bit arithmetic equations.
/// An activated bit satisfies `b * (b - s_transfer) = 0`, forcing zero on inactive
/// rows and Booleanity on transfer rows. Old/new packed bindings are gated so
/// opaque metadata values remain unconstrained by this transfer-only gadget.
///
/// All relations use only additions and multiplications over the supplied field,
/// so the same evaluator applies to arbitrary LDE and extension-field openings.
#[must_use]
pub fn constraint_residues<F: IntegerAirField>(
    s_transfer: F,
    old_value_len: F,
    new_value_len: F,
    witness: &TransferIntegerWitness<F>,
) -> [F; CONSTRAINT_COUNT] {
    let mut residues = [F::ZERO; CONSTRAINT_COUNT];
    residues[0] = s_transfer.mul(s_transfer.sub(F::ONE));
    residues[1] = s_transfer.mul(old_value_len.sub(F::from_u32(8)));
    residues[2] = s_transfer.mul(new_value_len.sub(F::from_u32(8)));
    let mut index = 3;
    for (value, binding_selector) in [
        (&witness.before, s_transfer),
        (&witness.after, s_transfer),
        (&witness.amount, F::ONE),
    ] {
        for &bit in &value.bits {
            residues[index] = bit.mul(bit.sub(s_transfer));
            index += 1;
        }
        residues[index] = binding_selector
            .mul(value.packed[0].sub(reconstruct_bits(&value.bits[..PACKED_LOW_BITS])));
        residues[index + 1] = binding_selector
            .mul(value.packed[1].sub(reconstruct_bits(&value.bits[PACKED_LOW_BITS..])));
        index += 2;
    }
    residues[index] = witness.carry_32.mul(witness.carry_32.sub(s_transfer));
    residues[index + 1] = witness.is_debit.mul(witness.is_debit.sub(s_transfer));
    index += 2;
    let radix = F::from_u32(u32::MAX).add(F::ONE);
    for half in 0..2 {
        let start = half * ARITHMETIC_LIMB_BITS;
        let end = start + ARITHMETIC_LIMB_BITS;
        let before = reconstruct_bits(&witness.before.bits[start..end]);
        let after = reconstruct_bits(&witness.after.bits[start..end]);
        let amount = reconstruct_bits(&witness.amount.bits[start..end]);
        let addend = before.add(witness.is_debit.mul(after.sub(before)));
        let result = after.add(witness.is_debit.mul(before.sub(after)));
        let difference = addend.add(amount).sub(result);
        residues[index + half] = if half == 0 {
            difference.sub(radix.mul(witness.carry_32))
        } else {
            difference.add(witness.carry_32)
        };
    }
    debug_assert_eq!(index + 2, CONSTRAINT_COUNT);
    residues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GOLDILOCKS_MODULUS_V1;

    fn valid(witness: &TransferIntegerWitness) -> bool {
        constraint_residues(1, 8, 8, witness)
            .iter()
            .all(|&residue| residue == 0)
    }

    #[test]
    fn base_adapter_preserves_full_u64_modulo_behavior() {
        let modulus = u128::from(GOLDILOCKS_MODULUS_V1);
        for left in [
            0,
            1,
            GOLDILOCKS_MODULUS_V1 - 1,
            GOLDILOCKS_MODULUS_V1,
            u64::MAX,
        ] {
            for right in [
                0,
                1,
                GOLDILOCKS_MODULUS_V1 - 1,
                GOLDILOCKS_MODULUS_V1,
                u64::MAX,
            ] {
                assert_eq!(
                    u128::from(IntegerAirField::add(left, right)),
                    (u128::from(left) + u128::from(right)) % modulus
                );
                assert_eq!(
                    u128::from(IntegerAirField::sub(left, right)),
                    (u128::from(left) + 2 * modulus - u128::from(right)) % modulus
                );
                assert_eq!(
                    u128::from(IntegerAirField::mul(left, right)),
                    (u128::from(left) * u128::from(right)) % modulus
                );
            }
        }
    }

    #[test]
    fn exact_u64_extremes_and_carries_satisfy_the_polynomials() {
        let boundaries = [
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
        for before in boundaries {
            for after in boundaries {
                let witness = TransferIntegerWitness::from_balances(before, after);
                assert!(valid(&witness), "{before} -> {after}");
                assert_eq!(
                    witness.before.packed,
                    crate::pack_bytes(&before.to_le_bytes()).limbs[..]
                );
                assert_eq!(
                    witness.after.packed,
                    crate::pack_bytes(&after.to_le_bytes()).limbs[..]
                );
            }
        }
        let carry = TransferIntegerWitness::from_balances(u64::from(u32::MAX), 1 << 32);
        assert_eq!(carry.carry_32, 1);
        assert_eq!(carry.is_debit, 0);
        let debit = TransferIntegerWitness::from_balances(1 << 32, u64::from(u32::MAX));
        assert_eq!(debit.carry_32, 1);
        assert_eq!(debit.is_debit, 1);
    }

    #[test]
    fn modular_field_wrap_cannot_masquerade_as_unsigned_arithmetic() {
        let mut credit = TransferIntegerWitness::from_balances(GOLDILOCKS_MODULUS_V1 - 1, 0);
        credit.amount = Unsigned64Witness::from_integer(1);
        credit.is_debit = 0;
        credit.carry_32 = 0;
        assert_eq!(IntegerAirField::add(GOLDILOCKS_MODULUS_V1 - 1, 1), 0);
        assert!(!valid(&credit));

        let mut debit = TransferIntegerWitness::from_balances(0, GOLDILOCKS_MODULUS_V1 - 1);
        debit.amount = Unsigned64Witness::from_integer(1);
        debit.is_debit = 1;
        debit.carry_32 = 0;
        assert!(!valid(&debit));
    }

    #[test]
    fn final_carry_cannot_hide_u64_overflow_or_underflow() {
        for is_debit in [0, 1] {
            let (before, after) = if is_debit == 0 {
                (u64::MAX, 0)
            } else {
                (0, u64::MAX)
            };
            let mut witness = TransferIntegerWitness::from_balances(before, after);
            witness.amount = Unsigned64Witness::from_integer(1);
            witness.is_debit = is_debit;
            witness.carry_32 = 1;
            let residues = constraint_residues(1, 8, 8, &witness);
            assert_eq!(
                residues[CONSTRAINT_COUNT - 2],
                0,
                "low carry equation holds"
            );
            assert_ne!(
                residues[CONSTRAINT_COUNT - 1],
                0,
                "high overflow must be rejected"
            );
        }
    }

    #[test]
    fn malformed_ranges_carry_direction_and_lengths_are_rejected() {
        let original = TransferIntegerWitness::from_balances(7, 9);
        for column in 0..AUXILIARY_COLUMN_COUNT {
            let mut auxiliary = original.auxiliary_values();
            auxiliary[column] = IntegerAirField::add(auxiliary[column], 2);
            let candidate = TransferIntegerWitness::from_auxiliary(
                original.before.packed,
                original.after.packed,
                &auxiliary,
            );
            assert!(
                !valid(&candidate),
                "auxiliary column {column} must be bound"
            );
        }
        for length in [0, 7, 9, 16] {
            assert!(
                constraint_residues(1, length, 8, &original)
                    .iter()
                    .any(|&r| r != 0)
            );
            assert!(
                constraint_residues(1, 8, length, &original)
                    .iter()
                    .any(|&r| r != 0)
            );
        }
        assert!(
            constraint_residues(2, 8, 8, &original)
                .iter()
                .any(|&r| r != 0)
        );
    }

    #[test]
    fn noncanonical_packed_limb_alias_is_rejected() {
        let mut candidate = TransferIntegerWitness::from_balances(1 << 56, (1 << 56) + 1);
        candidate.before.packed = [1 << 56, 0];
        assert!(
            !valid(&candidate),
            "equivalent integer must not bypass the 56-bit low-limb range"
        );
        let mut candidate = TransferIntegerWitness::from_balances(0, 1);
        candidate.after.packed[1] = 256;
        assert!(
            !valid(&candidate),
            "final packed limb must fit exactly eight bits"
        );
    }

    #[test]
    fn inactive_rows_require_zero_auxiliaries_but_allow_opaque_values() {
        let mut inactive = TransferIntegerWitness::<u64>::inactive();
        inactive.before.packed = [123, 456];
        inactive.after.packed = [789, 987];
        assert!(
            constraint_residues(0, 21, 19, &inactive)
                .iter()
                .all(|&r| r == 0)
        );
        for column in 0..AUXILIARY_COLUMN_COUNT {
            let mut auxiliary = inactive.auxiliary_values();
            auxiliary[column] = 1;
            let candidate = TransferIntegerWitness::from_auxiliary(
                inactive.before.packed,
                inactive.after.packed,
                &auxiliary,
            );
            assert!(
                constraint_residues(0, 21, 19, &candidate)
                    .iter()
                    .any(|&r| r != 0),
                "inactive auxiliary {column}"
            );
        }
    }

    #[test]
    fn auxiliary_schema_round_trips_without_duplicate_names() {
        let witness = TransferIntegerWitness::from_balances(u64::MAX, 17);
        assert_eq!(
            TransferIntegerWitness::from_auxiliary(
                witness.before.packed,
                witness.after.packed,
                &witness.auxiliary_values()
            ),
            witness
        );
        let names = auxiliary_column_names();
        assert_eq!(
            names
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            AUXILIARY_COLUMN_COUNT
        );
        assert_eq!(names[0], "transfer_old_bit_0");
        assert_eq!(names[195], "transfer_is_debit");
    }

    #[test]
    fn base_and_extension_evaluators_agree_on_every_residue() {
        let witness = TransferIntegerWitness::from_balances(u64::MAX, 17);
        let lift = |value| GoldilocksFp4V1::from_base(value).expect("canonical test opening");
        let mut auxiliary = witness.auxiliary_values();
        auxiliary[37] = 17;
        let base = TransferIntegerWitness::from_auxiliary(
            witness.before.packed,
            witness.after.packed,
            &auxiliary,
        );
        let extension = TransferIntegerWitness::from_auxiliary(
            witness.before.packed.map(lift),
            witness.after.packed.map(lift),
            &auxiliary.map(lift),
        );
        assert_eq!(
            constraint_residues(lift(1), lift(8), lift(8), &extension),
            constraint_residues(1, 8, 8, &base).map(lift)
        );
    }

    #[test]
    fn arbitrary_openings_obey_the_quadratic_degree_bound() {
        assert_eq!(MAX_CONSTRAINT_DEGREE, 2);
        let evaluate = |t: u64| {
            let auxiliary = core::array::from_fn(|index| {
                IntegerAirField::add(
                    (index as u64) * 17 + 3,
                    IntegerAirField::mul(t, (index as u64) * 11 + 7),
                )
            });
            let witness = TransferIntegerWitness::from_auxiliary(
                [t.add(9), t.mul(13).add(19)],
                [t.add(31), t.mul(37).add(41)],
                &auxiliary,
            );
            constraint_residues(t.add(2), t.add(8), t.mul(3).add(8), &witness)
        };
        let rows = [evaluate(0), evaluate(1), evaluate(2), evaluate(3)];
        for index in 0..CONSTRAINT_COUNT {
            let third_difference = rows[3][index]
                .sub(rows[2][index].mul(3))
                .add(rows[1][index].mul(3))
                .sub(rows[0][index]);
            assert_eq!(third_difference, 0, "constraint {index} exceeds degree two");
        }
    }
}
