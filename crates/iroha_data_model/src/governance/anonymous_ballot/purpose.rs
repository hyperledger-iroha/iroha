//! Why an anonymous-ballot attempt may exist (design §12.3).
//!
//! Every attempt of the anonymous definition family records its purpose at creation, and
//! the purpose travels into its boards and its certificate. A candidate-testing attempt
//! runs the real protocol on a network whose original signed genesis selected the
//! candidate; a qualified attempt runs under the owner's recorded qualification. The two
//! never mix: production enactment and the binding-ballot availability read by SCCP accept
//! only [`ParliamentBallotPurposeV1::Qualified`].
//!
//! A purpose is a recorded fact, not authority by itself. Core derives it from committed
//! State and this release ([`ParliamentBallotPurposeV1::select_v1`]); no instruction
//! payload carries a caller-chosen purpose.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::{Digest512, PARLIAMENT_BALLOT_QUALIFICATION_V1, ParliamentCandidateTestingPolicyV1};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};

/// Why a purpose is not one this release may record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ParliamentBallotPurposeErrorV1 {
    /// The candidate policy is not the supported V1 candidate selection.
    #[error("the candidate-testing policy is not the supported V1 candidate")]
    UnsupportedCandidate,
    /// The qualification digest is not the record this release names, or it names none.
    #[error("the qualification is not the record this release names")]
    Unqualified,
    /// A candidate-testing purpose was offered where production authority is required.
    #[error("a candidate-testing attempt carries no production authority")]
    CandidateTesting,
}

/// Why an anonymous-ballot attempt, its boards and its certificate exist.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(
    tag = "kind",
    content = "detail",
    rename_all = "snake_case",
    deny_unknown_fields
)]
#[norito_schema(name = "iroha_data_model::governance::anonymous_ballot::ParliamentBallotPurposeV1")]
pub enum ParliamentBallotPurposeV1 {
    /// Candidate testing under the policy the network's original signed genesis selected.
    /// It grants no qualification, SCCP or production enactment authority.
    #[codec(index = 0)]
    CandidateTesting(ParliamentCandidateTestingPolicyV1),
    /// Production under the owner's qualification record with this digest.
    #[codec(index = 1)]
    Qualified(Digest512),
}

impl ParliamentBallotPurposeV1 {
    /// The purpose of an attempt created now, from the release's qualification and the
    /// network's committed candidate-testing policy.
    ///
    /// A qualified release creates qualified attempts whatever the genesis selected. An
    /// unqualified release creates candidate attempts only where genesis selected the
    /// candidate, and otherwise none: `None` means no anonymous attempt may be created.
    #[must_use]
    pub fn select_v1(candidate: Option<&ParliamentCandidateTestingPolicyV1>) -> Option<Self> {
        Self::select_with(PARLIAMENT_BALLOT_QUALIFICATION_V1, candidate)
    }

    /// [`Self::select_v1`] for an explicit qualification, so the rule is testable while
    /// the release names none.
    fn select_with(
        qualification: Option<Digest512>,
        candidate: Option<&ParliamentCandidateTestingPolicyV1>,
    ) -> Option<Self> {
        match (qualification, candidate) {
            (Some(qualification), _) => Some(Self::Qualified(qualification)),
            (None, Some(policy)) if policy.validate().is_ok() => {
                Some(Self::CandidateTesting(*policy))
            }
            (None, _) => None,
        }
    }

    /// Whether this purpose carries production authority: the only purpose production
    /// enactment and the binding-ballot availability accept.
    #[must_use]
    pub const fn is_production(&self) -> bool {
        matches!(self, Self::Qualified(_))
    }

    /// Require production authority: the one gate of production enactment and of the
    /// binding-ballot availability SCCP reads. A candidate certificate never passes it,
    /// whatever its ballots decided.
    ///
    /// # Errors
    /// [`ParliamentBallotPurposeErrorV1::CandidateTesting`] for a candidate purpose, or
    /// [`ParliamentBallotPurposeErrorV1::Unqualified`] for a qualification this release
    /// does not name.
    pub fn require_production(&self) -> Result<(), ParliamentBallotPurposeErrorV1> {
        self.require_production_with(PARLIAMENT_BALLOT_QUALIFICATION_V1)
    }

    fn require_production_with(
        &self,
        qualification: Option<Digest512>,
    ) -> Result<(), ParliamentBallotPurposeErrorV1> {
        match self {
            Self::CandidateTesting(_) => Err(ParliamentBallotPurposeErrorV1::CandidateTesting),
            Self::Qualified(_) => self.validate_with(qualification),
        }
    }

    /// Check a recorded purpose against this release: the supported candidate, or exactly
    /// the qualification record the release names.
    ///
    /// # Errors
    /// [`ParliamentBallotPurposeErrorV1`] for another candidate or qualification.
    pub fn validate(&self) -> Result<(), ParliamentBallotPurposeErrorV1> {
        self.validate_with(PARLIAMENT_BALLOT_QUALIFICATION_V1)
    }

