// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! Zero-knowledge multi-circuit (MC) prover using NeutronNova folding (https://eprint.iacr.org/2024/1606), proved with the single-circuit prover.
use crate::start_span;
use crate::{
    Commitment, CommitmentKey, DEFAULT_COMMITMENT_WIDTH, VerifierKey,
    bellpepper::{
        r1cs::{
            MultiRoundVegaShape, MultiRoundVegaWitness, PrecommittedState, VegaShape, VegaWitness,
        },
        shape_cs::ShapeCS,
        solver::SatisfyingAssignment,
    },
    big_num::{
        DelayedReduction,
        montgomery::MontgomeryLimbs,
        small_value::{SmallAccumulator, to_small_vec_or_zero},
    },
    digest::DigestComputer,
    errors::VegaError,
    math::Math,
    nifs::NovaNIFS,
    polys::{
        eq::EqPolynomial,
        multilinear::{MultilinearPolynomial, SparsePolynomial},
        power::PowPolynomial,
        univariate::UniPoly,
    },
    r1cs::{
        R1CSInstance, R1CSShape, R1CSWitness, RelaxedR1CSInstance, SplitMultiRoundR1CSInstance,
        SplitMultiRoundR1CSShape, SplitR1CSInstance, SplitR1CSShape, weights_from_r,
    },
    sumcheck::SumcheckProof,
    traits::{
        Engine,
        circuit::VegaCircuit,
        pcs::{FoldingEngineTrait, PCSEngineTrait},
        snark::{DigestHelperTrait, VegaDigest},
        transcript::TranscriptEngineTrait,
    },
    zk::VegaMcVerifierCircuit,
};
use bincode::Options as _;
use ff::Field;
use once_cell::sync::OnceCell;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

fn compute_tensor_decomp(n: usize) -> (usize, usize, usize) {
    let ell = n.next_power_of_two().log_2();
    // we split ell into ell1 and ell2 such that ell1 + ell2 = ell and ell1 >= ell2
    let ell1 = ell.div_ceil(2); // This ensures ell1 >= ell2
    let ell2 = ell / 2;
    let left = 1 << ell1;
    let right = 1 << ell2;

    (ell, left, right)
}

/// A type that holds the NeutronNova NIFS (Non-Interactive Folding Scheme)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct VegaMcNIFS<E: Engine> {
    polys: Vec<UniPoly<E::Scalar>>,
}

#[inline(always)]
#[allow(clippy::needless_range_loop)]
fn suffix_weight_full<F: Field>(t: usize, ell_b: usize, pair_idx: usize, rhos: &[F]) -> F {
    let mut w = F::ONE;
    let mut k = pair_idx;
    for s in (t + 1)..ell_b {
        let bit = (k & 1) as u8; // LSB-first
        w *= if bit == 0 { F::ONE - rhos[s] } else { rhos[s] };
        k >>= 1;
    }
    w
}

