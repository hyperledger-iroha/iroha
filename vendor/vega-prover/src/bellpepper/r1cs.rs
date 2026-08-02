// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! Support for generating R1CS using bellpepper.
use crate::start_span;
use crate::{
    Blind, Commitment, CommitmentKey, PCS, VerifierKey,
    bellpepper::{shape_cs::ShapeCS, solver::SatisfyingAssignment},
    errors::VegaError,
    r1cs::{
        R1CSWitness, SparseMatrix, SplitMultiRoundR1CSInstance, SplitMultiRoundR1CSShape,
        SplitR1CSInstance, SplitR1CSShape,
    },
    traits::{
        Engine,
        circuit::{MultiRoundCircuit, VegaCircuit},
        pcs::PCSEngineTrait,
        transcript::TranscriptEngineTrait,
    },
};
use bellpepper::gadgets::num::AllocatedNum;
use bellpepper_core::{ConstraintSystem, Index, LinearCombination};
use ff::{Field, PrimeField};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// `VegaShape` provides methods for acquiring `SplitR1CSShape` from implementers.
pub trait VegaShape<E: Engine> {
    /// Return an appropriate `SplitR1CSShape`
    fn r1cs_shape<C: VegaCircuit<E>>(circuit: &C) -> Result<SplitR1CSShape<E>, VegaError>;
}

/// Defines rerandomization behavior for preprocessed state
pub trait RerandomizationTrait<E: Engine> {
    /// Returns a rerandomized version of self.
    fn rerandomize(&self, ck: &CommitmentKey<E>, S: &SplitR1CSShape<E>) -> Result<Self, VegaError>
    where
        Self: Sized;

    /// Returns a rerandomized version of self, reusing shared commitments and blinds if provided.
    fn rerandomize_with_shared(
        &self,
        ck: &CommitmentKey<E>,
        S: &SplitR1CSShape<E>,
        comm_W_shared: &Option<Commitment<E>>,
        r_W_shared: &Option<Blind<E>>,
    ) -> Result<Self, VegaError>
    where
        Self: Sized;
}

/// `VegaWitness` provide a method for acquiring an `SplitR1CSInstance` and `R1CSWitness` from implementers.
pub trait VegaWitness<E: Engine> {
    /// Holds the state of the prover after committing to the precommitted portions of the witness.
    type PrecommittedState: Send
        + Sync
        + Serialize
        + for<'de> Deserialize<'de>
        + RerandomizationTrait<E>;

    /// Return partial commitments (to shared variables) and the shared state
    fn shared_witness<C: VegaCircuit<E>>(
        S: &SplitR1CSShape<E>,
        ck: &CommitmentKey<E>,
        circuit: &C,
        is_small: bool,
    ) -> Result<Self::PrecommittedState, VegaError>;

    /// Return partial commitments (to shared and precommitted variables) and the partial witness vector.
    fn precommitted_witness<C: VegaCircuit<E>>(
        ps: &mut Self::PrecommittedState,
        S: &SplitR1CSShape<E>,
        ck: &CommitmentKey<E>,
        circuit: &C,
        is_small: bool,
    ) -> Result<(), VegaError>;

    /// Return an instance and witness, given a shape and ck.
    fn r1cs_instance_and_witness<C: VegaCircuit<E>>(
        ps: &mut Self::PrecommittedState,
        S: &SplitR1CSShape<E>,
        ck: &CommitmentKey<E>,
        circuit: &C,
        is_small: bool,
        transcript: &mut E::TE,
    ) -> Result<(SplitR1CSInstance<E>, R1CSWitness<E>), VegaError>;
}

/// `MultiRoundVegaShape` provides methods for acquiring `SplitMultiRoundR1CSShape` and `CommitmentKey` from implementers.
pub trait MultiRoundVegaShape<E: Engine> {
    /// Return an appropriate `SplitMultiRoundR1CSShape` and `CommitmentKey` structs.
    fn multiround_r1cs_shape<C: MultiRoundCircuit<E>>(
        circuit: &C,
    ) -> Result<
        (
            SplitMultiRoundR1CSShape<E>,
            CommitmentKey<E>,
            VerifierKey<E>,
        ),
        VegaError,
    >;
}

