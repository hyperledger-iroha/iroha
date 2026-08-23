//! Exact relaxed-Spartan tail for the governed Figure 9 verifier circuit.
//!
//! This stage consumes the satisfying Nova-folded verifier witness, continues
//! the same Fiat--Shamir transcript through both sum-checks and the two direct
//! Hyrax openings, converts the result into the canonical Microsoft wire
//! representation, and replays that wire through the compatibility verifier.
//! Spartan samples no randomness, so the proof-scoped ChaCha12 owner is carried
//! through unchanged for the final application opening.

use super::super::{
    commitment::CommitmentKey, spartan::RelaxedSpartanProof, sumcheck::SumcheckProof,
};
use super::{
    Figure9RelaxedSpartanError,
    prover_key::McProverKeyWire,
    random_nova::GovernedFigure9RandomNovaPrep,
    rng::Figure9StdRng,
    semantic_engine::GovernedFigure9SemanticPrep,
    verifier_key::McVerifierKeyWire,
    verify,
    wire::{
        CompressedPolynomialWire, McCommitment, RelaxedInstanceWire, RelaxedSpartanWire,
        SumcheckWire,
    },
};

const VERIFIER_VARIABLES: usize = 1_504;
const VERIFIER_CONSTRAINTS: usize = 512;
const VERIFIER_PUBLIC_VALUES: usize = 49;
const VERIFIER_COMMITMENT_WIDTH: usize = 32;
const OUTER_ROUNDS: usize = 9;
const OUTER_COEFFICIENTS: usize = 3;
const INNER_ROUNDS: usize = 12;
const INNER_COEFFICIENTS: usize = 2;

#[cfg(test)]
const VEGA_RELAXED_SPARTAN_SOURCE_SHA256: &str =
    "047e8aa047a548c6c51a7ff40c283aff252093785f8ee153caf3aed09da4ae8d";

/// Move-only state at the boundary before the final application opening.
///
/// The large application witness, all retained application blindings, the
/// continued RNG, and its semantic state remain under their earlier RAII
/// owners. The random verifier instance, Nova commitment, and Spartan proof
/// are public proof material and are intentionally not erased.
pub(super) struct GovernedFigure9RelaxedSpartanPrep<'a> {
    pub(super) semantic: GovernedFigure9SemanticPrep<'a>,
    pub(super) random_instance: RelaxedInstanceWire,
    pub(super) nova_cross_term: McCommitment,
    pub(super) relaxed_spartan: RelaxedSpartanWire,
    pub(super) transcript: Option<super::super::transcript::VegaTranscriptV1>,
    pub(super) rng: Option<Figure9StdRng>,
}

pub(super) fn build<'a>(
    mut random_nova: GovernedFigure9RandomNovaPrep<'a>,
    prover_key: &McProverKeyWire,
    verifier_key: &McVerifierKeyWire,
) -> Result<GovernedFigure9RelaxedSpartanPrep<'a>, Figure9RelaxedSpartanError> {
    let verifier_commitment_key = validate_geometry(&random_nova, prover_key, verifier_key)?;

    // Clone only the small public transcript state. The witness and its four
    // secret vectors remain borrowed from their move-only zeroizing owner.
    let mut replay_transcript = random_nova
        .transcript
        .as_ref()
        .ok_or(Figure9RelaxedSpartanError::MissingState)?
        .clone();
    let proof = RelaxedSpartanProof::prove(
        &random_nova.verifier_shape,
        &verifier_commitment_key,
        &random_nova.folded_instance,
        random_nova.folded_witness.as_witness(),
        random_nova
            .transcript
            .as_mut()
            .ok_or(Figure9RelaxedSpartanError::MissingState)?,
    )
    .map_err(|_| Figure9RelaxedSpartanError::Proof)?;
    let relaxed_spartan = spartan_to_wire(proof);

    // Exercise the same wire-level helper reached by `McProofWire` verification
    // before retaining the proof or releasing the folded verifier witness.
    verify::verify_relaxed_spartan(
        &relaxed_spartan,
        &prover_key.verifier_regular_shape,
        &verifier_commitment_key,
        &random_nova.folded_instance,
        &mut replay_transcript,
    )
    .map_err(|_| Figure9RelaxedSpartanError::SelfVerification)?;

    let GovernedFigure9RandomNovaPrep {
        semantic,
        verifier_shape: _,
        random_instance,
        nova_cross_term,
        folded_instance: _,
        folded_witness: _,
        transcript,
        rng,
    } = random_nova;
    Ok(GovernedFigure9RelaxedSpartanPrep {
        semantic,
        random_instance,
        nova_cross_term,
        relaxed_spartan,
        transcript,
        rng,
    })
}