impl<E: Engine> VegaMcNIFS<E>
where
    E::PCS: FoldingEngineTrait<E>,
{
    /// Computes the evaluations of the sum-check polynomial at 0, 2, and 3
    /// Uses two-level delayed modular reduction (inner + middle levels).
    /// Note: Outer level (over pairs) uses regular field arithmetic since there are few pairs.
    #[inline(always)]
    #[allow(clippy::needless_range_loop)]
    fn prove_helper(
        round: usize,
        (left, right): (usize, usize),
        e: &[E::Scalar],
        Az1: &[E::Scalar],
        Bz1: &[E::Scalar],
        Cz1: &[E::Scalar],
        Az2: &[E::Scalar],
        Bz2: &[E::Scalar],
    ) -> (E::Scalar, E::Scalar) {
        type Acc<S> = <S as DelayedReduction<S>>::Accumulator;

        // sanity check sizes
        assert_eq!(e.len(), left + right);
        assert_eq!(Az1.len(), left * right);

        let f = &e[left..];
        let e_left = &e[..left];
        let compute_e0 = round != 0;

        let mut acc_e0 = Acc::<E::Scalar>::default();
        let mut acc_quad = Acc::<E::Scalar>::default();

        for i in 0..right {
            let base = i * left;
            let mut inner_e0 = Acc::<E::Scalar>::default();
            let mut inner_quad = Acc::<E::Scalar>::default();

            if compute_e0 {
                for j in 0..left {
                    let k = base + j;
                    let inner_val = Az1[k] * Bz1[k] - Cz1[k];
                    <E::Scalar as DelayedReduction<E::Scalar>>::unreduced_multiply_accumulate(
                        &mut inner_e0,
                        &e_left[j],
                        &inner_val,
                    );
                    let az_diff = Az2[k] - Az1[k];
                    let bz_diff = Bz2[k] - Bz1[k];
                    let quad_val = az_diff * bz_diff;
                    <E::Scalar as DelayedReduction<E::Scalar>>::unreduced_multiply_accumulate(
                        &mut inner_quad,
                        &e_left[j],
                        &quad_val,
                    );
                }
            } else {
                for j in 0..left {
                    let k = base + j;
                    let az_diff = Az2[k] - Az1[k];
                    let bz_diff = Bz2[k] - Bz1[k];
                    let quad_val = az_diff * bz_diff;
                    <E::Scalar as DelayedReduction<E::Scalar>>::unreduced_multiply_accumulate(
                        &mut inner_quad,
                        &e_left[j],
                        &quad_val,
                    );
                }
            }

            let inner_e0_red = <E::Scalar as DelayedReduction<E::Scalar>>::reduce(&inner_e0);
            let inner_quad_red = <E::Scalar as DelayedReduction<E::Scalar>>::reduce(&inner_quad);

            let f_i = &f[i];
            <E::Scalar as DelayedReduction<E::Scalar>>::unreduced_multiply_accumulate(
                &mut acc_e0,
                f_i,
                &inner_e0_red,
            );
            <E::Scalar as DelayedReduction<E::Scalar>>::unreduced_multiply_accumulate(
                &mut acc_quad,
                f_i,
                &inner_quad_red,
            );
        }

        (
            <E::Scalar as DelayedReduction<E::Scalar>>::reduce(&acc_e0),
            <E::Scalar as DelayedReduction<E::Scalar>>::reduce(&acc_quad),
        )
    }

    /// AB-only variant of prove_helper: computes sum E[k]*Az_lo*Bz_lo (without Cz subtraction)
    /// and the quad term sum E[k]*(Az_hi-Az_lo)*(Bz_hi-Bz_lo).
    /// The caller subtracts the precomputed C_val contribution from e0_ab externally.
    #[inline(always)]
    #[allow(clippy::needless_range_loop)]
    fn prove_helper_ab_only(
        (left, right): (usize, usize),
        e: &[E::Scalar],
        Az1: &[E::Scalar],
        Bz1: &[E::Scalar],
        Az2: &[E::Scalar],
        Bz2: &[E::Scalar],
    ) -> (E::Scalar, E::Scalar) {
        type Acc<S> = <S as DelayedReduction<S>>::Accumulator;

        let f = &e[left..];
        let e_left = &e[..left];

        let mut acc_e0_ab = Acc::<E::Scalar>::default();
        let mut acc_quad = Acc::<E::Scalar>::default();

        for i in 0..right {
            let base = i * left;
            let mut inner_e0 = Acc::<E::Scalar>::default();
            let mut inner_quad = Acc::<E::Scalar>::default();

            for j in 0..left {
                let k = base + j;
                let ab_val = Az1[k] * Bz1[k];
                <E::Scalar as DelayedReduction<E::Scalar>>::unreduced_multiply_accumulate(
                    &mut inner_e0,
                    &e_left[j],
                    &ab_val,
                );
                let az_diff = Az2[k] - Az1[k];
                let bz_diff = Bz2[k] - Bz1[k];
                let quad_val = az_diff * bz_diff;
                <E::Scalar as DelayedReduction<E::Scalar>>::unreduced_multiply_accumulate(
                    &mut inner_quad,
                    &e_left[j],
                    &quad_val,
                );
            }

            let inner_e0_red = <E::Scalar as DelayedReduction<E::Scalar>>::reduce(&inner_e0);
            let inner_quad_red = <E::Scalar as DelayedReduction<E::Scalar>>::reduce(&inner_quad);

            let f_i = &f[i];
            <E::Scalar as DelayedReduction<E::Scalar>>::unreduced_multiply_accumulate(
                &mut acc_e0_ab,
                f_i,
                &inner_e0_red,
            );
            <E::Scalar as DelayedReduction<E::Scalar>>::unreduced_multiply_accumulate(
                &mut acc_quad,
                f_i,
                &inner_quad_red,
            );
        }

        (
            <E::Scalar as DelayedReduction<E::Scalar>>::reduce(&acc_e0_ab),
            <E::Scalar as DelayedReduction<E::Scalar>>::reduce(&acc_quad),
        )
    }

    /// Small-value variant of prove_helper for round 0 (compute_e0=false).
    ///
    /// Uses integer arithmetic for the inner loop: i64 subtraction + i128 multiplication,
    /// then `SmallAccumulator` for `field_mont * i128` accumulation.
    /// Large-value positions (where Az/Bz didn't fit i64) are corrected with field arithmetic.
    ///
    /// Returns only quad_coeff (e0 is always zero for round 0).
    #[inline(always)]
    #[allow(clippy::needless_range_loop)]
    fn prove_helper_small(
        (left, right): (usize, usize),
        e: &[E::Scalar],
        Az1: &[E::Scalar],
        Bz1: &[E::Scalar],
        Az2: &[E::Scalar],
        Bz2: &[E::Scalar],
        Az1_i64: &[i64],
        Bz1_i64: &[i64],
        Az2_i64: &[i64],
        Bz2_i64: &[i64],
        large_positions: &[usize],
    ) -> E::Scalar {
        type Acc<S> = <S as DelayedReduction<S>>::Accumulator;

        let f = &e[left..];
        let e_left = &e[..left];
        let total = left * right;

        let mut acc_quad = Acc::<E::Scalar>::default();

        for i in 0..right {
            let base = i * left;
            let mut inner_acc = SmallAccumulator::zero();

            for j in 0..left {
                let k = base + j;
                let az_diff = Az2_i64[k] as i128 - Az1_i64[k] as i128;
                let bz_diff = Bz2_i64[k] as i128 - Bz1_i64[k] as i128;
                let quad_val = az_diff * bz_diff;
                inner_acc.accumulate(e_left[j].to_limbs(), quad_val);
            }

            let inner_quad_red = inner_acc.reduce::<E::Scalar>();
            <E::Scalar as DelayedReduction<E::Scalar>>::unreduced_multiply_accumulate(
                &mut acc_quad,
                &f[i],
                &inner_quad_red,
            );
        }

        let mut quad = <E::Scalar as DelayedReduction<E::Scalar>>::reduce(&acc_quad);

        // Correction for large-value positions: add field arithmetic for positions
        // where the i64 path contributed 0 instead of the correct value.
        if !large_positions.is_empty() {
            for &k in large_positions {
                if k >= total {
                    continue;
                }
                let i = k / left;
                let j = k % left;
                let az_diff = Az2[k] - Az1[k];
                let bz_diff = Bz2[k] - Bz1[k];
                quad += f[i] * e_left[j] * az_diff * bz_diff;
            }
        }

        quad
    }

    /// Small-value prove_helper for rounds 1+ using cross-product decomposition.
    ///
    /// Instead of working on folded field data, computes (e0_ab, quad) directly from
    /// 4 original i64 layers per prove pair. The folded values are:
    ///   Az_lo[k] = (1-r_0)*a_0[k] + r_0*a_1[k]
    ///   Az_hi[k] = (1-r_0)*a_2[k] + r_0*a_3[k]
    ///
    /// The products decompose as cross-product sums with 3 weight classes:
    ///   c_0_0 = (1-r_0)^2, c_0_1 = (1-r_0)*r_0, c_1_1 = r_0^2
    #[inline(always)]
    #[allow(clippy::needless_range_loop)]
    fn prove_helper_ab_cross(
        (left, right): (usize, usize),
        e: &[E::Scalar],
        a_i64: [&[i64]; 4],
        b_i64: [&[i64]; 4],
        a_field: [&[E::Scalar]; 4],
        b_field: [&[E::Scalar]; 4],
        c00: &E::Scalar,
        c01: &E::Scalar,
        c11: &E::Scalar,
        r0: &E::Scalar,
        large_positions: &[usize],
    ) -> (E::Scalar, E::Scalar) {
        type Acc<S> = <S as DelayedReduction<S>>::Accumulator;

        let f = &e[left..];
        let e_left = &e[..left];
        let total = left * right;

        let mut acc_e0 = Acc::<E::Scalar>::default();
        let mut acc_quad = Acc::<E::Scalar>::default();

        for i in 0..right {
            let base = i * left;

            // Process e0 cross-product terms (Az_lo * Bz_lo)
            let mut sa_e0_00 = SmallAccumulator::zero();
            let mut sa_e0_01 = SmallAccumulator::zero();
            let mut sa_e0_11 = SmallAccumulator::zero();

            for j in 0..left {
                let k = base + j;
                let limbs = e_left[j].to_limbs();
                let (a0, a1) = (a_i64[0][k] as i128, a_i64[1][k] as i128);
                let (b0, b1) = (b_i64[0][k] as i128, b_i64[1][k] as i128);
                sa_e0_00.accumulate(limbs, a0 * b0);
                sa_e0_01.accumulate(limbs, a0 * b1 + a1 * b0);
                sa_e0_11.accumulate(limbs, a1 * b1);
            }

            let e0_inner = *c00 * sa_e0_00.reduce::<E::Scalar>()
                + *c01 * sa_e0_01.reduce::<E::Scalar>()
                + *c11 * sa_e0_11.reduce::<E::Scalar>();
            <E::Scalar as DelayedReduction<E::Scalar>>::unreduced_multiply_accumulate(
                &mut acc_e0,
                &f[i],
                &e0_inner,
            );

            // Process quad cross-product terms ((Az_hi-Az_lo) * (Bz_hi-Bz_lo))
            let mut sa_q_00 = SmallAccumulator::zero();
            let mut sa_q_01 = SmallAccumulator::zero();
            let mut sa_q_11 = SmallAccumulator::zero();

            for j in 0..left {
                let k = base + j;
                let limbs = e_left[j].to_limbs();
                let (da0, da1) = (
                    a_i64[2][k] as i128 - a_i64[0][k] as i128,
                    a_i64[3][k] as i128 - a_i64[1][k] as i128,
                );
                let (db0, db1) = (
                    b_i64[2][k] as i128 - b_i64[0][k] as i128,
                    b_i64[3][k] as i128 - b_i64[1][k] as i128,
                );
                sa_q_00.accumulate(limbs, da0 * db0);
                sa_q_01.accumulate(limbs, da0 * db1 + da1 * db0);
                sa_q_11.accumulate(limbs, da1 * db1);
            }

            let quad_inner = *c00 * sa_q_00.reduce::<E::Scalar>()
                + *c01 * sa_q_01.reduce::<E::Scalar>()
                + *c11 * sa_q_11.reduce::<E::Scalar>();
            <E::Scalar as DelayedReduction<E::Scalar>>::unreduced_multiply_accumulate(
                &mut acc_quad,
                &f[i],
                &quad_inner,
            );
        }

        let mut e0 = <E::Scalar as DelayedReduction<E::Scalar>>::reduce(&acc_e0);
        let mut quad = <E::Scalar as DelayedReduction<E::Scalar>>::reduce(&acc_quad);

        // Correct for large-value positions
        if !large_positions.is_empty() {
            let one_minus_r0 = E::Scalar::ONE - *r0;
            for &k in large_positions {
                if k >= total {
                    continue;
                }
                let i = k / left;
                let j = k % left;
                let ej_fi = e_left[j] * f[i];

                let az_lo = one_minus_r0 * a_field[0][k] + *r0 * a_field[1][k];
                let az_hi = one_minus_r0 * a_field[2][k] + *r0 * a_field[3][k];
                let bz_lo = one_minus_r0 * b_field[0][k] + *r0 * b_field[1][k];
                let bz_hi = one_minus_r0 * b_field[2][k] + *r0 * b_field[3][k];

                e0 += ej_fi * az_lo * bz_lo;
                quad += ej_fi * (az_hi - az_lo) * (bz_hi - bz_lo);
            }
        }

        (e0, quad)
    }

    /// Parallel fold of A/B layer chunks of size 4. Each chunk folds
    /// indices [0,1] into [0] and indices [2,3] into [2] using `r_b`.
    /// The resulting folded vectors are at positions 4j and 4j+2 after this call.
    /// Use `compact_folded_layers` to move them to 2j and 2j+1.
    fn par_fold_ab_chunks(a: &mut [Vec<E::Scalar>], b: &mut [Vec<E::Scalar>], r_b: E::Scalar) {
        a.par_chunks_mut(4)
            .zip(b.par_chunks_mut(4))
            .for_each(|(a_chunk, b_chunk)| {
                {
                    let (lo, hi) = a_chunk.split_at_mut(1);
                    lo[0]
                        .iter_mut()
                        .zip(hi[0].iter())
                        .for_each(|(l, h)| *l += r_b * (*h - *l));
                }
                {
                    let (lo, hi) = a_chunk.split_at_mut(3);
                    lo[2]
                        .iter_mut()
                        .zip(hi[0].iter())
                        .for_each(|(l, h)| *l += r_b * (*h - *l));
                }
                {
                    let (lo, hi) = b_chunk.split_at_mut(1);
                    lo[0]
                        .iter_mut()
                        .zip(hi[0].iter())
                        .for_each(|(l, h)| *l += r_b * (*h - *l));
                }
                {
                    let (lo, hi) = b_chunk.split_at_mut(3);
                    lo[2]
                        .iter_mut()
                        .zip(hi[0].iter())
                        .for_each(|(l, h)| *l += r_b * (*h - *l));
                }
            });
    }

    /// Compact folded results from positions [4j, 4j+2] (for j in 0..prove_pairs)
    /// down to positions [2j, 2j+1]. Runs serially but only does O(prove_pairs)
    /// swaps of `Vec` handles (pointer swaps, not data copies).
    fn compact_folded_layers(
        a: &mut [Vec<E::Scalar>],
        b: &mut [Vec<E::Scalar>],
        prove_pairs: usize,
    ) {
        for j in 0..prove_pairs {
            a.swap(2 * j, 4 * j);
            a.swap(2 * j + 1, 4 * j + 2);
            b.swap(2 * j, 4 * j);
            b.swap(2 * j + 1, 4 * j + 2);
        }
    }

    /// Like `compact_folded_layers` but also handles C layers (for non-i64 path).
    fn compact_folded_layers_abc(
        a: &mut [Vec<E::Scalar>],
        b: &mut [Vec<E::Scalar>],
        c: &mut [Vec<E::Scalar>],
        prove_pairs: usize,
    ) {
        for j in 0..prove_pairs {
            a.swap(2 * j, 4 * j);
            a.swap(2 * j + 1, 4 * j + 2);
            b.swap(2 * j, 4 * j);
            b.swap(2 * j + 1, 4 * j + 2);
            c.swap(2 * j, 4 * j);
            c.swap(2 * j + 1, 4 * j + 2);
        }
    }

    /// ZK version of NeutronNova NIFS prove. This function performs the NIFS folding
    /// rounds while interacting with the multi-round verifier circuit/state to derive
    /// per-round challenges via Fiat-Shamir, and populates the verifier circuit's
    /// NIFS-related public values. It returns:
    /// - the constructed NIFS (list of cubic univariate polynomials),
    /// - the split equality polynomial evaluations E (length left+right),
    /// - the final A/B/C layers after folding (as multilinear tables),
    /// - the final outer claim T_out for the step branch, and
    /// - the sequence of challenges r_b used to fold instances/witnesses.
    pub fn prove(
        S: &SplitR1CSShape<E>,
        ck: &CommitmentKey<E>,
        Us: Vec<R1CSInstance<E>>,
        Ws: Vec<R1CSWitness<E>>,
        cached_matvec: Option<Vec<(Vec<E::Scalar>, Vec<E::Scalar>, Vec<E::Scalar>)>>,
        cached_i64: Option<Vec<(Vec<i64>, Vec<i64>, Vec<i64>)>>,
        large_positions: &[usize],
        vc: &mut VegaMcVerifierCircuit<E>,
        vc_state: &mut <SatisfyingAssignment<E> as MultiRoundVegaWitness<E>>::MultiRoundState,
        vc_shape: &SplitMultiRoundR1CSShape<E>,
        vc_ck: &CommitmentKey<E>,
        transcript: &mut E::TE,
    ) -> Result<
        (
            Vec<E::Scalar>,  // E_eq (split evals, length left+right)
            Vec<E::Scalar>,  // Az layer 0
            Vec<E::Scalar>,  // Bz layer 0
            Vec<E::Scalar>,  // Cz layer 0
            R1CSWitness<E>,  // final folded witness
            R1CSInstance<E>, // final folded instance
        ),
        VegaError,
    > {
        // Determine padding and NIFS rounds
        let n = Us.len();
        let n_padded = Us.len().next_power_of_two();
        let ell_b = n_padded.log_2();

        info!(
            "NeutronNova NIFS prove for {} instances and padded to {} instances",
            Us.len(),
            n_padded
        );

        let mut Us = Us;
        let mut Ws = Ws;
        if Us.len() < n_padded {
            Us.extend(vec![Us[0].clone(); n_padded - n]);
            Ws.extend(vec![Ws[0].clone(); n_padded - n]);
        }
        for U in Us.iter() {
            transcript.absorb(b"U", U);
        }
        let T = E::Scalar::ZERO;
        transcript.absorb(b"T", &T);

        // Squeeze tau and rhos fresh inside this function (like ZK sum-check APIs)
        let (ell_cons, left, right) = compute_tensor_decomp(S.num_cons);
        let tau = transcript.squeeze(b"tau")?;

        let E_eq = PowPolynomial::split_evals(tau, ell_cons, left, right);

        let mut rhos = Vec::with_capacity(ell_b);
        for _ in 0..ell_b {
            rhos.push(transcript.squeeze(b"rho")?);
        }

        // Build Az, Bz, Cz tables for each (possibly padded) instance

        // Split cached matvec: consume owned triples for cached instances, compute rest
        let mut A_layers: Vec<Vec<E::Scalar>> = Vec::with_capacity(n_padded);
        let mut B_layers: Vec<Vec<E::Scalar>> = Vec::with_capacity(n_padded);
        let mut C_layers: Vec<Vec<E::Scalar>> = Vec::with_capacity(n_padded);

        let n_cached = cached_matvec.as_ref().map_or(0, |c| c.len());
        if let Some(cached) = cached_matvec {
            for (a, b, c) in cached {
                A_layers.push(a);
                B_layers.push(b);
                C_layers.push(c);
            }
        }
        // Compute matvec for any remaining (padded) instances
        for i in n_cached..n_padded {
            let w = &Ws[i].W;
            let x = &Us[i].X;
            let mut z = Vec::with_capacity(w.len() + 1 + x.len());
            z.extend_from_slice(w);
            z.push(E::Scalar::ONE);
            z.extend_from_slice(x);
            let (a, b, c) = S.multiply_vec(&z)?;
            A_layers.push(a);
            B_layers.push(b);
            C_layers.push(c);
        }
        // Build i64 layers for small-value NIFS optimization
        let n_i64_cached = cached_i64.as_ref().map_or(0, |c| c.len());
        let mut A_i64_layers: Vec<Vec<i64>> = Vec::with_capacity(n_padded);
        let mut B_i64_layers: Vec<Vec<i64>> = Vec::with_capacity(n_padded);
        let mut C_i64_layers: Vec<Vec<i64>> = Vec::with_capacity(n_padded);
        let has_i64 = cached_i64.is_some();
        if let Some(cached) = cached_i64 {
            for (a, b, c) in cached {
                A_i64_layers.push(a);
                B_i64_layers.push(b);
                C_i64_layers.push(c);
            }
        }
        // For padded instances, convert from field layers.
        // Padded instances are clones of Us[0], so their large positions should be a subset
        // of the global large_positions. We still zero at all global positions for safety.
        for i in n_i64_cached..n_padded {
            if has_i64 {
                let (mut a_i64, a_large) = to_small_vec_or_zero(&A_layers[i]);
                let (mut b_i64, b_large) = to_small_vec_or_zero(&B_layers[i]);
                let (mut c_i64, c_large) = to_small_vec_or_zero(&C_layers[i]);
                // Verify padded instance large positions are covered by global large_positions
                debug_assert!(
                    a_large.iter().all(|p| large_positions.contains(p)),
                    "padded instance has large position not in global set"
                );
                debug_assert!(
                    b_large.iter().all(|p| large_positions.contains(p)),
                    "padded instance has large position not in global set"
                );
                debug_assert!(
                    c_large.iter().all(|p| large_positions.contains(p)),
                    "padded instance has large position not in global set"
                );
                // Zero at global large_positions to maintain the invariant
                for &pos in large_positions {
                    a_i64[pos] = 0;
                    b_i64[pos] = 0;
                    c_i64[pos] = 0;
                }
                A_i64_layers.push(a_i64);
                B_i64_layers.push(b_i64);
                C_i64_layers.push(c_i64);
            }
        }

        // Execute NIFS rounds, generating cubic polynomials and driving r_b via multi-round state

        // Precompute C_val[b] = sum_k E_eq[k] * Cz_b[k] for each instance b.
        // This lets us skip C in fold_abc_pair and prove_helper, computing
        // the C contribution to e0 as a weighted sum of these scalars instead.
        // Uses two-level structure: E[k] = e_left[j] * f[i] where k = i*left + j.
        let c_vals: Vec<E::Scalar> = if has_i64 {
            let e_left = &E_eq[..left];
            let f = &E_eq[left..];

            let mut vals: Vec<E::Scalar> = (0..n_padded)
                .into_par_iter()
                .map(|b| {
                    let c_i64 = &C_i64_layers[b];
                    type Acc<S> = <S as DelayedReduction<S>>::Accumulator;
                    let mut acc = Acc::<E::Scalar>::default();
                    #[allow(clippy::needless_range_loop)]
                    for i in 0..right {
                        let base = i * left;
                        let mut inner = SmallAccumulator::zero();
                        for j in 0..left {
                            inner.accumulate(e_left[j].to_limbs(), c_i64[base + j] as i128);
                        }
                        let inner_red = inner.reduce::<E::Scalar>();
                        <E::Scalar as DelayedReduction<E::Scalar>>::unreduced_multiply_accumulate(
                            &mut acc, &f[i], &inner_red,
                        );
                    }
                    <E::Scalar as DelayedReduction<E::Scalar>>::reduce(&acc)
                })
                .collect();

            // Correct for large positions where i64 was 0 instead of actual value
            if !large_positions.is_empty() {
                let total = left * right;
                for &k in large_positions {
                    if k >= total {
                        continue;
                    }
                    let i = k / left;
                    let j = k % left;
                    let ej_fi = e_left[j] * f[i];
                    for b in 0..n_padded {
                        vals[b] += ej_fi * C_layers[b][k];
                    }
                }
            }
            vals
        } else {
            vec![]
        };

        let mut polys: Vec<UniPoly<E::Scalar>> = Vec::with_capacity(ell_b);
        let mut r_bs: Vec<E::Scalar> = Vec::with_capacity(ell_b);
        let mut T_cur = E::Scalar::ZERO; // the current target value, starts at 0
        let mut acc_eq = E::Scalar::ONE;
        let mut m = n_padded;

        // Helper closure: build polynomial, process round, extract r_b
        // (factored out since it's identical for standalone and merged rounds)
        macro_rules! finish_round {
            ($t:expr, $e0:expr, $quad_coeff:expr) => {{
                let rho_t = rhos[$t];
                let one_minus_rho = E::Scalar::ONE - rho_t;
                let two_rho_minus_one = rho_t - one_minus_rho;
                let c = $e0 * acc_eq;
                let a = $quad_coeff * acc_eq;
                let rho_t_inv: Option<E::Scalar> = rho_t.invert().into();
                let a_b_c =
                    (T_cur - c * one_minus_rho) * rho_t_inv.ok_or(VegaError::DivisionByZero)?;
                let b = a_b_c - a - c;
                let new_a = a * two_rho_minus_one;
                let new_b = b * two_rho_minus_one + a * one_minus_rho;
                let new_c = c * two_rho_minus_one + b * one_minus_rho;
                let new_d = c * one_minus_rho;

                let poly_t = UniPoly {
                    coeffs: vec![new_d, new_c, new_b, new_a],
                };
                polys.push(poly_t.clone());

                let c = &poly_t.coeffs;
                vc.nifs_polys[$t] = [c[0], c[1], c[2], c[3]];

                let chals = SatisfyingAssignment::<E>::process_round(
                    vc_state, vc_shape, vc_ck, vc, $t, transcript,
                )?;
                let r_b = chals[0];
                r_bs.push(r_b);

                acc_eq *= (E::Scalar::ONE - r_b) * (E::Scalar::ONE - rho_t) + r_b * rho_t;
                T_cur = poly_t.evaluate(&r_b);
                r_b
            }};
        }

        // Helper closure: fold one A/B pair from src indices to dest index.
        // C layers are NOT folded -- C contribution is handled via precomputed c_vals when has_i64.
        macro_rules! fold_ab_pair {
            ($src_even:expr, $src_odd:expr, $dest:expr, $r_b:expr) => {{
                {
                    let even = std::mem::take(&mut A_layers[$src_even]);
                    let odd = &A_layers[$src_odd];
                    let mut folded = even;
                    folded.iter_mut().zip(odd.iter()).for_each(|(l, h)| {
                        *l += $r_b * (*h - *l);
                    });
                    A_layers[$dest] = folded;
                }
                {
                    let even = std::mem::take(&mut B_layers[$src_even]);
                    let odd = &B_layers[$src_odd];
                    let mut folded = even;
                    folded.iter_mut().zip(odd.iter()).for_each(|(l, h)| {
                        *l += $r_b * (*h - *l);
                    });
                    B_layers[$dest] = folded;
                }
            }};
        }

        // Full A/B/C fold for fallback (non-i64) path
        macro_rules! fold_abc_pair {
            ($src_even:expr, $src_odd:expr, $dest:expr, $r_b:expr) => {{
                fold_ab_pair!($src_even, $src_odd, $dest, $r_b);
                {
                    let even = std::mem::take(&mut C_layers[$src_even]);
                    let odd = &C_layers[$src_odd];
                    let mut folded = even;
                    folded.iter_mut().zip(odd.iter()).for_each(|(l, h)| {
                        *l += $r_b * (*h - *l);
                    });
                    C_layers[$dest] = folded;
                }
            }};
        }

        // Round 0: prove_helper (compute_e0 = false for round 0)
        // Uses small-value integer arithmetic when i64 data is available.
        {
            let pairs = m / 2;
            let (e0, quad_coeff) = if has_i64 {
                // Small-value fast path: i64 subtraction + i128 multiplication
                let quad_coeff = A_layers
                    .par_chunks(2)
                    .zip(B_layers.par_chunks(2))
                    .zip(A_i64_layers.par_chunks(2))
                    .zip(B_i64_layers.par_chunks(2))
                    .enumerate()
                    .map(|(pair_idx, (((pair_a, pair_b), pair_a_i64), pair_b_i64))| {
                        let qc = Self::prove_helper_small(
                            (left, right),
                            &E_eq,
                            &pair_a[0],
                            &pair_b[0],
                            &pair_a[1],
                            &pair_b[1],
                            &pair_a_i64[0],
                            &pair_b_i64[0],
                            &pair_a_i64[1],
                            &pair_b_i64[1],
                            large_positions,
                        );
                        let w = suffix_weight_full::<E::Scalar>(0, ell_b, pair_idx, &rhos);
                        qc * w
                    })
                    .reduce(|| E::Scalar::ZERO, |a, b| a + b);
                (E::Scalar::ZERO, quad_coeff)
            } else {
                // Standard field arithmetic path
                A_layers
                    .par_chunks(2)
                    .zip(B_layers.par_chunks(2))
                    .zip(C_layers.par_chunks(2))
                    .enumerate()
                    .map(|(pair_idx, ((pair_a, pair_b), pair_c))| {
                        let (e0, quad_coeff) = Self::prove_helper(
                            0,
                            (left, right),
                            &E_eq,
                            &pair_a[0],
                            &pair_b[0],
                            &pair_c[0],
                            &pair_a[1],
                            &pair_b[1],
                        );
                        let w = suffix_weight_full::<E::Scalar>(0, ell_b, pair_idx, &rhos);
                        (e0 * w, quad_coeff * w)
                    })
                    .reduce(
                        || (E::Scalar::ZERO, E::Scalar::ZERO),
                        |a, b| (a.0 + b.0, a.1 + b.1),
                    )
            };
            let r_b = finish_round!(0, e0, quad_coeff);

            if ell_b == 1 {
                for i in 0..pairs {
                    if has_i64 {
                        fold_ab_pair!(2 * i, 2 * i + 1, i, r_b);
                    } else {
                        fold_abc_pair!(2 * i, 2 * i + 1, i, r_b);
                    }
                }
                A_layers.truncate(pairs);
                B_layers.truncate(pairs);
                if !has_i64 {
                    C_layers.truncate(pairs);
                }
                m = pairs;
            }
        }

        // Rounds 1..ell_b-1: merged fold(prev round) + prove_helper(current round)
        // When has_i64: skip C folds, use prove_helper_ab_only, subtract precomputed c_vals.
        if ell_b > 1 {
            let mut prev_r_b = r_bs[0];

            // Build prefix_coeffs incrementally: eq(v, (r_0, r_1, ...))
            // After round 0, the first challenge r_0 gives prefix = [(1-r_0), r_0]
            let mut prefix_coeffs: Vec<E::Scalar> = if has_i64 {
                let r0 = r_bs[0];
                vec![E::Scalar::ONE - r0, r0]
            } else {
                vec![]
            };

            for t in 1..ell_b {
                let fold_pairs = m / 2;
                let prove_pairs = fold_pairs / 2;
                let mut e0_acc = E::Scalar::ZERO;
                let mut quad_acc = E::Scalar::ZERO;

                if has_i64 {
                    let n_prefix = prefix_coeffs.len(); // 2^t

                    // Round 1 special case: use SA cross-product from i64 originals
                    if t == 1 {
                        let r0 = prev_r_b;
                        let one_minus_r0 = E::Scalar::ONE - r0;
                        let c00 = one_minus_r0 * one_minus_r0;
                        let c01 = one_minus_r0 * r0;
                        let c11 = r0 * r0;

                        // Parallel prove (read-only of A_layers / A_i64_layers etc.)
                        let prefix_coeffs_ref = &prefix_coeffs;
                        let c_vals_ref = &c_vals;
                        let e_eq_ref = &E_eq;
                        let rhos_ref = &rhos;
                        let a_layers_ref = &A_layers;
                        let b_layers_ref = &B_layers;
                        let a_i64_ref = &A_i64_layers;
                        let b_i64_ref = &B_i64_layers;
                        let (e0_sum, qc_sum) = (0..prove_pairs)
                            .into_par_iter()
                            .map(|j| {
                                let (e0_ab, qc) = Self::prove_helper_ab_cross(
                                    (left, right),
                                    e_eq_ref,
                                    [
                                        &a_i64_ref[4 * j],
                                        &a_i64_ref[4 * j + 1],
                                        &a_i64_ref[4 * j + 2],
                                        &a_i64_ref[4 * j + 3],
                                    ],
                                    [
                                        &b_i64_ref[4 * j],
                                        &b_i64_ref[4 * j + 1],
                                        &b_i64_ref[4 * j + 2],
                                        &b_i64_ref[4 * j + 3],
                                    ],
                                    [
                                        &a_layers_ref[4 * j],
                                        &a_layers_ref[4 * j + 1],
                                        &a_layers_ref[4 * j + 2],
                                        &a_layers_ref[4 * j + 3],
                                    ],
                                    [
                                        &b_layers_ref[4 * j],
                                        &b_layers_ref[4 * j + 1],
                                        &b_layers_ref[4 * j + 2],
                                        &b_layers_ref[4 * j + 3],
                                    ],
                                    &c00,
                                    &c01,
                                    &c11,
                                    &r0,
                                    large_positions,
                                );
                                let lo_base = (2 * j) * n_prefix;
                                let mut c_val_lo = E::Scalar::ZERO;
                                for v in 0..n_prefix {
                                    c_val_lo += prefix_coeffs_ref[v] * c_vals_ref[lo_base + v];
                                }
                                let e0 = e0_ab - c_val_lo;
                                let w = suffix_weight_full::<E::Scalar>(t, ell_b, j, rhos_ref);
                                (e0 * w, qc * w)
                            })
                            .reduce(
                                || (E::Scalar::ZERO, E::Scalar::ZERO),
                                |a, b| (a.0 + b.0, a.1 + b.1),
                            );
                        e0_acc += e0_sum;
                        quad_acc += qc_sum;

                        // Parallel fold of all prove_pairs chunks (field arithmetic)
                        if prove_pairs > 0 {
                            Self::par_fold_ab_chunks(
                                &mut A_layers[..4 * prove_pairs],
                                &mut B_layers[..4 * prove_pairs],
                                prev_r_b,
                            );
                            // Compact folded results from positions [4j, 4j+2] into [2j, 2j+1]
                            Self::compact_folded_layers(&mut A_layers, &mut B_layers, prove_pairs);
                        }
                        // Tail fold (for odd fold_pairs / non-power-of-two cases)
                        for i in (2 * prove_pairs)..fold_pairs {
                            fold_ab_pair!(2 * i, 2 * i + 1, i, prev_r_b);
                        }
                    } else {
                        // Rounds 2+: merged parallel fold + prove from field data
                        let prefix_coeffs_ref = &prefix_coeffs;
                        let c_vals_ref = &c_vals;
                        let e_eq_ref = &E_eq;
                        let rhos_ref = &rhos;

                        let (a_head, _) = A_layers.split_at_mut(4 * prove_pairs);
                        let (b_head, _) = B_layers.split_at_mut(4 * prove_pairs);

                        let (e0_sum, qc_sum) = a_head
                            .par_chunks_mut(4)
                            .zip(b_head.par_chunks_mut(4))
                            .enumerate()
                            .map(|(j, (a_chunk, b_chunk))| {
                                // Fold a_chunk[0] += r * (a_chunk[1] - a_chunk[0])
                                {
                                    let (lo, hi) = a_chunk.split_at_mut(1);
                                    lo[0]
                                        .iter_mut()
                                        .zip(hi[0].iter())
                                        .for_each(|(l, h)| *l += prev_r_b * (*h - *l));
                                }
                                // Fold a_chunk[2] += r * (a_chunk[3] - a_chunk[2])
                                {
                                    let (lo, hi) = a_chunk.split_at_mut(3);
                                    lo[2]
                                        .iter_mut()
                                        .zip(hi[0].iter())
                                        .for_each(|(l, h)| *l += prev_r_b * (*h - *l));
                                }
                                // Fold b_chunk[0] and b_chunk[2] similarly
                                {
                                    let (lo, hi) = b_chunk.split_at_mut(1);
                                    lo[0]
                                        .iter_mut()
                                        .zip(hi[0].iter())
                                        .for_each(|(l, h)| *l += prev_r_b * (*h - *l));
                                }
                                {
                                    let (lo, hi) = b_chunk.split_at_mut(3);
                                    lo[2]
                                        .iter_mut()
                                        .zip(hi[0].iter())
                                        .for_each(|(l, h)| *l += prev_r_b * (*h - *l));
                                }
                                // Prove from folded positions [0] and [2]
                                let (e0_ab, qc) = Self::prove_helper_ab_only(
                                    (left, right),
                                    e_eq_ref,
                                    &a_chunk[0],
                                    &b_chunk[0],
                                    &a_chunk[2],
                                    &b_chunk[2],
                                );
                                let lo_base = (2 * j) * n_prefix;
                                let mut c_val_lo = E::Scalar::ZERO;
                                for v in 0..n_prefix {
                                    c_val_lo += prefix_coeffs_ref[v] * c_vals_ref[lo_base + v];
                                }
                                let e0 = e0_ab - c_val_lo;
                                let w = suffix_weight_full::<E::Scalar>(t, ell_b, j, rhos_ref);
                                (e0 * w, qc * w)
                            })
                            .reduce(
                                || (E::Scalar::ZERO, E::Scalar::ZERO),
                                |a, b| (a.0 + b.0, a.1 + b.1),
                            );
                        e0_acc += e0_sum;
                        quad_acc += qc_sum;

                        // Compact folded results from positions [4j, 4j+2] into [2j, 2j+1]
                        Self::compact_folded_layers(&mut A_layers, &mut B_layers, prove_pairs);

                        for i in (2 * prove_pairs)..fold_pairs {
                            fold_ab_pair!(2 * i, 2 * i + 1, i, prev_r_b);
                        }
                    }
                } else {
                    // Parallel merged fold + prove for the non-i64 path (mirrors the i64 structure above).
                    let e_eq_ref = &E_eq;
                    let rhos_ref = &rhos;

                    let (a_head, _) = A_layers.split_at_mut(4 * prove_pairs);
                    let (b_head, _) = B_layers.split_at_mut(4 * prove_pairs);
                    let (c_head, _) = C_layers.split_at_mut(4 * prove_pairs);

                    let (e0_sum, qc_sum) = a_head
                        .par_chunks_mut(4)
                        .zip(b_head.par_chunks_mut(4))
                        .zip(c_head.par_chunks_mut(4))
                        .enumerate()
                        .map(|(j, ((a_chunk, b_chunk), c_chunk))| {
                            // Fold [0] += r * ([1] - [0]) and [2] += r * ([3] - [2]) for A, B, C
                            for chunk in [&mut *a_chunk, &mut *b_chunk, &mut *c_chunk] {
                                {
                                    let (lo, hi) = chunk.split_at_mut(1);
                                    lo[0]
                                        .iter_mut()
                                        .zip(hi[0].iter())
                                        .for_each(|(l, h)| *l += prev_r_b * (*h - *l));
                                }
                                {
                                    let (lo, hi) = chunk.split_at_mut(3);
                                    lo[2]
                                        .iter_mut()
                                        .zip(hi[0].iter())
                                        .for_each(|(l, h)| *l += prev_r_b * (*h - *l));
                                }
                            }
                            // Prove from folded positions [0] and [2]
                            let (e0, qc) = Self::prove_helper(
                                t,
                                (left, right),
                                e_eq_ref,
                                &a_chunk[0],
                                &b_chunk[0],
                                &c_chunk[0],
                                &a_chunk[2],
                                &b_chunk[2],
                            );
                            let w = suffix_weight_full::<E::Scalar>(t, ell_b, j, rhos_ref);
                            (e0 * w, qc * w)
                        })
                        .reduce(
                            || (E::Scalar::ZERO, E::Scalar::ZERO),
                            |a, b| (a.0 + b.0, a.1 + b.1),
                        );
                    e0_acc += e0_sum;
                    quad_acc += qc_sum;

                    // Compact folded results from positions [4j, 4j+2] into [2j, 2j+1]
                    Self::compact_folded_layers_abc(
                        &mut A_layers,
                        &mut B_layers,
                        &mut C_layers,
                        prove_pairs,
                    );

                    for i in (2 * prove_pairs)..fold_pairs {
                        fold_abc_pair!(2 * i, 2 * i + 1, i, prev_r_b);
                    }
                }

                A_layers.truncate(fold_pairs);
                B_layers.truncate(fold_pairs);
                if !has_i64 {
                    C_layers.truncate(fold_pairs);
                }
                m = fold_pairs;
                prev_r_b = finish_round!(t, e0_acc, quad_acc);

                // Extend prefix_coeffs: each c splits into c*(1-r_t) and c*r_t
                if has_i64 {
                    let r_t = prev_r_b;
                    let one_minus_r_t = E::Scalar::ONE - r_t;
                    let old = std::mem::take(&mut prefix_coeffs);
                    prefix_coeffs = Vec::with_capacity(old.len() * 2);
                    // Concatenate: first old*(1-r_t), then old*r_t.
                    // This matches the fold structure where bit t occupies the UPPER half
                    // of each group: v = v_old + n_old * b_t.
                    for c in &old {
                        prefix_coeffs.push(*c * one_minus_r_t);
                    }
                    for c in &old {
                        prefix_coeffs.push(*c * r_t);
                    }
                }
            }

            // Final fold: fold remaining A/B layers
            let final_pairs = m / 2;
            if has_i64 && final_pairs > 0 {
                // Parallel fold over pairs (2*final_pairs elements → final_pairs elements)
                A_layers[..2 * final_pairs]
                    .par_chunks_mut(2)
                    .zip(B_layers[..2 * final_pairs].par_chunks_mut(2))
                    .for_each(|(a_chunk, b_chunk)| {
                        {
                            let (lo, hi) = a_chunk.split_at_mut(1);
                            lo[0]
                                .iter_mut()
                                .zip(hi[0].iter())
                                .for_each(|(l, h)| *l += prev_r_b * (*h - *l));
                        }
                        {
                            let (lo, hi) = b_chunk.split_at_mut(1);
                            lo[0]
                                .iter_mut()
                                .zip(hi[0].iter())
                                .for_each(|(l, h)| *l += prev_r_b * (*h - *l));
                        }
                    });
                // After parallel fold, folded results are at positions 2i (i in 0..final_pairs).
                // Compact to positions [0..final_pairs].
                for i in 0..final_pairs {
                    A_layers.swap(i, 2 * i);
                    B_layers.swap(i, 2 * i);
                }
            } else {
                for i in 0..final_pairs {
                    if has_i64 {
                        fold_ab_pair!(2 * i, 2 * i + 1, i, prev_r_b);
                    } else {
                        fold_abc_pair!(2 * i, 2 * i + 1, i, prev_r_b);
                    }
                }
            }
            A_layers.truncate(final_pairs);
            B_layers.truncate(final_pairs);
            if !has_i64 {
                C_layers.truncate(final_pairs);
            }
        }

        // Compute final Cz_step from original C_i64 layers using SmallAccumulator
        if has_i64 {
            let final_weights = weights_from_r::<E::Scalar>(&r_bs, n_padded);
            let total = left * right;

            // Precompute weight limbs to avoid repeated to_limbs() calls
            let weight_limbs: Vec<&[u64; 4]> = final_weights.iter().map(|w| w.to_limbs()).collect();

            // Parallel across k: for each k, accumulate across all b serially.
            let mut cz_step: Vec<E::Scalar> = (0..total)
                .into_par_iter()
                .map(|k| {
                    let mut sa = SmallAccumulator::zero();
                    for b in 0..n_padded {
                        sa.accumulate(weight_limbs[b], C_i64_layers[b][k] as i128);
                    }
                    sa.reduce::<E::Scalar>()
                })
                .collect();

            // Correct large positions with field arithmetic
            if !large_positions.is_empty() {
                for &k in large_positions {
                    if k >= total {
                        continue;
                    }
                    let mut val = E::Scalar::ZERO;
                    for b in 0..n_padded {
                        val += final_weights[b] * C_layers[b][k];
                    }
                    cz_step[k] = val;
                }
            }

            C_layers = vec![cz_step];
        }
        // T_out = poly_last(r_last) / eq(r_b, rho)
        let acc_eq_inv: Option<E::Scalar> = acc_eq.invert().into();
        let T_out = T_cur * acc_eq_inv.ok_or(VegaError::DivisionByZero)?;
        vc.t_out_step = T_out;
        vc.eq_rho_at_rb = acc_eq;
        let _ = SatisfyingAssignment::<E>::process_round(
            vc_state, vc_shape, vc_ck, vc, ell_b, transcript,
        )?;

        // Fast path: when the rest witness is pure padding (num_rest_unpadded == 0) and the
        // shared+precommitted prefix is nonempty, fold only that prefix and rebuild the zero
        // rest rows from the folded blind. Otherwise fold the full witness and commitment.
        let effective_len = S.num_shared + S.num_precommitted;
        let use_truncated_fold = effective_len > 0 && S.num_rest_unpadded == 0;
        if use_truncated_fold {
            for w in Ws.iter_mut() {
                w.W.truncate(effective_len);
            }
        }

        let (_fold_final_span, fold_final_t) = start_span!("fold_witnesses");
        let mut folded_W = R1CSWitness::fold_multiple(&r_bs, &Ws)?;
        if use_truncated_fold {
            let full_dim = S.num_shared + S.num_precommitted + S.num_rest;
            folded_W.W.resize(full_dim, E::Scalar::ZERO);
        }
        info!(elapsed_ms = %fold_final_t.elapsed().as_millis(), "fold_witnesses");

        // Optimized instance fold: only MSM data rows (shared+precommitted),
        // compute rest rows from folded blind + h (field arithmetic instead of MSM).
        // Fall back to full fold when shared+precommitted=0.
        let (_fold_final_span, fold_final_t) = start_span!("fold_instances");
        let w = weights_from_r::<E::Scalar>(&r_bs, Us.len());
        let d = Us[0].X.len();

        let mut X_acc = vec![E::Scalar::ZERO; d];
        for (i, Ui) in Us.iter().enumerate() {
            let wi = w[i];
            for (j, Uij) in Ui.X.iter().enumerate() {
                X_acc[j] += wi * Uij;
            }
        }

        let comms: Vec<_> = Us.iter().map(|U| U.comm_W.clone()).collect();
        let comm_acc = if use_truncated_fold {
            let num_data_rows =
                (S.num_shared + S.num_precommitted).div_ceil(DEFAULT_COMMITMENT_WIDTH);
            <E::PCS as FoldingEngineTrait<E>>::fold_commitments_partial(
                &comms,
                &w,
                num_data_rows,
                &folded_W.r_W,
                ck,
            )?
        } else {
            <E::PCS as FoldingEngineTrait<E>>::fold_commitments(&comms, &w)?
        };
        let folded_U = R1CSInstance::<E>::new_unchecked(comm_acc, X_acc)?;
        info!(elapsed_ms = %fold_final_t.elapsed().as_millis(), "fold_instances");

        Ok((
            E_eq,
            std::mem::take(&mut A_layers[0]),
            std::mem::take(&mut B_layers[0]),
            std::mem::take(&mut C_layers[0]),
            folded_W,
            folded_U,
        ))
    }
}