/// `MultiRoundVegaWitness` provide a method for acquiring an `SplitMultiRoundR1CSInstance` and `R1CSWitness` from implementers.
pub trait MultiRoundVegaWitness<E: Engine> {
    /// Holds the state of the prover across multiple rounds.
    type MultiRoundState: Send + Sync + Serialize + for<'de> Deserialize<'de>;

    /// Initialize the multi-round witness process and return the initial state.
    fn initialize_multiround_witness(
        s: &SplitMultiRoundR1CSShape<E>,
    ) -> Result<Self::MultiRoundState, VegaError>;

    /// Process a specific round and update the state, returning the challenges generated after the commitment to the round.
    fn process_round<C: MultiRoundCircuit<E>>(
        state: &mut Self::MultiRoundState,
        s: &SplitMultiRoundR1CSShape<E>,
        ck: &CommitmentKey<E>,
        circuit: &C,
        round_index: usize,
        transcript: &mut E::TE,
    ) -> Result<Vec<E::Scalar>, VegaError>;

    /// Finalize the multi-round witness and return the instance and witness.
    fn finalize_multiround_witness(
        state: &mut Self::MultiRoundState,
        s: &SplitMultiRoundR1CSShape<E>,
    ) -> Result<(SplitMultiRoundR1CSInstance<E>, R1CSWitness<E>), VegaError>;
}

impl<E: Engine> VegaShape<E> for ShapeCS<E> {
    fn r1cs_shape<C: VegaCircuit<E>>(circuit: &C) -> Result<SplitR1CSShape<E>, VegaError> {
        let num_challenges = circuit.num_challenges();

        let mut cs: Self = Self::new();

        // allocate shared variables
        let shared = circuit
            .shared(&mut cs)
            .map_err(|e| VegaError::SynthesisError {
                reason: format!("Unable to allocate shared variables: {e}"),
            })?;

        // determine the number of shared variables
        let num_shared = cs.num_aux();
        debug!("num_shared: {}", num_shared);
        debug!("shared.len(): {}", shared.len());
        if shared.len() > num_shared {
            return Err(VegaError::SynthesisError {
                reason: "Shared variables are not allocated correctly".to_string(),
            });
        }

        // allocate precommitted variables
        let precommitted =
            circuit
                .precommitted(&mut cs, &shared)
                .map_err(|e| VegaError::SynthesisError {
                    reason: format!("Unable to allocate precommitted variables: {e}"),
                })?;

        let num_precommitted = cs.num_aux() - num_shared;
        debug!("num_precommitted: {}", num_precommitted);
        debug!("precommitted.len(): {}", precommitted.len());
        if precommitted.len() > num_precommitted {
            return Err(VegaError::SynthesisError {
                reason: "Precommitted variables are not allocated correctly".to_string(),
            });
        }

        // synthesize the circuit
        circuit
            .synthesize(&mut cs, &shared, &precommitted, None)
            .map_err(|e| VegaError::SynthesisError {
                reason: format!("Unable to synthesize circuit: {e}"),
            })?;

        let mut A = SparseMatrix::<E::Scalar>::empty();
        let mut B = SparseMatrix::<E::Scalar>::empty();
        let mut C = SparseMatrix::<E::Scalar>::empty();

        let mut num_cons_added = 0;
        let mut X = (&mut A, &mut B, &mut C, &mut num_cons_added);
        let num_inputs = cs.num_inputs();
        let num_constraints = cs.num_constraints();
        let num_vars = cs.num_aux();

        for constraint in cs.constraints.iter() {
            add_constraint(
                &mut X,
                num_vars,
                &constraint.0,
                &constraint.1,
                &constraint.2,
            );
        }
        assert_eq!(num_cons_added, num_constraints);
        assert!(num_inputs > num_challenges);

        A.cols = num_vars + num_inputs;
        B.cols = num_vars + num_inputs;
        C.cols = num_vars + num_inputs;

        let num_rest = num_vars - num_shared - num_precommitted;

        debug!("num_constraints: {}", num_constraints);
        debug!("num_vars: {}", num_vars);
        debug!("num_inputs: {}", num_inputs);
        debug!("num_shared: {}", num_shared);
        debug!("num_precommitted: {}", num_precommitted);
        debug!("num_rest: {}", num_rest);

        // Don't count One as an input for shape's purposes.
        let S = SplitR1CSShape::new(
            num_constraints,
            num_shared,
            num_precommitted,
            num_rest,
            num_inputs - 1 - num_challenges,
            num_challenges,
            A,
            B,
            C,
        )
        .unwrap();

        Ok(S)
    }
}

