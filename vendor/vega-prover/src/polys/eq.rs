// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! `EqPolynomial`: Represents multilinear extension of equality polynomials, evaluated based on binary input values.
use ff::PrimeField;
use rayon::prelude::*;

/// Represents the multilinear extension polynomial (MLE) of the equality polynomial $eq(x,e)$, denoted as $\tilde{eq}(x, e)$.
///
/// The polynomial is defined by the formula:
/// $$
/// \tilde{eq}(x, e) = \prod_{i=1}^m(e_i * x_i + (1 - e_i) * (1 - x_i))
/// $$
///
/// Each element in the vector `r` corresponds to a component $e_i$, representing a bit from the binary representation of an input value $e$.
/// This polynomial evaluates to 1 if every component $x_i$ equals its corresponding $e_i$, and 0 otherwise.
///
/// For instance, for e = 6 (with a binary representation of 0b110), the vector r would be [1, 1, 0].
#[derive(Debug)]
pub struct EqPolynomial<Scalar: PrimeField> {
    r: Vec<Scalar>,
}

impl<Scalar: PrimeField> EqPolynomial<Scalar> {
    /// Creates a new `EqPolynomial` from a vector of Scalars `r`.
    ///
    /// Each Scalar in `r` corresponds to a bit from the binary representation of an input value `e`.
    pub const fn new(r: Vec<Scalar>) -> Self {
        EqPolynomial { r }
    }

    /// Evaluates the `EqPolynomial` at a given point `rx`.
    ///
    /// This function computes the value of the polynomial at the point specified by `rx`.
    /// It expects `rx` to have the same length as the internal vector `r`.
    ///
    /// Panics if `rx` and `r` have different lengths.
    pub fn evaluate(&self, rx: &[Scalar]) -> Scalar {
        assert_eq!(self.r.len(), rx.len());
        (0..rx.len())
            .map(|i| rx[i] * self.r[i] + (Scalar::ONE - rx[i]) * (Scalar::ONE - self.r[i]))
            .fold(Scalar::ONE, |acc, item| acc * item)
    }

    /// Evaluates the `EqPolynomial` at all the `2^|r|` points in its domain.
    ///
    /// Returns a vector of Scalars, each corresponding to the polynomial evaluation at a specific point.
    pub fn evals(&self) -> Vec<Scalar> {
        Self::evals_from_points(&self.r)
    }

    /// Evaluates the `EqPolynomial` from the `2^|r|` points in its domain, without creating an intermediate polynomial
    /// representation.
    ///
    /// Returns a vector of Scalars, each corresponding to the polynomial evaluation at a specific point.
    pub fn evals_from_points(r: &[Scalar]) -> Vec<Scalar> {
        let ell = r.len();
        let mut evals: Vec<Scalar> = vec![Scalar::ZERO; (2_usize).pow(ell as u32)];
        let mut size = 1;
        evals[0] = Scalar::ONE;

        if rayon::current_num_threads() <= 1 {
            for r in r.iter().rev() {
                let (evals_left, evals_right) = evals.split_at_mut(size);
                let (evals_right, _) = evals_right.split_at_mut(size);

                for (x, y) in evals_left.iter_mut().zip(evals_right.iter_mut()) {
                    *y = *x * r;
                    *x -= &*y;
                }

                size *= 2;
            }
        } else {
            for r in r.iter().rev() {
                let (evals_left, evals_right) = evals.split_at_mut(size);
                let (evals_right, _) = evals_right.split_at_mut(size);

                zip_with_for_each!(par_iter_mut, (evals_left, evals_right), |x, y| {
                    *y = *x * r;
                    *x -= &*y;
                });

                size *= 2;
            }
        }

        evals
    }
    /// Like evals_from_points but reuses a pre-allocated buffer.
    /// Builds the evaluation table incrementally via push (no zeroing needed).
    #[inline(always)]
    pub fn evals_from_points_into(r: &[Scalar], out: &mut Vec<Scalar>) {
        let ell = r.len();
        let n = (2_usize).pow(ell as u32);
        out.clear();
        out.reserve(n);
        out.push(Scalar::ONE);

        for r_val in r.iter().rev() {
            let sz = out.len();
            // Push new right-half values: out[sz+i] = out[i] * r
            for i in 0..sz {
                let val = out[i] * r_val;
                out.push(val);
            }
            // Update left-half: out[i] -= out[sz+i] using split_at_mut
            let (left, right) = out.split_at_mut(sz);
            for i in 0..sz {
                left[i] -= right[i];
            }
        }
        debug_assert_eq!(out.len(), n);
    }
}

impl<Scalar: PrimeField> FromIterator<Scalar> for EqPolynomial<Scalar> {
    fn from_iter<I: IntoIterator<Item = Scalar>>(iter: I) -> Self {
        let r: Vec<_> = iter.into_iter().collect();
        EqPolynomial { r }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::pasta::pallas;

    fn test_eq_polynomial_with<F: PrimeField>() {
        let eq_poly = EqPolynomial::<F>::new(vec![F::ONE, F::ZERO, F::ONE]);
        let y = eq_poly.evaluate(vec![F::ONE, F::ONE, F::ONE].as_slice());
        assert_eq!(y, F::ZERO);

        let y = eq_poly.evaluate(vec![F::ONE, F::ZERO, F::ONE].as_slice());
        assert_eq!(y, F::ONE);

        let eval_list = eq_poly.evals();
        for (i, &coeff) in eval_list.iter().enumerate().take((2_usize).pow(3)) {
            if i == 5 {
                assert_eq!(coeff, F::ONE);
            } else {
                assert_eq!(coeff, F::ZERO);
            }
        }
    }

    #[test]
    fn test_eq_polynomial() {
        test_eq_polynomial_with::<pallas::Scalar>();
    }
}
