//! Canonical compressed sum-check verifier used by Vega Spartan.

use thiserror::Error;

use super::{
    VegaT256ScalarV1 as Scalar, VegaTranscriptError, VegaTranscriptV1,
    algebra::{AlgebraError, decompress_univariate, evaluate_univariate},
};

/// Failure while replaying a Vega sum-check proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(super) enum SumcheckError {
    #[error("Vega sum-check has the wrong number of rounds")]
    WrongRoundCount,
    #[error("Vega sum-check round has the wrong polynomial degree")]
    WrongDegree,
    #[error(transparent)]
    Algebra(#[from] AlgebraError),
    #[error(transparent)]
    Transcript(#[from] VegaTranscriptError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CompressedUnivariate {
    coefficients_except_linear: Vec<Scalar>,
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
    rounds: Vec<CompressedUnivariate>,
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
