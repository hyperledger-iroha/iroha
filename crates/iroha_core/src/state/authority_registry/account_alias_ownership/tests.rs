//! Native alias histories, malformed derivations and allocation-free capture controls.

use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{
        State,
        authority_registry::{
            complete::capture_account_alias_table_once,
            leaf::{LeafError, LeafLimits},
        },
    },
    test_allocations::allocations_during,
};
use iroha_data_model::{
    IntoKeyValue,
    account::{Account, AccountDetails},
    prelude::Registrable,
};
use iroha_model_base::topology::DataSpaceId;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use mv::storage::Storage;
use std::collections::BTreeMap;

fn alias(name: &str) -> AccountAlias {
    AccountAlias::domainless(name.parse().unwrap(), DataSpaceId::UNIVERSAL)
}

fn fixture() -> World {
    let mut world = World::default();
    for account in [&*ALICE_ID, &*BOB_ID] {
        let (key, value) = Account::new(account.clone())
            .build(account)
            .into_key_value();
        world.accounts.insert(key, value);
    }
    world
        .account_aliases
        .insert(alias("merchant"), ALICE_ID.clone());
    world.rebuild_account_alias_index().unwrap();
    world
}

fn without_allocations<T>(run: impl FnOnce() -> T) -> T {
    let mut value = None;
    assert_eq!(allocations_during(|| value = Some(run())), 0);
    value.unwrap()
}

fn check_error(world: &World) -> AliasOwnershipError {
    without_allocations(|| CheckedAccountAliases::capture(world, 16_777_216))
        .err()
        .unwrap()
}

#[test]
fn native_alias_transfer_keeps_exact_current_and_predecessor_readers() {
    let world = fixture();
    let mut aliases = world.account_aliases.block();
    let mut reverse = world.account_aliases_by_account.block();
    aliases.insert(alias("merchant"), BOB_ID.clone());
    reverse.remove(ALICE_ID.clone());
    reverse.insert(BOB_ID.clone(), BTreeSet::from([alias("merchant")]));
    aliases.commit();
    reverse.commit();
    let checked =
        without_allocations(|| CheckedAccountAliases::capture(&world, 16_777_216)).unwrap();
    assert_eq!(checked.aliases().get(&alias("merchant")), Some(&*BOB_ID));
    assert_eq!(
        get_at(
            checked.aliases(),
            AliasImage::Predecessor,
            &alias("merchant")
        ),
        Some(&*ALICE_ID)
    );
    assert!(without_allocations(|| checked.matches_current()).unwrap());
    // No-op writes rotate the actual publication identity even with equal values.
    let mut aliases = world.account_aliases.block();
    aliases.insert(alias("merchant"), BOB_ID.clone());
    aliases.commit();
    assert!(!without_allocations(|| checked.matches_current()).unwrap());
    assert_eq!(
        get_at(
            checked.aliases(),
            AliasImage::Predecessor,
            &alias("merchant")
        ),
        Some(&*ALICE_ID)
    );
}

#[test]
fn current_alias_relation_rejects_missing_foreign_empty_and_duplicate_buckets() {
    for (mutation, expected) in [
        (0, AliasMismatch::MissingAlias),
        (1, AliasMismatch::ForeignAlias),
        (2, AliasMismatch::EmptyBucket),
        (3, AliasMismatch::ForeignAlias),
    ] {
        let mut world = fixture();
        match mutation {
            0 => {
                world.account_aliases_by_account = Storage::default();
            }
            1 => {
                world.account_aliases_by_account.insert(
                    ALICE_ID.clone(),
                    BTreeSet::from([alias("merchant"), alias("ghost")]),
                );
            }
            2 => {
                world
                    .account_aliases_by_account
                    .insert(BOB_ID.clone(), BTreeSet::new());
            }
            3 => {
                world
                    .account_aliases_by_account
                    .insert(BOB_ID.clone(), BTreeSet::from([alias("merchant")]));
            }
            _ => unreachable!(),
        }
        assert_eq!(check_error(&world), corrupt(AliasImage::Current, expected));
    }
}

