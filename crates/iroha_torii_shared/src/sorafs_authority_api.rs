//! Exact authenticated observations of the `SoraFS` Parliament authority proposal host.
//!
//! These are current-state observations. They do not replace certified execution proofs.

use iroha_data_model::{
    NetworkId,
    account::AccountId,
    governance::types::{GovernanceAttemptId, GovernanceCertificateId, ProposalContentId},
    isi::sorafs_authority::{
        SorafsAuthorityActionV1, SorafsAuthorityProposalV1, SorafsOperatingAuthorityV1,
    },
    runtime::definition::DefinitionId,
};
use norito::derive::{JsonDeserialize, JsonSerialize};

/// Latest lifecycle status of an authority proposal, independent of its host encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(tag = "kind", content = "value", deny_unknown_fields)]
pub enum SorafsAuthorityProposalStatusV1 {
    /// Submitted; its latest attempt is active or certified, or has not yet started.
    Proposed,
    /// The latest attempt rejected the proposal.
    Rejected,
    /// Its exact certified effect applied.
    Enacted,
    /// The expected predecessor changed before enactment.
    Superseded,
    /// The exact certified effect failed atomically.
    ExecutionFailed,
}

/// Immutable proposal with the currently mirrored lifecycle status.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SorafsAuthorityProposalProjectionV1 {
    /// Original bonded proposer.
    pub proposer: AccountId,
    /// Complete network/root-bound voted content.
    pub proposal: SorafsAuthorityProposalV1,
    /// Canonical decimal creation height.
    pub created_height: String,
    /// Latest retained proposal status.
    pub status: SorafsAuthorityProposalStatusV1,
    /// Canonical deadline receipt, separate from the immutable reducer's latest status.
    #[norito(required)]
    pub deadline_closure: Option<SorafsAuthorityDeadlineClosureProjectionV1>,
}

/// Deterministic end of execution eligibility for still-pending content.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SorafsAuthorityDeadlineClosureProjectionV1 {
    /// Canonical decimal approval cutoff or early-revocation term end.
    pub closed_at_height: String,
}

/// Exact proposal lookup, including a typed absence observation.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SorafsAuthorityProposalReadV1 {
    /// Genesis-derived response network.
    pub network: NetworkId,
    /// Exact requested content identifier.
    pub proposal_content_id: ProposalContentId,
    /// Canonical decimal height of the single read view.
    pub current_height: String,
    /// Exact retained record, absent when no hosted proposal matches.
    #[norito(required)]
    pub record: Option<SorafsAuthorityProposalProjectionV1>,
}

/// Monotonic exact authority head with its last successful execution receipt.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SorafsAuthorityHeadProjectionV1 {
    /// Exact normalized lease plan as lowercase 32-byte hexadecimal.
    pub plan_id: String,
    /// Canonical nonzero decimal term index.
    pub term: String,
    /// Canonical decimal exclusive authority expiration height.
    pub expires_at_height: String,
    /// Exact authority and provider scope.
    pub authority: SorafsOperatingAuthorityV1,
    /// Canonical account holder.
    pub account: AccountId,
    /// Canonical nonzero decimal revision; remains after revocation.
    pub revision: String,
    /// Whether the direct native authority is currently installed.
    pub granted: bool,
    /// Last successfully enacted decision.
    pub proposal_content_id: ProposalContentId,
    /// Exact successful shared-engine attempt.
    pub governance_attempt_id: GovernanceAttemptId,
    /// Exact retained enactment certificate.
    pub certificate_id: GovernanceCertificateId,
    /// Canonical decimal exact effect height.
    pub enacted_at_height: String,
}

/// Exact authority subject lookup, including a typed absence observation.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SorafsAuthorityHeadReadV1 {
    /// Genesis-derived response network.
    pub network: NetworkId,
    /// Exact requested lowercase 32-byte hexadecimal subject identity.
    pub subject_id: String,
    /// Canonical decimal height of the single read view.
    pub current_height: String,
    /// Latest successful authority head, including revoked tombstones.
    #[norito(required)]
    pub head: Option<SorafsAuthorityHeadProjectionV1>,
}

