//! Dependency-free semantic core for the pinned Microsoft Figure 9 prover.
//!
//! This is a value-order port of the eight-way NeutronNova application NIFS,
//! the batched outer/inner Spartan sum-check schedule, and
//! `VegaMcVerifierCircuit::rounds` from the governed Microsoft sources.  It
//! stops after constructing and equation-checking the 47-round verifier
//! instance/witness.  The later random-instance Nova fold, relaxed Spartan
//! proof, and linear IPA are deliberately outside this module.

use super::super::{
    VegaT256ScalarV1 as Scalar,
    algebra::{eq_evals, eq_evaluate},
    commitment::{CommitmentKey, fold},
    r1cs::Instance,
    transcript::VegaTranscriptV1,
};
use super::{
    Figure9SemanticEngineError,
    application_prep::GovernedFigure9ApplicationPrep,
    prover_key::McProverKeyWire,
    split_adapter::Figure9SecretScalars,
    verifier_key::{McVerifierKeyWire, MultiRoundShapeWire, SparseMatrixWire, SplitShapeWire},
    verify,
    wire::{McCommitment, MultiRoundInstanceWire},
};

const FIGURE9_STEPS: usize = 8;
const NIFS_ROUNDS: usize = 3;
const OUTER_ROUNDS: usize = 18;
const INNER_ROUNDS: usize = 21;
const VERIFIER_ROUNDS: usize = 47;
const VERIFIER_PUBLIC_VALUES: usize = 6;
const APPLICATION_VARIABLES: usize = 1 << 20;
const APPLICATION_CONSTRAINTS: usize = 1 << 18;

// Exact read-only provenance for the dependency-free port.
#[cfg(test)]
const VEGA_MC_ZKP_SOURCE_SHA256: &str =
    "fe46c92678f11e6c5238ca836e2c8a0ea311e18dca4621f3507292deeadec01f";
#[cfg(test)]
const VEGA_VERIFIER_CIRCUIT_SOURCE_SHA256: &str =
    "8823d002c7645600d11f49ebbf4c53d2d77f9475d009dad55f61e8cb65c12694";
#[cfg(test)]
const VEGA_SUMCHECK_SOURCE_SHA256: &str =
    "085f93ae6c597abeed3fbe9ec23bd2fa4b7a37a99334114ab237de3e70d95dce";
#[cfg(test)]
const PYVEGA_VERIFIER_CIRCUIT_SOURCE_SHA256: &str =
    "e0758b4f51e89a72a303ef770d85ac9d637d0fa435f9bc69454fee7086cd65e5";

/// Move-only state at the exact boundary before the random verifier-instance
/// Nova fold.  Every retained witness scalar and blinding is zeroized by its
/// owner; commitments, transcript challenges, and public values are not.
pub(super) struct GovernedFigure9SemanticPrep<'a> {
    pub(super) application: GovernedFigure9ApplicationPrep<'a>,
    pub(super) verifier_witness: Figure9SecretScalars,
    pub(super) verifier_blindings: Figure9SecretScalars,
    pub(super) verifier_instance: MultiRoundInstanceWire,
    pub(super) folded_step_weights: Figure9SecretScalars,
    pub(super) folded_step_blindings: Figure9SecretScalars,
    pub(super) folded_step_instance: Instance,
    pub(super) inner_point: Vec<Scalar>,
    pub(super) transcript: Option<VegaTranscriptV1>,
}

/// A heap table whose live prefix is always wiped before release.
struct SecretTable {
    logical_len: usize,
    values: Vec<Scalar>,
}

impl SecretTable {
    fn try_zeroed(
        logical_len: usize,
        stored_len: usize,
    ) -> Result<Self, Figure9SemanticEngineError> {
        if logical_len == 0 || !logical_len.is_power_of_two() || stored_len > logical_len {
            return Err(Figure9SemanticEngineError::InvalidShape);
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(stored_len)
            .map_err(|_| Figure9SemanticEngineError::ResourceExhausted)?;
        values.resize(stored_len, Scalar::zero());
        Ok(Self {
            logical_len,
            values,
        })
    }

    fn try_from_values(
        logical_len: usize,
        values: Vec<Scalar>,
    ) -> Result<Self, Figure9SemanticEngineError> {
        if logical_len == 0 || !logical_len.is_power_of_two() || values.len() > logical_len {
            return Err(Figure9SemanticEngineError::InvalidShape);
        }
        Ok(Self {
            logical_len,
            values,
        })
    }

    fn len(&self) -> usize {
        self.logical_len
    }

    fn value(&self, index: usize) -> Scalar {
        self.values.get(index).copied().unwrap_or_else(Scalar::zero)
    }

    fn bind_top(&mut self, challenge: Scalar) -> Result<(), Figure9SemanticEngineError> {
        if self.logical_len < 2 || !self.logical_len.is_power_of_two() {
            return Err(Figure9SemanticEngineError::InvalidShape);
        }
        let half = self.logical_len / 2;
        let lower_stored = self.values.len().min(half);
        let upper_stored = self.values.len().saturating_sub(half).min(half);
        let result_stored = lower_stored.max(upper_stored);
        for index in 0..result_stored {
            let lower = self.value(index);
            let upper = self.value(half + index);
            self.values[index] = lower + challenge * (upper - lower);
        }
        clear_secret_scalars(&mut self.values[result_stored..]);
        self.values.truncate(result_stored);
        self.logical_len = half;
        Ok(())
    }

    fn fold_with(
        &mut self,
        other: &Self,
        challenge: Scalar,
    ) -> Result<(), Figure9SemanticEngineError> {
        if self.logical_len != other.logical_len || self.values.len() != other.values.len() {
            return Err(Figure9SemanticEngineError::InvalidShape);
        }
        for (left, right) in self.values.iter_mut().zip(&other.values) {
            *left += challenge * (*right - *left);
        }
        Ok(())
    }
}

impl Drop for SecretTable {
    fn drop(&mut self) {
        clear_secret_scalars(&mut self.values);
    }
}

struct SplitSecretTable {
    half_len: usize,
    lower: SecretTable,
    upper: SecretTable,
}

impl SplitSecretTable {
    fn new(
        half_len: usize,
        lower: SecretTable,
        upper: SecretTable,
    ) -> Result<Self, Figure9SemanticEngineError> {
        if half_len == 0
            || !half_len.is_power_of_two()
            || lower.logical_len != half_len
            || upper.logical_len != half_len
        {
            return Err(Figure9SemanticEngineError::InvalidShape);
        }
        Ok(Self {
            half_len,
            lower,
            upper,
        })
    }

    fn value_lower(&self, index: usize) -> Scalar {
        self.lower.value(index)
    }

    fn value_upper(&self, index: usize) -> Scalar {
        self.upper.value(index)
    }

    fn bind_first(self, challenge: Scalar) -> Result<SecretTable, Figure9SemanticEngineError> {
        let stored = self.lower.values.len().max(self.upper.values.len());
        let mut values = SecretTable::try_zeroed(self.half_len, stored)?;
        for index in 0..stored {
            let lower = self.value_lower(index);
            let upper = self.value_upper(index);
            values.values[index] = lower + challenge * (upper - lower);
        }
        Ok(values)
    }
}

struct MatrixLayer {
    a: SecretTable,
    b: SecretTable,
    c: SecretTable,
}

impl MatrixLayer {
    fn fold_with(
        &mut self,
        other: &Self,
        challenge: Scalar,
    ) -> Result<(), Figure9SemanticEngineError> {
        self.a.fold_with(&other.a, challenge)?;
        self.b.fold_with(&other.b, challenge)?;
        self.c.fold_with(&other.c, challenge)
    }
}

struct RoundMachine {
    key: CommitmentKey,
    shape: MultiRoundShapeWire,
    witness: Figure9SecretScalars,
    blindings: Figure9SecretScalars,
    commitments: Vec<McCommitment>,
    challenges: Vec<Vec<Scalar>>,
}

impl RoundMachine {
    fn new(
        key: CommitmentKey,
        shape: &MultiRoundShapeWire,
    ) -> Result<Self, Figure9SemanticEngineError> {
        let witness_len = shape
            .variables_per_round
            .iter()
            .try_fold(0_usize, |sum, count| sum.checked_add(*count))
            .ok_or(Figure9SemanticEngineError::InvalidShape)?;
        let blinding_len = shape
            .variables_per_round
            .iter()
            .try_fold(0_usize, |sum, count| {
                sum.checked_add(count.div_ceil(shape.commitment_width))
            })
            .ok_or(Figure9SemanticEngineError::InvalidShape)?;
        Ok(Self {
            key,
            shape: shape.clone(),
            witness: Figure9SecretScalars::with_capacity(witness_len),
            blindings: Figure9SecretScalars::with_capacity(blinding_len),
            commitments: Vec::with_capacity(shape.rounds),
            challenges: Vec::with_capacity(shape.rounds),
        })
    }