pub(crate) fn add_constraint<S: PrimeField>(
    X: &mut (
        &mut SparseMatrix<S>,
        &mut SparseMatrix<S>,
        &mut SparseMatrix<S>,
        &mut usize,
    ),
    num_vars: usize,
    a_lc: &LinearCombination<S>,
    b_lc: &LinearCombination<S>,
    c_lc: &LinearCombination<S>,
) {
    let (A, B, C, nn) = X;
    let n = **nn;
    assert_eq!(n + 1, A.indptr.len(), "A: invalid shape");
    assert_eq!(n + 1, B.indptr.len(), "B: invalid shape");
    assert_eq!(n + 1, C.indptr.len(), "C: invalid shape");

    let add_constraint_component = |index: Index, coeff: &S, M: &mut SparseMatrix<S>| {
        // we add constraints to the matrix only if the associated coefficient is non-zero
        if *coeff != S::ZERO {
            match index {
                Index::Input(idx) => {
                    // Inputs come last, with input 0, representing 'one',
                    // at position num_vars within the witness vector.
                    let idx = idx + num_vars;
                    M.data.push(*coeff);
                    M.indices.push(idx);
                }
                Index::Aux(idx) => {
                    M.data.push(*coeff);
                    M.indices.push(idx);
                }
            }
        }
    };

    for (index, coeff) in a_lc.iter() {
        add_constraint_component(index.0, coeff, A);
    }
    A.indptr.push(A.indices.len());

    for (index, coeff) in b_lc.iter() {
        add_constraint_component(index.0, coeff, B)
    }
    B.indptr.push(B.indices.len());

    for (index, coeff) in c_lc.iter() {
        add_constraint_component(index.0, coeff, C)
    }
    C.indptr.push(C.indices.len());

    **nn += 1;
}

/// A type that holds the pre-processed state for proving
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct PrecommittedState<E: Engine> {
    pub(crate) cs: SatisfyingAssignment<E>,
    pub(crate) shared: Vec<AllocatedNum<E::Scalar>>,
    pub(crate) precommitted: Vec<AllocatedNum<E::Scalar>>,
    pub(crate) comm_W_shared: Option<Commitment<E>>,
    pub(crate) r_W_shared: Option<Blind<E>>,
    pub(crate) comm_W_precommitted: Option<Commitment<E>>,
    pub(crate) r_W_precommitted: Option<Blind<E>>,
    pub(crate) W: Vec<E::Scalar>,
}

impl<E: Engine> VegaWitness<E> for SatisfyingAssignment<E> {
    type PrecommittedState = PrecommittedState<E>;

    fn shared_witness<C: VegaCircuit<E>>(
        S: &SplitR1CSShape<E>,
        ck: &CommitmentKey<E>,
        circuit: &C,
        is_small: bool,
    ) -> Result<Self::PrecommittedState, VegaError> {
        let (_synth_span, synth_t) = start_span!("shared_witness_synthesize");
        let mut cs: Self = Self::new();

        let num_vars = S.num_shared + S.num_precommitted + S.num_rest;
        debug!("num_vars: {}", num_vars);

        let mut W = vec![E::Scalar::ZERO; num_vars];

        // produce shared witness variables and commit to them
        let shared = circuit
            .shared(&mut cs)
            .map_err(|e| VegaError::SynthesisError {
                reason: format!("Unable to allocate shared variables: {e}"),
            })?;

        // we know that W is large enough to hold all shared variables
        if cs.aux_assignment.len() < S.num_shared_unpadded {
            return Err(VegaError::SynthesisError {
                reason: "Shared variables are not allocated correctly".to_string(),
            });
        }
        W[..S.num_shared_unpadded].copy_from_slice(&cs.aux_assignment[..S.num_shared_unpadded]);

        // partial commitment to shared witness variables
        let (_commit_span, commit_t) = start_span!("commit_witness_shared");
        let (comm_W_shared, r_W_shared) = if S.num_shared_unpadded > 0 {
            let r_W_shared = PCS::<E>::blind(ck, S.num_shared);
            let comm_W_shared = PCS::<E>::commit(ck, &W[0..S.num_shared], &r_W_shared, is_small)?;
            (Some(comm_W_shared), Some(r_W_shared))
        } else {
            (None, None)
        };
        info!(elapsed_ms = %commit_t.elapsed().as_millis(), "commit_witness_shared");
        info!(elapsed_ms = %synth_t.elapsed().as_millis(), "shared_witness_synthesize");

        Ok(PrecommittedState {
            cs,
            shared,
            precommitted: vec![],
            comm_W_shared,
            r_W_shared,
            comm_W_precommitted: None,
            r_W_precommitted: None,
            W,
        })
    }

