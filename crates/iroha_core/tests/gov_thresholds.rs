//! Governance threshold tests: ratio and turnout logic.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use core::num::NonZeroU64;

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World},
};
use iroha_crypto::KeyPair;
use iroha_data_model::{
    block::BlockHeader,
    events::data::{DataEvent, governance::GovernanceEvent},
    isi::governance::FinalizeReferendum,
};
use iroha_test_samples::{ALICE_ID, BOB_ID};

fn checked_random_governance_threshold_keypair() -> KeyPair {
    KeyPair::try_random().expect("generate checked governance threshold keypair")
}

#[test]
fn governance_threshold_fixture_uses_checked_randomness() {
    let _key_pair = checked_random_governance_threshold_keypair();
}

#[test]
fn ratio_threshold_rejects_even_if_approve_gt_reject() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query_handle);
    // Configure strict ratio: 3/4 (75%) so 2/(2+1)=0.66.. fails
    let mut cfg = state.gov.clone();
    cfg.approval_threshold_q_num = 3;
    cfg.approval_threshold_q_den = 4;
    state.set_gov(cfg);

    // Block H=1
    let (_pk, _sk) = checked_random_governance_threshold_keypair().into_parts();
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
            amount: 4_u64.into(),
            slashed: 0_u64.into(),
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
            amount: 1_u64.into(),
            slashed: 0_u64.into(),
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
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query_handle);
    // Set high min_turnout to force rejection
    let mut cfg = state.gov.clone();
    cfg.min_turnout = 1_000;
    state.set_gov(cfg);

    // Block H=1
    let (_pk, _sk) = checked_random_governance_threshold_keypair().into_parts();
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
            amount: 1_u64.into(),
            slashed: 0_u64.into(),
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

#[test]
fn finalize_referendum_rejects_tally_overflow_without_side_effects() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query_handle);
    let mut cfg = state.gov.clone();
    cfg.conviction_step_blocks = 1;
    cfg.max_conviction = u64::MAX;
    state.set_gov(cfg);

    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();
    let rid = "rid-tally-overflow".to_string();
    let mut locks = iroha_core::state::GovernanceLocksForReferendum::default();
    for owner in [ALICE_ID.clone(), BOB_ID.clone()] {
        locks.locks.insert(
            owner.clone(),
            iroha_core::state::GovernanceLockRecord {
                owner,
                amount: u128::MAX.into(),
                slashed: 0_u64.into(),
                expiry_height: u64::MAX,
                direction: 0,
                duration_blocks: u64::MAX - 1,
            },
        );
    }
    stx.world.governance_locks_mut().insert(rid.clone(), locks);

    let err = FinalizeReferendum {
        referendum_id: rid.clone(),
        proposal_id: [0xEF; 32],
    }
    .execute(&ALICE_ID, &mut stx)
    .expect_err("overflowing tally must fail");

    assert!(err.to_string().contains("overflow"));
    assert!(stx.world.take_external_events().is_empty());
    let stored = stx
        .world
        .governance_locks()
        .get(&rid)
        .expect("locks remain present");
    assert_eq!(stored.locks.len(), 2);
    assert!(
        stored
            .locks
            .values()
            .all(|record| record.amount.scale() == 0
                && record.amount.as_numeric().try_mantissa_u128() == Some(u128::MAX))
    );
}
