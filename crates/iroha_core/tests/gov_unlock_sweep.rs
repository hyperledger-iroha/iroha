//! Automatic unlock sweep at block height.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Skipped by default; enable with `IROHA_RUN_IGNORED=1`.

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_data_model::{block::BlockHeader, events::data::governance::GovernanceEvent};
use iroha_test_samples::ALICE_ID;
use nonzero_ext::nonzero;

#[test]
fn unlocks_after_expiry_height() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: unlock sweep test gated. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query_handle);

    let _kp = iroha_crypto::KeyPair::random();

    // Block H=1: insert a lock expiring at H=2 (will unlock at H>=3 per current policy)
    let header1 = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut sblock1 = state.block(header1);
    {
        let mut stx = sblock1.transaction();
        let mut map = iroha_core::state::GovernanceLocksForReferendum::default();
        map.locks.insert(
            ALICE_ID.clone(),
            iroha_core::state::GovernanceLockRecord {
                owner: ALICE_ID.clone(),
                amount: 10,
                slashed: 0,
                expiry_height: 2,
                direction: 0,
                duration_blocks: 0,
            },
        );
        stx.world
            .governance_locks_mut()
            .insert("rid-unlock".to_string(), map);
        stx.apply();
    }
    // Drain events (none expected yet)
    sblock1.world.take_external_events();

    // Block H=2: still not unlocked
    let header2 = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut sblock2 = state.block(header2);
    let evs2 = sblock2.world.take_external_events();
    assert!(evs2.iter().all(|e| {
        !matches!(
            e,
            iroha_data_model::events::EventBox::Data(ev)
                if matches!(
                    ev.as_ref(),
                    iroha_data_model::events::data::DataEvent::Governance(
                        GovernanceEvent::LockUnlocked(_)
                    )
                )
        )
    }));

    // Block H=3: unlock should occur
    let header3 = BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut sblock3 = state.block(header3);
    let evs3 = sblock3.world.take_external_events();
    assert!(evs3.iter().any(|e| matches!(
        e,
        iroha_data_model::events::EventBox::Data(ev)
            if matches!(
                ev.as_ref(),
                iroha_data_model::events::data::DataEvent::Governance(
                    GovernanceEvent::LockUnlocked(_)
                )
            )
    )));
}
