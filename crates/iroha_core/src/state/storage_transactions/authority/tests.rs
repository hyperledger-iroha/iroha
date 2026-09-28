//! Original-writer and complete-visitation controls for borrowed membership cuts.

use super::*;
use crate::state::storage_transactions::TransactionsStorage;
use iroha_crypto::{Hash, HashOf};
use std::{collections::BTreeMap, collections::HashSet, convert::Infallible, num::NonZeroUsize};

fn key(seed: &'static [u8]) -> Key {
    HashOf::from_untyped_unchecked(Hash::new(seed))
}

fn height(value: usize) -> Value {
    NonZeroUsize::new(value).unwrap()
}

fn commit(storage: &TransactionsStorage, keys: &[Key], value: usize) {
    let mut block = storage.block();
    block.insert_block(keys.iter().copied().collect(), height(value));
    block.commit().unwrap();
}

#[derive(Debug, PartialEq, Eq)]
struct CapturedFixture {
    frontier: u64,
    visits: usize,
    current: BTreeMap<Key, u64>,
    rollback: BTreeMap<Key, u64>,
}

fn capture(owner: &TransactionsBlock<'_>, allowance: usize) -> CapturedFixture {
    let cut = owner.membership_authority_cut(allowance).unwrap();
    let mut result = CapturedFixture {
        frontier: cut.frontier_height(),
        visits: cut.row_visits(),
        current: BTreeMap::new(),
        rollback: BTreeMap::new(),
    };
    cut.visit(|side, key, height| {
        let map = match side {
            TransactionMembershipSide::Current => &mut result.current,
            TransactionMembershipSide::Rollback => &mut result.rollback,
        };
        assert!(map.insert(*key, height).is_none());
        Ok::<(), Infallible>(())
    })
    .unwrap();
    result
}

#[test]
fn both_cuts_and_frontier_keep_the_same_original_writer() {
    let storage = TransactionsStorage::new();
    let first = key(b"borrowed-membership-first");
    let second = key(b"borrowed-membership-second");
    commit(&storage, &[first], 1);
    commit(&storage, &[second], 2);

    let owner = storage.block();
    let cut = owner.membership_authority_cut(3).unwrap();
    assert_eq!(cut.frontier_height(), 2);
    assert_eq!(cut.row_visits(), 3);
    assert!(storage.write_lock.try_lock().is_none());
    let mut rows = 0;
    cut.visit(|_, _, _| {
        assert!(storage.write_lock.try_lock().is_none());
        rows += 1;
        Ok::<(), Infallible>(())
    })
    .unwrap();
    assert_eq!(rows, 3);
    assert!(storage.write_lock.try_lock().is_none());
    let captured = capture(&owner, 3);
    assert_eq!(captured.current, BTreeMap::from([(first, 1), (second, 2)]));
    assert_eq!(captured.rollback, BTreeMap::from([(first, 1)]));
    drop(owner);
    assert!(storage.write_lock.try_lock().is_some());
}

#[test]
fn empty_frontier_needs_no_work_and_height_bounds_are_exact() {
    let storage = TransactionsStorage::new();
    let owner = storage.block();
    let captured = capture(&owner, 0);
    assert_eq!(captured.frontier, 0);
    assert_eq!(captured.visits, 0);
    assert!(captured.current.is_empty());
    assert!(captured.rollback.is_empty());
    assert_eq!(
        canonical_height(height(2), 1),
        Err(TransactionMembershipAuthorityError::InvalidHeight)
    );
    assert_eq!(canonical_height(height(1), 1), Ok(1));
}

#[test]
fn complete_visitation_refuses_before_any_consumer_can_run() {
    let storage = TransactionsStorage::new();
    let first = key(b"borrowed-membership-refusal-first");
    let second = key(b"borrowed-membership-refusal-second");
    commit(&storage, &[first], 1);
    commit(&storage, &[second], 2);
    let owner = storage.block();
    for limit in [0, 1, 2] {
        let mut visited = false;
        let result = owner.membership_authority_cut(limit).map(|cut| {
            cut.visit(|_, _, _| {
                visited = true;
                Ok::<(), Infallible>(())
            })
            .unwrap();
        });
        assert_eq!(
            result,
            Err(TransactionMembershipAuthorityError::TraversalRefused { required: 3, limit })
        );
        assert!(!visited);
    }
    assert_eq!(capture(&owner, 3).visits, 3);
}

#[test]
fn traversal_overflow_is_a_typed_refusal() {
    assert_eq!(admit_row_visits(1, 1, 3), Ok(3));
    assert_eq!(
        admit_row_visits(0, usize::MAX, usize::MAX),
        Err(TransactionMembershipAuthorityError::TraversalOverflow)
    );
    assert_eq!(
        admit_row_visits(usize::MAX, 1, usize::MAX),
        Err(TransactionMembershipAuthorityError::TraversalOverflow)
    );
}