fn validate_geometry(
    random_nova: &GovernedFigure9RandomNovaPrep<'_>,
    prover_key: &McProverKeyWire,
    verifier_key: &McVerifierKeyWire,
) -> Result<CommitmentKey, Figure9RelaxedSpartanError> {
    prover_key
        .validate_against(verifier_key)
        .map_err(|_| Figure9RelaxedSpartanError::InvalidKey)?;
    let key = verify::derive_and_match_key(&prover_key.verifier_commitment_key)
        .map_err(|_| Figure9RelaxedSpartanError::InvalidKey)?;
    let dimensions = verifier_key
        .proof_dimensions()
        .map_err(|_| Figure9RelaxedSpartanError::InvalidKey)?;
    let witness = random_nova.folded_witness.as_witness();
    if random_nova.verifier_shape.constraint_count() != VERIFIER_CONSTRAINTS
        || random_nova.verifier_shape.variable_count() != VERIFIER_VARIABLES
        || random_nova.verifier_shape.public_input_count() != VERIFIER_PUBLIC_VALUES
        || key.columns() != VERIFIER_COMMITMENT_WIDTH
        || dimensions.verifier_constraints != VERIFIER_CONSTRAINTS
        || dimensions.verifier_variables != VERIFIER_VARIABLES
        || dimensions.random_public_values != VERIFIER_PUBLIC_VALUES
        || dimensions.relaxed_outer_rounds != OUTER_ROUNDS
        || dimensions.relaxed_outer_coefficients != OUTER_COEFFICIENTS
        || dimensions.relaxed_inner_rounds != INNER_ROUNDS
        || dimensions.relaxed_inner_coefficients != INNER_COEFFICIENTS
        || dimensions.relaxed_opening_scalars != VERIFIER_COMMITMENT_WIDTH
        || random_nova.folded_instance.public_inputs.len() != VERIFIER_PUBLIC_VALUES
        || random_nova.folded_instance.witness_commitment.len()
            != VERIFIER_VARIABLES.div_ceil(VERIFIER_COMMITMENT_WIDTH)
        || random_nova.folded_instance.error_commitment.len()
            != VERIFIER_CONSTRAINTS.div_ceil(VERIFIER_COMMITMENT_WIDTH)
        || witness.values.len() != VERIFIER_VARIABLES
        || witness.witness_blindings.len() != VERIFIER_VARIABLES.div_ceil(VERIFIER_COMMITMENT_WIDTH)
        || witness.error.len() != VERIFIER_CONSTRAINTS
        || witness.error_blindings.len() != VERIFIER_CONSTRAINTS.div_ceil(VERIFIER_COMMITMENT_WIDTH)
        || random_nova.random_instance.public_values.len() != VERIFIER_PUBLIC_VALUES
        || random_nova.random_instance.witness_commitment.points.len()
            != VERIFIER_VARIABLES.div_ceil(VERIFIER_COMMITMENT_WIDTH)
        || random_nova.random_instance.error_commitment.points.len()
            != VERIFIER_CONSTRAINTS.div_ceil(VERIFIER_COMMITMENT_WIDTH)
        || random_nova.nova_cross_term.points.len()
            != VERIFIER_CONSTRAINTS.div_ceil(VERIFIER_COMMITMENT_WIDTH)
        || random_nova.transcript.is_none()
        || random_nova.rng.is_none()
    {
        return Err(Figure9RelaxedSpartanError::InvalidShape);
    }
    Ok(key)
}

fn spartan_to_wire(proof: RelaxedSpartanProof) -> RelaxedSpartanWire {
    let RelaxedSpartanProof {
        outer_sumcheck,
        outer_claims,
        inner_sumcheck,
        witness_opening,
        witness_opening_blinding,
        error_opening,
        error_opening_blinding,
    } = proof;
    RelaxedSpartanWire {
        outer_sumcheck: sumcheck_to_wire(outer_sumcheck),
        outer_claims,
        inner_sumcheck: sumcheck_to_wire(inner_sumcheck),
        witness_opening,
        witness_blinding: witness_opening_blinding,
        error_opening,
        error_blinding: error_opening_blinding,
    }
}

