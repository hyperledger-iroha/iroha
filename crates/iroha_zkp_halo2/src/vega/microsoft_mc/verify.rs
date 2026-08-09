//! Exact first-party verifier for the pinned Microsoft Vega-MC profile.

use super::super::{
    VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
    algebra::{eq_evals, eq_evaluate, inner_product, log2_exact},
    commitment::{Commitment, CommitmentKey, fold, msm},
    hyrax::verify_direct,
    r1cs::{Instance, RelaxedInstance},
    sumcheck::{CompressedUnivariate, SumcheckProof},
    transcript::VegaTranscriptV1,
};
use super::{
    verifier_key::{
        HyraxKeyWire, McVerifierKeyWire, MultiRoundShapeWire, RegularShapeWire, SplitShapeWire,
    },
    wire::{
        CompressedPolynomialWire, LinearIpaWire, McCodecError, McCommitment, McProofWire,
        MultiRoundInstanceWire, RelaxedSpartanWire, SplitInstanceWire, SumcheckWire,
    },
};

const DEFAULT_COMMITMENT_WIDTH: usize = 2_048;
const MAX_APPLICATION_EQ_TABLE_ITEMS: usize = 1 << 21;

/// Verify one already decoded proof under one already decoded canonical key.
pub(super) fn verify(
    proof: &McProofWire,
    key: &McVerifierKeyWire,
    num_instances: usize,
) -> Result<(Vec<Vec<Scalar>>, Vec<Scalar>), McCodecError> {
    if num_instances == 0
        || num_instances != proof.step_instances.len()
        || num_instances != key.num_steps
    {
        return invalid();
    }

    let application_key = derive_and_match_key(&key.application_key)?;
    let verifier_key = derive_and_match_key(&key.verifier_commitment_key)?;
    let digest = key.digest()?;

    let mut step_instances = proof.step_instances.clone();
    for instance in &mut step_instances {
        instance.shared.clone_from(&proof.shared_commitment);
    }
    let mut core_instance = proof.core_instance.clone();
    core_instance.shared.clone_from(&proof.shared_commitment);

    for (index, instance) in step_instances.iter().enumerate() {
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        transcript
            .absorb_raw(b"vk", &digest)
            .map_err(|_| McCodecError::InvalidEncoding)?;
        transcript
            .absorb_scalar(b"num_circuits", scalar_from_usize(step_instances.len())?)
            .map_err(|_| McCodecError::InvalidEncoding)?;
        transcript
            .absorb_scalar(b"circuit_index", scalar_from_usize(index)?)
            .map_err(|_| McCodecError::InvalidEncoding)?;
        transcript
            .absorb_scalars(b"public_values", &instance.public_values)
            .map_err(|_| McCodecError::InvalidEncoding)?;
        validate_split_instance(instance, &key.step_shape, &mut transcript)?;
    }

    let mut core_transcript = VegaTranscriptV1::new_neutron_nova();
    core_transcript
        .absorb_raw(b"vk", &digest)
        .map_err(|_| McCodecError::InvalidEncoding)?;
    core_transcript
        .absorb_scalars(b"public_values", &core_instance.public_values)
        .map_err(|_| McCodecError::InvalidEncoding)?;
    validate_split_instance(&core_instance, &key.core_shape, &mut core_transcript)?;

    let original_public_values = step_instances
        .iter()
        .map(|instance| instance.public_values.clone())
        .collect::<Vec<_>>();
    let core_public_values = core_instance.public_values.clone();

    let padded_count = step_instances
        .len()
        .checked_next_power_of_two()
        .ok_or(McCodecError::InvalidEncoding)?;
    while step_instances.len() < padded_count {
        step_instances.push(
            step_instances
                .first()
                .cloned()
                .ok_or(McCodecError::InvalidEncoding)?,
        );
    }
    let step_regular = step_instances
        .iter()
        .map(split_to_regular)
        .collect::<Result<Vec<_>, _>>()?;
    let core_regular = split_to_regular(&core_instance)?;

    let mut transcript = VegaTranscriptV1::new_neutron_nova();
    transcript
        .absorb_raw(b"vk", &digest)
        .map_err(|_| McCodecError::InvalidEncoding)?;
    transcript
        .absorb_r1cs_instance(
            b"core_instance",
            &core_regular.witness_commitment,
            &core_regular.public_inputs,
        )
        .map_err(|_| McCodecError::InvalidEncoding)?;
    for instance in &step_regular {
        transcript
            .absorb_r1cs_instance(b"U", &instance.witness_commitment, &instance.public_inputs)
            .map_err(|_| McCodecError::InvalidEncoding)?;
    }
    transcript
        .absorb_scalar(b"T", Scalar::zero())
        .map_err(|_| McCodecError::InvalidEncoding)?;

    let rounds_b = log2_exact(step_regular.len()).map_err(|_| McCodecError::InvalidEncoding)?;
    let step_variables = key.step_shape.variables()?;
    let rounds_x =
        log2_exact(key.step_shape.constraints).map_err(|_| McCodecError::InvalidEncoding)?;
    let rounds_y = log2_exact(step_variables)
        .map_err(|_| McCodecError::InvalidEncoding)?
        .checked_add(1)
        .ok_or(McCodecError::InvalidEncoding)?;
    let tau = transcript
        .squeeze(b"tau")
        .map_err(|_| McCodecError::InvalidEncoding)?;
    let mut rhos = Vec::with_capacity(rounds_b);
    for _ in 0..rounds_b {
        rhos.push(
            transcript
                .squeeze(b"rho")
                .map_err(|_| McCodecError::InvalidEncoding)?,
        );
    }

    validate_multi_round_instance(
        &proof.verifier_instance,
        &key.verifier_shape,
        &mut transcript,
    )?;
    let verifier_regular = multi_round_to_regular(&proof.verifier_instance)?;
    let challenge_count = rounds_b
        .checked_add(rounds_x)
        .and_then(|value| value.checked_add(1))
        .and_then(|value| value.checked_add(rounds_y))
        .ok_or(McCodecError::InvalidEncoding)?;
    if verifier_regular.public_inputs.len() != challenge_count + 6 {
        return invalid();
    }
    let (challenges, pinned_public_values) =
        verifier_regular.public_inputs.split_at(challenge_count);
    let (r_b, remaining) = challenges.split_at(rounds_b);
    let (r_x, remaining) = remaining.split_at(rounds_x);
    let (&r, r_y) = remaining
        .split_first()
        .ok_or(McCodecError::InvalidEncoding)?;
    if r_y.len() != rounds_y {
        return invalid();
    }

    let folded_step = fold_instances(r_b, &step_regular)?;
    let random_instance = RelaxedInstance {
        witness_commitment: proof.random_instance.witness_commitment.to_local()?,
        error_commitment: proof.random_instance.error_commitment.to_local()?,
        public_inputs: proof.random_instance.public_values.clone(),
        relaxation: proof.random_instance.relaxation,
    };
    let folded_verifier = verify_nifs(
        &verifier_key,
        &key.verifier_regular_shape,
        &mut transcript,
        &random_instance,
        &verifier_regular,
        &proof.nova_cross_term.to_local()?,
    )?;
    verify_relaxed_spartan(
        &proof.relaxed_spartan,
        &key.verifier_regular_shape,
        &verifier_key,
        &folded_verifier,
        &mut transcript,
    )?;

    let row_weights = eq_evals(r_x).map_err(|_| McCodecError::InvalidEncoding)?;
    let column_weights = application_eq_evals(r_y)?;
    let eval_a_step = key.step_shape.a.evaluate(&row_weights, &column_weights)?;
    let eval_b_step = key.step_shape.b.evaluate(&row_weights, &column_weights)?;
    let eval_c_step = key.step_shape.c.evaluate(&row_weights, &column_weights)?;
    let eval_a_core = key.core_shape.a.evaluate(&row_weights, &column_weights)?;
    let eval_b_core = key.core_shape.b.evaluate(&row_weights, &column_weights)?;
    let eval_c_core = key.core_shape.c.evaluate(&row_weights, &column_weights)?;

    let variables_log2 = log2_exact(step_variables).map_err(|_| McCodecError::InvalidEncoding)?;
    let mut folded_sparse = Vec::with_capacity(folded_step.public_inputs.len() + 1);
    folded_sparse.push(Scalar::one());
    folded_sparse.extend_from_slice(&folded_step.public_inputs);
    let mut core_sparse = Vec::with_capacity(core_regular.public_inputs.len() + 1);
    core_sparse.push(Scalar::one());
    core_sparse.extend_from_slice(&core_regular.public_inputs);
    let eval_x_step = sparse_polynomial_evaluate(variables_log2, &folded_sparse, &r_y[1..])?;
    let eval_x_core = sparse_polynomial_evaluate(variables_log2, &core_sparse, &r_y[1..])?;
    let r_squared = r.square();
    let quotient_step = eval_a_step + r * eval_b_step + r_squared * eval_c_step;
    let quotient_core = eval_a_core + r * eval_b_core + r_squared * eval_c_core;
    let tau_at_rx = power_polynomial_evaluate(tau, rounds_x, r_x)?;
    let rho_at_rb = eq_evaluate(r_b, &rhos).map_err(|_| McCodecError::InvalidEncoding)?;
    if pinned_public_values
        != [
            tau_at_rx,
            eval_x_step,
            eval_x_core,
            rho_at_rb,
            quotient_step,
            quotient_core,
        ]
    {
        return invalid();
    }

    let evaluation_batching = transcript
        .squeeze(b"c_eval")
        .map_err(|_| McCodecError::InvalidEncoding)?;
    let step_evaluation_round = rounds_b
        .checked_add(1)
        .and_then(|value| value.checked_add(rounds_x))
        .and_then(|value| value.checked_add(1))
        .and_then(|value| value.checked_add(rounds_y))
        .and_then(|value| value.checked_add(1))
        .ok_or(McCodecError::InvalidEncoding)?;
    let step_evaluation = proof
        .verifier_instance
        .commitments
        .get(step_evaluation_round)
        .ok_or(McCodecError::InvalidEncoding)?
        .to_local()?;
    let core_evaluation = proof
        .verifier_instance
        .commitments
        .get(step_evaluation_round + 1)
        .ok_or(McCodecError::InvalidEncoding)?
        .to_local()?;
    let batched_commitment = fold(
        &[
            &folded_step.witness_commitment,
            &core_regular.witness_commitment,
        ],
        &[Scalar::one(), evaluation_batching],
    )
    .map_err(|_| McCodecError::InvalidEncoding)?;
    let batched_evaluation = fold(
        &[&step_evaluation, &core_evaluation],
        &[Scalar::one(), evaluation_batching],
    )
    .map_err(|_| McCodecError::InvalidEncoding)?;
    verify_hyrax_opening(
        &key.evaluation_key,
        &key.verifier_commitment_key,
        &mut transcript,
        &batched_commitment,
        &r_y[1..],
        &batched_evaluation,
        &proof.evaluation_argument,
    )?;

    // Keep the derived keys live in the verifier boundary. The application key
    // equality check above prevents a substituted serialized generator set,
    // while the wire key is used directly by the final compatibility equation.
    if application_key.columns() != DEFAULT_COMMITMENT_WIDTH {
        return invalid();
    }
    Ok((original_public_values, core_public_values))
}