    fn process_round(
        &mut self,
        values: Vec<Scalar>,
        rng: &mut super::rng::Figure9StdRng,
        transcript: &mut VegaTranscriptV1,
    ) -> Result<Vec<Scalar>, Figure9SemanticEngineError> {
        let round = self.commitments.len();
        let expected_unpadded = *self
            .shape
            .variables_per_round_unpadded
            .get(round)
            .ok_or(Figure9SemanticEngineError::InvalidShape)?;
        let padded = *self
            .shape
            .variables_per_round
            .get(round)
            .ok_or(Figure9SemanticEngineError::InvalidShape)?;
        if values.len() != expected_unpadded || values.len() > padded {
            return Err(Figure9SemanticEngineError::InvalidRoundWitness);
        }
        let values = Figure9SecretScalars::from_vec(values);
        let rows = padded.div_ceil(self.shape.commitment_width);
        let mut round_blindings = Figure9SecretScalars::with_capacity(rows);
        for _ in 0..rows {
            round_blindings.push(rng.scalar());
        }
        let commitment = self
            .key
            .commit_padded_prefix(values.as_slice(), padded, round_blindings.as_slice())
            .map_err(|_| Figure9SemanticEngineError::Commitment)?;
        let commitment = McCommitment {
            points: commitment.into_points(),
        };
        transcript
            .absorb_commitment(
                b"comm_w_round",
                &commitment
                    .to_local()
                    .map_err(|_| Figure9SemanticEngineError::Transcript)?,
            )
            .map_err(|_| Figure9SemanticEngineError::Transcript)?;
        let challenge_count = *self
            .shape
            .challenges_per_round
            .get(round)
            .ok_or(Figure9SemanticEngineError::InvalidShape)?;
        let mut challenges = Vec::with_capacity(challenge_count);
        for _ in 0..challenge_count {
            challenges.push(
                transcript
                    .squeeze(b"challenge")
                    .map_err(|_| Figure9SemanticEngineError::Transcript)?,
            );
        }
        for value in values.iter().copied() {
            self.witness.push(value);
        }
        for _ in values.len()..padded {
            self.witness.push(Scalar::zero());
        }
        for blinding in round_blindings.iter().copied() {
            self.blindings.push(blinding);
        }
        self.commitments.push(commitment);
        self.challenges.push(challenges.clone());
        Ok(challenges)
    }

    fn finish(
        self,
        public_values: Vec<Scalar>,
    ) -> Result<RoundMachineOutput, Figure9SemanticEngineError> {
        if self.commitments.len() != self.shape.rounds
            || self.challenges.len() != self.shape.rounds
            || public_values.len() != self.shape.public_values
        {
            return Err(Figure9SemanticEngineError::InvalidShape);
        }
        let instance = MultiRoundInstanceWire {
            commitments: self.commitments,
            public_values,
            challenges_per_round: self.challenges,
        };
        Ok(RoundMachineOutput {
            witness: self.witness,
            blindings: self.blindings,
            instance,
        })
    }
}

struct RoundMachineOutput {
    witness: Figure9SecretScalars,
    blindings: Figure9SecretScalars,
    instance: MultiRoundInstanceWire,
}

fn clear_secret_scalars(values: &mut [Scalar]) {
    let values = core::hint::black_box(values);
    for value in values.iter_mut() {
        value.clear_secret();
    }
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    let _ = core::hint::black_box(&mut *values);
}

pub(super) fn build<'a>(
    mut application: GovernedFigure9ApplicationPrep<'a>,
    prover_key: &McProverKeyWire,
    verifier_key: &McVerifierKeyWire,
) -> Result<GovernedFigure9SemanticPrep<'a>, Figure9SemanticEngineError> {
    validate_geometry(&application, prover_key, verifier_key)?;
    let verifier_commitment_key = verify::derive_and_match_key(&prover_key.verifier_commitment_key)
        .map_err(|_| Figure9SemanticEngineError::InvalidKey)?;
    let mut transcript = start_application_transcript(&application, verifier_key)?;

    let tau = transcript
        .squeeze(b"tau")
        .map_err(|_| Figure9SemanticEngineError::Transcript)?;
    let mut rhos = [Scalar::zero(); NIFS_ROUNDS];
    for rho in &mut rhos {
        *rho = transcript
            .squeeze(b"rho")
            .map_err(|_| Figure9SemanticEngineError::Transcript)?;
    }
    let replay_transcript = transcript.clone();

    let mut rng = application
        .rng
        .take()
        .ok_or(Figure9SemanticEngineError::InvalidShape)?;
    let mut rounds = RoundMachine::new(verifier_commitment_key, &prover_key.verifier_shape)?;
    let nifs = prove_application_nifs(
        &application,
        &prover_key.step_shape,
        tau,
        rhos,
        &mut rounds,
        &mut transcript,
        &mut rng,
    )?;
    let core_layer = multiply_core_layer(&application, &prover_key.core_shape)?;
    let outer = prove_outer_sumcheck(
        tau,
        nifs.folded_layer,
        core_layer,
        nifs.t_out,
        &mut rounds,
        &mut transcript,
        &mut rng,
    )?;
    let inner = prove_inner_sumcheck(
        &application,
        &prover_key.step_shape,
        &prover_key.core_shape,
        &nifs.weights,
        &outer,
        &mut rounds,
        &mut transcript,
        &mut rng,
    )?;
    let output = rounds.finish(vec![
        outer.tau_at_rx,
        inner.eval_x_step,
        inner.eval_x_core,
        nifs.acc_eq,
        inner.quotient_step,
        inner.quotient_core,
    ])?;

    verify_round_instance_and_equations(&output, &prover_key.verifier_shape, replay_transcript)?;
    let (folded_step_instance, folded_step_blindings) =
        fold_application_instance(&application, &nifs.weights)?;
    application.rng = Some(rng);
    let mut folded_step_weights = Figure9SecretScalars::with_capacity(FIGURE9_STEPS);
    for weight in nifs.weights {
        folded_step_weights.push(weight);
    }
    Ok(GovernedFigure9SemanticPrep {
        application,
        verifier_witness: output.witness,
        verifier_blindings: output.blindings,
        verifier_instance: output.instance,
        folded_step_weights,
        folded_step_blindings,
        folded_step_instance,
        inner_point: inner.r_y,
        transcript: Some(transcript),
    })
}

fn fold_application_instance(
    application: &GovernedFigure9ApplicationPrep<'_>,
    weights: &[Scalar; FIGURE9_STEPS],
) -> Result<(Instance, Figure9SecretScalars), Figure9SemanticEngineError> {
    let mut instances = Vec::with_capacity(FIGURE9_STEPS);
    for step in &application.step_instances {
        let mut with_shared = step.clone();
        with_shared.shared = Some(application.shared_commitment.clone());
        instances.push(
            verify::split_to_regular(&with_shared)
                .map_err(|_| Figure9SemanticEngineError::InvalidApplicationInstance)?,
        );
    }
    let references = instances
        .iter()
        .map(|instance| &instance.witness_commitment)
        .collect::<Vec<_>>();
    let witness_commitment =
        fold(&references, weights).map_err(|_| Figure9SemanticEngineError::Commitment)?;
    let public_len = instances
        .first()
        .ok_or(Figure9SemanticEngineError::InvalidShape)?
        .public_inputs
        .len();
    if instances
        .iter()
        .any(|instance| instance.public_inputs.len() != public_len)
    {
        return Err(Figure9SemanticEngineError::InvalidShape);
    }
    let mut public_inputs = vec![Scalar::zero(); public_len];
    for (weight, instance) in weights.iter().zip(&instances) {
        for (output, value) in public_inputs.iter_mut().zip(&instance.public_inputs) {
            *output += *weight * *value;
        }
    }
    let shared_rows = application.shared_blindings.len();
    let precommitted_rows = application.step_private[0].precommitted_blindings.len();
    let rest_rows = application.step_private[0].rest_blindings.len();
    let total_rows = shared_rows
        .checked_add(precommitted_rows)
        .and_then(|rows| rows.checked_add(rest_rows))
        .ok_or(Figure9SemanticEngineError::InvalidShape)?;
    let mut blindings = Figure9SecretScalars::with_capacity(total_rows);
    let weight_sum = weights
        .iter()
        .copied()
        .fold(Scalar::zero(), |sum, weight| sum + weight);
    if weight_sum != Scalar::one() {
        return Err(Figure9SemanticEngineError::InvalidNifs);
    }
    for blinding in application.shared_blindings.iter().copied() {
        blindings.push(weight_sum * blinding);
    }
    for row in 0..precommitted_rows {
        let value = weights
            .iter()
            .zip(&application.step_private)
            .fold(Scalar::zero(), |sum, (weight, private)| {
                sum + *weight * private.precommitted_blindings[row]
            });
        blindings.push(value);
    }
    for row in 0..rest_rows {
        let value = weights
            .iter()
            .zip(&application.step_private)
            .fold(Scalar::zero(), |sum, (weight, private)| {
                sum + *weight * private.rest_blindings[row]
            });
        blindings.push(value);
    }
    Ok((
        Instance {
            witness_commitment,
            public_inputs,
        },
        blindings,
    ))
}

