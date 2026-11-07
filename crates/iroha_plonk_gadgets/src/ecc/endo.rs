//! Endoscalar multiplication: the GLV chain for a multiplier given directly
//! by its 64-bit digit words,
//!
//! ```text
//! alpha = (2^64 + 2 B1 + 1) + zeta (2^64 + 2 B2 + 1),   B1, B2 < 2^64
//! ```
//!
//! (Halo-style endoscaling; [`native::endoscalar`]). A verifier that draws a
//! 128-bit challenge limb `lo = B1 + 2^64 B2` and uses `alpha` as its scalar
//! multiplies by it in 64 iterations instead of the 128 of a general scalar,
//! with no split check.
//!
//! The chain is the GLV chain of the module documentation with 64 iterations
//! on the same gates: the guarded init row (running sums start at 0), 64
//! incomplete double-and-add rows, the row holding `acc_64` and the running
//! sums `Y1_64 = B1`, `Y2_64 = B2`, and the guarded output. After 64
//! iterations from `acc_0 = 2 S+` the accumulator is
//! `[(2^64 + 2 B1 + 1)] P + [(2^64 + 2 B2 + 1)] phi(P) = [alpha] P`: both low
//! bits are fixed to one, so there is no even-digit correction. Every
//! iteration `j <= 63` is exception-free by the lattice argument of the module
//! documentation (sup-norm at most `6 2^j - 1 < 2^66`, against the lattice
//! minimum `2^126.21`), so no complete tail is needed.
//!
//! The digit words are bounded by the running sums (`Y_0 = 0` and 64 boolean
//! steps give `Y_64 < 2^64`). Every chain copies its `Y1_64, Y2_64` onto the
//! caller's digit words, so all chains of one multiplier use the same digits;
//! the caller binds those words to its challenge (`lo = B1 + 2^64 B2`).
//!
//! Rows of one multiplication, relative to its first row `s` (68 rows, against
//! 142 for a guarded general multiplication):
//!
//! | rows | content |
//! | --- | --- |
//! | `s` | guarded init: `[is_id, inv, x_in, y_in, lambda_2P, lambda_S-]` |
//! | `s+1 ..= s+64` | incomplete iterations |
//! | `s+65` | `[x_64, y_64, B1, B2, -, -, x_P, y_P, x_-, y_-]` |
//! | `s+66` | the guard gate on `[is_id, -, x_64, y_64]` |
//! | `s+67` | result in `a2, a3` |

use iroha_pasta::{PastaCurve, PastaField};
use iroha_plonk::frontend::{Error, Region, Value};

use super::{
    AssignedPoint, EccChip,
    native::{ENDO_ITERATIONS, EndoChainWitness, endo_chain_witness},
};
use crate::cells::Word;

/// Rows of one endoscalar chain after its first row.
pub const ENDO_CHAIN_ROWS: usize = ENDO_ITERATIONS + 3;

/// `1` or `0` in `F`.
fn bit_field<F: PastaField>(bit: bool) -> F {
    if bit { F::ONE } else { F::ZERO }
}

/// The 128-bit limb `B1 + 2^64 B2` of two digit words' values.
fn limb_of<F: PastaField>(digits: [&Word<F>; 2]) -> Value<u128> {
    digits[0].value().zip(digits[1].value()).map(|(b1, b2)| {
        let low = |value: &F| {
            let limbs = value.to_canonical_limbs();
            u128::from(limbs[0]) | (u128::from(limbs[1]) << 64)
        };
        low(&b1) | (low(&b2) << 64)
    })
}

impl<C: PastaCurve> EccChip<C> {
    /// `[alpha] P` for the endoscalar `alpha` of the digit words `digits =
    /// [B1, B2]`, with the identity guarded. The chain's running sums are
    /// copied onto `digits`, which bounds them below `2^64`.
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn mul_endo(
        &mut self,
        region: &mut Region<'_, C::Base>,
        digits: [&Word<C::Base>; 2],
        point: &AssignedPoint<C::Base>,
    ) -> Result<AssignedPoint<C::Base>, Error> {
        let (out, chain) = self.endo_chain(region, point, limb_of(digits))?;
        for (copied, digit) in chain.iter().zip(digits) {
            region.constrain_equal(copied.cell(), digit.cell())?;
        }
        Ok(out)
    }

