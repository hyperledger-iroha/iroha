//! Final streamed Hyrax opening and linear IPA for governed Figure 9 proofs.
//!
//! The million-scalar application polynomial is never materialized.  This
//! stage folds each application row directly into the 2,048-scalar Hyrax bound
//! row, proves the exact linear inner-product argument with the continued
//! proof-scoped ChaCha12 stream, assembles the Microsoft wire, and accepts the
//! encoding only after the complete first-party verifier replays it.

use super::super::{
    VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
    algebra::{eq_evals, inner_product},
    commitment::{Commitment, CommitmentKey, fold, msm},
    transcript::VegaTranscriptV1,
};
use super::{
    Figure9FinalOpeningError,
    prover_key::McProverKeyWire,
    relaxed_spartan::GovernedFigure9RelaxedSpartanPrep,
    rng::Figure9StdRng,
    semantic_engine::GovernedFigure9SemanticPrep,
    verifier_key::{McVerifierKeyWire, SplitShapeWire},
    verify,
    wire::{LinearIpaWire, McProofWire},
};

const FIGURE9_STEPS: usize = 8;
const APPLICATION_VARIABLES: usize = 1 << 20;
const APPLICATION_COMMITMENT_WIDTH: usize = 2_048;
const APPLICATION_ROWS: usize = APPLICATION_VARIABLES / APPLICATION_COMMITMENT_WIDTH;
const OPENING_POINT_VARIABLES: usize = 20;
const ROW_POINT_VARIABLES: usize = 9;
const COLUMN_POINT_VARIABLES: usize = 11;
const VERIFIER_COMMITMENT_WIDTH: usize = 32;
const VERIFIER_ROUNDS: usize = 47;
const STEP_EVALUATION_ROUND: usize = 45;
const CORE_EVALUATION_ROUND: usize = 46;
const FINAL_LINEAR_IPA_SCALAR_DRAWS: usize = APPLICATION_COMMITMENT_WIDTH + 2;
#[cfg(test)]
const FULL_PROOF_SCALAR_DRAWS: usize = 2_560 + 47 + 1_633 + FINAL_LINEAR_IPA_SCALAR_DRAWS;

#[cfg(test)]
const VEGA_HYRAX_SOURCE_SHA256: &str =
    "bc803e643c67677c76c7eeca04f35000f68c6b94f7f2d14903d0d71431231f92";
#[cfg(test)]
const VEGA_LINEAR_IPA_SOURCE_SHA256: &str =
    "a650c7a1845e9f3a33a471f79d8a35fb3dcbbf4bb04efade653b86985d730e3b";
#[cfg(test)]
const PYVEGA_FINISH_SOURCE_SHA256: &str =
    "5b678f5a058ce4314ce7bb7b046073d64419744cecb7aa8ad68a1e6f369180de";

/// Move-only, exactly-sized scalar allocation erased on every non-public path.
struct SecretScalars(Vec<Scalar>);

impl SecretScalars {
    fn try_with_capacity(capacity: usize) -> Result<Self, Figure9FinalOpeningError> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(capacity)
            .map_err(|_| Figure9FinalOpeningError::ResourceExhausted)?;
        Ok(Self(values))
    }

    fn try_zeroed(len: usize) -> Result<Self, Figure9FinalOpeningError> {
        let mut values = Self::try_with_capacity(len)?;
        values.0.resize(len, Scalar::zero());
        Ok(values)
    }

    fn push(&mut self, mut value: Scalar) -> Result<(), Figure9FinalOpeningError> {
        if self.0.len() >= self.0.capacity() {
            value.clear_secret();
            return Err(Figure9FinalOpeningError::InvalidShape);
        }
        self.0.push(value);
        value.clear_secret();
        Ok(())
    }

    fn as_slice(&self) -> &[Scalar] {
        &self.0
    }

    fn as_mut_slice(&mut self) -> &mut [Scalar] {
        &mut self.0
    }

    fn into_vec(mut self) -> Vec<Scalar> {
        core::mem::take(&mut self.0)
    }
}

impl Drop for SecretScalars {
    fn drop(&mut self) {
        clear_secret_scalars(&mut self.0);
    }
}

/// Stack scalar erased when its derived secret is no longer needed.
struct SecretScalar(Scalar);

impl SecretScalar {
    fn new(value: Scalar) -> Self {
        Self(value)
    }

    fn get(&self) -> Scalar {
        self.0
    }

    fn add_assign(&mut self, mut value: Scalar) {
        self.0 += value;
        value.clear_secret();
    }
}

impl Drop for SecretScalar {
    fn drop(&mut self) {
        self.0.clear_secret();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut self.0);
    }
}

/// A linear-IPA wire whose responses are wiped unless explicitly published.
struct PendingLinearIpa(Option<LinearIpaWire>);