fn derive_and_match_key(wire: &HyraxKeyWire) -> Result<CommitmentKey, McCodecError> {
    let key =
        CommitmentKey::derive(b"ck", wire.columns).map_err(|_| McCodecError::InvalidEncoding)?;
    if key.generators() != wire.generators
        || key.hiding_generator() != wire.hiding_generator
        || key.columns() != wire.columns
    {
        return invalid();
    }
    Ok(key)
}

fn validate_split_instance(
    instance: &SplitInstanceWire,
    shape: &SplitShapeWire,
    transcript: &mut VegaTranscriptV1,
) -> Result<(), McCodecError> {
    if instance.public_values.len() != shape.public_values
        || instance.challenges.len() != shape.challenges
    {
        return invalid();
    }
    validate_optional_segment(
        instance.shared.as_ref(),
        shape.shared,
        DEFAULT_COMMITMENT_WIDTH,
    )?;
    if let Some(commitment) = &instance.shared {
        transcript
            .absorb_commitment(b"comm_W_shared", &commitment.to_local()?)
            .map_err(|_| McCodecError::InvalidEncoding)?;
    }
    validate_optional_segment(
        instance.precommitted.as_ref(),
        shape.precommitted,
        DEFAULT_COMMITMENT_WIDTH,
    )?;
    if let Some(commitment) = &instance.precommitted {
        transcript
            .absorb_commitment(b"comm_W_precommitted", &commitment.to_local()?)
            .map_err(|_| McCodecError::InvalidEncoding)?;
    }
    let mut derived = Vec::with_capacity(shape.challenges);
    for _ in 0..shape.challenges {
        derived.push(
            transcript
                .squeeze(b"challenge")
                .map_err(|_| McCodecError::InvalidEncoding)?,
        );
    }
    if derived != instance.challenges {
        return invalid();
    }
    validate_commitment(&instance.rest, shape.rest, DEFAULT_COMMITMENT_WIDTH)?;
    transcript
        .absorb_commitment(b"comm_W_rest", &instance.rest.to_local()?)
        .map_err(|_| McCodecError::InvalidEncoding)
}

