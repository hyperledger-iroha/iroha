//! Exact authority-host observations, including retained receipts after service retirement.

use super::{Arc, JsonBody, parse_exact_lower_hex32_path};
use iroha_core::state::{GovernanceProposalStatus, State, StateReadOnly, WorldReadOnly};
use iroha_data_model::governance::types::ProposalContentId;
use iroha_torii_shared::sorafs_authority_api::{
    SorafsAuthorityApprovalProjectionV1, SorafsAuthorityDeadlineClosureProjectionV1,
    SorafsAuthorityExecutionProjectionV1, SorafsAuthorityHeadProjectionV1,
    SorafsAuthorityHeadReadV1, SorafsAuthorityLeaseProjectionV1, SorafsAuthorityLeaseReadV1,
    SorafsAuthorityProposalProjectionV1, SorafsAuthorityProposalReadV1,
    SorafsAuthorityProposalStatusV1,
};
use mv::storage::StorageReadOnly;

/// Read one hosted proposal from one committed view, preserving typed absence.
pub(crate) async fn proposal(
    state: Arc<State>,
    id: String,
) -> Result<JsonBody<SorafsAuthorityProposalReadV1>, crate::Error> {
    let id = parse_exact_lower_hex32_path("SoraFS authority proposal id", &id)?;
    let view = state.query_view();
    let record = view
        .world()
        .sorafs_authority_proposals()
        .get(&id)
        .map(|record| SorafsAuthorityProposalProjectionV1 {
            proposer: record.proposer.clone(),
            proposal: record.proposal.clone(),
            created_height: record.created_height.to_string(),
            deadline_closure: record.deadline_closure.map(|closure| {
                SorafsAuthorityDeadlineClosureProjectionV1 {
                    closed_at_height: closure.closed_at_height.to_string(),
                }
            }),
            status: match record.status {
                GovernanceProposalStatus::Proposed => SorafsAuthorityProposalStatusV1::Proposed,
                GovernanceProposalStatus::Rejected => SorafsAuthorityProposalStatusV1::Rejected,
                GovernanceProposalStatus::Enacted => SorafsAuthorityProposalStatusV1::Enacted,
                GovernanceProposalStatus::Superseded => SorafsAuthorityProposalStatusV1::Superseded,
                GovernanceProposalStatus::ExecutionFailed => {
                    SorafsAuthorityProposalStatusV1::ExecutionFailed
                }
            },
        });
    Ok(JsonBody(SorafsAuthorityProposalReadV1 {
        network: *view.network_id(),
        proposal_content_id: ProposalContentId::new(id),
        current_height: view.height().to_string(),
        record,
    }))
}

/// Read the exact authority revision and last successful receipt, including revoked heads.
pub(crate) async fn head(
    state: Arc<State>,
    id: String,
) -> Result<JsonBody<SorafsAuthorityHeadReadV1>, crate::Error> {
    let subject = parse_exact_lower_hex32_path("SoraFS authority subject id", &id)?;
    let view = state.query_view();
    let head = view
        .world()
        .sorafs_authority_heads()
        .get(&subject)
        .map(|head| SorafsAuthorityHeadProjectionV1 {
            plan_id: hex::encode(head.plan_id),
            term: head.term.to_string(),
            expires_at_height: head.expires_at_height.to_string(),
            authority: head.authority,
            account: head.account.clone(),
            revision: head.revision.to_string(),
            granted: head.granted,
            proposal_content_id: head.proposal_content_id,
            governance_attempt_id: head.governance_attempt_id,
            certificate_id: head.certificate_id,
            enacted_at_height: head.enacted_at_height.to_string(),
        });
    Ok(JsonBody(SorafsAuthorityHeadReadV1 {
        network: *view.network_id(),
        subject_id: id,
        current_height: view.height().to_string(),
        head,
    }))
}

