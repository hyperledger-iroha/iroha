//! Fresh random verifier-instance mask and exact Nova fold for Figure 9.
//!
//! This stage continues the single proof-scoped ChaCha12 stream and the live
//! post-47-round transcript.  It samples one satisfying relaxed assignment for
//! the governed verifier circuit, commits it, folds the semantic verifier
//! witness into it, and replays the public Nova equation before retaining any
//! state.  Relaxed Spartan and the final application opening remain separate
//! downstream stages.

use super::super::{
    VegaT256ScalarV1 as Scalar,
    commitment::CommitmentKey,
    nifs::{NovaNifs, NovaNifsProverInput},
    r1cs::{
        CoefficientDictionaryCounter, R1csError, RelaxedInstance, RelaxedWitness, Shape,
        SparseMatrix, SparseMatrixRowBuilder, Witness,
    },
    transcript::VegaTranscriptV1,
};
use super::{
    Figure9RandomNovaError,
    prover_key::McProverKeyWire,
    rng::Figure9StdRng,
    semantic_engine::GovernedFigure9SemanticPrep,
    verifier_key::{McVerifierKeyWire, RegularShapeWire, SparseMatrixWire},
    verify,
    wire::{McCommitment, RelaxedInstanceWire},
};

const VERIFIER_VARIABLES: usize = 1_504;
const VERIFIER_CONSTRAINTS: usize = 512;
const VERIFIER_PUBLIC_VALUES: usize = 49;
const VERIFIER_COMMITMENT_WIDTH: usize = 32;
const RANDOM_Z_SCALARS: usize = VERIFIER_VARIABLES + 1 /* relaxation */ + VERIFIER_PUBLIC_VALUES;
const RANDOM_WITNESS_BLINDINGS: usize = 47;
const RANDOM_ERROR_BLINDINGS: usize = 16;
const NOVA_CROSS_TERM_BLINDINGS: usize = 16;
const RANDOM_NOVA_SCALAR_DRAWS: usize = RANDOM_Z_SCALARS
    + RANDOM_WITNESS_BLINDINGS
    + RANDOM_ERROR_BLINDINGS
    + NOVA_CROSS_TERM_BLINDINGS;

#[cfg(test)]
const VEGA_NIFS_SOURCE_SHA256: &str =
    "edf1ae07b510a2b7cc0c5edbc5fdf4655c0d27ceefacaf4a531e63d5bcba11bf";
#[cfg(test)]
const VEGA_R1CS_SOURCE_SHA256: &str =
    "8e70fd58b53c84665c10279d9347fafc6963461f1dc080e7d0ea75a3f0bf3a79";
#[cfg(test)]
const PYVEGA_FINISH_SOURCE_SHA256: &str =
    "5b678f5a058ce4314ce7bb7b046073d64419744cecb7aa8ad68a1e6f369180de";

/// Move-only state at the exact boundary before relaxed Spartan.
///
/// All mask/fold witness vectors, their blindings, the continued RNG, and the
/// earlier application state are erased by their nested owners on success,
/// error, or unwind.  Instances, commitments, and transcript state are public.
pub(super) struct GovernedFigure9RandomNovaPrep<'a> {
    pub(super) semantic: GovernedFigure9SemanticPrep<'a>,
    pub(super) verifier_shape: Shape,
    pub(super) random_instance: RelaxedInstanceWire,
    pub(super) nova_cross_term: McCommitment,
    pub(super) folded_instance: RelaxedInstance,
    pub(super) folded_witness: SecretRelaxedWitness,
    pub(super) transcript: Option<VegaTranscriptV1>,
    pub(super) rng: Option<Figure9StdRng>,
}

/// Heap scalars whose entire live allocation is erased before release.
struct SecretScalars(Vec<Scalar>);

