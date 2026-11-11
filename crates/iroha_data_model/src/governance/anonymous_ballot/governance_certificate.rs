//! The governance certificate of an anonymous Parliament attempt (design §3.8 and §12.3).
//!
//! A certificate is produced only after every attempt-level prerequisite is met: every
//! required public finding, the Policy Jury approval and, after a narrow approval, the
//! approval of a disjoint Confirmation Jury. Each binding body contributes the closure
//! record of its one ballot ([`AnonymousBallotCertificateBindingV1`]); no timed-OVN or TLE
//! field exists.
//!
//! The certificate records **why its attempt may exist**
//! ([`ParliamentBallotPurposeV1`]) and the policy snapshot the attempt froze at admission.
//! [`AnonymousGovernanceCertificateV1::require_production`] is the one gate of production
//! enactment: a candidate-testing certificate is a complete, checkable record that carries
//! no production authority.
//!
//! Certification and enactment are not stored. The final body result is a ballot closure,
//! so the certificate is final at that closure's anchor, and enactment is due once the
//! governance clock reaches that anchor's time plus the frozen enactment delay.

use std::collections::BTreeSet;

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::{
    AnonymousBallotBindingErrorV1, AnonymousBallotCertificateBindingV1, BallotClosureRecordV1,
    BodyDecisionModeV1, Digest512, FrozenBallotPolicyErrorV1, FrozenParliamentAttemptPolicyV1,
    ParliamentBallotPurposeErrorV1, ParliamentBallotPurposeV1, anonymous_body_instance_id_v1,
    anonymous_governance_attempt_id_v1,
};
use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize,
    governance::{
        GovernanceAnchorV1,
        types::{
            BeaconPulseId, BodyElectionAttemptId, BodyInstanceId, GovernanceAttemptId,
            GovernanceCertificateId, GovernanceExpectedHeadV1, ParliamentBody,
            ParliamentPublicFindingCertificateBindingV1, ProposalContentId, RiskTierV1,
            SortitionRequestV1, parliament_public_finding_endorsement_root_v1,
            parliament_quorum_seats_v1,
        },
    },
    governance_fingerprint::{
        ANONYMOUS_GOVERNANCE_CERTIFICATE_ID_V1, ANONYMOUS_PARLIAMENT_EXECUTION_FAILURE_ROOT_V1,
        fingerprint,
    },
};

/// Why an anonymous governance certificate is not valid.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AnonymousGovernanceCertificateErrorV1 {
    /// The recorded purpose is not one this release recognizes.
    #[error(transparent)]
    Purpose(#[from] ParliamentBallotPurposeErrorV1),
    /// The frozen policy snapshot is not a valid V1 policy.
    #[error(transparent)]
    Policy(#[from] FrozenBallotPolicyErrorV1),
    /// A ballot binding fails its closure record, its policy or its approval.
    #[error(transparent)]
    Ballot(#[from] AnonymousBallotBindingErrorV1),
    /// An identifier or commitment is the inert all-zero value.
    #[error("certificate bindings must be non-zero")]
    ZeroBinding,
    /// An identifier is not derived from its complete preimage.
    #[error("the certificate contains a noncanonical identifier")]
    NonCanonicalIdentifier,
    /// A retry sequence exceeds the frozen limit.
    #[error("the certificate contains an over-limit retry sequence")]
    RetryLimitExceeded,
    /// The certificate binds no body.
    #[error("the certificate has no body bindings")]
    EmptyBodyBindings,
    /// The bodies are not in strict canonical order.
    #[error("the body bindings are not in canonical order")]
    NonCanonicalBodyOrder,
    /// A bound body is not in the frozen pipeline.
    #[error("{body:?} is not a body of the frozen pipeline")]
    UnrequiredBody {
        /// The body.
        body: ParliamentBody,
    },
    /// A required body has no binding.
    #[error("the required body {body:?} has no result")]
    MissingBody {
        /// The body.
        body: ParliamentBody,
    },
    /// A body's identifiers disagree with its immutable sortition request.
    #[error("a sortition request binding is inconsistent")]
    SortitionRequestMismatch,
    /// A body claims no seat, or more seats than its target or its candidate snapshot.
    #[error("the certificate contains an impossible sealed-seat count")]
    InvalidSeatCount,
    /// Two bodies reuse an election, an instance or a request.
    #[error("the certificate reuses an attempt-local binding")]
    DuplicateBinding,
    /// A binding jury carries a public finding, or a public body a ballot.
    #[error("the result of {body:?} is not of its decision mode")]
    ResultKind {
        /// The body.
        body: ParliamentBody,
    },
    /// A public finding's endorsements, quorum or root are invalid.
    #[error("a public-finding endorsement binding is invalid")]
    InvalidPublicFinding,
    /// A ballot's closure record names another proposal, attempt, election, body or seat
    /// count than its binding, opened before its body was drawn, or disagrees with another
    /// jury's about the network or the proposal.
    #[error("a closure record's {field} is not its certificate binding's")]
    BallotContext {
        /// The disagreeing field.
        field: &'static str,
    },
    /// A narrow Policy Jury approval lacks its fresh Confirmation Jury result, or another
    /// approval carries one.
    #[error("Confirmation Jury presence or sortition binding is invalid")]
    ConfirmationJuryMismatch,
    /// A result is not after its sortition pulse, or after the certifying closure.
    #[error("the certificate's result chronology is invalid")]
    InvalidLifecycle,
    /// The compare-and-set head has an inert subject or head commitment.
    #[error("the certificate's compare-and-set head is invalid")]
    InvalidExpectedHead,
}

/// The quorum-endorsed finding of a public nonbinding body.
#[derive(
    Clone,
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
    name = "iroha_data_model::governance::anonymous_ballot::AnonymousPublicFindingResultV1"
)]
pub struct AnonymousPublicFindingResultV1 {
    /// Root of the public evidence, deliberation and dissent record.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub result_root: [u8; 32],
    /// The account-signed endorsements that reached quorum on that root.
    pub finding: ParliamentPublicFindingCertificateBindingV1,
}