impl PendingLinearIpa {
    fn new(wire: LinearIpaWire) -> Self {
        Self(Some(wire))
    }

    fn as_wire(&self) -> Result<&LinearIpaWire, Figure9FinalOpeningError> {
        self.0
            .as_ref()
            .ok_or(Figure9FinalOpeningError::InvalidShape)
    }

    fn into_wire(mut self) -> Result<LinearIpaWire, Figure9FinalOpeningError> {
        self.0.take().ok_or(Figure9FinalOpeningError::InvalidShape)
    }
}

impl Drop for PendingLinearIpa {
    fn drop(&mut self) {
        if let Some(wire) = &mut self.0 {
            clear_secret_scalars(&mut wire.responses);
            wire.delta_response.clear_secret();
            wire.beta_response.clear_secret();
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
            #[cfg(test)]
            let _ = PENDING_IPA_ZEROIZED_DROPS.try_with(|drops| {
                drops.set(drops.get().saturating_add(1));
            });
        }
    }
}

/// A complete wire whose linear responses remain private until self-verification.
struct PendingProof(Option<McProofWire>);

impl PendingProof {
    fn new(proof: McProofWire) -> Self {
        Self(Some(proof))
    }

    fn as_proof(&self) -> Result<&McProofWire, Figure9FinalOpeningError> {
        self.0
            .as_ref()
            .ok_or(Figure9FinalOpeningError::InvalidShape)
    }

    fn publish(mut self) -> Result<McProofWire, Figure9FinalOpeningError> {
        self.0.take().ok_or(Figure9FinalOpeningError::InvalidShape)
    }
}

impl Drop for PendingProof {
    fn drop(&mut self) {
        if let Some(proof) = &mut self.0 {
            clear_secret_scalars(&mut proof.evaluation_argument.responses);
            proof.evaluation_argument.delta_response.clear_secret();
            proof.evaluation_argument.beta_response.clear_secret();
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        }
    }
}

/// Canonical bytes erased until their decoded proof has passed every equation.
struct PendingEncoding(Option<Vec<u8>>);

impl PendingEncoding {
    fn new(bytes: Vec<u8>) -> Self {
        Self(Some(bytes))
    }

    fn as_slice(&self) -> Result<&[u8], Figure9FinalOpeningError> {
        self.0
            .as_deref()
            .ok_or(Figure9FinalOpeningError::InvalidShape)
    }

    fn publish(mut self) -> Result<Vec<u8>, Figure9FinalOpeningError> {
        self.0.take().ok_or(Figure9FinalOpeningError::InvalidShape)
    }
}

impl Drop for PendingEncoding {
    fn drop(&mut self) {
        if let Some(bytes) = &mut self.0 {
            let bytes = core::hint::black_box(bytes.as_mut_slice());
            bytes.fill(0);
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
            let _ = core::hint::black_box(&mut *bytes);
            #[cfg(test)]
            let _ = PENDING_ENCODING_ZEROIZED_DROPS.try_with(|drops| {
                drops.set(drops.get().saturating_add(1));
            });
        }
    }
}

#[cfg(test)]
std::thread_local! {
    static PENDING_IPA_ZEROIZED_DROPS: core::cell::Cell<usize> = const {
        core::cell::Cell::new(0)
    };
    static PENDING_ENCODING_ZEROIZED_DROPS: core::cell::Cell<usize> = const {
        core::cell::Cell::new(0)
    };
}

