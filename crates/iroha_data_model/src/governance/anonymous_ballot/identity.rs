//! Identifiers of the anonymous Parliament definition family (design §3.3 and §12.3).
//!
//! The family derives its own identifiers. An attempt of the frozen baseline and an
//! anonymous attempt of the same proposal and retry sequence therefore never share an
//! attempt, election, request or assignment identifier, because every later identifier is
//! derived from the attempt's. A sealed body is identified through the 64-byte roster root
//! of the extended preimage `(assignments, R, S, profile_id)`, so the sealed credential
//! list is bound into the body instance and, through it, into the ballot scope.
//!
//! These are opaque 32-byte State identifiers: State authenticates them and refuses a
//! duplicate. The collision-sensitive bindings of the ballot (the scope, `R`, the roster
//! root, the policy hash) keep their 64-byte digests.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::Digest512;
use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize,
    governance::types::{
        BodyElectionAttemptId, BodyInstanceId, GovernanceAttemptId, GovernanceAttemptStatusV1,
        GovernanceStageV1, ProposalContentId, RiskTierV1,
    },
    governance_fingerprint::{
        ANONYMOUS_BODY_INSTANCE_ID_V1, ANONYMOUS_GOVERNANCE_ATTEMPT_ID_V1, fingerprint,
    },
};

#[derive(Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::governance::anonymous_ballot::AnonymousGovernanceAttemptIdPreimageV1"
)]
struct AnonymousGovernanceAttemptIdPreimageV1 {
    proposal_content_id: ProposalContentId,
    sequence: u32,
}

#[derive(Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::governance::anonymous_ballot::AnonymousBodyInstanceIdPreimageV1"
)]
struct AnonymousBodyInstanceIdPreimageV1 {
    election_attempt_id: BodyElectionAttemptId,
    roster_root: Digest512,
}

/// The only valid identifier of the anonymous attempt of `proposal_content_id` with retry
/// `sequence`. It differs from the baseline identifier of the same proposal and sequence.
#[must_use]
pub fn anonymous_governance_attempt_id_v1(
    proposal_content_id: ProposalContentId,
    sequence: u32,
) -> GovernanceAttemptId {
    GovernanceAttemptId::new(fingerprint(
        ANONYMOUS_GOVERNANCE_ATTEMPT_ID_V1,
        &AnonymousGovernanceAttemptIdPreimageV1 {
            proposal_content_id,
            sequence,
        },
    ))
}

/// The snapshot of one retryable attempt of the anonymous family.
///
/// It is the family's own record. The baseline snapshot (`GovernanceAttemptV1`) checks its
/// identifier against the baseline derivation, which no anonymous attempt satisfies; this
/// record checks the family's.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::governance::anonymous_ballot::AnonymousGovernanceAttemptV1"
)]
pub struct AnonymousGovernanceAttemptV1 {
    /// Identifier unique to this retry, derived by [`anonymous_governance_attempt_id_v1`].
    pub id: GovernanceAttemptId,
    /// Immutable proposal content shared by all retries.
    pub proposal_content_id: ProposalContentId,
    /// Zero-based retry sequence for the proposal content.
    pub sequence: u32,
    /// Upward-only risk tier, locked at the first Policy Jury sortition request.
    pub risk_tier: RiskTierV1,
    /// Current sequential stage.
    pub stage: GovernanceStageV1,
    /// Current status.
    pub status: GovernanceAttemptStatusV1,
}

impl AnonymousGovernanceAttemptV1 {
    /// The initial snapshot of retry `sequence` of `proposal_content_id`: active at
    /// qualification under a policy-derived risk tier.
    #[must_use]
    pub fn new_v1(
        proposal_content_id: ProposalContentId,
        sequence: u32,
        risk_tier: RiskTierV1,
    ) -> Self {
        Self {
            id: anonymous_governance_attempt_id_v1(proposal_content_id, sequence),
            proposal_content_id,
            sequence,
            risk_tier,
            stage: GovernanceStageV1::Qualification,
            status: GovernanceAttemptStatusV1::Active,
        }
    }

    /// Whether the stored identifier is the family's derivation for this retry.
    #[must_use]
    pub fn has_canonical_id(&self) -> bool {
        self.id == anonymous_governance_attempt_id_v1(self.proposal_content_id, self.sequence)
    }
}

