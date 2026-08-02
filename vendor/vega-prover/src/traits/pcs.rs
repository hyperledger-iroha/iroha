// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! This module defines a collection of traits that define the behavior of a commitment engine
//! We require the commitment engine to provide a commitment to vectors with a single group element
use crate::{
    errors::VegaError,
    traits::{Engine, TranscriptReprTrait},
};
use core::fmt::Debug;
use ff::Field;
use serde::{Deserialize, Serialize};

/// This trait defines the behavior of the commitment
pub trait CommitmentTrait<E: Engine>:
    Clone
    + Debug
    + PartialEq
    + Eq
    + Send
    + Sync
    + TranscriptReprTrait<E::GE>
    + Serialize
    + for<'de> Deserialize<'de>
{
}

/// A trait that ties different pieces of the commitment generation together
pub trait PCSEngineTrait<E: Engine>: Clone + Send + Sync {
    /// Holds the type of the commitment key
    /// The key should quantify its length in terms of group generators.
    type CommitmentKey: Clone + Debug + Send + Sync + Serialize + for<'de> Deserialize<'de>;

    /// A type that holds the verifier key
    type VerifierKey: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de>;

    /// Holds the type of the commitment
    type Commitment: CommitmentTrait<E>;

    /// Holds the type of the blind
    type Blind: Clone + Debug + Send + Sync + PartialEq + Eq + Serialize + for<'de> Deserialize<'de>;

    /// A type that holds the evaluation argument
    type EvaluationArgument: Clone + Debug + Send + Sync + Serialize + for<'de> Deserialize<'de>;

    /// Samples a new commitment key of a specified size and a verifier key
    fn setup(
        label: &'static [u8],
        n: usize,
        width: usize,
    ) -> (Self::CommitmentKey, Self::VerifierKey);

    /// Eagerly initialize any lazily-computed tables in the commitment key.
    /// Call before cloning to ensure copies get precomputed state.
    fn precompute_ck(_ck: &Self::CommitmentKey) {}

    /// Returns a blind to be used for commitment to a polynomial of size `n`
    fn blind(ck: &Self::CommitmentKey, n: usize) -> Self::Blind;

    /// Commits to the provided vector using the provided ck and returns the commitment
    fn commit(
        ck: &Self::CommitmentKey,
        v: &[E::Scalar],
        r: &Self::Blind,
        is_small: bool,
    ) -> Result<Self::Commitment, VegaError>;

    /// Commits to an all-zero vector of size `n` with the given blind.
    /// Default: creates a zero vector and calls `commit`. Implementations may override
    /// for efficiency (e.g., computing only the blind contribution per row).
    fn commit_zeros(
        ck: &Self::CommitmentKey,
        n: usize,
        r: &Self::Blind,
    ) -> Result<Self::Commitment, VegaError> {
        let zeros = vec![E::Scalar::ZERO; n];
        Self::commit(ck, &zeros, r, true)
    }

    /// Checks if the provided commitment commits to a vector of the specified length
    fn check_commitment(comm: &Self::Commitment, n: usize, width: usize) -> Result<(), VegaError>;

    /// Rerandomizes the provided commitment using the provided blind
    fn rerandomize_commitment(
        ck: &Self::CommitmentKey,
        comm: &Self::Commitment,
        r_old: &Self::Blind,
        r_new: &Self::Blind,
    ) -> Result<Self::Commitment, VegaError>;

    /// Combines the provided commitments (each committing to a multilinear polynomial) into a single commitment.
    ///
    /// # Parameters
    /// - `comms`: A slice of commitments to be combined. The order of the commitments in the slice must match the order in which they were generated using `commit`.
    ///
    /// # Constraints
    /// - All commitments in the slice must be valid and correspond to the same commitment key.
    /// - The number of commitments must match the expected number for the final commitment.
    ///
    /// # Returns
    /// - A single combined commitment if the operation is successful.
    /// - An error of type `VegaError` if the combination fails due to invalid inputs or mismatched constraints.
    ///
    /// # Usage
    /// This method is used to finalize the commitment after multiple commitments have been made.
    /// Ensure that the commitments are provided in the correct order and meet all constraints.
    fn combine_commitments(comms: &[Self::Commitment]) -> Result<Self::Commitment, VegaError>;

    /// Combines the provided blinds into a single blind.
    /// The order of the blinds must match the order of the commitments used to generate them
    /// Returns an error if the combination fails
    fn combine_blinds(blinds: &[Self::Blind]) -> Result<Self::Blind, VegaError>;

