//! Checked identities through the unchanged anonymous certificate validation rules.

use super::*;
use crate::governance::{
    anonymous_ballot::{
        checked_anonymous_body_instance_id_v1, checked_anonymous_governance_attempt_id_v1,
    },
    types::{
        AssignmentId, CheckedSortitionRequestErrorV1,
        checked_parliament_public_finding_endorsement_root_v1,
    },
};

/// A certificate rule rejection or the original identity encoder refusal.
#[derive(Debug, thiserror::Error)]
pub enum CheckedAnonymousGovernanceCertificateErrorV1 {
    /// The existing certificate rules rejected the supplied record.
    #[error(transparent)]
    Rejected(AnonymousGovernanceCertificateErrorV1),
    /// An original codec attempt failed, preserving its local resource classification.
    #[error(transparent)]
    Codec(#[from] norito::core::DecodeAttemptError),
    /// The ballot's original allocator or enclosing budget refused its owned closure frame.
    #[error("ballot closure result is unavailable: {0}")]
    ClosureUnavailable(norito::core::DecodeLocalRefusal),
}

impl From<AnonymousGovernanceCertificateErrorV1> for CheckedAnonymousGovernanceCertificateErrorV1 {
    fn from(error: AnonymousGovernanceCertificateErrorV1) -> Self {
        match error {
            AnonymousGovernanceCertificateErrorV1::Ballot(AnonymousBallotBindingErrorV1::Codec(
                crate::governance::anonymous_ballot::AnonymousBallotCodecErrorV1::DecodeUnavailable(original),
            )) => Self::ClosureUnavailable(original),
            error => Self::Rejected(error),
        }
    }
}

pub(super) trait Identities {
    type Error: From<AnonymousGovernanceCertificateErrorV1>;
    fn attempt(
        &self,
        content: ProposalContentId,
        sequence: u32,
    ) -> Result<GovernanceAttemptId, Self::Error>;
    fn election(
        &self,
        attempt: GovernanceAttemptId,
        body: ParliamentBody,
        sequence: u32,
    ) -> Result<BodyElectionAttemptId, Self::Error>;
    fn body(
        &self,
        election: BodyElectionAttemptId,
        roster: &Digest512,
    ) -> Result<BodyInstanceId, Self::Error>;
    fn request_valid(&self, request: &SortitionRequestV1) -> Result<bool, Self::Error>;
    fn finding(
        &self,
        attempt: GovernanceAttemptId,
        body: BodyInstanceId,
        root: [u8; 32],
        assignments: &[AssignmentId],
    ) -> Result<[u8; 32], Self::Error>;
}

pub(super) struct Installed;
impl Identities for Installed {
    type Error = AnonymousGovernanceCertificateErrorV1;
    fn attempt(
        &self,
        content: ProposalContentId,
        sequence: u32,
    ) -> Result<GovernanceAttemptId, Self::Error> {
        Ok(anonymous_governance_attempt_id_v1(content, sequence))
    }
    fn election(
        &self,
        attempt: GovernanceAttemptId,
        body: ParliamentBody,
        sequence: u32,
    ) -> Result<BodyElectionAttemptId, Self::Error> {
        Ok(BodyElectionAttemptId::derive_v1(attempt, body, sequence))
    }
    fn body(
        &self,
        election: BodyElectionAttemptId,
        roster: &Digest512,
    ) -> Result<BodyInstanceId, Self::Error> {
        Ok(anonymous_body_instance_id_v1(election, roster))
    }
    fn request_valid(&self, request: &SortitionRequestV1) -> Result<bool, Self::Error> {
        Ok(request.validate(None).is_ok())
    }
    fn finding(
        &self,
        attempt: GovernanceAttemptId,
        body: BodyInstanceId,
        root: [u8; 32],
        assignments: &[AssignmentId],
    ) -> Result<[u8; 32], Self::Error> {
        Ok(parliament_public_finding_endorsement_root_v1(
            attempt,
            body,
            root,
            assignments,
        ))
    }
}

struct Checked;
impl Identities for Checked {
    type Error = CheckedAnonymousGovernanceCertificateErrorV1;
    fn attempt(
        &self,
        content: ProposalContentId,
        sequence: u32,
    ) -> Result<GovernanceAttemptId, Self::Error> {
        Ok(checked_anonymous_governance_attempt_id_v1(
            content, sequence,
        )?)
    }
    fn election(
        &self,
        attempt: GovernanceAttemptId,
        body: ParliamentBody,
        sequence: u32,
    ) -> Result<BodyElectionAttemptId, Self::Error> {
        Ok(BodyElectionAttemptId::derive_checked_v1(
            attempt, body, sequence,
        )?)
    }
    fn body(
        &self,
        election: BodyElectionAttemptId,
        roster: &Digest512,
    ) -> Result<BodyInstanceId, Self::Error> {
        Ok(checked_anonymous_body_instance_id_v1(election, roster)?)
    }
    fn request_valid(&self, request: &SortitionRequestV1) -> Result<bool, Self::Error> {
        match request.validate_checked_v1(None) {
            Ok(()) => Ok(true),
            Err(CheckedSortitionRequestErrorV1::Rejected(_)) => Ok(false),
            Err(CheckedSortitionRequestErrorV1::Codec(error)) => Err(error.into()),
        }
    }
    fn finding(
        &self,
        attempt: GovernanceAttemptId,
        body: BodyInstanceId,
        root: [u8; 32],
        assignments: &[AssignmentId],
    ) -> Result<[u8; 32], Self::Error> {
        Ok(checked_parliament_public_finding_endorsement_root_v1(
            attempt,
            body,
            root,
            assignments,
        )?)
    }
}

impl AnonymousSealedBodyV1 {
    /// Apply the existing sealed-body rules with original checked identities.
    ///
    /// # Errors
    /// The original deterministic rule rejection or identity codec attempt.
    pub fn validate_checked_v1(
        &self,
        attempt: GovernanceAttemptId,
        policy: &FrozenParliamentAttemptPolicyV1,
    ) -> Result<(), CheckedAnonymousGovernanceCertificateErrorV1> {
        self.validate_using(attempt, policy, &Checked)
    }

    /// Apply the existing public finding rules without copying its assignment list.
    ///
    /// # Errors
    /// The original deterministic rule rejection or identity codec attempt.
    pub fn validate_public_finding_checked_v1(
        &self,
        attempt: GovernanceAttemptId,
        result: &AnonymousPublicFindingResultV1,
    ) -> Result<(), CheckedAnonymousGovernanceCertificateErrorV1> {
        self.validate_public_finding_using(attempt, result, &Checked)
    }
}

impl AnonymousParliamentBodyBindingV1 {
    /// Apply the existing body-result rules with original checked identities.
    ///
    /// # Errors
    /// The original deterministic rule rejection or identity codec attempt.
    pub fn validate_checked_v1(
        &self,
        content: ProposalContentId,
        attempt: GovernanceAttemptId,
        risk: RiskTierV1,
        policy: &FrozenParliamentAttemptPolicyV1,
    ) -> Result<(), CheckedAnonymousGovernanceCertificateErrorV1> {
        self.validate_using(content, attempt, risk, policy, &Checked)
    }
}

impl AnonymousGovernanceCertificateV1 {
    /// Apply the existing certificate rules with original checked identities throughout.
    ///
    /// # Errors
    /// The original deterministic rule rejection or identity codec attempt.
    pub fn validate_checked_v1(&self) -> Result<(), CheckedAnonymousGovernanceCertificateErrorV1> {
        self.validate_using(&Checked)
    }
}
