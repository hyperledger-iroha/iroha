//! Plain ballot must fail when referendum is missing or closed.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World},
};
use iroha_data_model::{block::BlockHeader, isi::governance::CastPlainBallot};
use iroha_test_samples::ALICE_ID;
use nonzero_ext::nonzero;

#[test]
fn plain_ballot_rejected_when_referendum_absent_or_closed() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: plain governance missing-ref test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query_handle);
    let mut gov_cfg = state.gov.clone();
    gov_cfg.plain_voting_enabled = true;
    gov_cfg.min_bond_amount = 0;
    state.set_gov(gov_cfg);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();

    // No referendum exists
    let ballot = CastPlainBallot {
        referendum_id: "missing".to_string(),
        owner: ALICE_ID.clone(),
        amount: 10,
        duration_blocks: 10,
        direction: 0,
    };
    let err = ballot
        .clone()
        .execute(&ALICE_ID, &mut stx)
        .expect_err("ballot should fail when referendum is absent");
    assert!(
        err.to_string().contains("referendum"),
        "unexpected error: {err}"
    );

    // Insert a closed referendum and ensure rejection
    stx.world.governance_referenda_mut().insert(
        "closed".to_string(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 0,
            h_end: 0,
            status: iroha_core::state::GovernanceReferendumStatus::Closed,
            mode: iroha_core::state::GovernanceReferendumMode::Plain,
        },
    );
    let ballot_closed = CastPlainBallot {
        referendum_id: "closed".to_string(),
        owner: ALICE_ID.clone(),
        amount: 10,
        duration_blocks: 10,
        direction: 0,
    };
    let err_closed = ballot_closed
        .execute(&ALICE_ID, &mut stx)
        .expect_err("ballot should fail when referendum is closed");
    assert!(
        err_closed.to_string().contains("closed"),
        "unexpected error: {err_closed}"
    );
}
