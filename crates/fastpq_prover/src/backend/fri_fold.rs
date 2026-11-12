//! Checked radix-two multiplicative-coset folding for arities 2, 4, 8 and 16.
//!
//! For `f(X) = sum_j X^j f_j(X^r)`, folding returns
//! `sum_j beta^j f_j(x^r)` from the ordered fiber `f(x * omega^i)`.
//! The inverse butterfly transform uses base-field twiddles while preserving
//! all four challenge coordinates. This owner does not select a protocol,
//! transcript, degree bound, query count or production admission policy.
//!
//! The bounded verifier folds every authenticated fiber of the fixed profile
//! through [`FriFoldPlan::fold_coset`]. The producer folds in coefficient space
//! (`deep_coefficient_replay::fold_coefficients`), which is the same map.

use super::polynomial_field::PolynomialField;
use super::{GOLDILOCKS_MODULUS, field_inverse, field_pow, mul_mod};
use crate::{Error, Result, field::GoldilocksFp4V1};

const MAX_ARITY: usize = 16;

/// Checked, fixed-size inverse transform for one multiplicative fiber shape.
#[derive(Clone, Debug)]
pub(super) struct FriFoldPlan {
    arity: usize,
    inverse_arity: u64,
    inverse_twiddles: [u64; MAX_ARITY / 2],
    reverse: [usize; MAX_ARITY],
}

impl FriFoldPlan {
    /// Require a canonical base-field generator of exactly the selected order.
    pub(super) fn new(arity: usize, coset_generator: u64) -> Result<Self> {
        if !matches!(arity, 2 | 4 | 8 | 16)
            || coset_generator == 0
            || coset_generator >= GOLDILOCKS_MODULUS
            || field_pow(coset_generator, arity as u64) != 1
            || field_pow(coset_generator, (arity / 2) as u64) == 1
        {
            return Err(Error::FriDomainSize {
                length: arity,
                arity,
            });
        }
        let inverse_root = field_inverse(coset_generator);
        let mut inverse_twiddles = [1; MAX_ARITY / 2];
        for index in 1..arity / 2 {
            inverse_twiddles[index] = mul_mod(inverse_twiddles[index - 1], inverse_root);
        }
        let bits = arity.ilog2();
        let reverse = core::array::from_fn(|index| {
            if index < arity {
                index.reverse_bits() >> (usize::BITS - bits)
            } else {
                0
            }
        });
        Ok(Self {
            arity,
            inverse_arity: field_inverse(arity as u64),
            inverse_twiddles,
            reverse,
        })
    }

    /// Fold one ordered authenticated fiber with checked field inputs.
    ///
    /// `x` is the point for the first fiber value, and the remaining values
    /// correspond to `x * omega^i`. The caller authenticates these associations.
    pub(super) fn fold_coset(
        &self,
        values: &[GoldilocksFp4V1],
        challenge: GoldilocksFp4V1,
        x: u64,
    ) -> Result<GoldilocksFp4V1> {
        if values.len() != self.arity || x == 0 || x >= GOLDILOCKS_MODULUS {
            return Err(Error::FriDomainSize {
                length: values.len(),
                arity: self.arity,
            });
        }
        challenge.validate("fri_fold_challenge", &[])?;
        for (index, &value) in values.iter().enumerate() {
            value.validate("fri_fold_value", &[index])?;
        }
        Ok(self.fold_canonical(values, challenge, field_inverse(x)))
    }

    fn fold_canonical(
        &self,
        values: &[GoldilocksFp4V1],
        challenge: GoldilocksFp4V1,
        inverse_x: u64,
    ) -> GoldilocksFp4V1 {
        let mut coefficients = [GoldilocksFp4V1::ZERO; MAX_ARITY];
        for (index, coefficient) in coefficients[..self.arity].iter_mut().enumerate() {
            *coefficient = values[self.reverse[index]];
        }
        let mut width = 2;
        while width <= self.arity {
            let half = width / 2;
            let stride = self.arity / width;
            for chunk in coefficients[..self.arity].chunks_exact_mut(width) {
                let (left, right) = chunk.split_at_mut(half);
                for (index, (even, odd)) in left.iter_mut().zip(right).enumerate() {
                    let twiddled = odd.mul_base(self.inverse_twiddles[index * stride]);
                    let original_even = *even;
                    *even = original_even.add(twiddled);
                    *odd = original_even.sub(twiddled);
                }
            }
            width *= 2;
        }
        // The unnormalized inverse transform yields r*x^j*f_j(x^r).
        // Horner at beta/x followed by division by r produces the desired fold.
        let ratio = challenge.mul_base(inverse_x);
        coefficients[..self.arity]
            .iter()
            .rev()
            .fold(GoldilocksFp4V1::ZERO, |value, &coefficient| {
                value.mul(ratio).add(coefficient)
            })
            .mul_base(self.inverse_arity)
    }
}

#[cfg(test)]
#[path = "fri_fold/tests.rs"]
mod tests;
