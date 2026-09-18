//! Exact membership publication, refusal custody and installation lifetime.

use super::*;
use mv::PublicationPreparationError;

fn key(n: u8) -> Key {
    HashOf::from_untyped_unchecked(iroha_crypto::Hash::new([n]))
}

fn stage<'storage>(
    storage: &'storage TransactionsStorage,
    at: usize,
    keys: &[u8],
) -> TransactionsBlock<'storage> {
    let mut block = storage.block();
    block.insert_block(
        keys.iter().copied().map(key).collect(),
        NonZeroUsize::new(at).unwrap(),
    );
    block
}

fn prepare(
    detached: DetachedTransactionsBlock,
    storage: &TransactionsStorage,
) -> PreparedDetachedTransactionsBlock<'_, ()> {
    detached
        .try_prepare_publication(storage, |_, _| Ok::<_, &'static str>(()))
        .unwrap_or_else(|(_, error)| panic!("publication preparation: {error:?}"))
}

#[test]
fn publication_and_abort_keep_original_membership_allocation_and_history() {
    let storage = TransactionsStorage::new();
    stage(&storage, 1, &[1, 2]).commit().unwrap();
    let old_view = storage.view();
    let prepared = stage(&storage, 2, &[2, 3]).prepare_commit().unwrap();
    let expected = norito::json::to_json(&prepared).unwrap();
    let journal = prepared.detach();
    let pointer = std::ptr::from_ref(journal.staged_membership().1);
    let prepared = prepare(journal, &storage);
    assert!(storage.write_lock.try_lock().is_none());
    assert_eq!(storage.latest_height(), 1);
    assert_eq!(storage.view().get(&key(3)), None);
    let journal = prepared.abort();
    assert_eq!(std::ptr::from_ref(journal.staged_membership().1), pointer);
    assert_eq!(
        journal.observe_predecessor(&storage),
        MembershipPredecessorStatus::Current
    );
    prepare(journal, &storage).publish();
    assert_eq!(norito::json::to_json(&storage).unwrap(), expected);
    assert_eq!(storage.view().get(&key(1)), NonZeroUsize::new(1));
    assert_eq!(storage.view().get(&key(2)), NonZeroUsize::new(2));
    assert_eq!(old_view.get(&key(2)), NonZeroUsize::new(1));
    assert_eq!(old_view.get(&key(3)), None);
}

#[test]
fn repeat_does_not_promote_the_tip_and_replacement_recovers_older_membership() {
    let storage = TransactionsStorage::new();
    stage(&storage, 1, &[1, 2]).commit().unwrap();
    stage(&storage, 2, &[2, 3]).commit().unwrap();
    let next = stage(&storage, 3, &[4]).prepare_commit().unwrap().detach();
    prepare(
        stage(&storage, 2, &[2, 3])
            .prepare_commit()
            .unwrap()
            .detach(),
        &storage,
    )
    .publish();
    assert_eq!(
        next.observe_predecessor(&storage),
        MembershipPredecessorStatus::Current
    );
    let mut replacement = storage.block_and_revert();
    replacement.insert_block(HashSet::from([key(4)]), NonZeroUsize::new(2).unwrap());
    let replacement = replacement.prepare_commit().unwrap();
    let expected = norito::json::to_json(&replacement).unwrap();
    prepare(replacement.detach(), &storage).publish();
    assert_eq!(norito::json::to_json(&storage).unwrap(), expected);
    assert_eq!(storage.view().get(&key(2)), NonZeroUsize::new(1));
    assert_eq!(storage.view().get(&key(3)), None);
    assert_eq!(
        next.observe_predecessor(&storage),
        MembershipPredecessorStatus::Changed
    );
}

#[test]
fn busy_and_refused_installation_return_the_original_journal_for_retry() {
    let storage = TransactionsStorage::new();
    stage(&storage, 1, &[1]).commit().unwrap();
    let journal = stage(&storage, 2, &[2]).prepare_commit().unwrap().detach();
    let pointer = std::ptr::from_ref(journal.staged_membership().1);
    let writer = storage.block();
    let (journal, error) = journal
        .try_prepare_publication(&storage, |_, _| -> Result<(), &str> {
            panic!("busy observation must not attempt installation admission")
        })
        .err()
        .expect("busy writer");
    assert!(matches!(error, PublicationPreparationError::Busy(_)));
    drop(writer);
    let (journal, error) = journal
        .try_prepare_publication(&storage, |_, target| {
            assert!(
                target.write_lock.try_lock().is_some(),
                "admission precedes writer ownership"
            );
            Err::<(), _>("capacity")
        })
        .err()
        .expect("admission refusal");
    assert!(matches!(
        error,
        PublicationPreparationError::Admission("capacity")
    ));
    assert_eq!(std::ptr::from_ref(journal.staged_membership().1), pointer);
    assert_eq!(storage.latest_height(), 1);
    prepare(journal, &storage).publish();
    assert_eq!(storage.view().get(&key(2)), NonZeroUsize::new(2));
}