/// Finish and self-verify one exact governed Figure 9 Microsoft proof wire.
pub(super) fn build(
    mut prepared: GovernedFigure9RelaxedSpartanPrep<'_>,
    prover_key: &McProverKeyWire,
    verifier_key: &McVerifierKeyWire,
) -> Result<Vec<u8>, Figure9FinalOpeningError> {
    let (application_key, evaluation_key) = validate_geometry(&prepared, prover_key, verifier_key)?;
    let mut transcript = prepared
        .transcript
        .take()
        .ok_or(Figure9FinalOpeningError::MissingState)?;
    let mut rng = prepared
        .rng
        .take()
        .ok_or(Figure9FinalOpeningError::MissingState)?;
    let semantic = &prepared.semantic;
    let application = &semantic.application;

    // The verifier-round evaluation commitments are already transcript-bound.
    // Squeeze their batching challenge before any final-stage randomness.
    let evaluation_batching = transcript
        .squeeze(b"c_eval")
        .map_err(|_| Figure9FinalOpeningError::Transcript)?;
    let mut core_wire = application.core_instance.clone();
    core_wire.shared = Some(application.shared_commitment.clone());
    let core_instance =
        verify::split_to_regular(&core_wire).map_err(|_| Figure9FinalOpeningError::InvalidShape)?;
    let polynomial_commitment = fold(
        &[
            &semantic.folded_step_instance.witness_commitment,
            &core_instance.witness_commitment,
        ],
        &[Scalar::one(), evaluation_batching],
    )
    .map_err(|_| Figure9FinalOpeningError::Commitment)?;
    let step_evaluation_commitment = semantic.verifier_instance.commitments[STEP_EVALUATION_ROUND]
        .to_local()
        .map_err(|_| Figure9FinalOpeningError::Commitment)?;
    let core_evaluation_commitment = semantic.verifier_instance.commitments[CORE_EVALUATION_ROUND]
        .to_local()
        .map_err(|_| Figure9FinalOpeningError::Commitment)?;
    let evaluation_commitment = fold(
        &[&step_evaluation_commitment, &core_evaluation_commitment],
        &[Scalar::one(), evaluation_batching],
    )
    .map_err(|_| Figure9FinalOpeningError::Commitment)?;
    let evaluation_blinding = SecretScalar::new(
        semantic.verifier_blindings[STEP_EVALUATION_ROUND]
            + evaluation_batching * semantic.verifier_blindings[CORE_EVALUATION_ROUND],
    );

    let opening_point = semantic
        .inner_point
        .get(1..)
        .ok_or(Figure9FinalOpeningError::InvalidShape)?;
    let (row_point, column_point) = opening_point.split_at(ROW_POINT_VARIABLES);
    let row_weights = eq_evals(row_point).map_err(|_| Figure9FinalOpeningError::InvalidShape)?;
    let column_weights =
        eq_evals(column_point).map_err(|_| Figure9FinalOpeningError::InvalidShape)?;
    let (bound_row, bound_blinding) = bind_streamed_application_row(
        semantic,
        &prover_key.step_shape,
        &prover_key.core_shape,
        evaluation_batching,
        &row_weights,
    )?;
    let evaluation = SecretScalar::new(
        inner_product(bound_row.as_slice(), &column_weights)
            .map_err(|_| Figure9FinalOpeningError::InvalidShape)?,
    );

    // Reconstruct both hidden commitments from the streamed witnesses before
    // allowing the final RNG stream to advance.
    let committed_row = msm(&row_weights, polynomial_commitment.points())
        .map_err(|_| Figure9FinalOpeningError::Commitment)?;
    let expected_row = application_key
        .commit(bound_row.as_slice(), &[bound_blinding.get()])
        .map_err(|_| Figure9FinalOpeningError::Commitment)?;
    if expected_row.points() != [committed_row] {
        return Err(Figure9FinalOpeningError::EvaluationMismatch);
    }
    let expected_evaluation = evaluation_key
        .commit(&[evaluation.get()], &[evaluation_blinding.get()])
        .map_err(|_| Figure9FinalOpeningError::Commitment)?;
    if expected_evaluation != evaluation_commitment {
        return Err(Figure9FinalOpeningError::EvaluationMismatch);
    }

    let mut replay_transcript = transcript.clone();
    let mut final_draws = 0_usize;
    let argument = prove_hyrax_opening(
        &application_key,
        &evaluation_key,
        &mut transcript,
        &polynomial_commitment,
        committed_row,
        &column_weights,
        evaluation_commitment.points()[0],
        bound_row.as_slice(),
        bound_blinding.get(),
        evaluation_blinding.get(),
        &mut rng,
        &mut final_draws,
    )?;
    if final_draws != FINAL_LINEAR_IPA_SCALAR_DRAWS {
        return Err(Figure9FinalOpeningError::InvalidShape);
    }
    verify::verify_hyrax_opening(
        &verifier_key.evaluation_key,
        &verifier_key.verifier_commitment_key,
        &mut replay_transcript,
        &polynomial_commitment,
        opening_point,
        &evaluation_commitment,
        argument.as_wire()?,
    )
    .map_err(|_| Figure9FinalOpeningError::SelfVerification)?;

    let expected_steps = application
        .step_instances
        .iter()
        .map(|instance| instance.public_values.clone())
        .collect::<Vec<_>>();
    let expected_core = application.core_instance.public_values.clone();
    // Finish every allocation that can unwind before publishing the response
    // scalars out of their zeroizing pending owner.
    let shared_commitment = application.shared_commitment.clone();
    let step_instances = application.step_instances.clone();
    let core_instance = application.core_instance.clone();
    let verifier_instance = semantic.verifier_instance.clone();
    let evaluation_argument = argument.into_wire()?;
    let proof = PendingProof::new(McProofWire {
        shared_commitment: Some(shared_commitment),
        step_instances,
        core_instance,
        evaluation_argument,
        verifier_instance,
        nova_cross_term: prepared.nova_cross_term,
        random_instance: prepared.random_instance,
        relaxed_spartan: prepared.relaxed_spartan,
    });
    let verified = verify::verify(proof.as_proof()?, verifier_key, FIGURE9_STEPS)
        .map_err(|_| Figure9FinalOpeningError::SelfVerification)?;
    if verified != (expected_steps.clone(), expected_core.clone()) {
        return Err(Figure9FinalOpeningError::SelfVerification);
    }
    let encoded = PendingEncoding::new(
        proof
            .as_proof()?
            .encode()
            .map_err(|_| Figure9FinalOpeningError::Encoding)?,
    );
    let decoded = PendingProof::new(
        McProofWire::decode(
            encoded.as_slice()?,
            &verifier_key
                .proof_dimensions()
                .map_err(|_| Figure9FinalOpeningError::InvalidKey)?,
        )
        .map_err(|_| Figure9FinalOpeningError::Encoding)?,
    );
    let reencoded = PendingEncoding::new(
        decoded
            .as_proof()?
            .encode()
            .map_err(|_| Figure9FinalOpeningError::Encoding)?,
    );
    if decoded.as_proof()? != proof.as_proof()?
        || reencoded.as_slice()? != encoded.as_slice()?
        || verify::verify(decoded.as_proof()?, verifier_key, FIGURE9_STEPS)
            .map_err(|_| Figure9FinalOpeningError::SelfVerification)?
            != (expected_steps, expected_core)
    {
        return Err(Figure9FinalOpeningError::SelfVerification);
    }
    let _original = proof.publish()?;
    let _decoded = decoded.publish()?;
    let _reencoded = reencoded.publish()?;
    encoded.publish()
}