/// The immutable result of one body, of the kind its decision mode requires.
#[derive(
    Clone,
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
#[norito(tag = "kind", content = "detail", deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::governance::anonymous_ballot::AnonymousBodyResultV1")]
pub enum AnonymousBodyResultV1 {
    /// A public body's quorum-endorsed finding.
    #[codec(index = 0)]
    PublicFinding(Box<AnonymousPublicFindingResultV1>),
    /// A binding jury's approved ballot: its closure record and result root.
    #[codec(index = 1)]
    Ballot(Box<AnonymousBallotCertificateBindingV1>),
}

/// A sealed body: its election, its immutable sortition request and its roster.
///
/// These facts are fixed at seal and never change. A certificate binds them together with
/// the body's result.
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
#[norito_schema(name = "iroha_data_model::governance::anonymous_ballot::AnonymousSealedBodyV1")]
pub struct AnonymousSealedBodyV1 {
    /// Body instance, derived from its election and roster root.
    pub body_instance_id: BodyInstanceId,
    /// Election attempt that produced the roster.
    pub election_attempt_id: BodyElectionAttemptId,
    /// Zero-based retry sequence of that election.
    pub election_attempt_sequence: u32,
    /// The complete immutable request committed before its future pulse.
    pub sortition_request: SortitionRequestV1,
    /// Body role.
    pub body: ParliamentBody,
    /// Sealed seats: the immutable quorum denominator, `S` of a binding jury.
    pub seats: u16,
    /// Finalized pulse the sortition consumed.
    pub beacon_pulse_id: BeaconPulseId,
    /// 64-byte root of the extended roster preimage; a binding jury's commits `(R, S,
    /// profile_id)` and through them its sealed credential list.
    pub roster_root: Digest512,
    /// Root of the deterministic body and cross-body assignments.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub assignment_root: [u8; 32],
}

impl AnonymousSealedBodyV1 {
    /// Check the sealed body of `governance_attempt_id` against the frozen `policy`: its
    /// identifiers against their preimages, its request for exactly the frozen seat target,
    /// and its seat count.
    ///
    /// The roster root's own preimage (the assignments and, for a binding jury, the sealed
    /// credential list) is State's to check.
    ///
    /// # Errors
    /// The first violated rule as an [`AnonymousGovernanceCertificateErrorV1`].
    pub fn validate(
        &self,
        governance_attempt_id: GovernanceAttemptId,
        policy: &FrozenParliamentAttemptPolicyV1,
    ) -> Result<(), AnonymousGovernanceCertificateErrorV1> {
        use AnonymousGovernanceCertificateErrorV1 as Error;

        let required = policy
            .required_body(self.body)
            .ok_or(Error::UnrequiredBody { body: self.body })?;
        let request = &self.sortition_request;
        if self.body_instance_id.as_bytes() == &[0; 32]
            || self.election_attempt_id.as_bytes() == &[0; 32]
            || self.beacon_pulse_id.as_bytes() == &[0; 32]
            || self.roster_root.as_bytes() == &[0; 64]
            || self.assignment_root == [0; 32]
        {
            return Err(Error::ZeroBinding);
        }
        if request.validate(None).is_err()
            || (
                request.governance_attempt_id,
                request.body_election_attempt_id,
                request.body,
            ) != (governance_attempt_id, self.election_attempt_id, self.body)
        {
            return Err(Error::SortitionRequestMismatch);
        }
        // The request draws for exactly the frozen target; fewer seats may be sealed.
        if request.target_seats != u32::from(required.target_seats) {
            return Err(Error::SortitionRequestMismatch);
        }
        if self.seats == 0
            || self.seats > required.target_seats
            || u32::from(self.seats) > request.candidate_count
        {
            return Err(Error::InvalidSeatCount);
        }
        if self.election_attempt_sequence > policy.sortition_policy.max_sortition_retries {
            return Err(Error::RetryLimitExceeded);
        }
        if self.election_attempt_id
            != BodyElectionAttemptId::derive_v1(
                governance_attempt_id,
                self.body,
                self.election_attempt_sequence,
            )
            || self.body_instance_id
                != anonymous_body_instance_id_v1(self.election_attempt_id, &self.roster_root)
        {
            return Err(Error::NonCanonicalIdentifier);
        }
        Ok(())
    }

    /// Whether this sealed Confirmation Jury is a fresh draw for the narrow approval of
    /// the Policy Jury `policy`, accepted at `policy_result_height`.
    ///
    /// Confirmation admission is atomic with accepting the Policy result: the first
    /// Confirmation request is committed by that closure's block, a retry by a later one.
    /// The draw consumes another pulse than the Policy Jury's. Bodies drawn in one batch
    /// share their pulse, so no other pair of bodies is constrained.
    #[must_use]
    pub fn confirms(&self, policy: &Self, policy_result_height: u64) -> bool {
        let request_height = self.sortition_request.request_height;
        let committed_in_time = if self.election_attempt_sequence == 0 {
            request_height == policy_result_height
        } else {
            request_height > policy_result_height
        };
        self.body == ParliamentBody::ConfirmationJury
            && policy.body == ParliamentBody::PolicyJury
            && committed_in_time
            && self.beacon_pulse_id != policy.beacon_pulse_id
    }