    fn precommitted_witness<C: VegaCircuit<E>>(
        ps: &mut Self::PrecommittedState,
        S: &SplitR1CSShape<E>,
        ck: &CommitmentKey<E>,
        circuit: &C,
        is_small: bool,
    ) -> Result<(), VegaError> {
        let (_synth_span, synth_t) = start_span!("precommitted_witness_synthesize");
        // produce precommitted witness variables
        let precommitted = circuit.precommitted(&mut ps.cs, &ps.shared).map_err(|e| {
            VegaError::SynthesisError {
                reason: format!("Unable to allocate precommitted variables: {e}"),
            }
        })?;

        if ps.cs.aux_assignment[S.num_shared_unpadded..].len() < S.num_precommitted_unpadded {
            return Err(VegaError::SynthesisError {
                reason: "Precommitted variables are not allocated correctly".to_string(),
            });
        }
        ps.W[S.num_shared..S.num_shared + S.num_precommitted_unpadded].copy_from_slice(
            &ps.cs.aux_assignment
                [S.num_shared_unpadded..S.num_shared_unpadded + S.num_precommitted_unpadded],
        );

        // partial commitment to precommitted witness variables
        let (_commit_precommitted_span, commit_precommitted_t) =
            start_span!("commit_witness_precommitted");
        let (comm_W_precommitted, r_W_precommitted) = if S.num_precommitted_unpadded > 0 {
            let r_W_precommitted = PCS::<E>::blind(ck, S.num_precommitted);
            let comm_W_precommitted = PCS::<E>::commit(
                ck,
                &ps.W[S.num_shared..S.num_shared + S.num_precommitted],
                &r_W_precommitted,
                is_small,
            )?;
            (Some(comm_W_precommitted), Some(r_W_precommitted))
        } else {
            (None, None)
        };
        info!(elapsed_ms = %commit_precommitted_t.elapsed().as_millis(), "commit_witness_precommitted");

        // update the preprocessed state
        ps.comm_W_precommitted = comm_W_precommitted;
        ps.r_W_precommitted = r_W_precommitted;
        ps.precommitted = precommitted;
        info!(elapsed_ms = %synth_t.elapsed().as_millis(), "precommitted_witness_synthesize");

        Ok(())
    }

