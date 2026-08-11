// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! This module defines R1CS related types
use crate::{
    Blind, Commitment, CommitmentKey, DEFAULT_COMMITMENT_WIDTH, PCS, VerifierKey,
    big_num::DelayedReduction,
    big_num::montgomery::MontgomeryLimbs,
    digest::SimpleDigestible,
    errors::VegaError,
    traits::{
        Engine,
        pcs::{FoldingEngineTrait, PCSEngineTrait},
        transcript::{TranscriptEngineTrait, TranscriptReprTrait},
    },
};
use core::cmp::max;
use ff::{Field, PrimeField};
use once_cell::sync::OnceCell;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

mod folds;
mod sparse;
pub(crate) use sparse::FilteredSpmv;
pub(crate) use sparse::PrecomputedSparseMatrix;
pub(crate) use sparse::SparseMatrix;

/// Fused evaluation of three sparse matrices at (T_x, T_y).
/// Processes all three matrices per row to improve T_y cache reuse.
/// Hoists T_x[row] out of inner loop and uses delayed reduction.
#[inline(never)]
fn evaluate_three_matrices_fused<
    F: PrimeField + MontgomeryLimbs + Copy + DelayedReduction<F> + Send + Sync,
>(
    pa: &PrecomputedSparseMatrix<F>,
    pb: &PrecomputedSparseMatrix<F>,
    pc: &PrecomputedSparseMatrix<F>,
    t_x: &[F],
    t_y: &[F],
) -> (F, F, F) {
    let num_rows = pa.num_rows;

    if rayon::current_num_threads() <= 1 || num_rows <= 1024 {
        evaluate_three_matrices_sequential(pa, pb, pc, t_x, t_y, 0, num_rows)
    } else {
        use rayon::prelude::*;
        let num_threads = rayon::current_num_threads();
        let chunk_size = num_rows.div_ceil(num_threads);

        let partials: Vec<(F, F, F)> = (0..num_threads)
            .into_par_iter()
            .map(|t| {
                let start = t * chunk_size;
                let end = (start + chunk_size).min(num_rows);
                if start >= end {
                    return (F::ZERO, F::ZERO, F::ZERO);
                }
                evaluate_three_matrices_sequential(pa, pb, pc, t_x, t_y, start, end)
            })
            .collect();

        partials
            .into_iter()
            .fold((F::ZERO, F::ZERO, F::ZERO), |(a, b, c), (pa, pb, pc)| {
                (a + pa, b + pb, c + pc)
            })
    }
}

#[inline(never)]
fn evaluate_three_matrices_sequential<
    F: PrimeField + MontgomeryLimbs + Copy + DelayedReduction<F>,
>(
    pa: &PrecomputedSparseMatrix<F>,
    pb: &PrecomputedSparseMatrix<F>,
    pc: &PrecomputedSparseMatrix<F>,
    t_x: &[F],
    t_y: &[F],
    start_row: usize,
    end_row: usize,
) -> (F, F, F) {
    type Acc<S> = <S as DelayedReduction<S>>::Accumulator;
    let mut acc_a = Acc::<F>::default();
    let mut acc_b = Acc::<F>::default();
    let mut acc_c = Acc::<F>::default();

    #[allow(clippy::needless_range_loop)]
    for row in start_row..end_row {
        let row_sum_a = row_dot_ty(pa, row, t_y);
        let row_sum_b = row_dot_ty(pb, row, t_y);
        let row_sum_c = row_dot_ty(pc, row, t_y);

        let tx = &t_x[row];
        F::unreduced_multiply_accumulate(&mut acc_a, tx, &row_sum_a);
        F::unreduced_multiply_accumulate(&mut acc_b, tx, &row_sum_b);
        F::unreduced_multiply_accumulate(&mut acc_c, tx, &row_sum_c);
    }

    (F::reduce(&acc_a), F::reduce(&acc_b), F::reduce(&acc_c))
}

/// Compute dot product of a single matrix row with T_y: sum val * T_y[col].
/// Uses delayed reduction for general entries to avoid per-multiply Montgomery REDC.
#[inline(always)]
fn row_dot_ty<F: PrimeField + DelayedReduction<F>>(
    pm: &PrecomputedSparseMatrix<F>,
    row: usize,
    t_y: &[F],
) -> F {
    // Unit and small entries: regular field arithmetic (additions are cheap)
    let mut sum = F::ZERO;

    let (s, e) = pm.range_unit_pos(row);
    for i in s..e {
        sum += t_y[pm.unit_pos_cols[i] as usize];
    }
    let (s, e) = pm.range_unit_neg(row);
    for i in s..e {
        sum -= t_y[pm.unit_neg_cols[i] as usize];
    }
    let (s, e) = pm.range_small(row);
    for i in s..e {
        sum +=
            PrecomputedSparseMatrix::small_mul(pm.small_coeffs[i], t_y[pm.small_cols[i] as usize]);
    }

    // General entries: use delayed reduction to batch Montgomery reductions
    let (gs, ge) = pm.range_general(row);
    if gs < ge {
        type Acc<S> = <S as DelayedReduction<S>>::Accumulator;
        let mut acc = Acc::<F>::default();
        for i in gs..ge {
            F::unreduced_multiply_accumulate(
                &mut acc,
                &pm.general_vals[i],
                &t_y[pm.general_cols[i] as usize],
            );
        }
        sum += F::reduce(&acc);
    }

    sum
}

fn eq01<F: Field>(bit: u8, r: &F) -> F {
    if bit == 0 { F::ONE - *r } else { *r }
}

#[inline]
pub(crate) fn weights_from_r<F: Field>(r_bs: &[F], n: usize) -> Vec<F> {
    let ell = r_bs.len();
    (0..n)
        .map(|i| {
            let mut wi = F::ONE;
            let mut k = i;
            for r_bs_t in r_bs.iter().take(ell) {
                wi *= eq01((k & 1) as u8, r_bs_t);
                k >>= 1;
            }
            wi
        })
        .collect()
}

/// A type that holds the shape of the R1CS matrices
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct R1CSShape<E: Engine> {
    pub(crate) num_cons: usize,
    pub(crate) num_vars: usize,
    pub(crate) num_io: usize, // input/output
    pub(crate) A: SparseMatrix<E::Scalar>,
    pub(crate) B: SparseMatrix<E::Scalar>,
    pub(crate) C: SparseMatrix<E::Scalar>,
    #[serde(skip, default = "OnceCell::new")]
    pub(crate) digest: OnceCell<E::Scalar>,
}

impl<E: Engine> SimpleDigestible for R1CSShape<E> {}

/// A type that holds a witness for a given R1CS instance
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct R1CSWitness<E: Engine> {
    pub(crate) is_small: bool, // whether the witness elements fit in machine words
    pub(crate) W: Vec<E::Scalar>,
    pub(crate) r_W: Blind<E>,
}

/// A type that holds an R1CS instance
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct R1CSInstance<E: Engine> {
    pub(crate) comm_W: Commitment<E>,
    pub(crate) X: Vec<E::Scalar>,
}

/// A type that holds a witness for a given Relaxed R1CS instance
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct RelaxedR1CSWitness<E: Engine> {
    pub(crate) W: Vec<E::Scalar>,
    pub(crate) r_W: Blind<E>,
    pub(crate) E: Vec<E::Scalar>,
    pub(crate) r_E: Blind<E>,
}

/// A type that holds a Relaxed R1CS instance
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct RelaxedR1CSInstance<E: Engine> {
    pub(crate) comm_W: Commitment<E>,
    pub(crate) comm_E: Commitment<E>,
    pub(crate) X: Vec<E::Scalar>,
    pub(crate) u: E::Scalar,
}

impl<E: Engine> RelaxedR1CSWitness<E> {
    /// Commits to the witness using the supplied generators
    pub fn commit(
        &self,
        ck: &CommitmentKey<E>,
    ) -> Result<(Commitment<E>, Commitment<E>), VegaError> {
        Ok((
            PCS::<E>::commit(ck, &self.W, &self.r_W, false)?,
            PCS::<E>::commit(ck, &self.E, &self.r_E, false)?,
        ))
    }
}

#[cfg(test)]
mod tests_relaxed_sample {
    use super::*;
    use crate::{provider::P256HyraxEngine, traits::Engine};
    use ff::Field;