impl SecretScalars {
    fn try_with_capacity(capacity: usize) -> Result<Self, Figure9RandomNovaError> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(capacity)
            .map_err(|_| Figure9RandomNovaError::ResourceExhausted)?;
        Ok(Self(values))
    }

    fn try_from_slice(values: &[Scalar]) -> Result<Self, Figure9RandomNovaError> {
        let mut owned = Self::try_with_capacity(values.len())?;
        owned.0.extend_from_slice(values);
        Ok(owned)
    }

    fn push(&mut self, mut value: Scalar) -> Result<(), Figure9RandomNovaError> {
        if self.0.len() >= self.0.capacity() {
            value.clear_secret();
            return Err(Figure9RandomNovaError::InvalidShape);
        }
        self.0.push(value);
        value.clear_secret();
        Ok(())
    }

    fn as_slice(&self) -> &[Scalar] {
        &self.0
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

/// Zeroizing owner around the existing strict-witness primitive.
struct SecretWitness(Witness);

impl SecretWitness {
    fn as_witness(&self) -> &Witness {
        &self.0
    }
}

impl Drop for SecretWitness {
    fn drop(&mut self) {
        clear_secret_scalars(&mut self.0.values);
        clear_secret_scalars(&mut self.0.blindings);
    }
}

/// Zeroizing owner around the existing relaxed-witness primitive.
pub(super) struct SecretRelaxedWitness(RelaxedWitness);

impl SecretRelaxedWitness {
    fn new(
        values: SecretScalars,
        witness_blindings: SecretScalars,
        error: SecretScalars,
        error_blindings: SecretScalars,
    ) -> Self {
        Self(RelaxedWitness {
            values: values.into_vec(),
            witness_blindings: witness_blindings.into_vec(),
            error: error.into_vec(),
            error_blindings: error_blindings.into_vec(),
        })
    }

    fn from_witness(witness: RelaxedWitness) -> Self {
        Self(witness)
    }

    pub(super) fn as_witness(&self) -> &RelaxedWitness {
        &self.0
    }
}

impl Drop for SecretRelaxedWitness {
    fn drop(&mut self) {
        clear_secret_scalars(&mut self.0.values);
        clear_secret_scalars(&mut self.0.witness_blindings);
        clear_secret_scalars(&mut self.0.error);
        clear_secret_scalars(&mut self.0.error_blindings);
    }
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
    mut semantic: GovernedFigure9SemanticPrep<'a>,
    prover_key: &McProverKeyWire,
    verifier_key: &McVerifierKeyWire,
) -> Result<GovernedFigure9RandomNovaPrep<'a>, Figure9RandomNovaError> {
    validate_geometry(&semantic, prover_key, verifier_key)?;
    let verifier_key_local = verify::derive_and_match_key(&prover_key.verifier_commitment_key)
        .map_err(|_| Figure9RandomNovaError::InvalidKey)?;
    let verifier_shape = shape_from_wire(&prover_key.verifier_regular_shape)?;
    let verifier_instance = verify::multi_round_to_regular(&semantic.verifier_instance)
        .map_err(|_| Figure9RandomNovaError::InvalidShape)?;

    // Validate the complete retained strict pair before the first tail draw.
    let verifier_values = SecretScalars::try_from_slice(semantic.verifier_witness.as_slice())?;
    let verifier_blindings = SecretScalars::try_from_slice(semantic.verifier_blindings.as_slice())?;
    let verifier_witness = SecretWitness(Witness {
        values: verifier_values.into_vec(),
        blindings: verifier_blindings.into_vec(),
    });
    verifier_shape
        .validate_strict_assignment(
            &verifier_witness.as_witness().values,
            &verifier_instance.public_inputs,
        )
        .map_err(|_| Figure9RandomNovaError::UnsatisfiedWitness)?;
    if verifier_key_local
        .commit(
            &verifier_witness.as_witness().values,
            &verifier_witness.as_witness().blindings,
        )
        .map_err(|_| Figure9RandomNovaError::Commitment)?
        != verifier_instance.witness_commitment
    {
        return Err(Figure9RandomNovaError::UnsatisfiedWitness);
    }

    // The semantic stage owns both continuation values.  They are moved only
    // after every deterministic key/shape/witness check has succeeded.
    let mut transcript = semantic
        .transcript
        .take()
        .ok_or(Figure9RandomNovaError::Transcript)?;
    let mut replay_transcript = transcript.clone();
    let mut rng = semantic
        .application
        .rng
        .take()
        .ok_or(Figure9RandomNovaError::Transcript)?;
    let mut draw_count = 0_usize;

    let (random_instance, random_witness) = sample_random_relaxed(
        &verifier_shape,
        &verifier_key_local,
        &mut rng,
        &mut draw_count,
    )?;
    let mut cross_term_blindings = SecretScalars::try_with_capacity(NOVA_CROSS_TERM_BLINDINGS)?;
    for _ in 0..NOVA_CROSS_TERM_BLINDINGS {
        cross_term_blindings.push(draw_scalar(&mut rng, &mut draw_count)?)?;
    }

    let (nifs, folded_instance, folded_witness) = NovaNifs::prove(
        NovaNifsProverInput {
            key: &verifier_key_local,
            shape: &verifier_shape,
            relaxed_instance: &random_instance,
            relaxed_witness: random_witness.as_witness(),
            regular_instance: &verifier_instance,
            regular_witness: verifier_witness.as_witness(),
            cross_term_blindings: cross_term_blindings.as_slice(),
        },
        &mut transcript,
    )
    .map_err(|_| Figure9RandomNovaError::InvalidNova)?;
    let folded_witness = SecretRelaxedWitness::from_witness(folded_witness);
    if draw_count != RANDOM_NOVA_SCALAR_DRAWS {
        return Err(Figure9RandomNovaError::InvalidShape);
    }

    let replayed = nifs
        .verify(
            &verifier_key_local,
            &verifier_shape,
            &mut replay_transcript,
            &random_instance,
            &verifier_instance,
        )
        .map_err(|_| Figure9RandomNovaError::InvalidNova)?;
    if replayed != folded_instance {
        return Err(Figure9RandomNovaError::InvalidNova);
    }

    let random_instance = relaxed_instance_to_wire(&random_instance);
    let nova_cross_term = McCommitment {
        points: nifs.cross_term_commitment.points().to_vec(),
    };
    Ok(GovernedFigure9RandomNovaPrep {
        semantic,
        verifier_shape,
        random_instance,
        nova_cross_term,
        folded_instance,
        folded_witness,
        transcript: Some(transcript),
        rng: Some(rng),
    })
}

