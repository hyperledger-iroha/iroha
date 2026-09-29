//! Original frozen map reads are typed, source-bound and independent of target locks.

use super::*;

fn assert_reads(read: &impl StorageReadOnly<u64, String>) {
    assert_eq!(read.len(), 4);
    assert!(!read.is_empty());
    assert_eq!(read.get(&1).map(String::as_str), Some("one"));
    assert_eq!(read.get(&2).map(String::as_str), Some("two changed"));
    assert!(read.get(&3).is_none());
    assert_eq!(
        read.get_key_value(&4).map(|(k, v)| (*k, v.as_str())),
        Some((4, "four"))
    );
    assert_eq!(read.first_key_value().map(|(k, _)| *k), Some(1));
    assert_eq!(read.last_key_value().map(|(k, _)| *k), Some(9));
    assert_eq!(
        read.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
        [1, 2, 4, 9]
    );
    assert_eq!(
        read.range(2..9).rev().map(|(k, _)| *k).collect::<Vec<_>>(),
        [4, 2]
    );
}

#[test]
fn frozen_map_read_trait_borrows_complete_original_trees_while_writers_are_free() {
    let target: Storage<_, _> = [(1, "one"), (2, "two"), (3, "three"), (4, "four")]
        .into_iter()
        .map(|(k, v)| (k, String::from(v)))
        .collect();
    let mut block = target.block();
    let identity = block.publication_identity();
    block.insert(2, "two changed".into());
    block.remove(3);
    block.insert(9, "nine".into());
    let pointer = block.get(&2).unwrap().as_ptr();
    let before = block.get_before_block(&2).unwrap().as_ptr();
    let original = block.try_detach(|_| Ok::<_, ()>(())).unwrap();
    {
        let undo = target
            .revert
            .try_write()
            .expect("original undo writer released");
        let current = target
            .blocks
            .try_write()
            .expect("original current writer released");
        assert_reads(&original);
        assert_eq!(original.publication_identity(), identity);
        assert_eq!(original.get(&2).unwrap().as_ptr(), pointer);
        assert_eq!(original.get_before_block(&2).unwrap().as_ptr(), before);
        assert_eq!(
            original.get_before_block(&3).map(String::as_str),
            Some("three")
        );
        assert_eq!(
            original.get_before_block(&4).map(String::as_str),
            Some("four")
        );
        assert!(original.get_before_block(&9).is_none());
        drop((undo, current));
    }
    let held = target.blocks.write();
    let (original, refusal, cleanup) = original
        .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
        .err()
        .expect("real current writer refuses");
    assert!(matches!(refusal, PublicationPreparationError::Busy(_)));
    assert!(target.revert.try_write().is_some());
    assert_reads(&original);
    assert_eq!(original.get(&2).unwrap().as_ptr(), pointer);
    assert_eq!(original.publication_identity(), identity);
    drop(held);
    drop(cleanup);
    let prepared = original
        .try_prepare_publication(&target, |journal, _| {
            assert_reads(journal);
            Ok::<_, ()>(())
        })
        .unwrap_or_else(|(_, error, _)| panic!("original same-cut retry: {error:?}"));
    drop(prepared.publish());
    assert_eq!(target.view().get(&2).unwrap().as_ptr(), pointer);
}

#[test]
fn frozen_map_reads_do_not_authorize_equal_foreign_or_stale_generation_installation() {
    let target: Storage<_, _> = [(1, 10_u64)].into_iter().collect();
    let foreign: Storage<_, _> = [(1, 10_u64)].into_iter().collect();
    let original = target.block().try_detach(|_| Ok::<_, ()>(())).unwrap();
    let identity = original.publication_identity();
    let pointer = std::ptr::from_ref(original.get(&1).unwrap());
    let (original, refusal, cleanup) = original
        .try_prepare_publication(&foreign, |_, _| {
            Err::<(), _>("foreign source must fail before admission")
        })
        .err()
        .expect("foreign source refused");
    assert_eq!(refusal, PublicationPreparationError::Changed);
    drop(cleanup);
    target.block().commit();
    assert_eq!(target.view().get(&1), original.get(&1));
    assert_ne!(target.block().publication_identity(), identity);
    let (original, refusal, cleanup) = original
        .try_prepare_publication(&target, |_, _| {
            Err::<(), _>("stale generation must fail before admission")
        })
        .err()
        .expect("equal new generation refused");
    assert_eq!(refusal, PublicationPreparationError::Changed);
    drop(cleanup);
    assert_eq!(original.get_before_block(&1), Some(&10));
    assert_eq!(std::ptr::from_ref(original.get(&1).unwrap()), pointer);
    assert_eq!(original.publication_identity(), identity);
}

#[test]
fn frozen_replacement_map_keeps_exact_cut_and_borrowed_key_reads_without_target() {
    let original = {
        let target: Storage<_, _> = [(String::from("one"), 10_u64), (String::from("two"), 20)]
            .into_iter()
            .collect();
        let mut tip = target.block();
        tip.insert("one".into(), 11);
        tip.remove("two".to_owned());
        tip.insert("three".into(), 30);
        tip.commit();
        let mut replacement = target.block_and_revert();
        replacement.insert("one".into(), 12);
        let original = replacement.try_detach(|_| Ok::<_, ()>(())).unwrap();
        assert_eq!(target.view().get("one"), Some(&11));
        assert_eq!(original.mode(), BlockMode::Replace);
        original
    };
    assert_eq!(original.get("one"), Some(&12));
    assert_eq!(original.get_before_block(&String::from("one")), Some(&10));
    assert_eq!(original.get("two"), Some(&20));
    assert!(original.get("three").is_none());
    assert_eq!(
        original
            .range::<str>((
                std::ops::Bound::Included("one"),
                std::ops::Bound::Excluded("two")
            ))
            .count(),
        1
    );
}
