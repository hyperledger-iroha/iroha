// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! This module implements the Hyrax polynomial commitment scheme
use crate::{
    errors::VegaError,
    math::Math,
    polys::eq::EqPolynomial,
    provider::{
        pcs::ipa::{InnerProductArgumentLinear, InnerProductInstance, InnerProductWitness},
        traits::{DlogGroup, DlogGroupExt},
    },
    start_span,
    traits::{
        Engine,
        pcs::{CommitmentTrait, FoldingEngineTrait, PCSEngineTrait},
        transcript::{TranscriptEngineTrait, TranscriptReprTrait},
    },
};
use core::marker::PhantomData;
use ff::{Field, PrimeField};
use num_integer::div_ceil;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::big_num::delayed_reduction::DelayedReduction;
use crate::big_num::montgomery::MontgomeryLimbs;
use crate::provider::msm::{AffineGroupElement, FixedBaseMul, vartime_scalar_mul};

/// Bind polynomial top variables using delayed reduction for Montgomery multiply.
/// Avoids per-product REDC, reducing multiply cost by ~50%.
/// The accumulator array (r_len * 72B) must fit in L2 cache.
#[inline(never)]
fn bind_with_delayed<F: PrimeField + MontgomeryLimbs + Copy>(
    poly: &[F],
    l: &[F],
    r_len: usize,
) -> Vec<F> {
    assert_eq!(poly.len(), l.len() * r_len);
    type Acc<S> = <S as DelayedReduction<S>>::Accumulator;
    let mut acc = vec![Acc::<F>::default(); r_len];
    for j in 0..l.len() {
        let l_j = &l[j];
        let row = &poly[j * r_len..(j + 1) * r_len];
        for i in 0..r_len {
            F::unreduced_multiply_accumulate(&mut acc[i], l_j, &row[i]);
        }
    }
    acc.iter().map(|a| F::reduce(a)).collect()
}

/// A type that holds commitment generators for Hyrax commitments
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct HyraxCommitmentKey<E: Engine>
where
    E::GE: DlogGroup,
{
    num_cols: usize,
    ck: Vec<AffineGroupElement<E>>,
    h: E::GE,
    /// Precomputed fixed-base table for h (computed lazily, not serialized)
    #[serde(skip)]
    h_table: std::sync::OnceLock<FixedBaseMul<E>>,
    /// Precomputed fixed-base tables for ALL ck bases (for small ck widths).
    /// Enables fast per-row commits via table lookup instead of runtime MSM.
    #[serde(skip)]
    ck_tables: std::sync::OnceLock<Vec<FixedBaseMul<E>>>,
}

impl<E: Engine> HyraxCommitmentKey<E>
where
    E::GE: DlogGroupExt,
{
    /// Eagerly initialize the h_table for fixed-base scalar multiplication.
    /// Call before cloning to ensure both copies have the precomputed table.
    pub fn ensure_h_table(&self) {
        self.h_table
            .get_or_init(|| FixedBaseMul::precompute(&self.h, 8));
        // For small ck widths (e.g., VC with 32 bases), precompute tables for all bases.
        // This enables fast fixed-base MSM for commit (table lookup vs runtime MSM).
        if self.ck.len() <= 64 {
            self.ck_tables.get_or_init(|| {
                self.ck
                    .par_iter()
                    .map(|base| FixedBaseMul::precompute(&E::GE::group(base), 8))
                    .collect()
            });
        }
    }
}

/// A type that holds the verifier key for Hyrax commitments
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct HyraxVerifierKey<E: Engine>
where
    E::GE: DlogGroup,
{
    num_cols: usize,
    ck: Vec<AffineGroupElement<E>>,
    h: E::GE,
}

/// Structure that holds commitments
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct HyraxCommitment<E: Engine> {
    comm: Vec<E::GE>,
}

/// Structure that holds blinds
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct HyraxBlind<E: Engine> {
    blind: Vec<E::Scalar>,
}

/// Provides a commitment engine
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HyraxPCS<E: Engine> {
    _p: PhantomData<E>,
}

/// Provides an implementation of a polynomial evaluation argument
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct HyraxEvaluationArgument<E: Engine>
where
    E::GE: DlogGroupExt,
{
    ipa: InnerProductArgumentLinear<E>,
}

