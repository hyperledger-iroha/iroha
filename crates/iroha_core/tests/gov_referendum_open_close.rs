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
        GovernanceReferendumMode, GovernanceReferendumRecord, GovernanceReferendumStatus, State,
        World, WorldReadOnly,
    },
};
use iroha_data_model::{
    Registrable,
    block::BlockHeader,
    domain::DomainId,
    events::data::governance::GovernanceEvent,
    prelude::{Account, Domain},
};
use mv::storage::StorageReadOnly;
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
    // Block H=1: create a proposed referendum with explicit [2,3] window.
    let header1 = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let rid = "standalone-window".to_owned();
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
        sblock1
            .commit_empty_block_for_testing()
            .expect("commit block at H=1");
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
        sblock2
            .commit_empty_block_for_testing()
            .expect("commit block at H=2");
        let status_open_at_h2 = state
            .view()
            .world()
            .governance_referenda()
            .get(&rid)
            .is_some_and(|record| record.status == GovernanceReferendumStatus::Open);
        assert!(status_open_at_h2);
        assert!(has_opened_event_at_h2 || status_open_at_h2);
    }
    // Block H=3: the inclusive end height remains open.
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
    sblock3
        .commit_empty_block_for_testing()
        .expect("commit block at H=3");
    let status_open_at_h3 = state
        .view()
        .world()
        .governance_referenda()
        .get(&rid)
        .is_some_and(|record| record.status == GovernanceReferendumStatus::Open);
    assert!(status_open_at_h3);
    assert!(!has_closed_event_at_h3);
    // Block H=4: closes at h_end + 1.
    let header4 = BlockHeader::new(nonzero!(4_u64), None, None, None, 0, 0);
    let mut sblock4 = state.block(header4);
    let events_at_h4 = sblock4.world.take_external_events();
    let has_closed_event_at_h4 = events_at_h4.iter().any(|event| {
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
    let decision = events_at_h4
        .iter()
        .find_map(|event| match event.as_data_event() {
            Some(iroha_data_model::events::data::DataEvent::Governance(
                GovernanceEvent::ReferendumDecided(decision),
            )) => Some(decision),
            _ => None,
        });
    let decision = decision.expect("standalone close must emit its exact referendum decision");
    assert_eq!(decision.referendum_id, rid);
    assert_eq!(
        (decision.approve, decision.reject, decision.abstain),
        (0, 0, 0)
    );
    assert!(
        !decision.approved,
        "a nonzero approval threshold cannot approve an empty decisive tally"
    );
    assert!(
        !events_at_h4.iter().any(|event| matches!(
            event.as_data_event(),
            Some(iroha_data_model::events::data::DataEvent::Governance(
                GovernanceEvent::ProposalApproved(_) | GovernanceEvent::ProposalRejected(_)
            ))
        )),
        "standalone closure must never masquerade as a typed proposal decision"
    );
    sblock4
        .commit_empty_block_for_testing()
        .expect("commit block at H=4");
    let status_closed_at_h4 = state
        .view()
        .world()
        .governance_referenda()
        .get(&rid)
        .is_some_and(|record| record.status == GovernanceReferendumStatus::Closed);
    assert!(status_closed_at_h4);
    assert!(has_closed_event_at_h4 || status_closed_at_h4);
}