fn validate_geometry(
    prepared: &GovernedFigure9RelaxedSpartanPrep<'_>,
    prover_key: &McProverKeyWire,
    verifier_key: &McVerifierKeyWire,
) -> Result<(CommitmentKey, CommitmentKey), Figure9FinalOpeningError> {
    prover_key
        .validate_against(verifier_key)
        .map_err(|_| Figure9FinalOpeningError::InvalidKey)?;
    let application_key = verify::derive_and_match_key(&prover_key.application_key)
        .map_err(|_| Figure9FinalOpeningError::InvalidKey)?;
    let evaluation_key = verify::derive_and_match_key(&prover_key.verifier_commitment_key)
        .map_err(|_| Figure9FinalOpeningError::InvalidKey)?;
    let semantic = &prepared.semantic;
    let application = &semantic.application;
    let dimensions = verifier_key
        .proof_dimensions()
        .map_err(|_| Figure9FinalOpeningError::InvalidKey)?;
    let weight_sum = semantic
        .folded_step_weights
        .iter()
        .copied()
        .fold(Scalar::zero(), |sum, weight| sum + weight);
    if application_key.columns() != APPLICATION_COMMITMENT_WIDTH
        || evaluation_key.columns() != VERIFIER_COMMITMENT_WIDTH
        || application.application_key.columns() != APPLICATION_COMMITMENT_WIDTH
        || application.application_key.generators() != application_key.generators()
        || application.application_key.hiding_generator() != application_key.hiding_generator()
        || prover_key.step_shape.variables().ok() != Some(APPLICATION_VARIABLES)
        || prover_key.core_shape.variables().ok() != Some(APPLICATION_VARIABLES)
        || dimensions.evaluation_response_scalars != APPLICATION_COMMITMENT_WIDTH
        || dimensions.verifier_round_commitment_points.len() != VERIFIER_ROUNDS
        || semantic.inner_point.len() != OPENING_POINT_VARIABLES + 1
        || semantic.inner_point[1 + ROW_POINT_VARIABLES..].len() != COLUMN_POINT_VARIABLES
        || semantic.folded_step_weights.len() != FIGURE9_STEPS
        || weight_sum != Scalar::one()
        || semantic.folded_step_blindings.len() != APPLICATION_ROWS
        || semantic.folded_step_instance.witness_commitment.len() != APPLICATION_ROWS
        || semantic.verifier_instance.commitments.len() != VERIFIER_ROUNDS
        || semantic.verifier_blindings.len() != VERIFIER_ROUNDS
        || semantic.verifier_instance.commitments[STEP_EVALUATION_ROUND]
            .points
            .len()
            != 1
        || semantic.verifier_instance.commitments[CORE_EVALUATION_ROUND]
            .points
            .len()
            != 1
        || application.shared_blindings.len()
            + application.core_private.precommitted_blindings.len()
            + application.core_private.rest_blindings.len()
            != APPLICATION_ROWS
        || application.step_instances.len() != FIGURE9_STEPS
        || application.step_private.len() != FIGURE9_STEPS
        || prepared.transcript.is_none()
        || prepared.rng.is_none()
    {
        return Err(Figure9FinalOpeningError::InvalidShape);
    }
    Ok((application_key, evaluation_key))
}