fn validate_geometry(
    application: &GovernedFigure9ApplicationPrep<'_>,
    prover_key: &McProverKeyWire,
    verifier_key: &McVerifierKeyWire,
) -> Result<(), Figure9SemanticEngineError> {
    prover_key
        .validate_against(verifier_key)
        .map_err(|_| Figure9SemanticEngineError::InvalidKey)?;
    let shape = &prover_key.verifier_shape;
    let expected_unpadded = expected_round_unpadded();
    if application.step_instances.len() != FIGURE9_STEPS
        || verifier_key.num_steps != FIGURE9_STEPS
        || prover_key.step_shape.variables().ok() != Some(APPLICATION_VARIABLES)
        || prover_key.core_shape.variables().ok() != Some(APPLICATION_VARIABLES)
        || prover_key.step_shape.constraints != APPLICATION_CONSTRAINTS
        || prover_key.core_shape.constraints != APPLICATION_CONSTRAINTS
        || shape.rounds != VERIFIER_ROUNDS
        || shape.public_values != VERIFIER_PUBLIC_VALUES
        || shape.variables_per_round_unpadded != expected_unpadded
        || shape.variables_per_round.len() != VERIFIER_ROUNDS
        || shape
            .variables_per_round
            .iter()
            .any(|count| *count != shape.commitment_width)
        || shape.challenges_per_round.len() != VERIFIER_ROUNDS
        || shape
            .challenges_per_round
            .iter()
            .enumerate()
            .any(|(index, count)| {
                let expected = usize::from(!matches!(index, 3 | 44 | 45 | 46));
                *count != expected
            })
    {
        return Err(Figure9SemanticEngineError::InvalidShape);
    }
    Ok(())
}

fn expected_round_unpadded() -> Vec<usize> {
    let mut values = Vec::with_capacity(VERIFIER_ROUNDS);
    values.extend_from_slice(&[5, 7, 7, 5]);
    values.push(9);
    values.extend(core::iter::repeat_n(14, OUTER_ROUNDS - 1));
    values.push(15);
    values.push(11);
    values.extend(core::iter::repeat_n(10, INNER_ROUNDS - 1));
    values.push(10);
    values.extend_from_slice(&[32, 32]);
    values
}

fn start_application_transcript(
    application: &GovernedFigure9ApplicationPrep<'_>,
    verifier_key: &McVerifierKeyWire,
) -> Result<VegaTranscriptV1, Figure9SemanticEngineError> {
    let digest = verifier_key
        .digest()
        .map_err(|_| Figure9SemanticEngineError::InvalidKey)?;
    let mut core = application.core_instance.clone();
    core.shared = Some(application.shared_commitment.clone());
    let core = verify::split_to_regular(&core)
        .map_err(|_| Figure9SemanticEngineError::InvalidApplicationInstance)?;
    let mut transcript = VegaTranscriptV1::new_neutron_nova();
    transcript
        .absorb_raw(b"vk", &digest)
        .map_err(|_| Figure9SemanticEngineError::Transcript)?;
    transcript
        .absorb_r1cs_instance(
            b"core_instance",
            &core.witness_commitment,
            &core.public_inputs,
        )
        .map_err(|_| Figure9SemanticEngineError::Transcript)?;
    for instance in &application.step_instances {
        let mut instance = instance.clone();
        instance.shared = Some(application.shared_commitment.clone());
        let regular = verify::split_to_regular(&instance)
            .map_err(|_| Figure9SemanticEngineError::InvalidApplicationInstance)?;
        transcript
            .absorb_r1cs_instance(b"U", &regular.witness_commitment, &regular.public_inputs)
            .map_err(|_| Figure9SemanticEngineError::Transcript)?;
    }
    transcript
        .absorb_scalar(b"T", Scalar::zero())
        .map_err(|_| Figure9SemanticEngineError::Transcript)?;
    Ok(transcript)
}

struct NifsOutput {
    folded_layer: MatrixLayer,
    weights: [Scalar; FIGURE9_STEPS],
    t_out: Scalar,
    acc_eq: Scalar,
}

fn prove_application_nifs(
    application: &GovernedFigure9ApplicationPrep<'_>,
    shape: &SplitShapeWire,
    tau: Scalar,
    rhos: [Scalar; NIFS_ROUNDS],
    rounds: &mut RoundMachine,
    transcript: &mut VegaTranscriptV1,
    rng: &mut super::rng::Figure9StdRng,
) -> Result<NifsOutput, Figure9SemanticEngineError> {
    let mut current_claim = Scalar::zero();
    let mut acc_eq = Scalar::one();
    let mut polynomials = [[Scalar::zero(); 4]; NIFS_ROUNDS];
    let (e0, quadratic) = stream_first_nifs_round(application, shape, tau, &rhos)?;
    polynomials[0] = finish_nifs_polynomial(current_claim, acc_eq, rhos[0], e0, quadratic)?;
    let challenge_0 = one_challenge(rounds.process_round(
        nifs_round_witness(0, &polynomials, &[], Scalar::zero())?,
        rng,
        transcript,
    )?)?;
    acc_eq *= equality_factor(challenge_0, rhos[0]);
    current_claim = evaluate_polynomial(&polynomials[0], challenge_0);

    let mut layers = Vec::with_capacity(FIGURE9_STEPS / 2);
    for pair in 0..FIGURE9_STEPS / 2 {
        layers.push(multiply_folded_step_layer(
            application,
            shape,
            2 * pair,
            2 * pair + 1,
            challenge_0,
        )?);
    }
    let mut challenges = vec![challenge_0];
    for round_index in 1..NIFS_ROUNDS {
        let (e0, quadratic) = nifs_layer_round_stats(&layers, tau, round_index, &rhos)?;
        polynomials[round_index] =
            finish_nifs_polynomial(current_claim, acc_eq, rhos[round_index], e0, quadratic)?;
        let challenge = one_challenge(rounds.process_round(
            nifs_round_witness(round_index, &polynomials, &challenges, current_claim)?,
            rng,
            transcript,
        )?)?;
        acc_eq *= equality_factor(challenge, rhos[round_index]);
        current_claim = evaluate_polynomial(&polynomials[round_index], challenge);
        layers = fold_layer_pairs(layers, challenge)?;
        challenges.push(challenge);
    }
    if layers.len() != 1 || acc_eq.is_zero() {
        return Err(Figure9SemanticEngineError::InvalidNifs);
    }
    let t_out = current_claim
        * acc_eq
            .inverse()
            .map_err(|_| Figure9SemanticEngineError::DivisionByZero)?;
    let final_witness = nifs_final_witness(
        &polynomials[NIFS_ROUNDS - 1],
        challenges[NIFS_ROUNDS - 1],
        t_out,
        acc_eq,
    )?;
    if !rounds
        .process_round(final_witness, rng, transcript)?
        .is_empty()
    {
        return Err(Figure9SemanticEngineError::InvalidShape);
    }
    if eq_evaluate(&challenges, &rhos).map_err(|_| Figure9SemanticEngineError::InvalidNifs)?
        != acc_eq
    {
        return Err(Figure9SemanticEngineError::InvalidNifs);
    }
    let weights = weights_from_challenges(&challenges)?;
    Ok(NifsOutput {
        folded_layer: layers
            .pop()
            .ok_or(Figure9SemanticEngineError::InvalidNifs)?,
        weights,
        t_out,
        acc_eq,
    })
}

fn stream_first_nifs_round(
    application: &GovernedFigure9ApplicationPrep<'_>,
    shape: &SplitShapeWire,
    tau: Scalar,
    rhos: &[Scalar; NIFS_ROUNDS],
) -> Result<(Scalar, Scalar), Figure9SemanticEngineError> {
    let mut quadratic = Scalar::zero();
    let mut power = Scalar::one();
    for row in 0..shape.constraints {
        for pair in 0..FIGURE9_STEPS / 2 {
            let low = row_products_step(application, shape, 2 * pair, row)?;
            let high = row_products_step(application, shape, 2 * pair + 1, row)?;
            let weight = suffix_weight(0, pair, rhos);
            quadratic += power * weight * (high[0] - low[0]) * (high[1] - low[1]);
        }
        power *= tau;
    }
    // Every governed step assignment has already passed all three equations,
    // so the round-zero constant contribution is exactly zero.
    Ok((Scalar::zero(), quadratic))
}

fn nifs_layer_round_stats(
    layers: &[MatrixLayer],
    tau: Scalar,
    round_index: usize,
    rhos: &[Scalar; NIFS_ROUNDS],
) -> Result<(Scalar, Scalar), Figure9SemanticEngineError> {
    if layers.len() < 2 || !layers.len().is_multiple_of(2) {
        return Err(Figure9SemanticEngineError::InvalidNifs);
    }
    let mut e0 = Scalar::zero();
    let mut quadratic = Scalar::zero();
    let mut power = Scalar::one();
    for row in 0..APPLICATION_CONSTRAINTS {
        for pair in 0..layers.len() / 2 {
            let low = &layers[2 * pair];
            let high = &layers[2 * pair + 1];
            let weight = suffix_weight(round_index, pair, rhos);
            e0 += power * weight * (low.a.value(row) * low.b.value(row) - low.c.value(row));
            quadratic += power
                * weight
                * (high.a.value(row) - low.a.value(row))
                * (high.b.value(row) - low.b.value(row));
        }
        power *= tau;
    }
    Ok((e0, quadratic))
}

fn suffix_weight(round_index: usize, mut pair: usize, rhos: &[Scalar; NIFS_ROUNDS]) -> Scalar {
    let mut weight = Scalar::one();
    for rho in rhos.iter().skip(round_index + 1) {
        weight *= if pair & 1 == 0 {
            Scalar::one() - *rho
        } else {
            *rho
        };
        pair >>= 1;
    }
    weight
}

