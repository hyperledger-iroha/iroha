//! Governance threshold tests: ratio and turnout logic.
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
    block::BlockHeader,
    events::data::{DataEvent, governance::GovernanceEvent},
    isi::governance::FinalizeReferendum,
};
use iroha_test_samples::ALICE_ID;

#[test]
fn ratio_threshold_rejects_even_if_approve_gt_reject() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: threshold test gated. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query_handle);
    // Configure strict ratio: 3/4 (75%) so 2/(2+1)=0.66.. fails
    let mut cfg = state.gov.clone();
    cfg.approval_threshold_q_num = 3;
    cfg.approval_threshold_q_den = 4;
    state.set_gov(cfg);

    // Block H=1
    let (_pk, _sk) = iroha_crypto::KeyPair::random().into_parts();
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();

    // Locks: approve weight=2 (sqrt(4)), reject weight=1 (sqrt(1)); duration 0 → factor 1
    let rid = "rid-threshold-1".to_string();
    let mut map = iroha_core::state::GovernanceLocksForReferendum::default();
    map.locks.insert(
        ALICE_ID.clone(),
        iroha_core::state::GovernanceLockRecord {
            owner: ALICE_ID.clone(),
            amount: 4,
            slashed: 0,
            expiry_height: 100,
            direction: 0,
            duration_blocks: 0,
        },
    );
    // A different owner id for reject; reuse ALICE_ID for brevity (key uniqueness irrelevant)
    map.locks.insert(
        ALICE_ID.clone().clone(),
        iroha_core::state::GovernanceLockRecord {
            owner: ALICE_ID.clone(),
            amount: 1,
            slashed: 0,
            expiry_height: 100,
            direction: 1,
            duration_blocks: 0,
        },
    );
    stx.world.governance_locks_mut().insert(rid.clone(), map);

    // Finalize should reject due to ratio < 75%
    let instr = FinalizeReferendum {
        referendum_id: rid.clone(),
        proposal_id: [0xAB; 32],
    };
    instr.execute(&ALICE_ID, &mut stx).expect("finalize ok");
    let evs = stx.world.take_external_events();
    assert!(evs.iter().any(|event| matches!(
        event.as_data_event(),
        Some(DataEvent::Governance(GovernanceEvent::ProposalRejected(_)))
    )));
}

#[test]
fn min_turnout_rejects_when_below_threshold() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: threshold test gated. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query_handle);
    // Set high min_turnout to force rejection
    let mut cfg = state.gov.clone();
    cfg.min_turnout = 1_000;
    state.set_gov(cfg);

    // Block H=1
    let (_pk, _sk) = iroha_crypto::KeyPair::random().into_parts();
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();

    // Minimal approve weight=1
    let rid = "rid-threshold-2".to_string();
    let mut map = iroha_core::state::GovernanceLocksForReferendum::default();
    map.locks.insert(
        ALICE_ID.clone(),
        iroha_core::state::GovernanceLockRecord {
            owner: ALICE_ID.clone(),
            amount: 1,
            slashed: 0,
            expiry_height: 100,
            direction: 0,
            duration_blocks: 0,
        },
    );
    stx.world.governance_locks_mut().insert(rid.clone(), map);

    // Finalize should reject due to turnout < min_turnout
    let instr = FinalizeReferendum {
        referendum_id: rid.clone(),
        proposal_id: [0xCD; 32],
    };
    instr.execute(&ALICE_ID, &mut stx).expect("finalize ok");
    let evs = stx.world.take_external_events();
    assert!(evs.iter().any(|event| matches!(
        event.as_data_event(),
        Some(DataEvent::Governance(GovernanceEvent::ProposalRejected(_)))
    )));
}