fn bind_streamed_application_row(
    semantic: &GovernedFigure9SemanticPrep<'_>,
    step_shape: &SplitShapeWire,
    core_shape: &SplitShapeWire,
    batching: Scalar,
    row_weights: &[Scalar],
) -> Result<(SecretScalars, SecretScalar), Figure9FinalOpeningError> {
    if row_weights.len() != APPLICATION_ROWS {
        return Err(Figure9FinalOpeningError::InvalidShape);
    }
    let mut bound = SecretScalars::try_zeroed(APPLICATION_COMMITMENT_WIDTH)?;
    let mut bound_blinding = SecretScalar::new(Scalar::zero());
    for (row, row_weight) in row_weights.iter().copied().enumerate() {
        let row_start = row
            .checked_mul(APPLICATION_COMMITMENT_WIDTH)
            .ok_or(Figure9FinalOpeningError::InvalidShape)?;
        for column in 0..APPLICATION_COMMITMENT_WIDTH {
            let index = row_start
                .checked_add(column)
                .ok_or(Figure9FinalOpeningError::InvalidShape)?;
            let mut value =
                combined_application_value(semantic, step_shape, core_shape, index, batching)?;
            bound.as_mut_slice()[column] += row_weight * value;
            value.clear_secret();
        }
        let row_blinding = SecretScalar::new(
            semantic.folded_step_blindings[row]
                + batching * core_row_blinding(&semantic.application, row)?,
        );
        bound_blinding.add_assign(row_weight * row_blinding.get());
    }
    Ok((bound, bound_blinding))
}

fn combined_application_value(
    semantic: &GovernedFigure9SemanticPrep<'_>,
    step_shape: &SplitShapeWire,
    core_shape: &SplitShapeWire,
    index: usize,
    batching: Scalar,
) -> Result<Scalar, Figure9FinalOpeningError> {
    let application = &semantic.application;
    if index >= APPLICATION_VARIABLES
        || step_shape.shared != core_shape.shared
        || step_shape.precommitted != core_shape.precommitted
    {
        return Err(Figure9FinalOpeningError::InvalidShape);
    }
    let shared_end = step_shape.shared;
    let precommitted_end = shared_end
        .checked_add(step_shape.precommitted)
        .ok_or(Figure9FinalOpeningError::InvalidShape)?;
    if index < shared_end {
        let shared = application
            .shared_witness
            .get(index)
            .copied()
            .unwrap_or_else(Scalar::zero);
        return Ok((Scalar::one() + batching) * shared);
    }
    if index < precommitted_end {
        let offset = index - shared_end;
        let mut step = Scalar::zero();
        for (weight, instance) in semantic
            .folded_step_weights
            .iter()
            .zip(&application.step_instances)
        {
            step += *weight
                * instance
                    .public_values
                    .get(offset)
                    .copied()
                    .unwrap_or_else(Scalar::zero);
        }
        let core = application
            .core_instance
            .public_values
            .get(offset)
            .copied()
            .unwrap_or_else(Scalar::zero);
        return Ok(step + batching * core);
    }
    let rest = index - precommitted_end;
    let mut step = Scalar::zero();
    for (weight, private) in semantic
        .folded_step_weights
        .iter()
        .zip(&application.step_private)
    {
        step += *weight
            * private
                .rest_values
                .get(rest)
                .copied()
                .unwrap_or_else(Scalar::zero);
    }
    Ok(step)
}

fn core_row_blinding(
    application: &super::application_prep::GovernedFigure9ApplicationPrep<'_>,
    row: usize,
) -> Result<Scalar, Figure9FinalOpeningError> {
    let shared = application.shared_blindings.len();
    let precommitted = application.core_private.precommitted_blindings.len();
    if row < shared {
        return Ok(application.shared_blindings[row]);
    }
    if row < shared + precommitted {
        return Ok(application.core_private.precommitted_blindings[row - shared]);
    }
    application
        .core_private
        .rest_blindings
        .get(row - shared - precommitted)
        .copied()
        .ok_or(Figure9FinalOpeningError::InvalidShape)
}

