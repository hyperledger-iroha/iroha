//! Non-zero-knowledge Relaxed Spartan for the Nova-folded Vega instance.
//!
//! This proof deliberately absorbs only the folded scalar `u` and public
//! inputs. It is sound only inside the canonical composition where Nova NIFS
//! has already bound both input instances and the cross-term commitment to the
//! same transcript. The type and entry points remain crate-private.
use super::{
    VegaT256ScalarV1 as Scalar,
    algebra::{
        AlgebraError, eq_evals, eq_evaluate, evaluation_table_size, inner_product, log2_exact,
    },
    commitment::{CommitmentError, CommitmentKey},
    hyrax::{HyraxError, prove_direct, verify_direct},
    r1cs::{R1csError, RelaxedInstance, RelaxedWitness, Shape},
    sumcheck::{
        SecretScalarTable, SplitSecretScalarTable, SumcheckError, SumcheckProof,
        prove_cubic_with_split_first_owned, prove_quadratic_owned,
    },
    transcript::{VegaTranscriptError, VegaTranscriptV1},
};
use thiserror::Error;
/// Failure while proving or verifying the fixed-shape Relaxed Spartan proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(super) enum SpartanError {
    #[error("Vega Relaxed Spartan dimensions do not match the fixed R1CS profile")]
    InvalidDimension,
    #[error("Vega Relaxed Spartan witness commitment does not match the folded instance")]
    CommitmentMismatch,
    #[error("Vega Relaxed Spartan outer claim equation failed")]
    InvalidOuterClaim,
    #[error("Vega Relaxed Spartan inner claim equation failed")]
    InvalidInnerClaim,
    #[error(transparent)]
    Algebra(#[from] AlgebraError),
    #[error(transparent)]
    Commitment(#[from] CommitmentError),
    #[error(transparent)]
    Hyrax(#[from] HyraxError),
    #[error(transparent)]
    R1cs(#[from] R1csError),
    #[error(transparent)]
    Sumcheck(#[from] SumcheckError),
    #[error(transparent)]
    Transcript(#[from] VegaTranscriptError),
}
/// Canonical non-ZK Spartan proof for one relaxed R1CS instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RelaxedSpartanProof {
    pub(super) outer_sumcheck: SumcheckProof,
    pub(super) outer_claims: [Scalar; 3],
    pub(super) inner_sumcheck: SumcheckProof,
    pub(super) witness_opening: Vec<Scalar>,
    pub(super) witness_opening_blinding: Scalar,
    pub(super) error_opening: Vec<Scalar>,
    pub(super) error_opening_blinding: Scalar,
}
impl RelaxedSpartanProof {
    pub(super) fn prove(
        shape: &Shape,
        key: &CommitmentKey,
        instance: &RelaxedInstance,
        witness: &RelaxedWitness,
        transcript: &mut VegaTranscriptV1,
    ) -> Result<Self, SpartanError> {
        let dimensions = SpartanDimensions::validate(shape, key)?;
        validate_prover_input(shape, key, instance, witness, dimensions)?;
        transcript.absorb_scalar(b"u_relaxed", instance.relaxation)?;
        transcript.absorb_scalars(b"X_relaxed", &instance.public_inputs)?;
        let mut tau = Vec::with_capacity(dimensions.outer_rounds);
        for _ in 0..dimensions.outer_rounds {
            tau.push(transcript.squeeze(b"t")?);
        }
        let [a_product, b_product, mut d_product] =
            derive_assignment_products_split(shape, instance, witness)?;
        let (d_lower, d_upper) = d_product.as_mut_slices();
        let error_half = witness.error.len() / 2;
        for (c, error) in d_lower
            .iter_mut()
            .zip(witness.error[..error_half].iter().copied())
        {
            *c = instance.relaxation * *c + error;
        }
        for (c, error) in d_upper
            .iter_mut()
            .zip(witness.error[error_half..].iter().copied())
        {
            *c = instance.relaxation * *c + error;
        }
        let (outer_sumcheck, row_point, outer_claims) = prove_cubic_with_split_first_owned(
            Scalar::zero(),
            &tau,
            a_product,
            b_product,
            d_product,
            transcript,
        )?;
        drop(tau);
        transcript.absorb_scalars(b"claims_outer", &outer_claims)?;
        let batching_challenge = transcript.squeeze(b"r")?;
        let batching_challenge_squared = batching_challenge.square();
        let row_weights = SecretScalarTable::try_eq_evals(&row_point)?;
        let error_claim = inner_product(&witness.error, row_weights.as_slice())?;
        let inner_claim = outer_claims[0]
            + batching_challenge * outer_claims[1]
            + batching_challenge_squared * (outer_claims[2] - error_claim);
        let batched_matrix = bind_batched_matrix(
            shape,
            row_weights.as_slice(),
            batching_challenge,
            instance.relaxation,
            dimensions.assignment_table_len,
        )?;
        // The row weights are no longer needed. Free them before allocating
        // the inner assignment, keeping this phase at two full prover tables.
        drop(row_weights);
        let mut assignment = SecretScalarTable::try_zeroed(dimensions.assignment_table_len)?;
        let assignment_values = assignment.as_mut_slice();
        assignment_values[..witness.values.len()].copy_from_slice(&witness.values);
        assignment_values[shape.variable_count()] = instance.relaxation;
        assignment_values[shape.variable_count() + 1..shape.columns()]
            .copy_from_slice(&instance.public_inputs);
        let (inner_sumcheck, column_point, _) = prove_quadratic_owned(
            inner_claim,
            dimensions.inner_rounds,
            batched_matrix,
            assignment,
            transcript,
        )?;
        let (witness_opening, witness_opening_blinding) = prove_direct(
            key,
            &witness.values,
            &witness.witness_blindings,
            &column_point[1..],
        )?;
        let (error_opening, error_opening_blinding) =
            prove_direct(key, &witness.error, &witness.error_blindings, &row_point)?;
        transcript.absorb_scalars(b"v_W", &witness_opening)?;
        transcript.absorb_scalars(b"v_E", &error_opening)?;
        Ok(Self {
            outer_sumcheck,
            outer_claims,
            inner_sumcheck,
            witness_opening,
            witness_opening_blinding,
            error_opening,
            error_opening_blinding,
        })
    }
    pub(super) fn verify(
        &self,
        shape: &Shape,
        key: &CommitmentKey,
        instance: &RelaxedInstance,
        transcript: &mut VegaTranscriptV1,
    ) -> Result<(), SpartanError> {
        let dimensions = SpartanDimensions::validate(shape, key)?;
        validate_verifier_input(self, shape, key, instance, dimensions)?;
        transcript.absorb_scalar(b"u_relaxed", instance.relaxation)?;
        transcript.absorb_scalars(b"X_relaxed", &instance.public_inputs)?;
        let mut tau = Vec::with_capacity(dimensions.outer_rounds);
        for _ in 0..dimensions.outer_rounds {
            tau.push(transcript.squeeze(b"t")?);
        }
        let (outer_final, row_point) =
            self.outer_sumcheck
                .verify(Scalar::zero(), dimensions.outer_rounds, 3, transcript)?;
        let expected_outer = eq_evaluate(&tau, &row_point)?
            * (self.outer_claims[0] * self.outer_claims[1] - self.outer_claims[2]);
        if outer_final != expected_outer {
            return Err(SpartanError::InvalidOuterClaim);
        }
        transcript.absorb_scalars(b"claims_outer", &self.outer_claims)?;
        let batching_challenge = transcript.squeeze(b"r")?;
        let batching_challenge_squared = batching_challenge.square();
        let error_evaluation = verify_direct(
            key,
            &instance.error_commitment,
            &self.error_opening,
            self.error_opening_blinding,
            &row_point,
        )?;
        let inner_claim = self.outer_claims[0]
            + batching_challenge * self.outer_claims[1]
            + batching_challenge_squared * (self.outer_claims[2] - error_evaluation);
        let (inner_final, column_point) =
            self.inner_sumcheck
                .verify(inner_claim, dimensions.inner_rounds, 2, transcript)?;
        let witness_evaluation = verify_direct(
            key,
            &instance.witness_commitment,
            &self.witness_opening,
            self.witness_opening_blinding,
            &column_point[1..],
        )?;
        let row_weights = eq_evals(&row_point)?;
        let column_weights = eq_evals(&column_point)?;
        let mut assignment_evaluation = (Scalar::one() - column_point[0]) * witness_evaluation;
        assignment_evaluation += instance.relaxation * column_weights[shape.variable_count()];
        for (index, input) in instance.public_inputs.iter().copied().enumerate() {
            assignment_evaluation += input * column_weights[shape.variable_count() + 1 + index];
        }
        let a_evaluation = shape.a.evaluate(&row_weights, &column_weights)?;
        let b_evaluation = shape.b.evaluate(&row_weights, &column_weights)?;
        let c_evaluation = shape.c.evaluate(&row_weights, &column_weights)?;
        let batched_matrix_evaluation = a_evaluation
            + batching_challenge * b_evaluation
            + batching_challenge_squared * instance.relaxation * c_evaluation;
        if inner_final != batched_matrix_evaluation * assignment_evaluation {
            return Err(SpartanError::InvalidInnerClaim);
        }
        transcript.absorb_scalars(b"v_W", &self.witness_opening)?;
        transcript.absorb_scalars(b"v_E", &self.error_opening)?;
        Ok(())
    }
}
fn derive_assignment_products_split(
    shape: &Shape,
    instance: &RelaxedInstance,
    witness: &RelaxedWitness,
) -> Result<[SplitSecretScalarTable; 3], SpartanError> {
    if witness.values.len() != shape.variable_count()
        || instance.public_inputs.len() != shape.public_input_count()
    {
        return Err(SpartanError::InvalidDimension);
    }
    let mut products = [
        SplitSecretScalarTable::try_zeroed(shape.constraint_count())?,
        SplitSecretScalarTable::try_zeroed(shape.constraint_count())?,
        SplitSecretScalarTable::try_zeroed(shape.constraint_count())?,
    ];
    // Both halves of all three products are reserved before the first table
    // receives witness-derived data. A later reservation failure therefore
    // drops only zero-filled owners, while every populated owner remains
    // RAII-erased.
    for (matrix, product) in [&shape.a, &shape.b, &shape.c]
        .into_iter()
        .zip(products.iter_mut())
    {
        let (lower, upper) = product.as_mut_slices();
        let half = shape.constraint_count() / 2;
        for row in 0..shape.constraint_count() {
            let mut evaluation = Scalar::zero();
            for (column, coefficient) in
                matrix.row_entries(row).ok_or(R1csError::InvalidDimension)?
            {
                let assigned = if column < shape.variable_count() {
                    witness.values[column]
                } else if column == shape.variable_count() {
                    instance.relaxation
                } else {
                    *instance
                        .public_inputs
                        .get(column - shape.variable_count() - 1)
                        .ok_or(R1csError::InvalidDimension)?
                };
                evaluation += coefficient * assigned;
            }
            if row < half {
                lower[row] = evaluation;
            } else {
                upper[row - half] = evaluation;
            }
        }
    }
    Ok(products)
}
fn bind_batched_matrix(
    shape: &Shape,
    row_weights: &[Scalar],
    batching_challenge: Scalar,
    relaxation: Scalar,
    assignment_table_len: usize,
) -> Result<SecretScalarTable, SpartanError> {
    if row_weights.len() != shape.constraint_count() || assignment_table_len < shape.columns() {
        return Err(SpartanError::InvalidDimension);
    }
    let mut batched = SecretScalarTable::try_zeroed(assignment_table_len)?;
    let batching_challenge_squared = batching_challenge.square();
    for (matrix, scale) in [
        (&shape.a, Scalar::one()),
        (&shape.b, batching_challenge),
        (&shape.c, batching_challenge_squared * relaxation),
    ] {
        for (row, row_weight) in row_weights.iter().copied().enumerate() {
            for (column, coefficient) in
                matrix.row_entries(row).ok_or(R1csError::InvalidDimension)?
            {
                batched.as_mut_slice()[column] += scale * row_weight * coefficient;
            }
        }
    }
    Ok(batched)
}
#[derive(Clone, Copy)]
struct SpartanDimensions {
    outer_rounds: usize,
    inner_rounds: usize,
    assignment_table_len: usize,
    witness_rows: usize,
    error_rows: usize,
}
impl SpartanDimensions {
    fn validate(shape: &Shape, key: &CommitmentKey) -> Result<Self, SpartanError> {
        if !key.columns().is_power_of_two() {
            return Err(SpartanError::InvalidDimension);
        }
        let outer_rounds = log2_exact(shape.constraint_count())?;
        let witness_rounds = log2_exact(shape.variable_count())?;
        let inner_rounds = witness_rounds
            .checked_add(1)
            .ok_or(SpartanError::InvalidDimension)?;
        let assignment_table_len = shape
            .variable_count()
            .checked_mul(2)
            .ok_or(SpartanError::InvalidDimension)?;
        if shape.columns() > assignment_table_len {
            return Err(SpartanError::InvalidDimension);
        }
        if evaluation_table_size(outer_rounds)? != shape.constraint_count()
            || evaluation_table_size(inner_rounds)? != assignment_table_len
        {
            return Err(SpartanError::InvalidDimension);
        }
        let witness_rows = shape.variable_count().div_ceil(key.columns());
        let error_rows = shape.constraint_count().div_ceil(key.columns());
        if witness_rows == 0
            || error_rows == 0
            || !witness_rows.is_power_of_two()
            || !error_rows.is_power_of_two()
        {
            return Err(SpartanError::InvalidDimension);
        }
        Ok(Self {
            outer_rounds,
            inner_rounds,
            assignment_table_len,
            witness_rows,
            error_rows,
        })
    }
}
fn validate_prover_input(
    shape: &Shape,
    key: &CommitmentKey,
    instance: &RelaxedInstance,
    witness: &RelaxedWitness,
    dimensions: SpartanDimensions,
) -> Result<(), SpartanError> {
    validate_instance(shape, key, instance, dimensions)?;
    if witness.values.len() != shape.variable_count()
        || witness.witness_blindings.len() != dimensions.witness_rows
        || witness.error.len() != shape.constraint_count()
        || witness.error_blindings.len() != dimensions.error_rows
    {
        return Err(SpartanError::InvalidDimension);
    }
    shape.validate_relaxed_assignment(
        &witness.values,
        instance.relaxation,
        &instance.public_inputs,
        &witness.error,
    )?;
    if key.commit(&witness.values, &witness.witness_blindings)? != instance.witness_commitment
        || key.commit(&witness.error, &witness.error_blindings)? != instance.error_commitment
    {
        return Err(SpartanError::CommitmentMismatch);
    }
    Ok(())
}
fn validate_verifier_input(
    proof: &RelaxedSpartanProof,
    shape: &Shape,
    key: &CommitmentKey,
    instance: &RelaxedInstance,
    dimensions: SpartanDimensions,
) -> Result<(), SpartanError> {
    validate_instance(shape, key, instance, dimensions)?;
    if proof.outer_sumcheck.rounds.len() != dimensions.outer_rounds
        || proof
            .outer_sumcheck
            .rounds
            .iter()
            .any(|round| round.coefficients_except_linear.len() != 3)
        || proof.inner_sumcheck.rounds.len() != dimensions.inner_rounds
        || proof
            .inner_sumcheck
            .rounds
            .iter()
            .any(|round| round.coefficients_except_linear.len() != 2)
        || proof.witness_opening.len() != key.columns()
        || proof.error_opening.len() != key.columns()
    {
        return Err(SpartanError::InvalidDimension);
    }
    Ok(())
}
fn validate_instance(
    shape: &Shape,
    _key: &CommitmentKey,
    instance: &RelaxedInstance,
    dimensions: SpartanDimensions,
) -> Result<(), SpartanError> {
    if instance.public_inputs.len() != shape.public_input_count()
        || instance.witness_commitment.len() != dimensions.witness_rows
        || instance.error_commitment.len() != dimensions.error_rows
    {
        return Err(SpartanError::InvalidDimension);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::{
        nifs::{NovaNifs, NovaNifsProverInput},
        r1cs::{Instance, SparseMatrix, Witness},
    };
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
    fn fixture() -> (
        CommitmentKey,
        Shape,
        RelaxedInstance,
        RelaxedWitness,
        Instance,
        Witness,
    ) {
        // Four independent bit constraints x_i * x_i = x_i.
        let entries = (0..4).map(|index| (index, index, s(1))).collect::<Vec<_>>();
        let a = SparseMatrix::new(4, 5, &entries).expect("canonical A");
        let b = SparseMatrix::new(4, 5, &entries).expect("canonical B");
        let c = SparseMatrix::new(4, 5, &entries).expect("canonical C");
        let shape = Shape::new(4, 4, 0, a, b, c).expect("shape");
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
    fn streamed_products_and_reused_batch_match_materialized_algebra() {
        let (_, shape, instance, witness, _, _) = fixture();
        let mut assignment = witness.values.clone();
        assignment.push(instance.relaxation);
        assignment.extend_from_slice(&instance.public_inputs);
        let expected_products = shape.multiply(&assignment).expect("fixture dimensions");
        let [actual_a, actual_b, actual_c] =
            derive_assignment_products_split(&shape, &instance, &witness)
                .expect("streamed products");
        let half = shape.constraint_count() / 2;
        for (actual, expected) in [actual_a, actual_b, actual_c].into_iter().zip([
            expected_products.a,
            expected_products.b,
            expected_products.c,
        ]) {
            let (lower, upper) = actual.as_slices();
            assert_eq!(lower, &expected[..half]);
            assert_eq!(upper, &expected[half..]);
        }
        let row_weights = [s(2), s(3), s(5), s(7)];
        let challenge = s(11);
        let challenge_squared = challenge.square();
        let a_bound = shape.a.bind_rows(&row_weights).expect("A rows");
        let b_bound = shape.b.bind_rows(&row_weights).expect("B rows");
        let c_bound = shape.c.bind_rows(&row_weights).expect("C rows");
        let mut expected_batch = a_bound
            .into_iter()
            .zip(b_bound)
            .zip(c_bound)
            .map(|((a, b), c)| a + challenge * b + challenge_squared * instance.relaxation * c)
            .collect::<Vec<_>>();
        expected_batch.resize(shape.variable_count() * 2, Scalar::zero());
        let actual_batch = bind_batched_matrix(
            &shape,
            &row_weights,
            challenge,
            instance.relaxation,
            shape.variable_count() * 2,
        )
        .expect("single-buffer batch");
        assert_eq!(actual_batch.as_slice(), expected_batch.as_slice());
    }
    #[test]
    fn spartan_prover_source_splits_and_releases_first_round_tables() {
        let source = include_str!("spartan.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source");
        assert!(!production.contains("let u_cz_plus_error"));
        assert!(!production.contains("shape.multiply(&assignment)"));
        assert!(!production.contains(".bind_rows("));
        assert!(production.contains("*c = instance.relaxation * *c + error"));
        assert!(production.contains("prove_cubic_with_split_first_owned"));
        assert!(production.contains("derive_assignment_products_split"));
        assert!(!production.contains("SecretScalarTable::try_eq_evals(&tau)"));
        assert!(production.contains("prove_quadratic_owned"));
        assert!(production.contains("drop(row_weights)"));
    }
    fn prove_composed() -> (
        CommitmentKey,
        Shape,
        RelaxedInstance,
        Instance,
        NovaNifs,
        RelaxedInstance,
        RelaxedWitness,
        RelaxedSpartanProof,
    ) {
        let (key, shape, u1, w1, u2, w2) = fixture();
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        let cross_term_blindings = [s(31), s(37)];
        let (nifs, folded_instance, folded_witness) = NovaNifs::prove(
            NovaNifsProverInput {
                key: &key,
                shape: &shape,
                relaxed_instance: &u1,
                relaxed_witness: &w1,
                regular_instance: &u2,
                regular_witness: &w2,
                cross_term_blindings: &cross_term_blindings,
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
            u1,
            u2,
            nifs,
            folded_instance,
            folded_witness,
            proof,
        )
    }
    fn verify_composed(
        key: &CommitmentKey,
        shape: &Shape,
        u1: &RelaxedInstance,
        u2: &Instance,
        nifs: &NovaNifs,
        proof: &RelaxedSpartanProof,
    ) -> Result<RelaxedInstance, SpartanError> {
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        let folded = nifs
            .verify(key, shape, &mut transcript, u1, u2)
            .map_err(|_| SpartanError::InvalidDimension)?;
        proof.verify(shape, key, &folded, &mut transcript)?;
        Ok(folded)
    }
    #[test]
    fn composed_nifs_and_relaxed_spartan_prove_and_verify() {
        let (key, shape, u1, u2, nifs, folded, witness, proof) = prove_composed();
        let verified = verify_composed(&key, &shape, &u1, &u2, &nifs, &proof).expect("valid proof");
        assert_eq!(verified, folded);
        shape
            .validate_relaxed_assignment(
                &witness.values,
                verified.relaxation,
                &verified.public_inputs,
                &witness.error,
            )
            .expect("satisfying fold");
        assert_eq!(proof.outer_sumcheck.rounds.len(), 2);
        assert_eq!(proof.inner_sumcheck.rounds.len(), 3);
        assert_eq!(proof.witness_opening.len(), 2);
        assert_eq!(proof.error_opening.len(), 2);
        // Independent pure-Python pinned-source fixture. This locks the
        // high-to-low binding order, compressed polynomial coefficients,
        // transcript schedule, and both direct openings end-to-end.
        assert_eq!(
            verified.relaxation - s(7),
            h("1224ebf7043b96e2657a33ff70ec7f82256252490daebbcc825a5176d070efb0")
        );
        assert_eq!(
            proof
                .outer_sumcheck
                .rounds
                .iter()
                .map(|round| round.coefficients_except_linear.clone())
                .collect::<Vec<_>>(),
            vec![
                vec![
                    h("0000000000000000000000000000000000000000000000000000000000000000"),
                    h("c6144e819c4fda7e8ce05a0e035bfff5a8de6b4ccbb21b95cb3328610d6c3383"),
                    h("e7d26824b4241cf055febedde7fb8abcf8747f67e6d5d0884fa2fe86e7d1150d"),
                ],
                vec![
                    h("7796c374775c57196abd238db22be76647e1419d21b21403bfd03798122a7f43"),
                    h("005fe6f0a003108e35c8ea01bed7d917d3bfd7f826e667eafaad1452a7bb6f08"),
                    h("d1a39df4eceb6340f2ad19396b52a4a1183da44b4ba422f0f1077f169c0b606c"),
                ],
            ]
        );
        assert_eq!(
            proof.outer_claims,
            [
                h("0a96ea0672892cb3047980c13760c1dbc6912862049b69014af2bec1be637723"),
                h("0a96ea0672892cb3047980c13760c1dbc6912862049b69014af2bec1be637723"),
                h("44be28de81a93541d9823ff680efe378e3af7ed257775a0f9fc3a56813f6e56d"),
            ]
        );
        assert_eq!(
            proof
                .inner_sumcheck
                .rounds
                .iter()
                .map(|round| round.coefficients_except_linear.clone())
                .collect::<Vec<_>>(),
            vec![
                vec![
                    h("370293dc6bf9f7de5a913e16a59a745391cbaf98d40d0eb4bf820e1c0df2b7a7"),
                    h("4102bd92629885d15bf6ccbec18704e12e830735bdb7143f8d0359637df3a7d9"),
                ],
                vec![
                    h("2b7b00dad38fa3cffeffe47557e4ae1db59da3ed36e930bf82f3fbc02da97c47"),
                    h("ff065f5968b316539342491a4b3b0489ac251ff0f862cb97d6d6043fd36e58c4"),
                ],
                vec![
                    h("8c0cc32a4d3fd621ecafd7aa0f036ebefce35bccccc04da85522660323fb210c"),
                    h("16e9d210ab7f35dd7d0c228d7bfe4562868c185ace2ffd8632330dd1c88e1af7"),
                ],
            ]
        );
        assert_eq!(
            proof.witness_opening,
            vec![
                h("2fa4819bc9964f0e63b3c04f5d9d6c58b2bdda599344323cc29b4e33f5d42852"),
                h("e1d6ab5f25b4d29f6572b09c7f9f5186120d4a1ecf6c568750ae75db5cfcbb56"),
            ]
        );
        assert_eq!(
            proof.witness_opening_blinding,
            h("58db4b3a68d1e531878b120e8386bda5055b78f07b51965a58ef251f2d21d1b3")
        );
        assert_eq!(
            proof.error_opening,
            vec![
                h("547e9327e545a24cc427abe35aeb53fb5364f4dd12d0d2282d3c076bd77b60ad"),
                h("b7ba13c7e3f3e3a02bf605eb1d7e41b945af74598f57331ee2ea2ff09a9f4c9d"),
            ]
        );
        assert_eq!(
            proof.error_opening_blinding,
            h("7c4175b3a65853572a1520d371e8794bd8ff5a867a934a656d3246d7e302beea")
        );
    }
    #[test]
    fn every_spartan_response_category_is_bound_and_checked() {
        let (key, shape, u1, u2, nifs, _, _, proof) = prove_composed();
        for round in 0..proof.outer_sumcheck.rounds.len() {
            for coefficient in 0..3 {
                let mut altered = proof.clone();
                altered.outer_sumcheck.rounds[round].coefficients_except_linear[coefficient] +=
                    Scalar::one();
                assert!(
                    verify_composed(&key, &shape, &u1, &u2, &nifs, &altered).is_err(),
                    "outer round {round} coefficient {coefficient}"
                );
            }
        }
        for claim in 0..3 {
            let mut altered = proof.clone();
            altered.outer_claims[claim] += Scalar::one();
            assert!(
                verify_composed(&key, &shape, &u1, &u2, &nifs, &altered).is_err(),
                "outer claim {claim}"
            );
        }
        for round in 0..proof.inner_sumcheck.rounds.len() {
            for coefficient in 0..2 {
                let mut altered = proof.clone();
                altered.inner_sumcheck.rounds[round].coefficients_except_linear[coefficient] +=
                    Scalar::one();
                assert!(
                    verify_composed(&key, &shape, &u1, &u2, &nifs, &altered).is_err(),
                    "inner round {round} coefficient {coefficient}"
                );
            }
        }
        for index in 0..proof.witness_opening.len() {
            let mut altered = proof.clone();
            altered.witness_opening[index] += Scalar::one();
            assert!(
                verify_composed(&key, &shape, &u1, &u2, &nifs, &altered).is_err(),
                "witness opening {index}"
            );
        }
        let mut altered = proof.clone();
        altered.witness_opening_blinding += Scalar::one();
        assert!(verify_composed(&key, &shape, &u1, &u2, &nifs, &altered).is_err());
        for index in 0..proof.error_opening.len() {
            let mut altered = proof.clone();
            altered.error_opening[index] += Scalar::one();
            assert!(
                verify_composed(&key, &shape, &u1, &u2, &nifs, &altered).is_err(),
                "error opening {index}"
            );
        }
        let mut altered = proof.clone();
        altered.error_opening_blinding += Scalar::one();
        assert!(verify_composed(&key, &shape, &u1, &u2, &nifs, &altered).is_err());
    }
    #[test]
    fn spartan_rejects_round_degree_opening_and_instance_shape_attacks() {
        let (key, shape, u1, u2, nifs, folded, _, proof) = prove_composed();
        let mut altered = proof.clone();
        altered.outer_sumcheck.rounds.pop();
        assert_eq!(
            verify_composed(&key, &shape, &u1, &u2, &nifs, &altered),
            Err(SpartanError::InvalidDimension)
        );
        let mut altered = proof.clone();
        altered.inner_sumcheck.rounds[0]
            .coefficients_except_linear
            .push(Scalar::zero());
        assert_eq!(
            verify_composed(&key, &shape, &u1, &u2, &nifs, &altered),
            Err(SpartanError::InvalidDimension)
        );
        let mut altered = proof.clone();
        altered.witness_opening.pop();
        assert_eq!(
            verify_composed(&key, &shape, &u1, &u2, &nifs, &altered),
            Err(SpartanError::InvalidDimension)
        );
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        let verified_fold = nifs
            .verify(&key, &shape, &mut transcript, &u1, &u2)
            .expect("NIFS");
        assert_eq!(verified_fold, folded);
        let mut altered_instance = verified_fold.clone();
        altered_instance.relaxation += Scalar::one();
        assert!(
            proof
                .verify(&shape, &key, &altered_instance, &mut transcript)
                .is_err()
        );
        let mut altered_context = VegaTranscriptV1::new_neutron_nova();
        altered_context
            .domain_separator(b"cross-context-replay")
            .expect("bounded");
        let altered_fold = nifs
            .verify(&key, &shape, &mut altered_context, &u1, &u2)
            .expect("well-shaped NIFS");
        assert!(
            proof
                .verify(&shape, &key, &altered_fold, &mut altered_context)
                .is_err()
        );
    }
}