    fn tiny_r1cs<E: Engine>(num_vars: usize) -> R1CSShape<E> {
        let one = <E::Scalar as Field>::ONE;
        let (num_cons, num_vars, num_io, a_entries, b_entries, c_entries) = {
            let num_cons = 4;
            let num_io = 2;

            let mut A: Vec<(usize, usize, E::Scalar)> = Vec::new();
            let mut B: Vec<(usize, usize, E::Scalar)> = Vec::new();
            let mut C: Vec<(usize, usize, E::Scalar)> = Vec::new();

            // constraint 0: I0 * I0 - Z0 = 0
            A.push((0, num_vars + 1, one));
            B.push((0, num_vars + 1, one));
            C.push((0, 0, one));

            // constraint 1: Z0 * I0 - Z1 = 0
            A.push((1, 0, one));
            B.push((1, num_vars + 1, one));
            C.push((1, 1, one));

            // constraint 2: (Z1 + I0) * 1 - Z2 = 0
            A.push((2, 1, one));
            A.push((2, num_vars + 1, one));
            B.push((2, num_vars, one));
            C.push((2, 2, one));

            // constraint 3: (Z2 + 5) * 1 - I1 = 0
            A.push((3, 2, one));
            A.push((3, num_vars, one + one + one + one + one));
            B.push((3, num_vars, one));
            C.push((3, num_vars + 2, one));

            (num_cons, num_vars, num_io, A, B, C)
        };

        let rows = num_cons;
        let cols = num_vars + num_io + 1;

        R1CSShape::new(
            num_cons,
            num_vars,
            num_io,
            SparseMatrix::new(&a_entries, rows, cols),
            SparseMatrix::new(&b_entries, rows, cols),
            SparseMatrix::new(&c_entries, rows, cols),
        )
        .unwrap()
    }

    fn test_random_sample_with<E: Engine>() {
        let s = tiny_r1cs::<E>(4);
        let (ck, _) = s.commitment_key();
        let (inst, wit) = s.sample_random_instance_witness(&ck).unwrap();
        assert!(s.is_sat_relaxed(&ck, &inst, &wit).is_ok());
    }

    #[test]
    fn test_random_sample() {
        crate::iroha_rng::with_deterministic_test_seed(0x30, || {
            test_random_sample_with::<P256HyraxEngine>();
        });
    }

    #[test]
    fn test_fold_multiple_rejects_mismatched_x() {
        crate::iroha_rng::with_deterministic_test_seed(0x31, || {
            type E = P256HyraxEngine;
            let s = tiny_r1cs::<E>(4);
            let (ck, _) = s.commitment_key();
            let one = <<E as Engine>::Scalar as Field>::ONE;
            let r_w = PCS::<E>::blind(&ck, 1);
            let comm = PCS::<E>::commit(&ck, &[one], &r_w, false).unwrap();

            let u_a = R1CSInstance::<E> {
                comm_W: comm.clone(),
                X: vec![one, one],
            };
            let u_b = R1CSInstance::<E> {
                comm_W: comm,
                X: vec![one, one, one],
            };

            // equal-length instances fold; mismatched public IO lengths are rejected
            assert!(R1CSInstance::fold_multiple(&[one], &[u_a.clone(), u_a.clone()]).is_ok());
            assert!(R1CSInstance::fold_multiple(&[one], &[u_a, u_b]).is_err());
        });
    }
}

/// Round `n` up to the next multiple of width.
/// (If `n` is already a multiple and higher than zero, it is returned unchanged.)
#[inline]
pub fn pad_to_width(width: usize, n: usize) -> usize {
    if n == 0 {
        return 0;
    }

    // width == 1024 == 1 << 10, so the mask is width-1 == 0b111_1111_1111 (10 bits set).
    n.saturating_add(width - 1) & !(width - 1)
}

fn is_sparse_matrix_valid<E: Engine>(
    num_rows: usize,
    num_cols: usize,
    M: &SparseMatrix<E::Scalar>,
) -> Result<(), VegaError> {
    // Check if the indices and indptr are valid for the given number of rows and columns
    M.iter().try_for_each(|(row, col, _val)| {
        if row >= num_rows || col >= num_cols {
            Err(VegaError::InvalidIndex)
        } else {
            Ok(())
        }
    })
}

impl<E: Engine> R1CSShape<E> {
    /// Create an object of type `R1CSShape` from the explicitly specified R1CS matrices
    pub fn new(
        num_cons: usize,
        num_vars: usize,
        num_io: usize,
        A: SparseMatrix<E::Scalar>,
        B: SparseMatrix<E::Scalar>,
        C: SparseMatrix<E::Scalar>,
    ) -> Result<R1CSShape<E>, VegaError> {
        let num_rows = num_cons;
        let num_cols = num_vars + 1 + num_io; // +1 for the constant term

        is_sparse_matrix_valid::<E>(num_rows, num_cols, &A)?;
        is_sparse_matrix_valid::<E>(num_rows, num_cols, &B)?;
        is_sparse_matrix_valid::<E>(num_rows, num_cols, &C)?;

        Ok(R1CSShape {
            num_cons,
            num_vars,
            num_io,
            A,
            B,
            C,
            digest: OnceCell::new(),
        })
    }

    /// Checks if the R1CS instance is satisfiable given a witness and its shape
    #[allow(dead_code)]
    pub fn is_sat(
        &self,
        ck: &CommitmentKey<E>,
        U: &R1CSInstance<E>,
        W: &R1CSWitness<E>,
    ) -> Result<(), VegaError> {
        assert_eq!(W.W.len(), self.num_vars);
        assert_eq!(U.X.len(), self.num_io);

        // verify if Az * Bz = u*Cz
        let res_eq = {
            let z = [W.W.clone(), vec![E::Scalar::ONE], U.X.clone()].concat();
            let (Az, Bz, Cz) = self.multiply_vec(&z)?;
            assert_eq!(Az.len(), self.num_cons);
            assert_eq!(Bz.len(), self.num_cons);
            assert_eq!(Cz.len(), self.num_cons);

            (0..self.num_cons).all(|i| Az[i] * Bz[i] == Cz[i])
        };

        // verify if comm_W is a commitment to W
        let res_comm = U.comm_W == PCS::<E>::commit(ck, &W.W, &W.r_W, W.is_small)?;

        if !res_eq {
            return Err(VegaError::UnSat {
                reason: "R1CS is unsatisfiable".to_string(),
            });
        }

        if !res_comm {
            return Err(VegaError::UnSat {
                reason: "Invalid commitment".to_string(),
            });
        }

        Ok(())
    }

    /// Generates public parameters for a Rank-1 Constraint System (R1CS).
    ///
    /// This function takes into consideration the shape of the R1CS matrices
    ///
    /// # Arguments
    ///
    /// * `S`: The shape of the R1CS matrices.
    ///
    pub fn commitment_key(&self) -> (CommitmentKey<E>, VerifierKey<E>) {
        E::PCS::setup(b"ck", self.num_vars, DEFAULT_COMMITMENT_WIDTH)
    }

    pub fn multiply_vec(
        &self,
        z: &[E::Scalar],
    ) -> Result<(Vec<E::Scalar>, Vec<E::Scalar>, Vec<E::Scalar>), VegaError> {
        if z.len() != self.num_io + 1 + self.num_vars {
            return Err(VegaError::InvalidWitnessLength);
        }

        if rayon::current_num_threads() <= 1 {
            let az = self.A.multiply_vec(z)?;
            let bz = self.B.multiply_vec(z)?;
            let cz = self.C.multiply_vec(z)?;
            Ok((az, bz, cz))
        } else {
            let (Az, (Bz, Cz)) = rayon::join(
                || self.A.multiply_vec(z),
                || rayon::join(|| self.B.multiply_vec(z), || self.C.multiply_vec(z)),
            );
            Ok((Az?, Bz?, Cz?))
        }
    }
    /// Checks if the Relaxed R1CS instance is satisfiable given a witness and its shape
    pub fn is_sat_relaxed(
        &self,
        ck: &CommitmentKey<E>,
        U: &RelaxedR1CSInstance<E>,
        W: &RelaxedR1CSWitness<E>,
    ) -> Result<(), VegaError> {
        assert_eq!(W.W.len(), self.num_vars);
        assert_eq!(W.E.len(), self.num_cons);
        assert_eq!(U.X.len(), self.num_io);

        // verify if Az * Bz = u*Cz + E
        let res_eq = {
            let z = [W.W.clone(), vec![U.u], U.X.clone()].concat();
            let (az, bz, cz) = self.multiply_vec(&z)?;
            (0..self.num_cons).all(|i| az[i] * bz[i] == U.u * cz[i] + W.E[i])
        };

        // verify if comm_E and comm_W are commitments to E and W
        let res_comm = {
            let (comm_W_result, comm_E_result) = rayon::join(
                || PCS::<E>::commit(ck, &W.W, &W.r_W, false),
                || PCS::<E>::commit(ck, &W.E, &W.r_E, false),
            );
            let comm_W = comm_W_result?;
            let comm_E = comm_E_result?;
            U.comm_W == comm_W && U.comm_E == comm_E
        };

        if !res_eq {
            return Err(VegaError::UnSat {
                reason: "Relaxed R1CS is unsatisfiable".to_string(),
            });
        }

        if !res_comm {
            return Err(VegaError::UnSat {
                reason: "Invalid commitments".to_string(),
            });
        }

        Ok(())
    }