    /// The first field of `closure` that is not this sealed body's, or the attempt's: a
    /// closure record belongs to exactly one sealed body of one attempt, and its casting
    /// opened after the sortition pulse that drew the body. The rule holds for every
    /// outcome, an approval or not.
    #[must_use]
    pub fn closure_mismatch(
        &self,
        proposal_content_id: ProposalContentId,
        governance_attempt_id: GovernanceAttemptId,
        closure: &BallotClosureRecordV1,
    ) -> Option<&'static str> {
        if closure.proposal_content_id != proposal_content_id {
            Some("proposal_content_id")
        } else if closure.governance_attempt_id != governance_attempt_id {
            Some("governance_attempt_id")
        } else if closure.election_attempt_id != self.election_attempt_id {
            Some("election_attempt_id")
        } else if closure.body_instance_id != self.body_instance_id {
            Some("body_instance_id")
        } else if closure.body_role != self.body {
            Some("body_role")
        } else if closure.seats != self.seats {
            Some("seats")
        } else if closure.opening_anchor.height <= self.sortition_request.pulse_height {
            Some("opening_anchor")
        } else {
            None
        }
    }

    /// Check a public body's quorum-endorsed finding against this sealed body.
    ///
    /// # Errors
    /// [`AnonymousGovernanceCertificateErrorV1::InvalidPublicFinding`] for an inert root,
    /// endorsements other than exactly the quorum of the sealed seats, unordered or inert
    /// assignments, or an endorsement root that does not commit them.
    pub fn validate_public_finding(
        &self,
        governance_attempt_id: GovernanceAttemptId,
        result: &AnonymousPublicFindingResultV1,
    ) -> Result<(), AnonymousGovernanceCertificateErrorV1> {
        use AnonymousGovernanceCertificateErrorV1 as Error;

        let finding = &result.finding;
        let quorum = parliament_quorum_seats_v1(u32::from(self.seats));
        let endorsed = u32::try_from(finding.endorsing_assignments.len())
            .map_err(|_| Error::InvalidPublicFinding)?;
        if result.result_root == [0; 32]
            || finding.endorsement_root == [0; 32]
            || finding.quorum != quorum
            || finding.endorsements != quorum
            || endorsed != finding.endorsements
            || finding
                .endorsing_assignments
                .iter()
                .any(|assignment| assignment.as_bytes() == &[0; 32])
            || !finding
                .endorsing_assignments
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            || finding.endorsement_root
                != parliament_public_finding_endorsement_root_v1(
                    governance_attempt_id,
                    self.body_instance_id,
                    result.result_root,
                    &finding.endorsing_assignments,
                )
        {
            return Err(Error::InvalidPublicFinding);
        }
        Ok(())
    }
}

/// The sealed body and the result of one body of the pipeline.
#[derive(
    Clone,
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
    name = "iroha_data_model::governance::anonymous_ballot::AnonymousParliamentBodyBindingV1"
)]
pub struct AnonymousParliamentBodyBindingV1 {
    /// The sealed body contributing this result.
    pub sealed: AnonymousSealedBodyV1,
    /// Height at which this result became immutable.
    pub result_height: u64,
    /// The result.
    pub result: AnonymousBodyResultV1,
}

impl AnonymousParliamentBodyBindingV1 {
    /// The ballot binding of a binding jury.
    #[must_use]
    pub fn ballot(&self) -> Option<&AnonymousBallotCertificateBindingV1> {
        match &self.result {
            AnonymousBodyResultV1::Ballot(ballot) => Some(ballot),
            AnonymousBodyResultV1::PublicFinding(_) => None,
        }
    }

    /// Check the sealed body and its result for the attempt `governance_attempt_id` of
    /// `proposal_content_id` under the locked `risk_tier` and the frozen `policy`.
    ///
    /// # Errors
    /// The first violated rule as an [`AnonymousGovernanceCertificateErrorV1`].
    pub fn validate(
        &self,
        proposal_content_id: ProposalContentId,
        governance_attempt_id: GovernanceAttemptId,
        risk_tier: RiskTierV1,
        policy: &FrozenParliamentAttemptPolicyV1,
    ) -> Result<(), AnonymousGovernanceCertificateErrorV1> {
        use AnonymousGovernanceCertificateErrorV1 as Error;

        let sealed = &self.sealed;
        sealed.validate(governance_attempt_id, policy)?;
        if self.result_height <= sealed.sortition_request.pulse_height {
            return Err(Error::InvalidLifecycle);
        }
        match (BodyDecisionModeV1::of(sealed.body), &self.result) {
            (BodyDecisionModeV1::PublicFinding, AnonymousBodyResultV1::PublicFinding(result)) => {
                sealed.validate_public_finding(governance_attempt_id, result)
            }
            (BodyDecisionModeV1::AnonymousBallot, AnonymousBodyResultV1::Ballot(ballot)) => {
                ballot.validate(&policy.ballot_policy(risk_tier, sealed.body)?)?;
                let closure = &ballot.closure;
                let field = sealed
                    .closure_mismatch(proposal_content_id, governance_attempt_id, closure)
                    .or_else(|| {
                        (closure.closure_anchor.height != self.result_height)
                            .then_some("closure_anchor")
                    });
                field.map_or(Ok(()), |field| Err(Error::BallotContext { field }))
            }
            _ => Err(Error::ResultKind { body: sealed.body }),
        }
    }
}

/// The complete certificate of an anonymous Parliament attempt.
#[derive(
    Clone,
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
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::governance::anonymous_ballot::AnonymousGovernanceCertificateV1"
)]
pub struct AnonymousGovernanceCertificateV1 {
    /// Why the attempt exists: candidate testing, or production under the recorded
    /// qualification.
    pub purpose: ParliamentBallotPurposeV1,
    /// Immutable proposal content the certificate authorizes.
    pub proposal_content_id: ProposalContentId,
    /// The successful attempt, derived by the anonymous family's own rule.
    pub governance_attempt_id: GovernanceAttemptId,
    /// Zero-based retry sequence of that attempt.
    pub governance_attempt_sequence: u32,
    /// Final upward-only risk tier.
    pub risk_tier: RiskTierV1,
    /// The policy snapshot frozen at admission; every ballot policy derives from it.
    pub policy: FrozenParliamentAttemptPolicyV1,
    /// One binding per body that produced a result, in canonical body order.
    pub body_bindings: Vec<AnonymousParliamentBodyBindingV1>,
    /// Hash of the exact deterministic effect preimage.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub effect_preimage_hash: [u8; 32],
    /// Compare-and-set head required when the effect executes.
    pub expected_head: GovernanceExpectedHeadV1,
}

impl AnonymousGovernanceCertificateV1 {
    /// The binding of `body`, if the certificate has one.
    #[must_use]
    pub fn body(&self, body: ParliamentBody) -> Option<&AnonymousParliamentBodyBindingV1> {
        self.body_bindings
            .iter()
            .find(|binding| binding.sealed.body == body)
    }

