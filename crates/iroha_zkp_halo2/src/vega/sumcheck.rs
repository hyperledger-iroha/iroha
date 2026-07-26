//! Canonical compressed sum-check verifier used by Vega Spartan.

use thiserror::Error;

use super::{
    VegaT256ScalarV1 as Scalar, VegaTranscriptError, VegaTranscriptV1,
    algebra::{
        AlgebraError, decompress_univariate, eq_evals, evaluate_univariate, evaluation_table_size,
    },
};

/// Failure while replaying a Vega sum-check proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(super) enum SumcheckError {
    #[error("Vega sum-check has the wrong number of rounds")]
    WrongRoundCount,
    #[error("Vega sum-check round has the wrong polynomial degree")]
    WrongDegree,
    #[error("Vega sum-check prover claim does not match its evaluation tables")]
    InvalidClaim,
    #[error(transparent)]
    Algebra(#[from] AlgebraError),
    #[error(transparent)]
    Transcript(#[from] VegaTranscriptError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CompressedUnivariate {
    pub(super) coefficients_except_linear: Vec<Scalar>,
}

impl CompressedUnivariate {
    pub(super) fn new(
        coefficients_except_linear: Vec<Scalar>,
        degree: usize,
    ) -> Result<Self, SumcheckError> {
        if degree == 0 || coefficients_except_linear.len() != degree {
            return Err(SumcheckError::WrongDegree);
        }
        Ok(Self {
            coefficients_except_linear,
        })
    }

    pub(super) fn coefficients(&self) -> &[Scalar] {
        &self.coefficients_except_linear
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SumcheckProof {
    pub(super) rounds: Vec<CompressedUnivariate>,
}

/// Prove the cubic outer Spartan sum-check
/// `sum_x eq(tau,x) * (A(x) * B(x) - D(x))`.
pub(super) fn prove_cubic_with_three_inputs(
    initial_claim: Scalar,
    tau: &[Scalar],
    a: &[Scalar],
    b: &[Scalar],
    d: &[Scalar],
    transcript: &mut VegaTranscriptV1,
) -> Result<(SumcheckProof, Vec<Scalar>, [Scalar; 3]), SumcheckError> {
    let expected = table_size(tau.len())?;
    if expected == 0 || a.len() != expected || b.len() != expected || d.len() != expected {
        return Err(SumcheckError::WrongRoundCount);
    }
    let mut eq = eq_evals(tau)?;
    let mut a = a.to_vec();
    let mut b = b.to_vec();
    let mut d = d.to_vec();
    let mut claim = initial_claim;
    let mut rounds = Vec::with_capacity(tau.len());
    let mut challenges = Vec::with_capacity(tau.len());

    for _ in 0..tau.len() {
        let half = a.len() / 2;
        if half == 0 || b.len() != a.len() || d.len() != a.len() || eq.len() != a.len() {
            return Err(SumcheckError::WrongRoundCount);
        }
        let mut evaluation_zero = Scalar::zero();
        let mut evaluation_one = Scalar::zero();
        let mut evaluation_two = Scalar::zero();
        let mut evaluation_three = Scalar::zero();
        for index in 0..half {
            let eq_zero = eq[index];
            let a_zero = a[index];
            let b_zero = b[index];
            let d_zero = d[index];
            let delta_eq = eq[half + index] - eq_zero;
            let delta_a = a[half + index] - a_zero;
            let delta_b = b[half + index] - b_zero;
            let delta_d = d[half + index] - d_zero;

            evaluation_zero += eq_zero * (a_zero * b_zero - d_zero);
            evaluation_one +=
                (eq_zero + delta_eq) * ((a_zero + delta_a) * (b_zero + delta_b) - d_zero - delta_d);
            let two = Scalar::from_u64(2);
            evaluation_two += (eq_zero + two * delta_eq)
                * ((a_zero + two * delta_a) * (b_zero + two * delta_b) - d_zero - two * delta_d);
            let three = Scalar::from_u64(3);
            evaluation_three += (eq_zero + three * delta_eq)
                * ((a_zero + three * delta_a) * (b_zero + three * delta_b)
                    - d_zero
                    - three * delta_d);
        }
        if evaluation_zero + evaluation_one != claim {
            return Err(SumcheckError::InvalidClaim);
        }
        let coefficients = interpolate_cubic([
            evaluation_zero,
            evaluation_one,
            evaluation_two,
            evaluation_three,
        ])?;
        let compressed =
            CompressedUnivariate::new(vec![coefficients[0], coefficients[2], coefficients[3]], 3)?;
        transcript.absorb_univariate(b"p", compressed.coefficients())?;
        let challenge = transcript.squeeze(b"c")?;
        claim = evaluate_univariate(&coefficients, challenge)?;
        challenges.push(challenge);
        rounds.push(compressed);
        bind_top(&mut eq, challenge)?;
        bind_top(&mut a, challenge)?;
        bind_top(&mut b, challenge)?;
        bind_top(&mut d, challenge)?;
    }
    if a.len() != 1 || b.len() != 1 || d.len() != 1 {
        return Err(SumcheckError::WrongRoundCount);
    }
    Ok((SumcheckProof::new(rounds), challenges, [a[0], b[0], d[0]]))
}

/// Prove the quadratic inner Spartan sum-check `sum_x A(x) * B(x)`.
pub(super) fn prove_quadratic(
    initial_claim: Scalar,
    round_count: usize,
    a: &[Scalar],
    b: &[Scalar],
    transcript: &mut VegaTranscriptV1,
) -> Result<(SumcheckProof, Vec<Scalar>, [Scalar; 2]), SumcheckError> {
    let expected = table_size(round_count)?;
    if expected == 0 || a.len() != expected || b.len() != expected {
        return Err(SumcheckError::WrongRoundCount);
    }
    let mut a = a.to_vec();
    let mut b = b.to_vec();
    let mut claim = initial_claim;
    let mut rounds = Vec::with_capacity(round_count);
    let mut challenges = Vec::with_capacity(round_count);

    for _ in 0..round_count {
        let half = a.len() / 2;
        if half == 0 || b.len() != a.len() {
            return Err(SumcheckError::WrongRoundCount);
        }
        let mut evaluation_zero = Scalar::zero();
        let mut evaluation_one = Scalar::zero();
        let mut evaluation_two = Scalar::zero();
        for index in 0..half {
            let a_zero = a[index];
            let b_zero = b[index];
            let delta_a = a[half + index] - a_zero;
            let delta_b = b[half + index] - b_zero;
            evaluation_zero += a_zero * b_zero;
            evaluation_one += (a_zero + delta_a) * (b_zero + delta_b);
            let two = Scalar::from_u64(2);
            evaluation_two += (a_zero + two * delta_a) * (b_zero + two * delta_b);
        }
        if evaluation_zero + evaluation_one != claim {
            return Err(SumcheckError::InvalidClaim);
        }
        let coefficients =
            interpolate_quadratic([evaluation_zero, evaluation_one, evaluation_two])?;
        let compressed = CompressedUnivariate::new(vec![coefficients[0], coefficients[2]], 2)?;
        transcript.absorb_univariate(b"p", compressed.coefficients())?;
        let challenge = transcript.squeeze(b"c")?;
        claim = evaluate_univariate(&coefficients, challenge)?;
        challenges.push(challenge);
        rounds.push(compressed);
        bind_top(&mut a, challenge)?;
        bind_top(&mut b, challenge)?;
    }
    if a.len() != 1 || b.len() != 1 {
        return Err(SumcheckError::WrongRoundCount);
    }
    Ok((SumcheckProof::new(rounds), challenges, [a[0], b[0]]))
}

fn table_size(round_count: usize) -> Result<usize, SumcheckError> {
    Ok(evaluation_table_size(round_count)?)
}

fn bind_top(table: &mut Vec<Scalar>, challenge: Scalar) -> Result<(), SumcheckError> {
    if table.len() < 2 || !table.len().is_power_of_two() {
        return Err(SumcheckError::WrongRoundCount);
    }
    let half = table.len() / 2;
    let (lower, upper) = table.split_at_mut(half);
    for index in 0..half {
        lower[index] += challenge * (upper[index] - lower[index]);
    }
    table.truncate(half);
    Ok(())
}

fn interpolate_quadratic(evaluations: [Scalar; 3]) -> Result<[Scalar; 3], SumcheckError> {
    let two_inverse = Scalar::from_u64(2)
        .inverse()
        .map_err(|_| SumcheckError::InvalidClaim)?;
    let constant = evaluations[0];
    let quadratic =
        (evaluations[0] - Scalar::from_u64(2) * evaluations[1] + evaluations[2]) * two_inverse;
    let linear = evaluations[1] - constant - quadratic;
    Ok([constant, linear, quadratic])
}

fn interpolate_cubic(evaluations: [Scalar; 4]) -> Result<[Scalar; 4], SumcheckError> {
    let two_inverse = Scalar::from_u64(2)
        .inverse()
        .map_err(|_| SumcheckError::InvalidClaim)?;
    let six_inverse = Scalar::from_u64(6)
        .inverse()
        .map_err(|_| SumcheckError::InvalidClaim)?;
    let constant = evaluations[0];
    let cubic = (evaluations[3] - Scalar::from_u64(3) * evaluations[2]
        + Scalar::from_u64(3) * evaluations[1]
        - evaluations[0])
        * six_inverse;
    let quadratic = (evaluations[2] - Scalar::from_u64(2) * evaluations[1] + evaluations[0])
        * two_inverse
        - Scalar::from_u64(3) * cubic;
    let linear = evaluations[1] - constant - quadratic - cubic;
    Ok([constant, linear, quadratic, cubic])
}

impl SumcheckProof {
    pub(super) fn new(rounds: Vec<CompressedUnivariate>) -> Self {
        Self { rounds }
    }

    pub(super) fn verify(
        &self,
        initial_claim: Scalar,
        round_count: usize,
        degree: usize,
        transcript: &mut VegaTranscriptV1,
    ) -> Result<(Scalar, Vec<Scalar>), SumcheckError> {
        if self.rounds.len() != round_count {
            return Err(SumcheckError::WrongRoundCount);
        }
        if degree == 0 {
            return Err(SumcheckError::WrongDegree);
        }
        let mut claim = initial_claim;
        let mut challenges = Vec::with_capacity(round_count);
        for round in &self.rounds {
            if round.coefficients().len() != degree {
                return Err(SumcheckError::WrongDegree);
            }
            let polynomial = decompress_univariate(round.coefficients(), claim)?;
            transcript.absorb_univariate(b"p", round.coefficients())?;
            let challenge = transcript.squeeze(b"c")?;
            claim = evaluate_univariate(&polynomial, challenge)?;
            challenges.push(challenge);
        }
        Ok((claim, challenges))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::algebra::eq_evaluate;

    fn s(value: u64) -> Scalar {
        Scalar::from_u64(value)
    }

    fn multilinear_at(table: [Scalar; 4], x: Scalar, y: Scalar) -> Scalar {
        table[0] * (Scalar::one() - x) * (Scalar::one() - y)
            + table[1] * (Scalar::one() - x) * y
            + table[2] * x * (Scalar::one() - y)
            + table[3] * x * y
    }

    fn valid_two_round_proof() -> (SumcheckProof, Scalar) {
        let table = [s(2), s(3), s(5), s(7)];
        let claim = table
            .into_iter()
            .fold(Scalar::zero(), |sum, value| sum + value);
        let first_constant = table[0] + table[1];

        let mut prover_transcript = VegaTranscriptV1::new_neutron_nova();
        prover_transcript
            .absorb_univariate(b"p", &[first_constant])
            .expect("bounded");
        let first_challenge = prover_transcript.squeeze(b"c").expect("round");
        let second_constant = multilinear_at(table, first_challenge, Scalar::zero());
        (
            SumcheckProof::new(vec![
                CompressedUnivariate::new(vec![first_constant], 1).expect("linear"),
                CompressedUnivariate::new(vec![second_constant], 1).expect("linear"),
            ]),
            claim,
        )
    }

    fn bind_all(mut table: Vec<Scalar>, point: &[Scalar]) -> Scalar {
        for challenge in point {
            bind_top(&mut table, *challenge).expect("power-of-two table");
        }
        assert_eq!(table.len(), 1);
        table[0]
    }

    #[test]
    fn cubic_and_quadratic_provers_replay_against_the_verifier() {
        let tau = [s(13), s(17)];
        let a = [s(2), s(3), s(5), s(7)];
        let b = [s(11), s(13), s(17), s(19)];
        let d = [s(23), s(29), s(31), s(37)];
        let eq = eq_evals(&tau).expect("small table");
        let cubic_claim = eq
            .iter()
            .copied()
            .zip(a.iter().copied())
            .zip(b.iter().copied().zip(d.iter().copied()))
            .fold(Scalar::zero(), |sum, ((eq, a), (b, d))| {
                sum + eq * (a * b - d)
            });
        let mut prover_transcript = VegaTranscriptV1::new_neutron_nova();
        let (cubic, prover_challenges, claims) =
            prove_cubic_with_three_inputs(cubic_claim, &tau, &a, &b, &d, &mut prover_transcript)
                .expect("valid cubic proof");
        let mut verifier_transcript = VegaTranscriptV1::new_neutron_nova();
        let (final_claim, verifier_challenges) = cubic
            .verify(cubic_claim, 2, 3, &mut verifier_transcript)
            .expect("valid cubic verification");
        assert_eq!(prover_challenges, verifier_challenges);
        assert_eq!(
            claims,
            [
                bind_all(a.to_vec(), &verifier_challenges),
                bind_all(b.to_vec(), &verifier_challenges),
                bind_all(d.to_vec(), &verifier_challenges),
            ]
        );
        assert_eq!(
            final_claim,
            eq_evaluate(&tau, &verifier_challenges).expect("same dimension")
                * (claims[0] * claims[1] - claims[2])
        );

        let quadratic_claim = a
            .iter()
            .copied()
            .zip(b.iter().copied())
            .fold(Scalar::zero(), |sum, (a, b)| sum + a * b);
        let mut prover_transcript = VegaTranscriptV1::new_neutron_nova();
        let (quadratic, prover_challenges, claims) =
            prove_quadratic(quadratic_claim, 2, &a, &b, &mut prover_transcript)
                .expect("valid quadratic proof");
        let mut verifier_transcript = VegaTranscriptV1::new_neutron_nova();
        let (final_claim, verifier_challenges) = quadratic
            .verify(quadratic_claim, 2, 2, &mut verifier_transcript)
            .expect("valid quadratic verification");
        assert_eq!(prover_challenges, verifier_challenges);
        assert_eq!(
            claims,
            [
                bind_all(a.to_vec(), &verifier_challenges),
                bind_all(b.to_vec(), &verifier_challenges),
            ]
        );
        assert_eq!(final_claim, claims[0] * claims[1]);
    }

    #[test]
    fn sumcheck_provers_reject_false_claims_shapes_and_work_overflow() {
        let table = [s(1), s(2), s(3), s(4)];
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        assert_eq!(
            prove_quadratic(s(1), 2, &table, &table, &mut transcript),
            Err(SumcheckError::InvalidClaim)
        );
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        assert_eq!(
            prove_quadratic(s(30), 2, &table[..3], &table[..3], &mut transcript),
            Err(SumcheckError::WrongRoundCount)
        );
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        assert!(prove_quadratic(s(0), 21, &[], &[], &mut transcript).is_err());
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        assert_eq!(
            prove_cubic_with_three_inputs(
                s(0),
                &[s(1), s(2)],
                &table,
                &table,
                &table[..3],
                &mut transcript
            ),
            Err(SumcheckError::WrongRoundCount)
        );
    }

    #[test]
    fn compressed_sumcheck_replays_a_real_multilinear_claim() {
        let (proof, claim) = valid_two_round_proof();
        let mut verifier_transcript = VegaTranscriptV1::new_neutron_nova();
        let (final_claim, challenges) = proof
            .verify(claim, 2, 1, &mut verifier_transcript)
            .expect("valid sum-check");
        let table = [s(2), s(3), s(5), s(7)];
        assert_eq!(
            final_claim,
            multilinear_at(table, challenges[0], challenges[1])
        );
    }

    #[test]
    fn sumcheck_rejects_wrong_round_degree_claim_and_transcript_mutations() {
        let (proof, claim) = valid_two_round_proof();
        assert_eq!(
            proof.verify(claim, 3, 1, &mut VegaTranscriptV1::new_neutron_nova()),
            Err(SumcheckError::WrongRoundCount)
        );
        assert_eq!(
            proof.verify(claim, 2, 2, &mut VegaTranscriptV1::new_neutron_nova()),
            Err(SumcheckError::WrongDegree)
        );

        let mut mutated = proof.clone();
        mutated.rounds[0].coefficients_except_linear[0] += Scalar::one();
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        let (mutated_final, mutated_challenges) = mutated
            .verify(claim, 2, 1, &mut transcript)
            .expect("structurally valid but algebraically false");
        let table = [s(2), s(3), s(5), s(7)];
        assert_ne!(
            mutated_final,
            multilinear_at(table, mutated_challenges[0], mutated_challenges[1])
        );
    }
}
