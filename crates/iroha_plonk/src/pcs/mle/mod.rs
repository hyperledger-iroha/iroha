//! Multilinear openings of monomial-basis column commitments, for the
//! multilinear (v2) folding decider.
//!
//! A column `w` of `n = 2^k` values is committed as `C = <w, g[0..n)> + rho W`:
//! its values are the coefficients on the monomial generators (PLONK advice
//! is committed on the Lagrange basis instead). It is read as the multilinear
//! polynomial
//!
//! ```text
//! w(x) = sum_j eq(x, j) w_j,   eq(x, j) = prod_t (x_t j_t + (1 - x_t)(1 - j_t))
//! ```
//!
//! over little-endian row bits (`j = sum_t j_t 2^t`), so `w` at the bits of a
//! row is that row's value.
//!
//! - [`eq_table`] and [`evaluate`]: the vector `eq(x, .)` and `w(x)`;
//! - [`shifted_eq`] and [`shifted_eq_table`]: `sum_j eq(rho, j) eq(sigma, (j + r)
//!   mod n)`, the multilinear extension of "row `j` read at `Rotation(r)`"
//!   (iroha_plonk reads row `(j + r) mod n`), in `O(k)`;
//! - [`folded_eq`]: the public vector `eq(x, .)` after the inner-product
//!   argument's folds;
//! - [`open_batch`] and [`verify_batch`]: one inner-product argument opening
//!   many columns at one point (the `batch` module); [`open_batch_bound`] and
//!   [`verify_batch_bound`] for claims the caller's transcript already bound.

use ff::Field;
use iroha_pasta::PastaCurve;

mod batch;

pub use batch::{
    BatchOpening, MleClaim, open_batch, open_batch_bound, open_batch_bound_cancellable,
    open_batch_cancellable, verify_batch, verify_batch_bound,
};

/// Role tag of a batch opening, absorbed first.
pub const TAG_BATCH_OPENING: u64 = u64::from_le_bytes(*b"kgwfdbo1");

/// The largest `k` the row arithmetic here supports (`2^k` rows fit an `i64`).
pub const MAX_K: usize = 62;

/// The Fiat-Shamir transcript an opening runs in.
///
/// The caller's protocol owns it (the folding decider's RP57 transcript over
/// the commitment curve's base field), so an opening is domain-separated
/// inside that protocol by [`TAG_BATCH_OPENING`] and an in-circuit verifier
/// can absorb the same values natively.
pub trait MleTranscript<C: PastaCurve> {
    /// Absorbs a count, length or tag.
    fn absorb_u64(&mut self, value: u64);
    /// Absorbs scalars of `C`.
    fn absorb_scalars(&mut self, values: &[C::ScalarExt]);
    /// Absorbs a point of `C`, the identity included.
    fn absorb_point(&mut self, point: &C::AffineExt);
    /// A full-width challenge.
    fn challenge(&mut self) -> C::ScalarExt;
    /// A challenge below `2^128`, used as a scalar on commitments.
    fn challenge_128(&mut self) -> C::ScalarExt;
}

/// `eq(x, j)` for every row `j < 2^k`, `k = x.len()`.
///
/// # Panics
///
/// If `k` exceeds [`MAX_K`] (the table would not fit in memory long before).
#[must_use]
pub fn eq_table<F: Field>(point: &[F]) -> Vec<F> {
    assert!(point.len() <= MAX_K, "eq table of more than 2^{MAX_K} rows");
    let mut table = Vec::with_capacity(1_usize << point.len());
    table.push(F::ONE);
    // After x_0..x_{t-1}, row j < 2^t holds prod_{s<t} eq1(x_s, j_s); x_t
    // splits it into j (bit t clear) and j + 2^t (bit t set).
    for x in point {
        let len = table.len();
        for row in 0..len {
            let high = table[row] * x;
            table[row] -= high;
            table.push(high);
        }
    }
    table
}

