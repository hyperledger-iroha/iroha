//! Escrow grouping parity, optional membership and original publication custody.

use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{
        State,
        authority_registry::{
            complete::capture_escrows_once,
            leaf::{LeafError, LeafLimits},
        },
    },
    test_allocations::allocations_during,
};
use iroha_crypto::Hash;
use iroha_data_model::{
    account::Account, asset::AssetDefinitionId, escrow::AssetEscrowKind, prelude::Registrable,
};
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use mv::storage::Storage;

fn id() -> EscrowId {
    EscrowId::new(Hash::new(b"exact escrow groups"))
}

fn fixture(buyer: bool) -> World {
    let mut world = World::with(
        [],
        [
            Account::new(ALICE_ID.clone()).build(&ALICE_ID),
            Account::new(BOB_ID.clone()).build(&BOB_ID),
        ],
        [],
    );
    let record = AssetEscrowRecord {
        id: id(),
        seller: ALICE_ID.clone(),
        buyer: buyer.then(|| BOB_ID.clone()),
        asset_definition: AssetDefinitionId::derive_from_components(
            DomainId::try_new("escrowgroups", "universal").unwrap(),
            "token".parse().unwrap(),
        ),
        amount: Quantity::one(),
        custody: ALICE_ID.clone(),
        status: AssetEscrowStatus::Open,
        kind: AssetEscrowKind::Marketplace,
        remaining_amount: Quantity::one(),
        release_authority: None,
        expires_at_ms: None,
        evidence_hashes: Vec::new(),
        conditions: Vec::new(),
        created_at_ms: 1,
        accepted_at_ms: None,
        payment_sent_at_ms: None,
        disputed_at_ms: None,
        closed_at_ms: None,
        resolution: None,
    };
    world.asset_escrows.insert(id(), record);
    world.rebuild_escrow_indexes();
    world
}

fn check(world: &World, work: u64) -> Result<(), GroupedOwnershipError> {
    let mut result = None;
    assert_eq!(
        allocations_during(|| result = Some(CheckedEscrows::capture(world, work).map(|_| ()))),
        0
    );
    result.unwrap()
}

fn corrupt(index: &'static str, previous: bool, mismatch: GroupMismatch) -> GroupedOwnershipError {
    GroupedOwnershipError::Corrupt {
        index,
        image: if previous {
            GroupImage::Predecessor
        } else {
            GroupImage::Current
        },
        mismatch,
    }
}

#[test]
fn optional_buyer_changes_and_rollback_preserve_exact_original_images() {
    for initial_buyer in [false, true] {
        let mut world = fixture(initial_buyer);
        let original = world.asset_escrows.view().get(&id()).unwrap().clone();
        let mut replacement = original.clone();
        replacement.seller = BOB_ID.clone();
        replacement.buyer = (!initial_buyer).then(|| ALICE_ID.clone());
        replacement.status = AssetEscrowStatus::Accepted;
        {
            let mut block = world.block();
            let mut tx =
                block.transaction_without_telemetry(crate::state::LaneConfig::default(), 0);
            tx.insert_asset_escrow_entry(replacement.clone());
            tx.apply();
            block.commit();
        }
        assert_eq!(check(&world, 1024), Ok(()));
        let mut source_snapshot = String::new();
        crate::state::snapshot_storage::serialize(&world.asset_escrows, &mut source_snapshot);
        world.asset_escrows = norito::json::from_str::<
            crate::state::snapshot_storage::SnapshotStorage,
        >(&source_snapshot)
        .unwrap()
        .decode("escrow ownership fixture", |_, _| true)
        .unwrap();
        // Decode the actual canonical snapshot before rebuilding. Keep the real
        // predecessor, including a buyer appearing or disappearing, for rollback.
        world.rebuild_escrow_indexes();
        assert_eq!(check(&world, 1024), Ok(()));
        let checked = CheckedEscrows::capture(&world, 1024).unwrap();
        assert_eq!(checked.rows().get(&id()), Some(&replacement));
        assert_eq!(
            get_at(checked.rows(), GroupImage::Predecessor, &id()),
            Some(&original)
        );
        assert!(checked.matches_current().unwrap());
        drop(checked);
        world.block_and_revert().commit();
        assert_eq!(check(&world, 1024), Ok(()));
        assert_eq!(world.asset_escrows.view().get(&id()), Some(&original));
    }
}