#[test]
fn membership_abort_detach_and_commit_signal_the_exact_busy_writer() {
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Waker},
    };
    for finish in 0..5 {
        let storage = TransactionsStorage::new();
        stage(&storage, 1, &[1]).commit().unwrap();
        let journal = stage(&storage, 2, &[2]).prepare_commit().unwrap().detach();
        let pointer = std::ptr::from_ref(journal.staged_membership().1);
        let competitor = stage(&storage, 2, &[3]).prepare_commit().unwrap();
        let (journal, error) = journal
            .try_prepare_publication(&storage, |_, _| Ok::<_, ()>(()))
            .err()
            .expect("membership writer");
        let PublicationPreparationError::Busy(wait) = error else {
            panic!("writer wait");
        };
        let mut wait = wait.wait_for_release();
        let mut context = Context::from_waker(Waker::noop());
        if finish % 2 == 0 {
            assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
        }
        match finish {
            0 => drop(competitor),
            1 => {
                competitor.detach();
            }
            2 => {
                drop(prepare(competitor.detach(), &storage));
            }
            3 => {
                prepare(competitor.detach(), &storage).abort();
            }
            _ => competitor.publish(),
        }
        assert!(Pin::new(&mut wait).poll(&mut context).is_ready());
        assert_eq!(std::ptr::from_ref(journal.staged_membership().1), pointer);
        let retry = journal.try_prepare_publication(&storage, |_, _| Ok::<_, ()>(()));
        if finish == 4 {
            assert!(matches!(
                retry.err().unwrap().1,
                PublicationPreparationError::Changed
            ));
        } else {
            assert_eq!(storage.latest_height(), 1, "wake needs no commit");
            assert!(retry.is_ok());
        }
    }
}

#[test]
fn foreign_owner_and_an_admission_race_cannot_rebind_the_journal() {
    let storage = TransactionsStorage::new();
    stage(&storage, 1, &[1]).commit().unwrap();
    let restored: TransactionsStorage =
        norito::json::from_str(&norito::json::to_json(&storage).unwrap()).unwrap();
    let journal = stage(&storage, 2, &[2]).prepare_commit().unwrap().detach();
    let pointer = std::ptr::from_ref(journal.staged_membership().1);
    let (journal, error) = journal
        .try_prepare_publication(&restored, |_, _| -> Result<(), &str> {
            panic!("foreign identity must be rejected before admission")
        })
        .err()
        .expect("foreign owner");
    assert!(matches!(error, PublicationPreparationError::Changed));
    let (journal, error) = journal
        .try_prepare_publication(&storage, |_, target| {
            // Equal bytes after two real replacements still represent a new cut.
            for value in [3, 1] {
                let mut replacement = target.block_and_revert();
                replacement
                    .insert_block(HashSet::from([key(value)]), NonZeroUsize::new(1).unwrap());
                replacement.commit().unwrap();
            }
            Ok::<_, &str>(())
        })
        .err()
        .expect("predecessor changed during admission");
    assert!(matches!(error, PublicationPreparationError::Changed));
    assert_eq!(std::ptr::from_ref(journal.staged_membership().1), pointer);
    assert_eq!(storage.view().get(&key(1)), NonZeroUsize::new(1));
    assert_eq!(storage.view().get(&key(2)), None);
    assert!(storage.write_lock.try_lock().is_some());
}

struct Installation<'a> {
    storage: &'a TransactionsStorage,
    releases: &'a std::cell::Cell<usize>,
}

impl Drop for Installation<'_> {
    fn drop(&mut self) {
        assert!(
            self.storage.write_lock.try_lock().is_some(),
            "release admission after writer"
        );
        self.releases.set(self.releases.get() + 1);
    }
}

#[test]
fn installation_outlives_writer_on_drop_abort_and_publication() {
    let storage = TransactionsStorage::new();
    let releases = std::cell::Cell::new(0);
    let detached = || stage(&storage, 1, &[1]).prepare_commit().unwrap().detach();
    let admit = |_: &DetachedTransactionsBlock, _: &TransactionsStorage| {
        Ok::<_, &str>(Installation {
            storage: &storage,
            releases: &releases,
        })
    };
    let prepared = detached()
        .try_prepare_publication(&storage, admit)
        .unwrap_or_else(|_| panic!("prepare"));
    drop(prepared);
    assert_eq!(releases.get(), 1);
    assert_eq!(storage.latest_height(), 0);
    let journal = detached()
        .try_prepare_publication(&storage, admit)
        .unwrap_or_else(|_| panic!("prepare"))
        .abort();
    assert_eq!(releases.get(), 2);
    let guard = journal
        .try_prepare_publication(&storage, admit)
        .unwrap_or_else(|_| panic!("prepare"))
        .publish();
    assert_eq!(
        releases.get(),
        2,
        "aggregate owner retains installation admission"
    );
    assert_eq!(storage.latest_height(), 1);
    drop(guard);
    assert_eq!(releases.get(), 3);
}