impl<E: Engine> PCSEngineTrait<E> for HyraxPCS<E>
where
    E::GE: DlogGroupExt,
{
    type CommitmentKey = HyraxCommitmentKey<E>;
    type VerifierKey = HyraxVerifierKey<E>;
    type Commitment = HyraxCommitment<E>;
    type Blind = HyraxBlind<E>;
    type EvaluationArgument = HyraxEvaluationArgument<E>;

    /// Derives generators for Hyrax PC, where n is the size of the vector to be committed to and width is the number of columns.
    fn setup(
        label: &'static [u8],
        _n: usize,
        width: usize,
    ) -> (Self::CommitmentKey, Self::VerifierKey) {
        let num_cols = width;
        let gens = E::GE::from_label(label, num_cols + 1);
        let ck = gens[..num_cols].to_vec();
        let h = <E::GE as DlogGroup>::group(&gens[num_cols]);

        let vk = Self::VerifierKey {
            num_cols,
            ck: ck.clone(),
            h,
        };

        let ck = Self::CommitmentKey {
            num_cols,
            ck,
            h,
            h_table: std::sync::OnceLock::new(),
            ck_tables: std::sync::OnceLock::new(),
        };

        (ck, vk)
    }

    fn precompute_ck(ck: &Self::CommitmentKey) {
        ck.ensure_h_table();
        // Eagerly init fixed-base tables for small ck widths (e.g., VC with 32 bases)
        if ck.ck.len() <= 64 {
            ck.ck_tables.get_or_init(|| {
                ck.ck
                    .par_iter()
                    .map(|base| FixedBaseMul::precompute(&E::GE::group(base), 8))
                    .collect()
            });
        }
    }

    fn blind(ck: &Self::CommitmentKey, n: usize) -> Self::Blind {
        use crate::traits::PrimeFieldExt;
        let num_rows = div_ceil(n, ck.num_cols);

        // Bulk random generation: fill all bytes at once, then reduce mod p
        let mut buf = vec![0u8; num_rows * 64];
        crate::iroha_rng::fill_bytes(&mut buf);
        HyraxBlind {
            blind: (0..num_rows)
                .map(|i| E::Scalar::from_uniform(&buf[i * 64..(i + 1) * 64]))
                .collect(),
        }
    }

    fn commit(
        ck: &Self::CommitmentKey,
        v: &[E::Scalar],
        r: &Self::Blind,
        is_small: bool,
    ) -> Result<Self::Commitment, VegaError> {
        let n = v.len();

        // compute the expected number of columns
        let num_cols = ck.num_cols;
        let num_rows = div_ceil(n, num_cols);

        // Lazily initialize fixed-base tables for small ck widths (e.g., VC with 32 bases).
        // This ensures both prover and verifier benefit from fast fixed-base commits.
        if ck.ck.len() <= 64 {
            ck.ck_tables.get_or_init(|| {
                ck.ck
                    .par_iter()
                    .map(|base| FixedBaseMul::precompute(&E::GE::group(base), 8))
                    .collect()
            });
        }
        let ck_tables = ck.ck_tables.get();
        let comm = (0..num_rows)
            .into_par_iter()
            .map(|i| {
                let upper = i.saturating_mul(num_cols).saturating_add(num_cols);
                let lower = i.saturating_mul(num_cols);
                let scalars = if upper > n {
                    &v[lower..]
                } else {
                    &v[lower..upper]
                };

                let h_table = ck
                    .h_table
                    .get_or_init(|| FixedBaseMul::precompute(&ck.h, 8));

                // Fast path: skip MSM for rows where all scalars are zero (e.g., zero-padded suffix)
                if scalars.iter().all(|s| *s == E::Scalar::ZERO) {
                    return Ok(E::GE::zero() + h_table.mul(&r.blind[i]));
                }

                // Trim trailing zeros to reduce MSM size (e.g., boundary row between data and padding)
                let effective_len = scalars
                    .iter()
                    .rposition(|s| *s != E::Scalar::ZERO)
                    .map(|pos| pos + 1)
                    .unwrap_or(scalars.len());
                let scalars = &scalars[..effective_len];

                // Fixed-base path: use precomputed tables for each ck base (fast for small ck widths)
                let msm_result = if let Some(tables) = ck_tables {
                    FixedBaseMul::multi_mul(&tables[..scalars.len()], scalars)
                } else if scalars.len() <= 16 {
                    // For very small MSMs, skip classification overhead and use direct MSM
                    E::GE::vartime_multiscalar_mul(scalars, &ck.ck[..scalars.len()], false)?
                } else {
                    // Single-pass detect+convert: one to_repr per scalar instead of two
                    let mut scalars_small: Vec<u64> = Vec::with_capacity(scalars.len());
                    let mut all_small = is_small; // caller hint: skip detection if known small
                    if !is_small {
                        all_small = true;
                        for s in scalars.iter() {
                            let r = s.to_repr();
                            if r.as_ref()[8..].iter().any(|&b| b != 0) {
                                all_small = false;
                                break;
                            }
                            scalars_small
                                .push(u64::from_le_bytes(r.as_ref()[..8].try_into().unwrap()));
                        }
                    }
                    if all_small {
                        if scalars_small.len() < scalars.len() {
                            // is_small=true path or detection bailed early: convert remaining
                            scalars_small.clear();
                            scalars_small.extend(scalars.iter().map(|s| {
                                let bytes = s.to_repr();
                                u64::from_le_bytes(bytes.as_ref()[..8].try_into().unwrap())
                            }));
                        }
                        E::GE::vartime_multiscalar_mul_small(
                            &scalars_small,
                            &ck.ck[..scalars_small.len()],
                            false,
                        )?
                    } else {
                        // Full-size scalars: direct MSM
                        E::GE::vartime_multiscalar_mul(scalars, &ck.ck[..scalars.len()], false)?
                    }
                };
                Ok(msm_result + h_table.mul(&r.blind[i]))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(HyraxCommitment { comm })
    }

    fn commit_zeros(
        ck: &Self::CommitmentKey,
        n: usize,
        r: &Self::Blind,
    ) -> Result<Self::Commitment, VegaError> {
        let num_cols = ck.num_cols;
        let num_rows = div_ceil(n, num_cols);
        let h_table = ck
            .h_table
            .get_or_init(|| FixedBaseMul::precompute(&ck.h, 8));
        let comm = (0..num_rows)
            .map(|i| E::GE::zero() + h_table.mul(&r.blind[i]))
            .collect::<Vec<_>>();
        Ok(HyraxCommitment { comm })
    }

    fn rerandomize_commitment(
        ck: &Self::CommitmentKey,
        comm: &Self::Commitment,
        r_old: &Self::Blind,
        r_new: &Self::Blind,
    ) -> Result<Self::Commitment, VegaError> {
        if comm.comm.len() != r_old.blind.len() || comm.comm.len() != r_new.blind.len() {
            return Err(VegaError::InvalidInputLength {
                reason: "rerandomize_commitment: commitment and blinds must have the same length"
                    .to_string(),
            });
        }

        // Use precomputed fixed-base table for h to speed up scalar muls
        let h_table = ck
            .h_table
            .get_or_init(|| FixedBaseMul::precompute(&ck.h, 8));

        let new_comm = (0..comm.comm.len())
            .map(|i| comm.comm[i] + h_table.mul(&(r_new.blind[i] - r_old.blind[i])))
            .collect::<Vec<_>>();

        Ok(HyraxCommitment { comm: new_comm })
    }

    fn check_commitment(comm: &Self::Commitment, n: usize, width: usize) -> Result<(), VegaError> {
        let min_rows = div_ceil(n, width);
        if comm.comm.len() != min_rows {
            return Err(VegaError::InvalidCommitmentLength {
                reason: format!(
                    "InvalidCommitmentLength: actual: {}, expected: {}",
                    comm.comm.len(),
                    min_rows
                ),
            });
        }
        Ok(())
    }

    fn combine_commitments(comms: &[Self::Commitment]) -> Result<Self::Commitment, VegaError> {
        if comms.is_empty() {
            return Err(VegaError::InvalidInputLength {
                reason: "combine_commitments: No commitments provided".to_string(),
            });
        }
        // combine comm from each commitment
        let comm = comms
            .iter()
            .flat_map(|pc| pc.comm.clone())
            .collect::<Vec<_>>();
        Ok(HyraxCommitment { comm })
    }

    fn combine_blinds(blinds: &[Self::Blind]) -> Result<Self::Blind, VegaError> {
        if blinds.is_empty() {
            return Err(VegaError::InvalidInputLength {
                reason: "combine_blinds: No blinds provided".to_string(),
            });
        }
        let mut blinds_comb = Vec::new();
        for b in blinds {
            blinds_comb.extend_from_slice(&b.blind);
        }
        Ok(HyraxBlind { blind: blinds_comb })
    }

    fn prove(
        ck: &Self::CommitmentKey,
        ck_eval: &Self::CommitmentKey,
        transcript: &mut E::TE,
        comm: &Self::Commitment,
        poly: &[E::Scalar],
        blind: &Self::Blind,
        point: &[E::Scalar],
        comm_eval: &Self::Commitment,
        blind_eval: &Self::Blind,
    ) -> Result<Self::EvaluationArgument, VegaError> {
        let n = poly.len();
        let (_setup_span, setup_t) = start_span!("hyrax_prove_prep");
        if n != (2usize).pow(point.len() as u32) {
            return Err(VegaError::InvalidInputLength {
                reason: format!(
                    "Hyrax prove: Expected {} elements in poly, got {}",
                    (2_usize).pow(point.len() as u32),
                    n
                ),
            });
        }

        transcript.absorb(b"poly_com", comm);

        let num_cols = ck.num_cols;
        let num_rows = div_ceil(n, num_cols);

        let (num_vars_rows, _) = (num_rows.log_2(), num_cols.log_2());

        let (comm_LZ, R, LZ, r_LZ) = if num_vars_rows == 0 {
            let comm_LZ = comm.comm[0];
            let R = EqPolynomial::new(point.to_vec()).evals();
            let LZ = poly.to_vec();
            let r_LZ = blind.blind[0];

            (comm_LZ, R, LZ, r_LZ)
        } else {
            let (L, R) = if rayon::current_num_threads() > 1 {
                rayon::join(
                    || EqPolynomial::new(point[..num_vars_rows].to_vec()).evals(),
                    || EqPolynomial::new(point[num_vars_rows..].to_vec()).evals(),
                )
            } else {
                let l = EqPolynomial::new(point[..num_vars_rows].to_vec()).evals();
                let r = EqPolynomial::new(point[num_vars_rows..].to_vec()).evals();
                (l, r)
            };

            info!(elapsed_ms = %setup_t.elapsed().as_millis(), "hyrax_prove_prep");

            let (_bind_span, bind_t) = start_span!("hyrax_prove_bind");
            // compute the vector underneath L*Z
            // compute vector-matrix product between L and Z viewed as a matrix
            let LZ = bind_with_delayed(poly, &L, R.len());
            info!(elapsed_ms = %bind_t.elapsed().as_millis(), "hyrax_prove_bind");

            let (_commit_span, commit_t) = start_span!("hyrax_prove_commit");

            let r_LZ = L
                .iter()
                .zip(blind.blind.iter())
                .map(|(l, b)| *l * *b)
                .fold(E::Scalar::ZERO, |acc, x| acc + x);
            let h_table = ck
                .h_table
                .get_or_init(|| FixedBaseMul::precompute(&ck.h, 8));
            let comm_LZ =
                E::GE::vartime_multiscalar_mul(&LZ, &ck.ck[..LZ.len()], true)? + h_table.mul(&r_LZ);

            info!(elapsed_ms = %commit_t.elapsed().as_millis(), "hyrax_prove_commit");

            (comm_LZ, R, LZ, r_LZ)
        };

        // a dot product argument (IPA) of size R_size
        let (_ipa_span, ipa_t) = start_span!("hyrax_prove_ipa");
        let ipa_instance = InnerProductInstance::<E>::new(&comm_LZ, &R, &comm_eval.comm[0]);
        let ipa_witness = InnerProductWitness::<E>::new(&LZ, &r_LZ, &blind_eval.blind[0]);
        let ipa = InnerProductArgumentLinear::<E>::prove(
            &ck.ck,
            &ck.h,
            &ck_eval.ck[0],
            &ck_eval.h,
            &ipa_instance,
            &ipa_witness,
            transcript,
        )?;
        info!(elapsed_ms = %ipa_t.elapsed().as_millis(), "hyrax_prove_ipa");

        Ok(HyraxEvaluationArgument { ipa })
    }

    fn verify(
        vk: &Self::VerifierKey,
        ck_eval: &Self::CommitmentKey,
        transcript: &mut E::TE,
        comm: &Self::Commitment,
        point: &[E::Scalar],
        comm_eval: &Self::Commitment,
        arg: &Self::EvaluationArgument,
    ) -> Result<(), VegaError> {
        let (_verify_span, verify_t) = start_span!("hyrax_pcs_verify");
        transcript.absorb(b"poly_com", comm);

        // compute L and R
        let (_lr_span, lr_t) = start_span!("hyrax_compute_lr");
        // n = 2^point.len()
        let n = (2_usize).pow(point.len() as u32);
        let num_cols = vk.num_cols;
        let num_rows = div_ceil(n, num_cols);

        let (num_vars_rows, _num_vars_cols) = (num_rows.log_2(), num_cols.log_2());

        // The commitment must have exactly the number of rows implied by the opening
        // point, and the evaluation commitment must be non-empty.
        if comm.comm.len() != num_rows {
            return Err(VegaError::InvalidCommitmentLength {
                reason: format!(
                    "Hyrax verify: commitment has {} rows, expected exactly {}",
                    comm.comm.len(),
                    num_rows
                ),
            });
        }
        if comm_eval.comm.is_empty() {
            return Err(VegaError::InvalidCommitmentLength {
                reason: "Hyrax verify: evaluation commitment is empty".to_string(),
            });
        }

        let (comm_LZ, R) = if num_vars_rows == 0 {
            let R = EqPolynomial::new(point.to_vec()).evals();
            (comm.comm[0], R)
        } else {
            let L = EqPolynomial::new(point[..num_vars_rows].to_vec()).evals();
            let R = EqPolynomial::new(point[num_vars_rows..].to_vec()).evals();

            // compute a weighted sum of commitments and L
            // convert the commitments to affine form so we can do a multi-scalar multiplication
            let ck: Vec<_> = comm.comm.iter().map(|c| c.affine()).collect();
            let comm_LZ = E::GE::vartime_multiscalar_mul(&L, &ck[..L.len()], true)?;
            info!(elapsed_ms = %lr_t.elapsed().as_millis(), "hyrax_compute_lr");

            (comm_LZ, R)
        };

        let ipa_instance = InnerProductInstance::<E>::new(&comm_LZ, &R, &comm_eval.comm[0]);

        let result = arg.ipa.verify(
            &vk.ck,
            &vk.h,
            &ck_eval.ck[0],
            &ck_eval.h,
            R.len(),
            &ipa_instance,
            transcript,
        );

        info!(elapsed_ms = %verify_t.elapsed().as_millis(), "hyrax_pcs_verify");
        result
    }

    fn commit_without_blind(
        ck: &Self::CommitmentKey,
        v: &[E::Scalar],
        is_small: bool,
    ) -> Result<Vec<E::GE>, VegaError> {
        use ff::Field;
        let n = v.len();
        let num_cols = ck.ck.len();
        let num_rows = div_ceil(n, num_cols);

        let raw_points: Vec<E::GE> = (0..num_rows)
            .into_par_iter()
            .map(|i| {
                let row_start = i * num_cols;
                let row_end = std::cmp::min(row_start + num_cols, n);
                let row = &v[row_start..row_end];
                let all_zero = row.iter().all(|s| s.is_zero().into());
                if all_zero {
                    Ok(E::GE::zero())
                } else if is_small {
                    let scalars_small: Vec<u64> = row
                        .iter()
                        .map(|s| {
                            let r = s.to_repr();
                            u64::from_le_bytes(r.as_ref()[..8].try_into().unwrap())
                        })
                        .collect();
                    E::GE::vartime_multiscalar_mul_small(&scalars_small, &ck.ck[..row.len()], false)
                } else {
                    E::GE::vartime_multiscalar_mul(row, &ck.ck[..row.len()], false)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(raw_points)
    }

    fn commit_incremental(
        ck: &Self::CommitmentKey,
        raw: &[E::GE],
        delta: &[E::Scalar],
        blind: &Self::Blind,
    ) -> Result<Self::Commitment, VegaError> {
        use ff::Field;
        let num_cols = ck.ck.len();
        let n = delta.len();
        let num_rows = div_ceil(n, num_cols);
        let h_table = ck
            .h_table
            .get_or_init(|| FixedBaseMul::precompute(&ck.h, 8));

        let comm: Result<Vec<E::GE>, VegaError> = (0..num_rows)
            .into_par_iter()
            .map(|i| {
                let row_start = i * num_cols;
                let row_end = std::cmp::min(row_start + num_cols, n);
                let row = &delta[row_start..row_end];

                let all_zero = row.iter().all(|s| s.is_zero().into());

                let raw_point = if i < raw.len() { raw[i] } else { E::GE::zero() };

                let point = if all_zero {
                    raw_point
                } else {
                    let delta_msm =
                        E::GE::vartime_multiscalar_mul(row, &ck.ck[..row.len()], false)?;
                    raw_point + delta_msm
                };

                // Add blinding factor
                Ok(point + h_table.mul(&blind.blind[i]))
            })
            .collect();

        Ok(HyraxCommitment { comm: comm? })
    }

    fn prove_direct(
        ck: &Self::CommitmentKey,
        poly: &[E::Scalar],
        blind: &Self::Blind,
        point: &[E::Scalar],
    ) -> Result<(Vec<E::Scalar>, E::Scalar), VegaError> {
        let num_cols = ck.num_cols;
        // Derive dimensions from point length (must match how commitment was structured)
        let n = (2_usize).pow(point.len() as u32);
        let num_rows = div_ceil(n, num_cols);

        if num_rows == 1 {
            let mut v = poly.to_vec();
            v.resize(num_cols, E::Scalar::ZERO);
            return Ok((v, blind.blind[0]));
        }

        let num_vars_rows = num_rows.log_2();
        let point_left = &point[..num_vars_rows];

        // Pad polynomial to full n
        let mut padded_poly;
        let poly_ref = if poly.len() < n {
            padded_poly = poly.to_vec();
            padded_poly.resize(n, E::Scalar::ZERO);
            &padded_poly
        } else {
            poly
        };

        let L = EqPolynomial::new(point_left.to_vec()).evals();

        // v = L * poly_matrix (bind row variables)
        let v = bind_with_delayed(poly_ref, &L, num_cols);

        // combined_blind = <L, blind> (only over actual blind rows; padding rows have zero blind)
        let combined_blind = L
            .iter()
            .zip(blind.blind.iter())
            .map(|(l, b)| *l * *b)
            .fold(E::Scalar::ZERO, |acc, x| acc + x);

        Ok((v, combined_blind))
    }

    fn verify_direct(
        vk: &Self::VerifierKey,
        comm: &Self::Commitment,
        v: &[E::Scalar],
        combined_blind: &E::Scalar,
        point: &[E::Scalar],
    ) -> Result<E::Scalar, VegaError> {
        let num_cols = vk.num_cols;
        if v.len() != num_cols {
            return Err(VegaError::ProofVerifyError {
                reason: format!(
                    "Direct opening: v.len() ({}) != num_cols ({})",
                    v.len(),
                    num_cols
                ),
            });
        }
        // Derive dimensions from point length (same as standard Hyrax verify)
        let n = (2_usize).pow(point.len() as u32);
        let num_rows = div_ceil(n, num_cols);
        let num_vars_rows = num_rows.log_2();

        // A direct opening commits the (folded) witness, which may carry fewer rows than the
        // padded point space; the omitted high rows are zero commitments that contribute
        // identity to the row combination. The commitment must therefore be non-empty and
        // carry at most `num_rows` rows.
        if comm.comm.is_empty() || comm.comm.len() > num_rows {
            return Err(VegaError::InvalidCommitmentLength {
                reason: format!(
                    "Direct opening: commitment row count {} is invalid (expected 1..={})",
                    comm.comm.len(),
                    num_rows
                ),
            });
        }

        let comm_LZ = if num_vars_rows == 0 {
            comm.comm[0]
        } else {
            let L = EqPolynomial::new(point[..num_vars_rows].to_vec()).evals();
            let actual_rows = comm.comm.len();
            let ck_aff: Vec<_> = comm.comm.iter().map(|c| c.affine()).collect();
            E::GE::vartime_multiscalar_mul(&L[..actual_rows], &ck_aff, true)?
        };

        // Recompute commitment from v and combined_blind
        let expected =
            E::GE::vartime_multiscalar_mul(v, &vk.ck[..v.len()], false)? + vk.h * *combined_blind;

        if comm_LZ != expected {
            return Err(VegaError::ProofVerifyError {
                reason: "Direct opening: commitment mismatch".to_string(),
            });
        }

        // Compute evaluation: <v, eq(point_right)>
        let point_right = &point[num_vars_rows..];
        let R = EqPolynomial::new(point_right.to_vec()).evals();
        let eval = v
            .iter()
            .zip(R.iter())
            .map(|(vi, ri)| *vi * *ri)
            .fold(E::Scalar::ZERO, |acc, x| acc + x);

        Ok(eval)
    }
}

impl<E: Engine> TranscriptReprTrait<E::GE> for HyraxCommitment<E>
where
    E::GE: DlogGroupExt,
{
    fn to_transcript_bytes(&self) -> Vec<u8> {
        let mut v = Vec::new();
        v.append(&mut b"poly_commitment_begin".to_vec());

        for c in &self.comm {
            v.extend(c.to_transcript_bytes());
        }

        v.append(&mut b"poly_commitment_end".to_vec());
        v
    }
}

impl<E: Engine> CommitmentTrait<E> for HyraxCommitment<E> where E::GE: DlogGroupExt {}

impl<E: Engine> FoldingEngineTrait<E> for HyraxPCS<E>
where
    E::GE: DlogGroupExt,
{
    fn fold_commitments(
        comms: &[Self::Commitment],
        weights: &[E::Scalar],
    ) -> Result<Self::Commitment, VegaError> {
        if comms.is_empty() || weights.is_empty() || comms.len() != weights.len() {
            return Err(VegaError::InvalidInputLength {
                reason: "fold_commitments: Commitments and weights must have the same length"
                    .to_string(),
            });
        }

        // take weighted sum of commitments via MSM
        let n = comms[0].comm.len();
        if !comms.iter().all(|c| c.comm.len() == n) {
            return Err(VegaError::InvalidInputLength {
                reason: "fold_commitments: all inner commitment vectors must have the same length"
                    .into(),
            });
        }

        let num_comms = comms.len();

        // Fast path: fold 2 commitments where one weight is ONE.
        if num_comms == 2 {
            let (unit_idx, scalar_idx, scalar_w) = if weights[0] == E::Scalar::ONE {
                (0, 1, weights[1])
            } else if weights[1] == E::Scalar::ONE {
                (1, 0, weights[0])
            } else {
                (usize::MAX, usize::MAX, E::Scalar::ZERO)
            };
            if unit_idx != usize::MAX {
                let mut folded_comm = Vec::with_capacity(n);
                for (p, q) in comms[unit_idx]
                    .comm
                    .iter()
                    .zip(comms[scalar_idx].comm.iter())
                {
                    folded_comm.push(*p + vartime_scalar_mul::<E>(*q, &scalar_w));
                }
                return Ok(Self::Commitment { comm: folded_comm });
            }
        }

        // Batch-convert all projective points to affine (1 inversion via Montgomery's trick)
        let all_projective: Vec<E::GE> = (0..n)
            .flat_map(|row| comms.iter().map(move |c| c.comm[row]))
            .collect();
        let all_affine = E::GE::batch_affine(&all_projective);

        // Use shared-weight MSM: all rows share the same scalar weights,
        // so scalar decomposition is done once instead of n times.
        let bases_rows: Vec<&[<E::GE as DlogGroup>::AffineGroupElement]> = (0..n)
            .map(|row| &all_affine[row * num_comms..(row + 1) * num_comms])
            .collect();
        let folded_comm = E::GE::vartime_multiscalar_mul_shared_weights(weights, &bases_rows)?;

        Ok(Self::Commitment { comm: folded_comm })
    }

    fn fold_blinds(
        blinds: &[Self::Blind],
        weights: &[<E as Engine>::Scalar],
    ) -> Result<Self::Blind, VegaError> {
        if blinds.is_empty() || blinds.len() != weights.len() {
            return Err(VegaError::InvalidInputLength {
                reason: "fold_blinds: blinds and weights must be non-empty and same length".into(),
            });
        }
        let n = blinds[0].blind.len();
        if !blinds.iter().all(|b| b.blind.len() == n) {
            return Err(VegaError::InvalidInputLength {
                reason: "fold_blinds: all inner blind vectors must have the same length".into(),
            });
        }

        let mut acc = vec![<E as Engine>::Scalar::ZERO; n];
        for (b, w) in blinds.iter().zip(weights.iter()) {
            for (a, &x) in acc.iter_mut().zip(&b.blind) {
                *a += x * *w;
            }
        }

        Ok(Self::Blind { blind: acc })
    }

    fn fold_commitments_partial(
        comms: &[Self::Commitment],
        weights: &[E::Scalar],
        num_data_rows: usize,
        folded_blind: &Self::Blind,
        ck: &Self::CommitmentKey,
    ) -> Result<Self::Commitment, VegaError> {
        if comms.is_empty() || weights.is_empty() || comms.len() != weights.len() {
            return Err(VegaError::InvalidInputLength {
                reason:
                    "fold_commitments_partial: Commitments and weights must have the same length"
                        .to_string(),
            });
        }

        let total_rows = comms[0].comm.len();
        if num_data_rows > total_rows {
            return Err(VegaError::InvalidInputLength {
                reason: format!(
                    "fold_commitments_partial: num_data_rows ({}) exceeds total_rows ({})",
                    num_data_rows, total_rows
                ),
            });
        }
        if num_data_rows >= total_rows {
            return Self::fold_commitments(comms, weights);
        }

        let num_comms = comms.len();

        // Step 1: Fold only the data rows via MSM
        let data_comms: Vec<Self::Commitment> = comms
            .iter()
            .map(|c| HyraxCommitment {
                comm: c.comm[..num_data_rows].to_vec(),
            })
            .collect();
        let data_folded = Self::fold_commitments(&data_comms, &weights[..num_comms])?;

        // Step 2: Compute rest rows from folded blind + h
        // For rest rows, each comm_k[row] = blind_k[row] * h, so:
        //   folded[row] = sum_k w[k] * blind_k[row] * h = folded_blind[row] * h
        let h_table = ck
            .h_table
            .get_or_init(|| FixedBaseMul::precompute(&ck.h, 8));
        let num_rest_rows = total_rows - num_data_rows;
        let mut comm = data_folded.comm;
        comm.reserve(num_rest_rows);
        for i in 0..num_rest_rows {
            let row = num_data_rows + i;
            comm.push(h_table.mul(&folded_blind.blind[row]));
        }

        Ok(HyraxCommitment { comm })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        provider::PallasHyraxEngine,
        traits::{Engine, pcs::PCSEngineTrait, transcript::TranscriptEngineTrait},
    };

    type E = PallasHyraxEngine;
    type Scalar = <E as Engine>::Scalar;

    fn sample_poly(n: usize) -> Vec<Scalar> {
        (0..n).map(|i| Scalar::from(i as u64 + 1)).collect()
    }

    fn sample_point(num_vars: usize) -> Vec<Scalar> {
        (0..num_vars).map(|i| Scalar::from(i as u64 + 7)).collect()
    }

    // At the direct-opening boundary an empty or oversized commitment is rejected, and a
    // short commitment inconsistent with the opening fails the binding check.
    #[test]
    fn hyrax_direct_opening_rejects_malformed_commitment_rows() {
        crate::iroha_rng::with_deterministic_test_seed(0x20, || {
            let width = 32;
            let num_vars = 6; // n = 64 -> num_rows = 2
            let n = 1usize << num_vars;
            let (ck, vk_ee) = <E as Engine>::PCS::setup(b"test_direct", n, width);
            let poly = sample_poly(n);
            let point = sample_point(num_vars);
            let blind = <E as Engine>::PCS::blind(&ck, n);
            let comm = <E as Engine>::PCS::commit(&ck, &poly, &blind, false).unwrap();

            let (v, cb) = <E as Engine>::PCS::prove_direct(&ck, &poly, &blind, &point).unwrap();

            // Well-formed commitment opens successfully.
            assert!(<E as Engine>::PCS::verify_direct(&vk_ee, &comm, &v, &cb, &point).is_ok());

            // Extra row.
            let mut comm_extra = comm.clone();
            comm_extra.comm.push(comm.comm[0]);
            assert!(
                <E as Engine>::PCS::verify_direct(&vk_ee, &comm_extra, &v, &cb, &point).is_err()
            );

            // Missing row.
            let mut comm_short = comm.clone();
            comm_short.comm.pop();
            assert!(
                <E as Engine>::PCS::verify_direct(&vk_ee, &comm_short, &v, &cb, &point).is_err()
            );

            // Empty commitment.
            let comm_empty = HyraxCommitment::<E> { comm: vec![] };
            assert!(
                <E as Engine>::PCS::verify_direct(&vk_ee, &comm_empty, &v, &cb, &point).is_err()
            );
        });
    }

    // The single-row case (num_vars_rows == 0) must reject an empty commitment.
    #[test]
    fn hyrax_direct_opening_rejects_empty_single_row_commitment() {
        crate::iroha_rng::with_deterministic_test_seed(0x21, || {
            let width = 32;
            let num_vars = 4; // n = 16 -> num_rows = 1
            let n = 1usize << num_vars;
            let (ck, vk_ee) = <E as Engine>::PCS::setup(b"test_direct_single", n, width);
            let poly = sample_poly(n);
            let point = sample_point(num_vars);
            let blind = <E as Engine>::PCS::blind(&ck, n);

            let (v, cb) = <E as Engine>::PCS::prove_direct(&ck, &poly, &blind, &point).unwrap();
            let comm_empty = HyraxCommitment::<E> { comm: vec![] };
            assert!(
                <E as Engine>::PCS::verify_direct(&vk_ee, &comm_empty, &v, &cb, &point).is_err()
            );
        });
    }

    // The standard evaluation-opening boundary rejects a polynomial commitment too small
    // to open at the point, and an empty evaluation commitment.
    #[test]
    fn hyrax_opening_rejects_undersized_commitments() {
        crate::iroha_rng::with_deterministic_test_seed(0x22, || {
            let width = 32;
            let num_vars = 6; // n = 64 -> num_rows = 2
            let n = 1usize << num_vars;
            let (ck, vk_ee) = <E as Engine>::PCS::setup(b"test_open", n, width);
            let (ck_s, _) = <E as Engine>::PCS::setup(b"ck_s", 1, 1);
            let poly = sample_poly(n);
            let point = sample_point(num_vars);
            let blind = <E as Engine>::PCS::blind(&ck, n);
            let comm = <E as Engine>::PCS::commit(&ck, &poly, &blind, false).unwrap();

            // Exact evaluation certified by the opening.
            let (v, cb) = <E as Engine>::PCS::prove_direct(&ck, &poly, &blind, &point).unwrap();
            let eval = <E as Engine>::PCS::verify_direct(&vk_ee, &comm, &v, &cb, &point).unwrap();
            let blind_eval = <E as Engine>::PCS::blind(&ck_s, 1);
            let comm_eval = <E as Engine>::PCS::commit(&ck_s, &[eval], &blind_eval, false).unwrap();

            let mut tp = <E as Engine>::TE::new(b"open");
            let arg = <E as Engine>::PCS::prove(
                &ck,
                &ck_s,
                &mut tp,
                &comm,
                &poly,
                &blind,
                &point,
                &comm_eval,
                &blind_eval,
            )
            .unwrap();

            // Well-formed commitment verifies.
            let mut tv = <E as Engine>::TE::new(b"open");
            assert!(
                <E as Engine>::PCS::verify(&vk_ee, &ck_s, &mut tv, &comm, &point, &comm_eval, &arg)
                    .is_ok()
            );

            // Polynomial commitment too small to open at the point.
            let mut comm_short = comm.clone();
            comm_short.comm.pop();
            let mut tv = <E as Engine>::TE::new(b"open");
            assert!(
                <E as Engine>::PCS::verify(
                    &vk_ee,
                    &ck_s,
                    &mut tv,
                    &comm_short,
                    &point,
                    &comm_eval,
                    &arg
                )
                .is_err()
            );

            // Empty evaluation commitment.
            let comm_eval_empty = HyraxCommitment::<E> { comm: vec![] };
            let mut tv = <E as Engine>::TE::new(b"open");
            assert!(
                <E as Engine>::PCS::verify(
                    &vk_ee,
                    &ck_s,
                    &mut tv,
                    &comm,
                    &point,
                    &comm_eval_empty,
                    &arg
                )
                .is_err()
            );
        });
    }
}