fn finish_nifs_polynomial(
    current_claim: Scalar,
    acc_eq: Scalar,
    rho: Scalar,
    e0: Scalar,
    quadratic: Scalar,
) -> Result<[Scalar; 4], Figure9SemanticEngineError> {
    let one_minus_rho = Scalar::one() - rho;
    let two_rho_minus_one = rho - one_minus_rho;
    let c = e0 * acc_eq;
    let a = quadratic * acc_eq;
    let b = (current_claim - c * one_minus_rho)
        * rho
            .inverse()
            .map_err(|_| Figure9SemanticEngineError::DivisionByZero)?
        - a
        - c;
    let polynomial = [
        c * one_minus_rho,
        c * two_rho_minus_one + b * one_minus_rho,
        b * two_rho_minus_one + a * one_minus_rho,
        a * two_rho_minus_one,
    ];
    if boolean_sum(&polynomial) != current_claim {
        return Err(Figure9SemanticEngineError::InvalidNifs);
    }
    Ok(polynomial)
}

fn equality_factor(left: Scalar, right: Scalar) -> Scalar {
    left * right + (Scalar::one() - left) * (Scalar::one() - right)
}

fn fold_layer_pairs(
    mut layers: Vec<MatrixLayer>,
    challenge: Scalar,
) -> Result<Vec<MatrixLayer>, Figure9SemanticEngineError> {
    if layers.len() < 2 || !layers.len().is_multiple_of(2) {
        return Err(Figure9SemanticEngineError::InvalidNifs);
    }
    let mut folded = Vec::with_capacity(layers.len() / 2);
    let mut drain = layers.drain(..);
    while let Some(mut left) = drain.next() {
        let right = drain
            .next()
            .ok_or(Figure9SemanticEngineError::InvalidNifs)?;
        left.fold_with(&right, challenge)?;
        folded.push(left);
    }
    Ok(folded)
}

fn weights_from_challenges(
    challenges: &[Scalar],
) -> Result<[Scalar; FIGURE9_STEPS], Figure9SemanticEngineError> {
    if challenges.len() != NIFS_ROUNDS {
        return Err(Figure9SemanticEngineError::InvalidNifs);
    }
    Ok(core::array::from_fn(|index| {
        challenges
            .iter()
            .copied()
            .enumerate()
            .fold(Scalar::one(), |weight, (bit, challenge)| {
                if index & (1 << bit) == 0 {
                    weight * (Scalar::one() - challenge)
                } else {
                    weight * challenge
                }
            })
    }))
}

struct OuterOutput {
    r_x: Vec<Scalar>,
    batching: Scalar,
    claims_step: [Scalar; 3],
    claims_core: [Scalar; 3],
    tau_at_rx: Scalar,
}

fn prove_outer_sumcheck(
    tau: Scalar,
    folded_step: MatrixLayer,
    core: MatrixLayer,
    t_out: Scalar,
    rounds: &mut RoundMachine,
    transcript: &mut VegaTranscriptV1,
    rng: &mut super::rng::Figure9StdRng,
) -> Result<OuterOutput, Figure9SemanticEngineError> {
    let mut pow_tau = SecretTable::try_zeroed(APPLICATION_CONSTRAINTS, APPLICATION_CONSTRAINTS)?;
    let mut power = Scalar::one();
    for value in &mut pow_tau.values {
        *value = power;
        power *= tau;
    }
    let MatrixLayer {
        a: mut a_step,
        b: mut b_step,
        c: mut c_step,
    } = folded_step;
    let MatrixLayer {
        a: mut a_core,
        b: mut b_core,
        c: mut c_core,
    } = core;
    let mut claim_step = t_out;
    let mut claim_core = Scalar::zero();
    let mut previous_step = [Scalar::zero(); 4];
    let mut previous_core = [Scalar::zero(); 4];
    let mut previous_challenge = Scalar::zero();
    let mut r_x = Vec::with_capacity(OUTER_ROUNDS);
    for index in 0..OUTER_ROUNDS {
        let polynomial_step =
            outer_round_polynomial(&pow_tau, &a_step, &b_step, &c_step, claim_step)?;
        let polynomial_core =
            outer_round_polynomial(&pow_tau, &a_core, &b_core, &c_core, claim_core)?;
        let witness = outer_round_witness(
            index,
            polynomial_step,
            polynomial_core,
            previous_step,
            previous_core,
            previous_challenge,
            claim_step,
            claim_core,
        )?;
        let challenge = one_challenge(rounds.process_round(witness, rng, transcript)?)?;
        claim_step = evaluate_polynomial(&polynomial_step, challenge);
        claim_core = evaluate_polynomial(&polynomial_core, challenge);
        for table in [
            &mut pow_tau,
            &mut a_step,
            &mut b_step,
            &mut c_step,
            &mut a_core,
            &mut b_core,
            &mut c_core,
        ] {
            table.bind_top(challenge)?;
        }
        previous_step = polynomial_step;
        previous_core = polynomial_core;
        previous_challenge = challenge;
        r_x.push(challenge);
    }
    let claims_step = [a_step.value(0), b_step.value(0), c_step.value(0)];
    let claims_core = [a_core.value(0), b_core.value(0), c_core.value(0)];
    let tau_at_rx = pow_tau.value(0);
    if claim_step != tau_at_rx * (claims_step[0] * claims_step[1] - claims_step[2])
        || claim_core != tau_at_rx * (claims_core[0] * claims_core[1] - claims_core[2])
    {
        return Err(Figure9SemanticEngineError::InvalidOuterSumcheck);
    }
    let final_witness = outer_final_witness(
        previous_step,
        previous_core,
        previous_challenge,
        claims_step,
        claims_core,
        tau_at_rx,
    )?;
    let batching = one_challenge(rounds.process_round(final_witness, rng, transcript)?)?;
    Ok(OuterOutput {
        r_x,
        batching,
        claims_step,
        claims_core,
        tau_at_rx,
    })
}

fn outer_round_polynomial(
    power: &SecretTable,
    a: &SecretTable,
    b: &SecretTable,
    c: &SecretTable,
    claim: Scalar,
) -> Result<[Scalar; 4], Figure9SemanticEngineError> {
    let len = power.len();
    if len < 2 || a.len() != len || b.len() != len || c.len() != len {
        return Err(Figure9SemanticEngineError::InvalidOuterSumcheck);
    }
    let half = len / 2;
    let points = [
        Scalar::zero(),
        Scalar::one(),
        Scalar::from_u64(2),
        Scalar::from_u64(3),
    ];
    let mut evaluations = [Scalar::zero(); 4];
    for index in 0..half {
        for (evaluation, point) in evaluations.iter_mut().zip(points) {
            let p = affine(power.value(index), power.value(half + index), point);
            let av = affine(a.value(index), a.value(half + index), point);
            let bv = affine(b.value(index), b.value(half + index), point);
            let cv = affine(c.value(index), c.value(half + index), point);
            *evaluation += p * (av * bv - cv);
        }
    }
    if evaluations[0] + evaluations[1] != claim {
        return Err(Figure9SemanticEngineError::InvalidOuterSumcheck);
    }
    interpolate_cubic(evaluations)
}

struct InnerOutput {
    r_y: Vec<Scalar>,
    eval_x_step: Scalar,
    eval_x_core: Scalar,
    quotient_step: Scalar,
    quotient_core: Scalar,
}

