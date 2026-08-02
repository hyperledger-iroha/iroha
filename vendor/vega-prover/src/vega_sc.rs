// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! Single-circuit (SC) prover. Implements the Spartan SNARK protocol (https://eprint.iacr.org/2019/550).
use crate::{
    Blind, CommitmentKey,
    bellpepper::{
        r1cs::{PrecommittedState, VegaShape, VegaWitness},
        shape_cs::ShapeCS,
        solver::SatisfyingAssignment,
    },
    big_num::DelayedReduction,
    digest::DigestComputer,
    errors::VegaError,
    math::Math,
    polys::{
        eq::EqPolynomial,
        multilinear::{MultilinearPolynomial, SparsePolynomial},
        univariate::UniPoly,
    },
    r1cs::{SplitR1CSInstance, SplitR1CSShape},
    start_span,
    sumcheck::SumcheckProof,
    traits::{
        Engine,
        circuit::VegaCircuit,
        pcs::PCSEngineTrait,
        snark::{DigestHelperTrait, R1CSSNARKTrait, VegaDigest},
        transcript::TranscriptEngineTrait,
    },
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
    ck_s: CommitmentKey<E>,
    S: SplitR1CSShape<E>,
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
    ck_s: CommitmentKey<E>,
    S: SplitR1CSShape<E>,
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
        config
            .serialize_into(&mut *w, &self.ck_s)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        self.S.write_bytes(w)?;
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
pub struct VegaPrepSNARK<E: Engine> {
    ps: PrecommittedState<E>,
    // Cached partial matrix-vector products for precommitted witness columns (deterministic)
    cached_az: Vec<E::Scalar>,
    cached_bz: Vec<E::Scalar>,
    cached_cz: Vec<E::Scalar>,
    // Lazily cached rest-witness commitment (populated on first prove call, deterministic)
    cached_rest_witness: Option<Vec<E::Scalar>>,
    cached_rest_msm: Option<Vec<E::GE>>,
    // Pre-allocated scratch buffers (reused across prove calls, avoids mmap + page faults)
    scratch_az: Vec<E::Scalar>,
    scratch_bz: Vec<E::Scalar>,
    scratch_cz: Vec<E::Scalar>,
    z_buffer: Vec<E::Scalar>,
    evals_rx_buffer: Vec<E::Scalar>,
}

/// A succinct proof of knowledge of a witness to a relaxed R1CS instance
/// The proof is produced using Spartan's combination of the sum-check and
/// the commitment to a vector viewed as a polynomial commitment
#[derive(Debug, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct VegaSNARK<E: Engine> {
    U: SplitR1CSInstance<E>,
    sc_proof_outer: SumcheckProof<E>,
    claims_outer: (E::Scalar, E::Scalar, E::Scalar),
    sc_proof_inner: SumcheckProof<E>,
    eval_W: E::Scalar,
    blind_eval_W: Blind<E>, // it is okay to send the blind since we are targeting non-zk
    eval_arg: <E::PCS as PCSEngineTrait<E>>::EvaluationArgument,
}

impl<E: Engine> R1CSSNARKTrait<E> for VegaSNARK<E> {
    type ProverKey = VegaProverKey<E>;
    type VerifierKey = VegaVerifierKey<E>;
    type PrepSNARK = VegaPrepSNARK<E>;

    fn setup<C: VegaCircuit<E>>(
        circuit: C,
    ) -> Result<(Self::ProverKey, Self::VerifierKey), VegaError> {
        let S = ShapeCS::r1cs_shape(&circuit)?;
        let (ck, vk_ee) = SplitR1CSShape::commitment_key(&[&S])?;
        E::PCS::precompute_ck(&ck);
        let (ck_s, _) = E::PCS::setup(b"ck_s", 1, 1); // 1 base for committing a single scalar
        E::PCS::precompute_ck(&ck_s);

        let vk: VegaVerifierKey<E> = VegaVerifierKey {
            S: S.clone(),
            vk_ee,
            ck_s: ck_s.clone(),
            digest: OnceCell::new(),
        };

        let vk_digest = vk.digest()?;
        let pk = Self::ProverKey {
            ck,
            ck_s,
            S,
            vk_digest,
        };

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

        Ok(VegaPrepSNARK {
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
        })
    }

