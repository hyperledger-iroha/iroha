//! Plain ballots opened in the same block should emit ReferendumOpened with the correct window.

use core::num::NonZeroU64;

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{
        GovernanceReferendumMode, GovernanceReferendumRecord, GovernanceReferendumStatus, State,
        World,
    },
};
use iroha_data_model::{
    block::BlockHeader,
    events::data::{governance::GovernanceEvent, DataEvent},
    isi::governance::CastPlainBallot,
    permission::Permission,
    prelude::Grant,
};
use iroha_executor_data_model::permission::governance::CanSubmitGovernanceBallot;
use iroha_test_samples::ALICE_ID;

#[test]
fn plain_ballot_emits_open_event_with_window() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query);
    let mut cfg = state.gov.clone();
    cfg.plain_voting_enabled = true;
    cfg.min_bond_amount = 0;
    cfg.conviction_step_blocks = 1;
    state.set_gov(cfg);

    let rid = "plain-open-event".to_string();
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();

    let ballot_perm: Permission = CanSubmitGovernanceBallot {
        referendum_id: "any".into(),
    }
    .into();
    Grant::account_permission(ballot_perm, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant ballot permission");

    stx.world.governance_referenda_mut().insert(
        rid.clone(),
        GovernanceReferendumRecord {
            h_start: 1,
            h_end: 5,
            status: GovernanceReferendumStatus::Proposed,
            mode: GovernanceReferendumMode::Plain,
        },
    );

    CastPlainBallot {
        referendum_id: rid.clone(),
        owner: ALICE_ID.clone(),
        amount: 1,
        duration_blocks: 1,
        direction: 0,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("ballot ok");
    stx.apply();

    let events = sblock.world.take_external_events();
    let opened = events.iter().find_map(|event| match event.as_data_event() {
        Some(DataEvent::Governance(GovernanceEvent::ReferendumOpened(opened))) => {
            Some(opened.clone())
        }
        _ => None,
    });
    let opened = opened.expect("expected ReferendumOpened event");
    assert_eq!(opened.h_start, 1);
    assert_eq!(opened.h_end, 5);
}