/// Read independently approved mandates and exact effects for one normalized lease plan.
pub(crate) async fn lease(
    state: Arc<State>,
    id: String,
) -> Result<JsonBody<SorafsAuthorityLeaseReadV1>, crate::Error> {
    let plan_id = parse_exact_lower_hex32_path("SoraFS authority plan id", &id)?;
    let view = state.query_view();
    let lease = view
        .world()
        .sorafs_authority_leases()
        .get(&plan_id)
        .map(|lease| SorafsAuthorityLeaseProjectionV1 {
            plan: lease.plan.clone(),
            grant_approval: lease.grant_approval.as_ref().map(|approval| {
                SorafsAuthorityApprovalProjectionV1 {
                    proposal_content_id: approval.proposal_content_id,
                    governance_attempt_id: approval.governance_attempt_id,
                    certificate_id: approval.certificate_id,
                    approved_at_height: approval.approved_at_height.to_string(),
                }
            }),
            terminal_approval: lease.terminal_approval.as_ref().map(|approval| {
                SorafsAuthorityApprovalProjectionV1 {
                    proposal_content_id: approval.proposal_content_id,
                    governance_attempt_id: approval.governance_attempt_id,
                    certificate_id: approval.certificate_id,
                    approved_at_height: approval.approved_at_height.to_string(),
                }
            }),
            activation: lease.activation.as_ref().map(|execution| {
                SorafsAuthorityExecutionProjectionV1 {
                    revision: execution.revision.to_string(),
                    at_height: execution.at_height.to_string(),
                    approval: SorafsAuthorityApprovalProjectionV1 {
                        proposal_content_id: execution.approval.proposal_content_id,
                        governance_attempt_id: execution.approval.governance_attempt_id,
                        certificate_id: execution.approval.certificate_id,
                        approved_at_height: execution.approval.approved_at_height.to_string(),
                    },
                    services_profile: execution.services_profile,
                    implementation: execution.implementation,
                }
            }),
            completion: lease.completion.as_ref().map(|execution| {
                SorafsAuthorityExecutionProjectionV1 {
                    revision: execution.revision.to_string(),
                    at_height: execution.at_height.to_string(),
                    approval: SorafsAuthorityApprovalProjectionV1 {
                        proposal_content_id: execution.approval.proposal_content_id,
                        governance_attempt_id: execution.approval.governance_attempt_id,
                        certificate_id: execution.approval.certificate_id,
                        approved_at_height: execution.approval.approved_at_height.to_string(),
                    },
                    services_profile: execution.services_profile,
                    implementation: execution.implementation,
                }
            }),
            start_processed: lease.start_processed,
        });
    Ok(JsonBody(SorafsAuthorityLeaseReadV1 {
        network: *view.network_id(),
        plan_id: id,
        current_height: view.height().to_string(),
        lease,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_core::{kura::Kura, query::store::LiveQueryStore, state::World};

    #[tokio::test]
    async fn inactive_host_preserves_typed_absence_and_rejects_noncanonical_ids() {
        let state = Arc::new(State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        let absent = proposal(Arc::clone(&state), hex::encode([1; 32]))
            .await
            .unwrap()
            .0;
        assert!(absent.record.is_none());
        assert!(absent.matches(&state.network_id, ProposalContentId::new([1; 32])));
        let absent = head(Arc::clone(&state), hex::encode([1; 32]))
            .await
            .unwrap()
            .0;
        assert!(absent.head.is_none());
        assert!(absent.matches(&state.network_id, &[1; 32]));
        let absent = lease(Arc::clone(&state), hex::encode([1; 32]))
            .await
            .unwrap()
            .0;
        assert!(absent.lease.is_none());
        assert!(absent.matches(&state.network_id, &[1; 32]));
        for id in ["../other".into(), "AA".repeat(32)] {
            assert!(proposal(Arc::clone(&state), id.clone()).await.is_err());
            assert!(head(Arc::clone(&state), id.clone()).await.is_err());
            assert!(lease(Arc::clone(&state), id).await.is_err());
        }
    }

    #[tokio::test]
    async fn historical_hosted_proposal_is_readable_without_changing_inactive_ballots() {
        use iroha_core::state::WorldReadOnly;
        use iroha_data_model::{
            block::BlockHeader,
            isi::sorafs_authority::{
                SorafsAuthorityActionV1, SorafsAuthorityProposalV1, SorafsOperatingAuthorityV1,
            },
            runtime::{
                definition::DefinitionDescriptorV1,
                lifecycle::{RootScopeV1, SelectionTransitionV1},
            },
        };
        let state = Arc::new(State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        let proposal_content = SorafsAuthorityProposalV1 {
            network: state.network_id,
            root: RootScopeV1::Universal,
            account: iroha_test_samples::ALICE_ID.clone(),
            authority: SorafsOperatingAuthorityV1::SetReservePolicy,
            action: SorafsAuthorityActionV1::Grant,
            grid: iroha_data_model::isi::sorafs_authority::SorafsAuthorityTermGridV1 {
                epoch_length_blocks: 10,
            },
            term: 1,
            expected_revision: 0,
            completion_policy: None,
        };
        let id = proposal_content.proposal_content_id();
        let (history_change, selected) = {
            let world = state.world_view();
            let change = iroha_core::runtime::draft_addition(
                &world,
                state.network_id,
                "iroha.sorafs.authority_governance.v1",
                1,
                [1; 32],
            )
            .unwrap();
            let published: std::collections::BTreeMap<_, _> = world
                .runtime_definitions()
                .iter()
                .map(|(id, definition)| (*id, definition.clone()))
                .collect();
            let applied = change.apply(world.runtime_selection(), &published).unwrap();
            // Explicit read fixture: retained tables with inactive host services.
            // This does not claim a historical Parliament vote or retirement proof.
            let mut retained = world.runtime_selection().clone();
            retained.installed_storage = applied.selection.installed_storage.clone();
            let mut history_change = change;
            history_change.successor = retained.id().unwrap();
            history_change
                .publish
                .retain(|definition| matches!(definition, DefinitionDescriptorV1::Storage(_)));
            history_change.transitions.retain(|transition| {
                matches!(transition, SelectionTransitionV1::InstallStorage(_))
            });
            history_change
                .apply(world.runtime_selection(), &published)
                .unwrap();
            (history_change, applied.selection)
        };
        let mut block = state.block(BlockHeader::new(
            std::num::NonZeroU64::new(1).unwrap(),
            None,
            None,
            0,
            0,
        ));
        let mut tx = block.transaction();
        tx.world_mut_for_testing()
            .apply_runtime_change_set_for_testing(&history_change)
            .unwrap();
        // The public storage accessor fixes the private Core record type for decoding.
        let record = norito::json::from_value(norito::json!({
            "proposer":(iroha_test_samples::ALICE_ID.clone()),"proposal":(proposal_content.clone()),
            "created_height":1_u64,"status":"Rejected","deadline_closure":null
        }))
        .unwrap();
        tx.world_mut_for_testing()
            .sorafs_authority_proposals_mut()
            .insert(*id.as_bytes(), record);
        tx.apply();
        block.commit_empty_block_for_testing().unwrap();
        let response = proposal(Arc::clone(&state), id.to_hex()).await.unwrap().0;
        assert!(response.matches(&state.network_id, id));
        assert_eq!(response.network, state.network_id);
        assert_eq!(response.proposal_content_id, id);
        assert_eq!(response.record.unwrap().proposal, proposal_content);
        let world = state.world_view();
        for selector in [
            id.to_hex(),
            id.to_hex().to_ascii_uppercase(),
            format!("0x{}", id.to_hex()),
        ] {
            assert!(!super::super::is_stored_typed_proposal_fingerprint(
                &world, &selector
            ));
            assert!(super::super::reject_typed_proposal_ballot_selector(&world, &selector).is_ok());
        }
        assert!(world.governance_proposals().get(id.as_bytes()).is_none());

        // Classification compares the compiled successor without activating it.
        assert!(!super::super::selected_sorafs_authority_proposal(
            world.runtime_selection(),
            true
        ));
        assert!(super::super::selected_sorafs_authority_proposal(
            &selected, true
        ));
        assert!(!super::super::selected_sorafs_authority_proposal(
            &selected, false
        ));
    }
}
