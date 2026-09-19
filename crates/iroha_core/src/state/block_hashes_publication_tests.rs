//! Hash publication preserves exact allocations and refuses stale predecessors.

use super::*;
use mv::PublicationPreparationError;

fn hash(n: u8) -> HashOf<BlockHeader> {
    HashOf::from_untyped_unchecked(Hash::new([n]))
}

fn detached(owner: &BlockHashes, replace: bool, appends: &[u8]) -> DetachedBlockHashes {
    let mut block = if replace {
        owner.block_and_revert()
    } else {
        owner.block()
    };
    for &value in appends {
        block.push(hash(value));
    }
    block.detach()
}

fn prepare<'target>(
    journal: DetachedBlockHashes,
    owner: &'target BlockHashes,
) -> PreparedBlockHashes<'target, ()> {
    journal
        .try_prepare_publication(owner, |_, _| Ok::<_, &str>(()))
        .unwrap_or_else(|(_, error)| panic!("hash publication preparation: {error:?}"))
}

#[test]
fn publication_moves_the_original_vector_and_matches_direct_commit() {
    for (replace, appends) in [
        (false, vec![3, 4]),
        (true, vec![3]),
        (true, vec![]),
        (false, vec![]),
    ] {
        let owner = BlockHashes::new(vec![hash(1), hash(2)]);
        let direct = BlockHashes::new(vec![hash(1), hash(2)]);
        let mut direct_block = if replace {
            direct.block_and_revert()
        } else {
            direct.block()
        };
        for &value in &appends {
            direct_block.push(hash(value));
        }
        direct_block.commit();
        let journal = detached(&owner, replace, &appends);
        let pointer = journal.visible.as_ptr();
        let capacity = journal.visible.capacity();
        let old = detached(&owner, false, &[]);
        let prepared = prepare(journal, &owner);
        assert!(owner.inner.try_read().is_none());
        assert_eq!(owner.committed_height(), 2);
        prepared.publish();
        let view = owner.view();
        assert_eq!(
            view.as_ptr(),
            pointer,
            "publication must move, not copy, the original chain"
        );
        let BlockHashStorage::Owned { hashes, .. } = &**view.guard else {
            panic!("owned")
        };
        assert_eq!(hashes.capacity(), capacity);
        assert_eq!(&*view, &*direct.view());
        assert_eq!(owner.committed_height(), direct.committed_height());
        drop(view);
        assert!(
            !old.matches_current(&owner),
            "even untouched hash publication rotates identity"
        );
    }
}

#[test]
fn abort_and_drop_leave_the_exact_original_cut_available() {
    let owner = BlockHashes::new(vec![hash(1), hash(2)]);
    let journal = detached(&owner, true, &[3]);
    let visible = journal.visible.as_ptr();
    let pending = journal.pending.as_ptr();
    let journal = prepare(journal, &owner).abort();
    assert_eq!(journal.visible.as_ptr(), visible);
    assert_eq!(journal.pending.as_ptr(), pending);
    assert_eq!(journal.prefix(), &[hash(1)]);
    assert!(journal.matches_current(&owner));
    drop(prepare(journal, &owner));
    assert_eq!(&*owner.view(), &[hash(1), hash(2)]);
    assert_eq!(owner.committed_height(), 2);
    prepare(detached(&owner, true, &[4]), &owner).publish();
    assert_eq!(&*owner.view(), &[hash(1), hash(4)]);
}

#[test]
fn readers_writers_and_installation_refusal_preserve_retry_custody() {
    let owner = BlockHashes::new(vec![hash(1)]);
    let journal = detached(&owner, false, &[2]);
    let pointer = journal.visible.as_ptr();
    let writer = owner.inner.write();
    let (journal, error) = journal
        .try_prepare_publication(&owner, |_, _| -> Result<(), &str> {
            panic!("busy writer is observed before admission")
        })
        .err()
        .expect("busy writer");
    assert!(matches!(error, PublicationPreparationError::Busy(_)));
    drop(writer);
    let reader = owner.view();
    let (journal, error) = journal
        .try_prepare_publication(&owner, |_, _| Ok::<_, &str>(()))
        .err()
        .expect("busy reader");
    assert!(matches!(error, PublicationPreparationError::Busy(_)));
    drop(reader);
    let (journal, error) = journal
        .try_prepare_publication(&owner, |_, target| {
            assert!(
                target.inner.try_write().is_some(),
                "installation precedes write ownership"
            );
            Err::<(), _>("capacity")
        })
        .err()
        .expect("installation refusal");
    assert!(matches!(
        error,
        PublicationPreparationError::Admission("capacity")
    ));
    assert_eq!(journal.visible.as_ptr(), pointer);
    assert_eq!(&*owner.view(), &[hash(1)]);
    prepare(journal, &owner).publish();
    assert_eq!(&*owner.view(), &[hash(1), hash(2)]);
}

