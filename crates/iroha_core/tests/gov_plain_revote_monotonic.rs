//! Plain ballot re-vote monotonicity tests (extend-only and owner check).
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Skipped by default; enable with `IROHA_RUN_IGNORED=1`.

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World},
};
use iroha_data_model::{
    block::BlockHeader,
    events::data::{DataEvent, governance::GovernanceEvent},
    isi::governance::CastPlainBallot,
};
use iroha_test_samples::{ALICE_ID, BOB_ID};
use nonzero_ext::nonzero;

#[test]
fn plain_ballot_revotes_extend_only_and_owner_matches() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: governance re-vote monotonicity test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }

    // Minimal state
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query_handle);
    let mut gov_cfg = state.gov.clone();
    gov_cfg.plain_voting_enabled = true;
    gov_cfg.min_bond_amount = 0;
    state.set_gov(gov_cfg);

    // Build a signed block header at H=1
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();

    // First vote by ALICE
    let rid = "rid-revote".to_string();
    let first = CastPlainBallot {
        referendum_id: rid.clone(),
        owner: ALICE_ID.clone(),
        amount: 100,
        duration_blocks: 200,
        direction: 0,
    };
    first
        .execute(&ALICE_ID, &mut stx)
        .expect("first ballot should succeed");
    let events = stx.world.take_external_events();
    assert!(events.iter().any(|event| matches!(
        event.as_data_event(),
        Some(DataEvent::Governance(GovernanceEvent::LockCreated(_)))
    )));

    // Re-vote with shorter duration should be rejected
    let shorter = CastPlainBallot {
        referendum_id: rid.clone(),
        owner: ALICE_ID.clone(),
        amount: 100,
        duration_blocks: 10,
        direction: 0,
    };
    let err = shorter.execute(&ALICE_ID, &mut stx).unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("re-vote cannot reduce"));
    let evs_rej = stx.world.take_external_events();
    assert!(evs_rej.iter().any(|event| matches!(
        event.as_data_event(),
        Some(DataEvent::Governance(GovernanceEvent::BallotRejected(_)))
    )));

    // Re-vote with smaller amount should be rejected
    let smaller = CastPlainBallot {
        referendum_id: rid.clone(),
        owner: ALICE_ID.clone(),
        amount: 50,
        duration_blocks: 200,
        direction: 0,
    };
    let err2 = smaller.execute(&ALICE_ID, &mut stx).unwrap_err();
    let s2 = format!("{err2}");
    assert!(s2.contains("re-vote cannot reduce"));

    // Re-vote with longer duration (extend) should work and emit LockExtended
    let extend = CastPlainBallot {
        referendum_id: rid.clone(),
        owner: ALICE_ID.clone(),
        amount: 120,
        duration_blocks: 400,
        direction: 0,
    };
    extend
        .execute(&ALICE_ID, &mut stx)
        .expect("extend should succeed");
    let evs_ext = stx.world.take_external_events();
    assert!(evs_ext.iter().any(|event| matches!(
        event.as_data_event(),
        Some(DataEvent::Governance(GovernanceEvent::LockExtended(_)))
    )));

    // Owner mismatch must be rejected
    let mismatch = CastPlainBallot {
        referendum_id: rid,
        owner: BOB_ID.clone(),
        amount: 200,
        duration_blocks: 100,
        direction: 1,
    };
    let err3 = mismatch.execute(&ALICE_ID, &mut stx).unwrap_err();
    let s3 = format!("{err3}");
    assert!(s3.contains("owner must equal authority"));
}
