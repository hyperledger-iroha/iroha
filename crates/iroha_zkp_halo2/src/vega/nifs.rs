//! Canonical Nova non-interactive folding for Vega's relaxed R1CS instance.

use thiserror::Error;

use super::{
    VegaT256ScalarV1 as Scalar,
    commitment::{Commitment, CommitmentError, CommitmentKey, fold},
    r1cs::{Instance, R1csError, RelaxedInstance, RelaxedWitness, Shape, Witness},
    transcript::{VegaTranscriptError, VegaTranscriptV1},
};

/// Failure while proving or verifying the canonical Vega Nova fold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(super) enum NifsError {
    #[error("Vega Nova NIFS dimensions do not match the R1CS shape")]
    InvalidDimension,
    #[error("Vega Nova NIFS instance commitment does not match its witness")]
    CommitmentMismatch,
    #[error(transparent)]
    R1cs(#[from] R1csError),
    #[error(transparent)]
    Commitment(#[from] CommitmentError),
    #[error(transparent)]
    Transcript(#[from] VegaTranscriptError),
}

/// Nova's one-commitment non-interactive folding proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NovaNifs {
    pub(super) cross_term_commitment: Commitment,
}

/// Complete borrowed input to one Nova NIFS prover invocation.
pub(super) struct NovaNifsProverInput<'a> {
    pub(super) key: &'a CommitmentKey,
    pub(super) shape: &'a Shape,
    pub(super) relaxed_instance: &'a RelaxedInstance,
    pub(super) relaxed_witness: &'a RelaxedWitness,
    pub(super) regular_instance: &'a Instance,
    pub(super) regular_witness: &'a Witness,
    pub(super) cross_term_blindings: &'a [Scalar],
}

impl NovaNifs {
    /// Fold one satisfying relaxed pair with one satisfying regular pair.
    ///
    /// Randomness is supplied explicitly. This keeps the proof primitive
    /// deterministic and makes the caller responsible for sourcing fresh,
    /// cryptographically secure blindings.
    pub(super) fn prove(
        input: NovaNifsProverInput<'_>,
        transcript: &mut VegaTranscriptV1,
    ) -> Result<(Self, RelaxedInstance, RelaxedWitness), NifsError> {
        let NovaNifsProverInput {
            key,
            shape,
            relaxed_instance,
            relaxed_witness,
            regular_instance,
            regular_witness,
            cross_term_blindings,
        } = input;
        validate_prover_inputs(
            key,
            shape,
            relaxed_instance,
            relaxed_witness,
            regular_instance,
            regular_witness,
        )?;
        if cross_term_blindings.len() != commitment_rows(shape.constraint_count(), key.columns())? {
            return Err(NifsError::InvalidDimension);
        }

        transcript.absorb_relaxed_r1cs_instance(
            b"U1",
            &relaxed_instance.witness_commitment,
            &relaxed_instance.error_commitment,
            relaxed_instance.relaxation,
            &relaxed_instance.public_inputs,
        )?;
        transcript.absorb_r1cs_instance(
            b"U2",
            &regular_instance.witness_commitment,
            &regular_instance.public_inputs,
        )?;

        let cross_term = compute_cross_term(
            shape,
            relaxed_instance,
            relaxed_witness,
            regular_instance,
            regular_witness,
        )?;
        let cross_term_commitment = key.commit(&cross_term, cross_term_blindings)?;
        transcript.absorb_commitment(b"comm_T", &cross_term_commitment)?;
        let challenge = transcript.squeeze(b"r")?;

        let folded_instance = fold_public_instances(
            relaxed_instance,
            regular_instance,
            &cross_term_commitment,
            challenge,
        )?;
        let folded_witness = fold_private_witnesses(
            relaxed_witness,
            regular_witness,
            &cross_term,
            cross_term_blindings,
            challenge,
        )?;

        // These checks are redundant algebraically, but keep the native API
        // fail-closed if a future optimization changes either folding path.
        shape.validate_relaxed_assignment(
            &folded_witness.values,
            folded_instance.relaxation,
            &folded_instance.public_inputs,
            &folded_witness.error,
        )?;
        if key.commit(&folded_witness.values, &folded_witness.witness_blindings)?
            != folded_instance.witness_commitment
            || key.commit(&folded_witness.error, &folded_witness.error_blindings)?
                != folded_instance.error_commitment
        {
            return Err(NifsError::CommitmentMismatch);
        }

        Ok((
            Self {
                cross_term_commitment,
            },
            folded_instance,
            folded_witness,
        ))
    }