    /// Samples a new random `RelaxedR1CSInstance`/`RelaxedR1CSWitness` pair
    pub fn sample_random_instance_witness(
        &self,
        ck: &CommitmentKey<E>,
    ) -> Result<(RelaxedR1CSInstance<E>, RelaxedR1CSWitness<E>), VegaError> {
        // Bulk random generation: generate all random bytes at once via ChaCha CSPRNG,
        // then reduce each 64-byte chunk mod p via from_uniform (wide reduction).
        use crate::traits::PrimeFieldExt;
        let z_len = self.num_vars + self.num_io + 1;
        let total_bytes = z_len * 64;
        let mut buf = vec![0u8; total_bytes];
        crate::iroha_rng::fill_bytes(&mut buf);
        let Z: Vec<E::Scalar> = (0..z_len)
            .map(|i| E::Scalar::from_uniform(&buf[i * 64..(i + 1) * 64]))
            .collect();

        let r_W = PCS::<E>::blind(ck, self.num_vars);
        let r_E = PCS::<E>::blind(ck, self.num_cons);

        let u = Z[self.num_vars];

        // compute E <- AZ o BZ - u * CZ
        let (az, bz, cz) = self.multiply_vec(&Z)?;
        let E_vec = az
            .iter()
            .zip(bz.iter())
            .zip(cz.iter())
            .map(|((az_i, bz_i), cz_i)| *az_i * *bz_i - u * *cz_i)
            .collect::<Vec<E::Scalar>>();

        // compute commitments to W,E
        let (comm_W_res, comm_E_res) = if rayon::current_num_threads() > 1 {
            rayon::join(
                || PCS::<E>::commit(ck, &Z[..self.num_vars], &r_W, false),
                || PCS::<E>::commit(ck, &E_vec, &r_E, false),
            )
        } else {
            (
                PCS::<E>::commit(ck, &Z[..self.num_vars], &r_W, false),
                PCS::<E>::commit(ck, &E_vec, &r_E, false),
            )
        };

        Ok((
            RelaxedR1CSInstance {
                comm_W: comm_W_res?,
                comm_E: comm_E_res?,
                u,
                X: Z[self.num_vars + 1..].to_vec(),
            },
            RelaxedR1CSWitness {
                W: Z[..self.num_vars].to_vec(),
                r_W,
                E: E_vec,
                r_E,
            },
        ))
    }
}

impl<E: Engine> R1CSWitness<E> {
    /// A method to create a witness object using a vector of scalars
    pub fn new(
        ck: &CommitmentKey<E>,
        S: &R1CSShape<E>,
        W: &mut Vec<E::Scalar>,
        is_small: bool,
    ) -> Result<(R1CSWitness<E>, Commitment<E>), VegaError> {
        let r_W = PCS::<E>::blind(ck, W.len());

        // pad with zeros
        if W.len() < S.num_vars {
            W.resize(S.num_vars, E::Scalar::ZERO);
        }

        let comm_W = PCS::<E>::commit(ck, W, &r_W, is_small)?;

        let W = R1CSWitness {
            W: W.to_vec(),
            r_W,
            is_small,
        };

        Ok((W, comm_W))
    }

    /// A method to create a witness object using a vector of scalars
    pub fn new_unchecked(
        W: Vec<E::Scalar>,
        r_W: Blind<E>,
        is_small: bool,
    ) -> Result<R1CSWitness<E>, VegaError> {
        Ok(Self { W, r_W, is_small })
    }

    /// Fold multiple witnesses with a sequence of r_b values
    pub fn fold_multiple(
        r_bs: &[E::Scalar],
        Ws: &[R1CSWitness<E>],
    ) -> Result<R1CSWitness<E>, VegaError>
    where
        E::PCS: FoldingEngineTrait<E>,
    {
        let n = Ws.len();
        if n == 0 {
            return Err(VegaError::InvalidInputLength {
                reason: "fold_multiple: empty witness list".into(),
            });
        }

        let w = weights_from_r::<E::Scalar>(r_bs, n);

        if w.len() != n {
            return Err(VegaError::InvalidInputLength {
                reason: "fold_multiple: weights length mismatch".into(),
            });
        }

        let dim = Ws[0].W.len();

        if !Ws.iter().all(|z| z.W.len() == dim) {
            return Err(VegaError::InvalidInputLength {
                reason: "fold_multiple: all W vectors must have the same length".into(),
            });
        }

        let mut acc_W = vec![E::Scalar::ZERO; dim];
        let tile = 4096; // process 4096 elements at a time

        // Check if all witnesses have small values (fit in u64).
        // For SHA256 circuits, witnesses are mostly boolean (0/1),
        // so we can skip the expensive field multiplication for those.
        let all_small = Ws.iter().all(|wz| wz.is_small);

        acc_W
      .par_chunks_mut(tile)
      .enumerate()
      .for_each(|(block_idx, acc_blk)| {
        let start = block_idx * tile;
        let end = start + acc_blk.len(); // last block may be < tile

        if all_small {
          // Fast path: witness values fit in u64.
          // Skip zero values and use addition for unit values.
          let zero = E::Scalar::ZERO;
          let one = E::Scalar::ONE;
          for (i, &wi) in w.iter().enumerate() {
            let row_slice = &Ws[i].W[start..end];
            for (a, x) in acc_blk.iter_mut().zip(row_slice.iter()) {
              if *x == zero {
                // skip
              } else if *x == one {
                *a += wi;
              } else {
                *a += wi * *x;
              }
            }
          }
        } else {
          // General path: delayed reduction with element-major access.
          // Uses a single accumulator (in registers) per element instead of a large buffer.
          type Acc<S> = <S as DelayedReduction<S>>::Accumulator;
          for (j, acc_blk_j) in acc_blk.iter_mut().enumerate() {
            let mut acc = Acc::<E::Scalar>::default();
            for (i, &wi) in w.iter().enumerate() {
              <E::Scalar as DelayedReduction<E::Scalar>>::unreduced_multiply_accumulate(
                &mut acc,
                &wi,
                &Ws[i].W[start + j],
              );
            }
            *acc_blk_j = <E::Scalar as DelayedReduction<E::Scalar>>::reduce(&acc);
          }
        }
      });

        let acc_r = <E::PCS as FoldingEngineTrait<E>>::fold_blinds(
            &Ws.iter().map(|wz| wz.r_W.clone()).collect::<Vec<_>>(),
            &w,
        )?;

        Ok(R1CSWitness::<E> {
            W: acc_W,
            r_W: acc_r,
            is_small: false,
        })
    }
}

impl<E: Engine> R1CSInstance<E> {
    /// A method to create an instance object using constituent elements
    pub fn new(
        S: &R1CSShape<E>,
        comm_W: &Commitment<E>,
        X: &[E::Scalar],
    ) -> Result<R1CSInstance<E>, VegaError> {
        if S.num_io != X.len() {
            Err(VegaError::InvalidInputLength {
                reason: format!(
                    "R1CS instance: Expected {} elements in X, got {}",
                    S.num_io,
                    X.len()
                ),
            })
        } else {
            Ok(R1CSInstance {
                comm_W: comm_W.clone(),
                X: X.to_owned(),
            })
        }
    }

    /// A method to create an instance object using constituent elements
    pub fn new_unchecked(
        comm_W: Commitment<E>,
        X: Vec<E::Scalar>,
    ) -> Result<R1CSInstance<E>, VegaError> {
        Ok(R1CSInstance { comm_W, X })
    }

    /// Fold multiple instances with a sequence of r_b values
    pub fn fold_multiple(
        r_bs: &[E::Scalar],
        Us: &[R1CSInstance<E>],
    ) -> Result<R1CSInstance<E>, VegaError>
    where
        E::PCS: FoldingEngineTrait<E>,
    {
        let n = Us.len();
        if n == 0 {
            return Err(VegaError::InvalidInputLength {
                reason: "fold_multiple: empty instance list".into(),
            });
        }

        let w = weights_from_r::<E::Scalar>(r_bs, n);

        if w.len() != n {
            return Err(VegaError::InvalidInputLength {
                reason: "fold_multiple: weights length mismatch".into(),
            });
        }

        let d = Us[0].X.len();

        if !Us.iter().all(|U| U.X.len() == d) {
            return Err(VegaError::InvalidInputLength {
                reason: "fold_multiple: all X vectors must have the same length".into(),
            });
        }

        // X
        let mut X_acc = vec![E::Scalar::ZERO; d];
        for (i, Ui) in Us.iter().enumerate() {
            let wi = w[i];
            for (j, Uij) in Ui.X.iter().enumerate() {
                X_acc[j] += wi * Uij;
            }
        }

        // commitment (group lin. comb)
        let comm_acc = <E::PCS as FoldingEngineTrait<E>>::fold_commitments(
            &Us.iter().map(|U| U.comm_W.clone()).collect::<Vec<_>>(),
            &w,
        )?;

        Ok(R1CSInstance::<E> {
            X: X_acc,
            comm_W: comm_acc,
        })
    }
}

impl<E: Engine> TranscriptReprTrait<E::GE> for R1CSInstance<E> {
    fn to_transcript_bytes(&self) -> Vec<u8> {
        [
            self.comm_W.to_transcript_bytes(),
            self.X.as_slice().to_transcript_bytes(),
        ]
        .concat()
    }
}