fn sumcheck_to_wire(proof: SumcheckProof) -> SumcheckWire {
    SumcheckWire {
        rounds: proof
            .rounds
            .into_iter()
            .map(|round| CompressedPolynomialWire {
                coefficients_except_linear: round.coefficients_except_linear,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::{
        VegaT256ScalarV1 as Scalar,
        nifs::{NovaNifs, NovaNifsProverInput},
        r1cs::{Instance, RelaxedInstance, RelaxedWitness, Shape, SparseMatrix, Witness},
        transcript::VegaTranscriptV1,
    };

    use super::super::verifier_key::{RegularShapeWire, SparseMatrixWire};

    fn s(value: u64) -> Scalar {
        Scalar::from_u64(value)
    }

    fn h(value: &str) -> Scalar {
        let bytes: [u8; 32] = hex::decode(value)
            .expect("hex")
            .try_into()
            .expect("32-byte scalar");
        Scalar::from_be_bytes_exact(bytes).expect("canonical scalar")
    }

    fn matrix_wire() -> SparseMatrixWire {
        SparseMatrixWire {
            data: vec![Scalar::one(); 4],
            indices: vec![0, 1, 2, 3],
            row_offsets: vec![0, 1, 2, 3, 4],
            columns: 5,
        }
    }

    fn shape_pair() -> (Shape, RegularShapeWire) {
        let entries = (0..4).map(|index| (index, index, s(1))).collect::<Vec<_>>();
        let matrix = SparseMatrix::new(4, 5, &entries).expect("canonical matrix");
        let shape = Shape::new(
            4,
            4,
            0,
            matrix,
            SparseMatrix::new(4, 5, &entries).expect("canonical matrix"),
            SparseMatrix::new(4, 5, &entries).expect("canonical matrix"),
        )
        .expect("shape");
        let wire = RegularShapeWire {
            constraints: 4,
            variables: 4,
            public_values: 0,
            a: matrix_wire(),
            b: matrix_wire(),
            c: matrix_wire(),
        };
        (shape, wire)
    }

    fn composed_fixture() -> (
        CommitmentKey,
        Shape,
        RegularShapeWire,
        RelaxedInstance,
        Instance,
        NovaNifs,
        RelaxedInstance,
        RelaxedSpartanWire,
    ) {
        let (shape, wire_shape) = shape_pair();
        let key = CommitmentKey::derive(b"vega-spartan-test", 2).expect("key");
        let relaxed_witness = RelaxedWitness {
            values: vec![s(2), s(3), s(4), s(5)],
            witness_blindings: vec![s(11), s(13)],
            error: vec![-s(10), -s(12), -s(12), -s(10)],
            error_blindings: vec![s(17), s(19)],
        };
        let relaxed_instance = RelaxedInstance {
            witness_commitment: key
                .commit(&relaxed_witness.values, &relaxed_witness.witness_blindings)
                .expect("W1"),
            error_commitment: key
                .commit(&relaxed_witness.error, &relaxed_witness.error_blindings)
                .expect("E1"),
            public_inputs: vec![],
            relaxation: s(7),
        };
        let regular_witness = Witness {
            values: vec![s(1), s(0), s(1), s(1)],
            blindings: vec![s(23), s(29)],
        };
        let regular_instance = Instance {
            witness_commitment: key
                .commit(&regular_witness.values, &regular_witness.blindings)
                .expect("W2"),
            public_inputs: vec![],
        };
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        let (nifs, folded_instance, folded_witness) = NovaNifs::prove(
            NovaNifsProverInput {
                key: &key,
                shape: &shape,
                relaxed_instance: &relaxed_instance,
                relaxed_witness: &relaxed_witness,
                regular_instance: &regular_instance,
                regular_witness: &regular_witness,
                cross_term_blindings: &[s(31), s(37)],
            },
            &mut transcript,
        )
        .expect("NIFS");
        let proof = RelaxedSpartanProof::prove(
            &shape,
            &key,
            &folded_instance,
            &folded_witness,
            &mut transcript,
        )
        .expect("Spartan");
        (
            key,
            shape,
            wire_shape,
            relaxed_instance,
            regular_instance,
            nifs,
            folded_instance,
            spartan_to_wire(proof),
        )
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the test verifier keeps each Nova/Spartan transcript artifact explicit"
    )]
    fn verify_wire(
        key: &CommitmentKey,
        shape: &Shape,
        wire_shape: &RegularShapeWire,
        relaxed: &RelaxedInstance,
        regular: &Instance,
        nifs: &NovaNifs,
        folded: &RelaxedInstance,
        proof: &RelaxedSpartanWire,
    ) -> Result<(), super::super::wire::McCodecError> {
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        let replayed = nifs
            .verify(key, shape, &mut transcript, relaxed, regular)
            .map_err(|_| super::super::wire::McCodecError::InvalidEncoding)?;
        if &replayed != folded {
            return Err(super::super::wire::McCodecError::InvalidEncoding);
        }
        verify::verify_relaxed_spartan(proof, wire_shape, key, folded, &mut transcript)
    }

    #[test]
    fn canonical_wire_conversion_matches_the_independent_spartan_kat() {
        let (key, shape, wire_shape, relaxed, regular, nifs, folded, proof) = composed_fixture();
        verify_wire(
            &key,
            &shape,
            &wire_shape,
            &relaxed,
            &regular,
            &nifs,
            &folded,
            &proof,
        )
        .expect("wire-level compatibility replay");
        assert_eq!(proof.outer_sumcheck.rounds.len(), 2);
        assert_eq!(proof.inner_sumcheck.rounds.len(), 3);
        assert_eq!(
            proof.outer_claims[0],
            h("0a96ea0672892cb3047980c13760c1dbc6912862049b69014af2bec1be637723")
        );
        assert_eq!(
            proof.witness_opening[0],
            h("2fa4819bc9964f0e63b3c04f5d9d6c58b2bdda599344323cc29b4e33f5d42852")
        );
        assert_eq!(
            proof.error_blinding,
            h("7c4175b3a65853572a1520d371e8794bd8ff5a867a934a656d3246d7e302beea")
        );
    }

    #[test]
    fn every_wire_response_category_is_rejected_after_mutation() {
        let (key, shape, wire_shape, relaxed, regular, nifs, folded, proof) = composed_fixture();
        let mutations: [fn(&mut RelaxedSpartanWire); 7] = [
            |value| value.outer_sumcheck.rounds[0].coefficients_except_linear[0] += Scalar::one(),
            |value| value.outer_claims[0] += Scalar::one(),
            |value| value.inner_sumcheck.rounds[0].coefficients_except_linear[0] += Scalar::one(),
            |value| value.witness_opening[0] += Scalar::one(),
            |value| value.witness_blinding += Scalar::one(),
            |value| value.error_opening[0] += Scalar::one(),
            |value| value.error_blinding += Scalar::one(),
        ];
        for mutate in mutations {
            let mut changed = proof.clone();
            mutate(&mut changed);
            assert!(
                verify_wire(
                    &key,
                    &shape,
                    &wire_shape,
                    &relaxed,
                    &regular,
                    &nifs,
                    &folded,
                    &changed,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn governed_spartan_geometry_source_and_complete_boundary_are_pinned() {
        assert_eq!(VERIFIER_CONSTRAINTS.ilog2(), OUTER_ROUNDS as u32);
        assert_eq!(
            VERIFIER_VARIABLES.next_power_of_two().ilog2() + 1,
            INNER_ROUNDS as u32
        );
        assert_eq!(OUTER_COEFFICIENTS, 3);
        assert_eq!(INNER_COEFFICIENTS, 2);
        let source = include_bytes!("../../../../../vendor/vega-prover/src/spartan_relaxed.rs");
        assert_eq!(
            hex::encode(super::super::sha256::sha256(source).expect("bounded source")),
            VEGA_RELAXED_SPARTAN_SOURCE_SHA256
        );

        let implementation = include_str!("relaxed_spartan.rs");
        let production = implementation
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("production source");
        assert!(production.contains("RelaxedSpartanProof::prove"));
        assert!(production.contains("verify::verify_relaxed_spartan"));
        assert!(!production.contains(".scalar()"));
        assert!(!production.contains("use rand"));
        assert!(!production.contains("use rayon"));

        let boundary = include_str!("../canonical_mc_exact.rs");
        assert!(boundary.contains("Figure9ApplicationPrepError::FinalOpening"));
        assert!(boundary.contains("verify_figure9_mc(context, public_inputs, &envelope)?"));
        assert!(boundary.contains("Ok(envelope)"));
    }
}
