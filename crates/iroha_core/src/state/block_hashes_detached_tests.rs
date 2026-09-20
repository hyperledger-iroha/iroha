//! Actual hash journals become owned worker results without publishing or pinning readers.

use super::*;
use std::io::Write as _;

fn hash(value: u8) -> HashOf<BlockHeader> {
    HashOf::from_untyped_unchecked(Hash::prehashed([value; Hash::LENGTH]))
}

fn assert_unlocked(owner: &BlockHashes) {
    assert!(owner.inner.try_write().is_some());
}

#[test]
fn ordinary_detachment_moves_both_vectors_and_releases_the_original_read_guard() {
    let owner = BlockHashes::new(vec![hash(1), hash(2)]);
    let mut block = owner.block();
    block.push(hash(3));
    block.push(hash(4));
    let visible_pointer = block.visible.as_ptr();
    let visible_capacity = block.visible.capacity();
    let pending_pointer = block.pending.as_ptr();
    let pending_capacity = block.pending.capacity();
    assert!(owner.inner.try_write().is_none());
    let detached = block.detach();
    assert_unlocked(&owner);
    assert_eq!(detached.mode(), mv::BlockMode::Ordinary);
    assert_eq!(detached.prefix(), &[hash(1), hash(2)]);
    assert_eq!(detached.pending(), &[hash(3), hash(4)]);
    assert_eq!(detached.as_slice(), &[hash(1), hash(2), hash(3), hash(4)]);
    assert_eq!(detached.visible.as_ptr(), visible_pointer);
    assert_eq!(detached.visible.capacity(), visible_capacity);
    assert_eq!(detached.pending.as_ptr(), pending_pointer);
    assert_eq!(detached.pending.capacity(), pending_capacity);
    assert!(detached.matches_current(&owner));
    assert_eq!(&*owner.view(), &[hash(1), hash(2)]);
    assert_eq!(owner.committed_height(), 2);
    drop(detached);
    assert_unlocked(&owner);
    assert_eq!(&*owner.view(), &[hash(1), hash(2)]);
    assert_eq!(owner.committed_height(), 2);
}

#[test]
fn replacement_detachment_preserves_the_original_prefix_pending_and_mode() {
    let owner = BlockHashes::new(vec![hash(1), hash(2)]);
    let mut block = owner.block_and_revert();
    block.push(hash(3));
    let detached = block.detach();
    assert_unlocked(&owner);
    assert_eq!(detached.mode(), mv::BlockMode::Replace);
    assert_eq!(detached.prefix(), &[hash(1)]);
    assert_eq!(detached.pending(), &[hash(3)]);
    assert_eq!(detached.as_slice(), &[hash(1), hash(3)]);
    assert!(detached.matches_current(&owner));
    assert!(!detached.matches_block_predecessor(&owner.block()));
    assert!(detached.matches_block_predecessor(&owner.block_and_revert()));
    assert_eq!(&*owner.view(), &[hash(1), hash(2)]);
    assert_eq!(owner.committed_height(), 2);
    drop(detached);
    assert_unlocked(&owner);
    assert_eq!(&*owner.view(), &[hash(1), hash(2)]);
}

#[test]
fn replacement_without_candidate_touches_still_retains_tip_removal() {
    let owner = BlockHashes::new(vec![hash(1), hash(2)]);
    let detached = owner.block_and_revert().detach();
    assert_eq!(detached.mode(), mv::BlockMode::Replace);
    assert_eq!(detached.prefix(), &[hash(1)]);
    assert!(detached.pending().is_empty());
    assert_eq!(detached.as_slice(), &[hash(1)]);
    assert_eq!(&*owner.view(), &[hash(1), hash(2)]);
    // Only the existing normal writer can publish. Capture/drop never does.
    owner.block_and_revert().commit();
    assert_eq!(&*owner.view(), &[hash(1)]);
    assert_eq!(owner.committed_height(), 1);
    assert!(!detached.matches_current(&owner));

    let empty = BlockHashes::default();
    let ordinary = empty.block().detach();
    let replacement = empty.block_and_revert().detach();
    assert!(ordinary.as_slice().is_empty());
    assert!(replacement.as_slice().is_empty());
    assert_eq!(ordinary.mode(), mv::BlockMode::Ordinary);
    assert_eq!(replacement.mode(), mv::BlockMode::Replace);
    assert!(!ordinary.matches_block_predecessor(&empty.block_and_revert()));
    assert!(!replacement.matches_block_predecessor(&empty.block()));
}

#[test]
fn ordinary_abort_and_child_drop_do_not_rotate_the_captured_identity() {
    let owner = BlockHashes::new(vec![hash(1)]);
    let original = owner.block().detach();
    {
        let mut aborted = owner.block();
        aborted.push(hash(2));
    }
    assert_unlocked(&owner);
    assert!(original.matches_current(&owner));
    let mut block = owner.block();
    {
        let child = block.transaction();
        assert_eq!(&*child, &[hash(1)]);
    }
    {
        let child = block.transaction();
        assert_eq!(&*child, &[hash(1)]);
        child.apply();
    }
    let unchanged = block.detach();
    assert_eq!(unchanged.as_slice(), &[hash(1)]);
    assert!(unchanged.pending().is_empty());
    assert!(original.matches_current(&owner));
    assert!(unchanged.matches_current(&owner));
    assert_unlocked(&owner);
    owner.block().commit();
    assert_eq!(&*owner.view(), &[hash(1)]);
    assert_eq!(owner.committed_height(), 1);
    assert!(!original.matches_current(&owner));
    assert!(!unchanged.matches_current(&owner));
}