#[test]
fn hash_reader_and_aborted_block_release_wake_the_exact_publisher_without_a_commit() {
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Waker},
    };
    for finish in 0..4 {
        let owner = BlockHashes::new(vec![hash(1)]);
        let journal = detached(&owner, false, &[2]);
        let pointer = journal.visible.as_ptr();
        let mut block = (finish != 0).then(|| owner.block());
        let reader = (finish == 0).then(|| owner.view());
        let (journal, error) = journal
            .try_prepare_publication(&owner, |_, _| Ok::<_, ()>(()))
            .err()
            .expect("original reader excludes publication");
        let PublicationPreparationError::Busy(wait) = error else {
            panic!("reader wait");
        };
        let mut wait = wait.wait_for_release();
        let mut context = Context::from_waker(Waker::noop());
        assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
        match finish {
            0 => drop(reader),
            1 => drop(block.take()),
            2 => block.as_mut().unwrap().prepare_commit(),
            _ => {
                block.take().unwrap().detach();
            }
        }
        assert!(Pin::new(&mut wait).poll(&mut context).is_ready());
        assert_eq!(owner.committed_height(), 1, "wake needs no new block");
        assert_eq!(journal.visible.as_ptr(), pointer);
        prepare(journal, &owner).publish();
        assert_eq!(&*owner.view(), &[hash(1), hash(2)]);
    }
}

#[test]
fn hash_prepared_writer_release_before_wait_registration_is_not_lost() {
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Waker},
    };
    for finish in 0..3 {
        let owner = BlockHashes::new(vec![hash(1)]);
        let first = detached(&owner, false, &[2]);
        let second = detached(&owner, false, &[3]);
        let prepared = prepare(first, &owner);
        let (second, error) = second
            .try_prepare_publication(&owner, |_, _| Ok::<_, ()>(()))
            .err()
            .expect("writer excludes observation");
        let PublicationPreparationError::Busy(wait) = error else {
            panic!("writer wait");
        };
        match finish {
            0 => drop(prepared),
            1 => {
                prepared.abort();
            }
            _ => {
                prepared.publish();
            }
        }
        let mut wait = wait.wait_for_release();
        assert!(
            Pin::new(&mut wait)
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_ready()
        );
        let retry = second.try_prepare_publication(&owner, |_, _| Ok::<_, ()>(()));
        if finish == 2 {
            assert!(matches!(
                retry.err().unwrap().1,
                PublicationPreparationError::Changed
            ));
        } else {
            assert!(retry.is_ok());
        }
    }
}

#[test]
fn caught_hash_view_panic_does_not_poison_a_later_reader_wait() {
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Waker},
    };
    let owner = BlockHashes::new(vec![hash(1)]);
    let journal = detached(&owner, false, &[2]);
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _view = owner.view();
        panic!("read-only request failed");
    }));
    assert!(unwound.is_err());
    let reader = owner.view();
    let (journal, error) = journal
        .try_prepare_publication(&owner, |_, _| Ok::<_, ()>(()))
        .err()
        .expect("live reader excludes writer");
    let PublicationPreparationError::Busy(wait) = error else {
        panic!("reader did not poison storage");
    };
    let mut wait = wait.wait_for_release();
    let mut context = Context::from_waker(Waker::noop());
    assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
    drop(reader);
    assert!(Pin::new(&mut wait).poll(&mut context).is_ready());
    prepare(journal, &owner).publish();
    assert_eq!(&*owner.view(), &[hash(1), hash(2)]);
}

