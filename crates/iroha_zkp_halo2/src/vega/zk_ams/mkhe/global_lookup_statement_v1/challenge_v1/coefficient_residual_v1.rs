//! Move-only post-batching residual-commitment boundary.
//!
//! The three commitments are transcript material, but the vector-arithmetic
//! proof codec is still absent.  Consequently the production seal is
//! uninhabited and this child cannot authorize a proof or release.
use super::*;
use core::convert::Infallible;
pub(super) const COEFFICIENT_DIMENSIONS_V1: usize = 14;
pub(super) const TAU_FIRST_ORDINAL_V1: u32 = 33;
pub(super) const TAU_LAST_ORDINAL_V1: u32 = 46;
pub(super) const KAPPA_ORDINAL_V1: u32 = 47;
pub(super) const DELTA_ORDINAL_V1: u32 = 48;
pub(super) const COEFFICIENT_CHALLENGE_LANGUAGE_V1: &[u8] = b"after-mu:tau[0..13]=ordinals33..46-nonzero;kappa=47-outside-{0,1};delta=48-outside-{0,1};all-are-Fiat-Shamir-only-and-serialize-zero-proof-bytes";
pub(super) const COEFFICIENT_RESIDUAL_COMMITMENT_LANGUAGE_V1: &[u8] = b"after-delta-before-gtilde0:for-statement-in-(3,5,8)-validate-canonical-nonidentity-point33-then-absorb-frame(coefficient-residual-coordinate,statement_be_u16)-then-absorb-frame(coefficient-residual-commitment,point33);each-point-is-one-blinded-commitment-to-the-exact-length-2^14-vector-q_s[v]-in-Boolean-coordinate-order;one-required-uninstantiated-vector-arithmetic-proof-binds-every-entry-to-the-frozen-q_3/q_5/q_8-Boolean-formula-with-canonical-owner/residual-order;exactly3-points=99-wire-bytes;production-transition-seal=uninhabited;no-opaque-digest-bypass";
pub(super) const COEFFICIENT_GATE_LANGUAGE_V1: &[u8] = b"statements=0..13;Q_s=MLE(q_s);A_s=Q_s(r_s)-and-opens-the-same-framed-q_s-commitment-for-s-in(3,5,8)-or-verifier-derived-linear-aggregate-for-other-s-used-by-the-sumcheck;B_s=eq(tau,r_s)*A_s;Z_s=mask-terminal_s;gate0:B_s-eq(tau,r_s)*A_s=0;gate1:Cfinal_s-B_s-Z_s=0";
const FRAME_COEFFICIENT_RESIDUAL_COORDINATE_V1: &[u8] = b"coefficient-residual-coordinate";
const FRAME_COEFFICIENT_RESIDUAL_COMMITMENT_V1: &[u8] = b"coefficient-residual-commitment";
pub(super) struct CoefficientResidualCommitmentStageV1;
const _: () = {
    assert!(TAU_LAST_ORDINAL_V1 - TAU_FIRST_ORDINAL_V1 + 1 == COEFFICIENT_DIMENSIONS_V1 as u32);
    assert!(TAU_FIRST_ORDINAL_V1 == MU_ORDINAL_V1 + 1);
    assert!(KAPPA_ORDINAL_V1 == TAU_LAST_ORDINAL_V1 + 1);
    assert!(DELTA_ORDINAL_V1 == KAPPA_ORDINAL_V1 + 1);
    assert!(POST_BATCH_RESIDUAL_VECTOR_LENGTH_V1 == 1 << COEFFICIENT_DIMENSIONS_V1);
};
pub(super) fn coefficient_challenge_coordinate_v1(ordinal: u32) -> Option<ChallengeCoordinateV1> {
    let (purpose, predicate) = match ordinal {
        TAU_FIRST_ORDINAL_V1..=TAU_LAST_ORDINAL_V1 => (
            ChallengePurposeV1::scoped_v1(
                b"coefficient-tau-coordinate",
                NO_COORDINATE_V1 as usize,
                (ordinal - TAU_FIRST_ORDINAL_V1) as usize,
            ),
            ChallengePredicateV1::Nonzero,
        ),
        KAPPA_ORDINAL_V1 => (
            ChallengePurposeV1::unscoped_v1(b"coefficient-kappa"),
            ChallengePredicateV1::OutsideBooleanSet,
        ),
        DELTA_ORDINAL_V1 => (
            ChallengePurposeV1::unscoped_v1(b"coefficient-delta"),
            ChallengePredicateV1::OutsideBooleanSet,
        ),
        _ => return None,
    };
    Some(ChallengeCoordinateV1 {
        ordinal,
        purpose,
        predicate,
    })
}
pub(super) fn challenge_is_outside_boolean_set_v1(challenge: Scalar) -> bool {
    !challenge.is_zero() && challenge != Scalar::one()
}
pub(super) fn derive_coefficient_challenges_v1(
    state: &mut Keccak256,
    ordinal: &mut u32,
    challenges: &mut GlobalLookupChallengesV1,
) -> Result<(), GlobalLookupErrorV1> {
    for coordinate in 0..COEFFICIENT_DIMENSIONS_V1 {
        if *ordinal != TAU_FIRST_ORDINAL_V1 + coordinate as u32 {
            return Err(GlobalLookupErrorV1::Order);
        }
        challenges.tau[coordinate] = derive_coordinate_challenge_v1(state, ordinal)?;
    }
    for (expected, destination) in [
        (KAPPA_ORDINAL_V1, &mut challenges.kappa),
        (DELTA_ORDINAL_V1, &mut challenges.delta),
    ] {
        if *ordinal != expected {
            return Err(GlobalLookupErrorV1::Order);
        }
        *destination = derive_coordinate_challenge_v1(state, ordinal)?;
    }
    Ok(())
}
pub(super) enum CoefficientResidualCommitmentSealV1 {
    Production {
        blinded_residual_commitments: Infallible,
        vector_arithmetic_proofs: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
impl GlobalLookupTranscriptV1<CoefficientResidualCommitmentStageV1> {
    pub(super) fn absorb_coefficient_residual_commitments_v1(
        mut self,
        commitments: [[u8; 33]; REQUIRED_POST_BATCH_RESIDUAL_COMMITMENTS_V1],
        _seal: CoefficientResidualCommitmentSealV1,
    ) -> Result<GlobalLookupTranscriptV1<SumcheckStageV1>, GlobalLookupErrorV1> {
        if self.challenge_ordinal != FIRST_SUMCHECK_ORDINAL_V1 {
            return Err(GlobalLookupErrorV1::Order);
        }
        for (statement, commitment) in POST_BATCH_RESIDUAL_STATEMENTS_V1
            .into_iter()
            .zip(commitments)
        {
            validate_endpoint_v1(&commitment)?;
            absorb_frame_v1(
                &mut self.state,
                FRAME_COEFFICIENT_RESIDUAL_COORDINATE_V1,
                &(statement as u16).to_be_bytes(),
            )?;
            absorb_frame_v1(
                &mut self.state,
                FRAME_COEFFICIENT_RESIDUAL_COMMITMENT_V1,
                &commitment,
            )?;
        }
        Ok(self.transition_v1())
    }
}
pub(super) fn hash_coefficient_manifest_suffix_v1(hash: &mut Keccak256) {
    for ordinal in TAU_FIRST_ORDINAL_V1..=DELTA_ORDINAL_V1 {
        hash_manifest_challenge_v1(hash, ordinal);
    }
    for _ in POST_BATCH_RESIDUAL_STATEMENTS_V1 {
        hash_manifest_frame_v1(hash, FRAME_COEFFICIENT_RESIDUAL_COORDINATE_V1);
        hash_manifest_frame_v1(hash, FRAME_COEFFICIENT_RESIDUAL_COMMITMENT_V1);
    }
}
#[rustfmt::skip]
pub(super) fn coefficient_gate_residuals_v1(aggregate: Scalar, equality: Scalar, weighted: Scalar, mask_terminal: Scalar, final_claim: Scalar) -> [Scalar; 2] {
    [weighted - equality * aggregate, final_claim - weighted - mask_terminal]
}
#[cfg(test)]
#[path = "coefficient_residual_v1_tests.rs"]
mod tests;
