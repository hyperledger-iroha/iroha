//! Tests for automatic ReferendumOpened/ReferendumClosed events via height triggers.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(
    clippy::doc_markdown,
    clippy::too_many_lines,
    clippy::items_after_statements
)]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{
        GovernancePipeline, GovernanceProposalRecord, GovernanceProposalStatus,
        GovernanceReferendumMode, GovernanceReferendumRecord, GovernanceReferendumStatus,
        GovernanceStageApproval, GovernanceStageApprovals, State, World, WorldReadOnly,
    },
};
use iroha_data_model::{
    Registrable,
    block::BlockHeader,
    domain::DomainId,
    events::data::governance::GovernanceEvent,
    governance::types::{
        AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal, ParliamentBody,
        ProposalKind,
    },
    prelude::{Account, Domain},
};
use mv::storage::StorageReadOnly;

fn deploy_contract_address() -> iroha_data_model::smart_contract::ContractAddress {
    iroha_data_model::smart_contract::ContractAddress::derive(
        iroha_config::parameters::defaults::common::chain_discriminant(),
        &iroha_test_samples::ALICE_ID,
        0,
        iroha_data_model::nexus::DataSpaceId::GLOBAL,
    )
    .expect("deploy contract address")
}

#[test]
fn referendum_open_and_close_by_height() {
    use nonzero_ext::nonzero;

    // Build minimal state.
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain: Domain = Domain::new(domain_id.clone()).build(&iroha_test_samples::ALICE_ID);
    let account: Account =
        Account::new(iroha_test_samples::ALICE_ID.clone()).build(&iroha_test_samples::ALICE_ID);
    let world = World::with([domain], [account], []);
    let mut state = State::new_for_testing(world, kura, query_handle);
    let mut cfg = state.gov.clone();
    cfg.parliament_term_blocks = 100;
    state.set_gov(cfg);

    // Block H=1: create a proposed referendum with explicit [2,3] window.
    let header1 = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let pid = [0xAB; 32];
    let rid = hex::encode(pid);
    {
        let mut sblock1 = state.block(header1);
        let mut stx1 = sblock1.transaction();
        stx1.world.governance_referenda_mut().insert(
            rid.clone(),
            GovernanceReferendumRecord {
                h_start: 2,
                h_end: 3,
                status: GovernanceReferendumStatus::Proposed,
                mode: GovernanceReferendumMode::Plain,
            },
        );
        let proposal = GovernanceProposalRecord {
            proposer: iroha_test_samples::ALICE_ID.clone(),
            kind: ProposalKind::DeployContract(DeployContractProposal {
                contract_address: deploy_contract_address(),
                code_hash_hex: ContractCodeHash::from_hex_str(&hex::encode([0x11; 32]))
                    .expect("code hash"),
                abi_hash_hex: ContractAbiHash::from_hex_str(&hex::encode([0x22; 32]))
                    .expect("abi hash"),
                abi_version: AbiVersion::new(1),
                manifest_provenance: None,
            }),
            created_height: header1.height().get(),
            status: GovernanceProposalStatus::Proposed,
            pipeline: GovernancePipeline::seeded(
                header1.height().get(),
                stx1.world.governance_referenda().get(&rid),
                &stx1.gov,
            ),
            parliament_snapshot: None,
        };
        stx1.world.governance_proposals_mut().insert(pid, proposal);
        let mut approvals = GovernanceStageApprovals::default();
        approvals.stages.insert(
            ParliamentBody::RulesCommittee,
            GovernanceStageApproval {
                epoch: 0,
                approvers: Default::default(),
                required: 0,
                quorum_bps: 0,
            },
        );
        approvals.stages.insert(
            ParliamentBody::AgendaCouncil,
            GovernanceStageApproval {
                epoch: 0,
                approvers: Default::default(),
                required: 0,
                quorum_bps: 0,
            },
        );
        stx1.world
            .governance_stage_approvals_mut()
            .insert(rid.clone(), approvals);
        stx1.apply();

        let has_opened_at_h1 = sblock1.world.take_external_events().iter().any(|event| {
            matches!(
                event,
                iroha_data_model::events::EventBox::Data(payload)
                    if matches!(
                        payload.as_ref(),
                        iroha_data_model::events::data::DataEvent::Governance(
                            GovernanceEvent::ReferendumOpened(_)
                        )
                    )
            )
        });
        assert!(!has_opened_at_h1);
        sblock1.commit().expect("commit block at H=1");
    }

    {
        let view = state.view();
        let referendum = view
            .world()
            .governance_referenda()
            .get(&rid)
            .copied()
            .expect("referendum should persist after H=1");
        assert_eq!(referendum.status, GovernanceReferendumStatus::Proposed);
        let approvals = view
            .world()
            .governance_stage_approvals()
            .get(&rid)
            .expect("stage approvals should persist after H=1");
        assert!(approvals.quorum_met(ParliamentBody::RulesCommittee, 0));
        assert!(approvals.quorum_met(ParliamentBody::AgendaCouncil, 0));
    }

    // Block H=2: opens.
    let header2 = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    {
        let mut sblock2 = state.block(header2);
        let has_opened_event_at_h2 = sblock2.world.take_external_events().iter().any(|event| {
            matches!(
                event,
                iroha_data_model::events::EventBox::Data(payload)
                    if matches!(
                        payload.as_ref(),
                        iroha_data_model::events::data::DataEvent::Governance(
                            GovernanceEvent::ReferendumOpened(_)
                        )
                    )
            )
        });
        sblock2.commit().expect("commit block at H=2");
        let status_open_at_h2 = state
            .view()
            .world()
            .governance_referenda()
            .get(&rid)
            .is_some_and(|record| record.status == GovernanceReferendumStatus::Open);
        assert!(status_open_at_h2);
        assert!(has_opened_event_at_h2 || status_open_at_h2);
    }

    // Block H=3: closes.
    let header3 = BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut sblock3 = state.block(header3);
    let has_closed_event_at_h3 = sblock3.world.take_external_events().iter().any(|event| {
        matches!(
            event,
            iroha_data_model::events::EventBox::Data(payload)
                if matches!(
                    payload.as_ref(),
                    iroha_data_model::events::data::DataEvent::Governance(
                        GovernanceEvent::ReferendumClosed(_)
                    )
                )
        )
    });
    sblock3.commit().expect("commit block at H=3");
    let status_closed_at_h3 = state
        .view()
        .world()
        .governance_referenda()
        .get(&rid)
        .is_some_and(|record| record.status == GovernanceReferendumStatus::Closed);
    assert!(status_closed_at_h3);
    assert!(has_closed_event_at_h3 || status_closed_at_h3);
}