fn validate_multi_round_instance(
    instance: &MultiRoundInstanceWire,
    shape: &MultiRoundShapeWire,
    transcript: &mut VegaTranscriptV1,
) -> Result<(), McCodecError> {
    if instance.public_values.len() != shape.public_values
        || instance.commitments.len() != shape.rounds
        || instance.challenges_per_round.len() != shape.rounds
    {
        return invalid();
    }
    for round in 0..shape.rounds {
        validate_commitment(
            &instance.commitments[round],
            shape.variables_per_round[round],
            shape.commitment_width,
        )?;
        transcript
            .absorb_commitment(b"comm_w_round", &instance.commitments[round].to_local()?)
            .map_err(|_| McCodecError::InvalidEncoding)?;
        let mut derived = Vec::with_capacity(shape.challenges_per_round[round]);
        for _ in 0..shape.challenges_per_round[round] {
            derived.push(
                transcript
                    .squeeze(b"challenge")
                    .map_err(|_| McCodecError::InvalidEncoding)?,
            );
        }
        if derived != instance.challenges_per_round[round] {
            return invalid();
        }
    }
    Ok(())
}

fn validate_optional_segment(
    commitment: Option<&McCommitment>,
    values: usize,
    width: usize,
) -> Result<(), McCodecError> {
    match (commitment, values) {
        (None, 0) => Ok(()),
        (Some(commitment), values) if values != 0 => validate_commitment(commitment, values, width),
        _ => invalid(),
    }
}