#[allow(clippy::too_many_arguments)]
fn prove_inner_sumcheck(
    application: &GovernedFigure9ApplicationPrep<'_>,
    step_shape: &SplitShapeWire,
    core_shape: &SplitShapeWire,
    weights: &[Scalar; FIGURE9_STEPS],
    outer: &OuterOutput,
    rounds: &mut RoundMachine,
    transcript: &mut VegaTranscriptV1,
    rng: &mut super::rng::Figure9StdRng,
) -> Result<InnerOutput, Figure9SemanticEngineError> {
    let row_weights =
        eq_evals(&outer.r_x).map_err(|_| Figure9SemanticEngineError::InvalidInnerSumcheck)?;
    let abc_step = build_abc_table(step_shape, &row_weights, outer.batching)?;
    let abc_core = build_abc_table(core_shape, &row_weights, outer.batching)?;
    let (z_step, folded_public) = build_folded_step_z(application, step_shape, weights)?;
    let z_core = build_core_z(application, core_shape)?;
    let mut claim_step = outer.claims_step[0]
        + outer.batching * outer.claims_step[1]
        + outer.batching.square() * outer.claims_step[2];
    let mut claim_core = outer.claims_core[0]
        + outer.batching * outer.claims_core[1]
        + outer.batching.square() * outer.claims_core[2];

    let polynomial_step = inner_first_round_polynomial(&abc_step, &z_step, claim_step)?;
    let polynomial_core = inner_first_round_polynomial(&abc_core, &z_core, claim_core)?;
    let witness = inner_first_round_witness(
        polynomial_step,
        polynomial_core,
        outer.batching,
        outer.claims_step,
        outer.claims_core,
    )?;
    let first_challenge = one_challenge(rounds.process_round(witness, rng, transcript)?)?;
    claim_step = evaluate_polynomial(&polynomial_step, first_challenge);
    claim_core = evaluate_polynomial(&polynomial_core, first_challenge);
    let mut abc_step = abc_step.bind_first(first_challenge)?;
    let mut abc_core = abc_core.bind_first(first_challenge)?;
    let mut z_step = z_step.bind_first(first_challenge)?;
    let mut z_core = z_core.bind_first(first_challenge)?;
    let mut previous_step = polynomial_step;
    let mut previous_core = polynomial_core;
    let mut previous_challenge = first_challenge;
    let mut r_y = Vec::with_capacity(INNER_ROUNDS);
    r_y.push(first_challenge);

    for _index in 1..INNER_ROUNDS {
        let polynomial_step = inner_round_polynomial(&abc_step, &z_step, claim_step)?;
        let polynomial_core = inner_round_polynomial(&abc_core, &z_core, claim_core)?;
        let witness = inner_later_round_witness(
            polynomial_step,
            polynomial_core,
            previous_step,
            previous_core,
            previous_challenge,
            claim_step,
            claim_core,
        )?;
        let challenge = one_challenge(rounds.process_round(witness, rng, transcript)?)?;
        claim_step = evaluate_polynomial(&polynomial_step, challenge);
        claim_core = evaluate_polynomial(&polynomial_core, challenge);
        for table in [&mut abc_step, &mut abc_core, &mut z_step, &mut z_core] {
            table.bind_top(challenge)?;
        }
        previous_step = polynomial_step;
        previous_core = polynomial_core;
        previous_challenge = challenge;
        r_y.push(challenge);
    }
    let eval_z_step = z_step.value(0);
    let eval_z_core = z_core.value(0);
    if abc_step.value(0) * eval_z_step != claim_step
        || abc_core.value(0) * eval_z_core != claim_core
    {
        return Err(Figure9SemanticEngineError::InvalidInnerSumcheck);
    }
    let eval_x_step = sparse_x_evaluate(&[Scalar::one(), folded_public], &r_y[1..])?;
    let mut core_sparse = Vec::with_capacity(application.core_instance.public_values.len() + 1);
    core_sparse.push(Scalar::one());
    core_sparse.extend_from_slice(&application.core_instance.public_values);
    let eval_x_core = sparse_x_evaluate(&core_sparse, &r_y[1..])?;
    let inverse = (Scalar::one() - r_y[0])
        .inverse()
        .map_err(|_| Figure9SemanticEngineError::DivisionByZero)?;
    let eval_w_step = (eval_z_step - r_y[0] * eval_x_step) * inverse;
    let eval_w_core = (eval_z_core - r_y[0] * eval_x_core) * inverse;
    let sum_z_step = (Scalar::one() - r_y[0]) * eval_w_step + r_y[0] * eval_x_step;
    let sum_z_core = (Scalar::one() - r_y[0]) * eval_w_core + r_y[0] * eval_x_core;
    let quotient_step = checked_quotient(claim_step, sum_z_step)?;
    let quotient_core = checked_quotient(claim_core, sum_z_core)?;
    let final_witness = inner_final_witness(
        previous_step,
        previous_core,
        previous_challenge,
        eval_w_step,
        eval_w_core,
        eval_x_step,
        eval_x_core,
        r_y[0],
        claim_step,
        claim_core,
    )?;
    if !rounds
        .process_round(final_witness, rng, transcript)?
        .is_empty()
    {
        return Err(Figure9SemanticEngineError::InvalidShape);
    }
    if !rounds
        .process_round(
            evaluation_commit_round_witness(eval_w_step),
            rng,
            transcript,
        )?
        .is_empty()
        || !rounds
            .process_round(
                evaluation_commit_round_witness(eval_w_core),
                rng,
                transcript,
            )?
            .is_empty()
    {
        return Err(Figure9SemanticEngineError::InvalidShape);
    }
    Ok(InnerOutput {
        r_y,
        eval_x_step,
        eval_x_core,
        quotient_step,
        quotient_core,
    })
}

fn inner_first_round_polynomial(
    a: &SplitSecretTable,
    b: &SplitSecretTable,
    claim: Scalar,
) -> Result<[Scalar; 3], Figure9SemanticEngineError> {
    if a.half_len != b.half_len {
        return Err(Figure9SemanticEngineError::InvalidInnerSumcheck);
    }
    let count = a
        .lower
        .values
        .len()
        .max(a.upper.values.len())
        .max(b.lower.values.len())
        .max(b.upper.values.len());
    let points = [Scalar::zero(), Scalar::one(), Scalar::from_u64(2)];
    let mut evaluations = [Scalar::zero(); 3];
    for index in 0..count {
        for (evaluation, point) in evaluations.iter_mut().zip(points) {
            let av = affine(a.value_lower(index), a.value_upper(index), point);
            let bv = affine(b.value_lower(index), b.value_upper(index), point);
            *evaluation += av * bv;
        }
    }
    if evaluations[0] + evaluations[1] != claim {
        return Err(Figure9SemanticEngineError::InvalidInnerSumcheck);
    }
    interpolate_quadratic(evaluations)
}

fn inner_round_polynomial(
    a: &SecretTable,
    b: &SecretTable,
    claim: Scalar,
) -> Result<[Scalar; 3], Figure9SemanticEngineError> {
    if a.len() != b.len() || a.len() < 2 {
        return Err(Figure9SemanticEngineError::InvalidInnerSumcheck);
    }
    let half = a.len() / 2;
    let points = [Scalar::zero(), Scalar::one(), Scalar::from_u64(2)];
    let mut evaluations = [Scalar::zero(); 3];
    for index in 0..half {
        for (evaluation, point) in evaluations.iter_mut().zip(points) {
            *evaluation += affine(a.value(index), a.value(half + index), point)
                * affine(b.value(index), b.value(half + index), point);
        }
    }
    if evaluations[0] + evaluations[1] != claim {
        return Err(Figure9SemanticEngineError::InvalidInnerSumcheck);
    }
    interpolate_quadratic(evaluations)
}

fn build_abc_table(
    shape: &SplitShapeWire,
    row_weights: &[Scalar],
    batching: Scalar,
) -> Result<SplitSecretTable, Figure9SemanticEngineError> {
    let variables = shape
        .variables()
        .map_err(|_| Figure9SemanticEngineError::InvalidShape)?;
    if row_weights.len() != shape.constraints {
        return Err(Figure9SemanticEngineError::InvalidShape);
    }
    let lower_len = effective_witness_prefix(shape)?;
    let upper_len = 1_usize
        .checked_add(shape.public_values)
        .and_then(|value| value.checked_add(shape.challenges))
        .ok_or(Figure9SemanticEngineError::InvalidShape)?;
    let mut lower = SecretTable::try_zeroed(variables, lower_len)?;
    let mut upper = SecretTable::try_zeroed(variables, upper_len)?;
    for (matrix, factor) in [
        (&shape.a, Scalar::one()),
        (&shape.b, batching),
        (&shape.c, batching.square()),
    ] {
        for (row, row_weight) in row_weights.iter().copied().enumerate() {
            let row_factor = factor * row_weight;
            for (column, coefficient) in matrix
                .row_entries(row)
                .ok_or(Figure9SemanticEngineError::InvalidShape)?
            {
                if column < variables {
                    let slot = lower
                        .values
                        .get_mut(column)
                        .ok_or(Figure9SemanticEngineError::InvalidShape)?;
                    *slot += row_factor * coefficient;
                } else {
                    let upper_index = column - variables;
                    let slot = upper
                        .values
                        .get_mut(upper_index)
                        .ok_or(Figure9SemanticEngineError::InvalidShape)?;
                    *slot += row_factor * coefficient;
                }
            }
        }
    }
    SplitSecretTable::new(variables, lower, upper)
}

fn build_folded_step_z(
    application: &GovernedFigure9ApplicationPrep<'_>,
    shape: &SplitShapeWire,
    weights: &[Scalar; FIGURE9_STEPS],
) -> Result<(SplitSecretTable, Scalar), Figure9SemanticEngineError> {
    let variables = shape
        .variables()
        .map_err(|_| Figure9SemanticEngineError::InvalidShape)?;
    let lower_len = effective_witness_prefix(shape)?;
    let mut lower = SecretTable::try_zeroed(variables, lower_len)?;
    lower.values[..application.shared_witness.len()].copy_from_slice(application.shared_witness);
    let mut folded_public = Scalar::zero();
    for (weight, instance) in weights.iter().zip(&application.step_instances) {
        folded_public += *weight * instance.public_values[0];
    }
    lower.values[shape.shared] = folded_public;
    for rest_index in 0..shape.rest_unpadded {
        let mut value = Scalar::zero();
        for (weight, private) in weights.iter().zip(&application.step_private) {
            value += *weight * private.rest_values[rest_index];
        }
        lower.values[shape.shared + shape.precommitted + rest_index] = value;
    }
    let upper = SecretTable::try_from_values(variables, vec![Scalar::one(), folded_public])?;
    Ok((
        SplitSecretTable::new(variables, lower, upper)?,
        folded_public,
    ))
}