/// A type that represents the prover's key
#[derive(Serialize, Deserialize)]
#[serde(bound = "")]
pub struct VegaMcProverKey<E: Engine> {
    ck: CommitmentKey<E>,
    S_step: SplitR1CSShape<E>,
    S_core: SplitR1CSShape<E>,
    vk_digest: VegaDigest, // digest of the verifier's key
    vc_shape: SplitMultiRoundR1CSShape<E>,
    vc_shape_regular: R1CSShape<E>,
    vc_ck: CommitmentKey<E>,
}

/// A type that represents the verifier's key
#[derive(Serialize, Deserialize)]
#[serde(bound = "")]
pub struct VegaMcVerifierKey<E: Engine> {
    ck: CommitmentKey<E>,
    vk_ee: <E::PCS as PCSEngineTrait<E>>::VerifierKey,
    S_step: SplitR1CSShape<E>,
    S_core: SplitR1CSShape<E>,
    vc_shape: SplitMultiRoundR1CSShape<E>,
    vc_shape_regular: R1CSShape<E>,
    vc_ck: CommitmentKey<E>,
    vc_vk: VerifierKey<E>,
    num_steps: usize,
    #[serde(skip, default = "OnceCell::new")]
    digest: OnceCell<VegaDigest>,
}

/// Error returned by Iroha's narrow adapter for Microsoft's canonical Vega-MC
/// bincode representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrohaCanonicalCodecError {
    /// The canonical proof or verifier key could not be encoded.
    Encode,
    /// The bytes were not one complete canonical value.
    Decode,
    /// The supplied value or configured limit cannot be represented safely.
    SizeLimit,
}