fn validate_commitment(
    commitment: &McCommitment,
    values: usize,
    width: usize,
) -> Result<(), McCodecError> {
    if values == 0 || width == 0 || commitment.points.len() != values.div_ceil(width) {
        invalid()
    } else {
        Ok(())
    }
}

fn split_to_regular(instance: &SplitInstanceWire) -> Result<Instance, McCodecError> {
    let commitments = [
        instance.shared.as_ref(),
        instance.precommitted.as_ref(),
        Some(&instance.rest),
    ];
    let witness_commitment = concatenate_commitments(commitments.into_iter().flatten())?;
    let mut public_inputs = Vec::with_capacity(
        instance
            .public_values
            .len()
            .checked_add(instance.challenges.len())
            .ok_or(McCodecError::InvalidEncoding)?,
    );
    public_inputs.extend_from_slice(&instance.public_values);
    public_inputs.extend_from_slice(&instance.challenges);
    Ok(Instance {
        witness_commitment,
        public_inputs,
    })
}

fn multi_round_to_regular(instance: &MultiRoundInstanceWire) -> Result<Instance, McCodecError> {
    let witness_commitment = concatenate_commitments(instance.commitments.iter())?;
    let challenge_count = instance
        .challenges_per_round
        .iter()
        .try_fold(0_usize, |total, round| total.checked_add(round.len()))
        .ok_or(McCodecError::InvalidEncoding)?;
    let mut public_inputs = Vec::with_capacity(
        challenge_count
            .checked_add(instance.public_values.len())
            .ok_or(McCodecError::InvalidEncoding)?,
    );
    for round in &instance.challenges_per_round {
        public_inputs.extend_from_slice(round);
    }
    public_inputs.extend_from_slice(&instance.public_values);
    Ok(Instance {
        witness_commitment,
        public_inputs,
    })
}

fn concatenate_commitments<'a>(
    commitments: impl Iterator<Item = &'a McCommitment>,
) -> Result<Commitment, McCodecError> {
    let mut points = Vec::new();
    for commitment in commitments {
        points.extend_from_slice(&commitment.points);
    }
    Commitment::from_points(points).map_err(|_| McCodecError::InvalidEncoding)
}

fn fold_instances(challenges: &[Scalar], instances: &[Instance]) -> Result<Instance, McCodecError> {
    if instances.is_empty()
        || instances.len()
            != (1_usize
                .checked_shl(
                    u32::try_from(challenges.len()).map_err(|_| McCodecError::InvalidEncoding)?,
                )
                .ok_or(McCodecError::InvalidEncoding)?)
    {
        return invalid();
    }
    let weights = weights_from_challenges(challenges, instances.len())?;
    let public_count = instances[0].public_inputs.len();
    if instances
        .iter()
        .any(|instance| instance.public_inputs.len() != public_count)
    {
        return invalid();
    }
    let mut public_inputs = vec![Scalar::zero(); public_count];
    for (instance, weight) in instances.iter().zip(weights.iter().copied()) {
        for (output, input) in public_inputs.iter_mut().zip(&instance.public_inputs) {
            *output += weight * *input;
        }
    }
    let references = instances
        .iter()
        .map(|instance| &instance.witness_commitment)
        .collect::<Vec<_>>();
    let witness_commitment =
        fold(&references, &weights).map_err(|_| McCodecError::InvalidEncoding)?;
    Ok(Instance {
        witness_commitment,
        public_inputs,
    })
}