    fn r1cs_instance_and_witness<C: VegaCircuit<E>>(
        ps: &mut Self::PrecommittedState,
        S: &SplitR1CSShape<E>,
        ck: &CommitmentKey<E>,
        circuit: &C,
        is_small: bool,
        transcript: &mut E::TE,
    ) -> Result<(SplitR1CSInstance<E>, R1CSWitness<E>), VegaError> {
        let (_synth_span, synth_t) = start_span!("circuit_synthesize_rest");

        // partial commitment to precommitted witness variables
        if let Some(comm_W_shared) = &ps.comm_W_shared {
            transcript.absorb(b"comm_W_shared", comm_W_shared);
        }
        if let Some(comm_W_precommitted) = &ps.comm_W_precommitted {
            transcript.absorb(b"comm_W_precommitted", comm_W_precommitted);
        }

        let challenges = (0..S.num_challenges)
            .map(|_| transcript.squeeze(b"challenge"))
            .collect::<Result<Vec<E::Scalar>, VegaError>>()?;

        // Fast path: skip circuit synthesis when there are no rest witness variables to compute
        // and no verifier challenges to process. In this case, the witness W is already fully
        // populated from the prep phase (shared + precommitted sections), and the rest section
        // is all zeros (padding only). Public values come directly from circuit.public_values()
        // rather than from cs.input_assignment, since synthesize is not called.
        //
        // Safety invariants:
        //   1. num_rest_unpadded == 0 ==> no aux variables are allocated during synthesize
        //   2. challenges.is_empty() ==> no challenge-dependent computation is needed
        //   3. circuit.public_values() must return the same values that synthesize would inputize
        let skip_synthesize = S.num_rest_unpadded == 0 && challenges.is_empty();
        if !skip_synthesize {
            // Reset cs to prep-state size before re-synthesis so aux_assignment indices are correct
            // (without this, 2nd+ prove calls accumulate stale entries)
            let prep_aux_len = S.num_shared_unpadded + S.num_precommitted_unpadded;
            ps.cs.aux_assignment.truncate(prep_aux_len);
            ps.cs.input_assignment.truncate(1);

            circuit
                .synthesize(&mut ps.cs, &ps.shared, &ps.precommitted, Some(&challenges))
                .map_err(|e| VegaError::SynthesisError {
                    reason: format!("Unable to synthesize witness: {e}"),
                })?;

            ps.W[S.num_shared + S.num_precommitted
                ..S.num_shared + S.num_precommitted + S.num_rest_unpadded]
                .copy_from_slice(
                    &ps.cs.aux_assignment[S.num_shared_unpadded + S.num_precommitted_unpadded
                        ..S.num_shared_unpadded
                            + S.num_precommitted_unpadded
                            + S.num_rest_unpadded],
                );
        }

        // commit to the rest with partial commitment.
        let (_commit_rest_span, commit_rest_t) = start_span!("commit_witness_rest");
        let r_W_rest = PCS::<E>::blind(ck, S.num_rest);
        let (comm_W_rest, actual_is_small) = if S.num_rest_unpadded == 0 {
            // Fast path: rest is entirely zero-padding, skip MSM and auto-detect
            (PCS::<E>::commit_zeros(ck, S.num_rest, &r_W_rest)?, true)
        } else if is_small {
            let rest_slice = &ps.W
                [S.num_shared + S.num_precommitted..S.num_shared + S.num_precommitted + S.num_rest];
            (PCS::<E>::commit(ck, rest_slice, &r_W_rest, true)?, true)
        } else {
            // Only check non-zero portion for small-value detection (zero padding is trivially small)
            let rest_nz = &ps.W[S.num_shared + S.num_precommitted
                ..S.num_shared + S.num_precommitted + S.num_rest_unpadded];
            let detected_small = rest_nz.iter().all(|s| {
                let bytes = s.to_repr();
                bytes.as_ref()[8..].iter().all(|&b| b == 0)
            });
            let rest_slice = &ps.W
                [S.num_shared + S.num_precommitted..S.num_shared + S.num_precommitted + S.num_rest];
            (
                PCS::<E>::commit(ck, rest_slice, &r_W_rest, detected_small)?,
                detected_small,
            )
        };
        info!(elapsed_ms = %commit_rest_t.elapsed().as_millis(), "commit_witness_rest");
        transcript.absorb(b"comm_W_rest", &comm_W_rest);

        let public_values = if skip_synthesize {
            circuit
                .public_values()
                .map_err(|e| VegaError::SynthesisError {
                    reason: format!("Circuit does not provide public IO: {e}"),
                })?
        } else {
            ps.cs.input_assignment[1..].to_vec()[..S.num_public].to_vec()
        };
        let U = SplitR1CSInstance::<E>::new(
            S,
            ps.comm_W_shared.clone(),
            ps.comm_W_precommitted.clone(),
            comm_W_rest,
            public_values,
            challenges,
        )?;

        let mut blinds = Vec::with_capacity(3);
        if let Some(r_W_shared) = &ps.r_W_shared {
            blinds.push(r_W_shared.clone());
        }
        if let Some(r_W_precommitted) = &ps.r_W_precommitted {
            blinds.push(r_W_precommitted.clone());
        }
        blinds.push(r_W_rest);

        let r_W = PCS::<E>::combine_blinds(&blinds)?;

        // Move W into witness; repopulate ps.W from shared+precommitted (which don't change).
        // Rest portion will be re-copied from cs.aux_assignment on next call.
        let num_vars = S.num_shared + S.num_precommitted + S.num_rest;
        let w_vec = std::mem::replace(&mut ps.W, vec![E::Scalar::ZERO; num_vars]);
        ps.W[..S.num_shared_unpadded]
            .copy_from_slice(&ps.cs.aux_assignment[..S.num_shared_unpadded]);
        ps.W[S.num_shared..S.num_shared + S.num_precommitted_unpadded].copy_from_slice(
            &ps.cs.aux_assignment
                [S.num_shared_unpadded..S.num_shared_unpadded + S.num_precommitted_unpadded],
        );

        let W = R1CSWitness::<E>::new_unchecked(w_vec, r_W, actual_is_small)?;

        info!(elapsed_ms = %synth_t.elapsed().as_millis(), "circuit_synthesize_rest");

        Ok((U, W))
    }
}