/// The identifier of the body sealed by `election_attempt_id` with the 64-byte
/// `roster_root` ([`super::digests::anonymous_roster_root_v1`]).
#[must_use]
pub fn anonymous_body_instance_id_v1(
    election_attempt_id: BodyElectionAttemptId,
    roster_root: &Digest512,
) -> BodyInstanceId {
    BodyInstanceId::new(fingerprint(
        ANONYMOUS_BODY_INSTANCE_ID_V1,
        &AnonymousBodyInstanceIdPreimageV1 {
            election_attempt_id,
            roster_root: *roster_root,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::anonymous_ballot::{
        AnonymousParliamentRosterRootPreimageV1, AnonymousRosterBindingV1, ProofProfileIdV1,
        digests::anonymous_roster_root_v1, test_fixtures,
    };
    use crate::governance::types::ParliamentSeatAssignmentV1;

    fn proposal() -> ProposalContentId {
        ProposalContentId::new([0x01; 32])
    }

    fn election() -> BodyElectionAttemptId {
        BodyElectionAttemptId::new([0x22; 32])
    }

    #[test]
    fn an_attempt_snapshot_is_canonical_only_under_the_family_derivation() {
        let attempt = AnonymousGovernanceAttemptV1::new_v1(proposal(), 2, RiskTierV1::Standard);
        assert!(attempt.has_canonical_id());
        assert_eq!(
            attempt.id,
            anonymous_governance_attempt_id_v1(proposal(), 2)
        );
        assert_eq!(attempt.stage, GovernanceStageV1::Qualification);
        assert_eq!(attempt.status, GovernanceAttemptStatusV1::Active);

        // The baseline identifier of the same proposal and retry, and another retry's.
        let mut baseline = attempt;
        baseline.id = GovernanceAttemptId::derive_v1(proposal(), 2);
        assert!(!baseline.has_canonical_id());
        let mut other_retry = attempt;
        other_retry.sequence = 3;
        assert!(!other_retry.has_canonical_id());

        let bytes = norito::encode_canonical(&attempt).expect("encode");
        assert_eq!(
            norito::decode_canonical::<AnonymousGovernanceAttemptV1>(&bytes).expect("decode"),
            attempt
        );
        let json = norito::json::to_json(&attempt).expect("json");
        assert_eq!(
            norito::json::from_str::<AnonymousGovernanceAttemptV1>(&json).expect("from json"),
            attempt
        );
    }

    #[test]
    fn anonymous_and_baseline_attempts_never_share_an_identifier() {
        for sequence in 0..3 {
            let anonymous = anonymous_governance_attempt_id_v1(proposal(), sequence);
            assert_ne!(
                anonymous,
                GovernanceAttemptId::derive_v1(proposal(), sequence)
            );
            assert_eq!(
                anonymous,
                anonymous_governance_attempt_id_v1(proposal(), sequence)
            );
            assert_ne!(
                anonymous,
                anonymous_governance_attempt_id_v1(proposal(), sequence + 1)
            );
        }
        assert_ne!(
            anonymous_governance_attempt_id_v1(proposal(), 0),
            anonymous_governance_attempt_id_v1(ProposalContentId::new([0x02; 32]), 0)
        );
    }

    #[test]
    fn a_body_instance_is_bound_to_every_byte_of_its_roster_root() {
        let root = Digest512::new([0x0a; 64]);
        let id = anonymous_body_instance_id_v1(election(), &root);
        assert_eq!(id, anonymous_body_instance_id_v1(election(), &root));
        assert_ne!(
            id,
            anonymous_body_instance_id_v1(BodyElectionAttemptId::new([0x23; 32]), &root)
        );
        // A change in the half a 32-byte root would drop still changes the identifier.
        for byte in [0, 31, 32, 63] {
            let mut bytes = [0x0a; 64];
            bytes[byte] ^= 1;
            assert_ne!(
                id,
                anonymous_body_instance_id_v1(election(), &Digest512::new(bytes)),
                "byte {byte}"
            );
        }
        // The baseline derivation over a 32-byte root is another domain.
        assert_ne!(id, BodyInstanceId::derive_v1(election(), [0x0a; 32]));
    }

    #[test]
    fn the_sealed_list_reaches_the_body_instance_through_the_roster_root() {
        let assignments = (1..=3)
            .map(|tag| ParliamentSeatAssignmentV1 {
                assignment_id: test_fixtures::assignment(tag),
                member: test_fixtures::account(),
            })
            .collect::<Vec<_>>();
        let preimage = |list_digest: Digest512| AnonymousParliamentRosterRootPreimageV1 {
            election_attempt_id: election(),
            assignments: assignments.clone(),
            anonymous_ballot: Some(AnonymousRosterBindingV1 {
                list_digest,
                seats: 3,
                profile_id: ProofProfileIdV1::V1,
            }),
        };
        let body = |list_digest: Digest512| {
            anonymous_body_instance_id_v1(
                election(),
                &anonymous_roster_root_v1(&preimage(list_digest)).expect("roster root"),
            )
        };
        // The same seated assignments with another sealed list are another body.
        assert_ne!(
            body(Digest512::new([0x99; 64])),
            body(Digest512::new([0x98; 64]))
        );
    }
}