fn build_core_z(
    application: &GovernedFigure9ApplicationPrep<'_>,
    shape: &SplitShapeWire,
) -> Result<SplitSecretTable, Figure9SemanticEngineError> {
    let variables = shape
        .variables()
        .map_err(|_| Figure9SemanticEngineError::InvalidShape)?;
    let lower_len = effective_witness_prefix(shape)?;
    let mut lower = SecretTable::try_zeroed(variables, lower_len)?;
    lower.values[..application.shared_witness.len()].copy_from_slice(application.shared_witness);
    let public = &application.core_instance.public_values;
    lower.values[shape.shared..shape.shared + public.len()].copy_from_slice(public);
    let mut upper = Vec::with_capacity(public.len() + 1);
    upper.push(Scalar::one());
    upper.extend_from_slice(public);
    SplitSecretTable::new(
        variables,
        lower,
        SecretTable::try_from_values(variables, upper)?,
    )
}

fn effective_witness_prefix(shape: &SplitShapeWire) -> Result<usize, Figure9SemanticEngineError> {
    let prefix = if shape.rest_unpadded > 0 {
        shape
            .shared
            .checked_add(shape.precommitted)
            .and_then(|value| value.checked_add(shape.rest_unpadded))
    } else if shape.precommitted_unpadded > 0 {
        shape.shared.checked_add(shape.precommitted_unpadded)
    } else {
        Some(shape.shared_unpadded)
    }
    .ok_or(Figure9SemanticEngineError::InvalidShape)?;
    if prefix == 0
        || prefix
            > shape
                .variables()
                .map_err(|_| Figure9SemanticEngineError::InvalidShape)?
    {
        return Err(Figure9SemanticEngineError::InvalidShape);
    }
    Ok(prefix)
}

fn sparse_x_evaluate(
    evaluations: &[Scalar],
    point: &[Scalar],
) -> Result<Scalar, Figure9SemanticEngineError> {
    if evaluations.is_empty() || point.len() != 20 {
        return Err(Figure9SemanticEngineError::InvalidInnerSumcheck);
    }
    let padded = evaluations
        .len()
        .checked_next_power_of_two()
        .ok_or(Figure9SemanticEngineError::InvalidShape)?;
    let evaluation_variables = padded.trailing_zeros() as usize;
    let start = point
        .len()
        .checked_sub(
            evaluation_variables
                .checked_add(1)
                .ok_or(Figure9SemanticEngineError::InvalidShape)?,
        )
        .ok_or(Figure9SemanticEngineError::InvalidShape)?;
    let equality =
        eq_evals(&point[start..]).map_err(|_| Figure9SemanticEngineError::InvalidInnerSumcheck)?;
    let partial = evaluations
        .iter()
        .copied()
        .zip(equality)
        .fold(Scalar::zero(), |sum, (value, weight)| sum + value * weight);
    Ok(point[..start]
        .iter()
        .copied()
        .fold(partial, |value, coordinate| {
            value * (Scalar::one() - coordinate)
        }))
}

fn checked_quotient(
    numerator: Scalar,
    denominator: Scalar,
) -> Result<Scalar, Figure9SemanticEngineError> {
    if denominator.is_zero() {
        if numerator.is_zero() {
            Ok(Scalar::zero())
        } else {
            Err(Figure9SemanticEngineError::InvalidInnerSumcheck)
        }
    } else {
        Ok(numerator
            * denominator
                .inverse()
                .map_err(|_| Figure9SemanticEngineError::DivisionByZero)?)
    }
}

fn row_products_step(
    application: &GovernedFigure9ApplicationPrep<'_>,
    shape: &SplitShapeWire,
    step: usize,
    row: usize,
) -> Result<[Scalar; 3], Figure9SemanticEngineError> {
    let value = |column| step_assignment_value(application, shape, step, column);
    Ok([
        matrix_row_dot(&shape.a, row, &value)?,
        matrix_row_dot(&shape.b, row, &value)?,
        matrix_row_dot(&shape.c, row, &value)?,
    ])
}

fn multiply_folded_step_layer(
    application: &GovernedFigure9ApplicationPrep<'_>,
    shape: &SplitShapeWire,
    left: usize,
    right: usize,
    challenge: Scalar,
) -> Result<MatrixLayer, Figure9SemanticEngineError> {
    multiply_layer(shape, |column| {
        let left = step_assignment_value(application, shape, left, column)?;
        let right = step_assignment_value(application, shape, right, column)?;
        Ok(left + challenge * (right - left))
    })
}

fn multiply_core_layer(
    application: &GovernedFigure9ApplicationPrep<'_>,
    shape: &SplitShapeWire,
) -> Result<MatrixLayer, Figure9SemanticEngineError> {
    multiply_layer(shape, |column| {
        core_assignment_value(application, shape, column)
    })
}

fn multiply_layer(
    shape: &SplitShapeWire,
    value: impl Fn(usize) -> Result<Scalar, Figure9SemanticEngineError>,
) -> Result<MatrixLayer, Figure9SemanticEngineError> {
    let mut a = SecretTable::try_zeroed(shape.constraints, shape.constraints)?;
    let mut b = SecretTable::try_zeroed(shape.constraints, shape.constraints)?;
    let mut c = SecretTable::try_zeroed(shape.constraints, shape.constraints)?;
    for row in 0..shape.constraints {
        a.values[row] = matrix_row_dot(&shape.a, row, &value)?;
        b.values[row] = matrix_row_dot(&shape.b, row, &value)?;
        c.values[row] = matrix_row_dot(&shape.c, row, &value)?;
    }
    Ok(MatrixLayer { a, b, c })
}

fn matrix_row_dot(
    matrix: &SparseMatrixWire,
    row: usize,
    value: &impl Fn(usize) -> Result<Scalar, Figure9SemanticEngineError>,
) -> Result<Scalar, Figure9SemanticEngineError> {
    let mut result = Scalar::zero();
    for (column, coefficient) in matrix
        .row_entries(row)
        .ok_or(Figure9SemanticEngineError::InvalidShape)?
    {
        result += coefficient * value(column)?;
    }
    Ok(result)
}

fn step_assignment_value(
    application: &GovernedFigure9ApplicationPrep<'_>,
    shape: &SplitShapeWire,
    step: usize,
    column: usize,
) -> Result<Scalar, Figure9SemanticEngineError> {
    let variables = shape
        .variables()
        .map_err(|_| Figure9SemanticEngineError::InvalidShape)?;
    let instance = application
        .step_instances
        .get(step)
        .ok_or(Figure9SemanticEngineError::InvalidShape)?;
    if column < shape.shared {
        return Ok(application
            .shared_witness
            .get(column)
            .copied()
            .unwrap_or_else(Scalar::zero));
    }
    if column < shape.shared + shape.precommitted {
        let index = column - shape.shared;
        return Ok(instance
            .public_values
            .get(index)
            .copied()
            .unwrap_or_else(Scalar::zero));
    }
    if column < variables {
        let index = column - shape.shared - shape.precommitted;
        return Ok(application.step_private[step]
            .rest_values
            .get(index)
            .copied()
            .unwrap_or_else(Scalar::zero));
    }
    assignment_public_value(
        instance.public_values.as_slice(),
        &instance.challenges,
        variables,
        column,
    )
}

fn core_assignment_value(
    application: &GovernedFigure9ApplicationPrep<'_>,
    shape: &SplitShapeWire,
    column: usize,
) -> Result<Scalar, Figure9SemanticEngineError> {
    let variables = shape
        .variables()
        .map_err(|_| Figure9SemanticEngineError::InvalidShape)?;
    if column < shape.shared {
        return Ok(application
            .shared_witness
            .get(column)
            .copied()
            .unwrap_or_else(Scalar::zero));
    }
    if column < shape.shared + shape.precommitted {
        let index = column - shape.shared;
        return Ok(application
            .core_instance
            .public_values
            .get(index)
            .copied()
            .unwrap_or_else(Scalar::zero));
    }
    if column < variables {
        let index = column - shape.shared - shape.precommitted;
        return Ok(application
            .core_private
            .rest_values
            .get(index)
            .copied()
            .unwrap_or_else(Scalar::zero));
    }
    assignment_public_value(
        application.core_instance.public_values.as_slice(),
        &application.core_instance.challenges,
        variables,
        column,
    )
}

fn assignment_public_value(
    public_values: &[Scalar],
    challenges: &[Scalar],
    variables: usize,
    column: usize,
) -> Result<Scalar, Figure9SemanticEngineError> {
    if column == variables {
        return Ok(Scalar::one());
    }
    let index = column
        .checked_sub(variables + 1)
        .ok_or(Figure9SemanticEngineError::InvalidShape)?;
    if index < public_values.len() {
        Ok(public_values[index])
    } else {
        challenges
            .get(index - public_values.len())
            .copied()
            .ok_or(Figure9SemanticEngineError::InvalidShape)
    }
}

fn nifs_round_witness(
    round: usize,
    polynomials: &[[Scalar; 4]; NIFS_ROUNDS],
    previous_challenges: &[Scalar],
    expected_claim: Scalar,
) -> Result<Vec<Scalar>, Figure9SemanticEngineError> {
    let polynomial = *polynomials
        .get(round)
        .ok_or(Figure9SemanticEngineError::InvalidShape)?;
    let mut witness = Vec::with_capacity(if round == 0 { 5 } else { 7 });
    witness.extend_from_slice(&polynomial);
    if round == 0 {
        witness.push(Scalar::zero());
    } else {
        let challenge = *previous_challenges
            .get(round - 1)
            .ok_or(Figure9SemanticEngineError::InvalidShape)?;
        let (aux, claim) = horner_aux(&polynomials[round - 1], challenge);
        if claim != expected_claim {
            return Err(Figure9SemanticEngineError::InvalidNifs);
        }
        witness.extend_from_slice(&aux);
    }
    if boolean_sum(&polynomial) != expected_claim {
        return Err(Figure9SemanticEngineError::InvalidNifs);
    }
    Ok(witness)
}