fn weights_from_challenges(
    challenges: &[Scalar],
    count: usize,
) -> Result<Vec<Scalar>, McCodecError> {
    if count == 0 {
        return invalid();
    }
    let mut weights = Vec::with_capacity(count);
    for index in 0..count {
        let mut weight = Scalar::one();
        let mut bits = index;
        for challenge in challenges {
            weight *= if bits & 1 == 1 {
                *challenge
            } else {
                Scalar::one() - *challenge
            };
            bits >>= 1;
        }
        if bits != 0 {
            return invalid();
        }
        weights.push(weight);
    }
    Ok(weights)
}

fn verify_nifs(
    key: &CommitmentKey,
    shape: &RegularShapeWire,
    transcript: &mut VegaTranscriptV1,
    relaxed: &RelaxedInstance,
    regular: &Instance,
    cross_term_commitment: &Commitment,
) -> Result<RelaxedInstance, McCodecError> {
    let witness_rows = shape.variables.div_ceil(key.columns());
    let error_rows = shape.constraints.div_ceil(key.columns());
    if relaxed.public_inputs.len() != shape.public_values
        || regular.public_inputs.len() != shape.public_values
        || relaxed.witness_commitment.len() != witness_rows
        || regular.witness_commitment.len() != witness_rows
        || relaxed.error_commitment.len() != error_rows
        || cross_term_commitment.len() != error_rows
    {
        return invalid();
    }

    transcript
        .absorb_relaxed_r1cs_instance(
            b"U1",
            &relaxed.witness_commitment,
            &relaxed.error_commitment,
            relaxed.relaxation,
            &relaxed.public_inputs,
        )
        .map_err(|_| McCodecError::InvalidEncoding)?;
    transcript
        .absorb_r1cs_instance(b"U2", &regular.witness_commitment, &regular.public_inputs)
        .map_err(|_| McCodecError::InvalidEncoding)?;
    transcript
        .absorb_commitment(b"comm_T", cross_term_commitment)
        .map_err(|_| McCodecError::InvalidEncoding)?;
    let challenge = transcript
        .squeeze(b"r")
        .map_err(|_| McCodecError::InvalidEncoding)?;

    let witness_commitment = fold(
        &[&relaxed.witness_commitment, &regular.witness_commitment],
        &[Scalar::one(), challenge],
    )
    .map_err(|_| McCodecError::InvalidEncoding)?;
    let error_commitment = fold(
        &[&relaxed.error_commitment, cross_term_commitment],
        &[Scalar::one(), challenge],
    )
    .map_err(|_| McCodecError::InvalidEncoding)?;
    let public_inputs = relaxed
        .public_inputs
        .iter()
        .copied()
        .zip(regular.public_inputs.iter().copied())
        .map(|(left, right)| left + challenge * right)
        .collect();
    Ok(RelaxedInstance {
        witness_commitment,
        error_commitment,
        public_inputs,
        relaxation: relaxed.relaxation + challenge,
    })
}