/// One independently enacted Parliament authorization for an exact lease plan.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SorafsAuthorityApprovalProjectionV1 {
    /// Exact action-specific decision content.
    pub proposal_content_id: ProposalContentId,
    /// Shared Parliament attempt owning this authorization.
    pub governance_attempt_id: GovernanceAttemptId,
    /// Exact retained authorization certificate.
    pub certificate_id: GovernanceCertificateId,
    /// Canonical decimal authorization enactment height.
    pub approved_at_height: String,
}

/// Actual installation or removal, distinct from authorization enactment.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SorafsAuthorityExecutionProjectionV1 {
    /// Canonical decimal resulting authority revision.
    pub revision: String,
    /// Canonical decimal actual projection effect height.
    pub at_height: String,
    /// Exact retained authorization consumed by this effect.
    pub approval: SorafsAuthorityApprovalProjectionV1,
    /// Captured selected services semantics.
    pub services_profile: DefinitionId,
    /// Captured native authority implementation.
    pub implementation: DefinitionId,
}

/// Exact independently approved grant and terminal mandate with execution receipts.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SorafsAuthorityLeaseProjectionV1 {
    /// Complete plan content normalized to the Grant action.
    pub plan: SorafsAuthorityProposalV1,
    /// Independently enacted grant authorization.
    #[norito(required)]
    pub grant_approval: Option<SorafsAuthorityApprovalProjectionV1>,
    /// Independently enacted irrevocable terminal authorization.
    #[norito(required)]
    pub terminal_approval: Option<SorafsAuthorityApprovalProjectionV1>,
    /// Actual successful grant and captured completion owner.
    #[norito(required)]
    pub activation: Option<SorafsAuthorityExecutionProjectionV1>,
    /// Actual early or scheduled termination.
    #[norito(required)]
    pub completion: Option<SorafsAuthorityExecutionProjectionV1>,
    /// Whether the one permitted start has been processed, including refusal.
    pub start_processed: bool,
}

/// Exact lease plan lookup, including a typed absence observation.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SorafsAuthorityLeaseReadV1 {
    /// Genesis-derived response network.
    pub network: NetworkId,
    /// Exact requested lowercase 32-byte hexadecimal normalized plan identity.
    pub plan_id: String,
    /// Canonical decimal height of the single read view.
    pub current_height: String,
    /// Exact retained approvals and execution receipts, or typed absence.
    #[norito(required)]
    pub lease: Option<SorafsAuthorityLeaseProjectionV1>,
}

fn canonical_u64(value: &str) -> bool {
    value
        .parse::<u64>()
        .is_ok_and(|number| number.to_string() == value)
}

fn retained_height(value: &str, current: &str) -> bool {
    canonical_u64(value)
        && value.parse::<u64>().is_ok_and(|height| {
            height > 0
                && current
                    .parse::<u64>()
                    .is_ok_and(|current| height <= current)
        })
}

impl SorafsAuthorityProposalReadV1 {
    /// Verify the response belongs to the exact pinned network and requested proposal.
    #[must_use]
    pub fn matches(&self, network: &NetworkId, id: ProposalContentId) -> bool {
        &self.network == network
            && self.proposal_content_id == id
            && canonical_u64(&self.current_height)
            && self.record.as_ref().is_none_or(|record| {
                record.proposal.invariant_error().is_none()
                    && &record.proposal.network == network
                    && record.proposal.proposal_content_id() == id
                    && retained_height(&record.created_height, &self.current_height)
                    && record
                        .proposal
                        .grid
                        .window(record.proposal.term)
                        .is_some_and(|window| {
                            let (start, end) =
                                if record.proposal.action == SorafsAuthorityActionV1::Revoke {
                                    (window.start_height, window.end_height)
                                } else {
                                    (window.admission_height, window.cutoff_height)
                                };
                            record
                                .created_height
                                .parse::<u64>()
                                .is_ok_and(|height| (start..end).contains(&height))
                        })
                    && (record.status != SorafsAuthorityProposalStatusV1::Proposed
                        || record.deadline_closure.is_some()
                        || record
                            .proposal
                            .grid
                            .window(record.proposal.term)
                            .is_some_and(|window| {
                                let deadline =
                                    if record.proposal.action == SorafsAuthorityActionV1::Revoke {
                                        window.end_height
                                    } else {
                                        window.cutoff_height
                                    };
                                self.current_height
                                    .parse::<u64>()
                                    .is_ok_and(|height| height < deadline)
                            }))
                    && record.deadline_closure.as_ref().is_none_or(|closure| {
                        let window = record.proposal.grid.window(record.proposal.term);
                        let deadline = window.map(|w| {
                            if record.proposal.action == SorafsAuthorityActionV1::Revoke {
                                w.end_height
                            } else {
                                w.cutoff_height
                            }
                        });
                        record.status == SorafsAuthorityProposalStatusV1::Proposed
                            && retained_height(&closure.closed_at_height, &self.current_height)
                            && closure.closed_at_height.parse::<u64>().ok() == deadline
                            && record.created_height.parse::<u64>().ok() < deadline
                    })
            })
    }
}

