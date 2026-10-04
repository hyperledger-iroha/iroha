//! Checked unsigned arithmetic on range-checked cells ([`Uint`], typically
//! [`U128`] and [`U64`]), port of the M8 `U128Circuit` semantics.
//!
//! Every operation computes its result with one glue row and range-checks it
//! with the running-sum chip, so a result that leaves `[0, 2^BITS)` has no
//! satisfying assignment:
//!
//! - [`UintChip::checked_add`]: `a + b < 2^(BITS+1) <= 2^129 < p`, so the field
//!   sum is the integer sum, and its range check rejects an overflow (a sum
//!   of exactly `2^BITS` included).
//! - [`UintChip::checked_sub`]: for `a < b` the field difference is
//!   `p - (b - a) > p - 2^128 >= 2^BITS`, which the range check rejects.
//! - [`UintChip::assert_le`] / [`UintChip::assert_lt`]: the range check of
//!   `b - a` (or `b - a - 1`).
//! - [`UintChip::lt`]: a boolean `bit` with `bit (b - a - 1) + (1 - bit)(a -
//!   b)` range-checked, so exactly one branch can hold.
//!
//! Inputs must already be [`Uint`]s of the same width: the types carry the
//! range proofs, and `BITS <= 128` is enforced at compile time. The witness
//! is laid out even when a native check fails (the native references
//! [`checked_add_native`], [`checked_sub_native`], ... say so); the circuit,
//! not the witness generator, rejects it.

use iroha_pasta::PastaField;
use iroha_plonk::frontend::{Error, Region, Value};

use crate::{
    arith::{Coefficients, GLUE_WIDTH, GlueChip, Slot},
    cells::{Bit, U64, U128, Uint, Word},
    range::running_sum::RunningSumChip,
};

/// Whether `value < 2^bits` (every `u128` fits 128 bits or more).
#[must_use]
pub const fn fits(bits: usize, value: u128) -> bool {
    bits >= 128 || value < (1_u128 << bits)
}

/// Native reference of [`UintChip::checked_add`]: `None` when an operand or
/// the sum does not fit `bits` bits.
#[must_use]
pub fn checked_add_native(bits: usize, a: u128, b: u128) -> Option<u128> {
    if !fits(bits, a) || !fits(bits, b) {
        return None;
    }
    a.checked_add(b).filter(|sum| fits(bits, *sum))
}

/// Native reference of [`UintChip::checked_sub`]: `None` when an operand
/// does not fit or `a < b`.
#[must_use]
pub fn checked_sub_native(bits: usize, a: u128, b: u128) -> Option<u128> {
    if !fits(bits, a) || !fits(bits, b) {
        return None;
    }
    a.checked_sub(b)
}

/// Native reference of [`UintChip::lt`] (and of the [`UintChip::assert_lt`]
/// and [`UintChip::assert_le`] predicates): `None` when an operand does not
/// fit.
#[must_use]
pub const fn lt_native(bits: usize, a: u128, b: u128) -> Option<bool> {
    if fits(bits, a) && fits(bits, b) {
        Some(a < b)
    } else {
        None
    }
}

/// A view of a glue chip and a running-sum chip that performs checked
/// unsigned arithmetic.
#[derive(Debug)]
pub struct UintChip<'a, F: PastaField> {
    glue: &'a mut GlueChip<F>,
    range: &'a mut RunningSumChip<F>,
}

impl<'a, F: PastaField> UintChip<'a, F> {
    /// A view over `glue` and `range`.
    pub const fn new(glue: &'a mut GlueChip<F>, range: &'a mut RunningSumChip<F>) -> Self {
        Self { glue, range }
    }

    /// The glue chip.
    pub fn glue(&mut self) -> &mut GlueChip<F> {
        &mut *self.glue
    }

    /// The running-sum chip.
    pub fn range(&mut self) -> &mut RunningSumChip<F> {
        &mut *self.range
    }

    /// A new `BITS`-bit witness (range-checked where it is assigned).
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn assign<const BITS: usize>(
        &mut self,
        region: &mut Region<'_, F>,
        value: Value<u128>,
    ) -> Result<Uint<F, BITS>, Error> {
        const { assert!(BITS >= 1 && BITS <= 128, "Uint widths are 1..=128 bits") };
        let value = value.map(F::from_u128);
        self.range
            .witness_range_checked(region, value, BITS)
            .map(Uint::new)
    }

    /// Range-checks `word` to `BITS` bits and returns it as a [`Uint`].
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn range_check<const BITS: usize>(
        &mut self,
        region: &mut Region<'_, F>,
        word: &Word<F>,
    ) -> Result<Uint<F, BITS>, Error> {
        const { assert!(BITS >= 1 && BITS <= 128, "Uint widths are 1..=128 bits") };
        self.range.range_check(region, word, BITS)?;
        Ok(Uint::new(word.clone()))
    }