#[test]
fn every_group_rejects_missing_members_in_both_native_images() {
    for index in 0..3 {
        for previous in [false, true] {
            let mut world = fixture(true);
            macro_rules! remove {
                ($field:ident, $key:expr) => {{
                    world.$field = Storage::default();
                    if previous {
                        let mut block = world.$field.block();
                        block.insert($key, BTreeSet::from([id()]));
                        block.commit();
                    }
                }};
            }
            let name = match index {
                0 => {
                    remove!(asset_escrows_by_seller, ALICE_ID.clone());
                    "world.asset_escrows_by_seller"
                }
                1 => {
                    remove!(asset_escrows_by_buyer, BOB_ID.clone());
                    "world.asset_escrows_by_buyer"
                }
                2 => {
                    remove!(asset_escrows_by_status, AssetEscrowStatus::Open);
                    "world.asset_escrows_by_status"
                }
                _ => unreachable!(),
            };
            assert_eq!(
                check(&world, 1024),
                Err(corrupt(name, previous, GroupMismatch::MissingMember))
            );
        }
    }
}

#[test]
fn absent_buyers_cannot_have_empty_foreign_or_deleted_source_membership() {
    for previous in [false, true] {
        for case in 0..3 {
            let mut world = fixture(false);
            let members = match case {
                0 => BTreeSet::new(),
                1 => BTreeSet::from([id()]),
                _ => BTreeSet::from([EscrowId::new(Hash::new(b"absent escrow"))]),
            };
            world.asset_escrows_by_buyer.insert(BOB_ID.clone(), members);
            if previous {
                let mut block = world.asset_escrows_by_buyer.block();
                block.remove(BOB_ID.clone());
                block.commit();
            }
            assert_eq!(
                check(&world, 1024),
                Err(corrupt(
                    "world.asset_escrows_by_buyer",
                    previous,
                    if case == 0 {
                        GroupMismatch::EmptyGroup
                    } else {
                        GroupMismatch::ForeignMember
                    },
                ))
            );
        }
    }
}

#[test]
fn local_work_counts_buyerless_sources_and_absent_undo_rows() {
    for (buyer, exact) in [(false, 14), (true, 18)] {
        let world = fixture(buyer);
        assert_eq!(check(&world, exact), Ok(()));
        assert_eq!(
            check(&world, exact - 1),
            Err(GroupedOwnershipError::WorkLimit)
        );
        {
            let mut block = world.asset_escrows.block();
            block.remove(EscrowId::new(Hash::new(b"absent undo")));
            block.commit();
        }
        assert_eq!(
            check(&world, exact + 2),
            Err(GroupedOwnershipError::WorkLimit)
        );
        assert_eq!(check(&world, exact + 3), Ok(()));
    }
}

#[test]
fn every_original_reader_detects_even_an_empty_index_publication() {
    for index in 0..4 {
        let world = fixture(false);
        let checked = CheckedEscrows::capture(&world, 1024).unwrap();
        match index {
            0 => world.asset_escrows.block().commit(),
            1 => world.asset_escrows_by_seller.block().commit(),
            2 => world.asset_escrows_by_buyer.block().commit(),
            3 => world.asset_escrows_by_status.block().commit(),
            _ => unreachable!(),
        }
        assert!(!checked.matches_current().unwrap());
    }
}

#[test]
fn state_capture_checks_undo_indexes_before_encoding_and_preserves_pool_refusal() {
    let mut world = fixture(false);
    world
        .asset_escrows_by_buyer
        .insert(BOB_ID.clone(), BTreeSet::from([id()]));
    {
        let mut block = world.asset_escrows_by_buyer.block();
        block.remove(BOB_ID.clone());
        block.commit();
    }
    let mut state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let limits = LeafLimits {
        max_tables: 1,
        max_rows: 16,
        max_payload_bytes: 16384,
        max_ordered_table_bytes: 32768,
        max_streamed_value_bytes: 131072,
    };
    let pool = state.ivm_execution_budget();
    pool.set_limit_bytes(0);
    assert!(matches!(
        capture_escrows_once(&state, limits),
        Err(LeafError::GroupedOwnership(
            GroupedOwnershipError::Corrupt {
                image: GroupImage::Predecessor,
                mismatch: GroupMismatch::ForeignMember,
                ..
            }
        ))
    ));
    state.world.rebuild_escrow_indexes();
    assert!(matches!(
        capture_escrows_once(
            &state,
            LeafLimits {
                max_rows: 0,
                ..limits
            }
        ),
        Err(LeafError::GroupedOwnership(
            GroupedOwnershipError::WorkLimit
        ))
    ));
    assert!(matches!(
        capture_escrows_once(&state, limits),
        Err(LeafError::Admission(_) | LeafError::OrderedRange(_))
    ));
    pool.set_limit_bytes(16 * 1024 * 1024);
    let snapshot = capture_escrows_once(&state, limits).unwrap().unwrap();
    assert_eq!(snapshot.table_id(), "world.asset_escrows");
    assert_eq!(snapshot.row_count(), 1);
}