#[allow(clippy::too_many_arguments)]
fn prove_hyrax_opening(
    application_key: &CommitmentKey,
    evaluation_key: &CommitmentKey,
    transcript: &mut VegaTranscriptV1,
    polynomial_commitment: &Commitment,
    committed_row: Point,
    public_vector: &[Scalar],
    evaluation_commitment: Point,
    bound_row: &[Scalar],
    bound_blinding: Scalar,
    evaluation_blinding: Scalar,
    rng: &mut Figure9StdRng,
    draw_count: &mut usize,
) -> Result<PendingLinearIpa, Figure9FinalOpeningError> {
    if bound_row.len() != application_key.columns()
        || public_vector.len() != bound_row.len()
        || evaluation_key.generators().is_empty()
    {
        return Err(Figure9FinalOpeningError::InvalidShape);
    }
    transcript
        .absorb_commitment(b"poly_com", polynomial_commitment)
        .map_err(|_| Figure9FinalOpeningError::Transcript)?;
    transcript
        .domain_separator(b"inner product argument (linear)")
        .map_err(|_| Figure9FinalOpeningError::Transcript)?;
    let mut instance = Vec::with_capacity(128);
    instance.extend_from_slice(
        &committed_row
            .to_transcript_bytes()
            .map_err(|_| Figure9FinalOpeningError::Transcript)?,
    );
    instance.extend_from_slice(
        &evaluation_commitment
            .to_transcript_bytes()
            .map_err(|_| Figure9FinalOpeningError::Transcript)?,
    );
    transcript
        .absorb_raw(b"U", &instance)
        .map_err(|_| Figure9FinalOpeningError::Transcript)?;

    // Preserve the vendor draw order exactly: d[0..n], r_delta, r_beta.
    let mut mask = SecretScalars::try_with_capacity(public_vector.len())?;
    for _ in 0..public_vector.len() {
        mask.push(draw_scalar(rng, draw_count)?)?;
    }
    let mask_blinding = SecretScalar::new(draw_scalar(rng, draw_count)?);
    let result_mask_blinding = SecretScalar::new(draw_scalar(rng, draw_count)?);
    let delta = application_key
        .commit(mask.as_slice(), &[mask_blinding.get()])
        .map_err(|_| Figure9FinalOpeningError::Commitment)?
        .points()[0];
    let masked_result = SecretScalar::new(
        inner_product(public_vector, mask.as_slice())
            .map_err(|_| Figure9FinalOpeningError::InvalidShape)?,
    );
    let beta = evaluation_key
        .commit(&[masked_result.get()], &[result_mask_blinding.get()])
        .map_err(|_| Figure9FinalOpeningError::Commitment)?
        .points()[0];
    transcript
        .absorb_raw(
            b"delta",
            &delta
                .to_transcript_bytes()
                .map_err(|_| Figure9FinalOpeningError::Transcript)?,
        )
        .map_err(|_| Figure9FinalOpeningError::Transcript)?;
    transcript
        .absorb_raw(
            b"beta",
            &beta
                .to_transcript_bytes()
                .map_err(|_| Figure9FinalOpeningError::Transcript)?,
        )
        .map_err(|_| Figure9FinalOpeningError::Transcript)?;
    let challenge = transcript
        .squeeze(b"r")
        .map_err(|_| Figure9FinalOpeningError::Transcript)?;
    let mut responses = SecretScalars::try_with_capacity(bound_row.len())?;
    for (value, mask) in bound_row.iter().copied().zip(mask.as_slice()) {
        responses.push(challenge * value + *mask)?;
    }
    let delta_response = challenge * bound_blinding + mask_blinding.get();
    let beta_response = challenge * evaluation_blinding + result_mask_blinding.get();
    Ok(PendingLinearIpa::new(LinearIpaWire {
        delta,
        beta,
        responses: responses.into_vec(),
        delta_response,
        beta_response,
    }))
}

fn draw_scalar(
    rng: &mut Figure9StdRng,
    draw_count: &mut usize,
) -> Result<Scalar, Figure9FinalOpeningError> {
    *draw_count = draw_count
        .checked_add(1)
        .ok_or(Figure9FinalOpeningError::InvalidShape)?;
    Ok(rng.scalar())
}

fn clear_secret_scalars(values: &mut [Scalar]) {
    let values = core::hint::black_box(values);
    for value in values.iter_mut() {
        value.clear_secret();
    }
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    let _ = core::hint::black_box(&mut *values);
}

#[cfg(test)]
mod tests {
    use super::super::{verifier_key::HyraxKeyWire, wire::McCodecError};
    use super::*;

    fn s(value: u64) -> Scalar {
        Scalar::from_u64(value)
    }

    fn key_wire(key: &CommitmentKey) -> HyraxKeyWire {
        HyraxKeyWire {
            columns: key.columns(),
            generators: key.generators().to_vec(),
            hiding_generator: key.hiding_generator(),
        }
    }

