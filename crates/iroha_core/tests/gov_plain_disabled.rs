//! Plain voting disabled policy gate test.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
use core::num::NonZeroU64;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World},
};
use iroha_data_model::{
    Registrable, account::Account, events::data::governance::GovernanceEvent, isi::Grant,
    isi::governance::CastPlainBallot, permission::Permission,
};
use iroha_executor_data_model::permission::governance::CanSubmitGovernanceBallot;
use iroha_test_samples::ALICE_ID;
#[test]
fn plain_ballot_rejected_when_disabled() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let world = World::with([], [Account::new(ALICE_ID.clone()).build(&ALICE_ID)], []);
    let mut state = State::new_for_testing(world, kura, query_handle);
    // Disable plain voting
    let mut cfg = state.gov.clone();
    cfg.plain_voting_enabled = false;
    cfg.min_bond_amount = 0_u64.into();
    state.set_gov(cfg);
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(1).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();
    let permission: Permission = CanSubmitGovernanceBallot {
        referendum_id: "rid-disabled".into(),
    }
    .into();
    Grant::account_permission(permission, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant exact ballot permission");
    let instr = CastPlainBallot {
        referendum_id: "rid-disabled".to_string(),
        direction: 0,
        owner: ALICE_ID.clone(),
        amount: 1000_u64.into(),
        duration_blocks: 10,
    };
    let err = instr.execute(&ALICE_ID, &mut stx).unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("plain voting mode disabled"));
    // Check BallotRejected was emitted
    stx.apply();
    let events = sblock.world.take_external_events();
    assert!(events.iter().any(|e| matches!(
        e,
        iroha_data_model::events::EventBox::Data(ev)
            if matches!(
                ev.as_ref(),
                iroha_data_model::events::data::DataEvent::Governance(
                    GovernanceEvent::BallotRejected(_)
                )
            )
    )));
}