    /// A constant, pinned by the glue gate (no range check is needed).
    ///
    /// # Errors
    ///
    /// [`Error::Synthesis`] when `value` does not fit `BITS` bits, and
    /// [`Error`] from the layout.
    pub fn constant<const BITS: usize>(
        &mut self,
        region: &mut Region<'_, F>,
        value: u128,
    ) -> Result<Uint<F, BITS>, Error> {
        const { assert!(BITS >= 1 && BITS <= 128, "Uint widths are 1..=128 bits") };
        if !fits(BITS, value) {
            return Err(Error::Synthesis);
        }
        self.glue
            .constant(region, F::from_u128(value))
            .map(Uint::new)
    }

    /// Retypes a narrower value as a wider one (no constraint is needed).
    pub fn widen<const FROM: usize, const TO: usize>(value: &Uint<F, FROM>) -> Uint<F, TO> {
        const { assert!(FROM <= TO && TO <= 128, "widening only") };
        Uint::new(value.word().clone())
    }

    /// `a + b`, rejecting `a + b >= 2^BITS`.
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn checked_add<const BITS: usize>(
        &mut self,
        region: &mut Region<'_, F>,
        a: &Uint<F, BITS>,
        b: &Uint<F, BITS>,
    ) -> Result<Uint<F, BITS>, Error> {
        const { assert!(BITS >= 1 && BITS <= 128, "Uint widths are 1..=128 bits") };
        let sum = self.glue.add(region, a.word(), b.word())?;
        self.range.range_check(region, &sum, BITS)?;
        Ok(Uint::new(sum))
    }

    /// `a - b`, rejecting `a < b`.
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn checked_sub<const BITS: usize>(
        &mut self,
        region: &mut Region<'_, F>,
        a: &Uint<F, BITS>,
        b: &Uint<F, BITS>,
    ) -> Result<Uint<F, BITS>, Error> {
        const { assert!(BITS >= 1 && BITS <= 128, "Uint widths are 1..=128 bits") };
        let difference = self.glue.sub(region, a.word(), b.word())?;
        self.range.range_check(region, &difference, BITS)?;
        Ok(Uint::new(difference))
    }

    /// `a + constant`, rejecting an overflow (for example a sequence number
    /// advanced past `2^BITS - 1`).
    ///
    /// # Errors
    ///
    /// [`Error::Synthesis`] when `constant` does not fit `BITS` bits, and
    /// [`Error`] from the layout.
    pub fn checked_add_constant<const BITS: usize>(
        &mut self,
        region: &mut Region<'_, F>,
        a: &Uint<F, BITS>,
        constant: u128,
    ) -> Result<Uint<F, BITS>, Error> {
        const { assert!(BITS >= 1 && BITS <= 128, "Uint widths are 1..=128 bits") };
        if !fits(BITS, constant) {
            return Err(Error::Synthesis);
        }
        let sum = self
            .glue
            .add_constant(region, a.word(), F::from_u128(constant))?;
        self.range.range_check(region, &sum, BITS)?;
        Ok(Uint::new(sum))
    }

    /// Constrains `a <= b`.
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn assert_le<const BITS: usize>(
        &mut self,
        region: &mut Region<'_, F>,
        a: &Uint<F, BITS>,
        b: &Uint<F, BITS>,
    ) -> Result<(), Error> {
        const { assert!(BITS >= 1 && BITS <= 128, "Uint widths are 1..=128 bits") };
        let difference = self.glue.sub(region, b.word(), a.word())?;
        self.range
            .range_check(region, &difference, BITS)
            .map(|_| ())
    }

    /// Constrains `a < b`.
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn assert_lt<const BITS: usize>(
        &mut self,
        region: &mut Region<'_, F>,
        a: &Uint<F, BITS>,
        b: &Uint<F, BITS>,
    ) -> Result<(), Error> {
        const { assert!(BITS >= 1 && BITS <= 128, "Uint widths are 1..=128 bits") };
        let difference =
            self.glue
                .linear(region, &[(F::ONE, b.word()), (-F::ONE, a.word())], -F::ONE)?;
        self.range
            .range_check(region, &difference, BITS)
            .map(|_| ())
    }

    /// `[a < b]` as a [`Bit`].
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn lt<const BITS: usize>(
        &mut self,
        region: &mut Region<'_, F>,
        a: &Uint<F, BITS>,
        b: &Uint<F, BITS>,
    ) -> Result<Bit<F>, Error> {
        const { assert!(BITS >= 1 && BITS <= 128, "Uint widths are 1..=128 bits") };
        let less = a.value().zip(b.value()).map(|(a, b)| a < b);
        self.lt_with_witness(region, a, b, less)
    }

    /// [`Self::lt`] with the comparison bit supplied by the caller (tests
    /// force a false bit through it).
    pub(crate) fn lt_with_witness<const BITS: usize>(
        &mut self,
        region: &mut Region<'_, F>,
        a: &Uint<F, BITS>,
        b: &Uint<F, BITS>,
        less: Value<bool>,
    ) -> Result<Bit<F>, Error> {
        let t = self.glue.sub(region, b.word(), a.word())?;
        let bit = self.glue.boolean(region, less)?;
        // d = bit (b - a - 1) + (1 - bit)(a - b) = 2 bit t - bit - t.
        let d = bit
            .word()
            .value()
            .zip(t.value())
            .map(|(bit, t)| (bit + bit) * t - bit - t);
        let coefficients = Coefficients {
            m: F::from(2_u64),
            a: -F::ONE,
            b: -F::ONE,
            d: -F::ONE,
            ..Coefficients::zero()
        };
        let slots = [
            Slot::Copy(bit.word()),
            Slot::Copy(&t),
            Slot::Empty,
            Slot::Value(d),
        ];
        let mut words = self.glue.row(region, coefficients, slots, None)?;
        let d = words[GLUE_WIDTH - 1].take().ok_or(Error::Synthesis)?;
        self.range.range_check(region, &d, BITS)?;
        Ok(bit)
    }