impl<E: Engine> RerandomizationTrait<E> for PrecommittedState<E> {
    fn rerandomize(&self, ck: &CommitmentKey<E>, S: &SplitR1CSShape<E>) -> Result<Self, VegaError>
    where
        Self: Sized,
    {
        let mut result = self.clone();
        result.rerandomize_in_place(ck, S)?;
        Ok(result)
    }

    fn rerandomize_with_shared(
        &self,
        ck: &CommitmentKey<E>,
        S: &SplitR1CSShape<E>,
        comm_W_shared: &Option<Commitment<E>>,
        r_W_shared: &Option<Blind<E>>,
    ) -> Result<Self, VegaError>
    where
        Self: Sized,
    {
        let mut result = self.clone();
        result.rerandomize_with_shared_in_place(ck, S, comm_W_shared, r_W_shared)?;
        Ok(result)
    }
}

impl<E: Engine> PrecommittedState<E> {
    /// Rerandomize in-place, avoiding a full struct clone when we already own the data.
    pub fn rerandomize_in_place(
        &mut self,
        ck: &CommitmentKey<E>,
        S: &SplitR1CSShape<E>,
    ) -> Result<(), VegaError> {
        if let (Some(comm), Some(r_old)) = (&self.comm_W_shared, &self.r_W_shared) {
            let r_new = PCS::<E>::blind(ck, S.num_shared);
            self.comm_W_shared = Some(PCS::<E>::rerandomize_commitment(ck, comm, r_old, &r_new)?);
            self.r_W_shared = Some(r_new);
        }
        if let (Some(comm), Some(r_old)) = (&self.comm_W_precommitted, &self.r_W_precommitted) {
            let r_new = PCS::<E>::blind(ck, S.num_precommitted);
            self.comm_W_precommitted =
                Some(PCS::<E>::rerandomize_commitment(ck, comm, r_old, &r_new)?);
            self.r_W_precommitted = Some(r_new);
        }
        Ok(())
    }

    /// Rerandomize in-place, reusing shared commitments from another state.
    pub fn rerandomize_with_shared_in_place(
        &mut self,
        ck: &CommitmentKey<E>,
        S: &SplitR1CSShape<E>,
        comm_W_shared: &Option<Commitment<E>>,
        r_W_shared: &Option<Blind<E>>,
    ) -> Result<(), VegaError> {
        self.comm_W_shared = comm_W_shared.clone();
        self.r_W_shared = r_W_shared.clone();
        if let (Some(comm), Some(r_old)) = (&self.comm_W_precommitted, &self.r_W_precommitted) {
            let r_new = PCS::<E>::blind(ck, S.num_precommitted);
            self.comm_W_precommitted =
                Some(PCS::<E>::rerandomize_commitment(ck, comm, r_old, &r_new)?);
            self.r_W_precommitted = Some(r_new);
        }
        Ok(())
    }
}