impl<E: Engine> VegaMcVerifierKey<E> {
    /// Decode exactly one canonical Microsoft Vega-MC verifier key.
    ///
    /// The explicit limit is enforced by bincode before any attacker-controlled
    /// sequence allocation. Trailing bytes are rejected.
    pub fn decode_iroha_canonical(
        bytes: &[u8],
        byte_limit: usize,
    ) -> Result<Self, IrohaCanonicalCodecError> {
        if bytes.len() > byte_limit {
            return Err(IrohaCanonicalCodecError::SizeLimit);
        }
        let byte_limit =
            u64::try_from(byte_limit).map_err(|_| IrohaCanonicalCodecError::SizeLimit)?;
        bincode::DefaultOptions::new()
            .with_little_endian()
            .with_fixint_encoding()
            .with_limit(byte_limit)
            .reject_trailing_bytes()
            .deserialize(bytes)
            .map_err(|_| IrohaCanonicalCodecError::Decode)
    }
}

/// Exact sequence dimensions required by the canonical Vega-MC proof wire.
///
/// Iroha uses this read-only view for a non-allocating structural pre-scan
/// before Serde is allowed to deserialize an untrusted proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VegaMcProofDimensions {
    /// Number of step instances.
    pub num_steps: usize,
    /// Padded shared-witness variables reused by every step and the core.
    pub shared_variables: usize,
    /// Padded precommitted variables in each step instance.
    pub step_precommitted_variables: usize,
    /// Padded rest variables in each step instance.
    pub step_rest_variables: usize,
    /// Padded precommitted variables in the core instance.
    pub core_precommitted_variables: usize,
    /// Padded rest variables in the core instance.
    pub core_rest_variables: usize,
    /// Padded constraints in the uniform step shape.
    pub step_constraints: usize,
    /// Padded variables in the uniform step shape.
    pub step_variables: usize,
    /// Padded constraints in the core shape.
    pub core_constraints: usize,
    /// Padded variables in the core shape.
    pub core_variables: usize,
    /// Points in the hoisted shared-witness commitment.
    pub shared_commitment_points: usize,
    /// Points in each step precommitted commitment.
    pub step_precommitted_points: usize,
    /// Points in each step rest commitment.
    pub step_rest_points: usize,
    /// Public values in each step instance.
    pub step_public_values: usize,
    /// Fiat--Shamir challenges in each step instance.
    pub step_challenges: usize,
    /// Points in the core precommitted commitment.
    pub core_precommitted_points: usize,
    /// Points in the core rest commitment.
    pub core_rest_points: usize,
    /// Public values in the core instance.
    pub core_public_values: usize,
    /// Fiat--Shamir challenges in the core instance.
    pub core_challenges: usize,
    /// Scalars in the Hyrax linear IPA response vector.
    pub evaluation_response_scalars: usize,
    /// Points in each verifier-circuit round commitment.
    pub verifier_round_commitment_points: Vec<usize>,
    /// Public values in the verifier-circuit instance.
    pub verifier_public_values: usize,
    /// Challenges in each verifier-circuit round.
    pub verifier_challenges_per_round: Vec<usize>,
    /// Points in the Nova cross-term commitment.
    pub nova_cross_term_points: usize,
    /// Points in the random relaxed witness commitment.
    pub random_witness_commitment_points: usize,
    /// Points in the random relaxed error commitment.
    pub random_error_commitment_points: usize,
    /// Public scalars in the random relaxed instance.
    pub random_public_values: usize,
    /// Padded constraints in the in-circuit verifier's regular shape.
    pub verifier_constraints: usize,
    /// Padded variables in the in-circuit verifier's regular shape.
    pub verifier_variables: usize,
    /// Rounds in the relaxed-Spartan outer sum-check.
    pub relaxed_outer_rounds: usize,
    /// Stored scalars per relaxed-Spartan outer round polynomial.
    pub relaxed_outer_coefficients: usize,
    /// Rounds in the relaxed-Spartan inner sum-check.
    pub relaxed_inner_rounds: usize,
    /// Stored scalars per relaxed-Spartan inner round polynomial.
    pub relaxed_inner_coefficients: usize,
    /// Scalars in each relaxed-Spartan direct opening.
    pub relaxed_opening_scalars: usize,
}

impl<E: Engine> VegaMcVerifierKey<E> {
    /// Return all verifier-key-derived sequence dimensions in a proof.
    pub fn proof_dimensions(&self) -> VegaMcProofDimensions {
        let commitment_points = |values: usize, width: usize| values.div_ceil(width);
        let vc_width = self.vc_shape.commitment_width;
        let verifier_round_commitment_points = self
            .vc_shape
            .num_vars_per_round
            .iter()
            .map(|values| commitment_points(*values, vc_width))
            .collect();
        let relaxed_num_vars = self.vc_shape_regular.num_vars;
        let relaxed_num_cons = self.vc_shape_regular.num_cons;
        VegaMcProofDimensions {
            num_steps: self.num_steps,
            shared_variables: self.S_step.num_shared,
            step_precommitted_variables: self.S_step.num_precommitted,
            step_rest_variables: self.S_step.num_rest,
            core_precommitted_variables: self.S_core.num_precommitted,
            core_rest_variables: self.S_core.num_rest,
            step_constraints: self.S_step.num_cons,
            step_variables: self.S_step.num_shared
                + self.S_step.num_precommitted
                + self.S_step.num_rest,
            core_constraints: self.S_core.num_cons,
            core_variables: self.S_core.num_shared
                + self.S_core.num_precommitted
                + self.S_core.num_rest,
            shared_commitment_points: commitment_points(
                self.S_step.num_shared,
                DEFAULT_COMMITMENT_WIDTH,
            ),
            step_precommitted_points: commitment_points(
                self.S_step.num_precommitted,
                DEFAULT_COMMITMENT_WIDTH,
            ),
            step_rest_points: commitment_points(self.S_step.num_rest, DEFAULT_COMMITMENT_WIDTH),
            step_public_values: self.S_step.num_public,
            step_challenges: self.S_step.num_challenges,
            core_precommitted_points: commitment_points(
                self.S_core.num_precommitted,
                DEFAULT_COMMITMENT_WIDTH,
            ),
            core_rest_points: commitment_points(self.S_core.num_rest, DEFAULT_COMMITMENT_WIDTH),
            core_public_values: self.S_core.num_public,
            core_challenges: self.S_core.num_challenges,
            evaluation_response_scalars: DEFAULT_COMMITMENT_WIDTH,
            verifier_round_commitment_points,
            verifier_public_values: self.vc_shape.num_public,
            verifier_challenges_per_round: self.vc_shape.num_challenges_per_round.clone(),
            nova_cross_term_points: commitment_points(relaxed_num_cons, vc_width),
            random_witness_commitment_points: commitment_points(relaxed_num_vars, vc_width),
            random_error_commitment_points: commitment_points(relaxed_num_cons, vc_width),
            random_public_values: self.vc_shape_regular.num_io,
            verifier_constraints: relaxed_num_cons,
            verifier_variables: relaxed_num_vars,
            relaxed_outer_rounds: relaxed_num_cons.log_2(),
            relaxed_outer_coefficients: 3,
            relaxed_inner_rounds: relaxed_num_vars.next_power_of_two().log_2() + 1,
            relaxed_inner_coefficients: 2,
            relaxed_opening_scalars: vc_width,
        }
    }
}

impl<E: Engine> crate::digest::Digestible for VegaMcVerifierKey<E> {
    fn write_bytes<W: Sized + std::io::Write>(&self, w: &mut W) -> Result<(), std::io::Error> {
        let config = bincode::DefaultOptions::new()
            .with_little_endian()
            .with_fixint_encoding();
        config
            .serialize_into(&mut *w, &self.ck)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        config
            .serialize_into(&mut *w, &self.vk_ee)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        // Use fast raw-byte path for the R1CS shapes
        self.S_step.write_bytes(w)?;
        self.S_core.write_bytes(w)?;
        // Serialize remaining small fields with bincode
        config
            .serialize_into(&mut *w, &self.vc_shape)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        config
            .serialize_into(&mut *w, &self.vc_shape_regular)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        config
            .serialize_into(&mut *w, &self.vc_ck)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        config
            .serialize_into(&mut *w, &self.vc_vk)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        config
            .serialize_into(&mut *w, &self.num_steps)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(())
    }
}

impl<E: Engine> DigestHelperTrait<E> for VegaMcVerifierKey<E> {
    /// Returns the digest of the verifier's key.
    fn digest(&self) -> Result<VegaDigest, VegaError> {
        self.digest
            .get_or_try_init(|| {
                let dc = DigestComputer::<_>::new(self);
                dc.digest()
            })
            .cloned()
            .map_err(|_| VegaError::DigestError {
                reason: "Unable to compute digest for VegaVerifierKey".to_string(),
            })
    }
}