fn validate_geometry(
    semantic: &GovernedFigure9SemanticPrep<'_>,
    prover_key: &McProverKeyWire,
    verifier_key: &McVerifierKeyWire,
) -> Result<(), Figure9RandomNovaError> {
    prover_key
        .validate_against(verifier_key)
        .map_err(|_| Figure9RandomNovaError::InvalidKey)?;
    let shape = &prover_key.verifier_regular_shape;
    let width = prover_key.verifier_commitment_key.columns;
    if shape.constraints != VERIFIER_CONSTRAINTS
        || shape.variables != VERIFIER_VARIABLES
        || shape.public_values != VERIFIER_PUBLIC_VALUES
        || width != VERIFIER_COMMITMENT_WIDTH
        || semantic.verifier_witness.len() != VERIFIER_VARIABLES
        || semantic.verifier_blindings.len() != RANDOM_WITNESS_BLINDINGS
        || shape.variables.div_ceil(width) != RANDOM_WITNESS_BLINDINGS
        || shape.constraints.div_ceil(width) != RANDOM_ERROR_BLINDINGS
        || semantic.transcript.is_none()
        || semantic.application.rng.is_none()
    {
        return Err(Figure9RandomNovaError::InvalidShape);
    }
    Ok(())
}

fn shape_from_wire(shape: &RegularShapeWire) -> Result<Shape, Figure9RandomNovaError> {
    let columns = shape
        .variables
        .checked_add(1)
        .and_then(|value| value.checked_add(shape.public_values))
        .ok_or(Figure9RandomNovaError::InvalidShape)?;
    if [(&shape.a), (&shape.b), (&shape.c)]
        .into_iter()
        .any(|matrix| matrix.rows() != shape.constraints || matrix.columns != columns)
    {
        return Err(Figure9RandomNovaError::InvalidShape);
    }
    Shape::new(
        shape.constraints,
        shape.variables,
        shape.public_values,
        sparse_matrix_from_wire(&shape.a)?,
        sparse_matrix_from_wire(&shape.b)?,
        sparse_matrix_from_wire(&shape.c)?,
    )
    .map_err(map_r1cs_error)
}