///
////////////////// Split R1CS Types //////////////////
///
/// A type that holds a split R1CS shape
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitR1CSShape<E: Engine> {
    pub(crate) num_cons: usize,

    pub(crate) num_cons_unpadded: usize, // number of constraints before padding
    pub(crate) num_shared_unpadded: usize, // shared variables before padding
    pub(crate) num_precommitted_unpadded: usize, // precommitted variables before padding
    pub(crate) num_rest_unpadded: usize, // rest of the variables before padding

    pub(crate) num_shared: usize,       // shared variables
    pub(crate) num_precommitted: usize, // precommitted variables
    pub(crate) num_rest: usize,         // rest of the variables
    pub(crate) num_public: usize,       // number of public variables
    pub(crate) num_challenges: usize,   // number of public challenges
    pub(crate) A: SparseMatrix<E::Scalar>,
    pub(crate) B: SparseMatrix<E::Scalar>,
    pub(crate) C: SparseMatrix<E::Scalar>,
    #[serde(skip, default = "OnceCell::new")]
    pub(crate) digest: OnceCell<E::Scalar>,
    #[serde(skip, default = "OnceCell::new")]
    pub(crate) precomp_A: OnceCell<PrecomputedSparseMatrix<E::Scalar>>,
    #[serde(skip, default = "OnceCell::new")]
    pub(crate) precomp_B: OnceCell<PrecomputedSparseMatrix<E::Scalar>>,
    #[serde(skip, default = "OnceCell::new")]
    pub(crate) precomp_C: OnceCell<PrecomputedSparseMatrix<E::Scalar>>,
    #[serde(skip, default = "OnceCell::new")]
    pub(crate) filtered_A: OnceCell<FilteredSpmv<E::Scalar>>,
    #[serde(skip, default = "OnceCell::new")]
    pub(crate) filtered_B: OnceCell<FilteredSpmv<E::Scalar>>,
    #[serde(skip, default = "OnceCell::new")]
    pub(crate) filtered_C: OnceCell<FilteredSpmv<E::Scalar>>,
}

impl<E: Engine> crate::digest::Digestible for SplitR1CSShape<E> {
    fn write_bytes<W: Sized + std::io::Write>(&self, w: &mut W) -> Result<(), std::io::Error> {
        // Write dimension fields as fixed-size le bytes (deterministic, fast)
        w.write_all(&(self.num_cons as u64).to_le_bytes())?;
        w.write_all(&(self.num_cons_unpadded as u64).to_le_bytes())?;
        w.write_all(&(self.num_shared_unpadded as u64).to_le_bytes())?;
        w.write_all(&(self.num_precommitted_unpadded as u64).to_le_bytes())?;
        w.write_all(&(self.num_rest_unpadded as u64).to_le_bytes())?;
        w.write_all(&(self.num_shared as u64).to_le_bytes())?;
        w.write_all(&(self.num_precommitted as u64).to_le_bytes())?;
        w.write_all(&(self.num_rest as u64).to_le_bytes())?;
        w.write_all(&(self.num_public as u64).to_le_bytes())?;
        w.write_all(&(self.num_challenges as u64).to_le_bytes())?;
        // Write matrices as raw bytes (skip bincode per-element overhead)
        self.A.write_digest_bytes(w)?;
        self.B.write_digest_bytes(w)?;
        self.C.write_digest_bytes(w)?;
        Ok(())
    }
}

/// A type that holds a split R1CS instance
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct SplitR1CSInstance<E: Engine> {
    pub(crate) comm_W_shared: Option<Commitment<E>>,
    pub(crate) comm_W_precommitted: Option<Commitment<E>>,
    pub(crate) comm_W_rest: Commitment<E>,

    pub(crate) public_values: Vec<E::Scalar>,
    pub(crate) challenges: Vec<E::Scalar>,
}

impl<E: Engine> SplitR1CSShape<E> {
    /// Create an object of type `R1CSShape` from the explicitly specified R1CS matrices
    pub fn new(
        num_cons: usize,
        num_shared: usize,
        num_precommitted: usize,
        num_rest: usize,
        num_public: usize,
        num_challenges: usize,
        A: SparseMatrix<E::Scalar>,
        B: SparseMatrix<E::Scalar>,
        C: SparseMatrix<E::Scalar>,
    ) -> Result<SplitR1CSShape<E>, VegaError> {
        let width = DEFAULT_COMMITMENT_WIDTH;

        let num_rows = num_cons;
        let num_cols = num_shared + num_precommitted + num_rest + 1 + num_public + num_challenges; // +1 for the constant term

        is_sparse_matrix_valid::<E>(num_rows, num_cols, &A)?;
        is_sparse_matrix_valid::<E>(num_rows, num_cols, &B)?;
        is_sparse_matrix_valid::<E>(num_rows, num_cols, &C)?;

        // We need to pad num_shared, num_precommitted, and num_rest. We need each of them to be a multiple of num_cols.
        let num_shared_padded = pad_to_width(width, num_shared);
        let num_precommitted_padded = pad_to_width(width, num_precommitted);
        let mut num_rest_padded = pad_to_width(width, num_rest);

        // We need to make sure num_vars_padded >= num_public + num_challenges + 1 (for the constant term).
        let num_vars_padded = num_shared_padded + num_precommitted_padded + num_rest_padded;
        if num_vars_padded < num_public + num_challenges + 1 {
            // If not, we need to pad the rest to make it at least num_public + num_challenges + 1.
            num_rest_padded = max(num_public + num_challenges + 1, num_vars_padded)
                - (num_shared_padded + num_precommitted_padded);
        }

        // We need to make sure num_shared_padded + num_precommitted_padded + num_rest_padded is a power of two.
        let num_vars_padded = num_shared_padded + num_precommitted_padded + num_rest_padded;
        if num_vars_padded.next_power_of_two() != num_vars_padded {
            // If not, we need to pad the rest to the next power of two.
            num_rest_padded =
                num_vars_padded.next_power_of_two() - (num_shared_padded + num_precommitted_padded);
        }

        let num_vars = num_shared + num_precommitted + num_rest;
        let num_vars_padded = num_shared_padded + num_precommitted_padded + num_rest_padded;
        let num_cons_padded = num_cons.next_power_of_two();

        let apply_pad = |mut M: SparseMatrix<E::Scalar>| -> SparseMatrix<E::Scalar> {
            M.indices.par_iter_mut().for_each(|c| {
                if *c >= num_shared && *c < num_shared + num_precommitted {
                    // precommitted variables
                    *c += num_shared_padded - num_shared;
                } else if *c >= num_shared + num_precommitted && *c < num_vars {
                    // rest of the variables
                    *c +=
                        num_shared_padded + num_precommitted_padded - num_shared - num_precommitted;
                } else if *c >= num_vars {
                    // public and challenge variables
                    *c += num_vars_padded - num_vars;
                }
            });

            M.cols += num_vars_padded - num_vars;

            let ex = {
                let nnz = if M.indptr.is_empty() {
                    0
                } else {
                    M.indptr[M.indptr.len() - 1]
                };
                vec![nnz; num_cons_padded - num_cons]
            };
            M.indptr.extend(ex);
            M
        };

        let A_padded = apply_pad(A);
        let B_padded = apply_pad(B);
        let C_padded = apply_pad(C);

        Ok(Self {
            num_cons: num_cons_padded,
            num_shared: num_shared_padded,
            num_precommitted: num_precommitted_padded,
            num_rest: num_rest_padded,

            num_cons_unpadded: num_cons,
            num_shared_unpadded: num_shared,
            num_precommitted_unpadded: num_precommitted,
            num_rest_unpadded: num_rest,

            num_public,
            num_challenges,
            A: A_padded,
            B: B_padded,
            C: C_padded,
            digest: OnceCell::new(),
            precomp_A: OnceCell::new(),
            precomp_B: OnceCell::new(),
            precomp_C: OnceCell::new(),
            filtered_A: OnceCell::new(),
            filtered_B: OnceCell::new(),
            filtered_C: OnceCell::new(),
        })
    }