/// A type that holds the pre-processed state for proving
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct VegaMcPrepZkSNARK<E: Engine> {
    ps_step: Vec<PrecommittedState<E>>,
    ps_core: PrecommittedState<E>,
    /// Cached partial matrix-vector products for shared+precommitted columns per step circuit (deterministic).
    cached_step_matvec: Option<Vec<(Vec<E::Scalar>, Vec<E::Scalar>, Vec<E::Scalar>)>>,
    /// Small-value (i64) cache of Az/Bz/Cz for NIFS integer arithmetic.
    /// Large values are stored as 0 and corrected via field arithmetic using `large_positions`.
    cached_step_i64: Option<Vec<(Vec<i64>, Vec<i64>, Vec<i64>)>>,
    /// Positions where ANY instance's Az/Bz/Cz didn't fit i64 (union across all instances).
    /// i64 vectors are zeroed at these positions; correction uses field values.
    large_positions: Vec<usize>,
    /// Public values used when computing cached_step_matvec, for validation in prove.
    /// Non-empty when the matvec cache is active; prove checks that step circuits produce the same values.
    cached_step_public_values: Vec<Vec<E::Scalar>>,
}

/// Holds the zero-knowledge proof produced by the NeutronNova folding scheme followed by NeutronNova SNARK
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct VegaMcZkSNARK<E: Engine> {
    /// Shared commitment stored once (same for all step instances and core).
    comm_W_shared: Option<Commitment<E>>,
    step_instances: Vec<SplitR1CSInstance<E>>,
    core_instance: SplitR1CSInstance<E>,
    eval_arg: <E::PCS as PCSEngineTrait<E>>::EvaluationArgument,
    U_verifier: SplitMultiRoundR1CSInstance<E>,
    nifs: NovaNIFS<E>,
    random_U: RelaxedR1CSInstance<E>,
    relaxed_snark: crate::spartan_relaxed::RelaxedR1CSSpartanProof<E>,
}

impl<E: Engine> VegaMcZkSNARK<E> {
    /// Encode this proof using Microsoft's canonical Vega-MC representation.
    pub fn encode_iroha_canonical(&self) -> Result<Vec<u8>, IrohaCanonicalCodecError> {
        bincode::DefaultOptions::new()
            .with_little_endian()
            .with_fixint_encoding()
            .serialize(self)
            .map_err(|_| IrohaCanonicalCodecError::Encode)
    }

    /// Decode exactly one canonical Microsoft Vega-MC proof.
    ///
    /// The explicit limit is enforced by bincode before any attacker-controlled
    /// sequence allocation. Trailing bytes are rejected. Callers that accept
    /// untrusted proofs must still perform their verifier-key-derived structural
    /// pre-scan before invoking this adapter.
    pub fn decode_iroha_canonical(
        bytes: &[u8],
        byte_limit: usize,
    ) -> Result<Self, IrohaCanonicalCodecError> {
        if bytes.len() > byte_limit {
            return Err(IrohaCanonicalCodecError::SizeLimit);
        }
        let byte_limit =
            u64::try_from(byte_limit).map_err(|_| IrohaCanonicalCodecError::SizeLimit)?;
        bincode::DefaultOptions::new()
            .with_little_endian()
            .with_fixint_encoding()
            .with_limit(byte_limit)
            .reject_trailing_bytes()
            .deserialize(bytes)
            .map_err(|_| IrohaCanonicalCodecError::Decode)
    }
}