fn nifs_final_witness(
    previous: &[Scalar; 4],
    challenge: Scalar,
    t_out: Scalar,
    acc_eq: Scalar,
) -> Result<Vec<Scalar>, Figure9SemanticEngineError> {
    let (aux, claim) = horner_aux(previous, challenge);
    if acc_eq * t_out != claim {
        return Err(Figure9SemanticEngineError::InvalidNifs);
    }
    let mut witness = Vec::with_capacity(5);
    witness.extend_from_slice(&aux);
    witness.push(t_out);
    witness.push(acc_eq);
    Ok(witness)
}

#[allow(clippy::too_many_arguments)]
fn outer_round_witness(
    index: usize,
    polynomial_step: [Scalar; 4],
    polynomial_core: [Scalar; 4],
    previous_step: [Scalar; 4],
    previous_core: [Scalar; 4],
    previous_challenge: Scalar,
    expected_step: Scalar,
    expected_core: Scalar,
) -> Result<Vec<Scalar>, Figure9SemanticEngineError> {
    let mut witness = Vec::with_capacity(if index == 0 { 9 } else { 14 });
    witness.extend_from_slice(&polynomial_step);
    witness.extend_from_slice(&polynomial_core);
    if index == 0 {
        witness.push(Scalar::zero());
    } else {
        let (step_aux, step_claim) = horner_aux(&previous_step, previous_challenge);
        let (core_aux, core_claim) = horner_aux(&previous_core, previous_challenge);
        if step_claim != expected_step || core_claim != expected_core {
            return Err(Figure9SemanticEngineError::InvalidOuterSumcheck);
        }
        witness.extend_from_slice(&step_aux);
        witness.extend_from_slice(&core_aux);
    }
    if boolean_sum(&polynomial_step) != expected_step
        || boolean_sum(&polynomial_core) != expected_core
    {
        return Err(Figure9SemanticEngineError::InvalidOuterSumcheck);
    }
    Ok(witness)
}

fn outer_final_witness(
    previous_step: [Scalar; 4],
    previous_core: [Scalar; 4],
    challenge: Scalar,
    claims_step: [Scalar; 3],
    claims_core: [Scalar; 3],
    tau_at_rx: Scalar,
) -> Result<Vec<Scalar>, Figure9SemanticEngineError> {
    let (step_aux, step_claim) = horner_aux(&previous_step, challenge);
    let (core_aux, core_claim) = horner_aux(&previous_core, challenge);
    if step_claim != tau_at_rx * (claims_step[0] * claims_step[1] - claims_step[2])
        || core_claim != tau_at_rx * (claims_core[0] * claims_core[1] - claims_core[2])
    {
        return Err(Figure9SemanticEngineError::InvalidOuterSumcheck);
    }
    let mut witness = Vec::with_capacity(15);
    witness.extend_from_slice(&step_aux);
    witness.extend_from_slice(&core_aux);
    witness.extend_from_slice(&claims_step);
    witness.extend_from_slice(&claims_core);
    witness.push(tau_at_rx);
    witness.push(claims_step[0] * claims_step[1]);
    witness.push(claims_core[0] * claims_core[1]);
    Ok(witness)
}

fn inner_first_round_witness(
    polynomial_step: [Scalar; 3],
    polynomial_core: [Scalar; 3],
    batching: Scalar,
    claims_step: [Scalar; 3],
    claims_core: [Scalar; 3],
) -> Result<Vec<Scalar>, Figure9SemanticEngineError> {
    let expected_step =
        claims_step[0] + batching * claims_step[1] + batching.square() * claims_step[2];
    let expected_core =
        claims_core[0] + batching * claims_core[1] + batching.square() * claims_core[2];
    if boolean_sum(&polynomial_step) != expected_step
        || boolean_sum(&polynomial_core) != expected_core
    {
        return Err(Figure9SemanticEngineError::InvalidInnerSumcheck);
    }
    let r_squared = batching.square();
    let r_b_step = batching * claims_step[1];
    let joint_step = claims_step[0] + r_b_step + r_squared * claims_step[2];
    let r_b_core = batching * claims_core[1];
    let joint_core = claims_core[0] + r_b_core + r_squared * claims_core[2];
    Ok(vec![
        polynomial_step[0],
        polynomial_step[1],
        polynomial_step[2],
        polynomial_core[0],
        polynomial_core[1],
        polynomial_core[2],
        r_squared,
        r_b_step,
        joint_step,
        r_b_core,
        joint_core,
    ])
}

#[allow(clippy::too_many_arguments)]
fn inner_later_round_witness(
    polynomial_step: [Scalar; 3],
    polynomial_core: [Scalar; 3],
    previous_step: [Scalar; 3],
    previous_core: [Scalar; 3],
    challenge: Scalar,
    expected_step: Scalar,
    expected_core: Scalar,
) -> Result<Vec<Scalar>, Figure9SemanticEngineError> {
    let (step_aux, step_claim) = horner_aux(&previous_step, challenge);
    let (core_aux, core_claim) = horner_aux(&previous_core, challenge);
    if step_claim != expected_step
        || core_claim != expected_core
        || boolean_sum(&polynomial_step) != expected_step
        || boolean_sum(&polynomial_core) != expected_core
    {
        return Err(Figure9SemanticEngineError::InvalidInnerSumcheck);
    }
    let mut witness = Vec::with_capacity(10);
    witness.extend_from_slice(&polynomial_step);
    witness.extend_from_slice(&polynomial_core);
    witness.extend_from_slice(&step_aux);
    witness.extend_from_slice(&core_aux);
    Ok(witness)
}

#[allow(clippy::too_many_arguments)]
fn inner_final_witness(
    previous_step: [Scalar; 3],
    previous_core: [Scalar; 3],
    challenge: Scalar,
    eval_w_step: Scalar,
    eval_w_core: Scalar,
    eval_x_step: Scalar,
    eval_x_core: Scalar,
    r_y_zero: Scalar,
    expected_step: Scalar,
    expected_core: Scalar,
) -> Result<Vec<Scalar>, Figure9SemanticEngineError> {
    let (step_aux, step_claim) = horner_aux(&previous_step, challenge);
    let (core_aux, core_claim) = horner_aux(&previous_core, challenge);
    if step_claim != expected_step || core_claim != expected_core {
        return Err(Figure9SemanticEngineError::InvalidInnerSumcheck);
    }
    let tmp_step = eval_w_step * (Scalar::one() - r_y_zero);
    let sum_step = tmp_step + eval_x_step * r_y_zero;
    let tmp_core = eval_w_core * (Scalar::one() - r_y_zero);
    let sum_core = tmp_core + eval_x_core * r_y_zero;
    let mut witness = Vec::with_capacity(10);
    witness.extend_from_slice(&step_aux);
    witness.extend_from_slice(&core_aux);
    witness.push(eval_w_step);
    witness.push(eval_w_core);
    witness.push(tmp_step);
    witness.push(sum_step);
    witness.push(tmp_core);
    witness.push(sum_core);
    Ok(witness)
}

fn evaluation_commit_round_witness(value: Scalar) -> Vec<Scalar> {
    let mut witness = vec![Scalar::zero(); 32];
    witness[0] = value;
    witness
}

fn horner_aux<const N: usize>(coefficients: &[Scalar; N], point: Scalar) -> (Vec<Scalar>, Scalar) {
    let mut accumulator = coefficients[N - 1];
    let mut auxiliary = Vec::with_capacity(N - 1);
    for coefficient in coefficients[..N - 1].iter().rev() {
        accumulator = accumulator * point + *coefficient;
        auxiliary.push(accumulator);
    }
    (auxiliary, accumulator)
}

fn evaluate_polynomial<const N: usize>(coefficients: &[Scalar; N], point: Scalar) -> Scalar {
    coefficients
        .iter()
        .rev()
        .copied()
        .fold(Scalar::zero(), |value, coefficient| {
            value * point + coefficient
        })
}

fn boolean_sum<const N: usize>(coefficients: &[Scalar; N]) -> Scalar {
    coefficients
        .iter()
        .copied()
        .fold(coefficients[0], |sum, coefficient| sum + coefficient)
}

fn affine(lower: Scalar, upper: Scalar, point: Scalar) -> Scalar {
    lower + point * (upper - lower)
}

fn interpolate_quadratic(
    evaluations: [Scalar; 3],
) -> Result<[Scalar; 3], Figure9SemanticEngineError> {
    let two_inverse = Scalar::from_u64(2)
        .inverse()
        .map_err(|_| Figure9SemanticEngineError::DivisionByZero)?;
    let constant = evaluations[0];
    let quadratic =
        (evaluations[0] - Scalar::from_u64(2) * evaluations[1] + evaluations[2]) * two_inverse;
    Ok([constant, evaluations[1] - constant - quadratic, quadratic])
}