    fn small_opening(
        seed: [u8; 32],
    ) -> (
        LinearIpaWire,
        HyraxKeyWire,
        HyraxKeyWire,
        Commitment,
        Vec<Scalar>,
        Commitment,
    ) {
        let application_key = CommitmentKey::derive(b"figure9-final-opening-test", 4).expect("key");
        let evaluation_key =
            CommitmentKey::derive(b"figure9-final-evaluation-test", 2).expect("evaluation key");
        let polynomial = [s(2), s(3), s(5), s(7), s(11), s(13), s(17), s(19)];
        let blindings = [s(23), s(29)];
        let commitment = application_key
            .commit(&polynomial, &blindings)
            .expect("commitment");
        let point = vec![s(31), s(37), s(41)];
        let left = eq_evals(&point[..1]).expect("left equality");
        let right = eq_evals(&point[1..]).expect("right equality");
        let mut row = vec![Scalar::zero(); 4];
        for (weight, values) in left.iter().zip(polynomial.chunks_exact(4)) {
            for (output, value) in row.iter_mut().zip(values) {
                *output += *weight * *value;
            }
        }
        let row_blinding = left[0] * blindings[0] + left[1] * blindings[1];
        let evaluation = inner_product(&row, &right).expect("evaluation");
        let evaluation_blinding = s(43);
        let evaluation_commitment = evaluation_key
            .commit(&[evaluation], &[evaluation_blinding])
            .expect("evaluation commitment");
        let committed_row = msm(&left, commitment.points()).expect("committed row");
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        let mut rng = Figure9StdRng::from_seed(seed);
        let mut draws = 0;
        let pending = prove_hyrax_opening(
            &application_key,
            &evaluation_key,
            &mut transcript,
            &commitment,
            committed_row,
            &right,
            evaluation_commitment.points()[0],
            &row,
            row_blinding,
            evaluation_blinding,
            &mut rng,
            &mut draws,
        )
        .expect("linear IPA");
        assert_eq!(draws, right.len() + 2);
        (
            pending.into_wire().expect("publish test wire"),
            key_wire(&application_key),
            key_wire(&evaluation_key),
            commitment,
            point,
            evaluation_commitment,
        )
    }

    fn verify_small(
        argument: &LinearIpaWire,
        application_key: &HyraxKeyWire,
        evaluation_key: &HyraxKeyWire,
        commitment: &Commitment,
        point: &[Scalar],
        evaluation_commitment: &Commitment,
    ) -> Result<(), McCodecError> {
        verify::verify_hyrax_opening(
            application_key,
            evaluation_key,
            &mut VegaTranscriptV1::new_neutron_nova(),
            commitment,
            point,
            evaluation_commitment,
            argument,
        )
    }

    #[test]
    fn deterministic_linear_ipa_roundtrips_and_binds_every_response_category() {
        let fixture = small_opening([0x42; 32]);
        let repeated = small_opening([0x42; 32]);
        assert_eq!(fixture.0, repeated.0);
        verify_small(
            &fixture.0, &fixture.1, &fixture.2, &fixture.3, &fixture.4, &fixture.5,
        )
        .expect("exact verifier replay");

        let mutations: [fn(&mut LinearIpaWire, &HyraxKeyWire); 5] = [
            |proof, key| proof.delta += key.generators[0],
            |proof, key| proof.beta += key.generators[0],
            |proof, _| proof.responses[0] += Scalar::one(),
            |proof, _| proof.delta_response += Scalar::one(),
            |proof, _| proof.beta_response += Scalar::one(),
        ];
        for (index, mutate) in mutations.into_iter().enumerate() {
            let mut changed = fixture.0.clone();
            let key = if index == 1 { &fixture.2 } else { &fixture.1 };
            mutate(&mut changed, key);
            assert!(
                verify_small(
                    &changed, &fixture.1, &fixture.2, &fixture.3, &fixture.4, &fixture.5
                )
                .is_err()
            );
        }

        let mut wrong_commitment_points = fixture.3.points().to_vec();
        wrong_commitment_points[0] += fixture.1.generators[0];
        let wrong_commitment =
            Commitment::from_points(wrong_commitment_points).expect("non-identity mutation");
        assert!(
            verify_small(
                &fixture.0,
                &fixture.1,
                &fixture.2,
                &wrong_commitment,
                &fixture.4,
                &fixture.5,
            )
            .is_err()
        );

        let mut wrong_evaluation_points = fixture.5.points().to_vec();
        wrong_evaluation_points[0] += fixture.2.generators[0];
        let wrong_evaluation =
            Commitment::from_points(wrong_evaluation_points).expect("non-identity mutation");
        assert!(
            verify_small(
                &fixture.0,
                &fixture.1,
                &fixture.2,
                &fixture.3,
                &fixture.4,
                &wrong_evaluation,
            )
            .is_err()
        );

        let mut wrong_point = fixture.4.clone();
        wrong_point.swap(0, 1);
        assert!(
            verify_small(
                &fixture.0,
                &fixture.1,
                &fixture.2,
                &fixture.3,
                &wrong_point,
                &fixture.5,
            )
            .is_err()
        );

        let mut wrong_key = fixture.1.clone();
        wrong_key.generators.swap(0, 1);
        assert!(
            verify_small(
                &fixture.0, &wrong_key, &fixture.2, &fixture.3, &fixture.4, &fixture.5,
            )
            .is_err()
        );

        let mut changed_transcript = VegaTranscriptV1::new_neutron_nova();
        changed_transcript
            .absorb_raw(b"mutated-prefix", b"1")
            .expect("bounded transcript mutation");
        assert!(
            verify::verify_hyrax_opening(
                &fixture.1,
                &fixture.2,
                &mut changed_transcript,
                &fixture.3,
                &fixture.4,
                &fixture.5,
                &fixture.0,
            )
            .is_err()
        );
    }