impl<E: Engine> VegaMcZkSNARK<E>
where
    E::PCS: FoldingEngineTrait<E>,
{
    /// Sets up the NeutronNova SNARK for a batch of circuits of type `C1` and a single circuit of type `C2`
    ///
    /// # Parameters
    /// - `step_circuit`: The circuit to be folded in the batch
    /// - `core_circuit`: The core circuit that connects the batch together
    /// - `num_steps`: The number of step circuits in the batch (must be at least 2; padded to next power of two internally)
    pub fn setup<C1: VegaCircuit<E>, C2: VegaCircuit<E>>(
        step_circuit: &C1,
        core_circuit: &C2,
        num_steps: usize,
    ) -> Result<(VegaMcProverKey<E>, VegaMcVerifierKey<E>), VegaError> {
        // NeutronNova folds a batch of at least two step instances; a single
        // instance should use the single-circuit SNARK instead.
        if num_steps < 2 {
            return Err(VegaError::InvalidInputLength {
                reason: format!("num_steps must be at least 2, got {num_steps}"),
            });
        }

        let (_setup_span, setup_t) = start_span!("neutronnova_setup");

        let (_r1cs_span, r1cs_t) = start_span!("r1cs_shape_generation");
        debug!("Synthesizing step circuit");
        let mut S_step = ShapeCS::r1cs_shape(step_circuit)?;
        debug!("Finished synthesizing step circuit");

        debug!("Synthesizing core circuit");
        let mut S_core = ShapeCS::r1cs_shape(core_circuit)?;
        debug!("Finished synthesizing core circuit");

        SplitR1CSShape::equalize(&mut S_step, &mut S_core);

        info!(
            "Step circuit's witness sizes: shared = {}, precommitted = {}, rest = {}",
            S_step.num_shared, S_step.num_precommitted, S_step.num_rest
        );
        info!(
            "Core circuit's witness sizes: shared = {}, precommitted = {}, rest = {}",
            S_core.num_shared, S_core.num_precommitted, S_core.num_rest
        );
        info!(elapsed_ms = %r1cs_t.elapsed().as_millis(), "r1cs_shape_generation");

        let (_ck_span, ck_t) = start_span!("commitment_key_generation");
        let (ck, vk_ee) = SplitR1CSShape::commitment_key(&[&S_step, &S_core])?;
        E::PCS::precompute_ck(&ck);
        info!(elapsed_ms = %ck_t.elapsed().as_millis(), "commitment_key_generation");

        // Calculate num_rounds_b from num_steps by padding to next power of two
        let (_vc_span, vc_t) = start_span!("verifier_circuit_setup");
        let num_rounds_b = num_steps.next_power_of_two().log_2();

        let num_vars = S_step.num_shared + S_step.num_precommitted + S_step.num_rest;
        let num_rounds_x = usize::try_from(S_step.num_cons.ilog2()).unwrap();
        let num_rounds_y = usize::try_from(num_vars.ilog2()).unwrap() + 1;
        let vc = VegaMcVerifierCircuit::<E>::default(num_rounds_b, num_rounds_x, num_rounds_y, 32);
        let (vc_shape, vc_ck, vc_vk) =
            <ShapeCS<E> as MultiRoundVegaShape<E>>::multiround_r1cs_shape(&vc)?;
        let vc_shape_regular = vc_shape.to_regular_shape();
        info!(elapsed_ms = %vc_t.elapsed().as_millis(), "verifier_circuit_setup");
        // Eagerly init FixedBaseMul table before cloning so both pk/vk get it
        E::PCS::precompute_ck(&vc_ck);
        let vk: VegaMcVerifierKey<E> = VegaMcVerifierKey {
            ck: ck.clone(),
            S_step: S_step.clone(),
            S_core: S_core.clone(),
            vk_ee,
            vc_shape: vc_shape.clone(),
            vc_shape_regular: vc_shape_regular.clone(),
            vc_ck: vc_ck.clone(),
            vc_vk: vc_vk.clone(),
            num_steps,
            digest: OnceCell::new(),
        };

        let vk_digest = vk.digest()?;
        let pk = VegaMcProverKey {
            ck,
            S_step,
            S_core,
            vc_shape,
            vc_shape_regular,
            vc_ck,
            vk_digest,
        };

        // Eagerly precompute sparse matrix data for the step and core circuits
        pk.S_step.precompute();
        pk.S_core.precompute();
        vk.S_step.precompute();
        vk.S_core.precompute();
        info!(elapsed_ms = %setup_t.elapsed().as_millis(), "neutronnova_setup");
        Ok((pk, vk))
    }

    /// Prepares the pre-processed state for proving
    pub fn prep_prove<C1: VegaCircuit<E>, C2: VegaCircuit<E>>(
        pk: &VegaMcProverKey<E>,
        step_circuits: &[C1],
        core_circuit: &C2,
        is_small: bool, // do witness elements fit in machine words?
    ) -> Result<VegaMcPrepZkSNARK<E>, VegaError> {
        let (_prep_span, prep_t) = start_span!("neutronnova_prep_prove");

        // we synthesize shared witness for the first circuit; every other circuit including the core circuit shares this witness
        let (_shared_span, shared_t) = start_span!("generate_shared_witness");
        let mut ps =
            SatisfyingAssignment::shared_witness(&pk.S_step, &pk.ck, &step_circuits[0], is_small)?;
        info!(elapsed_ms = %shared_t.elapsed().as_millis(), "generate_shared_witness");

        let (_precommit_span, precommit_t) = start_span!(
            "generate_precommitted_witnesses",
            circuits = step_circuits.len() + 1
        );
        let ps_step = (0..step_circuits.len())
            .into_par_iter()
            .map(|i| {
                // copy ps to avoid mutating the original shared witness
                let mut ps_i = ps.clone();
                SatisfyingAssignment::precommitted_witness(
                    &mut ps_i,
                    &pk.S_step,
                    &pk.ck,
                    &step_circuits[i],
                    is_small,
                )?;
                Ok(ps_i)
            })
            .collect::<Result<Vec<_>, _>>()?;

        // we don't need to make a copy of ps for the core circuit, as it will be used only once
        SatisfyingAssignment::precommitted_witness(
            &mut ps,
            &pk.S_core,
            &pk.ck,
            core_circuit,
            is_small,
        )?;
        info!(elapsed_ms = %precommit_t.elapsed().as_millis(), circuits = step_circuits.len() + 1, "generate_precommitted_witnesses");

        // Precompute full matrix-vector products for step circuits (deterministic).
        // Only valid when step circuits have no rest variables and no challenges,
        // meaning z = [shared_W, precommitted_W, 0..., 1, public_values] is fully known during prep.
        let can_cache_matvec = pk.S_step.num_challenges == 0 && pk.S_step.num_rest_unpadded == 0;

        let (cached_step_matvec, cached_step_i64, large_positions, cached_step_public_values) =
            if can_cache_matvec {
                // Collect public values for each step circuit so we can validate in prove
                let step_public_values: Vec<Vec<E::Scalar>> = step_circuits
                    .iter()
                    .map(|c| {
                        c.public_values().map_err(|e| VegaError::SynthesisError {
                            reason: format!("Circuit does not provide public IO: {e}"),
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                let matvec: Vec<_> = (0..ps_step.len())
                    .into_par_iter()
                    .map(|i| {
                        let ps_i = &ps_step[i];
                        let public_values = &step_public_values[i];
                        let mut z = Vec::with_capacity(ps_i.W.len() + 1 + public_values.len());
                        z.extend_from_slice(&ps_i.W);
                        z.push(E::Scalar::ONE);
                        z.extend_from_slice(public_values);
                        pk.S_step.multiply_vec(&z)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                // Convert Az/Bz to i64 for small-value NIFS round 0 optimization.
                let mut all_i64 = Vec::with_capacity(matvec.len());
                let mut large_pos_set = std::collections::BTreeSet::new();
                for (az, bz, cz) in &matvec {
                    let (az_i64, az_large) = to_small_vec_or_zero(az);
                    let (bz_i64, bz_large) = to_small_vec_or_zero(bz);
                    let (cz_i64, cz_large) = to_small_vec_or_zero(cz);
                    for pos in az_large {
                        large_pos_set.insert(pos);
                    }
                    for pos in bz_large {
                        large_pos_set.insert(pos);
                    }
                    for pos in cz_large {
                        large_pos_set.insert(pos);
                    }
                    all_i64.push((az_i64, bz_i64, cz_i64));
                }
                let lp: Vec<usize> = large_pos_set.into_iter().collect();
                info!(
                    n_large = lp.len(),
                    total = matvec[0].0.len(),
                    "i64_conversion_stats"
                );

                // Zero out i64 values at ALL large_positions in ALL instances.
                if !lp.is_empty() {
                    for (az_i64, bz_i64, cz_i64) in &mut all_i64 {
                        for &pos in &lp {
                            az_i64[pos] = 0;
                            bz_i64[pos] = 0;
                            cz_i64[pos] = 0;
                        }
                    }
                }
                (Some(matvec), Some(all_i64), lp, step_public_values)
            } else {
                info!(
                    "Step circuit has rest_unpadded={} challenges={}, skipping matvec/i64 caching",
                    pk.S_step.num_rest_unpadded, pk.S_step.num_challenges
                );
                (None, None, Vec::new(), Vec::new())
            };

        info!(elapsed_ms = %prep_t.elapsed().as_millis(), "neutronnova_prep_prove");
        Ok(VegaMcPrepZkSNARK {
            ps_step,
            ps_core: ps,
            cached_step_matvec,
            cached_step_i64,
            large_positions,
            cached_step_public_values,
        })
    }

    /// Prove the folding of a batch of R1CS instances and a core circuit that connects them together.
    /// Takes ownership of `prep_snark` to avoid cloning large witness vectors (~66MB).
    /// Returns the proof and the (consumed) prep state, which can be passed to prove again
    /// after re-running prep_prove or simply re-rerandomized.
    pub fn prove<C1: VegaCircuit<E>, C2: VegaCircuit<E>>(
        pk: &VegaMcProverKey<E>,
        step_circuits: &[C1],
        core_circuit: &C2,
        mut prep_snark: VegaMcPrepZkSNARK<E>,
        is_small: bool, // do witness elements fit in machine words?
    ) -> Result<(Self, VegaMcPrepZkSNARK<E>), VegaError> {
        let (_prove_span, prove_t) = start_span!("neutronnova_prove");

        // rerandomize prep state in-place (we own it, no clone needed)
        let (_rerandomize_span, rerandomize_t) = start_span!("rerandomize_prep_state");
        prep_snark
            .ps_core
            .rerandomize_in_place(&pk.ck, &pk.S_core)?;
        let comm_W_shared = prep_snark.ps_core.comm_W_shared.clone();
        let r_W_shared = prep_snark.ps_core.r_W_shared.clone();
        prep_snark.ps_step.par_iter_mut().try_for_each(|ps_i| {
            ps_i.rerandomize_with_shared_in_place(&pk.ck, &pk.S_step, &comm_W_shared, &r_W_shared)
        })?;
        info!(elapsed_ms = %rerandomize_t.elapsed().as_millis(), "rerandomize_prep_state");

        // The prepared step state corresponds one-to-one with the step circuits.
        if prep_snark.ps_step.len() != step_circuits.len() {
            return Err(VegaError::InvalidInputLength {
                reason: format!(
                    "prove received {} step circuits but prep state holds {} step instances",
                    step_circuits.len(),
                    prep_snark.ps_step.len()
                ),
            });
        }

        // Validate that cached matvec matches current step circuit public values.
        // The cache computed in prep_prove includes public_values in the z vector;
        // if the circuits changed, the cache is stale and would produce incorrect proofs.
        if !prep_snark.cached_step_public_values.is_empty() {
            if prep_snark.cached_step_public_values.len() != step_circuits.len() {
                return Err(VegaError::InternalError {
                    reason: format!(
                        "Cached matvec was computed for {} step circuits, but prove received {}",
                        prep_snark.cached_step_public_values.len(),
                        step_circuits.len()
                    ),
                });
            }
            for (i, circuit) in step_circuits.iter().enumerate() {
                let current_pv =
                    circuit
                        .public_values()
                        .map_err(|e| VegaError::SynthesisError {
                            reason: format!("Circuit does not provide public IO: {e}"),
                        })?;
                if prep_snark.cached_step_public_values[i] != current_pv {
                    return Err(VegaError::InternalError {
                        reason: format!(
                            "Step circuit {i} public values changed between prep_prove and prove"
                        ),
                    });
                }
            }
        }

        // Parallel generation of instances and witnesses
        let (_gen_span, gen_t) = start_span!(
            "generate_instances_witnesses",
            step_circuits = step_circuits.len()
        );
        let (res_steps, res_core) = rayon::join(
            || {
                prep_snark
                    .ps_step
                    .par_iter_mut()
                    .zip(step_circuits.par_iter().enumerate())
                    .map(|(pre_state, (i, circuit))| {
                        let mut transcript = E::TE::new(b"neutronnova_prove");
                        transcript.absorb(b"vk", &pk.vk_digest);
                        transcript.absorb(
                            b"num_circuits",
                            &E::Scalar::from(step_circuits.len() as u64),
                        );
                        transcript.absorb(b"circuit_index", &E::Scalar::from(i as u64));

                        let public_values =
                            circuit
                                .public_values()
                                .map_err(|e| VegaError::SynthesisError {
                                    reason: format!("Circuit does not provide public IO: {e}"),
                                })?;
                        transcript.absorb(b"public_values", &public_values.as_slice());

                        SatisfyingAssignment::r1cs_instance_and_witness(
                            pre_state,
                            &pk.S_step,
                            &pk.ck,
                            circuit,
                            is_small,
                            &mut transcript,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map(|pairs| {
                        let (instances, witnesses): (Vec<_>, Vec<_>) = pairs.into_iter().unzip();
                        (instances, witnesses)
                    })
            },
            || {
                let mut transcript = E::TE::new(b"neutronnova_prove");
                transcript.absorb(b"vk", &pk.vk_digest);
                let public_values_core =
                    core_circuit
                        .public_values()
                        .map_err(|e| VegaError::SynthesisError {
                            reason: format!("Core circuit does not provide public IO: {e}"),
                        })?;
                transcript.absorb(b"public_values", &public_values_core.as_slice());
                SatisfyingAssignment::r1cs_instance_and_witness(
                    &mut prep_snark.ps_core,
                    &pk.S_core,
                    &pk.ck,
                    core_circuit,
                    is_small,
                    &mut transcript,
                )
            },
        );

        let ((step_instances, step_witnesses), (core_instance, core_witness)) =
            (res_steps?, res_core?);
        info!(elapsed_ms = %gen_t.elapsed().as_millis(), step_circuits = step_circuits.len(), "generate_instances_witnesses");

        let (_reg_span, reg_t) = start_span!("convert_to_regular_instances");
        let step_instances_regular = step_instances
            .iter()
            .map(|u| u.to_regular_instance())
            .collect::<Result<Vec<_>, _>>()?;

        let core_instance_regular = core_instance.to_regular_instance()?;
        info!(elapsed_ms = %reg_t.elapsed().as_millis(), "convert_to_regular_instances");
        // We start a new transcript for the NeutronNova NIFS proof
        // All instances will be absorbed into the transcript
        let mut transcript = E::TE::new(b"neutronnova_prove");
        transcript.absorb(b"vk", &pk.vk_digest);

        // absorb the core instance; NIFS will absorb the step instances
        transcript.absorb(b"core_instance", &core_instance_regular);

        let n_padded = step_instances_regular.len().next_power_of_two();
        let num_vars = pk.S_step.num_shared + pk.S_step.num_precommitted + pk.S_step.num_rest;
        let num_rounds_b = n_padded.log_2();
        let num_rounds_x = pk.S_step.num_cons.log_2();
        let num_rounds_y = num_vars.log_2() + 1;

        let mut vc = VegaMcVerifierCircuit::<E>::default(
            num_rounds_b,
            num_rounds_x,
            num_rounds_y,
            pk.vc_shape.commitment_width,
        );
        let mut vc_state = SatisfyingAssignment::<E>::initialize_multiround_witness(&pk.vc_shape)?;

        // Perform ZK NIFS prove and collect outputs.
        // Clone caches (matvec/i64) before passing to NIFS, which consumes them.
        // This keeps `prep_snark.cached_step_matvec` / `cached_step_i64` populated
        // so that a subsequent `prove` call on the same prep state reuses them
        // (production reuse scenario). large_positions is also kept intact via `&`.
        let (_nifs_span, nifs_t) = start_span!("NIFS");
        // Parallel clone: each inner triple (Vec, Vec, Vec) of size num_cons is
        // large enough that serial clone becomes a bottleneck at many instances.
        let cached_matvec = prep_snark
            .cached_step_matvec
            .as_ref()
            .map(|v| v.par_iter().cloned().collect::<Vec<_>>());
        let cached_i64 = prep_snark
            .cached_step_i64
            .as_ref()
            .map(|v| v.par_iter().cloned().collect::<Vec<_>>());
        let (E_eq, Az_step, Bz_step, Cz_step, folded_W, folded_U) = VegaMcNIFS::<E>::prove(
            &pk.S_step,
            &pk.ck,
            step_instances_regular,
            step_witnesses,
            cached_matvec,
            cached_i64,
            &prep_snark.large_positions,
            &mut vc,
            &mut vc_state,
            &pk.vc_shape,
            &pk.vc_ck,
            &mut transcript,
        )?;
        info!(elapsed_ms = %nifs_t.elapsed().as_millis(), "NIFS");

        let (_tensor_span, tensor_t) = start_span!("compute_tensor_and_poly_tau");
        let (_ell, left, _right) = compute_tensor_decomp(pk.S_step.num_cons);
        let mut E1 = E_eq;
        let E2 = E1.split_off(left);

        let mut poly_tau_left = MultilinearPolynomial::new(E1);
        let poly_tau_right = MultilinearPolynomial::new(E2);

        info!(elapsed_ms = %tensor_t.elapsed().as_millis(), "compute_tensor_and_poly_tau");

        // outer sum-check preparation
        let (_mp_span, mp_t) = start_span!("prepare_multilinear_polys");
        let (mut poly_Az_step, mut poly_Bz_step, mut poly_Cz_step) = (
            MultilinearPolynomial::new(Az_step),
            MultilinearPolynomial::new(Bz_step),
            MultilinearPolynomial::new(Cz_step),
        );

        let (mut poly_Az_core, mut poly_Bz_core, mut poly_Cz_core) = {
            let (_core_span, core_t) = start_span!("compute_core_polys");
            let z = [
                core_witness.W.clone(),
                vec![E::Scalar::ONE],
                core_instance.public_values.clone(),
                core_instance.challenges.clone(),
            ]
            .concat();

            let (Az, Bz, Cz) = pk.S_core.multiply_vec(&z)?;
            info!(elapsed_ms = %core_t.elapsed().as_millis(), "compute_core_polys");
            (
                MultilinearPolynomial::new(Az),
                MultilinearPolynomial::new(Bz),
                MultilinearPolynomial::new(Cz),
            )
        };

        info!(elapsed_ms = %mp_t.elapsed().as_millis(), "prepare_multilinear_polys");
        let outer_start_index = num_rounds_b + 1;
        // outer sum-check (batched)
        let (_sc_span, sc_t) = start_span!("outer_sumcheck_batched");
        let r_x = SumcheckProof::<E>::prove_cubic_with_additive_term_batched_zk(
            num_rounds_x,
            &mut poly_tau_left,
            &poly_tau_right,
            &mut poly_Az_step,
            &mut poly_Az_core,
            &mut poly_Bz_step,
            &mut poly_Bz_core,
            &mut poly_Cz_step,
            &mut poly_Cz_core,
            &mut vc,
            &mut vc_state,
            &pk.vc_shape,
            &pk.vc_ck,
            &mut transcript,
            outer_start_index,
        )?;
        info!(elapsed_ms = %sc_t.elapsed().as_millis(), "outer_sumcheck_batched");
        vc.claim_Az_step = poly_Az_step[0];
        vc.claim_Bz_step = poly_Bz_step[0];
        vc.claim_Cz_step = poly_Cz_step[0];
        vc.claim_Az_core = poly_Az_core[0];
        vc.claim_Bz_core = poly_Bz_core[0];
        vc.claim_Cz_core = poly_Cz_core[0];
        vc.tau_at_rx = poly_tau_left[0];

        let chals = SatisfyingAssignment::<E>::process_round(
            &mut vc_state,
            &pk.vc_shape,
            &pk.vc_ck,
            &vc,
            outer_start_index + num_rounds_x,
            &mut transcript,
        )?;
        let r = chals[0];

        // inner sum-check preparation
        let claim_inner_joint_step =
            vc.claim_Az_step + r * vc.claim_Bz_step + r * r * vc.claim_Cz_step;
        let claim_inner_joint_core =
            vc.claim_Az_core + r * vc.claim_Bz_core + r * r * vc.claim_Cz_core;

        let (_eval_rx_span, eval_rx_t) = start_span!("compute_eval_rx");
        let evals_rx = EqPolynomial::evals_from_points(&r_x);
        info!(elapsed_ms = %eval_rx_t.elapsed().as_millis(), "compute_eval_rx");

        let (_sparse_span, sparse_t) = start_span!("compute_eval_table_sparse");
        let (poly_ABC_step, step_lo_eff, step_hi_eff) =
            pk.S_step.bind_and_prepare_poly_ABC_full(&evals_rx, &r);
        let (poly_ABC_core, core_lo_eff, core_hi_eff) =
            pk.S_core.bind_and_prepare_poly_ABC_full(&evals_rx, &r);
        info!(elapsed_ms = %sparse_t.elapsed().as_millis(), "compute_eval_table_sparse");
        // inner sum-check
        let (_sc2_span, sc2_t) = start_span!("inner_sumcheck_batched");

        debug!("Proving inner sum-check with {} rounds", num_rounds_y);
        debug!(
            "Inner sum-check sizes - poly_ABC_step: {}, poly_ABC_core: {}",
            poly_ABC_step.len(),
            poly_ABC_core.len()
        );

        // Build z vectors for the folded and core instances.
        // Non-zero prefix = w_len + 1 + x_len (witness + u + public inputs).
        let (z_folded_vec, z_folded_lo, z_folded_hi) = {
            let mut v = vec![E::Scalar::ZERO; num_vars * 2];
            let w_len = folded_W.W.len();
            v[..w_len].copy_from_slice(&folded_W.W);
            v[w_len] = E::Scalar::ONE;
            let x_len = folded_U.X.len();
            v[w_len + 1..w_len + 1 + x_len].copy_from_slice(&folded_U.X);
            let last_nz = w_len + 1 + x_len;
            (v, last_nz.min(num_vars), last_nz.saturating_sub(num_vars))
        };
        let (z_core_vec, z_core_lo, z_core_hi) = {
            let mut v = vec![E::Scalar::ZERO; num_vars * 2];
            let w_len = core_witness.W.len();
            v[..w_len].copy_from_slice(&core_witness.W);
            v[w_len] = E::Scalar::ONE;
            let x_len = core_instance_regular.X.len();
            v[w_len + 1..w_len + 1 + x_len].copy_from_slice(&core_instance_regular.X);
            let last_nz = w_len + 1 + x_len;
            (v, last_nz.min(num_vars), last_nz.saturating_sub(num_vars))
        };

        // Use actual X length for hi_eff (num_public in SplitR1CSShape may not include shared inputs)
        let step_hi_eff = step_hi_eff.max(z_folded_hi);
        let core_hi_eff = core_hi_eff.max(z_core_hi);

        let (r_y, evals) = SumcheckProof::<E>::prove_quad_batched_zk(
            &[claim_inner_joint_step, claim_inner_joint_core],
            num_rounds_y,
            &mut MultilinearPolynomial::new_with_halves(poly_ABC_step, step_lo_eff, step_hi_eff),
            &mut MultilinearPolynomial::new_with_halves(poly_ABC_core, core_lo_eff, core_hi_eff),
            &mut MultilinearPolynomial::new_with_halves(z_folded_vec, z_folded_lo, z_folded_hi),
            &mut MultilinearPolynomial::new_with_halves(z_core_vec, z_core_lo, z_core_hi),
            &mut vc,
            &mut vc_state,
            &pk.vc_shape,
            &pk.vc_ck,
            &mut transcript,
            outer_start_index + num_rounds_x + 1,
        )?;
        info!(elapsed_ms = %sc2_t.elapsed().as_millis(), "inner_sumcheck_batched");

        let eval_Z_step = evals[2];
        let eval_Z_core = evals[3];

        let eval_X_step = {
            let X = vec![E::Scalar::ONE]
                .into_iter()
                .chain(folded_U.X.iter().cloned())
                .collect::<Vec<E::Scalar>>();
            let num_vars_log2 = usize::try_from(num_vars.ilog2()).unwrap();
            SparsePolynomial::new(num_vars_log2, X).evaluate(&r_y[1..])
        };
        let eval_X_core = {
            let X = vec![E::Scalar::ONE]
                .into_iter()
                .chain(core_instance_regular.X.iter().cloned())
                .collect::<Vec<E::Scalar>>();
            let num_vars_log2 = usize::try_from(num_vars.ilog2()).unwrap();
            SparsePolynomial::new(num_vars_log2, X).evaluate(&r_y[1..])
        };
        let inv: Option<E::Scalar> = (E::Scalar::ONE - r_y[0]).invert().into();
        let one_minus_ry0_inv = inv.ok_or(VegaError::DivisionByZero)?;
        let eval_W_step = (eval_Z_step - r_y[0] * eval_X_step) * one_minus_ry0_inv;
        let eval_W_core = (eval_Z_core - r_y[0] * eval_X_core) * one_minus_ry0_inv;

        vc.eval_W_step = eval_W_step;
        vc.eval_W_core = eval_W_core;
        vc.eval_X_step = eval_X_step;
        vc.eval_X_core = eval_X_core;

        // Inner final equality round
        let _ = SatisfyingAssignment::<E>::process_round(
            &mut vc_state,
            &pk.vc_shape,
            &pk.vc_ck,
            &vc,
            outer_start_index + num_rounds_x + 1 + num_rounds_y,
            &mut transcript,
        )?;

        // Commit eval_W_step
        let eval_w_step_commit_round = outer_start_index + num_rounds_x + 1 + num_rounds_y + 1;
        let _ = SatisfyingAssignment::<E>::process_round(
            &mut vc_state,
            &pk.vc_shape,
            &pk.vc_ck,
            &vc,
            eval_w_step_commit_round,
            &mut transcript,
        )?;

        // Commit eval_W_core
        let _ = SatisfyingAssignment::<E>::process_round(
            &mut vc_state,
            &pk.vc_shape,
            &pk.vc_ck,
            &vc,
            eval_w_step_commit_round + 1,
            &mut transcript,
        )?;

        let (U_verifier, W_verifier) =
            SatisfyingAssignment::<E>::finalize_multiround_witness(&mut vc_state, &pk.vc_shape)?;

        let U_verifier_regular = U_verifier.to_regular_instance()?;

        // Sample fresh random instance/witness for ZK (must be done per-prove to preserve zero-knowledge).
        let (random_U, random_W) = pk
            .vc_shape_regular
            .sample_random_instance_witness(&pk.vc_ck)?;
        let (nifs, folded_W_verifier, folded_u, folded_X) = NovaNIFS::<E>::prove(
            &pk.vc_ck,
            &pk.vc_shape_regular,
            &random_U,
            &random_W,
            &U_verifier_regular,
            &W_verifier,
            &mut transcript,
        )?;

        // Prove satisfiability of the folded VC instance via relaxed R1CS Spartan
        let relaxed_snark = crate::spartan_relaxed::RelaxedR1CSSpartanProof::prove(
            &pk.vc_shape_regular,
            &pk.vc_ck,
            &folded_u,
            &folded_X,
            &folded_W_verifier,
            &mut transcript,
        )?;
        // access two claimed commitments to evaluations of W_step and W_core
        let comm_eval_W_step = U_verifier.comm_w_per_round[eval_w_step_commit_round].clone();
        let blind_eval_W_step = vc_state.r_w_per_round[eval_w_step_commit_round].clone();

        let comm_eval_W_core = U_verifier.comm_w_per_round[eval_w_step_commit_round + 1].clone();
        let blind_eval_W_core = vc_state.r_w_per_round[eval_w_step_commit_round + 1].clone();

        // the commitments are already absorbed in the transcript, so we simply squeeze the challenge
        let c_eval = transcript.squeeze(b"c_eval")?;

        // fold evaluation claims into one
        let (_fold_eval_span, fold_eval_t) = start_span!("fold_evaluation_claims");
        let comm = <E::PCS as FoldingEngineTrait<E>>::fold_commitments(
            &[folded_U.comm_W, core_instance_regular.comm_W],
            &[E::Scalar::ONE, c_eval],
        )?;
        let blind = <E::PCS as FoldingEngineTrait<E>>::fold_blinds(
            &[folded_W.r_W.clone(), core_witness.r_W.clone()],
            &[E::Scalar::ONE, c_eval],
        )?;
        let W = folded_W
            .W
            .par_iter()
            .zip(core_witness.W.par_iter())
            .map(|(w1, w2)| *w1 + c_eval * *w2)
            .collect::<Vec<_>>();
        let comm_eval = <E::PCS as FoldingEngineTrait<E>>::fold_commitments(
            &[comm_eval_W_step, comm_eval_W_core],
            &[E::Scalar::ONE, c_eval],
        )?;
        let blind_eval = <E::PCS as FoldingEngineTrait<E>>::fold_blinds(
            &[blind_eval_W_step, blind_eval_W_core],
            &[E::Scalar::ONE, c_eval],
        )?;
        info!(elapsed_ms = %fold_eval_t.elapsed().as_millis(), "fold_evaluation_claims");

        let (_pcs_span, pcs_t) = start_span!("pcs_prove");
        let eval_arg = E::PCS::prove(
            &pk.ck,
            &pk.vc_ck,
            &mut transcript,
            &comm,
            &W,
            &blind,
            &r_y[1..],
            &comm_eval,
            &blind_eval,
        )?;
        info!(elapsed_ms = %pcs_t.elapsed().as_millis(), "pcs_prove");

        // Extract shared commitment (same for all step instances and core) and strip from instances
        let comm_W_shared = step_instances.first().and_then(|u| u.comm_W_shared.clone());
        let step_instances = step_instances
            .into_iter()
            .map(|mut u| {
                u.comm_W_shared = None;
                u
            })
            .collect::<Vec<_>>();
        let mut core_instance = core_instance;
        core_instance.comm_W_shared = None;

        let result = Self {
            comm_W_shared,
            step_instances,
            core_instance,
            eval_arg,
            U_verifier,
            nifs,
            random_U,
            relaxed_snark,
        };

        info!(elapsed_ms = %prove_t.elapsed().as_millis(), "neutronnova_prove");
        Ok((result, prep_snark))
    }

    /// Verifies the VegaMcZkSNARK and returns the public IO from the instances
    pub fn verify(
        &self,
        vk: &VegaMcVerifierKey<E>,
        num_instances: usize,
    ) -> Result<(Vec<Vec<E::Scalar>>, Vec<E::Scalar>), VegaError> {
        let (_verify_span, _verify_t) = start_span!("neutronnova_verify");
        if num_instances == 0 || num_instances != self.step_instances.len() {
            return Err(VegaError::ProofVerifyError {
                reason: format!(
                    "Expected {} instances (non-zero), got {}",
                    num_instances,
                    self.step_instances.len()
                ),
            });
        }
        if num_instances != vk.num_steps {
            return Err(VegaError::ProofVerifyError {
                reason: format!(
                    "Verifier key is bound to {} step instances, got {}",
                    vk.num_steps, num_instances
                ),
            });
        }

        // Reconstruct step instances and core instance with the shared commitment
        let step_instances: Vec<SplitR1CSInstance<E>> = self
            .step_instances
            .iter()
            .map(|u| {
                let mut u = u.clone();
                u.comm_W_shared = self.comm_W_shared.clone();
                u
            })
            .collect();
        let mut core_instance = self.core_instance.clone();
        core_instance.comm_W_shared = self.comm_W_shared.clone();

        // validate the step instances
        let (_validate_span, validate_t) =
            start_span!("validate_instances", instances = step_instances.len());
        for (i, u) in step_instances.iter().enumerate() {
            let mut transcript = E::TE::new(b"neutronnova_prove");
            transcript.absorb(b"vk", &vk.digest()?);
            transcript.absorb(
                b"num_circuits",
                &E::Scalar::from(step_instances.len() as u64),
            );
            transcript.absorb(b"circuit_index", &E::Scalar::from(i as u64));
            // absorb the public IO into the transcript
            transcript.absorb(b"public_values", &u.public_values.as_slice());

            u.validate(&vk.S_step, &mut transcript)?;
        }

        // validate the core instance
        let mut transcript = E::TE::new(b"neutronnova_prove");
        transcript.absorb(b"vk", &vk.digest()?);
        // absorb the public IO into the transcript
        transcript.absorb(b"public_values", &core_instance.public_values.as_slice());

        core_instance.validate(&vk.S_core, &mut transcript)?;
        info!(elapsed_ms = %validate_t.elapsed().as_millis(), instances = step_instances.len(), "validate_instances");

        // shared commitment consistency was enforced at construction -- all step instances share comm_W_shared
        // also verify it matches the core instance
        for u in &step_instances {
            if u.comm_W_shared != core_instance.comm_W_shared {
                return Err(VegaError::ProofVerifyError {
                    reason: "All instances must have the same shared commitment".to_string(),
                });
            }
        }

        let (_convert_span, convert_t) = start_span!("convert_to_regular_verify");
        let mut step_instances_padded = step_instances.clone();
        if step_instances_padded.len() != step_instances_padded.len().next_power_of_two() {
            step_instances_padded.extend(std::iter::repeat_n(
                step_instances_padded[0].clone(),
                step_instances_padded.len().next_power_of_two() - step_instances_padded.len(),
            ));
        }
        let step_instances_regular = step_instances_padded
            .par_iter()
            .map(|u| u.to_regular_instance())
            .collect::<Result<Vec<_>, _>>()?;

        let core_instance_regular = core_instance.to_regular_instance()?;
        info!(elapsed_ms = %convert_t.elapsed().as_millis(), "convert_to_regular_verify");
        // We start a new transcript for the NeutronNova NIFS proof
        let mut transcript = E::TE::new(b"neutronnova_prove");

        // absorb the verifier key and instances
        transcript.absorb(b"vk", &vk.digest()?);
        transcript.absorb(b"core_instance", &core_instance_regular);
        for U in step_instances_regular.iter() {
            transcript.absorb(b"U", U);
        }
        transcript.absorb(b"T", &E::Scalar::ZERO); // we always have T=0 in NeutronNova

        // compute the number of rounds of NIFS, outer sum-check, and inner sum-check
        let num_rounds_b = step_instances_regular.len().log_2();
        let num_vars = vk.S_step.num_shared + vk.S_step.num_precommitted + vk.S_step.num_rest;
        let num_rounds_x = vk.S_step.num_cons.log_2();
        let num_rounds_y = num_vars.log_2() + 1;

        // we need num_rounds_b challenges for folding the step instances; we also need tau to compress multiple R1CS checks
        let tau = transcript.squeeze(b"tau")?;
        let rhos = (0..num_rounds_b)
            .map(|_| transcript.squeeze(b"rho"))
            .collect::<Result<Vec<_>, _>>()?;

        // validate the provided multi-round verifier instance and advance transcript
        self.U_verifier.validate(&vk.vc_shape, &mut transcript)?;

        let U_verifier_regular = self.U_verifier.to_regular_instance()?;

        // extract challenges and public IO from U_verifier's public IO
        let num_public_values = 6usize;
        let num_challenges = num_rounds_b + num_rounds_x + 1 + num_rounds_y;
        if U_verifier_regular.X.len() != num_challenges + num_public_values {
            return Err(VegaError::ProofVerifyError {
                reason: format!(
                    "Verifier instance has incorrect number of public IO: expected {}, got {}",
                    num_challenges + num_public_values,
                    U_verifier_regular.X.len()
                ),
            });
        }

        let challenges = &U_verifier_regular.X[0..num_challenges];
        let public_values = &U_verifier_regular.X[num_challenges..num_challenges + 6];

        let r_b = challenges[0..num_rounds_b].to_vec();
        let r_x = challenges[num_rounds_b..num_rounds_b + num_rounds_x].to_vec();
        let r = challenges[num_rounds_b + num_rounds_x]; // r for combining inner claims
        let r_y = challenges[num_rounds_b + num_rounds_x + 1..].to_vec();

        // fold_multiple and nifs.verify are independent: overlap them
        let (folded_U_result, folded_U_verifier_result) = rayon::join(
            || R1CSInstance::fold_multiple(&r_b, &step_instances_regular),
            || {
                self.nifs
                    .verify(&mut transcript, &self.random_U, &U_verifier_regular)
            },
        );
        let folded_U = folded_U_result?;
        let folded_U_verifier = folded_U_verifier_result?;
        self.relaxed_snark
            .verify(
                &vk.vc_shape_regular,
                &vk.vc_vk,
                &folded_U_verifier,
                &mut transcript,
            )
            .map_err(|e| VegaError::ProofVerifyError {
                reason: format!("Relaxed Spartan verify failed: {e}"),
            })?;
        let (_matrix_eval_span, matrix_eval_t) = start_span!("matrix_evaluations");
        let (eval_A_step, eval_B_step, eval_C_step, eval_A_core, eval_B_core, eval_C_core) = {
            let T_x = EqPolynomial::evals_from_points(&r_x);
            let T_y = EqPolynomial::evals_from_points(&r_y);
            let (eval_A_step, eval_B_step, eval_C_step) =
                vk.S_step.evaluate_with_tables_fast(&T_x, &T_y);
            let (eval_A_core, eval_B_core, eval_C_core) =
                vk.S_core.evaluate_with_tables_fast(&T_x, &T_y);

            (
                eval_A_step,
                eval_B_step,
                eval_C_step,
                eval_A_core,
                eval_B_core,
                eval_C_core,
            )
        };
        info!(elapsed_ms = %matrix_eval_t.elapsed().as_millis(), "matrix_evaluations");

        let eval_X_step = {
            let X = vec![E::Scalar::ONE]
                .into_iter()
                .chain(folded_U.X.iter().cloned())
                .collect::<Vec<E::Scalar>>();
            let num_vars_log2 = usize::try_from(num_vars.ilog2()).unwrap();
            SparsePolynomial::new(num_vars_log2, X).evaluate(&r_y[1..])
        };
        let eval_X_core = {
            let X = vec![E::Scalar::ONE]
                .into_iter()
                .chain(core_instance_regular.X.iter().cloned())
                .collect::<Vec<E::Scalar>>();
            let num_vars_log2 = usize::try_from(num_vars.ilog2()).unwrap();
            SparsePolynomial::new(num_vars_log2, X).evaluate(&r_y[1..])
        };

        // Compute quotient_* = (eval_A + r*eval_B + r^2*eval_C) for both branches
        let quotient_step = eval_A_step + r * eval_B_step + r * r * eval_C_step;
        let quotient_core = eval_A_core + r * eval_B_core + r * r * eval_C_core;
        let tau_at_rx = PowPolynomial::new(&tau, r_x.len()).evaluate(&r_x)?;
        let eq_rho_at_rb = EqPolynomial::new(r_b).evaluate(&rhos);

        if public_values[0] != tau_at_rx
            || public_values[1] != eval_X_step
            || public_values[2] != eval_X_core
            || public_values[3] != eq_rho_at_rb
            || public_values[4] != quotient_step
            || public_values[5] != quotient_core
        {
            return Err(VegaError::ProofVerifyError {
        reason:
          "Verifier instance public tau_at_rx/eval_X_step/eq_rho_at_rb/eval_X_core/quotients do not match recomputation"
            .to_string(),
      });
        }

        // verify PCS eval
        let c_eval = transcript.squeeze(b"c_eval")?;

        let eval_w_step_commit_round = num_rounds_b + 1 + num_rounds_x + 1 + num_rounds_y + 1;
        let comm_eval_W_step = self.U_verifier.comm_w_per_round[eval_w_step_commit_round].clone();
        let comm_eval_W_core =
            self.U_verifier.comm_w_per_round[eval_w_step_commit_round + 1].clone();

        let comm = <E::PCS as FoldingEngineTrait<E>>::fold_commitments(
            &[folded_U.comm_W, core_instance_regular.comm_W],
            &[E::Scalar::ONE, c_eval],
        )?;
        let comm_eval = <E::PCS as FoldingEngineTrait<E>>::fold_commitments(
            &[comm_eval_W_step, comm_eval_W_core],
            &[E::Scalar::ONE, c_eval],
        )?;

        let (_pcs_verify_span, pcs_verify_t) = start_span!("pcs_verify");
        E::PCS::verify(
            &vk.vk_ee,
            &vk.vc_ck,
            &mut transcript,
            &comm,
            &r_y[1..],
            &comm_eval,
            &self.eval_arg,
        )?;
        info!(elapsed_ms = %pcs_verify_t.elapsed().as_millis(), "pcs_verify");

        info!(elapsed_ms = %_verify_t.elapsed().as_millis(), "neutronnova_verify");

        let public_values_step = step_instances
            .iter()
            .take(num_instances)
            .map(|u| u.public_values.clone())
            .collect::<Vec<Vec<_>>>();

        let public_values_core = core_instance.public_values.clone();

        // return a vector of public values
        Ok((public_values_step, public_values_core))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::T256HyraxEngine;
    use bellpepper::gadgets::{
        boolean::{AllocatedBit, Boolean},
        num::AllocatedNum,
        sha256::sha256,
    };
    use bellpepper_core::{ConstraintSystem, SynthesisError};
    use core::marker::PhantomData;
    use ff::PrimeField;

    #[derive(Clone, Debug)]
    struct Sha256Circuit<E: Engine> {
        preimage: Vec<u8>,
        _p: PhantomData<E>,
    }

    impl<E: Engine> VegaCircuit<E> for Sha256Circuit<E> {
        fn public_values(&self) -> Result<Vec<E::Scalar>, SynthesisError> {
            Ok(vec![E::Scalar::ZERO]) // Placeholder, we don't use public values in this example
        }

        fn shared<CS: ConstraintSystem<E::Scalar>>(
            &self,
            _: &mut CS,
        ) -> Result<Vec<AllocatedNum<E::Scalar>>, SynthesisError> {
            Ok(vec![]) // Placeholder, we don't use shared variables in this example
        }

        fn precommitted<CS: ConstraintSystem<E::Scalar>>(
            &self,
            _: &mut CS,
            _: &[AllocatedNum<E::Scalar>],
        ) -> Result<Vec<AllocatedNum<E::Scalar>>, SynthesisError> {
            Ok(vec![]) // Placeholder, we don't use precommitted variables in this example
        }

        fn num_challenges(&self) -> usize {
            0 // Placeholder, we don't use challenges in this example
        }

        fn synthesize<CS: ConstraintSystem<E::Scalar>>(
            &self,
            cs: &mut CS,
            _shared: &[AllocatedNum<E::Scalar>],
            _precommitted: &[AllocatedNum<E::Scalar>],
            _challenges: Option<&[E::Scalar]>, // challenges from the verifier
        ) -> Result<(), SynthesisError> {
            // we write a circuit that checks if the input is a SHA256 preimage
            let bit_values: Vec<_> = self
                .preimage
                .clone()
                .into_iter()
                .flat_map(|byte| (0..8).map(move |i| (byte >> i) & 1u8 == 1u8))
                .map(Some)
                .collect();
            assert_eq!(bit_values.len(), self.preimage.len() * 8);

            let preimage_bits = bit_values
                .into_iter()
                .enumerate()
                .map(|(i, b)| AllocatedBit::alloc(cs.namespace(|| format!("preimage bit {i}")), b))
                .map(|b| b.map(Boolean::from))
                .collect::<Result<Vec<_>, _>>()?;

            let _ = sha256(cs.namespace(|| "sha256"), &preimage_bits)?;

            let x = AllocatedNum::alloc(cs.namespace(|| "x"), || Ok(E::Scalar::ZERO))?;
            x.inputize(cs.namespace(|| "inputize x"))?;

            Ok(())
        }
    }

    fn generate_sha_r1cs<E: Engine>(
        num_circuits: usize,
        len: usize,
    ) -> (
        VegaMcProverKey<E>,
        VegaMcVerifierKey<E>,
        Vec<Sha256Circuit<E>>,
    )
    where
        E::PCS: FoldingEngineTrait<E>, // Ensure that the PCS supports folding
    {
        let circuit = Sha256Circuit::<E> {
            preimage: vec![0u8; len],
            _p: Default::default(),
        };

        let (pk, vk) = VegaMcZkSNARK::<E>::setup(&circuit, &circuit, num_circuits).unwrap();

        let circuits = (0..num_circuits)
            .map(|i| Sha256Circuit::<E> {
                preimage: vec![i as u8; len],
                _p: Default::default(),
            })
            .collect::<Vec<_>>();

        (pk, vk, circuits)
    }

    // A tiny example circuit `x^3 + x + 5 = y` (x = 2, y = 15), used as a small
    // oracle for the stand-alone reference implementation. Same public/shared/
    // precommitted/challenge structure as the SHA circuit but only four
    // constraints (two multiplications, the cubic relation, and the output
    // inputization).
    #[derive(Clone, Debug, Default)]
    struct CubicCircuit {}

    impl<E: Engine> VegaCircuit<E> for CubicCircuit {
        fn public_values(&self) -> Result<Vec<E::Scalar>, SynthesisError> {
            Ok(vec![E::Scalar::from(15u64)])
        }

        fn shared<CS: ConstraintSystem<E::Scalar>>(
            &self,
            _: &mut CS,
        ) -> Result<Vec<AllocatedNum<E::Scalar>>, SynthesisError> {
            Ok(vec![])
        }

        fn precommitted<CS: ConstraintSystem<E::Scalar>>(
            &self,
            _: &mut CS,
            _: &[AllocatedNum<E::Scalar>],
        ) -> Result<Vec<AllocatedNum<E::Scalar>>, SynthesisError> {
            Ok(vec![])
        }

        fn num_challenges(&self) -> usize {
            0
        }

        fn synthesize<CS: ConstraintSystem<E::Scalar>>(
            &self,
            cs: &mut CS,
            _shared: &[AllocatedNum<E::Scalar>],
            _precommitted: &[AllocatedNum<E::Scalar>],
            _challenges: Option<&[E::Scalar]>,
        ) -> Result<(), SynthesisError> {
            let x =
                AllocatedNum::alloc(cs.namespace(|| "x"), || Ok(E::Scalar::ONE + E::Scalar::ONE))?;
            let x_sq = x.square(cs.namespace(|| "x_sq"))?;
            let x_cu = x_sq.mul(cs.namespace(|| "x_cu"), &x)?;
            let y = AllocatedNum::alloc(cs.namespace(|| "y"), || {
                Ok(x_cu.get_value().unwrap() + x.get_value().unwrap() + E::Scalar::from(5u64))
            })?;

            cs.enforce(
                || "y = x^3 + x + 5",
                |lc| {
                    lc + x_cu.get_variable()
                        + x.get_variable()
                        + CS::one()
                        + CS::one()
                        + CS::one()
                        + CS::one()
                        + CS::one()
                },
                |lc| lc + CS::one(),
                |lc| lc + y.get_variable(),
            );

            y.inputize(cs.namespace(|| "output"))?;

            Ok(())
        }
    }

    fn generate_cubic_r1cs<E: Engine>(
        num_circuits: usize,
    ) -> (VegaMcProverKey<E>, VegaMcVerifierKey<E>, Vec<CubicCircuit>)
    where
        E::PCS: FoldingEngineTrait<E>,
    {
        let circuit = CubicCircuit::default();
        let (pk, vk) = VegaMcZkSNARK::<E>::setup(&circuit, &circuit, num_circuits).unwrap();
        let circuits = (0..num_circuits)
            .map(|_| CubicCircuit::default())
            .collect::<Vec<_>>();
        (pk, vk, circuits)
    }

    fn test_neutron_inner<E: Engine, C1: VegaCircuit<E>, C2: VegaCircuit<E>>(
        name: &str,
        pk: &VegaMcProverKey<E>,
        vk: &VegaMcVerifierKey<E>,
        step_circuits: &[C1],
        core_circuit: &C2,
    ) where
        E::PCS: FoldingEngineTrait<E>,
    {
        println!(
            "[bench_neutron_inner] name: {name}, num_circuits: {}",
            step_circuits.len()
        );

        let ps = VegaMcZkSNARK::<E>::prep_prove(pk, step_circuits, core_circuit, true).unwrap();
        let res = VegaMcZkSNARK::prove(pk, step_circuits, core_circuit, ps, true);
        assert!(res.is_ok());

        let (snark, _ps) = res.unwrap();
        let res = snark.verify(vk, step_circuits.len());
        println!(
            "[bench_neutron_inner] name: {name}, num_circuits: {}, verify res: {:?}",
            step_circuits.len(),
            res
        );
        assert!(res.is_ok());

        let (public_values_step, _public_values_core) = res.unwrap();
        assert_eq!(public_values_step.len(), step_circuits.len());
    }

    #[test]
    fn test_neutron_sha256() {
        crate::iroha_rng::with_deterministic_test_seed(0x70, || {
            let _ = tracing_subscriber::fmt()
                .with_target(false)
                .with_ansi(true)
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .try_init();

            type E = T256HyraxEngine;

            for num_circuits in [2, 7, 32, 64] {
                for len in [32, 64].iter() {
                    let (pk, vk, circuits) = generate_sha_r1cs::<E>(num_circuits, *len);
                    test_neutron_inner(
                        &format!("sha256_num_circuits={num_circuits}_len={len}"),
                        &pk,
                        &vk,
                        &circuits,
                        &circuits[0], // core circuit is the first one, for test purposes
                    );
                }
            }
        });
    }

    #[test]
    fn test_mc_zk_prove_rejects_prep_state_with_fewer_steps() {
        type E = T256HyraxEngine;
        let (pk, _vk, circuits) = generate_sha_r1cs::<E>(7, 32);
        // Prepare with only 5 of the 7 step circuits, then prove with all 7.
        let ps = VegaMcZkSNARK::<E>::prep_prove(&pk, &circuits[..5], &circuits[0], true).unwrap();
        let res = VegaMcZkSNARK::prove(&pk, &circuits, &circuits[0], ps, true);
        assert!(res.is_err());
    }

    #[test]
    fn test_mc_zk_rejects_step_count_different_from_setup_count() {
        crate::iroha_rng::with_deterministic_test_seed(0x71, || {
            type E = T256HyraxEngine;
            // A valid 5-step proof under a key set up for exactly 5 steps.
            let (pk5, vk5, circuits5) = generate_sha_r1cs::<E>(5, 32);
            let ps = VegaMcZkSNARK::<E>::prep_prove(&pk5, &circuits5, &circuits5[0], true).unwrap();
            let (snark, _) =
                VegaMcZkSNARK::prove(&pk5, &circuits5, &circuits5[0], ps, true).unwrap();

            // It verifies under its own 5-step key.
            assert!(snark.verify(&vk5, 5).is_ok());

            // A key set up for a different number of steps must reject this proof.
            let (_pk7, vk7, _c7) = generate_sha_r1cs::<E>(7, 32);
            assert!(snark.verify(&vk7, 5).is_err());
        });
    }

    #[test]
    fn test_mc_setup_rejects_fewer_than_two_steps() {
        type E = T256HyraxEngine;
        let circuit = Sha256Circuit::<E> {
            preimage: vec![0u8; 32],
            _p: Default::default(),
        };
        // NeutronNova folding requires at least two step instances.
        for num_steps in [0, 1] {
            assert!(VegaMcZkSNARK::<E>::setup(&circuit, &circuit, num_steps).is_err());
        }
    }

    // Emits a deterministic Keccak-transcript conformance vector for the external
    // reference implementation. It exercises new/absorb(scalar,scalars,point)/squeeze
    // exactly as the protocol does, then writes the squeezed challenges to
    // reference/fixtures/cubic/transcript_vector.json. Run explicitly with:
    //   cargo test --lib export_transcript_vector -- --ignored --nocapture
    #[test]
    #[ignore]
    fn export_transcript_vector() {
        use crate::provider::traits::DlogGroup;
        use crate::traits::transcript::TranscriptEngineTrait;
        use std::fs;
        type E = T256HyraxEngine;

        let mut t = <E as Engine>::TE::new(b"refimpl_vector");

        // Absorb a single scalar.
        let s1 = <E as Engine>::Scalar::from(2u64);
        t.absorb(b"s1", &s1);
        let c1: <E as Engine>::Scalar = t.squeeze(b"c1").unwrap();

        // Absorb a group element (the generator) and a vector of scalars.
        let g = <<E as Engine>::GE as DlogGroup>::generator();
        t.absorb(b"g", &g);
        let vs: Vec<<E as Engine>::Scalar> = vec![
            <E as Engine>::Scalar::from(7u64),
            <E as Engine>::Scalar::from(9u64),
        ];
        t.absorb(b"vs", &vs.as_slice());
        let c2: <E as Engine>::Scalar = t.squeeze(b"c2").unwrap();

        // Absorb the two previous challenges, then squeeze twice without absorbing.
        t.absorb(b"c", &c1);
        t.absorb(b"c", &c2);
        let c3: <E as Engine>::Scalar = t.squeeze(b"c3").unwrap();
        let c4: <E as Engine>::Scalar = t.squeeze(b"c4").unwrap();

        let hexs = |s: <E as Engine>::Scalar| hex::encode(s.to_repr().as_ref());
        let json = format!(
            "{{\n  \"label\": \"refimpl_vector\",\n  \"c1\": \"{}\",\n  \"c2\": \"{}\",\n  \"c3\": \"{}\",\n  \"c4\": \"{}\"\n}}\n",
            hexs(c1),
            hexs(c2),
            hexs(c3),
            hexs(c4),
        );
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("reference/fixtures/cubic");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("transcript_vector.json"), &json).unwrap();
        eprintln!(
            "transcript vector: c1={} c2={} c3={} c4={}",
            hexs(c1),
            hexs(c2),
            hexs(c3),
            hexs(c4)
        );
    }

    // Exports a tiny cubic (x^3+x+5=y) MC fixture used as a small oracle for the
    // stand-alone Python reference implementation. Run explicitly with:
    //   cargo test --lib export_cubic_fixtures -- --ignored --nocapture
    #[test]
    #[ignore]
    fn export_cubic_fixtures() {
        use std::fs;
        type E = T256HyraxEngine;

        let num_circuits = 2usize;
        let (pk, vk, circuits) = generate_cubic_r1cs::<E>(num_circuits);

        let ps = VegaMcZkSNARK::<E>::prep_prove(&pk, &circuits, &circuits[0], true).unwrap();
        let (snark, _ps) = VegaMcZkSNARK::prove(&pk, &circuits, &circuits[0], ps, true).unwrap();

        let (pv_step, pv_core) = snark.verify(&vk, num_circuits).unwrap();

        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("reference/fixtures/cubic");
        fs::create_dir_all(&dir).unwrap();

        let proof_bytes = bincode::serialize(&snark).unwrap();
        fs::write(dir.join("proof.bin"), &proof_bytes).unwrap();

        let vk_bytes = bincode::serialize(&vk).unwrap();
        fs::write(dir.join("vk.bin"), &vk_bytes).unwrap();

        let digest = vk.digest().unwrap();
        fs::write(dir.join("vk_digest.bin"), digest).unwrap();

        let pv_step_hex: Vec<Vec<String>> = pv_step
            .iter()
            .map(|row| {
                row.iter()
                    .map(|s| hex::encode(s.to_repr().as_ref()))
                    .collect()
            })
            .collect();
        let pv_core_hex: Vec<String> = pv_core
            .iter()
            .map(|s| hex::encode(s.to_repr().as_ref()))
            .collect();

        let meta = format!(
            "{{\n  \"engine\": \"T256HyraxEngine\",\n  \"circuit\": \"cubic x^3+x+5=y\",\n  \"num_steps\": {},\n  \"proof_len\": {},\n  \"vk_len\": {},\n  \"public_values_step\": {:?},\n  \"public_values_core\": {:?}\n}}\n",
            num_circuits,
            proof_bytes.len(),
            vk_bytes.len(),
            pv_step_hex,
            pv_core_hex,
        );
        fs::write(dir.join("meta.json"), meta).unwrap();

        eprintln!(
            "exported cubic fixtures to {}: proof={} B, vk={} B, digest=32 B",
            dir.display(),
            proof_bytes.len(),
            vk_bytes.len(),
        );
    }
}