#[test]
fn equal_bytes_in_another_owner_or_after_aba_never_authorize_publication() {
    let owner = BlockHashes::new(vec![hash(1)]);
    let foreign = BlockHashes::new(vec![hash(1)]);
    let journal = detached(&owner, false, &[2]);
    let pointer = journal.visible.as_ptr();
    let (journal, error) = journal
        .try_prepare_publication(&foreign, |_, _| -> Result<(), &str> {
            panic!("reject foreign owner before admission")
        })
        .err()
        .expect("foreign owner");
    assert!(matches!(error, PublicationPreparationError::Changed));
    let (journal, error) = journal
        .try_prepare_publication(&owner, |_, target| {
            for value in [3, 1] {
                let mut block = target.block_and_revert();
                block.push(hash(value));
                block.commit();
            }
            Ok::<_, &str>(())
        })
        .err()
        .expect("changed during admission");
    assert!(matches!(error, PublicationPreparationError::Changed));
    assert_eq!(journal.visible.as_ptr(), pointer);
    assert_eq!(&*owner.view(), &[hash(1)]);
    assert_eq!(owner.committed_height(), 1);
    assert!(owner.inner.try_write().is_some());
}

struct Installation<'a> {
    owner: &'a BlockHashes,
    releases: &'a std::cell::Cell<usize>,
}

impl Drop for Installation<'_> {
    fn drop(&mut self) {
        assert!(
            self.owner.inner.try_write().is_some(),
            "writer releases before installation admission"
        );
        self.releases.set(self.releases.get() + 1);
    }
}

#[test]
fn installation_is_retained_until_after_drop_abort_or_aggregate_publication() {
    let owner = BlockHashes::new(vec![hash(1)]);
    let releases = std::cell::Cell::new(0);
    let admit = |_: &DetachedBlockHashes, _: &BlockHashes| {
        Ok::<_, &str>(Installation {
            owner: &owner,
            releases: &releases,
        })
    };
    let prepare = |journal: DetachedBlockHashes| {
        journal
            .try_prepare_publication(&owner, admit)
            .unwrap_or_else(|_| panic!("prepare"))
    };
    drop(prepare(detached(&owner, false, &[2])));
    assert_eq!(releases.get(), 1);
    let journal = prepare(detached(&owner, false, &[2])).abort();
    assert_eq!(releases.get(), 2);
    assert_eq!(&*owner.view(), &[hash(1)]);
    let admission = prepare(journal).publish();
    assert_eq!(releases.get(), 2);
    assert_eq!(&*owner.view(), &[hash(1), hash(2)]);
    drop(admission);
    assert_eq!(releases.get(), 3);
}

#[test]
fn late_hash_refusal_releases_prepared_membership_before_exact_retry() {
    use crate::state::storage_transactions::{TransactionsReadOnly, TransactionsStorage};
    let hashes = BlockHashes::new(vec![hash(1)]);
    let membership = TransactionsStorage::new();
    let entrypoint = HashOf::from_untyped_unchecked(Hash::new(b"publication membership"));
    let mut block = membership.block();
    block.insert_block(
        std::collections::HashSet::from([entrypoint]),
        NonZeroUsize::new(1).unwrap(),
    );
    let journal = block.prepare_commit().unwrap().detach();
    let admitted = journal
        .try_prepare_publication(&membership, |_, _| Ok::<_, &str>(()))
        .unwrap_or_else(|_| panic!("membership prepare"));
    let reader = hashes.view();
    let (hash_journal, error) = detached(&hashes, false, &[2])
        .try_prepare_publication(&hashes, |_, _| Ok::<_, &str>(()))
        .err()
        .expect("late busy hash component");
    assert!(matches!(error, PublicationPreparationError::Busy(_)));
    let journal = admitted.abort();
    assert_eq!(membership.view().get(&entrypoint), None);
    assert_eq!(hashes.committed_height(), 1);
    drop(reader);
    let admitted = journal
        .try_prepare_publication(&membership, |_, _| Ok::<_, &str>(()))
        .unwrap_or_else(|_| panic!("membership retry"));
    let hash_admitted = prepare(hash_journal, &hashes);
    admitted.publish();
    hash_admitted.publish();
    assert_eq!(membership.view().get(&entrypoint), NonZeroUsize::new(1));
    assert_eq!(&*hashes.view(), &[hash(1), hash(2)]);
}