    #[test]
    fn unpublished_linear_ipa_responses_are_zeroized_on_drop() {
        let application_key = CommitmentKey::derive(b"figure9-final-drop-test", 2).expect("key");
        let evaluation_key =
            CommitmentKey::derive(b"figure9-final-drop-eval-test", 2).expect("key");
        let values = [s(2), s(3)];
        let commitment = application_key
            .commit(&values, &[s(5)])
            .expect("commitment");
        let public = [s(7), s(11)];
        let evaluation = inner_product(&values, &public).expect("inner product");
        let evaluation_commitment = evaluation_key
            .commit(&[evaluation], &[s(13)])
            .expect("evaluation commitment");
        let before = PENDING_IPA_ZEROIZED_DROPS.with(core::cell::Cell::get);
        let mut draws = 0;
        let pending = prove_hyrax_opening(
            &application_key,
            &evaluation_key,
            &mut VegaTranscriptV1::new_neutron_nova(),
            &commitment,
            commitment.points()[0],
            &public,
            evaluation_commitment.points()[0],
            &values,
            s(5),
            s(13),
            &mut Figure9StdRng::from_seed([0x24; 32]),
            &mut draws,
        )
        .expect("pending IPA");
        drop(pending);
        assert_eq!(
            PENDING_IPA_ZEROIZED_DROPS.with(core::cell::Cell::get),
            before + 1
        );
    }

    #[test]
    fn unpublished_canonical_bytes_are_zeroized_on_drop_and_unwind() {
        let before = PENDING_ENCODING_ZEROIZED_DROPS.with(core::cell::Cell::get);
        drop(PendingEncoding::new(vec![1, 2, 3]));
        assert_eq!(
            PENDING_ENCODING_ZEROIZED_DROPS.with(core::cell::Cell::get),
            before + 1
        );

        let unwind_before = PENDING_ENCODING_ZEROIZED_DROPS.with(core::cell::Cell::get);
        let result = std::panic::catch_unwind(|| {
            let _pending = PendingEncoding::new(vec![4, 5, 6]);
            panic!("injected unwind");
        });
        assert!(result.is_err());
        assert_eq!(
            PENDING_ENCODING_ZEROIZED_DROPS.with(core::cell::Cell::get),
            unwind_before + 1
        );

        let published_before = PENDING_ENCODING_ZEROIZED_DROPS.with(core::cell::Cell::get);
        let bytes = PendingEncoding::new(vec![7, 8, 9])
            .publish()
            .expect("published proof bytes");
        assert_eq!(bytes, [7, 8, 9]);
        assert_eq!(
            PENDING_ENCODING_ZEROIZED_DROPS.with(core::cell::Cell::get),
            published_before
        );
    }

    #[test]
    fn final_geometry_sources_and_streaming_contract_are_pinned() {
        assert_eq!(APPLICATION_ROWS, 512);
        assert_eq!(
            ROW_POINT_VARIABLES + COLUMN_POINT_VARIABLES,
            OPENING_POINT_VARIABLES
        );
        assert_eq!(FINAL_LINEAR_IPA_SCALAR_DRAWS, 2_050);
        assert_eq!(FULL_PROOF_SCALAR_DRAWS, 6_290);
        for (source, expected) in [
            (
                include_bytes!("../../../../../vendor/vega-prover/src/provider/pcs/hyrax_pc.rs")
                    .as_slice(),
                VEGA_HYRAX_SOURCE_SHA256,
            ),
            (
                include_bytes!("../../../../../vendor/vega-prover/src/provider/pcs/ipa.rs")
                    .as_slice(),
                VEGA_LINEAR_IPA_SOURCE_SHA256,
            ),
            (
                include_bytes!(
                    "../../../../../vendor/vega-prover/reference/pyvega/prover_finish.py"
                )
                .as_slice(),
                PYVEGA_FINISH_SOURCE_SHA256,
            ),
        ] {
            assert_eq!(
                hex::encode(super::super::sha256::sha256(source).expect("bounded source")),
                expected
            );
        }
        let source = include_str!("final_opening.rs");
        let production = source
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("production source");
        assert!(production.contains("bind_streamed_application_row"));
        assert!(production.contains("verify::verify_hyrax_opening"));
        assert!(production.contains("verify::verify(proof.as_proof()?"));
        assert!(!production.contains("vec![Scalar::zero(); APPLICATION_VARIABLES]"));
        assert!(!production.contains("use rand"));
        assert!(!production.contains("use rayon"));
    }
}