    /// produces a succinct proof of satisfiability of an R1CS instance
    fn prove<C: VegaCircuit<E>>(
        pk: &Self::ProverKey,
        circuit: C,
        mut prep_snark: Self::PrepSNARK,
        is_small: bool,
    ) -> Result<(Self, Self::PrepSNARK), VegaError> {
        let (_prove_span, prove_t) = start_span!("spartan_snark_prove");
        let mut transcript = E::TE::new(b"VegaSNARK");
        transcript.absorb(b"vk", &pk.vk_digest);

        let public_values = circuit
            .public_values()
            .map_err(|e| VegaError::SynthesisError {
                reason: format!("Circuit does not provide public IO: {e}"),
            })?;

        // absorb the public values into the transcript
        transcript.absorb(b"public_values", &public_values.as_slice());

        let (U, W) = SatisfyingAssignment::r1cs_instance_and_witness(
            &mut prep_snark.ps,
            &pk.S,
            &pk.ck,
            &circuit,
            is_small,
            &mut transcript,
        )?;

        // Build z using pre-allocated buffer (avoids mmap + page faults after first prove)
        let mut z = std::mem::take(&mut prep_snark.z_buffer);
        z.clear();
        z.extend_from_slice(&W.W);
        z.push(E::Scalar::ONE);
        z.extend_from_slice(&U.public_values);
        z.extend_from_slice(&U.challenges);

        let num_vars = pk.S.num_shared + pk.S.num_precommitted + pk.S.num_rest;
        let (num_rounds_x, num_rounds_y) = (
            usize::try_from(pk.S.num_cons.ilog2()).unwrap(),
            (usize::try_from(num_vars.ilog2()).unwrap() + 1),
        );

        // outer sum-check preparation
        let tau = (0..num_rounds_x)
            .map(|_i| transcript.squeeze(b"t"))
            .collect::<Result<Vec<_>, VegaError>>()?;

        // Use incremental matvec with cached precommitted products and scratch buffers
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
        // outer sum-check

        let (_sc_span, sc_t) = start_span!("outer_sumcheck");
        let (sc_proof_outer, r_x, claims_outer) = SumcheckProof::prove_cubic_with_three_inputs(
            &E::Scalar::ZERO, // claim is zero
            tau,
            &mut poly_Az,
            &mut poly_Bz,
            &mut poly_Cz,
            &mut transcript,
        )?;

        // claims from the end of sum-check
        let (claim_Az, claim_Bz, claim_Cz): (E::Scalar, E::Scalar, E::Scalar) =
            (claims_outer[0], claims_outer[1], claims_outer[2]);
        transcript.absorb(b"claims_outer", &[claim_Az, claim_Bz, claim_Cz].as_slice());
        info!(elapsed_ms = %sc_t.elapsed().as_millis(), "outer_sumcheck");
        // Recover scratch buffers (bound down to 1 element, but allocation preserved)
        let scratch_az = poly_Az.into_vec();
        let scratch_bz = poly_Bz.into_vec();
        let scratch_cz = poly_Cz.into_vec();
        // inner sum-check preparation
        let (_r_span, r_t) = start_span!("prepare_inner_claims");
        let r = transcript.squeeze(b"r")?;
        let claim_inner_joint = claim_Az + r * claim_Bz + r * r * claim_Cz;
        info!(elapsed_ms = %r_t.elapsed().as_millis(), "prepare_inner_claims");

        // Merged: compute eq(r_x), bind row variables, and prepare poly_ABC in a single pipeline
        let (_abc_span, abc_t) = start_span!("prepare_poly_ABC");
        let mut evals_rx_buffer = std::mem::take(&mut prep_snark.evals_rx_buffer);
        EqPolynomial::evals_from_points_into(&r_x, &mut evals_rx_buffer);
        let mut poly_ABC_vec = pk.S.bind_and_prepare_poly_ABC(&evals_rx_buffer, &r);
        info!(elapsed_ms = %abc_t.elapsed().as_millis(), "prepare_poly_ABC");
        // inner sum-check with manual first round (BDDT optimization)

        let (_sc2_span, sc2_t) = start_span!("inner_sumcheck");
        // Manual first round: the "virtual" polynomial pair is:
        //   ABC_low[j] = poly_ABC_vec[j] for j=0..num_vars-1
        //   ABC_high[j] = poly_ABC_vec[num_vars+j] for j=0..num_extra-1, else 0
        //   z_low[j] = z[j], z_high[j] = z[num_vars+j] for j=0..num_extra-1, else 0
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

        // Compute corrections for num_extra terms where high values are non-zero
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
        let evals_r0 = vec![eval0, claim_inner_joint - eval0, eval2];
        let inner_r0_poly = UniPoly::from_evals(&evals_r0)?;

        // Append round-0 polynomial to transcript (matching prove_quad protocol)
        transcript.absorb(b"p", &inner_r0_poly);
        let r0_inner = transcript.squeeze(b"c")?;
        let claim_after_r0 = inner_r0_poly.evaluate(&r0_inner);

        // Fused bind: for j < num_extra standard bind; for j >= num_extra both scale by (1-r0)
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
        let mut poly_z = MultilinearPolynomial::new(z);
        let (sc_proof_inner, r_y_rest, _claims_inner) = SumcheckProof::prove_quad(
            &claim_after_r0,
            num_rounds_y - 1,
            &mut MultilinearPolynomial::new(poly_ABC_vec),
            &mut poly_z,
            &mut transcript,
        )?;
        // Recover the allocation from poly_z for reuse across prove calls.
        let z_buffer = poly_z.Z;

        // Reconstruct full r_y and prepend round-0 to inner sumcheck proof
        let mut r_y = Vec::with_capacity(num_rounds_y);
        r_y.push(r0_inner);
        r_y.extend_from_slice(&r_y_rest);

        // Prepend the manual round-0 polynomial to the inner sumcheck proof
        let sc_proof_inner = sc_proof_inner.prepend_round(inner_r0_poly);

        // eval_Z: claims_inner[1] is the z polynomial evaluated at all inner sumcheck challenges,
        // which equals z evaluated at r_y (since the manual round-0 binding was applied first).
        let eval_Z = _claims_inner[1];
        info!(elapsed_ms = %sc2_t.elapsed().as_millis(), "inner_sumcheck");

        // Compute eval_W = (eval_Z - r_y[0] * eval_X) / (1 - r_y[0]) because Z = (W, 1, X)
        let U_regular = U.to_regular_instance()?;
        let eval_X = {
            let X = vec![E::Scalar::ONE]
                .into_iter()
                .chain(U_regular.X.iter().cloned())
                .collect::<Vec<E::Scalar>>();
            SparsePolynomial::new(num_rounds_y - 1, X).evaluate(&r_y[1..])
        };
        let inv: Option<E::Scalar> = (E::Scalar::ONE - r_y[0]).invert().into();
        let eval_W = (eval_Z - r_y[0] * eval_X) * inv.ok_or(VegaError::DivisionByZero)?;

        let (_pcs_span, pcs_t) = start_span!("pcs_prove");
        let blind_eval_W = E::PCS::blind(&pk.ck_s, 1);
        let comm_eval_W = E::PCS::commit(&pk.ck_s, &[eval_W], &blind_eval_W, false)?;
        let eval_arg = E::PCS::prove(
            &pk.ck,
            &pk.ck_s,
            &mut transcript,
            &U_regular.comm_W,
            &W.W,
            &W.r_W,
            &r_y[1..],
            &comm_eval_W,
            &blind_eval_W,
        )?;
        info!(elapsed_ms = %pcs_t.elapsed().as_millis(), "pcs_prove");

        info!(elapsed_ms = %prove_t.elapsed().as_millis(), "spartan_snark_prove");
        // Return proof and updated prep state with preserved scratch buffers
        let updated_prep = VegaPrepSNARK {
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
            VegaSNARK {
                U,
                sc_proof_outer,
                claims_outer: (claim_Az, claim_Bz, claim_Cz),
                sc_proof_inner,
                eval_W,
                blind_eval_W,
                eval_arg,
            },
            updated_prep,
        ))
    }

    /// verifies a proof of satisfiability of a `RelaxedR1CS` instance
    fn verify(&self, vk: &Self::VerifierKey) -> Result<Vec<E::Scalar>, VegaError> {
        let (_verify_span, verify_t) = start_span!("spartan_snark_verify");
        let mut transcript = E::TE::new(b"VegaSNARK");

        // append the digest of R1CS matrices
        transcript.absorb(b"vk", &vk.digest()?);
        transcript.absorb(b"public_values", &self.U.public_values.as_slice());

        // validate the provided split R1CS instance and convert to regular instance
        self.U.validate(&vk.S, &mut transcript)?;
        let U_regular = self.U.to_regular_instance()?;

        let num_vars = vk.S.num_shared + vk.S.num_precommitted + vk.S.num_rest;

        let (num_rounds_x, num_rounds_y) = (
            usize::try_from(vk.S.num_cons.ilog2()).unwrap(),
            (usize::try_from(num_vars.ilog2()).unwrap() + 1),
        );

        info!(
            "Verifying R1CS SNARK with {} rounds for outer sum-check and {} rounds for inner sum-check",
            num_rounds_x, num_rounds_y
        );

        // outer sum-check
        let (_tau_span, tau_t) = start_span!("compute_tau_verify");
        let tau = (0..num_rounds_x)
            .map(|_i| transcript.squeeze(b"t"))
            .collect::<Result<EqPolynomial<_>, VegaError>>()?;
        info!(elapsed_ms = %tau_t.elapsed().as_millis(), "compute_tau_verify");

        let (_outer_sumcheck_span, outer_sumcheck_t) = start_span!("outer_sumcheck_verify");
        let (claim_outer_final, r_x) =
            self.sc_proof_outer
                .verify(E::Scalar::ZERO, num_rounds_x, 3, &mut transcript)?;

        // verify claim_outer_final
        let (claim_Az, claim_Bz, claim_Cz) = self.claims_outer;
        let taus_bound_rx = tau.evaluate(&r_x);
        let claim_outer_final_expected = taus_bound_rx * (claim_Az * claim_Bz - claim_Cz);
        if claim_outer_final != claim_outer_final_expected {
            return Err(VegaError::InvalidSumcheckProof);
        }
        info!(elapsed_ms = %outer_sumcheck_t.elapsed().as_millis(), "outer_sumcheck_verify");

        transcript.absorb(
            b"claims_outer",
            &[
                self.claims_outer.0,
                self.claims_outer.1,
                self.claims_outer.2,
            ]
            .as_slice(),
        );

        // inner sum-check
        let (_inner_sumcheck_span, inner_sumcheck_t) = start_span!("inner_sumcheck_verify");
        let r = transcript.squeeze(b"r")?;
        let claim_inner_joint =
            self.claims_outer.0 + r * self.claims_outer.1 + r * r * self.claims_outer.2;

        let (claim_inner_final, r_y) =
            self.sc_proof_inner
                .verify(claim_inner_joint, num_rounds_y, 2, &mut transcript)?;

        // verify claim_inner_final
        let eval_Z = {
            let eval_X = {
                // public IO is (1, X)
                let X = vec![E::Scalar::ONE]
                    .into_iter()
                    .chain(U_regular.X.iter().cloned())
                    .collect::<Vec<E::Scalar>>();
                SparsePolynomial::new(num_vars.log_2(), X).evaluate(&r_y[1..])
            };
            (E::Scalar::ONE - r_y[0]) * self.eval_W + r_y[0] * eval_X
        };

        // compute evaluations of R1CS matrices
        let (_matrix_eval_span, matrix_eval_t) = start_span!("matrix_evaluations");
        let T_x = EqPolynomial::evals_from_points(&r_x);
        let T_y = EqPolynomial::evals_from_points(&r_y);
        let (eval_A, eval_B, eval_C) = vk.S.evaluate_with_tables_fast(&T_x, &T_y);

        let claim_inner_final_expected = (eval_A + r * eval_B + r * r * eval_C) * eval_Z;
        if claim_inner_final != claim_inner_final_expected {
            return Err(VegaError::InvalidSumcheckProof);
        }
        info!(elapsed_ms = %matrix_eval_t.elapsed().as_millis(), "matrix_evaluations");
        info!(elapsed_ms = %inner_sumcheck_t.elapsed().as_millis(), "inner_sumcheck_verify");

        // verify
        let (_pcs_verify_span, pcs_verify_t) = start_span!("pcs_verify");
        let comm_eval_W = E::PCS::commit(&vk.ck_s, &[self.eval_W], &self.blind_eval_W, false)?; // commitment to eval_W
        E::PCS::verify(
            &vk.vk_ee,
            &vk.ck_s,
            &mut transcript,
            &U_regular.comm_W,
            &r_y[1..],
            &comm_eval_W,
            &self.eval_arg,
        )?;
        info!(elapsed_ms = %pcs_verify_t.elapsed().as_millis(), "pcs_verify");

        info!(elapsed_ms = %verify_t.elapsed().as_millis(), "spartan_snark_verify");
        Ok(self.U.public_values.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bellpepper_core::{ConstraintSystem, SynthesisError, num::AllocatedNum};
    use tracing_subscriber::EnvFilter;

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
    fn test_snark() {
        let _ = tracing_subscriber::fmt()
            .with_target(false)
            .with_ansi(true) // no bold colour codes
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();

        type E = crate::provider::PallasHyraxEngine;
        type S = VegaSNARK<E>;
        test_snark_with::<E, S>();

        type E2 = crate::provider::T256HyraxEngine;
        type S2 = VegaSNARK<E2>;
        test_snark_with::<E2, S2>();
    }

    fn test_snark_with<E: Engine, S: R1CSSNARKTrait<E>>() {
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

    #[test]
    fn test_validate_rejects_overlong_public_io() {
        type E = crate::provider::PallasHyraxEngine;
        let circuit = CubicCircuit::default();

        let (pk, vk) = VegaSNARK::<E>::setup(circuit.clone()).unwrap();
        let prep_snark = VegaSNARK::<E>::prep_prove(&pk, circuit.clone(), false).unwrap();
        let (mut snark, _prep_snark) =
            VegaSNARK::<E>::prove(&pk, circuit, prep_snark, false).unwrap();

        // a well-formed instance passes validation
        let mut transcript = <E as Engine>::TE::new(b"test");
        assert!(snark.U.validate(&vk.S, &mut transcript).is_ok());

        // a `public_values` length that does not match `num_public` is rejected
        snark
            .U
            .public_values
            .push(<E as Engine>::Scalar::from(42u64));
        let mut transcript = <E as Engine>::TE::new(b"test");
        assert!(snark.U.validate(&vk.S, &mut transcript).is_err());
    }

    #[test]
    fn test_validate_rejects_commitment_for_empty_segment() {
        type E = crate::provider::PallasHyraxEngine;
        let circuit = CubicCircuit::default();

        let (pk, vk) = VegaSNARK::<E>::setup(circuit.clone()).unwrap();
        let prep_snark = VegaSNARK::<E>::prep_prove(&pk, circuit.clone(), false).unwrap();
        let (mut snark, _prep_snark) =
            VegaSNARK::<E>::prove(&pk, circuit, prep_snark, false).unwrap();

        // this circuit has no shared segment
        assert_eq!(vk.S.num_shared, 0);

        // a well-formed instance passes validation
        let mut transcript = <E as Engine>::TE::new(b"test");
        assert!(snark.U.validate(&vk.S, &mut transcript).is_ok());

        // a commitment supplied for the empty shared segment is rejected
        snark.U.comm_W_shared = Some(snark.U.comm_W_rest.clone());
        let mut transcript = <E as Engine>::TE::new(b"test");
        assert!(snark.U.validate(&vk.S, &mut transcript).is_err());
    }
}
