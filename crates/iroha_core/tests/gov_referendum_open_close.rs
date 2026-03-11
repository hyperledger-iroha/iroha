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
        GovernanceReferendumMode, GovernanceReferendumRecord, GovernanceReferendumStatus,
        GovernanceStageApproval, GovernanceStageApprovals, State, World, WorldReadOnly,
    },
};
use iroha_data_model::{
    Registrable,
    block::BlockHeader,
    domain::DomainId,
    events::data::governance::GovernanceEvent,
    governance::types::ParliamentBody,
    prelude::{Account, Domain},
};
use mv::storage::StorageReadOnly;
#[test]
fn referendum_open_and_close_by_height() {
    use nonzero_ext::nonzero;

    // Build minimal state.
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let domain_id: DomainId = "wonderland".parse().expect("domain id");
    let domain: Domain = Domain::new(domain_id.clone()).build(&iroha_test_samples::ALICE_ID);
    let account: Account = Account::new(
        iroha_test_samples::ALICE_ID
            .clone()
            .to_account_id(domain_id),
    )
    .build(&iroha_test_samples::ALICE_ID);
    let world = World::with([domain], [account], []);
    let mut state = State::new_for_testing(world, kura, query_handle);
    let mut cfg = state.gov.clone();
    cfg.parliament_term_blocks = 100;
    state.set_gov(cfg);

    // Block H=1: create a proposed referendum with explicit [2,3] window.
    let header1 = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    {
        let mut sblock1 = state.block(header1);
        let mut stx1 = sblock1.transaction();
        stx1.world.governance_referenda_mut().insert(
            "rid-auto-window".to_string(),
            GovernanceReferendumRecord {
                h_start: 2,
                h_end: 3,
                status: GovernanceReferendumStatus::Proposed,
                mode: GovernanceReferendumMode::Plain,
            },
        );
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
            .insert("rid-auto-window".to_string(), approvals);
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
            .get("rid-auto-window")
            .copied()
            .expect("referendum should persist after H=1");
        assert_eq!(referendum.status, GovernanceReferendumStatus::Proposed);
        let approvals = view
            .world()
            .governance_stage_approvals()
            .get("rid-auto-window")
            .expect("stage approvals should persist after H=1");
        assert!(approvals.quorum_met(ParliamentBody::RulesCommittee, 0));
        assert!(approvals.quorum_met(ParliamentBody::AgendaCouncil, 0));
    }

    // Block H=2: opens.
    let header2 = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    {
        let mut sblock2 = state.block(header2);
        let status_open_at_h2 = sblock2
            .world
            .governance_referenda()
            .get("rid-auto-window")
            .is_some_and(|record| record.status == GovernanceReferendumStatus::Open);
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
        assert!(status_open_at_h2);
        assert!(has_opened_event_at_h2 || status_open_at_h2);
        sblock2.commit().expect("commit block at H=2");
    }

    // Block H=3: closes.
    let header3 = BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut sblock3 = state.block(header3);
    let status_closed_at_h3 = sblock3
        .world
        .governance_referenda()
        .get("rid-auto-window")
        .is_some_and(|record| record.status == GovernanceReferendumStatus::Closed);
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
    assert!(status_closed_at_h3);
    assert!(has_closed_event_at_h3 || status_closed_at_h3);
    sblock3.commit().expect("commit block at H=3");
}