    /// Constrains `a != 0`.
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn assert_nonzero<const BITS: usize>(
        &mut self,
        region: &mut Region<'_, F>,
        a: &Uint<F, BITS>,
    ) -> Result<(), Error> {
        self.glue.assert_nonzero(region, a.word())
    }

    /// A new [`U128`] witness.
    ///
    /// # Errors
    ///
    /// As [`Self::assign`].
    pub fn assign_u128(
        &mut self,
        region: &mut Region<'_, F>,
        value: Value<u128>,
    ) -> Result<U128<F>, Error> {
        self.assign::<128>(region, value)
    }

    /// A new [`U64`] witness.
    ///
    /// # Errors
    ///
    /// As [`Self::assign`].
    pub fn assign_u64(
        &mut self,
        region: &mut Region<'_, F>,
        value: Value<u64>,
    ) -> Result<U64<F>, Error> {
        self.assign::<64>(region, value.map(u128::from))
    }
}

#[cfg(test)]
mod tests {
    use iroha_pasta::Fp;
    use iroha_plonk::{
        check::{CheckMode, check_circuit},
        cs::ConstraintSystem,
        frontend::{Circuit, Layouter, SimpleFloorPlanner},
    };

    use super::*;
    use crate::{
        arith::GlueConfig,
        range::running_sum::{LimbBits, RunningSumConfig},
    };

    /// `lt(a, b)` laid out with a chosen comparison bit.
    #[derive(Clone, Copy)]
    struct ForcedLt {
        a: u128,
        b: u128,
        less: bool,
    }

    impl Circuit<Fp> for ForcedLt {
        type Config = (GlueConfig, RunningSumConfig);
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            *self
        }

        fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
            let advice = core::array::from_fn(|_| meta.advice_column());
            let constants = meta.fixed_column();
            let glue = GlueConfig::configure(meta, advice, constants);
            let z = meta.advice_column();
            let bits = LimbBits::new(4).unwrap_or_else(|| unreachable!("valid width"));
            (glue, RunningSumConfig::configure(meta, z, bits))
        }

        fn synthesize(
            &self,
            (glue, range): Self::Config,
            mut layouter: impl Layouter<Fp>,
        ) -> Result<(), Error> {
            let mut glue = GlueChip::new(glue);
            let mut range = RunningSumChip::new(range);
            range.load_table(&mut layouter)?;
            layouter.assign_region(
                || "forced lt",
                |mut region| {
                    let mut uint = UintChip::new(&mut glue, &mut range);
                    let a = uint.assign::<128>(&mut region, Value::known(self.a))?;
                    let b = uint.assign::<128>(&mut region, Value::known(self.b))?;
                    uint.lt_with_witness(&mut region, &a, &b, Value::known(self.less))
                        .map(|_| ())
                },
            )
        }
    }

    fn satisfied(circuit: ForcedLt) -> bool {
        check_circuit(&circuit, 8, &[], CheckMode::Strict)
            .expect("check")
            .is_satisfied()
    }

    #[test]
    fn a_false_comparison_bit_has_no_witness() {
        for (a, b) in [(3, 9), (9, 3), (5, 5), (0, u128::MAX), (u128::MAX, 0)] {
            let honest = lt_native(128, a, b).expect("in range");
            assert!(satisfied(ForcedLt { a, b, less: honest }), "{a} < {b}");
            assert!(
                !satisfied(ForcedLt {
                    a,
                    b,
                    less: !honest
                }),
                "{a} < {b} lie"
            );
        }
    }

    #[test]
    fn native_references() {
        assert!(fits(128, u128::MAX));
        assert!(fits(64, u128::from(u64::MAX)));
        assert!(!fits(64, 1 << 64));
        assert_eq!(checked_add_native(128, u128::MAX - 1, 1), Some(u128::MAX));
        assert_eq!(checked_add_native(128, u128::MAX, 1), None);
        assert_eq!(checked_add_native(64, u128::from(u64::MAX), 1), None);
        assert_eq!(checked_add_native(64, 1 << 64, 0), None);
        assert_eq!(checked_sub_native(128, 5, 5), Some(0));
        assert_eq!(checked_sub_native(128, 4, 5), None);
        assert_eq!(checked_sub_native(8, 256, 1), None);
        assert_eq!(lt_native(128, 4, 5), Some(true));
        assert_eq!(lt_native(128, 5, 5), Some(false));
        assert_eq!(lt_native(4, 16, 1), None);
    }
}