fn sparse_matrix_from_wire(
    matrix: &SparseMatrixWire,
) -> Result<SparseMatrix, Figure9RandomNovaError> {
    let mut coefficients = CoefficientDictionaryCounter::new();
    for coefficient in matrix.data.iter().copied() {
        coefficients.observe(coefficient).map_err(map_r1cs_error)?;
    }
    let mut builder = SparseMatrixRowBuilder::new(
        matrix.rows(),
        matrix.columns,
        matrix.data.len(),
        coefficients.len(),
    )
    .map_err(map_r1cs_error)?;
    drop(coefficients);
    for row in 0..matrix.rows() {
        let start = *matrix
            .row_offsets
            .get(row)
            .ok_or(Figure9RandomNovaError::InvalidShape)?;
        let end = *matrix
            .row_offsets
            .get(row + 1)
            .ok_or(Figure9RandomNovaError::InvalidShape)?;
        if start > end || end > matrix.data.len() || end > matrix.indices.len() {
            return Err(Figure9RandomNovaError::InvalidShape);
        }
        builder
            .append_canonical_row(
                (start..end).map(|index| (matrix.indices[index], matrix.data[index])),
            )
            .map_err(map_r1cs_error)?;
    }
    builder.finish().map_err(map_r1cs_error)
}

fn map_r1cs_error(error: R1csError) -> Figure9RandomNovaError {
    match error {
        R1csError::CsrStorageAllocation => Figure9RandomNovaError::ResourceExhausted,
        _ => Figure9RandomNovaError::InvalidShape,
    }
}

fn sample_random_relaxed(
    shape: &Shape,
    key: &CommitmentKey,
    rng: &mut Figure9StdRng,
    draw_count: &mut usize,
) -> Result<(RelaxedInstance, SecretRelaxedWitness), Figure9RandomNovaError> {
    let mut values = SecretScalars::try_with_capacity(shape.variable_count())?;
    for _ in 0..shape.variable_count() {
        values.push(draw_scalar(rng, draw_count)?)?;
    }
    let relaxation = draw_scalar(rng, draw_count)?;
    let mut public_inputs = Vec::new();
    public_inputs
        .try_reserve_exact(shape.public_input_count())
        .map_err(|_| Figure9RandomNovaError::ResourceExhausted)?;
    for _ in 0..shape.public_input_count() {
        public_inputs.push(draw_scalar(rng, draw_count)?);
    }

    let mut error = SecretScalars::try_with_capacity(shape.constraint_count())?;
    for row in 0..shape.constraint_count() {
        let a = assignment_row(
            &shape.a,
            row,
            values.as_slice(),
            relaxation,
            &public_inputs,
            shape.variable_count(),
        )?;
        let b = assignment_row(
            &shape.b,
            row,
            values.as_slice(),
            relaxation,
            &public_inputs,
            shape.variable_count(),
        )?;
        let c = assignment_row(
            &shape.c,
            row,
            values.as_slice(),
            relaxation,
            &public_inputs,
            shape.variable_count(),
        )?;
        error.push(a * b - relaxation * c)?;
    }

    let witness_rows = shape.variable_count().div_ceil(key.columns());
    let error_rows = shape.constraint_count().div_ceil(key.columns());
    let mut witness_blindings = SecretScalars::try_with_capacity(witness_rows)?;
    for _ in 0..witness_rows {
        witness_blindings.push(draw_scalar(rng, draw_count)?)?;
    }
    let mut error_blindings = SecretScalars::try_with_capacity(error_rows)?;
    for _ in 0..error_rows {
        error_blindings.push(draw_scalar(rng, draw_count)?)?;
    }
    let witness_commitment = key
        .commit(values.as_slice(), witness_blindings.as_slice())
        .map_err(|_| Figure9RandomNovaError::Commitment)?;
    let error_commitment = key
        .commit(error.as_slice(), error_blindings.as_slice())
        .map_err(|_| Figure9RandomNovaError::Commitment)?;
    let instance = RelaxedInstance {
        witness_commitment,
        error_commitment,
        public_inputs,
        relaxation,
    };
    let witness = SecretRelaxedWitness::new(values, witness_blindings, error, error_blindings);
    shape
        .validate_relaxed_assignment(
            &witness.as_witness().values,
            instance.relaxation,
            &instance.public_inputs,
            &witness.as_witness().error,
        )
        .map_err(|_| Figure9RandomNovaError::UnsatisfiedWitness)?;
    Ok((instance, witness))
}