impl<E: Engine> MultiRoundVegaShape<E> for ShapeCS<E> {
    fn multiround_r1cs_shape<C: MultiRoundCircuit<E>>(
        circuit: &C,
    ) -> Result<
        (
            SplitMultiRoundR1CSShape<E>,
            CommitmentKey<E>,
            VerifierKey<E>,
        ),
        VegaError,
    > {
        let num_rounds = circuit.num_rounds();
        let mut cs: Self = Self::new();

        // Collect variables and challenges per round
        let mut vars_per_round: Vec<Vec<AllocatedNum<E::Scalar>>> = Vec::new();
        let mut challenges_per_round: Vec<Vec<AllocatedNum<E::Scalar>>> = Vec::new();
        let mut num_vars_per_round: Vec<usize> = Vec::new();
        let mut num_challenges_per_round: Vec<usize> = Vec::new();

        // Process each round to collect shape information
        for round in 0..num_rounds {
            let num_challenges =
                circuit
                    .num_challenges(round)
                    .map_err(|e| VegaError::SynthesisError {
                        reason: format!("Unable to get num_challenges for round {round}: {e}"),
                    })?;
            num_challenges_per_round.push(num_challenges);

            // For shape generation, we don't need actual challenge values, so pass None
            let prev_aux = cs.num_aux();
            let (round_vars, round_challenges) = circuit
                .rounds(&mut cs, round, &vars_per_round, &challenges_per_round, None)
                .map_err(|e| VegaError::SynthesisError {
                    reason: format!("Unable to synthesize round {round}: {e}"),
                })?;

            // Determine how many new auxiliary variables were allocated in this round.
            let new_aux = cs.num_aux();
            num_vars_per_round.push(new_aux - prev_aux);

            // Record the high-level variables/challenges that the round chose to return so
            // they can be fed into subsequent rounds.
            vars_per_round.push(round_vars);
            challenges_per_round.push(round_challenges);
        }

        let mut A = SparseMatrix::<E::Scalar>::empty();
        let mut B = SparseMatrix::<E::Scalar>::empty();
        let mut C = SparseMatrix::<E::Scalar>::empty();

        let mut num_cons_added = 0;
        let mut x = (&mut A, &mut B, &mut C, &mut num_cons_added);
        let num_inputs = cs.num_inputs();
        let num_constraints = cs.num_constraints();
        let total_vars = cs.num_aux();

        for constraint in cs.constraints.iter() {
            add_constraint(
                &mut x,
                total_vars,
                &constraint.0,
                &constraint.1,
                &constraint.2,
            );
        }
        assert_eq!(num_cons_added, num_constraints);

        A.cols = total_vars + num_inputs;
        B.cols = total_vars + num_inputs;
        C.cols = total_vars + num_inputs;

        // Don't count One as an input for shape's purposes.
        let num_public_values = num_inputs - 1 - num_challenges_per_round.iter().sum::<usize>();
        let S = SplitMultiRoundR1CSShape::new(
            circuit.commitment_width(),
            num_constraints,
            num_vars_per_round,
            num_challenges_per_round,
            num_public_values,
            A,
            B,
            C,
        )?;

        let (ck, vk) = S.commitment_key();

        Ok((S, ck, vk))
    }
}

/// A type that holds the multi-round state for proving
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct MultiRoundState<E: Engine> {
    cs: SatisfyingAssignment<E>,
    vars_per_round: Vec<Vec<AllocatedNum<E::Scalar>>>,
    challenges_per_round: Vec<Vec<AllocatedNum<E::Scalar>>>,
    challenges: Vec<Vec<E::Scalar>>,
    comm_w_per_round: Vec<Commitment<E>>,
    pub(crate) r_w_per_round: Vec<Blind<E>>,
    w: Vec<E::Scalar>,
    current_round: usize,
    num_rounds: usize,
}

impl<E: Engine> MultiRoundVegaWitness<E> for SatisfyingAssignment<E> {
    type MultiRoundState = MultiRoundState<E>;

    fn initialize_multiround_witness(
        s: &SplitMultiRoundR1CSShape<E>,
    ) -> Result<Self::MultiRoundState, VegaError> {
        let cs = Self::new();
        let total_vars: usize = s.num_vars_per_round.iter().sum();
        let w = vec![E::Scalar::ZERO; total_vars];

        Ok(MultiRoundState {
            cs,
            vars_per_round: Vec::new(),
            challenges_per_round: Vec::new(),
            challenges: Vec::new(),
            comm_w_per_round: Vec::new(),
            r_w_per_round: Vec::new(),
            w,
            current_round: 0,
            num_rounds: s.num_rounds,
        })
    }