    pub fn equalize(S_A: &mut Self, S_B: &mut Self) {
        let orig_cons_a = S_A.num_cons;
        let orig_cons_b = S_B.num_cons;

        let num_cons_padded = max(S_A.num_cons, S_B.num_cons);
        let num_vars_padded = max(
            S_A.num_shared + S_A.num_precommitted + S_A.num_rest,
            S_B.num_shared + S_B.num_precommitted + S_B.num_rest,
        );

        S_A.num_cons = num_cons_padded;
        S_B.num_cons = num_cons_padded;

        let move_public_vars =
            |M: &mut SparseMatrix<E::Scalar>, num_cons: usize, num_vars: usize| {
                M.indices.par_iter_mut().for_each(|c| {
                    if *c >= num_vars {
                        // public and challenge variables
                        *c += num_vars_padded - num_vars;
                    }
                });

                M.cols += num_vars_padded - num_vars;

                let ex = {
                    let nnz = if M.indptr.is_empty() {
                        0
                    } else {
                        M.indptr[M.indptr.len() - 1]
                    };
                    vec![nnz; num_cons_padded - num_cons]
                };
                M.indptr.extend(ex);
            };

        // Grow variables (if needed) and pad rows using original constraint counts
        if S_A.num_shared + S_A.num_precommitted + S_A.num_rest != num_vars_padded {
            let num_vars = S_A.num_shared + S_A.num_precommitted + S_A.num_rest;
            S_A.num_rest = num_vars_padded - (S_A.num_shared + S_A.num_precommitted);
            move_public_vars(&mut S_A.A, orig_cons_a, num_vars);
            move_public_vars(&mut S_A.B, orig_cons_a, num_vars);
            move_public_vars(&mut S_A.C, orig_cons_a, num_vars);
        } else {
            // No var growth; still ensure row padding happens
            let num_vars = S_A.num_shared + S_A.num_precommitted + S_A.num_rest;
            move_public_vars(&mut S_A.A, orig_cons_a, num_vars);
            move_public_vars(&mut S_A.B, orig_cons_a, num_vars);
            move_public_vars(&mut S_A.C, orig_cons_a, num_vars);
        }

        if S_B.num_shared + S_B.num_precommitted + S_B.num_rest != num_vars_padded {
            let num_vars = S_B.num_shared + S_B.num_precommitted + S_B.num_rest;
            S_B.num_rest = num_vars_padded - (S_B.num_shared + S_B.num_precommitted);
            move_public_vars(&mut S_B.A, orig_cons_b, num_vars);
            move_public_vars(&mut S_B.B, orig_cons_b, num_vars);
            move_public_vars(&mut S_B.C, orig_cons_b, num_vars);
        } else {
            let num_vars = S_B.num_shared + S_B.num_precommitted + S_B.num_rest;
            move_public_vars(&mut S_B.A, orig_cons_b, num_vars);
            move_public_vars(&mut S_B.B, orig_cons_b, num_vars);
            move_public_vars(&mut S_B.C, orig_cons_b, num_vars);
        }
    }

    pub fn to_regular_shape(&self) -> R1CSShape<E> {
        R1CSShape {
            num_cons: self.num_cons,
            num_vars: self.num_shared + self.num_precommitted + self.num_rest,
            num_io: self.num_public + self.num_challenges,
            A: self.A.clone(),
            B: self.B.clone(),
            C: self.C.clone(),
            digest: OnceCell::new(),
        }
    }

    /// Returns statistics about the shape of the R1CS matrices.
    ///
    /// This function returns an array of 10 elements, where each element represents a specific
    /// statistic about the R1CS matrices. The elements are as follows:
    /// - `num_cons_unpadded`: The number of constraints in the unpadded R1CS matrix.
    /// - `num_shared_unpadded`: The number of shared variables in the unpadded R1CS matrix.
    /// - `num_precommitted_unpadded`: The number of precommitted variables in the unpadded R1CS matrix.
    /// - `num_rest_unpadded`: The number of remaining variables in the unpadded R1CS matrix.
    /// - `num_cons`: The number of constraints in the padded R1CS matrix.
    /// - `num_shared`: The number of shared variables in the padded R1CS matrix.
    /// - `num_precommitted`: The number of precommitted variables in the padded R1CS matrix.
    /// - `num_rest`: The number of remaining variables in the padded R1CS matrix.
    /// - `num_public`: The number of public inputs/outputs in the R1CS matrix.
    /// - `num_challenges`: The number of challenges in the R1CS matrix.
    ///
    /// The terms "unpadded" and "padded" refer to the state of the R1CS matrices:
    /// - "Unpadded" values represent the original dimensions of the matrices before any padding
    ///   is applied to meet alignment or size requirements.
    /// - "Padded" values represent the dimensions of the matrices after padding has been applied.
    ///
    pub fn sizes(&self) -> [usize; 10] {
        [
            self.num_cons_unpadded,
            self.num_shared_unpadded,
            self.num_precommitted_unpadded,
            self.num_rest_unpadded,
            self.num_cons,
            self.num_shared,
            self.num_precommitted,
            self.num_rest,
            self.num_public,
            self.num_challenges,
        ]
    }

    /// Generates public parameters for a Rank-1 Constraint System (R1CS).
    ///
    /// This function takes into consideration the shape of the R1CS matrices
    ///
    /// # Arguments
    ///
    /// * `S`: The shape of the R1CS matrices.
    ///
    pub fn commitment_key(
        shapes: &[&SplitR1CSShape<E>],
    ) -> Result<(CommitmentKey<E>, VerifierKey<E>), VegaError> {
        let max = shapes
            .iter()
            .map(|s| s.num_shared + s.num_precommitted + s.num_rest)
            .max()
            .ok_or(VegaError::InvalidInputLength {
                reason: "commitment_key: unable to find max number of variables".to_string(),
            })?;

        Ok(E::PCS::setup(b"ck", max, DEFAULT_COMMITMENT_WIDTH))
    }

    /// Lazily build precomputed matrices for fast SpMV.
    fn ensure_precomputed(&self) {
        self.precomp_A
            .get_or_init(|| PrecomputedSparseMatrix::from_sparse(&self.A));
        self.precomp_B
            .get_or_init(|| PrecomputedSparseMatrix::from_sparse(&self.B));
        self.precomp_C
            .get_or_init(|| PrecomputedSparseMatrix::from_sparse(&self.C));
    }

    /// Eagerly build precomputed matrices during setup.
    pub fn precompute(&self) {
        self.ensure_precomputed();
        // Also build filtered entries for incremental SpMV
        let col_min = self.num_shared + self.num_precommitted;
        let nr = self.num_cons_unpadded;
        self.filtered_A
            .get_or_init(|| self.precomp_A.get().unwrap().build_filtered(col_min, nr));
        self.filtered_B
            .get_or_init(|| self.precomp_B.get().unwrap().build_filtered(col_min, nr));
        self.filtered_C
            .get_or_init(|| self.precomp_C.get().unwrap().build_filtered(col_min, nr));
    }

    pub fn multiply_vec(
        &self,
        z: &[E::Scalar],
    ) -> Result<(Vec<E::Scalar>, Vec<E::Scalar>, Vec<E::Scalar>), VegaError> {
        if z.len()
            != self.num_public
                + self.num_challenges
                + 1
                + self.num_shared
                + self.num_precommitted
                + self.num_rest
        {
            return Err(VegaError::InvalidWitnessLength);
        }

        self.ensure_precomputed();
        let pa = self.precomp_A.get().unwrap();
        let pb = self.precomp_B.get().unwrap();
        let pc = self.precomp_C.get().unwrap();

        if rayon::current_num_threads() <= 1 {
            let az = pa.multiply_vec(z);
            let bz = pb.multiply_vec(z);
            let cz = pc.multiply_vec(z);
            Ok((az, bz, cz))
        } else {
            let (az, (bz, cz)) = rayon::join(
                || pa.multiply_vec(z),
                || rayon::join(|| pb.multiply_vec(z), || pc.multiply_vec(z)),
            );
            Ok((az, bz, cz))
        }
    }

    /// Compute partial matrix-vector product for shared + precommitted columns.
    /// Returns (Az_cached, Bz_cached, Cz_cached) covering all deterministic witness columns.
    /// The input slice must have length `num_shared + num_precommitted` (the full cached portion of W).
    pub fn multiply_vec_precommitted(
        &self,
        z_cached: &[E::Scalar],
    ) -> Result<(Vec<E::Scalar>, Vec<E::Scalar>, Vec<E::Scalar>), VegaError> {
        let cached_len = self.num_shared + self.num_precommitted;
        assert_eq!(
            z_cached.len(),
            cached_len,
            "multiply_vec_precommitted expects shared + precommitted ({cached_len}), got {}",
            z_cached.len()
        );
        // Build a full-size z vector with shared + precommitted values at correct positions
        let total_len = cached_len + self.num_rest + 1 + self.num_public + self.num_challenges;
        let mut z_full = vec![E::Scalar::ZERO; total_len];
        z_full[..cached_len].copy_from_slice(z_cached);
        self.multiply_vec(&z_full)
    }

    /// Batched matrix-vector multiply: compute (Az, Bz, Cz) for multiple z vectors in a single pass.
    /// Traverses each sparse matrix (A, B, C) once instead of n_vecs times.
    pub fn multiply_vec_batched(
        &self,
        zs: &[Vec<E::Scalar>],
    ) -> Result<Vec<(Vec<E::Scalar>, Vec<E::Scalar>, Vec<E::Scalar>)>, VegaError> {
        let expected_len = self.num_public
            + self.num_challenges
            + 1
            + self.num_shared
            + self.num_precommitted
            + self.num_rest;
        for z in zs {
            if z.len() != expected_len {
                return Err(VegaError::InvalidWitnessLength);
            }
        }

        self.ensure_precomputed();
        let pa = self.precomp_A.get().unwrap();
        let pb = self.precomp_B.get().unwrap();
        let pc = self.precomp_C.get().unwrap();

        let z_refs: Vec<&[E::Scalar]> = zs.iter().map(|z| z.as_slice()).collect();
        let a_results = pa.multiply_vec_batched(&z_refs);
        let b_results = pb.multiply_vec_batched(&z_refs);
        let c_results = pc.multiply_vec_batched(&z_refs);

        Ok(a_results
            .into_iter()
            .zip(b_results)
            .zip(c_results)
            .map(|((a, b), c)| (a, b, c))
            .collect())
    }

