//! Fused transcript/oracle bridge for the bounded global-lookup suffix.
//!
//! The only production handoff type has no production minter. Once that
//! prefix exists, this move-only session forces each oracle message through
//! the exact shared transcript before the resulting challenge can fold A, U,
//! or M. No scalar, challenge, context, or storage owner is returned.
use super::super::external_sumcheck_storage_v1::{
    EvaluatedGlobalRoundV1, FoldSinkSealV1, GlobalCubicCompleteV1, GlobalCubicOracleV1,
    GlobalCubicPrefixReadyV1, MOracleErrorV1, OracleTransitionV1, begin_global_cubic_oracle_v1,
};
use super::*;
const GLOBAL_MESSAGE_OFFSET_V1: usize = 205;
const EXTERNAL_FIRST_ROUND_V1: usize = 3;
const EXTERNAL_LAST_ROUND_V1: usize = 28;
const HANDOFF_NEXT_SUMCHECK_V1: usize = 208;
const HANDOFF_CHALLENGE_ORDINAL_V1: u32 = 257;
const _: () = {
    assert!(GLOBAL_MESSAGE_OFFSET_V1 + EXTERNAL_FIRST_ROUND_V1 == HANDOFF_NEXT_SUMCHECK_V1);
    assert!(
        FIRST_SUMCHECK_ORDINAL_V1 + HANDOFF_NEXT_SUMCHECK_V1 as u32 == HANDOFF_CHALLENGE_ORDINAL_V1
    );
    assert!(GLOBAL_MESSAGE_OFFSET_V1 + EXTERNAL_LAST_ROUND_V1 + 1 == REQUIRED_CUBIC_MESSAGES_V1);
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GlobalLookupExternalSumcheckErrorV1 {
    Transcript(GlobalLookupErrorV1),
    Oracle(MOracleErrorV1),
}
#[must_use = "dropping this session closes the transcript and authenticated oracle"]
pub(super) struct GlobalLookupExternalSumcheckSessionV1 {
    live: Option<(
        GlobalLookupTranscriptV1<SumcheckStageV1>,
        GlobalCubicOracleV1,
    )>,
}
#[must_use = "the public message and move-only transition must be consumed together"]
pub(super) enum GlobalLookupExternalSumcheckTransitionV1 {
    Continue {
        message: [u8; CUBIC_MESSAGE_BYTES_V1],
        session: GlobalLookupExternalSumcheckSessionV1,
    },
    Complete {
        message: [u8; CUBIC_MESSAGE_BYTES_V1],
        transcript: GlobalLookupTranscriptV1<EndpointStageV1>,
        oracle: GlobalCubicCompleteV1,
    },
}
impl GlobalLookupExternalSumcheckTransitionV1 {
    pub(super) fn message_v1(&self) -> &[u8; CUBIC_MESSAGE_BYTES_V1] {
        match self {
            Self::Continue { message, .. } | Self::Complete { message, .. } => message,
        }
    }
}
impl GlobalLookupExternalSumcheckSessionV1 {
    pub(super) fn begin_v1(
        transcript: GlobalLookupTranscriptV1<SumcheckStageV1>,
        prefix: GlobalCubicPrefixReadyV1,
    ) -> Result<Self, GlobalLookupExternalSumcheckErrorV1> {
        if transcript.next_sumcheck != HANDOFF_NEXT_SUMCHECK_V1
            || transcript.challenge_ordinal != HANDOFF_CHALLENGE_ORDINAL_V1
        {
            return Err(GlobalLookupExternalSumcheckErrorV1::Transcript(
                GlobalLookupErrorV1::Order,
            ));
        }
        let oracle = begin_global_cubic_oracle_v1(prefix)
            .map_err(GlobalLookupExternalSumcheckErrorV1::Oracle)?;
        if usize::from(oracle.next_round_v1()) != EXTERNAL_FIRST_ROUND_V1 {
            return Err(GlobalLookupExternalSumcheckErrorV1::Transcript(
                GlobalLookupErrorV1::Order,
            ));
        }
        validate_alignment_v1(&transcript, &oracle)?;
        Ok(Self {
            live: Some((transcript, oracle)),
        })
    }
    #[cfg(test)]
    fn from_aligned_test_only_v1(
        transcript: GlobalLookupTranscriptV1<SumcheckStageV1>,
        prefix: GlobalCubicPrefixReadyV1,
    ) -> Result<Self, GlobalLookupExternalSumcheckErrorV1> {
        let oracle = begin_global_cubic_oracle_v1(prefix)
            .map_err(GlobalLookupExternalSumcheckErrorV1::Oracle)?;
        validate_alignment_v1(&transcript, &oracle)?;
        Ok(Self {
            live: Some((transcript, oracle)),
        })
    }
    pub(super) fn advance_v1(
        mut self,
        sink: FoldSinkSealV1,
    ) -> Result<GlobalLookupExternalSumcheckTransitionV1, GlobalLookupExternalSumcheckErrorV1> {
        let (transcript, oracle) =
            self.live
                .take()
                .ok_or(GlobalLookupExternalSumcheckErrorV1::Transcript(
                    GlobalLookupErrorV1::Order,
                ))?;
        validate_alignment_v1(&transcript, &oracle)?;
        let round = usize::from(oracle.next_round_v1());
        if !(EXTERNAL_FIRST_ROUND_V1..=EXTERNAL_LAST_ROUND_V1).contains(&round) {
            return Err(GlobalLookupExternalSumcheckErrorV1::Transcript(
                GlobalLookupErrorV1::Order,
            ));
        }
        let message_ordinal = GLOBAL_MESSAGE_OFFSET_V1.checked_add(round).ok_or(
            GlobalLookupExternalSumcheckErrorV1::Transcript(GlobalLookupErrorV1::Arithmetic),
        )?;
        let evaluated: EvaluatedGlobalRoundV1 = oracle
            .evaluate_next_v1()
            .map_err(GlobalLookupExternalSumcheckErrorV1::Oracle)?;
        let message = *evaluated.message_v1();
        let transcript = transcript
            .absorb_gtilde_v1(message_ordinal, message)
            .map_err(GlobalLookupExternalSumcheckErrorV1::Transcript)?;
        let challenge = transcript.challenges.sumcheck[message_ordinal];
        let transition = evaluated
            .fold_with_raw_challenge_v1(challenge, sink)
            .map_err(GlobalLookupExternalSumcheckErrorV1::Oracle)?;
        match transition {
            OracleTransitionV1::Continue(oracle) if round < EXTERNAL_LAST_ROUND_V1 => {
                validate_alignment_v1(&transcript, &oracle)?;
                Ok(GlobalLookupExternalSumcheckTransitionV1::Continue {
                    message,
                    session: Self {
                        live: Some((transcript, oracle)),
                    },
                })
            }
            OracleTransitionV1::Complete(oracle) if round == EXTERNAL_LAST_ROUND_V1 => {
                let transcript = transcript
                    .finish_sumcheck_v1()
                    .map_err(GlobalLookupExternalSumcheckErrorV1::Transcript)?;
                Ok(GlobalLookupExternalSumcheckTransitionV1::Complete {
                    message,
                    transcript,
                    oracle,
                })
            }
            OracleTransitionV1::Continue(_) | OracleTransitionV1::Complete(_) => Err(
                GlobalLookupExternalSumcheckErrorV1::Transcript(GlobalLookupErrorV1::Order),
            ),
        }
    }
}
fn validate_alignment_v1(
    transcript: &GlobalLookupTranscriptV1<SumcheckStageV1>,
    oracle: &GlobalCubicOracleV1,
) -> Result<(), GlobalLookupExternalSumcheckErrorV1> {
    let round = usize::from(oracle.next_round_v1());
    let expected_next = GLOBAL_MESSAGE_OFFSET_V1.checked_add(round).ok_or(
        GlobalLookupExternalSumcheckErrorV1::Transcript(GlobalLookupErrorV1::Arithmetic),
    )?;
    if !(EXTERNAL_FIRST_ROUND_V1..=EXTERNAL_LAST_ROUND_V1).contains(&round)
        || transcript.next_sumcheck != expected_next
        || transcript.challenge_ordinal != FIRST_SUMCHECK_ORDINAL_V1 + expected_next as u32
    {
        return Err(GlobalLookupExternalSumcheckErrorV1::Transcript(
            GlobalLookupErrorV1::Order,
        ));
    }
    let challenges = &transcript.challenges;
    if !oracle.matches_transcript_v1(
        &transcript.bound_context_digest,
        &challenges.z,
        &challenges.rho,
        &challenges.alpha,
        &challenges.lambda,
        &challenges.mu,
        &challenges.sumcheck[GLOBAL_MESSAGE_OFFSET_V1..expected_next],
    ) {
        return Err(GlobalLookupExternalSumcheckErrorV1::Transcript(
            GlobalLookupErrorV1::Context,
        ));
    }
    Ok(())
}
#[cfg(test)]
#[path = "global_lookup_external_sumcheck_v1_tests.rs"]
mod tests;