    /// The identity-guarded Horner chain `sum_i alpha^i P_i` (`P_{n-1}`
    /// first) for the endoscalar `alpha` of `digits`: `acc = [alpha] acc +
    /// P_i` with complete joins, each result read in place.
    ///
    /// # Errors
    ///
    /// [`Error::Synthesis`] for no points, and [`Error`] from the layout.
    pub fn horner_endo(
        &mut self,
        region: &mut Region<'_, C::Base>,
        digits: [&Word<C::Base>; 2],
        points: &[AssignedPoint<C::Base>],
    ) -> Result<AssignedPoint<C::Base>, Error> {
        let (last, rest) = points.split_last().ok_or(Error::Synthesis)?;
        let mut acc = last.clone();
        for point in rest.iter().rev() {
            let product = self.mul_endo(region, digits, &acc)?;
            acc = self.add(region, point, &product)?;
        }
        Ok(acc)
    }

    /// One guarded endoscalar chain; returns the result and the chain's final
    /// running sums `[Y1_64, Y2_64]`.
    fn endo_chain(
        &mut self,
        region: &mut Region<'_, C::Base>,
        input: &AssignedPoint<C::Base>,
        lo: Value<u128>,
    ) -> Result<(AssignedPoint<C::Base>, [Word<C::Base>; 2]), Error> {
        let witness = input
            .value()
            .zip(lo)
            .map(|(input, lo)| endo_chain_witness::<C>(input, lo));
        witness.error_if_known_and(Option::is_none)?;
        let witness: Value<EndoChainWitness<C::Base>> =
            witness.and_then(|witness| witness.map_or_else(Value::unknown, Value::known));
        let w = witness.as_ref();
        let (start, in_place) = self.begin(ENDO_CHAIN_ROWS + 1, Some(input))?;
        let selectors = self.config.selectors;
        // Guarded init row.
        selectors.init_guarded.enable(region, start)?;
        if !in_place {
            self.copy_point(region, input, 2, start)?;
        }
        let is_identity = self.assign(region, 0, start, w.map(|w| bit_field(w.is_identity)))?;
        self.assign(region, 1, start, w.map(|w| w.inverse))?;
        self.assign(region, 4, start, w.map(|w| w.lambda_double))?;
        self.assign(region, 5, start, w.map(|w| w.lambda_minus))?;
        // Iteration rows `start + 1 + j`, `j = 0..=64`.
        let mut last = None;
        for j in 0..=ENDO_ITERATIONS {
            let row = start + 1 + j;
            let y1 = self.assign(region, 2, row, w.map(|w| w.running[0][j]))?;
            let y2 = self.assign(region, 3, row, w.map(|w| w.running[1][j]))?;
            self.assign(region, 6, row, w.map(|w| w.base.0))?;
            self.assign(region, 7, row, w.map(|w| w.base.1))?;
            self.assign(region, 8, row, w.map(|w| w.minus.0))?;
            self.assign(region, 9, row, w.map(|w| w.minus.1))?;
            let acc = self.assign_point(region, 0, row, w.map(|w| w.acc[j]))?;
            if j < ENDO_ITERATIONS {
                selectors.incomplete.enable(region, row)?;
                self.assign(region, 4, row, w.map(|w| w.lambdas[j].0))?;
                self.assign(region, 5, row, w.map(|w| w.lambdas[j].1))?;
            } else {
                last = Some((acc, [y1, y2]));
            }
        }
        let (acc, digits) = last.ok_or(Error::Synthesis)?;
        // The guard: `is_id` and `acc_64`, the result on the next row.
        let guard_row = start + ENDO_ITERATIONS + 2;
        selectors.guard_out.enable(region, guard_row)?;
        self.copy(region, &is_identity, 0, guard_row)?;
        self.copy_point(region, &acc, 2, guard_row)?;
        let row = guard_row + 1;
        let out = self.assign_point(region, 2, row, w.map(|w| w.output))?;
        if row != start + ENDO_CHAIN_ROWS {
            return Err(Error::Synthesis);
        }
        self.pending = Some(row);
        Ok((out, digits))
    }
}