impl SorafsAuthorityHeadReadV1 {
    /// Verify network, subject, canonical counters and exact receipt identifier bindings.
    #[must_use]
    pub fn matches(&self, network: &NetworkId, subject: &[u8; 32]) -> bool {
        &self.network == network
            && self.subject_id == hex::encode(subject)
            && canonical_u64(&self.current_height)
            && self.head.as_ref().is_none_or(|head| {
                SorafsAuthorityProposalV1::subject_id(
                    network,
                    iroha_data_model::runtime::lifecycle::RootScopeV1::Universal,
                    head.authority,
                    &head.account,
                ) == *subject
                    && canonical_u64(&head.revision)
                    && head.revision != "0"
                    && canonical_u64(&head.term)
                    && head.term != "0"
                    && head.plan_id.len() == 64
                    && head
                        .plan_id
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                    && head.plan_id != "0".repeat(64)
                    && canonical_u64(&head.expires_at_height)
                    && head.expires_at_height != "0"
                    && (!head.granted
                        || self.current_height.parse::<u64>().ok()
                            < head.expires_at_height.parse::<u64>().ok())
                    && retained_height(&head.enacted_at_height, &self.current_height)
                    && !matches!(head.authority,
                        SorafsOperatingAuthorityV1::CompleteReplicationOrder { provider_id }
                            if provider_id.as_bytes() == &[0;32])
                    && head.proposal_content_id.as_bytes() != &[0; 32]
                    && head.governance_attempt_id.as_bytes() != &[0; 32]
                    && head.certificate_id.as_bytes() != &[0; 32]
            })
    }
}

impl SorafsAuthorityApprovalProjectionV1 {
    fn matches(
        &self,
        plan: &SorafsAuthorityProposalV1,
        action: SorafsAuthorityActionV1,
        current: &str,
    ) -> bool {
        let mut decision = plan.clone();
        decision.action = action;
        self.proposal_content_id == decision.proposal_content_id()
            && self.governance_attempt_id.as_bytes() != &[0; 32]
            && self.certificate_id.as_bytes() != &[0; 32]
            && retained_height(&self.approved_at_height, current)
    }
}

impl SorafsAuthorityExecutionProjectionV1 {
    fn matches(&self, revision: u64, current: &str) -> bool {
        self.revision == revision.to_string()
            && retained_height(&self.at_height, current)
            && self.approval.approved_at_height.parse::<u64>().ok()
                <= self.at_height.parse::<u64>().ok()
            && self.services_profile.0 != [0; 32]
            && self.implementation.0 != [0; 32]
    }
}