    /// The closure that completed the certificate: the Confirmation Jury's when one was
    /// required, the Policy Jury's otherwise.
    #[must_use]
    pub fn certifying_ballot(&self) -> Option<&AnonymousBallotCertificateBindingV1> {
        self.body(ParliamentBody::ConfirmationJury)
            .or_else(|| self.body(ParliamentBody::PolicyJury))
            .and_then(AnonymousParliamentBodyBindingV1::ballot)
    }

    /// The anchor at which the certificate became final.
    #[must_use]
    pub fn certified_at(&self) -> Option<GovernanceAnchorV1> {
        self.certifying_ballot()
            .map(|ballot| ballot.closure.closure_anchor)
    }

    /// The governance-clock time from which enactment is due: the certifying closure's
    /// time plus the frozen enactment delay.
    #[must_use]
    pub fn enact_at_ms(&self) -> Option<u64> {
        self.certified_at()?
            .timestamp_ms
            .checked_add(self.policy.lifecycle_policy.enactment_delay_ms)
    }

    /// The gate of production enactment.
    ///
    /// # Errors
    /// [`ParliamentBallotPurposeErrorV1`] for a candidate-testing certificate, or one whose
    /// qualification is not this release's.
    pub fn require_production(&self) -> Result<(), ParliamentBallotPurposeErrorV1> {
        self.purpose.require_production()
    }

    /// Check the certificate's context-free structure.
    ///
    /// Core additionally compares every binding with the persisted attempt, rosters,
    /// pulses, boards, effect and compare-and-set state.
    ///
    /// # Errors
    /// The first violated rule as an [`AnonymousGovernanceCertificateErrorV1`].
    pub fn validate(&self) -> Result<(), AnonymousGovernanceCertificateErrorV1> {
        use AnonymousGovernanceCertificateErrorV1 as Error;

        self.purpose.validate()?;
        self.policy.validate()?;
        self.validate_context()?;

        let mut instances = BTreeSet::new();
        let mut elections = BTreeSet::new();
        let mut requests = BTreeSet::new();
        let mut voted_on = None;
        let mut previous = None;
        for binding in &self.body_bindings {
            if previous.is_some_and(|previous| previous >= binding.sealed.body) {
                return Err(Error::NonCanonicalBodyOrder);
            }
            previous = Some(binding.sealed.body);
            binding.validate(
                self.proposal_content_id,
                self.governance_attempt_id,
                self.risk_tier,
                &self.policy,
            )?;
            if !instances.insert(binding.sealed.body_instance_id)
                || !elections.insert(binding.sealed.election_attempt_id)
                || !requests.insert(binding.sealed.sortition_request.id)
            {
                return Err(Error::DuplicateBinding);
            }
            // Every jury voted on one network about one proposal, in full.
            if let Some(ballot) = binding.ballot() {
                let closure = &ballot.closure;
                let (network_id, proposal_digest) =
                    *voted_on.get_or_insert((closure.network_id, closure.proposal_digest));
                if network_id != closure.network_id {
                    return Err(Error::BallotContext {
                        field: "network_id",
                    });
                }
                if proposal_digest != closure.proposal_digest {
                    return Err(Error::BallotContext {
                        field: "proposal_digest",
                    });
                }
            }
        }

        for required in &self.policy.required_bodies {
            if required.body != ParliamentBody::ConfirmationJury
                && self.body(required.body).is_none()
            {
                return Err(Error::MissingBody {
                    body: required.body,
                });
            }
        }
        let policy = self
            .body(ParliamentBody::PolicyJury)
            .ok_or(Error::MissingBody {
                body: ParliamentBody::PolicyJury,
            })?;
        let policy_ballot = policy.ballot().ok_or(Error::ResultKind {
            body: ParliamentBody::PolicyJury,
        })?;
        match (
            policy_ballot.requires_confirmation(),
            self.body(ParliamentBody::ConfirmationJury),
        ) {
            (false, None) => {}
            // Confirmation admission is atomic with accepting the Policy result: its
            // first request is committed by that closure's block, a retry by a later one.
            (true, Some(confirmation))
                if confirmation
                    .sealed
                    .confirms(&policy.sealed, policy.result_height) => {}
            _ => return Err(Error::ConfirmationJuryMismatch),
        }

        let certified_at = self.certified_at().ok_or(Error::InvalidLifecycle)?;
        if self
            .body_bindings
            .iter()
            .any(|binding| binding.result_height > certified_at.height)
            || self.enact_at_ms().is_none()
        {
            return Err(Error::InvalidLifecycle);
        }
        Ok(())
    }

    fn validate_context(&self) -> Result<(), AnonymousGovernanceCertificateErrorV1> {
        use AnonymousGovernanceCertificateErrorV1 as Error;

        if self.proposal_content_id.as_bytes() == &[0; 32]
            || self.governance_attempt_id.as_bytes() == &[0; 32]
            || self.effect_preimage_hash == [0; 32]
        {
            return Err(Error::ZeroBinding);
        }
        if self.governance_attempt_id
            != anonymous_governance_attempt_id_v1(
                self.proposal_content_id,
                self.governance_attempt_sequence,
            )
        {
            return Err(Error::NonCanonicalIdentifier);
        }
        if self.governance_attempt_sequence
            > self.policy.sortition_policy.max_governance_attempt_retries
        {
            return Err(Error::RetryLimitExceeded);
        }
        if self.body_bindings.is_empty() {
            return Err(Error::EmptyBodyBindings);
        }
        let head_is_valid = match self.expected_head {
            GovernanceExpectedHeadV1::Absent(head) => head.subject_id != [0; 32],
            GovernanceExpectedHeadV1::Present(head) => {
                head.subject_id != [0; 32] && head.version != 0 && head.head_root != [0; 32]
            }
        };
        if !head_is_valid {
            return Err(Error::InvalidExpectedHead);
        }
        Ok(())
    }
}

#[derive(Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::governance::anonymous_ballot::AnonymousExecutionFailureRootPreimageV1"
)]
struct AnonymousExecutionFailureRootPreimageV1 {
    certificate_id: GovernanceCertificateId,
    failed_at: GovernanceAnchorV1,
}