fn interpolate_cubic(evaluations: [Scalar; 4]) -> Result<[Scalar; 4], Figure9SemanticEngineError> {
    let two_inverse = Scalar::from_u64(2)
        .inverse()
        .map_err(|_| Figure9SemanticEngineError::DivisionByZero)?;
    let six_inverse = Scalar::from_u64(6)
        .inverse()
        .map_err(|_| Figure9SemanticEngineError::DivisionByZero)?;
    let constant = evaluations[0];
    let cubic = (evaluations[3] - Scalar::from_u64(3) * evaluations[2]
        + Scalar::from_u64(3) * evaluations[1]
        - evaluations[0])
        * six_inverse;
    let quadratic = (evaluations[2] - Scalar::from_u64(2) * evaluations[1] + evaluations[0])
        * two_inverse
        - Scalar::from_u64(3) * cubic;
    Ok([
        constant,
        evaluations[1] - constant - quadratic - cubic,
        quadratic,
        cubic,
    ])
}

fn one_challenge(challenges: Vec<Scalar>) -> Result<Scalar, Figure9SemanticEngineError> {
    match challenges.as_slice() {
        [challenge] => Ok(*challenge),
        _ => Err(Figure9SemanticEngineError::InvalidShape),
    }
}

fn verify_round_instance_and_equations(
    output: &RoundMachineOutput,
    shape: &MultiRoundShapeWire,
    mut replay_transcript: VegaTranscriptV1,
) -> Result<(), Figure9SemanticEngineError> {
    verify::validate_multi_round_instance(&output.instance, shape, &mut replay_transcript)
        .map_err(|_| Figure9SemanticEngineError::Transcript)?;
    let challenge_count = output
        .instance
        .challenges_per_round
        .iter()
        .try_fold(0_usize, |sum, round| sum.checked_add(round.len()))
        .ok_or(Figure9SemanticEngineError::InvalidShape)?;
    let mut assignment = Vec::with_capacity(
        output.witness.len() + 1 + challenge_count + output.instance.public_values.len(),
    );
    assignment.extend_from_slice(output.witness.as_slice());
    assignment.push(Scalar::one());
    for challenges in &output.instance.challenges_per_round {
        assignment.extend_from_slice(challenges);
    }
    assignment.extend_from_slice(&output.instance.public_values);
    if assignment.len() != shape.a.columns
        || shape.a.columns != shape.b.columns
        || shape.a.columns != shape.c.columns
    {
        return Err(Figure9SemanticEngineError::InvalidShape);
    }
    for row in 0..shape.constraints {
        let a = matrix_row_dot_values(&shape.a, row, &assignment)?;
        let b = matrix_row_dot_values(&shape.b, row, &assignment)?;
        let c = matrix_row_dot_values(&shape.c, row, &assignment)?;
        if a * b != c {
            return Err(Figure9SemanticEngineError::UnsatisfiedVerifierCircuit);
        }
    }
    Ok(())
}

fn matrix_row_dot_values(
    matrix: &SparseMatrixWire,
    row: usize,
    assignment: &[Scalar],
) -> Result<Scalar, Figure9SemanticEngineError> {
    let mut result = Scalar::zero();
    for (column, coefficient) in matrix
        .row_entries(row)
        .ok_or(Figure9SemanticEngineError::InvalidShape)?
    {
        result += coefficient
            * assignment
                .get(column)
                .copied()
                .ok_or(Figure9SemanticEngineError::InvalidShape)?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(value: u64) -> Scalar {
        Scalar::from_u64(value)
    }

    #[test]
    fn exact_figure9_round_geometry_matches_the_pinned_allocation_schedule() {
        let unpadded = expected_round_unpadded();
        assert_eq!(unpadded.len(), VERIFIER_ROUNDS);
        assert_eq!(&unpadded[..4], &[5, 7, 7, 5]);
        assert_eq!(unpadded[4], 9);
        assert!(unpadded[5..22].iter().all(|count| *count == 14));
        assert_eq!(unpadded[22], 15);
        assert_eq!(unpadded[23], 11);
        assert!(unpadded[24..44].iter().all(|count| *count == 10));
        assert_eq!(&unpadded[44..], &[10, 32, 32]);
        assert_eq!(unpadded.iter().sum::<usize>(), 571);
        assert_eq!(VERIFIER_ROUNDS * 32, 1_504);
    }

    #[test]
    fn interpolation_and_horner_replay_have_fixed_coefficient_order() {
        let coefficients = [s(3), s(5), s(7), s(11)];
        let evaluations =
            core::array::from_fn(|index| evaluate_polynomial(&coefficients, s(index as u64)));
        assert_eq!(
            interpolate_cubic(evaluations).expect("nonzero fixed denominators"),
            coefficients
        );
        let (aux, value) = horner_aux(&coefficients, s(13));
        assert_eq!(aux.len(), 3);
        assert_eq!(aux[0], s(11) * s(13) + s(7));
        assert_eq!(value, evaluate_polynomial(&coefficients, s(13)));

        let quadratic = [s(17), s(19), s(23)];
        let evaluations =
            core::array::from_fn(|index| evaluate_polynomial(&quadratic, s(index as u64)));
        assert_eq!(
            interpolate_quadratic(evaluations).expect("nonzero fixed denominator"),
            quadratic
        );
    }

    #[test]
    fn nifs_polynomial_kat_and_mutations_enforce_the_boolean_claim() {
        let rho = s(5);
        let polynomial =
            finish_nifs_polynomial(s(29), s(7), rho, s(11), s(13)).expect("fixed nonzero rho");
        assert_eq!(boolean_sum(&polynomial), s(29));
        let mut polynomials = [[Scalar::zero(); 4]; NIFS_ROUNDS];
        polynomials[0] = polynomial;
        assert_eq!(
            nifs_round_witness(0, &polynomials, &[], s(29))
                .expect("exact first-round assignment")
                .len(),
            5
        );
        polynomials[0][0] += Scalar::one();
        assert_eq!(
            nifs_round_witness(0, &polynomials, &[], s(29)),
            Err(Figure9SemanticEngineError::InvalidNifs)
        );
    }

    #[test]
    fn verifier_round_replay_rejects_polynomial_and_claim_mutations() {
        let prior_step = [s(2), s(3), s(5)];
        let prior_core = [s(7), s(11), s(13)];
        let challenge = s(17);
        let step_claim = evaluate_polynomial(&prior_step, challenge);
        let core_claim = evaluate_polynomial(&prior_core, challenge);
        let next_step = [s(19), step_claim - s(38) - s(23), s(23)];
        let next_core = [s(29), core_claim - s(58) - s(31), s(31)];
        assert_eq!(boolean_sum(&next_step), step_claim);
        assert_eq!(boolean_sum(&next_core), core_claim);
        assert_eq!(
            inner_later_round_witness(
                next_step, next_core, prior_step, prior_core, challenge, step_claim, core_claim,
            )
            .expect("exact semantic round")
            .len(),
            10
        );
        let mut changed = next_step;
        changed[2] += Scalar::one();
        assert_eq!(
            inner_later_round_witness(
                changed, next_core, prior_step, prior_core, challenge, step_claim, core_claim,
            ),
            Err(Figure9SemanticEngineError::InvalidInnerSumcheck)
        );
    }

    #[test]
    fn pinned_vendor_sources_have_the_reviewed_digests() {
        let cases = [
            (
                include_bytes!("../../../../../vendor/vega-prover/src/vega_mc_zkp.rs").as_slice(),
                VEGA_MC_ZKP_SOURCE_SHA256,
            ),
            (
                include_bytes!("../../../../../vendor/vega-prover/src/zk.rs").as_slice(),
                VEGA_VERIFIER_CIRCUIT_SOURCE_SHA256,
            ),
            (
                include_bytes!("../../../../../vendor/vega-prover/src/sumcheck.rs").as_slice(),
                VEGA_SUMCHECK_SOURCE_SHA256,
            ),
            (
                include_bytes!(
                    "../../../../../vendor/vega-prover/reference/pyvega/verifier_circuit.py"
                )
                .as_slice(),
                PYVEGA_VERIFIER_CIRCUIT_SOURCE_SHA256,
            ),
        ];
        for (source, expected) in cases {
            assert_eq!(
                hex::encode(super::super::sha256::sha256(source).expect("bounded source")),
                expected
            );
        }
    }

    #[test]
    fn source_contract_keeps_the_core_dependency_free_through_complete_emission() {
        let source = include_str!("semantic_engine.rs");
        let production = source
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("production semantic engine source");
        assert!(production.contains("prove_application_nifs"));
        assert!(production.contains("prove_outer_sumcheck"));
        assert!(production.contains("prove_inner_sumcheck"));
        assert!(production.contains("verify_round_instance_and_equations"));
        assert!(!production.contains("use bellpepper"));
        assert!(!production.contains("use rayon"));
        assert!(!production.contains("use rand"));

        let boundary = include_str!("../canonical_mc_exact.rs");
        assert!(boundary.contains("Figure9ApplicationPrepError::RandomNova"));
        assert!(boundary.contains("Figure9ApplicationPrepError::FinalOpening"));
        assert!(boundary.contains("verify_figure9_mc(context, public_inputs, &envelope)?"));
    }
}