fn verify_relaxed_spartan(
    proof: &RelaxedSpartanWire,
    shape: &RegularShapeWire,
    key: &CommitmentKey,
    instance: &RelaxedInstance,
    transcript: &mut VegaTranscriptV1,
) -> Result<(), McCodecError> {
    let outer_rounds = log2_exact(shape.constraints).map_err(|_| McCodecError::InvalidEncoding)?;
    let padded_variables = shape
        .variables
        .checked_next_power_of_two()
        .ok_or(McCodecError::InvalidEncoding)?;
    let inner_rounds = log2_exact(padded_variables)
        .map_err(|_| McCodecError::InvalidEncoding)?
        .checked_add(1)
        .ok_or(McCodecError::InvalidEncoding)?;
    let assignment_table_len = padded_variables
        .checked_mul(2)
        .ok_or(McCodecError::InvalidEncoding)?;
    let assignment_values = shape
        .variables
        .checked_add(1)
        .and_then(|value| value.checked_add(shape.public_values))
        .ok_or(McCodecError::InvalidEncoding)?;
    let witness_rows = shape.variables.div_ceil(key.columns());
    let error_rows = shape.constraints.div_ceil(key.columns());
    if assignment_values > assignment_table_len
        || instance.public_inputs.len() != shape.public_values
        || instance.witness_commitment.len() != witness_rows
        || instance.error_commitment.len() != error_rows
        || proof.witness_opening.len() != key.columns()
        || proof.error_opening.len() != key.columns()
    {
        return invalid();
    }

    transcript
        .absorb_scalar(b"u_relaxed", instance.relaxation)
        .map_err(|_| McCodecError::InvalidEncoding)?;
    transcript
        .absorb_scalars(b"X_relaxed", &instance.public_inputs)
        .map_err(|_| McCodecError::InvalidEncoding)?;

    let mut tau = Vec::with_capacity(outer_rounds);
    for _ in 0..outer_rounds {
        tau.push(
            transcript
                .squeeze(b"t")
                .map_err(|_| McCodecError::InvalidEncoding)?,
        );
    }
    let (outer_final, row_point) = sumcheck_from_wire(&proof.outer_sumcheck, 3)?
        .verify(Scalar::zero(), outer_rounds, 3, transcript)
        .map_err(|_| McCodecError::InvalidEncoding)?;
    let expected_outer = eq_evaluate(&tau, &row_point)
        .map_err(|_| McCodecError::InvalidEncoding)?
        * (proof.outer_claims[0] * proof.outer_claims[1] - proof.outer_claims[2]);
    if outer_final != expected_outer {
        return invalid();
    }
    transcript
        .absorb_scalars(b"claims_outer", &proof.outer_claims)
        .map_err(|_| McCodecError::InvalidEncoding)?;

    let batching_challenge = transcript
        .squeeze(b"r")
        .map_err(|_| McCodecError::InvalidEncoding)?;
    let batching_challenge_squared = batching_challenge.square();
    let error_evaluation = verify_direct(
        key,
        &instance.error_commitment,
        &proof.error_opening,
        proof.error_blinding,
        &row_point,
    )
    .map_err(|_| McCodecError::InvalidEncoding)?;
    let inner_claim = proof.outer_claims[0]
        + batching_challenge * proof.outer_claims[1]
        + batching_challenge_squared * (proof.outer_claims[2] - error_evaluation);
    let (inner_final, column_point) = sumcheck_from_wire(&proof.inner_sumcheck, 2)?
        .verify(inner_claim, inner_rounds, 2, transcript)
        .map_err(|_| McCodecError::InvalidEncoding)?;
    let (&first_column_coordinate, witness_point) = column_point
        .split_first()
        .ok_or(McCodecError::InvalidEncoding)?;
    let witness_evaluation = verify_direct(
        key,
        &instance.witness_commitment,
        &proof.witness_opening,
        proof.witness_blinding,
        witness_point,
    )
    .map_err(|_| McCodecError::InvalidEncoding)?;

    let row_weights = eq_evals(&row_point).map_err(|_| McCodecError::InvalidEncoding)?;
    let column_weights = eq_evals(&column_point).map_err(|_| McCodecError::InvalidEncoding)?;
    if column_weights.len() != assignment_table_len {
        return invalid();
    }
    let mut assignment_evaluation = (Scalar::one() - first_column_coordinate) * witness_evaluation;
    assignment_evaluation += instance.relaxation
        * *column_weights
            .get(shape.variables)
            .ok_or(McCodecError::InvalidEncoding)?;
    for (index, input) in instance.public_inputs.iter().copied().enumerate() {
        let column = shape
            .variables
            .checked_add(1)
            .and_then(|column| column.checked_add(index))
            .ok_or(McCodecError::InvalidEncoding)?;
        assignment_evaluation += input
            * *column_weights
                .get(column)
                .ok_or(McCodecError::InvalidEncoding)?;
    }
    let a_evaluation = shape.a.evaluate(&row_weights, &column_weights)?;
    let b_evaluation = shape.b.evaluate(&row_weights, &column_weights)?;
    let c_evaluation = shape.c.evaluate(&row_weights, &column_weights)?;
    let batched_matrix_evaluation = a_evaluation
        + batching_challenge * b_evaluation
        + batching_challenge_squared * instance.relaxation * c_evaluation;
    if inner_final != batched_matrix_evaluation * assignment_evaluation {
        return invalid();
    }

    transcript
        .absorb_scalars(b"v_W", &proof.witness_opening)
        .map_err(|_| McCodecError::InvalidEncoding)?;
    transcript
        .absorb_scalars(b"v_E", &proof.error_opening)
        .map_err(|_| McCodecError::InvalidEncoding)
}