    /// Replay the exact Fiat--Shamir schedule and fold public instance data.
    pub(super) fn verify(
        &self,
        key: &CommitmentKey,
        shape: &Shape,
        transcript: &mut VegaTranscriptV1,
        relaxed_instance: &RelaxedInstance,
        regular_instance: &Instance,
    ) -> Result<RelaxedInstance, NifsError> {
        validate_public_inputs(
            key,
            shape,
            relaxed_instance,
            regular_instance,
            &self.cross_term_commitment,
        )?;
        transcript.absorb_relaxed_r1cs_instance(
            b"U1",
            &relaxed_instance.witness_commitment,
            &relaxed_instance.error_commitment,
            relaxed_instance.relaxation,
            &relaxed_instance.public_inputs,
        )?;
        transcript.absorb_r1cs_instance(
            b"U2",
            &regular_instance.witness_commitment,
            &regular_instance.public_inputs,
        )?;
        transcript.absorb_commitment(b"comm_T", &self.cross_term_commitment)?;
        let challenge = transcript.squeeze(b"r")?;
        fold_public_instances(
            relaxed_instance,
            regular_instance,
            &self.cross_term_commitment,
            challenge,
        )
    }
}

fn validate_prover_inputs(
    key: &CommitmentKey,
    shape: &Shape,
    relaxed_instance: &RelaxedInstance,
    relaxed_witness: &RelaxedWitness,
    regular_instance: &Instance,
    regular_witness: &Witness,
) -> Result<(), NifsError> {
    validate_public_inputs(
        key,
        shape,
        relaxed_instance,
        regular_instance,
        &relaxed_instance.error_commitment,
    )?;
    let witness_rows = commitment_rows(shape.variable_count(), key.columns())?;
    let error_rows = commitment_rows(shape.constraint_count(), key.columns())?;
    if relaxed_witness.values.len() != shape.variable_count()
        || relaxed_witness.witness_blindings.len() != witness_rows
        || relaxed_witness.error.len() != shape.constraint_count()
        || relaxed_witness.error_blindings.len() != error_rows
        || regular_witness.values.len() != shape.variable_count()
        || regular_witness.blindings.len() != witness_rows
    {
        return Err(NifsError::InvalidDimension);
    }

    shape.validate_relaxed_assignment(
        &relaxed_witness.values,
        relaxed_instance.relaxation,
        &relaxed_instance.public_inputs,
        &relaxed_witness.error,
    )?;
    shape.validate_relaxed_assignment(
        &regular_witness.values,
        Scalar::one(),
        &regular_instance.public_inputs,
        &vec![Scalar::zero(); shape.constraint_count()],
    )?;

    if key.commit(&relaxed_witness.values, &relaxed_witness.witness_blindings)?
        != relaxed_instance.witness_commitment
        || key.commit(&relaxed_witness.error, &relaxed_witness.error_blindings)?
            != relaxed_instance.error_commitment
        || key.commit(&regular_witness.values, &regular_witness.blindings)?
            != regular_instance.witness_commitment
    {
        return Err(NifsError::CommitmentMismatch);
    }
    Ok(())
}

fn validate_public_inputs(
    key: &CommitmentKey,
    shape: &Shape,
    relaxed_instance: &RelaxedInstance,
    regular_instance: &Instance,
    cross_term_commitment: &Commitment,
) -> Result<(), NifsError> {
    let witness_rows = commitment_rows(shape.variable_count(), key.columns())?;
    let error_rows = commitment_rows(shape.constraint_count(), key.columns())?;
    if relaxed_instance.public_inputs.len() != shape.public_input_count()
        || regular_instance.public_inputs.len() != shape.public_input_count()
        || relaxed_instance.witness_commitment.len() != witness_rows
        || regular_instance.witness_commitment.len() != witness_rows
        || relaxed_instance.error_commitment.len() != error_rows
        || cross_term_commitment.len() != error_rows
    {
        return Err(NifsError::InvalidDimension);
    }
    Ok(())
}

