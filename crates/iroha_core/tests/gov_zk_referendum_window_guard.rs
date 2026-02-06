#![doc = "Governance ZK ballot must fail when referendum is missing or outside window."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(all(feature = "zk-tests", feature = "halo2-dev-tests"))]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World},
};
use iroha_data_model::{block::BlockHeader, isi::governance::CastZkBallot};
use iroha_test_samples::ALICE_ID;
use nonzero_ext::nonzero;

#[test]
fn zk_ballot_rejected_when_referendum_absent_or_out_of_window() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: zk referendum window guard gated. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query_handle);
    let mut gov_cfg = state.gov.clone();
    gov_cfg.min_bond_amount = 0;
    gov_cfg.min_enactment_delay = 0;
    gov_cfg.window_span = 10;
    state.set_gov(gov_cfg);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();

    // Missing referendum
    let ballot = CastZkBallot {
        election_id: "missing".to_string(),
        proof_b64: "AA==".to_string(),
        public_inputs_json: "{}".to_string(),
    };
    let err = ballot
        .clone()
        .execute(&ALICE_ID, &mut stx)
        .expect_err("ballot should fail when referendum missing");
    assert!(
        err.to_string().contains("referendum"),
        "unexpected error: {err}"
    );

    // Closed referendum
    stx.world.governance_referenda_mut().insert(
        "closed".to_string(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 0,
            h_end: 0,
            status: iroha_core::state::GovernanceReferendumStatus::Closed,
            mode: iroha_core::state::GovernanceReferendumMode::Zk,
        },
    );
    let ballot_closed = CastZkBallot {
        election_id: "closed".to_string(),
        proof_b64: "AA==".to_string(),
        public_inputs_json: "{}".to_string(),
    };
    let err_closed = ballot_closed
        .execute(&ALICE_ID, &mut stx)
        .expect_err("ballot should fail when referendum closed");
    assert!(
        err_closed.to_string().contains("closed"),
        "unexpected error: {err_closed}"
    );

    // Outside window
    stx.world.governance_referenda_mut().insert(
        "late".to_string(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 5,
            h_end: 6,
            status: iroha_core::state::GovernanceReferendumStatus::Proposed,
            mode: iroha_core::state::GovernanceReferendumMode::Zk,
        },
    );
    let header_late = BlockHeader::new(nonzero!(10_u64), None, None, None, 0, 0);
    let mut sblock_late = state.block(header_late);
    let mut stx_late = sblock_late.transaction();
    // Reinsert referendum into late state snapshot
    stx_late.world.governance_referenda_mut().insert(
        "late".to_string(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 5,
            h_end: 6,
            status: iroha_core::state::GovernanceReferendumStatus::Proposed,
            mode: iroha_core::state::GovernanceReferendumMode::Zk,
        },
    );
    let err_late = ballot
        .execute(&ALICE_ID, &mut stx_late)
        .expect_err("ballot should fail when outside window");
    assert!(
        err_late.to_string().contains("not active") || err_late.to_string().contains("referendum"),
        "unexpected error: {err_late}"
    );
}
