//! Checked radix-two multiplicative-coset folding for arities 2, 4, 8 and 16.
//!
//! For `f(X) = sum_j X^j f_j(X^r)`, folding returns
//! `sum_j beta^j f_j(x^r)` from the ordered fiber `f(x * omega^i)`.
//! The inverse butterfly transform uses base-field twiddles while preserving
//! all four challenge coordinates. This owner does not select a protocol,
//! transcript, degree bound, query count or production admission policy.
//!
//! TODO: Wire this owner into the reviewed compact protocol's authenticated
//! fibers and degree progression before replacing the existing binary profile.

use rayon::prelude::*;

use super::polynomial_field::PolynomialField;
use super::{FriDomain, GOLDILOCKS_MODULUS, field_inverse, field_pow, mul_mod};
use crate::{Error, Result, field::GoldilocksFp4V1};

const MAX_ARITY: usize = 16;
const MAX_LAYER_JOBS: usize = 32;
const PARALLEL_OUTPUT_THRESHOLD: usize = 1024;

/// Checked, fixed-size inverse transform for one multiplicative fiber shape.
#[derive(Clone, Debug)]
pub(super) struct FriFoldPlan {
    arity: usize,
    coset_generator: u64,
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
            coset_generator,
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

    /// Fold a complete layer into an exactly sized caller-owned output slice.
    ///
    /// Validation finishes before any output is changed. Groups are strided:
    /// `values[i + j * output.len()]` belongs to the fiber rooted at `domain[i]`.
    /// Each job owns a fixed 16-element stack workspace; no per-fiber allocation
    /// or inversion occurs. Serial and parallel execution use the same kernel.
    pub(super) fn fold_layer_into(
        &self,
        values: &[GoldilocksFp4V1],
        challenge: GoldilocksFp4V1,
        domain: FriDomain,
        output: &mut [GoldilocksFp4V1],
    ) -> Result<()> {
        if values.len() < self.arity
            || !values.len().is_power_of_two()
            || output.len() != values.len() / self.arity
            || domain.generator == 0
            || domain.generator >= GOLDILOCKS_MODULUS
            || domain.offset == 0
            || domain.offset >= GOLDILOCKS_MODULUS
            || field_pow(domain.generator, values.len() as u64) != 1
            || field_pow(domain.generator, (values.len() / 2) as u64) == 1
            || domain.coset_generator(output.len()) != self.coset_generator
        {
            return Err(Error::FriDomainSize {
                length: values.len(),
                arity: self.arity,
            });
        }
        challenge.validate("fri_fold_challenge", &[])?;
        for (index, &value) in values.iter().enumerate() {
            value.validate("fri_fold_value", &[index])?;
        }
        let inverse_offset = field_inverse(domain.offset);
        let inverse_generator = field_inverse(domain.generator);
        let output_len = output.len();
        let fill = |start: usize, destination: &mut [GoldilocksFp4V1]| {
            let mut inverse_x = mul_mod(inverse_offset, field_pow(inverse_generator, start as u64));
            let mut fiber = [GoldilocksFp4V1::ZERO; MAX_ARITY];
            for (local_index, result) in destination.iter_mut().enumerate() {
                let index = start + local_index;
                for (position, value) in fiber[..self.arity].iter_mut().enumerate() {
                    *value = values[index + position * output_len];
                }
                *result = self.fold_canonical(&fiber[..self.arity], challenge, inverse_x);
                inverse_x = mul_mod(inverse_x, inverse_generator);
            }
        };
        if output_len < PARALLEL_OUTPUT_THRESHOLD {
            fill(0, output);
        } else {
            let chunk_len = output_len.div_ceil(MAX_LAYER_JOBS);
            output
                .par_chunks_mut(chunk_len)
                .enumerate()
                .for_each(|(chunk, destination)| fill(chunk * chunk_len, destination));
        }
        Ok(())
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
