//! Enforce minimum lock duration for plain ballots.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World},
};
use iroha_data_model::{
    block::BlockHeader, events::data::governance::GovernanceEvent, isi::governance::CastPlainBallot,
};
use iroha_test_samples::ALICE_ID;
use nonzero_ext::nonzero;

#[test]
fn plain_ballot_rejected_when_duration_below_min() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query_handle);
    // Set min step > requested duration
    let mut cfg = state.gov.clone();
    cfg.plain_voting_enabled = true;
    cfg.min_bond_amount = 0;
    cfg.conviction_step_blocks = 100;
    state.set_gov(cfg);

    let _kp = iroha_crypto::KeyPair::random();
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();

    let instr = CastPlainBallot {
        referendum_id: "rid-min-dur".to_string(),
        owner: ALICE_ID.clone(),
        amount: 100,
        duration_blocks: 10, // below min
        direction: 0,
    };
    let err = instr.execute(&ALICE_ID, &mut stx).unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("lock duration shorter"));
    let evs = stx.world.take_external_events();
    assert!(evs.iter().any(|e| matches!(
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
