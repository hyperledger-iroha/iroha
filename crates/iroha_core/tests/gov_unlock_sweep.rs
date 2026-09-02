//! Automatic unlock sweep at block height.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{
        GovernanceLockCustody, GovernanceLockRecord, GovernanceLocksForReferendum,
        GovernanceReferendumRecord, GovernanceReferendumStatus, GovernanceReferendumTallyV1, State,
        World, WorldReadOnly,
    },
};
use iroha_crypto::KeyPair;
use iroha_data_model::{block::BlockHeader, events::data::governance::GovernanceEvent};
use iroha_test_samples::ALICE_ID;
use nonzero_ext::nonzero;
fn checked_random_governance_unlock_keypair() -> KeyPair {
    KeyPair::try_random().expect("generate checked governance unlock keypair")
}
#[test]
fn governance_unlock_fixture_uses_checked_randomness() {
    let _key_pair = checked_random_governance_unlock_keypair();
}
#[test]
fn unlocks_after_expiry_height() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query_handle);
    let _kp = checked_random_governance_unlock_keypair();
    // Block H=1: insert a lock expiring at H=2 (will unlock at H>=3 per current policy)
    let header1 = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    {
        let mut sblock1 = state.block(header1);
        let mut stx = sblock1.transaction();
        stx.world.put_governance_referendum_for_testing(
            "rid-unlock".to_owned(),
            GovernanceReferendumRecord {
                h_start: 0,
                h_end: 0,
                status: GovernanceReferendumStatus::Closed,
                final_tally: Some(GovernanceReferendumTallyV1::new(0, 0, 0)),
            },
        );
        let mut map = GovernanceLocksForReferendum::default();
        map.locks.insert(
            ALICE_ID.clone(),
            GovernanceLockRecord {
                owner: ALICE_ID.clone(),
                amount: 0_u64.into(),
                slashed: 0_u64.into(),
                expiry_height: 2,
                direction: iroha_data_model::isi::governance::GovernancePlainBallotDirectionV1::Aye,
                duration_blocks: 2,
                custody: GovernanceLockCustody {
                    escrowed: false,
                    asset_definition_id: state.gov.voting_asset_id.clone(),
                    bond_escrow_account: state.gov.bond_escrow_account.clone(),
                    slash_receiver_account: state.gov.slash_receiver_account.clone(),
                },
            },
        );
        stx.world
            .governance_locks_mut()
            .insert("rid-unlock".to_string(), map);
        stx.apply();
        // Drain events (none expected yet)
        sblock1.world.take_external_events();
        sblock1
            .commit_empty_block_for_testing()
            .expect("commit block at H=1");
    }
    // Block H=2: still not unlocked
    let header2 = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    {
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
        sblock2
            .commit_empty_block_for_testing()
            .expect("commit block at H=2");
    }
    // Block H=3: unlock should occur
    let header3 = BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    {
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
        sblock3
            .commit_empty_block_for_testing()
            .expect("commit block at H=3");
    }
    let view = state.view();
    let world = view.world();
    assert!(world.governance_locks().get("rid-unlock").is_none());
    assert!(world.governance_referenda().get("rid-unlock").is_none());
}
