//! Exact widened accumulation for the existing scalar-residue/Fp4 AIR mixture.
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

impl Drop for WideMixScratchV1 {
    fn drop(&mut self) {
        self.sums.zeroize();
        self.carries.zeroize();
    }
}

/// Mix canonical residues and their exact, equally sized public alpha vector.
/// The caller validates both public widths before producing any residues.
pub(super) fn mix_residues_v1(residues: &[F], alphas: &[E]) -> E {
    debug_assert_eq!(residues.len(), alphas.len());
    let mut result = E::ZERO;
    for (residues, alphas) in residues
        .chunks(TERMS_PER_BLOCK_V1)
        .zip(alphas.chunks(TERMS_PER_BLOCK_V1))
    {
        let mut scratch = WideMixScratchV1::default();
        for (residue, alpha) in residues.iter().zip(alphas) {
            let coefficients = alpha.coefficients();
            for (lane, coefficient) in coefficients.iter().enumerate() {
                let product = u128::from(coefficient.0) * u128::from(residue.0);
                let (sum, carry) = scratch.sums[lane].overflowing_add(product);
                scratch.sums[lane] = sum;
                scratch.carries[lane] += u64::from(carry);
            }
        }
        let coefficients = core::array::from_fn(|lane| {
            // At most 1023 carries are possible in a public 1024-term block,
            // so this correction is a canonical field element below 2^42.
            F::reduce(scratch.sums[lane]).sub(F(scratch.carries[lane] << 32))
        });
        result = result.add(E::from_coefficients(coefficients).expect("canonical field reduction"));
    }
    result
}

#[cfg(test)]
#[path = "main_composition_mix_tests.rs"]
mod tests;