    fn validate_with(
        &self,
        qualification: Option<Digest512>,
    ) -> Result<(), ParliamentBallotPurposeErrorV1> {
        match self {
            Self::CandidateTesting(policy) => policy
                .validate()
                .map_err(|_| ParliamentBallotPurposeErrorV1::UnsupportedCandidate),
            Self::Qualified(digest) if qualification == Some(*digest) => Ok(()),
            Self::Qualified(_) => Err(ParliamentBallotPurposeErrorV1::Unqualified),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::anonymous_ballot::ProofProfileIdV1;

    const CANDIDATE: ParliamentCandidateTestingPolicyV1 =
        ParliamentCandidateTestingPolicyV1::CANDIDATE_V1;

    fn qualification(tag: u8) -> Digest512 {
        Digest512::new([tag; 64])
    }

    #[test]
    fn this_release_creates_candidate_attempts_only_where_genesis_selected_them() {
        // The release names no qualification record.
        assert_eq!(PARLIAMENT_BALLOT_QUALIFICATION_V1, None);
        assert_eq!(ParliamentBallotPurposeV1::select_v1(None), None);
        let selected = ParliamentBallotPurposeV1::select_v1(Some(&CANDIDATE))
            .expect("genesis selected the candidate");
        assert_eq!(
            selected,
            ParliamentBallotPurposeV1::CandidateTesting(CANDIDATE)
        );
        assert!(!selected.is_production());
        assert_eq!(selected.validate(), Ok(()));

        // A policy this release does not support selects nothing.
        let unsupported = ParliamentCandidateTestingPolicyV1 {
            version: 2,
            ..CANDIDATE
        };
        assert_eq!(
            ParliamentBallotPurposeV1::select_v1(Some(&unsupported)),
            None
        );
        assert_eq!(
            ParliamentBallotPurposeV1::CandidateTesting(unsupported).validate(),
            Err(ParliamentBallotPurposeErrorV1::UnsupportedCandidate)
        );
        let another_profile = ParliamentCandidateTestingPolicyV1 {
            profile: ProofProfileIdV1::new([0x55; ProofProfileIdV1::LENGTH]),
            ..CANDIDATE
        };
        assert_eq!(
            ParliamentBallotPurposeV1::select_v1(Some(&another_profile)),
            None
        );
    }

    #[test]
    fn a_qualified_release_creates_only_qualified_attempts() {
        let record = qualification(0x71);
        for candidate in [None, Some(&CANDIDATE)] {
            let selected = ParliamentBallotPurposeV1::select_with(Some(record), candidate)
                .expect("a qualified release always selects");
            assert_eq!(selected, ParliamentBallotPurposeV1::Qualified(record));
            assert!(selected.is_production());
            assert_eq!(selected.validate_with(Some(record)), Ok(()));
        }
    }

    #[test]
    fn a_qualified_purpose_needs_exactly_the_release_record() {
        let purpose = ParliamentBallotPurposeV1::Qualified(qualification(0x71));
        // This release names no record, so no recorded qualification is valid in it.
        assert_eq!(
            purpose.validate(),
            Err(ParliamentBallotPurposeErrorV1::Unqualified)
        );
        assert_eq!(
            purpose.validate_with(Some(qualification(0x72))),
            Err(ParliamentBallotPurposeErrorV1::Unqualified)
        );
        // A candidate purpose stays a candidate under a qualified release: it is valid
        // history and never production.
        let candidate = ParliamentBallotPurposeV1::CandidateTesting(CANDIDATE);
        assert_eq!(candidate.validate_with(Some(qualification(0x71))), Ok(()));
        assert!(!candidate.is_production());
    }

    #[test]
    fn only_the_release_qualification_carries_production_authority() {
        let record = qualification(0x71);
        let candidate = ParliamentBallotPurposeV1::CandidateTesting(CANDIDATE);
        let qualified = ParliamentBallotPurposeV1::Qualified(record);
        // A candidate is refused whether or not the release is qualified.
        for release in [None, Some(record)] {
            assert_eq!(
                candidate.require_production_with(release),
                Err(ParliamentBallotPurposeErrorV1::CandidateTesting)
            );
        }
        assert_eq!(qualified.require_production_with(Some(record)), Ok(()));
        assert_eq!(
            qualified.require_production_with(Some(qualification(0x72))),
            Err(ParliamentBallotPurposeErrorV1::Unqualified)
        );
        // This release names no record: nothing has production authority in it.
        assert_eq!(
            candidate.require_production(),
            Err(ParliamentBallotPurposeErrorV1::CandidateTesting)
        );
        assert_eq!(
            qualified.require_production(),
            Err(ParliamentBallotPurposeErrorV1::Unqualified)
        );
    }

    #[test]
    fn purposes_roundtrip_with_distinct_canonical_encodings() {
        let purposes = [
            ParliamentBallotPurposeV1::CandidateTesting(CANDIDATE),
            ParliamentBallotPurposeV1::Qualified(qualification(0x71)),
        ];
        let mut encodings = Vec::new();
        for purpose in purposes {
            let bytes = norito::encode_canonical(&purpose).expect("encode");
            assert_eq!(
                norito::decode_canonical::<ParliamentBallotPurposeV1>(&bytes).expect("decode"),
                purpose
            );
            let json = norito::json::to_json(&purpose).expect("json");
            assert_eq!(
                norito::json::from_str::<ParliamentBallotPurposeV1>(&json).expect("from json"),
                purpose
            );
            encodings.push(bytes);
        }
        assert_ne!(encodings[0], encodings[1]);
    }
}
