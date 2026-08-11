// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! Zero-knowledge single-circuit (SC) prover. Implements the Spartan protocol with Nova folding for ZK.
use crate::{
    CommitmentKey, PCS, VerifierKey,
    bellpepper::{
        r1cs::{
            MultiRoundVegaShape, MultiRoundVegaWitness, PrecommittedState, VegaShape, VegaWitness,
        },
        shape_cs::ShapeCS,
        solver::SatisfyingAssignment,
    },
    big_num::DelayedReduction,
    digest::DigestComputer,
    errors::VegaError,
    math::Math,
    nifs::NovaNIFS,
    polys::{
        eq::EqPolynomial,
        multilinear::{MultilinearPolynomial, SparsePolynomial},
        univariate::UniPoly,
    },
    r1cs::{
        R1CSShape, RelaxedR1CSInstance, SplitMultiRoundR1CSInstance, SplitMultiRoundR1CSShape,
        SplitR1CSInstance, SplitR1CSShape,
    },
    start_span,
    sumcheck::SumcheckProof,
    traits::{
        Engine,
        circuit::VegaCircuit,
        pcs::{FoldingEngineTrait, PCSEngineTrait},
        snark::{DigestHelperTrait, R1CSSNARKTrait, VegaDigest},
        transcript::TranscriptEngineTrait,
    },
    zk::VegaVerifierCircuit,
};
use ff::Field;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use tracing::info;

/// A type that represents the prover's key
#[derive(Serialize, Deserialize)]
#[serde(bound = "")]
pub struct VegaProverKey<E: Engine> {
    ck: CommitmentKey<E>,
    S: SplitR1CSShape<E>,
    // Verifier-circuit (multi-round) parameters
    vc_shape: SplitMultiRoundR1CSShape<E>,
    // Precomputed regular (single-round) verifier shape
    vc_shape_regular: R1CSShape<E>,
    vc_ck: CommitmentKey<E>,
    vk_digest: VegaDigest, // digest of the verifier's key
}

impl<E: Engine> VegaProverKey<E> {
    /// Returns sizes associated with the SplitR1CSShape.
    /// It returns an array of 10 elements containing:
    /// [num_cons_unpadded, num_shared_unpadded, num_precommitted_unpadded, num_rest_unpadded,
    ///  num_cons, num_shared, num_precommitted, num_rest,
    ///  num_public, num_challenges]
    pub fn sizes(&self) -> [usize; 10] {
        self.S.sizes()
    }
}

/// A type that represents the verifier's key
#[derive(Serialize, Deserialize)]
#[serde(bound = "")]
pub struct VegaVerifierKey<E: Engine> {
    vk_ee: <E::PCS as PCSEngineTrait<E>>::VerifierKey,
    S: SplitR1CSShape<E>,
    // Verifier-circuit (multi-round) shape
    vc_shape: SplitMultiRoundR1CSShape<E>,
    // Precomputed regular (single-round) verifier shape
    vc_shape_regular: R1CSShape<E>,
    // Commitment key for the verifier-circuit (multi-round) shape; shared with prover
    vc_ck: CommitmentKey<E>,
    // PCS verifier key for the verifier circuit (used by relaxed Spartan verify)
    vc_vk: VerifierKey<E>,
    #[serde(skip, default = "OnceCell::new")]
    digest: OnceCell<VegaDigest>,
}

impl<E: Engine> crate::digest::Digestible for VegaVerifierKey<E> {
    fn write_bytes<W: Sized + std::io::Write>(&self, w: &mut W) -> Result<(), std::io::Error> {
        use bincode::Options;
        let config = bincode::DefaultOptions::new()
            .with_little_endian()
            .with_fixint_encoding();
        config
            .serialize_into(&mut *w, &self.vk_ee)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        // Use fast raw-byte path for the main shape
        self.S.write_bytes(w)?;
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
        Ok(())
    }
}

impl<E: Engine> DigestHelperTrait<E> for VegaVerifierKey<E> {
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
pub struct VegaPrepZkSNARK<E: Engine> {
    ps: PrecommittedState<E>,
    // Cached partial matrix-vector products for precommitted witness columns (deterministic)
    cached_az: Vec<E::Scalar>,
    cached_bz: Vec<E::Scalar>,
    cached_cz: Vec<E::Scalar>,
    // Lazily cached rest-witness commitment (populated on first prove call, deterministic)
    cached_rest_witness: Option<Vec<E::Scalar>>, // rest-witness values (unpadded)
    cached_rest_msm: Option<Vec<E::GE>>,         // raw MSM per row (no blinding)
    // Pre-allocated scratch buffers (reused across prove calls, avoids mmap + page faults)
    scratch_az: Vec<E::Scalar>,
    scratch_bz: Vec<E::Scalar>,
    scratch_cz: Vec<E::Scalar>,
    z_buffer: Vec<E::Scalar>,
    evals_rx_buffer: Vec<E::Scalar>,
}

/// A zero-knowledge succinct non-interactive argument of knowledge (zkSNARK) for a relaxed R1CS instance,
/// produced using Spartan's combination of sum-check protocols and polynomial commitments.
/// This proof attests to knowledge of a witness satisfying the given R1CS constraints, without revealing the witness.
#[derive(Debug, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct VegaZkSNARK<E: Engine> {
    /// Original R1CS instance
    U: SplitR1CSInstance<E>,