fn assignment_row(
    matrix: &SparseMatrix,
    row: usize,
    witness: &[Scalar],
    relaxation: Scalar,
    public_inputs: &[Scalar],
    variables: usize,
) -> Result<Scalar, Figure9RandomNovaError> {
    matrix
        .row_entries(row)
        .ok_or(Figure9RandomNovaError::InvalidShape)?
        .try_fold(Scalar::zero(), |sum, (column, coefficient)| {
            let value = if column < variables {
                witness
                    .get(column)
                    .copied()
                    .ok_or(Figure9RandomNovaError::InvalidShape)?
            } else if column == variables {
                relaxation
            } else {
                public_inputs
                    .get(column - variables - 1)
                    .copied()
                    .ok_or(Figure9RandomNovaError::InvalidShape)?
            };
            Ok(sum + coefficient * value)
        })
}

fn draw_scalar(
    rng: &mut Figure9StdRng,
    draw_count: &mut usize,
) -> Result<Scalar, Figure9RandomNovaError> {
    *draw_count = draw_count
        .checked_add(1)
        .ok_or(Figure9RandomNovaError::InvalidShape)?;
    Ok(rng.scalar())
}

fn relaxed_instance_to_wire(instance: &RelaxedInstance) -> RelaxedInstanceWire {
    RelaxedInstanceWire {
        witness_commitment: McCommitment {
            points: instance.witness_commitment.points().to_vec(),
        },
        error_commitment: McCommitment {
            points: instance.error_commitment.points().to_vec(),
        },
        public_values: instance.public_inputs.clone(),
        relaxation: instance.relaxation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::{nifs::NovaNifs, r1cs::Instance, spartan::RelaxedSpartanProof};

    fn s(value: u64) -> Scalar {
        Scalar::from_u64(value)
    }

    fn non_power_shape() -> Shape {
        // z = [a,b,c,u,x].  a*b=x and c*u=c.  The strict witness has three
        // variables but Spartan must use a four-variable padded domain.
        let a = SparseMatrix::new(2, 5, &[(0, 0, s(1)), (1, 2, s(1))]).expect("canonical A");
        let b = SparseMatrix::new(2, 5, &[(0, 1, s(1)), (1, 3, s(1))]).expect("canonical B");
        let c = SparseMatrix::new(2, 5, &[(0, 4, s(1)), (1, 2, s(1))]).expect("canonical C");
        Shape::new(2, 3, 1, a, b, c).expect("non-power witness shape")
    }

    #[test]
    fn non_power_random_nova_and_spartan_roundtrip_and_mutations_fail() {
        let shape = non_power_shape();
        let key = CommitmentKey::derive(b"figure9-random-nova-test", 2).expect("key");
        let regular_witness = SecretWitness(Witness {
            values: vec![s(2), s(3), s(5)],
            blindings: vec![s(7), s(11)],
        });
        let regular_instance = Instance {
            witness_commitment: key
                .commit(
                    &regular_witness.as_witness().values,
                    &regular_witness.as_witness().blindings,
                )
                .expect("strict commitment"),
            public_inputs: vec![s(6)],
        };
        let mut rng = Figure9StdRng::from_seed([0x42; 32]);
        let mut draws = 0;
        let (random_instance, random_witness) =
            sample_random_relaxed(&shape, &key, &mut rng, &mut draws).expect("random mask");
        // Z=(3 W, u, 1 X), then ceil(3/2)=2 W blinds and one E blind.
        assert_eq!(draws, 8);
        let mut cross_blindings = SecretScalars::try_with_capacity(1).expect("one row");
        cross_blindings
            .push(draw_scalar(&mut rng, &mut draws).expect("draw"))
            .expect("capacity");
        assert_eq!(draws, 9);

        let mut prover_transcript = VegaTranscriptV1::new_neutron_nova();
        let (nifs, folded_instance, folded_witness) = NovaNifs::prove(
            NovaNifsProverInput {
                key: &key,
                shape: &shape,
                relaxed_instance: &random_instance,
                relaxed_witness: random_witness.as_witness(),
                regular_instance: &regular_instance,
                regular_witness: regular_witness.as_witness(),
                cross_term_blindings: cross_blindings.as_slice(),
            },
            &mut prover_transcript,
        )
        .expect("Nova fold");
        let folded_witness = SecretRelaxedWitness::from_witness(folded_witness);
        let proof = RelaxedSpartanProof::prove(
            &shape,
            &key,
            &folded_instance,
            folded_witness.as_witness(),
            &mut prover_transcript,
        )
        .expect("non-power Spartan");
        assert_eq!(proof.inner_sumcheck.rounds.len(), 3);
        assert_eq!(proof.witness_opening.len(), 2);

        let mut verifier_transcript = VegaTranscriptV1::new_neutron_nova();
        let replayed = nifs
            .verify(
                &key,
                &shape,
                &mut verifier_transcript,
                &random_instance,
                &regular_instance,
            )
            .expect("Nova replay");
        assert_eq!(replayed, folded_instance);
        proof
            .verify(&shape, &key, &replayed, &mut verifier_transcript)
            .expect("Spartan replay");

        let mut changed = proof.clone();
        changed.outer_claims[0] += Scalar::one();
        let mut verifier_transcript = VegaTranscriptV1::new_neutron_nova();
        let replayed = nifs
            .verify(
                &key,
                &shape,
                &mut verifier_transcript,
                &random_instance,
                &regular_instance,
            )
            .expect("Nova replay");
        assert!(
            changed
                .verify(&shape, &key, &replayed, &mut verifier_transcript)
                .is_err()
        );
    }

    #[test]
    fn governed_tail_geometry_and_source_pins_are_exact() {
        assert_eq!(RANDOM_Z_SCALARS, 1_554);
        assert_eq!(RANDOM_NOVA_SCALAR_DRAWS, 1_633);
        assert_eq!(VERIFIER_VARIABLES.div_ceil(VERIFIER_COMMITMENT_WIDTH), 47);
        assert_eq!(VERIFIER_CONSTRAINTS.div_ceil(VERIFIER_COMMITMENT_WIDTH), 16);
        let cases = [
            (
                include_bytes!("../../../../../vendor/vega-prover/src/nifs.rs").as_slice(),
                VEGA_NIFS_SOURCE_SHA256,
            ),
            (
                include_bytes!("../../../../../vendor/vega-prover/src/r1cs/mod.rs").as_slice(),
                VEGA_R1CS_SOURCE_SHA256,
            ),
            (
                include_bytes!(
                    "../../../../../vendor/vega-prover/reference/pyvega/prover_finish.py"
                )
                .as_slice(),
                PYVEGA_FINISH_SOURCE_SHA256,
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
    fn source_contract_continues_one_rng_into_the_complete_governed_pipeline() {
        let source = include_str!("random_nova.rs");
        let production = source
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("production source");
        assert!(production.contains("let mut rng = semantic"));
        assert!(production.contains("let mut transcript = semantic"));
        assert!(production.contains("NovaNifs::prove"));
        assert!(production.contains("let replayed = nifs"));
        assert!(production.contains(".verify("));
        assert!(!production.contains("use rand"));
        assert!(!production.contains("use rayon"));

        let boundary = include_str!("../canonical_mc_exact.rs");
        assert!(boundary.contains("let raw_proof ="));
        assert!(boundary.contains("verify_figure9_mc(context, public_inputs, &envelope)?"));
        assert!(boundary.contains("Ok(envelope)"));
    }
}