    /// Like multiply_vec_incremental but writes into pre-allocated buffers.
    /// Reuses the allocation if capacity is sufficient (avoids mmap + page faults).
    pub fn multiply_vec_incremental_into(
        &self,
        z: &[E::Scalar],
        cached_az: &[E::Scalar],
        cached_bz: &[E::Scalar],
        cached_cz: &[E::Scalar],
        az: &mut Vec<E::Scalar>,
        bz: &mut Vec<E::Scalar>,
        cz: &mut Vec<E::Scalar>,
    ) -> Result<(), VegaError> {
        // Use filtered entries (built during precompute/setup)
        let col_min = self.num_shared + self.num_precommitted;
        let nr = self.num_cons_unpadded;
        self.ensure_precomputed();
        self.filtered_A
            .get_or_init(|| self.precomp_A.get().unwrap().build_filtered(col_min, nr));
        self.filtered_B
            .get_or_init(|| self.precomp_B.get().unwrap().build_filtered(col_min, nr));
        self.filtered_C
            .get_or_init(|| self.precomp_C.get().unwrap().build_filtered(col_min, nr));

        let fa = self.filtered_A.get().unwrap();
        let fb = self.filtered_B.get().unwrap();
        let fc = self.filtered_C.get().unwrap();

        // Reuse allocation: clear and copy from cached
        az.clear();
        az.extend_from_slice(cached_az);
        bz.clear();
        bz.extend_from_slice(cached_bz);
        cz.clear();
        cz.extend_from_slice(cached_cz);

        fa.multiply_vec_add(z, az);
        fb.multiply_vec_add(z, bz);
        fc.multiply_vec_add(z, cz);

        Ok(())
    }

    /// Optimized MLE evaluation using PrecomputedSparseMatrix.
    /// Fuses A/B/C into a single row-major pass for better T_y cache reuse,
    /// hoists T_x\[row\] out of inner loop, and uses delayed reduction.
    pub fn evaluate_with_tables_fast(
        &self,
        T_x: &[E::Scalar],
        T_y: &[E::Scalar],
    ) -> (E::Scalar, E::Scalar, E::Scalar) {
        self.ensure_precomputed();
        let pa = self.precomp_A.get().unwrap();
        let pb = self.precomp_B.get().unwrap();
        let pc = self.precomp_C.get().unwrap();
        evaluate_three_matrices_fused(pa, pb, pc, T_x, T_y)
    }

    /// Merged bind_row_vars + prepare_poly_ABC.
    ///
    /// Computes poly_ABC[j] = sum_i (A[i,j] + r*B[i,j] + r^2*C[i,j]) * rx[i]
    /// directly, avoiding the intermediate evals_A, evals_B, evals_C allocations.
    ///
    /// Compact variant: output length = num_vars + num_extra (not 2*num_vars).
    /// Used by SpartanZK which handles the first inner sumcheck round manually.
    pub(crate) fn bind_and_prepare_poly_ABC(
        &self,
        rx: &[E::Scalar],
        r: &E::Scalar,
    ) -> Vec<E::Scalar> {
        let num_vars = self.num_shared + self.num_precommitted + self.num_rest;
        let num_extra = 1 + self.num_public + self.num_challenges;
        let out_len = num_vars + num_extra;
        self.bind_and_prepare_poly_ABC_inner(rx, r, out_len)
    }

    /// Full-size variant: output length = 2*num_vars (zero-padded).
    /// Used by NeutronNova which passes poly_ABC directly to batched sumcheck.
    /// Returns (poly_vec, lo_eff, hi_eff) where lo_eff/hi_eff are the non-zero
    /// prefix lengths of the lo and hi halves respectively.
    pub(crate) fn bind_and_prepare_poly_ABC_full(
        &self,
        rx: &[E::Scalar],
        r: &E::Scalar,
    ) -> (Vec<E::Scalar>, usize, usize) {
        let num_vars = self.num_shared + self.num_precommitted + self.num_rest;
        let vec = self.bind_and_prepare_poly_ABC_inner(rx, r, num_vars * 2);
        // Variable layout in z: [shared|precommitted|rest|u|X|zeros]
        // Sections start at padded boundaries: shared at 0, precommitted at num_shared,
        // rest at num_shared+num_precommitted. Only unpadded vars within each section
        // appear in constraints.
        let lo_eff = if self.num_rest_unpadded > 0 {
            self.num_shared + self.num_precommitted + self.num_rest_unpadded
        } else if self.num_precommitted_unpadded > 0 {
            self.num_shared + self.num_precommitted_unpadded
        } else {
            self.num_shared_unpadded
        };
        let hi_eff = 1 + self.num_public + self.num_challenges; // u + public inputs + challenges
        (vec, lo_eff, hi_eff)
    }

    fn bind_and_prepare_poly_ABC_inner(
        &self,
        rx: &[E::Scalar],
        r: &E::Scalar,
        out_len: usize,
    ) -> Vec<E::Scalar> {
        assert_eq!(rx.len(), self.num_cons);

        self.ensure_precomputed();
        let pa = self.precomp_A.get().unwrap();
        let pb = self.precomp_B.get().unwrap();
        let pc = self.precomp_C.get().unwrap();

        let r2 = *r * *r;
        let num_rows = self.num_cons_unpadded;

        if rayon::current_num_threads() <= 1 || num_rows <= 4096 {
            // Sequential path: single accumulator
            let mut poly_abc = vec![E::Scalar::ZERO; out_len];
            Self::accumulate_rows(pa, pb, pc, rx, r, &r2, 0, num_rows, &mut poly_abc);
            poly_abc
        } else {
            // Parallel path: per-thread accumulators + reduce
            use rayon::prelude::*;
            let num_threads = rayon::current_num_threads();
            let chunk_size = num_rows.div_ceil(num_threads);

            (0..num_threads)
                .into_par_iter()
                .map(|t| {
                    let start = t * chunk_size;
                    let end = (start + chunk_size).min(num_rows);
                    if start >= end {
                        return vec![E::Scalar::ZERO; out_len];
                    }
                    let mut local = vec![E::Scalar::ZERO; out_len];
                    Self::accumulate_rows(pa, pb, pc, rx, r, &r2, start, end, &mut local);
                    local
                })
                .reduce(
                    || vec![E::Scalar::ZERO; out_len],
                    |mut a, b| {
                        for (x, y) in a.iter_mut().zip(b.iter()) {
                            *x += *y;
                        }
                        a
                    },
                )
        }
    }

    #[inline(never)]
    fn accumulate_rows(
        pa: &PrecomputedSparseMatrix<E::Scalar>,
        pb: &PrecomputedSparseMatrix<E::Scalar>,
        pc: &PrecomputedSparseMatrix<E::Scalar>,
        rx: &[E::Scalar],
        r: &E::Scalar,
        r2: &E::Scalar,
        start_row: usize,
        end_row: usize,
        poly_abc: &mut [E::Scalar],
    ) {
        #[allow(clippy::needless_range_loop)]
        for row in start_row..end_row {
            let rx_row = rx[row];
            let r_rx_row = rx_row * *r;
            let r2_rx_row = rx_row * *r2;

            // A contributions (coefficient = rx[row])
            let (start, end) = pa.range_unit_pos(row);
            for i in start..end {
                poly_abc[pa.unit_pos_cols[i] as usize] += rx_row;
            }
            let (start, end) = pa.range_unit_neg(row);
            for i in start..end {
                poly_abc[pa.unit_neg_cols[i] as usize] -= rx_row;
            }
            let (start, end) = pa.range_small(row);
            for i in start..end {
                poly_abc[pa.small_cols[i] as usize] +=
                    PrecomputedSparseMatrix::small_mul(pa.small_coeffs[i], rx_row);
            }
            let (start, end) = pa.range_general(row);
            for i in start..end {
                poly_abc[pa.general_cols[i] as usize] += pa.general_vals[i] * rx_row;
            }

            // B contributions (coefficient = r * rx[row])
            let (start, end) = pb.range_unit_pos(row);
            for i in start..end {
                poly_abc[pb.unit_pos_cols[i] as usize] += r_rx_row;
            }
            let (start, end) = pb.range_unit_neg(row);
            for i in start..end {
                poly_abc[pb.unit_neg_cols[i] as usize] -= r_rx_row;
            }
            let (start, end) = pb.range_small(row);
            for i in start..end {
                poly_abc[pb.small_cols[i] as usize] +=
                    PrecomputedSparseMatrix::small_mul(pb.small_coeffs[i], r_rx_row);
            }
            let (start, end) = pb.range_general(row);
            for i in start..end {
                poly_abc[pb.general_cols[i] as usize] += pb.general_vals[i] * r_rx_row;
            }

            // C contributions (coefficient = r^2 * rx[row])
            let (start, end) = pc.range_unit_pos(row);
            for i in start..end {
                poly_abc[pc.unit_pos_cols[i] as usize] += r2_rx_row;
            }
            let (start, end) = pc.range_unit_neg(row);
            for i in start..end {
                poly_abc[pc.unit_neg_cols[i] as usize] -= r2_rx_row;
            }
            let (start, end) = pc.range_small(row);
            for i in start..end {
                poly_abc[pc.small_cols[i] as usize] +=
                    PrecomputedSparseMatrix::small_mul(pc.small_coeffs[i], r2_rx_row);
            }
            let (start, end) = pc.range_general(row);
            for i in start..end {
                poly_abc[pc.general_cols[i] as usize] += pc.general_vals[i] * r2_rx_row;
            }
        }
    }
}