    /// Multi-round verifier instance capturing the non-ZK verification trace
    U_verifier: SplitMultiRoundR1CSInstance<E>,
    /// The random relaxed instance used for folding
    random_U: RelaxedR1CSInstance<E>,
    /// NIFS proof for folding a random relaxed instance with the verifier instance
    nifs: NovaNIFS<E>,
    /// Relaxed R1CS Spartan proof of the folded instance (replaces raw folded witness)
    relaxed_snark: crate::spartan_relaxed::RelaxedR1CSSpartanProof<E>,

    /// PCS evaluation argument
    eval_arg: <E::PCS as PCSEngineTrait<E>>::EvaluationArgument,
}

impl<E: Engine> R1CSSNARKTrait<E> for VegaZkSNARK<E>
where
    E::PCS: FoldingEngineTrait<E>,
{
    type ProverKey = VegaProverKey<E>;
    type VerifierKey = VegaVerifierKey<E>;
    type PrepSNARK = VegaPrepZkSNARK<E>;

    fn setup<C: VegaCircuit<E>>(
        circuit: C,
    ) -> Result<(Self::ProverKey, Self::VerifierKey), VegaError> {
        let S = ShapeCS::r1cs_shape(&circuit)?;
        let (ck, vk_ee) = SplitR1CSShape::commitment_key(&[&S])?;
        E::PCS::precompute_ck(&ck);
        // Derive verifier-circuit (multi-round) shape based on outer/inner rounds
        let num_vars = S.num_shared + S.num_precommitted + S.num_rest;
        let num_rounds_x = S.num_cons.log_2();
        let num_rounds_y = num_vars.log_2() + 1;
        let vc = VegaVerifierCircuit::<E>::default(num_rounds_x, num_rounds_y, 16);
        let (vc_shape, vc_ck, vc_vk) =
            <ShapeCS<E> as MultiRoundVegaShape<E>>::multiround_r1cs_shape(&vc)?;
        let vc_shape_regular = vc_shape.to_regular_shape();
        // Eagerly init FixedBaseMul table before cloning so both pk/vk get it
        E::PCS::precompute_ck(&vc_ck);
        let vk = Self::VerifierKey {
            S: S.clone(),
            vk_ee,
            vc_shape: vc_shape.clone(),
            vc_shape_regular: vc_shape_regular.clone(),
            vc_ck: vc_ck.clone(),
            vc_vk,
            digest: OnceCell::new(),
        };

        let vk_digest = vk.digest()?;
        let pk = Self::ProverKey {
            ck,
            S,
            vc_shape,
            vc_shape_regular,
            vc_ck,
            vk_digest,
        };
        // Eagerly build precomputed matrices so prove()/verify() don't pay the cost
        pk.S.precompute();
        vk.S.precompute();
        Ok((pk, vk))
    }

    /// Prepares the SNARK for proving
    fn prep_prove<C: VegaCircuit<E>>(
        pk: &Self::ProverKey,
        circuit: C,
        is_small: bool, // do witness elements fit in machine words?
    ) -> Result<Self::PrepSNARK, VegaError> {
        let mut ps = SatisfyingAssignment::shared_witness(&pk.S, &pk.ck, &circuit, is_small)?;
        SatisfyingAssignment::precommitted_witness(&mut ps, &pk.S, &pk.ck, &circuit, is_small)?;

        // Pre-compute partial matrix-vector products for shared + precommitted witness columns.
        // These values are the same across proofs since shared/precommitted witness doesn't change.
        pk.S.precompute();
        let pre_end = pk.S.num_shared + pk.S.num_precommitted;
        let (cached_az, cached_bz, cached_cz) = pk.S.multiply_vec_precommitted(&ps.W[..pre_end])?;

        // Pre-allocate scratch buffers (reused across prove calls to avoid mmap + page faults)
        let num_cons = pk.S.num_cons;
        let num_z = pk.S.num_shared
            + pk.S.num_precommitted
            + pk.S.num_rest
            + 1
            + pk.S.num_public
            + pk.S.num_challenges;
        let scratch_az = vec![E::Scalar::ZERO; num_cons];
        let scratch_bz = vec![E::Scalar::ZERO; num_cons];
        let scratch_cz = vec![E::Scalar::ZERO; num_cons];
        let z_buffer = vec![E::Scalar::ZERO; num_z];
        let evals_rx_buffer = Vec::with_capacity(num_cons);

        let prep = VegaPrepZkSNARK {
            ps,
            cached_az,
            cached_bz,
            cached_cz,
            cached_rest_witness: None,
            cached_rest_msm: None,
            scratch_az,
            scratch_bz,
            scratch_cz,
            z_buffer,
            evals_rx_buffer,
        };

        Ok(prep)
    }

    /// produces a zero-knowledge succinct proof of satisfiability of an R1CS instance.
    /// Takes ownership of prep state, rerandomizes in-place, and returns it for reuse.
    fn prove<C: VegaCircuit<E>>(
        pk: &Self::ProverKey,
        circuit: C,
        mut prep_snark: Self::PrepSNARK,
        is_small: bool,
    ) -> Result<(Self, Self::PrepSNARK), VegaError> {
        let (_prove_span, prove_t) = start_span!("spartan_zk_prove");
        // rerandomize the prep state in-place (we own it)
        let (_rerandomize_span, rerandomize_t) = start_span!("rerandomize_prep_state");
        prep_snark.ps.rerandomize_in_place(&pk.ck, &pk.S)?;
        info!(elapsed_ms = %rerandomize_t.elapsed().as_millis(), "rerandomize_prep_state");
        let mut transcript = E::TE::new(b"VegaZkSNARK");
        transcript.absorb(b"vk", &pk.vk_digest);
        // absorb public IO before
        let public_values = circuit
            .public_values()
            .map_err(|e| VegaError::SynthesisError {
                reason: format!("Circuit does not provide public IO: {e}"),
            })?;
        transcript.absorb(b"public_values", &public_values.as_slice());

        // Original R1CS instance and witness (used for PCS evaluation only)
        // Uses precomputed rest-witness MSMs from prep_prove -- only the delta needs new MSMs.

        // Absorb precommitted commitments
        if let Some(comm_W_shared) = &prep_snark.ps.comm_W_shared {
            transcript.absorb(b"comm_W_shared", comm_W_shared);
        }
        if let Some(comm_W_precommitted) = &prep_snark.ps.comm_W_precommitted {
            transcript.absorb(b"comm_W_precommitted", comm_W_precommitted);
        }

        let challenges = (0..pk.S.num_challenges)
            .map(|_| transcript.squeeze(b"challenge"))
            .collect::<Result<Vec<E::Scalar>, VegaError>>()?;

        // Reset cs to prep-state size so synthesize appends to the right position
        let prep_aux_len = pk.S.num_shared_unpadded + pk.S.num_precommitted_unpadded;
        prep_snark.ps.cs.aux_assignment.truncate(prep_aux_len);
        prep_snark.ps.cs.input_assignment.truncate(1);

        // Synthesize rest-witness with real challenges
        circuit
            .synthesize(
                &mut prep_snark.ps.cs,
                &prep_snark.ps.shared,
                &prep_snark.ps.precommitted,
                Some(&challenges),
            )
            .map_err(|e| VegaError::SynthesisError {
                reason: format!("Unable to synthesize witness: {e}"),
            })?;
        // Copy rest-witness into W
        let rest_src_start = pk.S.num_shared_unpadded + pk.S.num_precommitted_unpadded;
        let rest_src_end = rest_src_start + pk.S.num_rest_unpadded;
        let rest_dst_start = pk.S.num_shared + pk.S.num_precommitted;
        prep_snark.ps.W[rest_dst_start..rest_dst_start + pk.S.num_rest_unpadded]
            .copy_from_slice(&prep_snark.ps.cs.aux_assignment[rest_src_start..rest_src_end]);

        // Commit rest-witness -- use delta-based approach if we have a cache from a prior prove
        let r_W_rest = PCS::<E>::blind(&pk.ck, pk.S.num_rest);

        let comm_W_rest = if let (Some(cached_msm), Some(cached_witness)) =
            (&prep_snark.cached_rest_msm, &prep_snark.cached_rest_witness)
        {
            // Delta-based: only MSM the changed entries
            let mut delta = vec![E::Scalar::ZERO; pk.S.num_rest];
            for i in 0..pk.S.num_rest_unpadded {
                delta[i] = prep_snark.ps.W[rest_dst_start + i] - cached_witness[i];
            }
            PCS::<E>::commit_incremental(&pk.ck, cached_msm, &delta, &r_W_rest)?
        } else {
            // First prove: commit normally and cache for next time
            let rest_witness_unpadded =
                prep_snark.ps.cs.aux_assignment[rest_src_start..rest_src_end].to_vec();
            let mut W_rest_padded = vec![E::Scalar::ZERO; pk.S.num_rest];
            W_rest_padded[..pk.S.num_rest_unpadded].copy_from_slice(&rest_witness_unpadded);

            // Cache the raw MSMs (deterministic, no randomness)
            let raw_msms = E::PCS::commit_without_blind(&pk.ck, &W_rest_padded, is_small)?;
            prep_snark.cached_rest_witness = Some(rest_witness_unpadded);
            prep_snark.cached_rest_msm = Some(raw_msms.clone());

            // Commit normally (raw + blinding)
            PCS::<E>::commit_incremental(
                &pk.ck,
                &raw_msms,
                &vec![E::Scalar::ZERO; pk.S.num_rest],
                &r_W_rest,
            )?
        };
        transcript.absorb(b"comm_W_rest", &comm_W_rest);

        // Build instance and witness
        let public_values_vec = prep_snark.ps.cs.input_assignment[1..1 + pk.S.num_public].to_vec();
        let U = SplitR1CSInstance::<E>::new(
            &pk.S,
            prep_snark.ps.comm_W_shared.clone(),
            prep_snark.ps.comm_W_precommitted.clone(),
            comm_W_rest,
            public_values_vec,
            challenges,
        )?;

        let mut blinds = Vec::with_capacity(3);
        if let Some(r_W_shared) = &prep_snark.ps.r_W_shared {
            blinds.push(r_W_shared.clone());
        }
        if let Some(r_W_precommitted) = &prep_snark.ps.r_W_precommitted {
            blinds.push(r_W_precommitted.clone());
        }
        blinds.push(r_W_rest);
        let r_W = PCS::<E>::combine_blinds(&blinds)?;
        // Prepare vectors and polynomials for building the verifier-circuit trace
        // Build z using pre-allocated buffer (avoids 32MB mmap + page faults after first prove)
        let mut z = std::mem::take(&mut prep_snark.z_buffer);
        z.clear();
        z.extend_from_slice(&prep_snark.ps.W);
        z.push(E::Scalar::ONE);
        z.extend_from_slice(&U.public_values);
        z.extend_from_slice(&U.challenges);

        let num_vars = pk.S.num_shared + pk.S.num_precommitted + pk.S.num_rest;
        let (num_rounds_x, num_rounds_y) = (pk.S.num_cons.log_2(), num_vars.log_2() + 1);
        info!(
          num_shared = %pk.S.num_shared,
          num_precommitted = %pk.S.num_precommitted,
          num_rest = %pk.S.num_rest,
          num_public = %pk.S.num_public,
          num_challenges = %pk.S.num_challenges,
          num_cons = %pk.S.num_cons,
          num_rounds_x = %num_rounds_x,
          num_rounds_y = %num_rounds_y,
          "circuit_sizes"
        );

        // Sample tau challenges used for the outer equality polynomial
        let (_taus_span, taus_t) = start_span!("sample_taus");
        let taus = (0..num_rounds_x)
            .map(|_| transcript.squeeze(b"t"))
            .collect::<Result<Vec<_>, VegaError>>()?;
        info!(elapsed_ms = %taus_t.elapsed().as_millis(), "sample_taus");
        // Use pre-allocated scratch buffers (avoids 96MB mmap + page faults after first prove)
        let (_mv_span, mv_t) = start_span!("matrix_vector_multiply");
        let mut scratch_az = std::mem::take(&mut prep_snark.scratch_az);
        let mut scratch_bz = std::mem::take(&mut prep_snark.scratch_bz);
        let mut scratch_cz = std::mem::take(&mut prep_snark.scratch_cz);
        pk.S.multiply_vec_incremental_into(
            &z,
            &prep_snark.cached_az,
            &prep_snark.cached_bz,
            &prep_snark.cached_cz,
            &mut scratch_az,
            &mut scratch_bz,
            &mut scratch_cz,
        )?;
        info!(
          elapsed_ms = %mv_t.elapsed().as_millis(),
          "matrix_vector_multiply"
        );

        let (_mp_span, mp_t) = start_span!("prepare_multilinear_polys");
        let mut poly_Az = MultilinearPolynomial::new(scratch_az);
        let mut poly_Bz = MultilinearPolynomial::new(scratch_bz);
        let mut poly_Cz = MultilinearPolynomial::new(scratch_cz);
        info!(elapsed_ms = %mp_t.elapsed().as_millis(), "prepare_multilinear_polys");
        // Initialize multi-round verifier circuit (will be filled as we go)
        let mut verifier_circuit = VegaVerifierCircuit::<E>::default(
            num_rounds_x,
            num_rounds_y,
            pk.vc_shape.commitment_width,
        );
        let mut state = SatisfyingAssignment::<E>::initialize_multiround_witness(&pk.vc_shape)?;

        // Outer sum-check
        let (_sc_span, sc_t) = start_span!("outer_sumcheck");
        let r_x = SumcheckProof::<E>::prove_cubic_with_additive_term_zk(
            num_rounds_x,
            &taus,
            &mut poly_Az,
            &mut poly_Bz,
            &mut poly_Cz,
            &mut verifier_circuit,
            &mut state,
            &pk.vc_shape,
            &pk.vc_ck,
            &mut transcript,
        )?;
        info!(elapsed_ms = %sc_t.elapsed().as_millis(), "outer_sumcheck");
        // Outer final round data
        verifier_circuit.claim_Az = poly_Az[0];
        verifier_circuit.claim_Bz = poly_Bz[0];
        verifier_circuit.claim_Cz = poly_Cz[0];
        // Recover scratch buffers (bound down to 1 element, but allocation preserved)
        let scratch_az = poly_Az.into_vec();
        let scratch_bz = poly_Bz.into_vec();
        let scratch_cz = poly_Cz.into_vec();
        verifier_circuit.tau_at_rx = EqPolynomial::new(taus).evaluate(&r_x);

        // Process the "outer final" round in the circuit and capture challenge
        let chals = SatisfyingAssignment::<E>::process_round(
            &mut state,
            &pk.vc_shape,
            &pk.vc_ck,
            &verifier_circuit,
            num_rounds_x,
            &mut transcript,
        )?;
        let r = chals[0];

        // Merged: compute eq(r_x), bind row variables, and prepare poly_ABC in a single pipeline
        // poly_ABC is num_vars + num_extra long (contrast: old code used 2*num_vars).
        // The "virtual" polynomial is 2*num_vars, with:
        //   poly_ABC[0..num_vars]: witness column contributions (the "low" half)
        //   poly_ABC[num_vars..num_vars+num_extra]: 1/public/challenge column contributions
        //   positions num_vars+num_extra..2*num_vars: implicitly zero
        let (_abc_span, abc_t) = start_span!("prepare_poly_ABC");
        let mut evals_rx_buffer = std::mem::take(&mut prep_snark.evals_rx_buffer);
        EqPolynomial::evals_from_points_into(&r_x, &mut evals_rx_buffer);
        let mut poly_ABC_vec = pk.S.bind_and_prepare_poly_ABC(&evals_rx_buffer, &r);
        info!(elapsed_ms = %abc_t.elapsed().as_millis(), "prepare_poly_ABC");
        // Inner sum-check
        let (_sc2_span, sc2_t) = start_span!("inner_sumcheck");
        let claim_inner_joint = verifier_circuit.claim_Az
            + r * verifier_circuit.claim_Bz
            + r * r * verifier_circuit.claim_Cz;

        // --- Manual first round of inner sumcheck ---
        // The "virtual" polynomial pair is:
        //   ABC_low[j] = poly_ABC_vec[j] for j=0..num_vars-1
        //   ABC_high[j] = poly_ABC_vec[num_vars+j] for j=0..num_extra-1, else 0
        //   z_low[j] = z[j] for j=0..num_vars-1
        //   z_high[j] = z[num_vars+j] for j=0..num_extra-1, else 0
        //
        // eval_0 = sum ABC_low[j] * z_low[j]
        // t_inf = sum (ABC_high[j] - ABC_low[j]) * (z_high[j] - z_low[j])
        //   Split: for j >= num_extra both high values are 0, so contribution = ABC_low[j]*z_low[j]
        //   Hence: t_inf = eval_0 - sum_{j<num_extra} ABC_low[j]*z_low[j]
        //                 + sum_{j<num_extra} (ABC_high[j] - ABC_low[j])*(z_high[j] - z_low[j])
        type Acc<S> = <S as DelayedReduction<S>>::Accumulator;
        let num_extra = 1 + pk.S.num_public + pk.S.num_challenges;

        // Compute eval_0 = inner product of ABC_low and z_low
        let mut acc_eval0 = Acc::<E::Scalar>::default();
        for j in 0..num_vars {
            <E::Scalar as DelayedReduction<E::Scalar>>::unreduced_multiply_accumulate(
                &mut acc_eval0,
                &poly_ABC_vec[j],
                &z[j],
            );
        }
        let eval0 = <E::Scalar as DelayedReduction<E::Scalar>>::reduce(&acc_eval0);

        // Compute corrections for the num_extra terms where high values are non-zero
        let mut correction_low = E::Scalar::ZERO;
        let mut correction_cross = E::Scalar::ZERO;
        for j in 0..num_extra {
            let abc_low = poly_ABC_vec[j];
            let abc_high = poly_ABC_vec[num_vars + j];
            let z_low_j = z[j];
            let z_high_j = z[num_vars + j];
            correction_low += abc_low * z_low_j;
            correction_cross += (abc_high - abc_low) * (z_high_j - z_low_j);
        }

        let t_inf = eval0 - correction_low + correction_cross;

        // BDDT: eval_2 = 2*claim - 3*eval_0 + 2*t_inf
        let three_eval0 = eval0 + eval0 + eval0;
        let eval2 = claim_inner_joint + claim_inner_joint - three_eval0 + t_inf + t_inf;
        let evals = vec![eval0, claim_inner_joint - eval0, eval2];
        let inner_r0_poly = UniPoly::from_evals(&evals)?;

        verifier_circuit.inner_polys[0] = [
            inner_r0_poly.coeffs[0],
            inner_r0_poly.coeffs[1],
            inner_r0_poly.coeffs[2],
        ];

        // Process round 0 of inner sumcheck through verifier circuit
        let chals_inner_r0 = SatisfyingAssignment::<E>::process_round(
            &mut state,
            &pk.vc_shape,
            &pk.vc_ck,
            &verifier_circuit,
            num_rounds_x + 1,
            &mut transcript,
        )?;
        let r0_inner = chals_inner_r0[0];
        let claim_after_r0 = inner_r0_poly.evaluate(&r0_inner);

        // Bind poly_ABC and z together for better cache utilization:
        // For j < num_extra: standard bind for both
        // For j >= num_extra: both scale by (1-r0) -- fused into single pass
        let one_minus_r0 = E::Scalar::ONE - r0_inner;
        for j in 0..num_extra {
            let abc_low = poly_ABC_vec[j];
            let abc_high = poly_ABC_vec[num_vars + j];
            poly_ABC_vec[j] = abc_low + r0_inner * (abc_high - abc_low);
            let z_low = z[j];
            let z_high = z[num_vars + j];
            z[j] = z_low + r0_inner * (z_high - z_low);
        }
        for j in num_extra..num_vars {
            poly_ABC_vec[j] *= one_minus_r0;
            z[j] *= one_minus_r0;
        }
        poly_ABC_vec.truncate(num_vars);
        z.truncate(num_vars);

        // Continue with remaining rounds of inner sumcheck
        let mut poly_ABC = MultilinearPolynomial::new(poly_ABC_vec);
        let mut poly_z = MultilinearPolynomial::new(z);

        let (r_y_rest, evals) = SumcheckProof::<E>::prove_quad_zk(
            &claim_after_r0,
            num_rounds_y - 1,
            &mut poly_ABC,
            &mut poly_z,
            &mut verifier_circuit,
            &mut state,
            &pk.vc_shape,
            &pk.vc_ck,
            &mut transcript,
            num_rounds_x + 2,
            1,
        )?;

        let mut r_y = Vec::with_capacity(num_rounds_y);
        r_y.push(r0_inner);
        r_y.extend_from_slice(&r_y_rest);

        let eval_Z = evals[1]; // evaluation of Z at r_y
        info!(elapsed_ms = %sc2_t.elapsed().as_millis(), "inner_sumcheck");
        // Recover scratch buffers from sumcheck (allocation preserved, contents destroyed)
        let z_buffer = poly_z.into_vec();
        drop(poly_ABC);
        // Compute final evaluations needed for the inner-final round
        let U_regular = U.to_regular_instance()?;
        let eval_X = {
            let X = vec![E::Scalar::ONE]
                .into_iter()
                .chain(U_regular.X.iter().cloned())
                .collect::<Vec<E::Scalar>>();
            SparsePolynomial::new(num_rounds_y - 1, X).evaluate(&r_y[1..])
        };

        // compute eval_W = (eval_Z - r_y[0] * eval_X) / (1 - r_y[0]) because Z = (W, 1, X)
        let inv: Option<E::Scalar> = (E::Scalar::ONE - r_y[0]).invert().into();
        let eval_W = (eval_Z - r_y[0] * eval_X) * inv.ok_or(VegaError::DivisionByZero)?;

        // Process the inner-final equality round
        // Set verifier circuit public values before processing inner-final round
        verifier_circuit.eval_W = eval_W;
        verifier_circuit.eval_X = eval_X;
        _ = SatisfyingAssignment::<E>::process_round(
            &mut state,
            &pk.vc_shape,
            &pk.vc_ck,
            &verifier_circuit,
            (num_rounds_x + 1) + num_rounds_y,
            &mut transcript,
        )?;

        // Process the dedicated commit-only round for eval_W
        let eval_w_commit_round = num_rounds_x + 1 + num_rounds_y + 1;
        let _ = SatisfyingAssignment::<E>::process_round(
            &mut state,
            &pk.vc_shape,
            &pk.vc_ck,
            &verifier_circuit,
            eval_w_commit_round,
            &mut transcript,
        )?;

        // Finalize multi-round witness and construct NIFS proof
        let (_nifs_span, nifs_t) = start_span!("finalize_and_nifs");
        let (U_verifier, W_verifier) =
            SatisfyingAssignment::<E>::finalize_multiround_witness(&mut state, &pk.vc_shape)?;
        // Use the instance as produced by witness finalization; its public values
        // are exactly those absorbed during round 0 by the prover.
        let U_verifier_regular = U_verifier.to_regular_instance()?;
        let S_verifier = &pk.vc_shape_regular;

        // Sample fresh random instance/witness for ZK (must be done per-prove to preserve zero-knowledge).
        let (random_U, random_W) = S_verifier.sample_random_instance_witness(&pk.vc_ck)?;
        let (nifs, folded_W, folded_u, folded_X) = NovaNIFS::<E>::prove(
            &pk.vc_ck,
            S_verifier,
            &random_U,
            &random_W,
            &U_verifier_regular,
            &W_verifier,
            &mut transcript,
        )?;
        // Prove satisfiability of the folded VC instance via relaxed R1CS Spartan
        let relaxed_snark = crate::spartan_relaxed::RelaxedR1CSSpartanProof::prove(
            S_verifier,
            &pk.vc_ck,
            &folded_u,
            &folded_X,
            &folded_W,
            &mut transcript,
        )?;
        info!(elapsed_ms = %nifs_t.elapsed().as_millis(), "finalize_and_nifs");

        // prove the claimed polynomial evaluation at point r_y[1..]
        let (_pcs_span, pcs_t) = start_span!("pcs_prove");
        let eval_arg = E::PCS::prove(
            &pk.ck,
            &pk.vc_ck,
            &mut transcript,
            &U_regular.comm_W,
            &prep_snark.ps.W,
            &r_W,
            &r_y[1..],
            &U_verifier.comm_w_per_round[eval_w_commit_round],
            &state.r_w_per_round[eval_w_commit_round],
        )?;
        info!(elapsed_ms = %pcs_t.elapsed().as_millis(), "pcs_prove");

        info!(elapsed_ms = %prove_t.elapsed().as_millis(), "spartan_zk_prove");
        // Return proof and updated prep state (deterministic caches only)
        let updated_prep = VegaPrepZkSNARK {
            ps: prep_snark.ps,
            cached_az: prep_snark.cached_az,
            cached_bz: prep_snark.cached_bz,
            cached_cz: prep_snark.cached_cz,
            cached_rest_witness: prep_snark.cached_rest_witness,
            cached_rest_msm: prep_snark.cached_rest_msm,
            scratch_az,
            scratch_bz,
            scratch_cz,
            z_buffer,
            evals_rx_buffer,
        };
        Ok((
            VegaZkSNARK {
                U_verifier,
                nifs,
                random_U,
                relaxed_snark,
                eval_arg,
                U,
            },
            updated_prep,
        ))
    }

    /// verifies a proof of satisfiability of a `RelaxedR1CS` instance
    fn verify(&self, vk: &Self::VerifierKey) -> Result<Vec<E::Scalar>, VegaError> {
        // Verify by checking the multi-round verifier instance via NIFS folding
        let (_verify_span, verify_t) = start_span!("spartan_zk_verify");
        let mut transcript = E::TE::new(b"VegaZkSNARK");
        transcript.absorb(b"vk", &vk.digest()?);
        transcript.absorb(b"public_values", &self.U.public_values.as_slice());

        // Validate the provided split R1CS instance and advance the transcript
        self.U.validate(&vk.S, &mut transcript)?;

        // Recreate tau polynomial coefficients via Fiat-Shamir and advance transcript
        let (_tau_span, tau_t) = start_span!("compute_tau_verify");
        let num_rounds_x = vk.S.num_cons.log_2();
        let tau = (0..num_rounds_x)
            .map(|_| transcript.squeeze(b"t"))
            .collect::<Result<EqPolynomial<_>, VegaError>>()?;
        info!(elapsed_ms = %tau_t.elapsed().as_millis(), "compute_tau_verify");
        // validate the provided multi-round verifier instance and advance transcript
        self.U_verifier.validate(&vk.vc_shape, &mut transcript)?;

        // Derive expected challenge counts from the original shape sizes
        let num_vars = vk.S.num_shared + vk.S.num_precommitted + vk.S.num_rest;
        let num_rounds_x = vk.S.num_cons.log_2();
        let num_rounds_y = num_vars.log_2() + 1;

        let U_verifier_regular = self.U_verifier.to_regular_instance()?;

        let num_public_values = 3usize;
        let num_challenges = num_rounds_x + 1 + num_rounds_y;

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
        let public_values = &U_verifier_regular.X[num_challenges..num_challenges + 3];

        let r_x = challenges[0..num_rounds_x].to_vec();
        let r = challenges[num_rounds_x]; // r for combining inner claims
        let r_y = challenges[num_rounds_x + 1..].to_vec();

        // compute eval_A, eval_B, eval_C at (r_x, r_y)
        let (_matrix_eval_span, matrix_eval_t) = start_span!("matrix_evaluations");
        let T_x = EqPolynomial::evals_from_points(&r_x);
        let T_y = EqPolynomial::evals_from_points(&r_y);
        let (eval_A, eval_B, eval_C) = vk.S.evaluate_with_tables_fast(&T_x, &T_y);
        let quotient = eval_A + r * eval_B + r * r * eval_C;
        info!(elapsed_ms = %matrix_eval_t.elapsed().as_millis(), "matrix_evaluations");
        // Recompute eval_X from original circuit public IO at r_y[1..]
        let U_regular = self.U.to_regular_instance()?;

        let eval_X = {
            let X = vec![E::Scalar::ONE]
                .into_iter()
                .chain(U_regular.X.iter().cloned())
                .collect::<Vec<E::Scalar>>();
            let num_vars = vk.S.num_shared + vk.S.num_precommitted + vk.S.num_rest;
            SparsePolynomial::new(num_vars.log_2(), X).evaluate(&r_y[1..])
        };

        // Recompute tau(r_x) using the same tau polynomial challenges
        let tau_at_rx = tau.evaluate(&r_x);

        // Compare against the instance's public inputs [tau_at_rx, eval_X, quotient]
        if public_values[0] != tau_at_rx
            || public_values[1] != eval_X
            || public_values[2] != quotient
        {
            return Err(VegaError::ProofVerifyError {
        reason:
          "Verifier instance public values do not match recomputed evaluations (tau_at_rx, eval_X, quotient)"
            .to_string(),
      });
        }

        // Finally, run NIFS verification using the same transcript
        let (_nifs_verify_span, nifs_verify_t) = start_span!("nifs_verify");
        let folded_U = self
            .nifs
            .verify(&mut transcript, &self.random_U, &U_verifier_regular)?;

        // Verify the relaxed R1CS Spartan proof of the folded instance
        self.relaxed_snark
            .verify(&vk.vc_shape_regular, &vk.vc_vk, &folded_U, &mut transcript)
            .map_err(|e| VegaError::ProofVerifyError {
                reason: format!("Relaxed Spartan verify failed: {e}"),
            })?;
        info!(elapsed_ms = %nifs_verify_t.elapsed().as_millis(), "nifs_verify");

        // Continue with PCS verification on the same transcript
        // Use the commitment from the dedicated eval_W commit-only last round
        let (_pcs_verify_span, pcs_verify_t) = start_span!("pcs_verify");
        let eval_w_commit_round = num_rounds_x + 1 + num_rounds_y + 1;
        E::PCS::verify(
            &vk.vk_ee,
            &vk.vc_ck,
            &mut transcript,
            &U_regular.comm_W,
            &r_y[1..],
            &self.U_verifier.comm_w_per_round[eval_w_commit_round],
            &self.eval_arg,
        )?;
        info!(elapsed_ms = %pcs_verify_t.elapsed().as_millis(), "pcs_verify");

        info!(elapsed_ms = %verify_t.elapsed().as_millis(), "spartan_zk_verify");
        // Return original circuit public IO carried in the proof
        Ok(self.U.public_values.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bellpepper_core::{ConstraintSystem, SynthesisError, num::AllocatedNum};
    use tracing_subscriber::EnvFilter;

    #[cfg(feature = "jem")]
    use tikv_jemallocator::Jemalloc;
    #[cfg(feature = "jem")]
    #[global_allocator]
    static GLOBAL: Jemalloc = tikv_jemallocator::Jemalloc;

    #[derive(Clone, Debug, Default)]
    struct CubicCircuit {}

    impl<E: Engine> VegaCircuit<E> for CubicCircuit {
        fn public_values(&self) -> Result<Vec<<E as Engine>::Scalar>, SynthesisError> {
            Ok(vec![E::Scalar::from(15u64)])
        }

        fn shared<CS: ConstraintSystem<E::Scalar>>(
            &self,
            _: &mut CS,
        ) -> Result<Vec<AllocatedNum<E::Scalar>>, SynthesisError> {
            // In this example, we do not have shared variables.
            Ok(vec![])
        }

        fn precommitted<CS: ConstraintSystem<<E as Engine>::Scalar>>(
            &self,
            _: &mut CS,
            _: &[AllocatedNum<E::Scalar>], // shared variables, if any
        ) -> Result<Vec<AllocatedNum<<E as Engine>::Scalar>>, SynthesisError> {
            // In this example, we do not have precommitted variables.
            Ok(vec![])
        }

        fn num_challenges(&self) -> usize {
            // In this example, we do not use challenges.
            0
        }

        fn synthesize<CS: ConstraintSystem<E::Scalar>>(
            &self,
            cs: &mut CS,
            _: &[AllocatedNum<E::Scalar>],
            _: &[AllocatedNum<E::Scalar>],
            _: Option<&[E::Scalar]>,
        ) -> Result<(), SynthesisError> {
            // Consider a cubic equation: `x^3 + x + 5 = y`, where `x` and `y` are respectively the input and output.
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

            let _ = y.inputize(cs.namespace(|| "output"));

            Ok(())
        }
    }

    #[test]
    fn test_zksnark() {
        crate::iroha_rng::with_deterministic_test_seed(0x60, || {
            let _ = tracing_subscriber::fmt()
                .with_target(false)
                .with_ansi(true) // no bold colour codes
                .with_env_filter(EnvFilter::from_default_env())
                .try_init();

            type E = crate::provider::PallasHyraxEngine;
            type S = VegaZkSNARK<E>;
            test_zksnark_with::<E, S>();

            type E2 = crate::provider::T256HyraxEngine;
            type S2 = VegaZkSNARK<E2>;
            test_zksnark_with::<E2, S2>();
        });
    }

    fn test_zksnark_with<E: Engine, S: R1CSSNARKTrait<E>>() {
        let circuit = CubicCircuit::default();

        // produce keys
        let (pk, vk) = S::setup(circuit.clone()).unwrap();

        // generate pre-processed state for proving
        let prep_snark = S::prep_prove(&pk, circuit.clone(), false).unwrap();

        // generate a witness and proof
        let res = S::prove(&pk, circuit.clone(), prep_snark, false);
        assert!(res.is_ok());
        let (snark, _prep_snark) = res.unwrap();

        // verify the SNARK
        let res = snark.verify(&vk);
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), [<E as Engine>::Scalar::from(15u64)])
    }
}