fn sumcheck_from_wire(proof: &SumcheckWire, degree: usize) -> Result<SumcheckProof, McCodecError> {
    let rounds = proof
        .rounds
        .iter()
        .map(
            |CompressedPolynomialWire {
                 coefficients_except_linear,
             }| {
                CompressedUnivariate::new(coefficients_except_linear.clone(), degree)
                    .map_err(|_| McCodecError::InvalidEncoding)
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SumcheckProof::new(rounds))
}

fn application_eq_evals(point: &[Scalar]) -> Result<Vec<Scalar>, McCodecError> {
    let size = application_eq_table_size(point.len())?;
    let mut evaluations = vec![Scalar::zero(); size];
    evaluations[0] = Scalar::one();
    let mut populated = 1;
    for coordinate in point.iter().rev().copied() {
        for index in 0..populated {
            let selected = evaluations[index] * coordinate;
            evaluations[populated + index] = selected;
            evaluations[index] -= selected;
        }
        populated *= 2;
    }
    Ok(evaluations)
}

fn application_eq_table_size(variable_count: usize) -> Result<usize, McCodecError> {
    let size = 1_usize
        .checked_shl(u32::try_from(variable_count).map_err(|_| McCodecError::InvalidEncoding)?)
        .ok_or(McCodecError::InvalidEncoding)?;
    if size > MAX_APPLICATION_EQ_TABLE_ITEMS {
        return invalid();
    }
    Ok(size)
}

fn power_polynomial_evaluate(
    base: Scalar,
    variables: usize,
    point: &[Scalar],
) -> Result<Scalar, McCodecError> {
    if point.len() != variables {
        return invalid();
    }
    let mut powers = Vec::with_capacity(variables);
    let mut power = base;
    for _ in 0..variables {
        powers.push(power);
        power = power.square();
    }
    Ok(point.iter().rev().copied().zip(powers).fold(
        Scalar::one(),
        |accumulator, (coordinate, power)| {
            accumulator * (Scalar::one() + (power - Scalar::one()) * coordinate)
        },
    ))
}

fn sparse_polynomial_evaluate(
    variables: usize,
    evaluations: &[Scalar],
    point: &[Scalar],
) -> Result<Scalar, McCodecError> {
    if variables != point.len() || evaluations.is_empty() {
        return invalid();
    }
    let padded = evaluations
        .len()
        .checked_next_power_of_two()
        .ok_or(McCodecError::InvalidEncoding)?;
    let evaluation_variables = log2_exact(padded).map_err(|_| McCodecError::InvalidEncoding)?;
    let start = variables
        .checked_sub(
            evaluation_variables
                .checked_add(1)
                .ok_or(McCodecError::InvalidEncoding)?,
        )
        .ok_or(McCodecError::InvalidEncoding)?;
    let equality = eq_evals(&point[start..]).map_err(|_| McCodecError::InvalidEncoding)?;
    if equality.len() < evaluations.len() {
        return invalid();
    }
    let partial = evaluations
        .iter()
        .copied()
        .zip(equality)
        .fold(Scalar::zero(), |sum, (evaluation, weight)| {
            sum + evaluation * weight
        });
    Ok(point[..start]
        .iter()
        .copied()
        .fold(partial, |value, coordinate| {
            value * (Scalar::one() - coordinate)
        }))
}

fn verify_hyrax_opening(
    key: &HyraxKeyWire,
    evaluation_key: &HyraxKeyWire,
    transcript: &mut VegaTranscriptV1,
    commitment: &Commitment,
    point: &[Scalar],
    evaluation_commitment: &Commitment,
    argument: &LinearIpaWire,
) -> Result<(), McCodecError> {
    transcript
        .absorb_commitment(b"poly_com", commitment)
        .map_err(|_| McCodecError::InvalidEncoding)?;
    let evaluation_count = 1_usize
        .checked_shl(u32::try_from(point.len()).map_err(|_| McCodecError::InvalidEncoding)?)
        .ok_or(McCodecError::InvalidEncoding)?;
    let row_count = evaluation_count.div_ceil(key.columns);
    let row_variables = log2_exact(row_count).map_err(|_| McCodecError::InvalidEncoding)?;
    if commitment.len() != row_count || evaluation_commitment.len() != 1 {
        return invalid();
    }
    let (left_point, right_point) = point.split_at(row_variables);
    let right_weights = eq_evals(right_point).map_err(|_| McCodecError::InvalidEncoding)?;
    let committed_row = if row_variables == 0 {
        commitment.points()[0]
    } else {
        let left_weights = eq_evals(left_point).map_err(|_| McCodecError::InvalidEncoding)?;
        msm(&left_weights, commitment.points()).map_err(|_| McCodecError::InvalidEncoding)?
    };
    verify_linear_ipa(
        key,
        evaluation_key,
        transcript,
        committed_row,
        &right_weights,
        evaluation_commitment.points()[0],
        argument,
    )
}

fn verify_linear_ipa(
    key: &HyraxKeyWire,
    evaluation_key: &HyraxKeyWire,
    transcript: &mut VegaTranscriptV1,
    vector_commitment: Point,
    public_vector: &[Scalar],
    result_commitment: Point,
    argument: &LinearIpaWire,
) -> Result<(), McCodecError> {
    if argument.responses.len() != public_vector.len()
        || argument.responses.len() > key.generators.len()
        || evaluation_key.generators.is_empty()
    {
        return invalid();
    }
    transcript
        .domain_separator(b"inner product argument (linear)")
        .map_err(|_| McCodecError::InvalidEncoding)?;
    let mut instance = Vec::with_capacity(128);
    instance.extend_from_slice(
        &vector_commitment
            .to_transcript_bytes()
            .map_err(|_| McCodecError::InvalidEncoding)?,
    );
    instance.extend_from_slice(
        &result_commitment
            .to_transcript_bytes()
            .map_err(|_| McCodecError::InvalidEncoding)?,
    );
    transcript
        .absorb_raw(b"U", &instance)
        .map_err(|_| McCodecError::InvalidEncoding)?;
    transcript
        .absorb_raw(
            b"delta",
            &argument
                .delta
                .to_transcript_bytes()
                .map_err(|_| McCodecError::InvalidEncoding)?,
        )
        .map_err(|_| McCodecError::InvalidEncoding)?;
    transcript
        .absorb_raw(
            b"beta",
            &argument
                .beta
                .to_transcript_bytes()
                .map_err(|_| McCodecError::InvalidEncoding)?,
        )
        .map_err(|_| McCodecError::InvalidEncoding)?;
    let challenge = transcript
        .squeeze(b"r")
        .map_err(|_| McCodecError::InvalidEncoding)?;

    let lhs_vector = vector_commitment.mul_scalar(challenge) + argument.delta;
    let rhs_vector = msm(
        &argument.responses,
        &key.generators[..argument.responses.len()],
    )
    .map_err(|_| McCodecError::InvalidEncoding)?
        + key.hiding_generator.mul_scalar(argument.delta_response);
    if lhs_vector != rhs_vector {
        return invalid();
    }
    let result = inner_product(&argument.responses, public_vector)
        .map_err(|_| McCodecError::InvalidEncoding)?;
    let lhs_result = result_commitment.mul_scalar(challenge) + argument.beta;
    let rhs_result = evaluation_key.generators[0].mul_scalar(result)
        + evaluation_key
            .hiding_generator
            .mul_scalar(argument.beta_response);
    if lhs_result != rhs_result {
        return invalid();
    }
    Ok(())
}

fn scalar_from_usize(value: usize) -> Result<Scalar, McCodecError> {
    u64::try_from(value)
        .map(Scalar::from_u64)
        .map_err(|_| McCodecError::InvalidEncoding)
}

fn invalid<T>() -> Result<T, McCodecError> {
    Err(McCodecError::InvalidEncoding)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PYTHON_VK: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vendor/vega-prover/reference/fixtures/cubic/python_vk.bin"
    ));
    const PYTHON_PROOF: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vendor/vega-prover/reference/fixtures/cubic/python_standalone_proof.bin"
    ));

    #[test]
    fn independent_python_proof_verifies_and_equation_mutation_fails() {
        let key = McVerifierKeyWire::decode(PYTHON_VK).expect("canonical Python key");
        assert!(
            !key.verifier_regular_shape.variables.is_power_of_two(),
            "the fixture exercises Spartan's actual-variable/padded-table split"
        );
        let dimensions = key.proof_dimensions().expect("key-derived dimensions");
        let proof = McProofWire::decode(PYTHON_PROOF, &dimensions).expect("canonical Python proof");
        let (step, core) = verify(&proof, &key, key.num_steps).expect("independent proof verifies");
        assert_eq!(step, vec![vec![Scalar::from_u64(15)]; key.num_steps]);
        assert_eq!(core, vec![Scalar::from_u64(15)]);

        let mut corrupted = proof.clone();
        corrupted.relaxed_spartan.error_blinding += Scalar::one();
        assert!(verify(&corrupted, &key, key.num_steps).is_err());
    }

    #[test]
    fn application_equality_table_bound_covers_exact_figure9_width() {
        assert_eq!(application_eq_table_size(21), Ok(1 << 21));
        assert_eq!(
            application_eq_table_size(22),
            Err(McCodecError::InvalidEncoding)
        );
    }
}