#[test]
fn shadowed_and_filtered_history_are_funded_even_when_not_emitted() {
    let storage = TransactionsStorage::new();
    let reused = key(b"borrowed-membership-reused");
    let filtered = key(b"borrowed-membership-filtered");
    commit(&storage, &[reused], 1);
    commit(&storage, &[reused], 2);
    // Recovery-corruption fixture: the existing logical visitors exclude this
    // physical row from both cuts. Its inspection must still consume work.
    storage.budget.with_deferred_refund_notifications(|_| {
        let mut writer = storage
            .blocks
            .try_write_admitted(|demand| super::super::history::admit(&storage.budget, demand))
            .expect("original history writer admission");
        writer
            .try_insert_admitted(filtered, height(3), |demand| {
                super::super::history::admit(&storage.budget, demand)
            })
            .expect("funded physical corruption fixture");
        drop(writer.prepare_commit().publish().release());
    });
    let owner = storage.block();
    assert!(matches!(
        owner.membership_authority_cut(4),
        Err(TransactionMembershipAuthorityError::TraversalRefused {
            required: 5,
            limit: 4
        })
    ));
    let captured = capture(&owner, 5);
    assert_eq!(captured.frontier, 2);
    assert_eq!(captured.visits, 5);
    assert_eq!(captured.current, BTreeMap::from([(reused, 2)]));
    assert_eq!(captured.rollback, BTreeMap::from([(reused, 1)]));
}

#[test]
fn replacement_mode_and_staged_rows_do_not_change_the_committed_cut() {
    let storage = TransactionsStorage::new();
    let first = key(b"borrowed-membership-staging-first");
    let second = key(b"borrowed-membership-staging-second");
    let staged = key(b"borrowed-membership-staging-third");
    commit(&storage, &[first], 1);
    commit(&storage, &[second], 2);
    let expected = capture(&storage.block(), 3);

    let mut ordinary = storage.block();
    ordinary.insert_block(HashSet::from([staged]), height(3));
    assert_eq!(capture(&ordinary, 3), expected);
    drop(ordinary);
    let mut replacement = storage.block_and_revert();
    replacement.insert_block(HashSet::from([staged]), height(2));
    assert_eq!(capture(&replacement, 3), expected);
    drop(replacement);
    assert_eq!(capture(&storage.block(), 3), expected);
}

#[test]
fn advance_replace_and_exact_repeat_keep_original_membership_semantics() {
    let storage = TransactionsStorage::new();
    let first = key(b"borrowed-membership-publication-first");
    let second = key(b"borrowed-membership-publication-second");
    let replacement = key(b"borrowed-membership-publication-replacement");
    commit(&storage, &[first], 1);
    assert_eq!(
        capture(&storage.block(), 1).current,
        BTreeMap::from([(first, 1)])
    );
    commit(&storage, &[second], 2);
    let before_repeat = capture(&storage.block(), 3);
    commit(&storage, &[second], 2);
    assert_eq!(capture(&storage.block(), 3), before_repeat);
    let mut owner = storage.block_and_revert();
    owner.insert_block(HashSet::from([replacement]), height(2));
    owner.commit().unwrap();
    let result = capture(&storage.block(), 3);
    assert_eq!(result.frontier, 2);
    assert_eq!(
        result.current,
        BTreeMap::from([(first, 1), (replacement, 2)])
    );
    assert_eq!(result.rollback, BTreeMap::from([(first, 1)]));
}

#[test]
fn callback_failure_preserves_its_exact_error_and_never_reports_a_prefix() {
    let storage = TransactionsStorage::new();
    let first = key(b"borrowed-membership-callback-first");
    let second = key(b"borrowed-membership-callback-second");
    commit(&storage, &[first], 1);
    commit(&storage, &[second], 2);
    let owner = storage.block();
    let before = capture(&owner, 3);
    for failed_side in [
        TransactionMembershipSide::Current,
        TransactionMembershipSide::Rollback,
    ] {
        let cut = owner.membership_authority_cut(3).unwrap();
        let mut calls = 0;
        let mut refused = false;
        let result = cut.visit(|side, _, _| {
            assert!(!refused, "consumer called after refusal");
            calls += 1;
            if side == failed_side {
                refused = true;
                Err("original consumer refusal")
            } else {
                Ok(())
            }
        });
        assert_eq!(
            result,
            Err(TransactionMembershipVisitError::Consumer(
                "original consumer refusal"
            ))
        );
        assert!(refused);
        assert_eq!(
            calls,
            if failed_side == TransactionMembershipSide::Current {
                1
            } else {
                3
            }
        );
        assert_eq!(capture(&owner, 3), before);
    }
}

#[test]
fn nonblocking_capture_returns_original_busy_release_and_never_stages() {
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        task::{Context, Wake, Waker},
    };
    #[derive(Default)]
    struct Wakes(AtomicUsize);
    impl Wake for Wakes {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let storage = TransactionsStorage::new();
    let owner = storage.block();
    let expected = storage.released.observe();
    let Err(crate::state::storage_transactions::MembershipAdmissionError::Busy(wait)) =
        storage.try_membership_observation()
    else {
        panic!("same original writer is busy")
    };
    assert_eq!(wait, expected);
    let wakes = Arc::new(Wakes::default());
    let waker = Waker::from(wakes.clone());
    let mut context = Context::from_waker(&waker);
    let mut waiting = wait.wait_for_release();
    assert!(Pin::new(&mut waiting).poll(&mut context).is_pending());
    let other = TransactionsStorage::new();
    drop(other.block());
    assert_eq!(wakes.0.load(Ordering::SeqCst), 0);
    drop(owner);
    assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
    assert!(Pin::new(&mut waiting).poll(&mut context).is_ready());
    let owner = storage.try_membership_observation().unwrap();
    assert!(owner.publication_surface().current.is_none());
    assert_eq!(
        owner.membership_authority_cut(0).unwrap().frontier_height(),
        0
    );
    let Err(crate::state::storage_transactions::MembershipAdmissionError::Busy(wait)) =
        storage.try_membership_observation()
    else {
        panic!("retained writer")
    };
    drop(owner);
    assert!(
        Pin::new(&mut wait.wait_for_release())
            .poll(&mut context)
            .is_ready(),
        "release before registration remains visible"
    );
}