/// A type that holds a multi-round split R1CS shape
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitMultiRoundR1CSShape<E: Engine> {
    pub(crate) num_cons: usize,
    pub(crate) num_cons_unpadded: usize, // number of constraints before padding

    pub(crate) num_rounds: usize,
    pub(crate) num_vars_per_round_unpadded: Vec<usize>, // variables per round before padding
    pub(crate) num_vars_per_round: Vec<usize>,          // variables per round after padding
    pub(crate) num_challenges_per_round: Vec<usize>,    // challenges per round
    pub(crate) num_public: usize,                       // number of public variables
    pub(crate) commitment_width: usize,                 // width for per-round commitments

    pub(crate) A: SparseMatrix<E::Scalar>,
    pub(crate) B: SparseMatrix<E::Scalar>,
    pub(crate) C: SparseMatrix<E::Scalar>,
    #[serde(skip, default = "OnceCell::new")]
    pub(crate) digest: OnceCell<E::Scalar>,
}

impl<E: Engine> SimpleDigestible for SplitMultiRoundR1CSShape<E> {}

/// A type that holds a multi-round split R1CS instance
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct SplitMultiRoundR1CSInstance<E: Engine> {
    pub(crate) comm_w_per_round: Vec<Commitment<E>>,
    pub(crate) public_values: Vec<E::Scalar>,
    pub(crate) challenges_per_round: Vec<Vec<E::Scalar>>,
}

impl<E: Engine> SplitR1CSInstance<E> {
    /// A method to create a split R1CS instance object using constituent elements
    pub fn new(
        S: &SplitR1CSShape<E>,
        comm_W_shared: Option<Commitment<E>>,
        comm_W_precommitted: Option<Commitment<E>>,
        comm_W_rest: Commitment<E>,
        public_values: Vec<E::Scalar>,
        challenges: Vec<E::Scalar>,
    ) -> Result<SplitR1CSInstance<E>, VegaError> {
        if public_values.len() != S.num_public {
            return Err(VegaError::InvalidInputLength {
                reason: format!(
                    "SplitR1CS instance: Expected {} public values, got {}",
                    S.num_public,
                    public_values.len()
                ),
            });
        }
        if challenges.len() != S.num_challenges {
            return Err(VegaError::InvalidInputLength {
                reason: format!(
                    "SplitR1CS instance: Expected {} challenges, got {}",
                    S.num_challenges,
                    challenges.len()
                ),
            });
        }

        // check if the commitments commit to the right number of variables
        if S.num_shared > 0 && comm_W_shared.is_none() {
            return Err(VegaError::InvalidCommitmentLength {
                reason: "comm_W_shared is missing".to_string(),
            });
        }
        if S.num_precommitted > 0 && comm_W_precommitted.is_none() {
            return Err(VegaError::InvalidCommitmentLength {
                reason: "comm_W_precommitted is missing".to_string(),
            });
        }
        // a commitment must be absent when its segment is empty
        if S.num_shared == 0 && comm_W_shared.is_some() {
            return Err(VegaError::InvalidCommitmentLength {
                reason: "comm_W_shared present for an empty segment".to_string(),
            });
        }
        if S.num_precommitted == 0 && comm_W_precommitted.is_some() {
            return Err(VegaError::InvalidCommitmentLength {
                reason: "comm_W_precommitted present for an empty segment".to_string(),
            });
        }

        if let Some(ref comm) = comm_W_shared {
            E::PCS::check_commitment(comm, S.num_shared, DEFAULT_COMMITMENT_WIDTH)?;
        }
        if let Some(ref comm) = comm_W_precommitted {
            E::PCS::check_commitment(comm, S.num_precommitted, DEFAULT_COMMITMENT_WIDTH)?;
        }
        E::PCS::check_commitment(&comm_W_rest, S.num_rest, DEFAULT_COMMITMENT_WIDTH)?;

        Ok(SplitR1CSInstance {
            comm_W_shared,
            comm_W_precommitted,
            comm_W_rest,
            public_values,
            challenges,
        })
    }

    pub fn validate(&self, S: &SplitR1CSShape<E>, transcript: &mut E::TE) -> Result<(), VegaError> {
        // `public_values` must have exactly `num_public` entries: the regular
        // instance places `[public_values, challenges]` into `X`.
        if self.public_values.len() != S.num_public {
            return Err(VegaError::ProofVerifyError {
                reason: format!(
                    "public_values length ({}) does not match shape num_public ({})",
                    self.public_values.len(),
                    S.num_public
                ),
            });
        }

        // A split commitment is present exactly when its segment is non-empty;
        // `to_regular_instance` folds every present commitment into the witness commitment.
        if S.num_shared > 0 {
            if let Some(comm) = &self.comm_W_shared {
                E::PCS::check_commitment(comm, S.num_shared, DEFAULT_COMMITMENT_WIDTH)?;
                transcript.absorb(b"comm_W_shared", comm);
            } else {
                return Err(VegaError::ProofVerifyError {
                    reason: "comm_W_shared is missing".to_string(),
                });
            }
        } else if self.comm_W_shared.is_some() {
            return Err(VegaError::ProofVerifyError {
                reason: "comm_W_shared present for an empty segment".to_string(),
            });
        }

        if S.num_precommitted > 0 {
            if let Some(comm) = &self.comm_W_precommitted {
                E::PCS::check_commitment(comm, S.num_precommitted, DEFAULT_COMMITMENT_WIDTH)?;
                transcript.absorb(b"comm_W_precommitted", comm);
            } else {
                return Err(VegaError::ProofVerifyError {
                    reason: "comm_W_precommitted is missing".to_string(),
                });
            }
        } else if self.comm_W_precommitted.is_some() {
            return Err(VegaError::ProofVerifyError {
                reason: "comm_W_precommitted present for an empty segment".to_string(),
            });
        }

        // obtain challenges from the transcript
        let challenges = (0..S.num_challenges)
            .map(|_| transcript.squeeze(b"challenge"))
            .collect::<Result<Vec<E::Scalar>, VegaError>>()?;

        // check that the challenges of the circuit matches the expected values
        if challenges != self.challenges {
            return Err(VegaError::ProofVerifyError {
                reason: "Challenges do not match".to_string(),
            });
        }

        E::PCS::check_commitment(&self.comm_W_rest, S.num_rest, DEFAULT_COMMITMENT_WIDTH)?;
        transcript.absorb(b"comm_W_rest", &self.comm_W_rest);

        Ok(())
    }

    pub fn to_regular_instance(&self) -> Result<R1CSInstance<E>, VegaError> {
        let partial_comms = [
            self.comm_W_shared.clone(),
            self.comm_W_precommitted.clone(),
            Some(self.comm_W_rest.clone()),
        ]
        .iter()
        .filter_map(|comm| comm.clone())
        .collect::<Vec<Commitment<E>>>();
        let comm_W = PCS::<E>::combine_commitments(&partial_comms)?;

        Ok(R1CSInstance {
            comm_W,
            X: [self.public_values.clone(), self.challenges.clone()].concat(),
        })
    }
}

