//! Bounded streaming widened accumulation for AIR mixtures and DEEP dot products.
//!
//! Public block lengths bound the carry counter. For the Goldilocks prime,
//! `2^128 = -2^32 (mod p)`, so overflow of an unsigned 128-bit sum is accounted
//! for by subtracting `carry * 2^32` after one ordinary field reduction. This
//! changes neither residue order, challenge coordinates nor the final value.

use super::super::super::private_table::zeroize_words_v1;
use super::*;

const TERMS_PER_BLOCK_V1: usize = 1 << 10;

/// One ordered mixture with no retained residue vector and a fixed carry bound.
pub(super) struct MainResidueMixV1 {
    result: E,
    sums: [u128; 4],
    carries: [u64; 4],
    pending: usize,
}

impl MainResidueMixV1 {
    /// Start an empty mixture whose private accumulators clear on every exit.
    pub(super) fn new_v1() -> Self {
        Self {
            result: E::ZERO,
            sums: [0; 4],
            carries: [0; 4],
            pending: 0,
        }
    }

    /// Add one canonical residue with its exact public challenge coordinate.
    pub(super) fn push_v1(&mut self, residue: F, alpha: &E) {
        let coefficients = alpha.coefficients();
        for (lane, coefficient) in coefficients.iter().enumerate() {
            let product = u128::from(coefficient.0) * u128::from(residue.0);
            let (sum, carry) = self.sums[lane].overflowing_add(product);
            self.sums[lane] = sum;
            self.carries[lane] += u64::from(carry);
        }
        self.pending += 1;
        if self.pending == TERMS_PER_BLOCK_V1 {
            self.flush_v1();
        }
    }

    /// Reduce the last public block and return the exact field mixture.
    pub(super) fn finish_v1(mut self) -> E {
        self.flush_v1();
        self.result
    }

    fn flush_v1(&mut self) {
        if self.pending == 0 {
            return;
        }
        let coefficients = core::array::from_fn(|lane| {
            // At most 1023 carries are possible in a public 1024-term block,
            // so this correction is a canonical field element below 2^42.
            F::reduce(self.sums[lane]).sub(F(self.carries[lane] << 32))
        });
        self.result = self
            .result
            .add(E::from_coefficients(coefficients).expect("canonical field reduction"));
        zeroize_words_v1(&mut self.sums);
        zeroize_words_v1(&mut self.carries);
        self.pending = 0;
    }
}

impl Drop for MainResidueMixV1 {
    fn drop(&mut self) {
        zeroize_words_v1(core::slice::from_mut(&mut self.result));
        zeroize_words_v1(&mut self.sums);
        zeroize_words_v1(&mut self.carries);
    }
}

/// Mix canonical scalar values and their exact, equally sized Fp4 weights.
/// The caller validates both public widths before producing any private values.
pub(super) fn mix_residues_v1(residues: &[F], alphas: &[E]) -> E {
    debug_assert_eq!(residues.len(), alphas.len());
    mix_indexed_residues_v1(residues.len(), |index| (residues[index], alphas[index]))
}

/// Mix validated canonical terms without gathering private scalar arrays.
#[inline]
pub(super) fn mix_indexed_residues_v1(length: usize, mut term: impl FnMut(usize) -> (F, E)) -> E {
    if length == 1 {
        let (residue, alpha) = term(0);
        return alpha.mul_base(residue);
    }
    let mut mix = MainResidueMixV1::new_v1();
    for index in 0..length {
        let (residue, alpha) = term(index);
        mix.push_v1(residue, &alpha);
    }
    mix.finish_v1()
}

#[cfg(test)]
#[path = "main_composition_mix_tests.rs"]
mod tests;
