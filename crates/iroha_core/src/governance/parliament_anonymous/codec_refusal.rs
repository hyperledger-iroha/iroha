//! Retain original codec custody through credential, seal and board error wrappers.

use super::*;
use iroha_data_model::governance::anonymous_ballot::{
    AnonymousBallotCorpusErrorV1, BallotContextErrorV1, ParliamentBallotBoardRecordErrorV1,
};
use iroha_parliament_ballot::{
    context::ContextErrorV1, corpus::CorpusErrorV1, inputs::ProofInputErrorV1,
    registration::RegistrationErrorV1,
};
use norito::core::DecodeLocalRefusal;

fn codec(error: &AnonymousBallotCodecErrorV1) -> Option<&DecodeLocalRefusal> {
    match error {
        AnonymousBallotCodecErrorV1::DecodeUnavailable(original) => Some(original),
        _ => None,
    }
}

fn structure(error: &BallotContextErrorV1) -> Option<&DecodeLocalRefusal> {
    match error {
        BallotContextErrorV1::Codec(error) => codec(error),
        _ => None,
    }
}

fn input(error: &ProofInputErrorV1) -> Option<&DecodeLocalRefusal> {
    match error {
        ProofInputErrorV1::Codec(error) => codec(error),
        ProofInputErrorV1::Structure(error) => structure(error),
        _ => None,
    }
}

fn context(error: &ContextErrorV1) -> Option<&DecodeLocalRefusal> {
    match error {
        ContextErrorV1::Inputs(error) => input(error),
        ContextErrorV1::Structure(error) => structure(error),
        _ => None,
    }
}

impl AnonymousParliamentErrorV1 {
    /// Borrow the original local codec refusal through the exact typed lifecycle wrappers.
    /// Semantic errors and unrelated verifier failures never acquire codec authority here.
    pub(crate) fn original_codec_refusal(&self) -> Option<&DecodeLocalRefusal> {
        match self {
            Self::ProposalFrame(error) => codec(error),
            Self::Credential(error) => match error.as_ref() {
                RegistrationLedgerErrorV1::Context(error) => context(error),
                RegistrationLedgerErrorV1::Registration(RegistrationErrorV1::Inputs(error)) => {
                    input(error)
                }
                _ => None,
            },
            Self::Seal(error) => match error.as_ref() {
                SealErrorV1::Context(error) => context(error),
                _ => None,
            },
            Self::Board(error) => match error.as_ref() {
                BoardErrorV1::Context(error) => context(error),
                BoardErrorV1::Record(ParliamentBallotBoardRecordErrorV1::Codec(error)) => {
                    codec(error)
                }
                BoardErrorV1::Record(ParliamentBallotBoardRecordErrorV1::Context(error)) => {
                    structure(error)
                }
                BoardErrorV1::Corpus(CorpusErrorV1::Codec(error)) => codec(error),
                BoardErrorV1::Corpus(CorpusErrorV1::Entry(
                    AnonymousBallotCorpusErrorV1::Codec(error),
                )) => codec(error),
                _ => None,
            },
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_wrappers_retain_the_same_original_codec_refusal() {
        let payload = std::collections::HashMap::from([(1_u64, 2_u64)]);
        let error = norito::core::with_decode_limits_scope(
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 64),
            || iroha_data_model::governance::checked_fingerprint(b"test.identity", &payload),
        )
        .unwrap_err();
        let original = error.local_refusal().unwrap();
        let codec = AnonymousBallotCodecErrorV1::from_attempt(error);
        let structure = BallotContextErrorV1::Codec(codec.clone());
        let input = ProofInputErrorV1::Codec(codec.clone());
        let errors = [
            AnonymousParliamentErrorV1::ProposalFrame(codec.clone()),
            AnonymousParliamentErrorV1::Credential(Box::new(RegistrationLedgerErrorV1::Context(
                ContextErrorV1::Structure(structure.clone()),
            ))),
            AnonymousParliamentErrorV1::Credential(Box::new(
                RegistrationLedgerErrorV1::Registration(RegistrationErrorV1::Inputs(input.clone())),
            )),
            AnonymousParliamentErrorV1::Seal(Box::new(SealErrorV1::Context(
                ContextErrorV1::Structure(structure.clone()),
            ))),
            AnonymousParliamentErrorV1::Seal(Box::new(SealErrorV1::Context(
                ContextErrorV1::Inputs(input),
            ))),
            AnonymousParliamentErrorV1::Board(Box::new(BoardErrorV1::Record(
                ParliamentBallotBoardRecordErrorV1::Context(structure),
            ))),
            AnonymousParliamentErrorV1::Board(Box::new(BoardErrorV1::Record(
                ParliamentBallotBoardRecordErrorV1::Codec(codec.clone()),
            ))),
            AnonymousParliamentErrorV1::Board(Box::new(BoardErrorV1::Corpus(
                CorpusErrorV1::Codec(codec.clone()),
            ))),
            AnonymousParliamentErrorV1::Board(Box::new(BoardErrorV1::Corpus(
                CorpusErrorV1::Entry(AnonymousBallotCorpusErrorV1::Codec(codec)),
            ))),
        ];
        for error in errors {
            assert_eq!(error.original_codec_refusal(), Some(&original));
            assert!(
                error.is_local(),
                "a planner must not record this as a refused item"
            );
        }
        for error in [
            AnonymousParliamentErrorV1::ProposalFrame(AnonymousBallotCodecErrorV1::Codec(
                "invalid encoding".into(),
            )),
            AnonymousParliamentErrorV1::Seal(Box::new(SealErrorV1::MissingCredential)),
        ] {
            assert!(error.original_codec_refusal().is_none());
            assert!(!error.is_local());
        }
    }
}