/// The root recording that the exact effect of `certificate` failed deterministic
/// execution in the block-start pass anchored at `failed_at`. Callers supply neither an
/// effect binding nor a root of their own.
#[must_use]
pub fn anonymous_execution_failure_root_v1(
    certificate: &AnonymousGovernanceCertificateV1,
    failed_at: GovernanceAnchorV1,
) -> [u8; 32] {
    fingerprint(
        ANONYMOUS_PARLIAMENT_EXECUTION_FAILURE_ROOT_V1,
        &AnonymousExecutionFailureRootPreimageV1 {
            certificate_id: anonymous_governance_certificate_id_v1(certificate),
            failed_at,
        },
    )
}

/// The content identifier of an exact anonymous certificate. Its domain is the family's
/// own, so no baseline certificate shares it.
#[must_use]
pub fn anonymous_governance_certificate_id_v1(
    certificate: &AnonymousGovernanceCertificateV1,
) -> GovernanceCertificateId {
    GovernanceCertificateId::new(fingerprint(
        ANONYMOUS_GOVERNANCE_CERTIFICATE_ID_V1,
        certificate,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::{
        anonymous_ballot::{
            BallotClosureRecordV1, BallotOutcomeV1, PARLIAMENT_BALLOT_QUALIFICATION_V1,
            ParliamentCandidateTestingPolicyV1,
            test_fixtures::{anchor, attempt_policy, closure},
        },
        types::{AssignmentId, BeaconSessionId, GovernanceExpectedHeadAbsentV1, ProposalContentId},
    };

    const CONTENT: ProposalContentId = ProposalContentId::new([0x01; 32]);
    const SEQUENCE: u32 = 1;

    fn attempt() -> GovernanceAttemptId {
        anonymous_governance_attempt_id_v1(CONTENT, SEQUENCE)
    }

    fn candidate() -> ParliamentBallotPurposeV1 {
        ParliamentBallotPurposeV1::CandidateTesting(
            ParliamentCandidateTestingPolicyV1::CANDIDATE_V1,
        )
    }

    /// The sealed body of `body` with `seats`, its request committed at `request_height`.
    fn sealed(
        body: ParliamentBody,
        seats: u16,
        request_height: u64,
        result_height: u64,
    ) -> AnonymousParliamentBodyBindingV1 {
        let election_attempt_id = BodyElectionAttemptId::derive_v1(attempt(), body, 0);
        let target_seats = attempt_policy()
            .required_body(body)
            .expect("required body")
            .target_seats;
        let sortition_request = SortitionRequestV1::try_new_canonical(
            attempt(),
            election_attempt_id,
            body,
            [0x70; 32],
            2_000,
            u32::from(target_seats),
            request_height,
            request_height + 5,
            BeaconSessionId::new([0x71; 32]),
            None,
        )
        .expect("canonical request");
        let mut root = [0x80; 64];
        root[0] = body as u8;
        let roster_root = Digest512::new(root);
        let body_instance_id = anonymous_body_instance_id_v1(election_attempt_id, &roster_root);
        let mut pulse = [0x90; 32];
        pulse[0] = body as u8;
        AnonymousParliamentBodyBindingV1 {
            sealed: AnonymousSealedBodyV1 {
                body_instance_id,
                election_attempt_id,
                election_attempt_sequence: 0,
                sortition_request,
                body,
                seats,
                beacon_pulse_id: BeaconPulseId::new(pulse),
                roster_root,
                assignment_root: [0xa0; 32],
            },
            result_height,
            result: AnonymousBodyResultV1::PublicFinding(Box::new(
                AnonymousPublicFindingResultV1 {
                    result_root: [0; 32],
                    finding: ParliamentPublicFindingCertificateBindingV1 {
                        endorsement_root: [0; 32],
                        endorsing_assignments: Vec::new(),
                        endorsements: 0,
                        quorum: 0,
                    },
                },
            )),
        }
    }

    fn public_body(body: ParliamentBody, seats: u16) -> AnonymousParliamentBodyBindingV1 {
        let mut binding = sealed(body, seats, 10, 40);
        let quorum = parliament_quorum_seats_v1(u32::from(seats));
        let endorsing_assignments: Vec<_> = (1..=quorum)
            .map(|index| {
                let mut bytes = [0xb0; 32];
                bytes[..4].copy_from_slice(&index.to_be_bytes());
                AssignmentId::new(bytes)
            })
            .collect();
        let result_root = [0xc0; 32];
        binding.result =
            AnonymousBodyResultV1::PublicFinding(Box::new(AnonymousPublicFindingResultV1 {
                result_root,
                finding: ParliamentPublicFindingCertificateBindingV1 {
                    endorsement_root: parliament_public_finding_endorsement_root_v1(
                        attempt(),
                        binding.sealed.body_instance_id,
                        result_root,
                        &endorsing_assignments,
                    ),
                    endorsing_assignments,
                    endorsements: quorum,
                    quorum,
                },
            }));
        binding
    }

    /// The jury `body` approving `aye` to `nay` of `seats`, closed at `closed`.
    fn jury(
        body: ParliamentBody,
        seats: u16,
        (aye, nay): (u32, u32),
        request_height: u64,
        closed: GovernanceAnchorV1,
    ) -> AnonymousParliamentBodyBindingV1 {
        let mut binding = sealed(body, seats, request_height, closed.height);
        let policy = attempt_policy()
            .ballot_policy(RiskTierV1::Standard, body)
            .expect("ballot policy");
        // Casting opens after the draw and closes one window after its clock.
        let casting_window_ms = policy.binding.casting_window_ms;
        let casting_clock_timestamp_ms = closed.timestamp_ms - casting_window_ms - 500;
        let record = BallotClosureRecordV1 {
            proposal_content_id: CONTENT,
            governance_attempt_id: attempt(),
            election_attempt_id: binding.sealed.election_attempt_id,
            body_instance_id: binding.sealed.body_instance_id,
            body_role: body,
            opening_anchor: anchor(request_height + 50, 2, casting_clock_timestamp_ms - 1_000),
            casting_clock_height: request_height + 51,
            casting_clock_timestamp_ms,
            casting_window_ms,
            closing_ms: casting_clock_timestamp_ms + casting_window_ms,
            closure_anchor: closed,
            ..closure(seats, aye, nay, 0)
        };
        binding.result = AnonymousBodyResultV1::Ballot(Box::new(
            AnonymousBallotCertificateBindingV1::approve(record, &policy).expect("approval"),
        ));
        binding
    }

    /// Every public body of the fixture pipeline and a clear Policy Jury approval.
    fn certificate(purpose: ParliamentBallotPurposeV1) -> AnonymousGovernanceCertificateV1 {
        let policy = attempt_policy();
        let mut body_bindings: Vec<_> = policy
            .required_bodies
            .iter()
            .filter(|required| {
                BodyDecisionModeV1::of(required.body) == BodyDecisionModeV1::PublicFinding
            })
            .map(|required| public_body(required.body, required.target_seats.min(9)))
            .collect();
        body_bindings.push(jury(
            ParliamentBody::PolicyJury,
            150,
            (120, 20),
            50,
            anchor(90_000, 0, 87_401_500),
        ));
        body_bindings.sort_by_key(|binding| binding.sealed.body);
        AnonymousGovernanceCertificateV1 {
            purpose,
            proposal_content_id: CONTENT,
            governance_attempt_id: attempt(),
            governance_attempt_sequence: SEQUENCE,
            risk_tier: RiskTierV1::Standard,
            policy,
            body_bindings,
            effect_preimage_hash: [0xd0; 32],
            expected_head: GovernanceExpectedHeadV1::Absent(GovernanceExpectedHeadAbsentV1 {
                subject_id: [0xe0; 32],
            }),
        }
    }

    /// The same certificate after a narrow Policy approval and its Confirmation.
    fn confirmed(purpose: ParliamentBallotPurposeV1) -> AnonymousGovernanceCertificateV1 {
        let mut certificate = certificate(purpose);
        certificate
            .body_bindings
            .retain(|binding| binding.sealed.body != ParliamentBody::PolicyJury);
        certificate.body_bindings.push(jury(
            ParliamentBody::PolicyJury,
            150,
            (51, 49),
            50,
            anchor(90_000, 0, 87_401_500),
        ));
        certificate.body_bindings.push(jury(
            ParliamentBody::ConfirmationJury,
            150,
            (120, 20),
            90_000,
            anchor(190_000, 1, 187_401_500),
        ));
        certificate
            .body_bindings
            .sort_by_key(|binding| binding.sealed.body);
        certificate
    }

    /// Replace the ballot of `body` by the approval of its edited closure record.
    fn rebind(
        certificate: &mut AnonymousGovernanceCertificateV1,
        body: ParliamentBody,
        edit: impl Fn(&mut BallotClosureRecordV1),
    ) {
        let policy = certificate
            .policy
            .ballot_policy(certificate.risk_tier, body)
            .expect("ballot policy");
        let binding = body_mut(certificate, body);
        let mut record = binding.ballot().expect("ballot").closure;
        edit(&mut record);
        binding.result = AnonymousBodyResultV1::Ballot(Box::new(
            AnonymousBallotCertificateBindingV1::approve(record, &policy).expect("approval"),
        ));
    }

    fn body_mut(
        certificate: &mut AnonymousGovernanceCertificateV1,
        body: ParliamentBody,
    ) -> &mut AnonymousParliamentBodyBindingV1 {
        certificate
            .body_bindings
            .iter_mut()
            .find(|binding| binding.sealed.body == body)
            .expect("bound body")
    }

    #[test]
    fn a_complete_certificate_validates_and_roundtrips() {
        let certificate = certificate(candidate());
        assert_eq!(certificate.validate(), Ok(()));
        // Final at the Policy Jury's closure; due after the frozen delay.
        assert_eq!(
            certificate.certified_at(),
            Some(anchor(90_000, 0, 87_401_500))
        );
        assert_eq!(
            certificate.enact_at_ms(),
            Some(87_401_500 + certificate.policy.lifecycle_policy.enactment_delay_ms)
        );

        let bytes = norito::encode_canonical(&certificate).expect("encode");
        assert_eq!(
            norito::decode_canonical::<AnonymousGovernanceCertificateV1>(&bytes).expect("decode"),
            certificate
        );
        let json = norito::json::to_json(&certificate).expect("json");
        assert_eq!(
            norito::json::from_str::<AnonymousGovernanceCertificateV1>(&json).expect("from json"),
            certificate
        );
    }

    #[test]
    fn a_candidate_certificate_carries_no_production_authority() {
        let candidate = certificate(candidate());
        assert_eq!(candidate.validate(), Ok(()));
        assert_eq!(
            candidate.require_production(),
            Err(ParliamentBallotPurposeErrorV1::CandidateTesting)
        );

        // No release qualification is recorded, so no purpose passes the gate and a
        // qualified purpose is not even a valid certificate.
        assert_eq!(PARLIAMENT_BALLOT_QUALIFICATION_V1, None);
        let claimed = certificate(ParliamentBallotPurposeV1::Qualified(Digest512::new(
            [0x42; 64],
        )));
        assert_eq!(
            claimed.validate(),
            Err(AnonymousGovernanceCertificateErrorV1::Purpose(
                ParliamentBallotPurposeErrorV1::Unqualified
            ))
        );
        assert_eq!(
            claimed.require_production(),
            Err(ParliamentBallotPurposeErrorV1::Unqualified)
        );

        // The purpose is part of the certificate's identity.
        assert_ne!(
            anonymous_governance_certificate_id_v1(&candidate),
            anonymous_governance_certificate_id_v1(&claimed)
        );
    }

    #[test]
    fn an_execution_failure_root_commits_the_certificate_and_its_anchor() {
        let certificate = certificate(candidate());
        let at = anchor(100_000, 3, 99_000_000);
        let root = anonymous_execution_failure_root_v1(&certificate, at);
        assert_ne!(root, [0; 32]);
        assert_eq!(root, anonymous_execution_failure_root_v1(&certificate, at));
        for other in [
            anchor(100_001, 3, 99_000_000),
            anchor(100_000, 4, 99_000_000),
            anchor(100_000, 3, 99_000_001),
        ] {
            assert_ne!(
                root,
                anonymous_execution_failure_root_v1(&certificate, other)
            );
        }
        assert_ne!(
            root,
            anonymous_execution_failure_root_v1(&confirmed(candidate()), at)
        );
        // The root is not the certificate's identifier.
        assert_ne!(
            root,
            *anonymous_governance_certificate_id_v1(&certificate).as_bytes()
        );
    }

    #[test]
    fn a_sealed_body_is_checked_on_its_own() {
        use AnonymousGovernanceCertificateErrorV1 as Error;

        let policy = attempt_policy();
        let sealed = public_body(ParliamentBody::RulesCommittee, 9).sealed;
        assert_eq!(sealed.validate(attempt(), &policy), Ok(()));
        // Another attempt's body.
        assert_eq!(
            sealed.validate(anonymous_governance_attempt_id_v1(CONTENT, 2), &policy),
            Err(Error::SortitionRequestMismatch)
        );
        // The request draws for exactly the frozen target.
        let mut off_target = sealed;
        off_target.sortition_request = SortitionRequestV1::try_new_canonical(
            attempt(),
            sealed.election_attempt_id,
            sealed.body,
            [0x70; 32],
            2_000,
            9,
            10,
            15,
            BeaconSessionId::new([0x71; 32]),
            None,
        )
        .expect("canonical request");
        assert_eq!(
            off_target.validate(attempt(), &policy),
            Err(Error::SortitionRequestMismatch)
        );
        let mut over_target = sealed;
        over_target.seats = u16::MAX;
        assert_eq!(
            over_target.validate(attempt(), &policy),
            Err(Error::InvalidSeatCount)
        );
        let mut retry = sealed;
        retry.election_attempt_sequence = u32::MAX;
        assert_eq!(
            retry.validate(attempt(), &policy),
            Err(Error::RetryLimitExceeded)
        );
    }

    #[test]
    fn a_narrow_policy_approval_is_certified_only_with_its_confirmation() {
        let confirmed = confirmed(candidate());
        assert_eq!(confirmed.validate(), Ok(()));
        // The Confirmation Jury's closure certifies.
        assert_eq!(
            confirmed.certified_at(),
            Some(anchor(190_000, 1, 187_401_500))
        );

        let mut unconfirmed = confirmed.clone();
        unconfirmed
            .body_bindings
            .retain(|binding| binding.sealed.body != ParliamentBody::ConfirmationJury);
        assert_eq!(
            unconfirmed.validate(),
            Err(AnonymousGovernanceCertificateErrorV1::ConfirmationJuryMismatch)
        );

        // A clear approval never carries a Confirmation result.
        let mut superfluous = certificate(candidate());
        superfluous.body_bindings.push(jury(
            ParliamentBody::ConfirmationJury,
            150,
            (120, 20),
            90_000,
            anchor(190_000, 1, 187_401_500),
        ));
        superfluous
            .body_bindings
            .sort_by_key(|binding| binding.sealed.body);
        assert_eq!(
            superfluous.validate(),
            Err(AnonymousGovernanceCertificateErrorV1::ConfirmationJuryMismatch)
        );

        // Both juries decide the same proposal, in full: its 64-byte digest.
        let mut other_proposal = confirmed.clone();
        rebind(
            &mut other_proposal,
            ParliamentBody::ConfirmationJury,
            |record| record.proposal_digest = Digest512::new([0x12; 64]),
        );
        assert_eq!(
            other_proposal.validate(),
            Err(AnonymousGovernanceCertificateErrorV1::BallotContext {
                field: "proposal_digest"
            })
        );

        // A Confirmation Jury is a fresh draw: never the Policy Jury's pulse.
        let mut same_pulse = confirmed.clone();
        let policy_pulse = body_mut(&mut same_pulse, ParliamentBody::PolicyJury)
            .sealed
            .beacon_pulse_id;
        body_mut(&mut same_pulse, ParliamentBody::ConfirmationJury)
            .sealed
            .beacon_pulse_id = policy_pulse;
        assert_eq!(
            same_pulse.validate(),
            Err(AnonymousGovernanceCertificateErrorV1::ConfirmationJuryMismatch)
        );
        // Bodies drawn in one batch share their pulse.
        let mut shared_pulse = confirmed.clone();
        body_mut(&mut shared_pulse, ParliamentBody::RulesCommittee)
            .sealed
            .beacon_pulse_id = policy_pulse;
        assert_eq!(shared_pulse.validate(), Ok(()));

        // The first Confirmation request is committed by the Policy closure's block.
        let mut late = confirmed;
        let stale = jury(
            ParliamentBody::ConfirmationJury,
            150,
            (120, 20),
            90_001,
            anchor(190_000, 1, 187_401_500),
        );
        *body_mut(&mut late, ParliamentBody::ConfirmationJury) = stale;
        assert_eq!(
            late.validate(),
            Err(AnonymousGovernanceCertificateErrorV1::ConfirmationJuryMismatch)
        );
    }

    #[test]
    fn every_required_body_is_bound_once_in_order_and_by_its_own_mode() {
        use AnonymousGovernanceCertificateErrorV1 as Error;

        let valid = certificate(candidate());

        let mut missing = valid.clone();
        missing
            .body_bindings
            .retain(|binding| binding.sealed.body != ParliamentBody::RulesCommittee);
        assert_eq!(
            missing.validate(),
            Err(Error::MissingBody {
                body: ParliamentBody::RulesCommittee
            })
        );

        let mut reordered = valid.clone();
        reordered.body_bindings.swap(0, 1);
        assert_eq!(reordered.validate(), Err(Error::NonCanonicalBodyOrder));

        let mut empty = valid.clone();
        empty.body_bindings.clear();
        assert_eq!(empty.validate(), Err(Error::EmptyBodyBindings));

        // A jury cannot be certified by a public finding, nor a public body by a ballot.
        let mut finding_jury = valid.clone();
        let finding = public_body(ParliamentBody::RulesCommittee, 9).result;
        body_mut(&mut finding_jury, ParliamentBody::PolicyJury).result = finding;
        assert_eq!(
            finding_jury.validate(),
            Err(Error::ResultKind {
                body: ParliamentBody::PolicyJury
            })
        );
        let mut ballot_committee = valid.clone();
        let ballot = body_mut(&mut ballot_committee, ParliamentBody::PolicyJury)
            .result
            .clone();
        body_mut(&mut ballot_committee, ParliamentBody::RulesCommittee).result = ballot;
        assert_eq!(
            ballot_committee.validate(),
            Err(Error::ResultKind {
                body: ParliamentBody::RulesCommittee
            })
        );

        // A body outside the frozen pipeline.
        let mut unrequired = valid;
        unrequired
            .policy
            .required_bodies
            .retain(|required| required.body != ParliamentBody::RulesCommittee);
        assert!(matches!(
            unrequired.validate(),
            Err(Error::UnrequiredBody {
                body: ParliamentBody::RulesCommittee
            }) | Err(Error::Policy(_))
        ));
    }

    #[test]
    fn identifiers_seats_and_findings_are_checked_against_their_preimages() {
        use AnonymousGovernanceCertificateErrorV1 as Error;

        let valid = certificate(candidate());

        // The baseline derivation of the attempt identifier is not this family's.
        let mut baseline_attempt = valid.clone();
        baseline_attempt.governance_attempt_id = GovernanceAttemptId::derive_v1(CONTENT, SEQUENCE);
        assert_eq!(
            baseline_attempt.validate(),
            Err(Error::NonCanonicalIdentifier)
        );

        // A body instance is derived from the whole 64-byte roster root.
        let mut other_root = valid.clone();
        let rules = body_mut(&mut other_root, ParliamentBody::RulesCommittee);
        let mut root = *rules.sealed.roster_root.as_bytes();
        root[63] ^= 1;
        rules.sealed.roster_root = Digest512::new(root);
        assert_eq!(other_root.validate(), Err(Error::NonCanonicalIdentifier));

        let mut no_seat = valid.clone();
        body_mut(&mut no_seat, ParliamentBody::RulesCommittee)
            .sealed
            .seats = 0;
        assert_eq!(no_seat.validate(), Err(Error::InvalidSeatCount));

        let mut short_quorum = valid.clone();
        if let AnonymousBodyResultV1::PublicFinding(result) =
            &mut body_mut(&mut short_quorum, ParliamentBody::RulesCommittee).result
        {
            result.finding.endorsing_assignments.pop();
            result.finding.endorsements -= 1;
        }
        assert_eq!(short_quorum.validate(), Err(Error::InvalidPublicFinding));

        let mut zero_effect = valid.clone();
        zero_effect.effect_preimage_hash = [0; 32];
        assert_eq!(zero_effect.validate(), Err(Error::ZeroBinding));

        let mut inert_head = valid.clone();
        inert_head.expected_head =
            GovernanceExpectedHeadV1::Absent(GovernanceExpectedHeadAbsentV1 {
                subject_id: [0; 32],
            });
        assert_eq!(inert_head.validate(), Err(Error::InvalidExpectedHead));

        let mut over_limit = valid;
        over_limit.governance_attempt_sequence = u32::MAX;
        over_limit.governance_attempt_id = anonymous_governance_attempt_id_v1(CONTENT, u32::MAX);
        assert_eq!(over_limit.validate(), Err(Error::RetryLimitExceeded));
    }

    #[test]
    fn a_ballot_binding_is_the_record_of_its_own_body_and_an_approval() {
        use AnonymousGovernanceCertificateErrorV1 as Error;

        let valid = certificate(candidate());
        let refused = |edit: &dyn Fn(&mut BallotClosureRecordV1)| {
            let mut certificate = valid.clone();
            rebind(&mut certificate, ParliamentBody::PolicyJury, edit);
            certificate.validate()
        };
        let context = |field| Err(Error::BallotContext { field });

        assert_eq!(
            refused(&|record| record.proposal_content_id = ProposalContentId::new([0x02; 32])),
            context("proposal_content_id")
        );
        assert_eq!(
            refused(&|record| record.governance_attempt_id = GovernanceAttemptId::new([0x12; 32])),
            context("governance_attempt_id")
        );
        assert_eq!(
            refused(&|record| record.body_instance_id = BodyInstanceId::new([0x13; 32])),
            context("body_instance_id")
        );
        assert_eq!(
            refused(&|record| record.closure_anchor.height += 1),
            context("closure_anchor")
        );
        // Casting cannot open before the pulse that drew the jury.
        let pulse_height = valid
            .body(ParliamentBody::PolicyJury)
            .expect("Policy Jury")
            .sealed
            .sortition_request
            .pulse_height;
        assert_eq!(
            refused(&|record| {
                record.opening_anchor.height = pulse_height;
                record.casting_clock_height = pulse_height + 1;
            }),
            context("opening_anchor")
        );
        assert_eq!(
            refused(&|record| {
                record.opening_anchor.height = pulse_height + 1;
                record.casting_clock_height = pulse_height + 2;
            }),
            Ok(())
        );

        // A rejection is never bound, whatever root accompanies it.
        let mut rejected = valid.clone();
        let binding = body_mut(&mut rejected, ParliamentBody::PolicyJury);
        let mut ballot = *binding.ballot().expect("ballot");
        ballot.closure = BallotClosureRecordV1 {
            proposal_content_id: CONTENT,
            governance_attempt_id: attempt(),
            election_attempt_id: binding.sealed.election_attempt_id,
            body_instance_id: binding.sealed.body_instance_id,
            closure_anchor: ballot.closure.closure_anchor,
            ..closure(150, 20, 120, 0)
        };
        ballot.result_root = ballot.closure.result_root().expect("root");
        binding.result = AnonymousBodyResultV1::Ballot(Box::new(ballot));
        assert_eq!(
            rejected.validate(),
            Err(Error::Ballot(AnonymousBallotBindingErrorV1::NotApproved {
                outcome: BallotOutcomeV1::Rejected
            }))
        );

        // A result after the certifying closure is not part of this certificate.
        let mut late_finding = valid;
        body_mut(&mut late_finding, ParliamentBody::RulesCommittee).result_height = 90_001;
        assert_eq!(late_finding.validate(), Err(Error::InvalidLifecycle));
    }
}
