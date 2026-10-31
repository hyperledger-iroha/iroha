//! Exact widened accumulation for scalar/Fp4 AIR mixtures and DEEP dot products.
//!
//! Public block lengths bound the carry counter. For the Goldilocks prime,
//! `2^128 = -2^32 (mod p)`, so overflow of an unsigned 128-bit sum is accounted
//! for by subtracting `carry * 2^32` after one ordinary field reduction. This
//! changes neither residue order, challenge coordinates nor the final value.

use super::*;
use zeroize::Zeroize as _;

const TERMS_PER_BLOCK_V1: usize = 1 << 10;

#[derive(Default)]
struct WideMixScratchV1 {
    sums: [u128; 4],
    carries: [u64; 4],
}

impl WideMixScratchV1 {
    #[inline]
    fn add_v1(&mut self, residue: F, alpha: E) {
        for (lane, coefficient) in alpha.coefficients().iter().enumerate() {
            let product = u128::from(coefficient.0) * u128::from(residue.0);
            let (sum, carry) = self.sums[lane].overflowing_add(product);
            self.sums[lane] = sum;
            self.carries[lane] += u64::from(carry);
        }
    }

    #[inline]
    fn reduced_v1(&self) -> E {
        let coefficients = core::array::from_fn(|lane| {
            // At most 1023 carries are possible in a public 1024-term block,
            // so this correction is a canonical field element below 2^42.
            F::reduce(self.sums[lane]).sub(F(self.carries[lane] << 32))
        });
        E::from_coefficients(coefficients).expect("canonical field reduction")
    }
}

impl Drop for WideMixScratchV1 {
    fn drop(&mut self) {
        self.sums.zeroize();
        self.carries.zeroize();
    }
}

/// Mix canonical scalar values and their exact, equally sized Fp4 weights.
/// The caller validates both public widths before producing any private values.
pub(super) fn mix_residues_v1(residues: &[F], alphas: &[E]) -> E {
    debug_assert_eq!(residues.len(), alphas.len());
    let mut result = E::ZERO;
    for (residues, alphas) in residues
        .chunks(TERMS_PER_BLOCK_V1)
        .zip(alphas.chunks(TERMS_PER_BLOCK_V1))
    {
        let mut scratch = WideMixScratchV1::default();
        for (residue, alpha) in residues.iter().zip(alphas) {
            scratch.add_v1(*residue, *alpha);
        }
        result = result.add(scratch.reduced_v1());
    }
    result
}

/// Mix validated canonical terms without gathering private scalar arrays.
/// The public length bounds carries exactly as in the contiguous slice path.
#[inline]
pub(super) fn mix_indexed_residues_v1(length: usize, mut term: impl FnMut(usize) -> (F, E)) -> E {
    if length == 1 {
        let (residue, alpha) = term(0);
        return alpha.mul_base(residue);
    }
    let mut result = E::ZERO;
    for first in (0..length).step_by(TERMS_PER_BLOCK_V1) {
        let mut scratch = WideMixScratchV1::default();
        for index in first..first + (length - first).min(TERMS_PER_BLOCK_V1) {
            let (residue, alpha) = term(index);
            scratch.add_v1(residue, alpha);
        }
        let reduced = scratch.reduced_v1();
        result = if first == 0 {
            reduced
        } else {
            result.add(reduced)
        };
    }
    result
}

#[cfg(test)]
#[path = "main_composition_mix_tests.rs"]
mod tests;
