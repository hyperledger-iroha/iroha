//! Hyrax multilinear polynomial commitments and linear inner-product argument.

use thiserror::Error;

use super::{
    VegaCurveError, VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar, VegaTranscriptError,
    VegaTranscriptV1,
    algebra::{AlgebraError, eq_evals, inner_product, log2_exact},
    commitment::{Commitment, CommitmentError, CommitmentKey, msm},
};

const IPA_PROTOCOL_NAME: &[u8] = b"inner product argument (linear)";

/// Failure while proving or verifying a canonical Hyrax opening.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(super) enum HyraxError {
    #[error("Vega Hyrax dimensions do not match the fixed opening shape")]
    InvalidDimension,
    #[error("Vega Hyrax inner-product equation failed")]
    InvalidInnerProductProof,
    #[error("Vega Hyrax direct opening does not match its commitment")]
    InvalidDirectOpening,
    #[error(transparent)]
    Algebra(#[from] AlgebraError),
    #[error(transparent)]
    Commitment(#[from] CommitmentError),
    #[error(transparent)]
    Curve(#[from] VegaCurveError),
    #[error(transparent)]
    Transcript(#[from] VegaTranscriptError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct InnerProductArgument {
    pub(super) delta: Point,
    pub(super) beta: Point,
    pub(super) z_vec: Vec<Scalar>,
    pub(super) z_delta: Scalar,
    pub(super) z_beta: Scalar,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EvaluationArgument {
    pub(super) inner_product: InnerProductArgument,
}

pub(super) struct InnerProductRandomness<'a> {
    pub(super) d_vec: &'a [Scalar],
    pub(super) r_delta: Scalar,
    pub(super) r_beta: Scalar,
}

struct InnerProductInstance {
    comm_a: Point,
    comm_c: Point,
}

struct InnerProductWitness<'a> {
    a_vec: &'a [Scalar],
    r_a: Scalar,
    r_c: Scalar,
}

pub(super) fn prove_inner_product(
    key: &CommitmentKey,
    evaluation_key: &CommitmentKey,
    comm_a: Point,
    b_vec: &[Scalar],
    comm_c: Point,
    a_vec: &[Scalar],
    r_a: Scalar,
    r_c: Scalar,
    randomness: InnerProductRandomness<'_>,
    transcript: &mut VegaTranscriptV1,
) -> Result<InnerProductArgument, HyraxError> {
    let instance = InnerProductInstance { comm_a, comm_c };
    let witness = InnerProductWitness { a_vec, r_a, r_c };
    if a_vec.len() != b_vec.len()
        || randomness.d_vec.len() != b_vec.len()
        || key.generators().len() < b_vec.len()
        || evaluation_key.generators().is_empty()
        || comm_a.is_identity()
        || comm_c.is_identity()
    {
        return Err(HyraxError::InvalidDimension);
    }

    absorb_inner_product_instance(&instance, transcript)?;
    let delta = msm(
        randomness.d_vec,
        &key.generators()[..randomness.d_vec.len()],
    )?
    .add(key.hiding_generator().mul_scalar(randomness.r_delta));
    let beta = evaluation_key.generators()[0]
        .mul_scalar(inner_product(b_vec, randomness.d_vec)?)
        .add(
            evaluation_key
                .hiding_generator()
                .mul_scalar(randomness.r_beta),
        );
    if delta.is_identity() || beta.is_identity() {
        return Err(HyraxError::InvalidDimension);
    }
    transcript.absorb_point(b"delta", delta)?;
    transcript.absorb_point(b"beta", beta)?;
    let challenge = transcript.squeeze(b"r")?;

    let z_vec = witness
        .a_vec
        .iter()
        .copied()
        .zip(randomness.d_vec.iter().copied())
        .map(|(witness, mask)| challenge * witness + mask)
        .collect();
    Ok(InnerProductArgument {
        delta,
        beta,
        z_vec,
        z_delta: challenge * witness.r_a + randomness.r_delta,
        z_beta: challenge * witness.r_c + randomness.r_beta,
    })
}

pub(super) fn verify_inner_product(
    key: &CommitmentKey,
    evaluation_key: &CommitmentKey,
    comm_a: Point,
    b_vec: &[Scalar],
    comm_c: Point,
    expected_size: usize,
    argument: &InnerProductArgument,
    transcript: &mut VegaTranscriptV1,
) -> Result<(), HyraxError> {
    let instance = InnerProductInstance { comm_a, comm_c };
    if b_vec.len() != expected_size
        || argument.z_vec.len() != expected_size
        || key.generators().len() < expected_size
        || evaluation_key.generators().is_empty()
        || argument.delta.is_identity()
        || argument.beta.is_identity()
    {
        return Err(HyraxError::InvalidDimension);
    }
    absorb_inner_product_instance(&instance, transcript)?;
    transcript.absorb_point(b"delta", argument.delta)?;
    transcript.absorb_point(b"beta", argument.beta)?;
    let challenge = transcript.squeeze(b"r")?;

    let first_left = comm_a.mul_scalar(challenge).add(argument.delta);
    let first_right = msm(&argument.z_vec, &key.generators()[..argument.z_vec.len()])?
        .add(key.hiding_generator().mul_scalar(argument.z_delta));
    if first_left != first_right {
        return Err(HyraxError::InvalidInnerProductProof);
    }

    let second_left = comm_c.mul_scalar(challenge).add(argument.beta);
    let second_right = evaluation_key.generators()[0]
        .mul_scalar(inner_product(&argument.z_vec, b_vec)?)
        .add(
            evaluation_key
                .hiding_generator()
                .mul_scalar(argument.z_beta),
        );
    if second_left != second_right {
        return Err(HyraxError::InvalidInnerProductProof);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prove_evaluation(
    key: &CommitmentKey,
    evaluation_key: &CommitmentKey,
    transcript: &mut VegaTranscriptV1,
    commitment: &Commitment,
    polynomial: &[Scalar],
    blindings: &[Scalar],
    point: &[Scalar],
    evaluation_commitment: &Commitment,
    evaluation_blinding: Scalar,
    randomness: InnerProductRandomness<'_>,
) -> Result<EvaluationArgument, HyraxError> {
    let expected_len = 1_usize
        .checked_shl(u32::try_from(point.len()).map_err(|_| HyraxError::InvalidDimension)?)
        .ok_or(HyraxError::InvalidDimension)?;
    if polynomial.len() != expected_len
        || evaluation_commitment.len() != 1
        || commitment.len() != polynomial.len().div_ceil(key.columns())
        || blindings.len() != commitment.len()
        || !key.columns().is_power_of_two()
    {
        return Err(HyraxError::InvalidDimension);
    }
    transcript.absorb_commitment(b"poly_com", commitment)?;
    let (comm_lz, right_weights, bound_vector, bound_blinding) =
        bind_rows(key, commitment, polynomial, blindings, point)?;
    let inner_product = prove_inner_product(
        key,
        evaluation_key,
        comm_lz,
        &right_weights,
        evaluation_commitment.points()[0],
        &bound_vector,
        bound_blinding,
        evaluation_blinding,
        randomness,
        transcript,
    )?;
    Ok(EvaluationArgument { inner_product })
}

pub(super) fn verify_evaluation(
    key: &CommitmentKey,
    evaluation_key: &CommitmentKey,
    transcript: &mut VegaTranscriptV1,
    commitment: &Commitment,
    point: &[Scalar],
    evaluation_commitment: &Commitment,
    argument: &EvaluationArgument,
) -> Result<(), HyraxError> {
    let polynomial_len = 1_usize
        .checked_shl(u32::try_from(point.len()).map_err(|_| HyraxError::InvalidDimension)?)
        .ok_or(HyraxError::InvalidDimension)?;
    let row_count = polynomial_len.div_ceil(key.columns());
    if commitment.len() != row_count
        || evaluation_commitment.len() != 1
        || !row_count.is_power_of_two()
        || !key.columns().is_power_of_two()
    {
        return Err(HyraxError::InvalidDimension);
    }
    transcript.absorb_commitment(b"poly_com", commitment)?;
    let row_variables = log2_exact(row_count)?;
    let (comm_lz, right_weights) = if row_variables == 0 {
        (commitment.points()[0], eq_evals(point)?)
    } else {
        let left_weights = eq_evals(&point[..row_variables])?;
        let right_weights = eq_evals(&point[row_variables..])?;
        (msm(&left_weights, commitment.points())?, right_weights)
    };
    verify_inner_product(
        key,
        evaluation_key,
        comm_lz,
        &right_weights,
        evaluation_commitment.points()[0],
        right_weights.len(),
        &argument.inner_product,
        transcript,
    )
}

pub(super) fn prove_direct(
    key: &CommitmentKey,
    polynomial: &[Scalar],
    blindings: &[Scalar],
    point: &[Scalar],
) -> Result<(Vec<Scalar>, Scalar), HyraxError> {
    let padded_len = 1_usize
        .checked_shl(u32::try_from(point.len()).map_err(|_| HyraxError::InvalidDimension)?)
        .ok_or(HyraxError::InvalidDimension)?;
    if polynomial.is_empty() || polynomial.len() > padded_len || !key.columns().is_power_of_two() {
        return Err(HyraxError::InvalidDimension);
    }
    let row_count = padded_len.div_ceil(key.columns());
    if blindings.len() != polynomial.len().div_ceil(key.columns()) || !row_count.is_power_of_two() {
        return Err(HyraxError::InvalidDimension);
    }
    if row_count == 1 {
        let mut values = polynomial.to_vec();
        values.resize(key.columns(), Scalar::zero());
        return Ok((values, blindings[0]));
    }
    let row_variables = log2_exact(row_count)?;
    let left_weights = eq_evals(&point[..row_variables])?;
    let mut padded = polynomial.to_vec();
    padded.resize(padded_len, Scalar::zero());
    let mut values = vec![Scalar::zero(); key.columns()];
    for (row, weight) in padded
        .chunks_exact(key.columns())
        .zip(left_weights.iter().copied())
    {
        for (output, value) in values.iter_mut().zip(row.iter().copied()) {
            *output += weight * value;
        }
    }
    let combined_blinding = inner_product(&left_weights[..blindings.len()], blindings)?;
    Ok((values, combined_blinding))
}

pub(super) fn verify_direct(
    key: &CommitmentKey,
    commitment: &Commitment,
    values: &[Scalar],
    combined_blinding: Scalar,
    point: &[Scalar],
) -> Result<Scalar, HyraxError> {
    if values.len() != key.columns() || commitment.len() == 0 {
        return Err(HyraxError::InvalidDimension);
    }
    let padded_len = 1_usize
        .checked_shl(u32::try_from(point.len()).map_err(|_| HyraxError::InvalidDimension)?)
        .ok_or(HyraxError::InvalidDimension)?;
    let row_count = padded_len.div_ceil(key.columns());
    if commitment.len() > row_count
        || !row_count.is_power_of_two()
        || !key.columns().is_power_of_two()
    {
        return Err(HyraxError::InvalidDimension);
    }
    let row_variables = log2_exact(row_count)?;
    let comm_lz = if row_variables == 0 {
        commitment.points()[0]
    } else {
        let left_weights = eq_evals(&point[..row_variables])?;
        msm(&left_weights[..commitment.len()], commitment.points())?
    };
    let expected =
        msm(values, key.generators())?.add(key.hiding_generator().mul_scalar(combined_blinding));
    if comm_lz != expected {
        return Err(HyraxError::InvalidDirectOpening);
    }
    let right_weights = eq_evals(&point[row_variables..])?;
    Ok(inner_product(values, &right_weights)?)
}

fn absorb_inner_product_instance(
    instance: &InnerProductInstance,
    transcript: &mut VegaTranscriptV1,
) -> Result<(), HyraxError> {
    transcript.domain_separator(IPA_PROTOCOL_NAME)?;
    let mut bytes = Vec::with_capacity(128);
    bytes.extend_from_slice(&instance.comm_a.to_transcript_bytes()?);
    bytes.extend_from_slice(&instance.comm_c.to_transcript_bytes()?);
    transcript.absorb_raw(b"U", &bytes)?;
    Ok(())
}

fn bind_rows(
    key: &CommitmentKey,
    commitment: &Commitment,
    polynomial: &[Scalar],
    blindings: &[Scalar],
    point: &[Scalar],
) -> Result<(Point, Vec<Scalar>, Vec<Scalar>, Scalar), HyraxError> {
    let row_count = polynomial.len().div_ceil(key.columns());
    if !row_count.is_power_of_two() {
        return Err(HyraxError::InvalidDimension);
    }
    let row_variables = log2_exact(row_count)?;
    if row_variables == 0 {
        return Ok((
            commitment.points()[0],
            eq_evals(point)?,
            polynomial.to_vec(),
            blindings[0],
        ));
    }
    let left_weights = eq_evals(&point[..row_variables])?;
    let right_weights = eq_evals(&point[row_variables..])?;
    if polynomial.len() != left_weights.len() * right_weights.len() {
        return Err(HyraxError::InvalidDimension);
    }
    let mut bound_vector = vec![Scalar::zero(); right_weights.len()];
    for (row, weight) in polynomial
        .chunks_exact(right_weights.len())
        .zip(left_weights.iter().copied())
    {
        for (output, value) in bound_vector.iter_mut().zip(row.iter().copied()) {
            *output += weight * value;
        }
    }
    let bound_blinding = inner_product(&left_weights, blindings)?;
    let comm_lz = msm(&left_weights, commitment.points())?;
    Ok((comm_lz, right_weights, bound_vector, bound_blinding))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(value: u64) -> Scalar {
        Scalar::from_u64(value)
    }

    fn setup() -> (CommitmentKey, CommitmentKey) {
        (
            CommitmentKey::derive(b"hyrax-main-test", 4).expect("main key"),
            CommitmentKey::derive(b"hyrax-eval-test", 4).expect("evaluation key"),
        )
    }

    #[test]
    fn linear_inner_product_proves_and_rejects_each_equation_mutation() {
        let (key, evaluation_key) = setup();
        let a = [s(2), s(3), s(5), s(7)];
        let b = [s(11), s(13), s(17), s(19)];
        let r_a = s(23);
        let r_c = s(29);
        let comm_a = msm(&a, key.generators())
            .expect("aligned")
            .add(key.hiding_generator().mul_scalar(r_a));
        let comm_c = evaluation_key.generators()[0]
            .mul_scalar(inner_product(&a, &b).expect("aligned"))
            .add(evaluation_key.hiding_generator().mul_scalar(r_c));
        let randomness_values = [s(31), s(37), s(41), s(43)];
        let randomness = InnerProductRandomness {
            d_vec: &randomness_values,
            r_delta: s(47),
            r_beta: s(53),
        };
        let mut prover_transcript = VegaTranscriptV1::new_neutron_nova();
        let proof = prove_inner_product(
            &key,
            &evaluation_key,
            comm_a,
            &b,
            comm_c,
            &a,
            r_a,
            r_c,
            randomness,
            &mut prover_transcript,
        )
        .expect("valid proof");
        verify_inner_product(
            &key,
            &evaluation_key,
            comm_a,
            &b,
            comm_c,
            4,
            &proof,
            &mut VegaTranscriptV1::new_neutron_nova(),
        )
        .expect("valid verification");

        for mutate in 0..5 {
            let mut bad = proof.clone();
            match mutate {
                0 => bad.delta = bad.delta.add(key.generators()[0]),
                1 => bad.beta = bad.beta.add(key.generators()[0]),
                2 => bad.z_vec[0] += Scalar::one(),
                3 => bad.z_delta += Scalar::one(),
                _ => bad.z_beta += Scalar::one(),
            }
            assert!(
                verify_inner_product(
                    &key,
                    &evaluation_key,
                    comm_a,
                    &b,
                    comm_c,
                    4,
                    &bad,
                    &mut VegaTranscriptV1::new_neutron_nova(),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn hyrax_evaluation_and_direct_opening_roundtrip() {
        let (key, evaluation_key) = setup();
        let polynomial = [s(2), s(3), s(5), s(7)];
        let point = [s(11), s(13)];
        let weights = eq_evals(&point).expect("small table");
        let evaluation = inner_product(&polynomial, &weights).expect("aligned");
        let polynomial_blinding = s(17);
        let evaluation_blinding = s(19);
        let commitment = key
            .commit(&polynomial, &[polynomial_blinding])
            .expect("one row");
        let evaluation_commitment = evaluation_key
            .commit(
                &[evaluation, Scalar::zero(), Scalar::zero(), Scalar::zero()],
                &[evaluation_blinding],
            )
            .expect("one row");
        let masks = [s(23), s(29), s(31), s(37)];
        let mut prover_transcript = VegaTranscriptV1::new_neutron_nova();
        let argument = prove_evaluation(
            &key,
            &evaluation_key,
            &mut prover_transcript,
            &commitment,
            &polynomial,
            &[polynomial_blinding],
            &point,
            &evaluation_commitment,
            evaluation_blinding,
            InnerProductRandomness {
                d_vec: &masks,
                r_delta: s(41),
                r_beta: s(43),
            },
        )
        .expect("valid opening");
        verify_evaluation(
            &key,
            &evaluation_key,
            &mut VegaTranscriptV1::new_neutron_nova(),
            &commitment,
            &point,
            &evaluation_commitment,
            &argument,
        )
        .expect("valid verification");

        let (values, blind) =
            prove_direct(&key, &polynomial, &[polynomial_blinding], &point).expect("direct");
        assert_eq!(
            verify_direct(&key, &commitment, &values, blind, &point).expect("valid direct"),
            evaluation
        );
        let mut bad_values = values;
        bad_values[0] += Scalar::one();
        assert!(verify_direct(&key, &commitment, &bad_values, blind, &point).is_err());
    }

    #[test]
    fn hyrax_rejects_shape_and_randomness_confusion() {
        let (key, evaluation_key) = setup();
        let polynomial = [s(1), s(2), s(3), s(4)];
        let commitment = key.commit(&polynomial, &[s(5)]).expect("one row");
        let eval_commitment = evaluation_key
            .commit(&[s(1), s(0), s(0), s(0)], &[s(7)])
            .expect("one row");
        assert!(
            prove_evaluation(
                &key,
                &evaluation_key,
                &mut VegaTranscriptV1::new_neutron_nova(),
                &commitment,
                &polynomial,
                &[s(5)],
                &[s(9), s(11)],
                &eval_commitment,
                s(7),
                InnerProductRandomness {
                    d_vec: &[s(1)],
                    r_delta: s(2),
                    r_beta: s(3),
                },
            )
            .is_err()
        );
        assert!(
            verify_evaluation(
                &key,
                &evaluation_key,
                &mut VegaTranscriptV1::new_neutron_nova(),
                &commitment,
                &[s(9), s(11), s(13)],
                &eval_commitment,
                &EvaluationArgument {
                    inner_product: InnerProductArgument {
                        delta: key.generators()[0],
                        beta: key.generators()[1],
                        z_vec: vec![s(1); 4],
                        z_delta: s(1),
                        z_beta: s(1),
                    }
                },
            )
            .is_err()
        );
    }
}