#[test]
fn same_height_replacement_and_equal_content_aba_cannot_restore_identity() {
    let owner = BlockHashes::new(vec![hash(1), hash(2)]);
    let original = owner.block().detach();
    let mut replacement = owner.block_and_revert();
    replacement.push(hash(3));
    replacement.commit();
    assert_eq!(owner.committed_height(), 2);
    assert_eq!(&*owner.view(), &[hash(1), hash(3)]);
    assert!(!original.matches_current(&owner));
    let mut replacement = owner.block_and_revert();
    replacement.push(hash(2));
    replacement.commit();
    assert_eq!(owner.committed_height(), 2);
    assert_eq!(&*owner.view(), original.as_slice());
    assert!(!original.matches_current(&owner));
    assert!(!original.matches_block_predecessor(&owner.block()));
}

#[test]
fn detachment_after_prepare_commit_keeps_the_original_identity_and_content() {
    let owner = BlockHashes::new(vec![hash(1)]);
    let mut original = owner.block();
    original.push(hash(2));
    original.prepare_commit();
    assert_unlocked(&owner);
    let mut replacement = owner.block_and_revert();
    replacement.push(hash(3));
    replacement.commit();
    let detached = original.detach();
    assert_eq!(detached.prefix(), &[hash(1)]);
    assert_eq!(detached.pending(), &[hash(2)]);
    assert_eq!(detached.as_slice(), &[hash(1), hash(2)]);
    assert!(!detached.matches_current(&owner));
    assert_eq!(&*owner.view(), &[hash(3)]);
    assert_eq!(owner.committed_height(), 1);
    assert_unlocked(&owner);
}

#[test]
fn detached_hashes_cross_static_worker_handoff_without_retaining_the_owner() {
    fn assert_owned<T: Send + Sync + 'static>() {}
    assert_owned::<DetachedBlockHashes>();
    let owner = Arc::new(BlockHashes::new(vec![hash(1)]));
    let worker = Arc::clone(&owner);
    let first = std::thread::spawn(move || {
        let mut block = worker.block();
        block.push(hash(2));
        block.detach()
    })
    .join()
    .unwrap();
    assert_eq!(Arc::strong_count(&owner), 1);
    assert_unlocked(&owner);
    let worker = Arc::clone(&owner);
    let second = std::thread::spawn(move || {
        let mut block = worker.block();
        block.push(hash(3));
        block.detach()
    })
    .join()
    .unwrap();
    assert_eq!(Arc::strong_count(&owner), 1);
    assert_unlocked(&owner);
    assert!(first.matches_current(&owner));
    assert!(second.matches_current(&owner));
    assert_eq!(first.as_slice(), &[hash(1), hash(2)]);
    assert_eq!(second.as_slice(), &[hash(1), hash(3)]);
    let weak_owner = Arc::downgrade(&owner);
    drop(owner);
    assert!(weak_owner.upgrade().is_none());
    assert_eq!(first.prefix(), &[hash(1)]);
    assert_eq!(second.pending(), &[hash(3)]);
    assert!(!first.matches_current(&BlockHashes::new(vec![hash(1)])));
}

#[test]
fn json_restoration_preserves_hash_values_but_not_original_owner_identity() {
    let owner = BlockHashes::new(vec![hash(1), hash(2)]);
    let detached = owner.block_and_revert().detach();
    let encoded = norito::json::to_json(&owner.view().to_vec()).unwrap();
    let restored = BlockHashes::new(norito::json::from_json(&encoded).unwrap());
    assert_eq!(
        norito::json::to_json(&restored.view().to_vec()).unwrap(),
        encoded
    );
    assert_eq!(restored.committed_height(), owner.committed_height());
    assert_eq!(&*restored.view(), &*owner.view());
    assert!(detached.matches_current(&owner));
    assert!(!detached.matches_current(&restored));
    assert!(!detached.matches_block_predecessor(&restored.block_and_revert()));
    assert_eq!(restored.block_and_revert().detach().prefix(), &[hash(1)]);
}

#[test]
fn mapped_fast_journal_still_refuses_ordinary_and_replacement_capture() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(hash(1).as_ref()).unwrap();
    let mapping = ReadOnlyMmap::copy_read_only(file.as_file(), Hash::LENGTH).unwrap();
    let mapped = BlockHashes::new_emergency_fast_mapped(mapping, 1);
    let detached = BlockHashes::new(vec![hash(1)]).block().detach();
    assert!(!detached.matches_current(&mapped));
    for replace in [false, true] {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if replace {
                mapped.block_and_revert().detach()
            } else {
                mapped.block().detach()
            }
        }));
        assert!(result.is_err());
        assert_unlocked(&mapped);
        assert_eq!(&*mapped.view(), &[hash(1)]);
        assert_eq!(mapped.committed_height(), 1);
        assert!(matches!(
            &*mapped.inner.read(),
            BlockHashStorage::EmergencyFastMapped(_)
        ));
    }
}

#[test]
fn identity_and_actual_hash_changes_share_the_existing_write_lock() {
    let owner = BlockHashes::new(vec![hash(1)]);
    let original = owner.block().detach();
    let next = Arc::new(BlockHashPublication);
    let mut writer = owner.inner.write();
    let BlockHashStorage::Owned {
        hashes,
        publication,
    } = &mut *writer
    else {
        panic!("owned test journal")
    };
    // Exercise a same-height write while the exact original storage guard is
    // held. No observer can see a half-updated data/token combination.
    hashes[0] = hash(2);
    assert!(owner.inner.try_read().is_none());
    *publication = next;
    assert!(owner.inner.try_read().is_none());
    drop(writer);
    assert!(!original.matches_current(&owner));
    assert_eq!(&*owner.view(), &[hash(2)]);
    assert_eq!(owner.committed_height(), 1);
    assert_unlocked(&owner);
}