    /// A method to prove the evaluation of a multilinear polynomial
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
    ) -> Result<Self::EvaluationArgument, VegaError>;

    /// A method to verify the purported evaluation of a multilinear polynomials
    fn verify(
        vk: &Self::VerifierKey,
        ck_eval: &Self::CommitmentKey,
        transcript: &mut E::TE,
        comm: &Self::Commitment,
        point: &[E::Scalar],
        comm_eval: &Self::Commitment,
        arg: &Self::EvaluationArgument,
    ) -> Result<(), VegaError>;

    /// Compute raw (unblinded) commitment. Returns per-row group elements.
    /// Default: not supported (panics). Override for schemes that support this.
    fn commit_without_blind(
        _ck: &Self::CommitmentKey,
        _v: &[E::Scalar],
        _is_small: bool,
    ) -> Result<Vec<E::GE>, VegaError> {
        Err(VegaError::InternalError {
            reason: "commit_without_blind not supported for this PCS".to_string(),
        })
    }

    /// Build commitment from precomputed raw MSMs plus a delta vector.
    /// For each row: final = raw[i] + MSM(delta[row_i], gens) + blind[i] * H
    /// Default: not supported. Override for schemes that support this.
    fn commit_incremental(
        _ck: &Self::CommitmentKey,
        _raw: &[E::GE],
        _delta: &[E::Scalar],
        _blind: &Self::Blind,
    ) -> Result<Self::Commitment, VegaError> {
        Err(VegaError::InternalError {
            reason: "commit_incremental not supported for this PCS".to_string(),
        })
    }

    /// Direct polynomial opening (prover side).
    ///
    /// Splits the evaluation point into row and column parts based on the commitment width,
    /// computes the RLC'd vector `v` (of length `width`) and the combined scalar blind.
    /// Returns `(v, combined_blind)`.
    ///
    /// The evaluation is then `<v, eq(point_right)>`.
    fn prove_direct(
        _ck: &Self::CommitmentKey,
        _poly: &[E::Scalar],
        _blind: &Self::Blind,
        _point: &[E::Scalar],
    ) -> Result<(Vec<E::Scalar>, E::Scalar), VegaError> {
        Err(VegaError::InternalError {
            reason: "prove_direct not supported for this PCS".to_string(),
        })
    }

    /// Verify a direct polynomial opening (verifier side).
    ///
    /// Checks that the RLC'd vector `v` is consistent with the row commitments in `comm`,
    /// then computes and returns the polynomial evaluation `<v, eq(point_right)>`.
    fn verify_direct(
        _vk: &Self::VerifierKey,
        _comm: &Self::Commitment,
        _v: &[E::Scalar],
        _combined_blind: &E::Scalar,
        _point: &[E::Scalar],
    ) -> Result<E::Scalar, VegaError> {
        Err(VegaError::InternalError {
            reason: "verify_direct not supported for this PCS".to_string(),
        })
    }
}

/// A trait that extends the PCSEngineTrait to include folding capabilities
/// This trait allows for folding multiple commitments (and blinds) into a single commitment (a single blind) using specified weights
pub trait FoldingEngineTrait<E: Engine>: PCSEngineTrait<E> {
    /// A method to fold the provided commitments into a single commitment using the provided weights
    /// The weights should be the same length as the number of commitments
    fn fold_commitments(
        comms: &[Self::Commitment],
        weights: &[E::Scalar],
    ) -> Result<Self::Commitment, VegaError>;

    /// A method to fold the provided blinds into a single blind using the provided weights
    /// The weights should be the same length as the number of blinds
    fn fold_blinds(blinds: &[Self::Blind], weights: &[E::Scalar])
    -> Result<Self::Blind, VegaError>;

    /// Fold commitments, but for rows beyond `num_data_rows`, compute from
    /// folded blind + h instead of full MSM. This is an optimization for
    /// split instances where rest rows are blind-only (zero witness).
    /// Default: falls back to full fold_commitments.
    fn fold_commitments_partial(
        comms: &[Self::Commitment],
        weights: &[E::Scalar],
        _num_data_rows: usize,
        _folded_blind: &Self::Blind,
        _ck: &Self::CommitmentKey,
    ) -> Result<Self::Commitment, VegaError> {
        Self::fold_commitments(comms, weights)
    }
}
