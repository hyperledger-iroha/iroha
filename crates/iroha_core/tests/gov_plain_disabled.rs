//! Plain voting disabled policy gate test.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Skipped by default; enable with `IROHA_RUN_IGNORED=1`.

use core::num::NonZeroU64;

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World},
};
use iroha_data_model::{
    events::data::governance::GovernanceEvent, isi::governance::CastPlainBallot,
};
use iroha_test_samples::ALICE_ID;

#[test]
fn plain_ballot_rejected_when_disabled() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: plain disabled test gated. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query_handle);
    // Disable plain voting
    let mut cfg = state.gov.clone();
    cfg.plain_voting_enabled = false;
    cfg.min_bond_amount = 0;
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

    let instr = CastPlainBallot {
        referendum_id: "rid-disabled".to_string(),
        owner: ALICE_ID.clone(),
        amount: 1000,
        duration_blocks: 10,
        direction: 0,
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