    fn process_round<C: MultiRoundCircuit<E>>(
        state: &mut Self::MultiRoundState,
        s: &SplitMultiRoundR1CSShape<E>,
        ck: &CommitmentKey<E>,
        circuit: &C,
        round_index: usize,
        transcript: &mut E::TE,
    ) -> Result<Vec<E::Scalar>, VegaError> {
        if round_index != state.current_round {
            return Err(VegaError::SynthesisError {
                reason: format!(
                    "Expected round {}, got {}",
                    state.current_round, round_index
                ),
            });
        }

        // Process this round, supplying references to variables/challenges from previous rounds
        let chals = if round_index == 0 {
            None
        } else {
            state.challenges.get(round_index - 1).map(|c| c.as_slice())
        };

        let (round_vars, round_challenges) = circuit
            .rounds(
                &mut state.cs,
                round_index,
                &state.vars_per_round,
                &state.challenges_per_round,
                chals,
            )
            .map_err(|e| VegaError::SynthesisError {
                reason: format!("Unable to synthesize round {round_index}: {e}"),
            })?;

        // Update witness with new variables
        // We need to map the *unpadded* auxiliary variables produced by this round
        // into the appropriate *padded* segment of the global witness vector.
        // 1. `start_idx_unpadded` - offset within `aux_assignment` (contains only the
        //    actual variables produced so far).
        // 2. `start_idx_padded`   - offset within the global witness vector (which
        //    has per-round padding applied).
        let start_idx_unpadded: usize = s.num_vars_per_round_unpadded[..round_index].iter().sum();
        let start_idx_padded: usize = s.num_vars_per_round[..round_index].iter().sum();
        let round_vars_unpadded = s.num_vars_per_round_unpadded[round_index];

        // Copy only the actually-allocated variables; leave the padding slots as 0.
        if state.cs.aux_assignment.len() >= start_idx_unpadded + round_vars_unpadded {
            state.w[start_idx_padded..start_idx_padded + round_vars_unpadded].copy_from_slice(
                &state.cs.aux_assignment
                    [start_idx_unpadded..start_idx_unpadded + round_vars_unpadded],
            );
        }

        // Commit to this round's variables
        let start_padded: usize = s.num_vars_per_round[..round_index].iter().sum();
        let r_w_per_round = PCS::<E>::blind(ck, s.num_vars_per_round[round_index]);
        let comm_w_round = PCS::<E>::commit(
            ck,
            &state.w[start_padded..start_padded + s.num_vars_per_round[round_index]],
            &r_w_per_round,
            false,
        )?;

        // absorb commitment to this round's variables
        transcript.absorb(b"comm_w_round", &comm_w_round);

        // Generate challenges for this round
        let challenges = (0..s.num_challenges_per_round[round_index])
            .map(|_| transcript.squeeze(b"challenge"))
            .collect::<Result<Vec<E::Scalar>, VegaError>>()?;

        state.vars_per_round.push(round_vars);
        state.challenges_per_round.push(round_challenges);
        state.comm_w_per_round.push(comm_w_round);
        state.r_w_per_round.push(r_w_per_round);
        state.challenges.push(challenges.clone());
        state.current_round += 1;

        // Return the challenges that were generated for this round
        Ok(challenges)
    }

    fn finalize_multiround_witness(
        state: &mut Self::MultiRoundState,
        s: &SplitMultiRoundR1CSShape<E>,
    ) -> Result<(SplitMultiRoundR1CSInstance<E>, R1CSWitness<E>), VegaError> {
        if state.current_round != state.num_rounds {
            return Err(VegaError::SynthesisError {
                reason: format!(
                    "Expected {} rounds, processed {}",
                    state.num_rounds, state.current_round
                ),
            });
        }

        let num_challenges: usize = s.num_challenges_per_round.iter().sum();

        // collect public values
        let public_values = state.cs.input_assignment[1 + num_challenges..].to_vec();

        let u = SplitMultiRoundR1CSInstance::<E>::new(
            s,
            state.comm_w_per_round.clone(),
            public_values,
            state.challenges.clone(),
        )?;

        let r_w = PCS::<E>::combine_blinds(&state.r_w_per_round)?;
        let w = R1CSWitness::<E>::new_unchecked(state.w.clone(), r_w, false)?;

        Ok((u, w))
    }
}