fn commitment_rows(length: usize, columns: usize) -> Result<usize, NifsError> {
    if length == 0 || columns == 0 {
        return Err(NifsError::InvalidDimension);
    }
    Ok(length.div_ceil(columns))
}

fn compute_cross_term(
    shape: &Shape,
    relaxed_instance: &RelaxedInstance,
    relaxed_witness: &RelaxedWitness,
    regular_instance: &Instance,
    regular_witness: &Witness,
) -> Result<Vec<Scalar>, NifsError> {
    let mut combined_assignment = Vec::with_capacity(shape.columns());
    combined_assignment.extend(
        relaxed_witness
            .values
            .iter()
            .copied()
            .zip(regular_witness.values.iter().copied())
            .map(|(left, right)| left + right),
    );
    let effective_relaxation = relaxed_instance.relaxation + Scalar::one();
    combined_assignment.push(effective_relaxation);
    combined_assignment.extend(
        relaxed_instance
            .public_inputs
            .iter()
            .copied()
            .zip(regular_instance.public_inputs.iter().copied())
            .map(|(left, right)| left + right),
    );
    if combined_assignment.len() != shape.columns() {
        return Err(NifsError::InvalidDimension);
    }
    let products = shape.multiply(&combined_assignment)?;
    Ok(products
        .a
        .into_iter()
        .zip(products.b)
        .zip(
            products
                .c
                .into_iter()
                .zip(relaxed_witness.error.iter().copied()),
        )
        .map(|((a, b), (c, error))| a * b - effective_relaxation * c - error)
        .collect())
}

fn fold_public_instances(
    relaxed: &RelaxedInstance,
    regular: &Instance,
    cross_term_commitment: &Commitment,
    challenge: Scalar,
) -> Result<RelaxedInstance, NifsError> {
    if relaxed.public_inputs.len() != regular.public_inputs.len() {
        return Err(NifsError::InvalidDimension);
    }
    Ok(RelaxedInstance {
        witness_commitment: fold(
            &[&relaxed.witness_commitment, &regular.witness_commitment],
            &[Scalar::one(), challenge],
        )?,
        error_commitment: fold(
            &[&relaxed.error_commitment, cross_term_commitment],
            &[Scalar::one(), challenge],
        )?,
        public_inputs: relaxed
            .public_inputs
            .iter()
            .copied()
            .zip(regular.public_inputs.iter().copied())
            .map(|(left, right)| left + challenge * right)
            .collect(),
        relaxation: relaxed.relaxation + challenge,
    })
}