/// The multilinear extension of `values` (`2^k` rows) at `point`, or `None`
/// when `values` is not `2^point.len()` long.
#[must_use]
pub fn evaluate<F: Field>(values: &[F], point: &[F]) -> Option<F> {
    if point.len() > MAX_K || values.len() != 1_usize << point.len() {
        return None;
    }
    // Fold the top bit first: w(x) = w_lo(x') + x_{k-1} (w_hi(x') - w_lo(x')).
    let mut folded = values.to_vec();
    for x in point.iter().rev() {
        let half = folded.len() / 2;
        let (low, high) = folded.split_at_mut(half);
        for (low, high) in low.iter_mut().zip(high.iter()) {
            *low += (*high - *low) * x;
        }
        folded.truncate(half);
    }
    folded.first().copied()
}

/// `sum_j eq(rho, j) eq(sigma, (j + rotation) mod n)`, `n = 2^k`: the
/// multilinear extension, in both points, of the rows `(j, (j + rotation) mod
/// n)`. For a column `w`, `sum_y w(y) shifted_eq(rho, y, r)` over boolean `y`
/// is the multilinear extension at `rho` of the column read at `Rotation(r)`.
///
/// The shift `c = rotation mod n` is added bit by bit through a carry automaton
/// with states "no carry" and "carry": bit `t` maps carry `e` and row bit `x`
/// to the shifted bit `(x + c_t + e) mod 2` and the carry `(x + c_t + e) / 2`
/// with weight `eq1(rho_t, x) eq1(sigma_t, shifted bit)`. The final carry is
/// dropped (the sum is modulo `n`), which covers negative rotations and the
/// wrap past the last row.
///
/// Returns `None` when the points differ in length or `k` exceeds [`MAX_K`].
#[must_use]
pub fn shifted_eq<F: Field>(rho: &[F], sigma: &[F], rotation: i32) -> Option<F> {
    let shift = shift_of(rho.len(), rotation)?;
    if sigma.len() != rho.len() {
        return None;
    }
    // Weights of reaching bit t without and with a carry.
    let mut weights = [F::ONE, F::ZERO];
    for (t, (rho_t, sigma_t)) in rho.iter().zip(sigma).enumerate() {
        let shift_bit = usize::from((shift >> t) & 1 == 1);
        let rho_bit = [F::ONE - rho_t, *rho_t];
        let sigma_bit = [F::ONE - sigma_t, *sigma_t];
        let mut next = [F::ZERO; 2];
        for (carry, weight) in weights.iter().enumerate() {
            for (row_bit, rho_weight) in rho_bit.iter().enumerate() {
                let total = row_bit + shift_bit + carry;
                next[total >> 1] += *weight * rho_weight * sigma_bit[total & 1];
            }
        }
        weights = next;
    }
    Some(weights[0] + weights[1])
}

/// `shifted_eq(rho, y, rotation)` for every row `y`: `eq(rho, (y - rotation)
/// mod n)`. `None` when `k` exceeds [`MAX_K`].
#[must_use]
pub fn shifted_eq_table<F: Field>(rho: &[F], rotation: i32) -> Option<Vec<F>> {
    let shift = usize::try_from(shift_of(rho.len(), rotation)?).ok()?;
    let mut table = eq_table(rho);
    table.rotate_right(shift);
    Some(table)
}

/// The value of `b = eq(point, .)` after the inner-product argument folds it
/// with `challenges` (top index bit first, `b_lo += u_j b_hi` as in
/// [`crate::pcs::ipa::prover`]): `prod_j ((1 - x_{k-1-j}) + u_j x_{k-1-j})`.
/// It equals `<s(u), eq(point, .)>` for [`crate::pcs::ipa::fold_scalars`]
/// `s(u)`. `None` when the lengths differ.
#[must_use]
pub fn folded_eq<F: Field>(point: &[F], challenges: &[F]) -> Option<F> {
    if challenges.len() != point.len() {
        return None;
    }
    Some(
        point
            .iter()
            .rev()
            .zip(challenges)
            .fold(F::ONE, |acc, (x, u)| acc * (F::ONE - x + *u * x)),
    )
}

/// `rotation mod 2^k`, or `None` when `k` exceeds [`MAX_K`].
fn shift_of(k: usize, rotation: i32) -> Option<u64> {
    if k > MAX_K {
        return None;
    }
    let n = 1_i64 << k;
    u64::try_from(i64::from(rotation).rem_euclid(n)).ok()
}

#[cfg(test)]
mod tests;