impl<E: Engine> SplitMultiRoundR1CSShape<E> {
    /// Create an object of type `SplitMultiRoundR1CSShape` from the explicitly specified R1CS matrices
    pub fn new(
        width: usize,
        num_cons: usize,
        num_vars_per_round: Vec<usize>,
        num_challenges_per_round: Vec<usize>,
        num_public: usize,
        A: SparseMatrix<E::Scalar>,
        B: SparseMatrix<E::Scalar>,
        C: SparseMatrix<E::Scalar>,
    ) -> Result<SplitMultiRoundR1CSShape<E>, VegaError> {
        let num_rounds = num_vars_per_round.len();
        if width == 0 || !width.is_power_of_two() {
            return Err(VegaError::InvalidInputLength {
                reason: format!(
                    "SplitMultiRoundR1CSShape: width must be a non-zero power of two, got {}",
                    width
                ),
            });
        }
        if num_challenges_per_round.len() != num_rounds {
            return Err(VegaError::InvalidInputLength {
                reason: format!(
                    "SplitMultiRoundR1CSShape: Expected {} challenges per round, got {}",
                    num_rounds,
                    num_challenges_per_round.len()
                ),
            });
        }

        let total_vars: usize = num_vars_per_round.iter().sum();
        let total_challenges: usize = num_challenges_per_round.iter().sum();
        let num_rows = num_cons;
        let num_cols = total_vars + 1 + num_public + total_challenges; // +1 for the constant term

        is_sparse_matrix_valid::<E>(num_rows, num_cols, &A)?;
        is_sparse_matrix_valid::<E>(num_rows, num_cols, &B)?;
        is_sparse_matrix_valid::<E>(num_rows, num_cols, &C)?;

        // Pad each round's variables to be a multiple of width
        let num_vars_per_round_padded: Vec<usize> = num_vars_per_round
            .iter()
            .map(|&n| pad_to_width(width, n))
            .collect();

        let total_vars_padded: usize = num_vars_per_round_padded.iter().sum();
        let num_cons_padded = num_cons.next_power_of_two();

        // Apply padding transformation to matrices
        let apply_pad = |mut m: SparseMatrix<E::Scalar>| -> SparseMatrix<E::Scalar> {
            m.indices.par_iter_mut().for_each(|c| {
                // Find which round this variable belongs to and apply appropriate offset
                let mut current_offset = 0;
                let mut current_padded_offset = 0;

                for round in 0..num_rounds {
                    if *c >= current_offset && *c < current_offset + num_vars_per_round[round] {
                        // Variable belongs to this round, apply the padded offset
                        *c = current_padded_offset + (*c - current_offset);
                        return;
                    }
                    current_offset += num_vars_per_round[round];
                    current_padded_offset += num_vars_per_round_padded[round];
                }

                // If we get here, it's a public/challenge variable, apply total padding offset
                if *c >= total_vars {
                    *c += total_vars_padded - total_vars;
                }
            });

            m.cols += total_vars_padded - total_vars;

            let ex = {
                let nnz = if m.indptr.is_empty() {
                    0
                } else {
                    m.indptr[m.indptr.len() - 1]
                };
                vec![nnz; num_cons_padded - num_cons]
            };
            m.indptr.extend(ex);
            m
        };

        let A_padded = apply_pad(A);
        let B_padded = apply_pad(B);
        let C_padded = apply_pad(C);

        Ok(Self {
            num_cons: num_cons_padded,
            num_cons_unpadded: num_cons,
            num_rounds,
            num_vars_per_round_unpadded: num_vars_per_round,
            num_vars_per_round: num_vars_per_round_padded,
            num_challenges_per_round,
            num_public,
            commitment_width: width,
            A: A_padded,
            B: B_padded,
            C: C_padded,
            digest: OnceCell::new(),
        })
    }

    pub fn to_regular_shape(&self) -> R1CSShape<E> {
        let total_vars: usize = self.num_vars_per_round.iter().sum();
        let total_challenges: usize = self.num_challenges_per_round.iter().sum();

        R1CSShape {
            num_cons: self.num_cons,
            num_vars: total_vars,
            num_io: total_challenges + self.num_public,
            A: self.A.clone(),
            B: self.B.clone(),
            C: self.C.clone(),
            digest: OnceCell::new(),
        }
    }

    /// Returns statistics about the shape of the multi-round R1CS matrices
    pub fn sizes(&self) -> (usize, Vec<usize>, Vec<usize>, Vec<usize>, usize) {
        (
            self.num_cons_unpadded,
            self.num_vars_per_round_unpadded.clone(),
            self.num_vars_per_round.clone(),
            self.num_challenges_per_round.clone(),
            self.num_public,
        )
    }

    /// Generates public parameters for a multi-round R1CS
    pub fn commitment_key(&self) -> (CommitmentKey<E>, VerifierKey<E>) {
        let total_vars: usize = self.num_vars_per_round.iter().sum();
        E::PCS::setup(b"ck", total_vars, self.commitment_width)
    }

    pub fn multiply_vec(
        &self,
        z: &[E::Scalar],
    ) -> Result<(Vec<E::Scalar>, Vec<E::Scalar>, Vec<E::Scalar>), VegaError> {
        let total_vars: usize = self.num_vars_per_round.iter().sum();
        let total_challenges: usize = self.num_challenges_per_round.iter().sum();

        if z.len() != self.num_public + total_challenges + 1 + total_vars {
            return Err(VegaError::InvalidWitnessLength);
        }

        let (az, (bz, cz)) = rayon::join(
            || self.A.multiply_vec(z),
            || rayon::join(|| self.B.multiply_vec(z), || self.C.multiply_vec(z)),
        );

        Ok((az?, bz?, cz?))
    }
}

impl<E: Engine> SplitMultiRoundR1CSInstance<E> {
    /// A method to create a multi-round split R1CS instance object using constituent elements
    pub fn new(
        s: &SplitMultiRoundR1CSShape<E>,
        comm_w_per_round: Vec<Commitment<E>>,
        public_values: Vec<E::Scalar>,
        challenges_per_round: Vec<Vec<E::Scalar>>,
    ) -> Result<SplitMultiRoundR1CSInstance<E>, VegaError> {
        if public_values.len() != s.num_public {
            return Err(VegaError::InvalidInputLength {
                reason: format!(
                    "SplitMultiRoundR1CS instance: Expected {} public values, got {}",
                    s.num_public,
                    public_values.len()
                ),
            });
        }
        if challenges_per_round.len() != s.num_rounds {
            return Err(VegaError::InvalidInputLength {
                reason: format!(
                    "SplitMultiRoundR1CS instance: Expected {} rounds, got {}",
                    s.num_rounds,
                    challenges_per_round.len()
                ),
            });
        }
        if comm_w_per_round.len() != s.num_rounds {
            return Err(VegaError::InvalidInputLength {
                reason: format!(
                    "SplitMultiRoundR1CS instance: Expected {} rounds, got {}",
                    s.num_rounds,
                    comm_w_per_round.len()
                ),
            });
        }

        // Validate challenges per round
        for (round, challenges) in challenges_per_round.iter().enumerate() {
            if challenges.len() != s.num_challenges_per_round[round] {
                return Err(VegaError::InvalidInputLength {
                    reason: format!(
                        "SplitMultiRoundR1CS instance: Expected {} challenges in round {}, got {}",
                        s.num_challenges_per_round[round],
                        round,
                        challenges.len()
                    ),
                });
            }
        }

        // Validate commitments per round
        for (round, comm) in comm_w_per_round.iter().enumerate() {
            E::PCS::check_commitment(comm, s.num_vars_per_round[round], s.commitment_width)?;
        }

        Ok(SplitMultiRoundR1CSInstance {
            comm_w_per_round,
            public_values,
            challenges_per_round,
        })
    }

    pub fn validate(
        &self,
        s: &SplitMultiRoundR1CSShape<E>,
        transcript: &mut E::TE,
    ) -> Result<(), VegaError> {
        // The regular instance folds every per-round commitment and flattens every
        // per-round challenge into `X`, so the instance must match the shape exactly.
        if self.public_values.len() != s.num_public {
            return Err(VegaError::ProofVerifyError {
                reason: format!(
                    "public_values length ({}) does not match shape num_public ({})",
                    self.public_values.len(),
                    s.num_public
                ),
            });
        }
        if self.comm_w_per_round.len() != s.num_rounds {
            return Err(VegaError::ProofVerifyError {
                reason: format!(
                    "comm_w_per_round length ({}) does not match shape num_rounds ({})",
                    self.comm_w_per_round.len(),
                    s.num_rounds
                ),
            });
        }
        if self.challenges_per_round.len() != s.num_rounds {
            return Err(VegaError::ProofVerifyError {
                reason: format!(
                    "challenges_per_round length ({}) does not match shape num_rounds ({})",
                    self.challenges_per_round.len(),
                    s.num_rounds
                ),
            });
        }

        // Process each round, absorbing the previous round's commitment before deriving this round's challenges
        for round in 0..s.num_rounds {
            E::PCS::check_commitment(
                &self.comm_w_per_round[round],
                s.num_vars_per_round[round],
                s.commitment_width,
            )?;
            transcript.absorb(b"comm_w_round", &self.comm_w_per_round[round]);

            let derived_challenges = (0..s.num_challenges_per_round[round])
                .map(|_| transcript.squeeze(b"challenge"))
                .collect::<Result<Vec<E::Scalar>, VegaError>>()?;

            if self.challenges_per_round[round] != derived_challenges {
                return Err(VegaError::ProofVerifyError {
                    reason: format!(
                        "MultiRoundR1CSInstance:: Challenges do not match for round {round}"
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn to_regular_instance(&self) -> Result<R1CSInstance<E>, VegaError> {
        let partial_comms = self.comm_w_per_round.clone();
        let comm_w = PCS::<E>::combine_commitments(&partial_comms)?;

        let challenges: Vec<E::Scalar> = self
            .challenges_per_round
            .iter()
            .flatten()
            .cloned()
            .collect();

        Ok(R1CSInstance {
            comm_W: comm_w,
            // Multi-round circuits inputize challenges before public values during synthesis.
            // The regular instance must reflect the same ordering for satisfiability checks.
            X: [challenges, self.public_values.clone()].concat(),
        })
    }
}