fn fold_private_witnesses(
    relaxed: &RelaxedWitness,
    regular: &Witness,
    cross_term: &[Scalar],
    cross_term_blindings: &[Scalar],
    challenge: Scalar,
) -> Result<RelaxedWitness, NifsError> {
    if relaxed.values.len() != regular.values.len()
        || relaxed.witness_blindings.len() != regular.blindings.len()
        || relaxed.error.len() != cross_term.len()
        || relaxed.error_blindings.len() != cross_term_blindings.len()
    {
        return Err(NifsError::InvalidDimension);
    }
    Ok(RelaxedWitness {
        values: relaxed
            .values
            .iter()
            .copied()
            .zip(regular.values.iter().copied())
            .map(|(left, right)| left + challenge * right)
            .collect(),
        witness_blindings: relaxed
            .witness_blindings
            .iter()
            .copied()
            .zip(regular.blindings.iter().copied())
            .map(|(left, right)| left + challenge * right)
            .collect(),
        error: relaxed
            .error
            .iter()
            .copied()
            .zip(cross_term.iter().copied())
            .map(|(left, right)| left + challenge * right)
            .collect(),
        error_blindings: relaxed
            .error_blindings
            .iter()
            .copied()
            .zip(cross_term_blindings.iter().copied())
            .map(|(left, right)| left + challenge * right)
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::r1cs::SparseMatrix;

    fn s(value: u64) -> Scalar {
        Scalar::from_u64(value)
    }

    fn scalar_hex(value: &str) -> Scalar {
        let bytes: [u8; 32] = hex::decode(value)
            .expect("hex")
            .try_into()
            .expect("32-byte scalar");
        Scalar::from_be_bytes_exact(bytes).expect("canonical scalar")
    }

    fn multiplication_shape() -> Shape {
        // z = [x, ONE, y], constraint x * x = y.
        let a = SparseMatrix::new(1, 3, &[(0, 0, s(1))]).expect("canonical A");
        let b = SparseMatrix::new(1, 3, &[(0, 0, s(1))]).expect("canonical B");
        let c = SparseMatrix::new(1, 3, &[(0, 2, s(1))]).expect("canonical C");
        Shape::new(1, 1, 1, a, b, c).expect("valid shape")
    }

    fn fixture() -> (
        CommitmentKey,
        Shape,
        RelaxedInstance,
        RelaxedWitness,
        Instance,
        Witness,
    ) {
        let key = CommitmentKey::derive(b"vega-nifs-test", 1).expect("key");
        let shape = multiplication_shape();
        // Relaxed pair: 2*2 = 5*7 + (-31).
        let relaxed_witness = RelaxedWitness {
            values: vec![s(2)],
            witness_blindings: vec![s(11)],
            error: vec![-s(31)],
            error_blindings: vec![s(13)],
        };
        let relaxed_instance = RelaxedInstance {
            witness_commitment: key
                .commit(&relaxed_witness.values, &relaxed_witness.witness_blindings)
                .expect("commit W1"),
            error_commitment: key
                .commit(&relaxed_witness.error, &relaxed_witness.error_blindings)
                .expect("commit E1"),
            public_inputs: vec![s(7)],
            relaxation: s(5),
        };
        let regular_witness = Witness {
            values: vec![s(3)],
            blindings: vec![s(17)],
        };
        let regular_instance = Instance {
            witness_commitment: key
                .commit(&regular_witness.values, &regular_witness.blindings)
                .expect("commit W2"),
            public_inputs: vec![s(9)],
        };
        (
            key,
            shape,
            relaxed_instance,
            relaxed_witness,
            regular_instance,
            regular_witness,
        )
    }

    #[test]
    fn nova_nifs_roundtrip_produces_one_satisfying_fold() {
        let (key, shape, u1, w1, u2, w2) = fixture();
        let mut prover_transcript = VegaTranscriptV1::new_neutron_nova();
        let (proof, folded_instance, folded_witness) = NovaNifs::prove(
            NovaNifsProverInput {
                key: &key,
                shape: &shape,
                relaxed_instance: &u1,
                relaxed_witness: &w1,
                regular_instance: &u2,
                regular_witness: &w2,
                cross_term_blindings: &[s(19)],
            },
            &mut prover_transcript,
        )
        .expect("valid fold");
        let mut verifier_transcript = VegaTranscriptV1::new_neutron_nova();
        let verifier_instance = proof
            .verify(&key, &shape, &mut verifier_transcript, &u1, &u2)
            .expect("valid verification");
        assert_eq!(
            hex::encode(
                proof.cross_term_commitment.points()[0]
                    .to_non_identity_wire_bytes()
                    .expect("nonidentity")
            ),
            "00e5177c55877bc43d4684d36ce2d63db0a2d30874fadbb1e6a6e68ded6540316d"
        );
        assert_eq!(
            verifier_instance.relaxation - u1.relaxation,
            scalar_hex("4176fcf490e12962562102e528bf065ef0de69bd3407b9bfd4c4431e2ec94f6f")
        );
        assert_eq!(verifier_instance, folded_instance);
        shape
            .validate_relaxed_assignment(
                &folded_witness.values,
                verifier_instance.relaxation,
                &verifier_instance.public_inputs,
                &folded_witness.error,
            )
            .expect("folded assignment satisfies");
        assert_eq!(
            key.commit(&folded_witness.values, &folded_witness.witness_blindings)
                .expect("folded W commitment"),
            verifier_instance.witness_commitment
        );
        assert_eq!(
            key.commit(&folded_witness.error, &folded_witness.error_blindings)
                .expect("folded E commitment"),
            verifier_instance.error_commitment
        );
    }

    #[test]
    fn nova_nifs_rejects_invalid_dimensions_witnesses_and_commitments() {
        let (key, shape, u1, w1, u2, w2) = fixture();
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        assert_eq!(
            NovaNifs::prove(
                NovaNifsProverInput {
                    key: &key,
                    shape: &shape,
                    relaxed_instance: &u1,
                    relaxed_witness: &w1,
                    regular_instance: &u2,
                    regular_witness: &w2,
                    cross_term_blindings: &[],
                },
                &mut transcript
            ),
            Err(NifsError::InvalidDimension)
        );

        let mut bad_u2 = u2.clone();
        bad_u2.public_inputs[0] = s(8);
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        assert!(
            NovaNifs::prove(
                NovaNifsProverInput {
                    key: &key,
                    shape: &shape,
                    relaxed_instance: &u1,
                    relaxed_witness: &w1,
                    regular_instance: &bad_u2,
                    regular_witness: &w2,
                    cross_term_blindings: &[s(19)],
                },
                &mut transcript
            )
            .is_err()
        );

        let mut bad_w1 = w1.clone();
        bad_w1.witness_blindings[0] = s(12);
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        assert_eq!(
            NovaNifs::prove(
                NovaNifsProverInput {
                    key: &key,
                    shape: &shape,
                    relaxed_instance: &u1,
                    relaxed_witness: &bad_w1,
                    regular_instance: &u2,
                    regular_witness: &w2,
                    cross_term_blindings: &[s(19)],
                },
                &mut transcript
            ),
            Err(NifsError::CommitmentMismatch)
        );

        let wider_key = CommitmentKey::derive(b"vega-nifs-wide-test", 1).expect("key");
        let oversized = wider_key
            .commit(&[s(1), s(2)], &[s(3), s(4)])
            .expect("two rows");
        let malformed_proof = NovaNifs {
            cross_term_commitment: oversized,
        };
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        assert_eq!(
            malformed_proof.verify(&key, &shape, &mut transcript, &u1, &u2),
            Err(NifsError::InvalidDimension)
        );
    }

    #[test]
    fn nova_nifs_transcript_and_statement_mutations_cannot_reuse_the_witness() {
        let (key, shape, u1, w1, u2, w2) = fixture();
        let mut prover_transcript = VegaTranscriptV1::new_neutron_nova();
        let (proof, folded_instance, folded_witness) = NovaNifs::prove(
            NovaNifsProverInput {
                key: &key,
                shape: &shape,
                relaxed_instance: &u1,
                relaxed_witness: &w1,
                regular_instance: &u2,
                regular_witness: &w2,
                cross_term_blindings: &[s(19)],
            },
            &mut prover_transcript,
        )
        .expect("valid fold");

        let alternate_commitment = key.commit(&[-s(40)], &[s(20)]).expect("alternate T");
        let altered_proof = NovaNifs {
            cross_term_commitment: alternate_commitment,
        };
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        let altered = altered_proof
            .verify(&key, &shape, &mut transcript, &u1, &u2)
            .expect("well-shaped proof folds");
        assert_ne!(altered, folded_instance);
        assert!(
            shape
                .validate_relaxed_assignment(
                    &folded_witness.values,
                    altered.relaxation,
                    &altered.public_inputs,
                    &folded_witness.error,
                )
                .is_err()
                || key
                    .commit(&folded_witness.error, &folded_witness.error_blindings)
                    .expect("original folded error")
                    != altered.error_commitment
        );

        let mut altered_u2 = u2.clone();
        altered_u2.public_inputs[0] += Scalar::one();
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        let altered_statement = proof
            .verify(&key, &shape, &mut transcript, &u1, &altered_u2)
            .expect("well-shaped statement folds");
        assert_ne!(altered_statement, folded_instance);

        let mut altered_schedule = VegaTranscriptV1::new_neutron_nova();
        altered_schedule
            .domain_separator(b"nifs-adversarial-prefix")
            .expect("bounded");
        let altered_transcript = proof
            .verify(&key, &shape, &mut altered_schedule, &u1, &u2)
            .expect("well-shaped but different transcript");
        assert_ne!(altered_transcript, folded_instance);
    }
}