impl SorafsAuthorityLeaseReadV1 {
    /// Verify the pinned network, exact plan and separate authorization/effect bindings.
    #[must_use]
    pub fn matches(&self, network: &NetworkId, id: &[u8; 32]) -> bool {
        if &self.network != network
            || self.plan_id != hex::encode(id)
            || !canonical_u64(&self.current_height)
        {
            return false;
        }
        let Some(lease) = &self.lease else {
            return true;
        };
        let plan = &lease.plan;
        let Some(window) = plan.grid.window(plan.term) else {
            return false;
        };
        if plan.invariant_error().is_some()
            || &plan.network != network
            || plan.action != SorafsAuthorityActionV1::Grant
            || plan.plan_id() != *id
            || (lease.grant_approval.is_none() && lease.terminal_approval.is_none())
        {
            return false;
        }
        let Ok(current) = self.current_height.parse::<u64>() else {
            return false;
        };
        if lease.start_processed != (current >= window.start_height) {
            return false;
        }
        for (approval, action) in [
            (&lease.grant_approval, SorafsAuthorityActionV1::Grant),
            (
                &lease.terminal_approval,
                SorafsAuthorityActionV1::AuthorizeTerminalRevoke,
            ),
        ] {
            if approval.as_ref().is_some_and(|approval| {
                !approval.matches(plan, action, &self.current_height)
                    || approval
                        .approved_at_height
                        .parse::<u64>()
                        .is_ok_and(|height| {
                            height < window.admission_height || height >= window.cutoff_height
                        })
            }) {
                return false;
            }
        }
        let Some(activation) = &lease.activation else {
            return lease.completion.is_none();
        };
        if !lease.start_processed
            || lease.grant_approval.as_ref() != Some(&activation.approval)
            || lease.terminal_approval.is_none()
            || !activation.matches(plan.expected_revision + 1, &self.current_height)
            || activation.at_height != window.start_height.to_string()
        {
            return false;
        }
        let Some(completion) = &lease.completion else {
            return current < window.end_height;
        };
        if !completion.matches(plan.expected_revision + 2, &self.current_height)
            || completion.services_profile != activation.services_profile
            || completion.implementation != activation.implementation
            || completion
                .at_height
                .parse::<u64>()
                .is_ok_and(|height| height < window.start_height)
        {
            return false;
        }
        if lease.terminal_approval.as_ref() == Some(&completion.approval) {
            completion
                .at_height
                .parse::<u64>()
                .is_ok_and(|height| height == window.end_height)
        } else {
            completion
                .approval
                .matches(plan, SorafsAuthorityActionV1::Revoke, &self.current_height)
                && completion.at_height == completion.approval.approved_at_height
                && completion
                    .at_height
                    .parse::<u64>()
                    .is_ok_and(|height| height < window.end_height)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Hash, HashOf, KeyPair};

    fn network(byte: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([byte; 32])))
    }

    #[test]
    fn exact_reads_reject_cross_network_ids_and_noncanonical_counters() {
        let network = network(1);
        let mut response = SorafsAuthorityHeadReadV1 {
            network,
            subject_id: hex::encode([1; 32]),
            current_height: "50".into(),
            head: None,
        };
        assert!(response.matches(&network, &[1; 32]));
        let mut missing = norito::json::to_value(&response).unwrap();
        missing.as_object_mut().unwrap().remove("head");
        assert!(norito::json::from_value::<SorafsAuthorityHeadReadV1>(missing).is_err());
        assert!(!response.matches(&self::network(2), &[1; 32]));
        assert!(!response.matches(&network, &[2; 32]));
        response.current_height = "050".into();
        assert!(!response.matches(&network, &[1; 32]));
        assert_eq!(
            norito::json::from_str::<SorafsAuthorityHeadReadV1>(
                &norito::json::to_json(&response).unwrap()
            )
            .unwrap(),
            response
        );
        let p = SorafsAuthorityProposalV1 {
            network,
            root: iroha_data_model::runtime::lifecycle::RootScopeV1::Universal,
            account: AccountId::new(KeyPair::random().public_key().clone()),
            authority: SorafsOperatingAuthorityV1::SetReservePolicy,
            action: iroha_data_model::isi::sorafs_authority::SorafsAuthorityActionV1::Grant,
            grid: iroha_data_model::isi::sorafs_authority::SorafsAuthorityTermGridV1 {
                epoch_length_blocks: 10,
            },
            term: 1,
            expected_revision: 0,
            completion_policy: None,
        };
        let id = p.proposal_content_id();
        let mut response = SorafsAuthorityProposalReadV1 {
            network,
            proposal_content_id: id,
            current_height: "50".into(),
            record: Some(SorafsAuthorityProposalProjectionV1 {
                proposer: p.account.clone(),
                proposal: p,
                created_height: "40".into(),
                status: SorafsAuthorityProposalStatusV1::Proposed,
                deadline_closure: None,
            }),
        };
        assert!(response.matches(&network, id));
        let mut missing = norito::json::to_value(&response).unwrap();
        missing.as_object_mut().unwrap().remove("record");
        assert!(norito::json::from_value::<SorafsAuthorityProposalReadV1>(missing).is_err());
        assert_eq!(
            norito::json::from_str::<SorafsAuthorityProposalReadV1>(
                &norito::json::to_json(&response).unwrap()
            )
            .unwrap(),
            response
        );
        for invalid in ["0", "51", "040"] {
            response.record.as_mut().unwrap().created_height = invalid.into();
            assert!(!response.matches(&network, id));
        }
        response.record.as_mut().unwrap().created_height = "40".into();
        response.current_height = "631".into();
        assert!(
            !response.matches(&network, id),
            "expired Proposed content requires its separate closure"
        );
        response.record.as_mut().unwrap().deadline_closure =
            Some(SorafsAuthorityDeadlineClosureProjectionV1 {
                closed_at_height: "631".into(),
            });
        assert!(response.matches(&network, id));
        response
            .record
            .as_mut()
            .unwrap()
            .deadline_closure
            .as_mut()
            .unwrap()
            .closed_at_height = "630".into();
        assert!(!response.matches(&network, id));
        response.record.as_mut().unwrap().deadline_closure = None;
        response.current_height = "50".into();
        response.record.as_mut().unwrap().proposal.expected_revision = 1;
        assert!(!response.matches(&network, id));
    }

    #[test]
    fn head_reads_reject_impossible_height_and_zero_provider_scope() {
        let network = network(1);
        let account = AccountId::new(KeyPair::random().public_key().clone());
        let mut head = SorafsAuthorityHeadProjectionV1 {
            plan_id: hex::encode([4; 32]),
            term: "1".into(),
            expires_at_height: "1281".into(),
            authority: SorafsOperatingAuthorityV1::SetReservePolicy,
            account,
            revision: "1".into(),
            granted: true,
            proposal_content_id: ProposalContentId::new([1; 32]),
            governance_attempt_id: GovernanceAttemptId::new([2; 32]),
            certificate_id: GovernanceCertificateId::new([3; 32]),
            enacted_at_height: "40".into(),
        };
        for authority in [
            SorafsOperatingAuthorityV1::SetReservePolicy,
            SorafsOperatingAuthorityV1::CompleteReplicationOrder {
                provider_id: iroha_data_model::sorafs::capacity::ProviderId::new([0; 32]),
            },
        ] {
            head.authority = authority;
            let subject = SorafsAuthorityProposalV1::subject_id(
                &network,
                iroha_data_model::runtime::lifecycle::RootScopeV1::Universal,
                authority,
                &head.account,
            );
            for height in ["0", "40", "51", "040"] {
                head.enacted_at_height = height.into();
                let response = SorafsAuthorityHeadReadV1 {
                    network,
                    subject_id: hex::encode(subject),
                    current_height: "50".into(),
                    head: Some(head.clone()),
                };
                assert_eq!(
                    response.matches(&network, &subject),
                    height == "40" && authority == SorafsOperatingAuthorityV1::SetReservePolicy
                );
            }
        }
    }
    #[test]
    fn lease_reads_bind_both_votes_and_distinguish_approval_from_execution() {
        let network = network(1);
        let plan = SorafsAuthorityProposalV1 {
            network,
            root: iroha_data_model::runtime::lifecycle::RootScopeV1::Universal,
            account: AccountId::new(KeyPair::random().public_key().clone()),
            authority: SorafsOperatingAuthorityV1::SetReservePolicy,
            action: SorafsAuthorityActionV1::Grant,
            grid: iroha_data_model::isi::sorafs_authority::SorafsAuthorityTermGridV1 {
                epoch_length_blocks: 10,
            },
            term: 1,
            expected_revision: 0,
            completion_policy: None,
        };
        let approval = |action| {
            let mut decision = plan.clone();
            decision.action = action;
            SorafsAuthorityApprovalProjectionV1 {
                proposal_content_id: decision.proposal_content_id(),
                governance_attempt_id: GovernanceAttemptId::new([2; 32]),
                certificate_id: GovernanceCertificateId::new([3; 32]),
                approved_at_height: "40".into(),
            }
        };
        let grant = approval(SorafsAuthorityActionV1::Grant);
        let terminal = approval(SorafsAuthorityActionV1::AuthorizeTerminalRevoke);
        let activation = SorafsAuthorityExecutionProjectionV1 {
            revision: "1".into(),
            at_height: "641".into(),
            approval: grant.clone(),
            services_profile: DefinitionId([4; 32]),
            implementation: DefinitionId([5; 32]),
        };
        let plan_id = plan.plan_id();
        let mut response = SorafsAuthorityLeaseReadV1 {
            network,
            plan_id: hex::encode(plan_id),
            current_height: "641".into(),
            lease: Some(SorafsAuthorityLeaseProjectionV1 {
                plan,
                grant_approval: Some(grant),
                terminal_approval: Some(terminal.clone()),
                activation: Some(activation.clone()),
                completion: None,
                start_processed: true,
            }),
        };
        assert!(response.matches(&network, &plan_id));
        assert!(!response.matches(&network, &[8; 32]));
        assert!(!response.matches(&self::network(9), &plan_id));
        let valid = response.clone();
        response.lease.as_mut().unwrap().grant_approval = None;
        response.lease.as_mut().unwrap().terminal_approval = None;
        response.lease.as_mut().unwrap().activation = None;
        assert!(
            !response.matches(&network, &plan_id),
            "an orphan plan has no authorization provenance"
        );
        response = valid.clone();
        response.lease.as_mut().unwrap().terminal_approval =
            response.lease.as_ref().unwrap().grant_approval.clone();
        assert!(!response.matches(&network, &plan_id));
        response = valid.clone();
        response.lease.as_mut().unwrap().plan.expected_revision += 1;
        assert!(!response.matches(&network, &plan_id));
        response = valid.clone();
        response
            .lease
            .as_mut()
            .unwrap()
            .terminal_approval
            .as_mut()
            .unwrap()
            .approved_at_height = "631".into();
        assert!(!response.matches(&network, &plan_id));
        response = valid;
        response.current_height = "1281".into();
        assert!(!response.matches(&network, &plan_id));
        response.lease.as_mut().unwrap().completion = Some(SorafsAuthorityExecutionProjectionV1 {
            revision: "2".into(),
            at_height: "1281".into(),
            approval: terminal,
            ..activation
        });
        assert!(response.matches(&network, &plan_id));
        assert_eq!(
            norito::json::from_value::<SorafsAuthorityLeaseReadV1>(
                norito::json::to_value(&response).unwrap()
            )
            .unwrap(),
            response
        );
        for field in [
            "grant_approval",
            "terminal_approval",
            "activation",
            "completion",
        ] {
            let mut encoded = norito::json::to_value(&response).unwrap();
            encoded
                .as_object_mut()
                .unwrap()
                .get_mut("lease")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert!(norito::json::from_value::<SorafsAuthorityLeaseReadV1>(encoded).is_err());
        }
        let mut absent = SorafsAuthorityLeaseReadV1 {
            network,
            plan_id: hex::encode(plan_id),
            current_height: "0".into(),
            lease: None,
        };
        assert!(absent.matches(&network, &plan_id));
        absent.current_height = "00".into();
        assert!(!absent.matches(&network, &plan_id));
        let mut encoded = norito::json::to_value(&absent).unwrap();
        encoded.as_object_mut().unwrap().remove("lease");
        assert!(norito::json::from_value::<SorafsAuthorityLeaseReadV1>(encoded).is_err());
    }
}