#[test]
fn current_correct_index_cannot_hide_corrupt_predecessor() {
    let mut world = fixture();
    world.account_aliases_by_account = Storage::from_snapshot_parts(
        BTreeMap::from([(ALICE_ID.clone(), BTreeSet::from([alias("merchant")]))]),
        BTreeMap::from([(ALICE_ID.clone(), None)]),
    );
    assert_eq!(
        check_error(&world),
        corrupt(AliasImage::Predecessor, AliasMismatch::MissingAlias)
    );
    // A current account cannot justify an alias whose predecessor had no account.
    let mut world = fixture();
    let rows = world
        .accounts
        .view()
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    world.accounts = Storage::from_snapshot_parts(rows, BTreeMap::from([(ALICE_ID.clone(), None)]));
    assert_eq!(
        check_error(&world),
        corrupt(AliasImage::Predecessor, AliasMismatch::MissingAccount)
    );
}

#[test]
fn primary_labels_and_raw_private_labels_are_checked_without_allocations() {
    let mut world = fixture();
    let mut details = AccountDetails::default();
    details.set_label(Some(alias("merchant")));
    world
        .accounts
        .insert(ALICE_ID.clone(), AccountValue::new(details));
    drop(without_allocations(|| CheckedAccountAliases::capture(&world, 16_777_216)).unwrap());
    for owner in [&*ALICE_ID, &*BOB_ID] {
        let mut world = fixture();
        let mut details = AccountDetails::default();
        details.set_label(Some(alias("unbound")));
        world
            .accounts
            .insert(owner.clone(), AccountValue::new(details));
        assert_eq!(
            check_error(&world),
            corrupt(AliasImage::Current, AliasMismatch::PrimaryLabel)
        );
    }
    let mut world = fixture();
    let mut details = AccountDetails::default();
    details.set_label(Some(alias("merchant")));
    world
        .accounts
        .insert(BOB_ID.clone(), AccountValue::new(details));
    assert_eq!(
        check_error(&world),
        corrupt(AliasImage::Current, AliasMismatch::PrimaryLabel)
    );
    let mut world = fixture();
    world
        .account_aliases
        .insert(alias("12345678"), ALICE_ID.clone());
    assert_eq!(
        check_error(&world),
        corrupt(AliasImage::Current, AliasMismatch::PrivateLabel)
    );
    let mut world = fixture();
    world.accounts = Storage::default();
    assert_eq!(
        check_error(&world),
        corrupt(AliasImage::Current, AliasMismatch::MissingAccount)
    );
}

#[test]
fn physical_absent_preimages_are_charged_and_equal_dependency_writes_are_detected() {
    let mut world = fixture();
    world.account_aliases = Storage::from_snapshot_parts(
        BTreeMap::from([(alias("merchant"), ALICE_ID.clone())]),
        BTreeMap::from([(alias("transient"), None)]),
    );
    // The original row-only boundary was 11. Full controller/alias comparisons,
    // complete lookup tails and both tombstone scans cost 796, independently:
    // unchanged two-account fixture 722 + 2*((1+17+18) + 1).
    let exact = test_support::exact_world_work(&world);
    assert_eq!(exact, 796);
    for bound in 0..exact {
        assert_eq!(
            without_allocations(|| CheckedAccountAliases::capture(&world, bound))
                .err()
                .unwrap(),
            AliasOwnershipError::WorkLimit
        );
    }
    drop(without_allocations(|| CheckedAccountAliases::capture(&world, exact)).unwrap());
    for accounts in [true, false] {
        let checked =
            without_allocations(|| CheckedAccountAliases::capture(&world, 16_777_216)).unwrap();
        if accounts {
            let mut block = world.accounts.block();
            block.insert(ALICE_ID.clone(), block.get(&*ALICE_ID).unwrap().clone());
            block.commit();
        } else {
            let mut block = world.account_aliases_by_account.block();
            block.insert(ALICE_ID.clone(), BTreeSet::from([alias("merchant")]));
            block.commit();
        }
        assert!(!without_allocations(|| checked.matches_current()).unwrap());
    }
}

#[test]
fn scoped_alias_capture_consumes_the_checked_source_before_leaf_allocation() {
    let mut world = fixture();
    world
        .account_aliases_by_account
        .insert(BOB_ID.clone(), BTreeSet::from([alias("merchant")]));
    let mut state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let limits = LeafLimits {
        max_tables: 1,
        max_rows: 16,
        max_payload_bytes: 16_384,
        max_ordered_table_bytes: 32_768,
        max_streamed_value_bytes: 131_072,
    };
    let pool = state.ivm_execution_budget();
    pool.set_limit_bytes(0);
    assert!(matches!(
        capture_account_alias_table_once(&state, limits),
        Err(LeafError::AliasOwnership(
            AliasOwnershipError::Corrupt { .. }
        ))
    ));
    state.world.rebuild_account_alias_index().unwrap();
    assert!(matches!(
        capture_account_alias_table_once(
            &state,
            LeafLimits {
                max_rows: 0,
                ..limits
            }
        ),
        Err(LeafError::AliasOwnership(AliasOwnershipError::WorkLimit))
    ));
    assert!(matches!(
        capture_account_alias_table_once(&state, limits),
        Err(LeafError::Admission(_) | LeafError::OrderedRange(_))
    ));
    pool.set_limit_bytes(16 * 1024 * 1024);
    let snapshot = capture_account_alias_table_once(&state, limits)
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.table_id(), "world.account_aliases");
    assert_eq!(snapshot.row_count(), 1);
}

#[test]
fn each_original_alias_publication_precedes_every_validation_outcome() {
    for source in 0..3 {
        for outcome in 0..3 {
            let world = fixture();
            let checked = CheckedAccountAliases::capture(&world, 16_777_216).unwrap();
            match source {
                0 => world.accounts.block().commit(),
                1 => world.account_aliases.block().commit(),
                2 => world.account_aliases_by_account.block().commit(),
                _ => unreachable!(),
            }
            let result = match outcome {
                0 => Ok(()),
                1 => Err(AliasOwnershipError::WorkLimit),
                2 => Err(corrupt(
                    AliasImage::Predecessor,
                    AliasMismatch::PrivateLabel,
                )),
                _ => unreachable!(),
            };
            assert_eq!(
                without_allocations(|| checked.finish_validation(result)).err(),
                Some(AliasOwnershipError::Publication(
                    PublicationPreparationError::Changed
                ))
            );
        }
    }
}

#[test]
fn original_alias_busy_refusal_precedes_later_change_work_and_corruption() {
    for mismatch in [false, true] {
        let world = fixture();
        let checked = CheckedAccountAliases::capture(&world, 16_777_216).unwrap();
        world.account_aliases.block().commit();
        world.account_aliases_by_account.block().commit();
        let detached = world
            .accounts
            .block()
            .try_detach(|_| Ok::<_, ()>(()))
            .unwrap();
        let prepared = detached
            .try_prepare_publication(&world.accounts, |_, _| Ok::<_, ()>(()))
            .unwrap_or_else(|(_, error, _)| panic!("original preparation: {error:?}"));
        let error = checked
            .accounts
            .try_matches_current(&world.accounts)
            .unwrap_err();
        assert!(matches!(error, PublicationPreparationError::Busy(_)));
        let result = if mismatch {
            Err(corrupt(AliasImage::Current, AliasMismatch::ForeignAlias))
        } else {
            Err(AliasOwnershipError::WorkLimit)
        };
        assert_eq!(
            without_allocations(|| checked.finish_validation(result)).err(),
            Some(AliasOwnershipError::Publication(error))
        );
        drop(prepared);
        without_allocations(|| CheckedAccountAliases::capture(&world, 16_777_216)).unwrap();
    }
}
