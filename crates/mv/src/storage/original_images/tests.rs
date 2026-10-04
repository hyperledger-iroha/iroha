//! Exact physical source images, unchanged identities and comparison-free traversal.

use super::*;
use std::{cell::Cell, cmp::Ordering};

#[test]
fn frozen_images_preserve_ordinary_and_replacement_preimages_and_physical_touches() {
    for mode in [BlockMode::Ordinary, BlockMode::Replace] {
        let target: Storage<u64, String> =
            [(1, "base"), (2, "deleted"), (4, "equal"), (8, "untouched")]
                .into_iter()
                .map(|(key, value)| (key, value.into()))
                .collect();
        let mut tip = target.block();
        tip.insert(1, "tip".into());
        tip.commit();
        let mut block = match mode {
            BlockMode::Ordinary => target.block(),
            BlockMode::Replace => target.block_and_revert(),
        };
        block.insert(1, "successor".into());
        block.remove(2);
        block.remove(3);
        block.insert(4, "equal".into());
        block.insert(5, "new".into());
        let identity = block.publication_identity();
        let current_pointer = block.get(&1).unwrap().as_ptr();
        let before_pointer = block.get_before_block(&1).unwrap().as_ptr();
        let original = block.try_detach(|_| Ok::<_, ()>(())).unwrap();
        let images = original.original_images();
        assert_eq!(images.mode(), mode);
        assert!(images.belongs_to(&target));
        assert_eq!(images.publication_identity(), identity);
        assert_eq!(
            images.current_entries().next().unwrap().1.as_ptr(),
            current_pointer
        );
        assert_eq!(
            images
                .undo_entries()
                .next()
                .unwrap()
                .1
                .as_ref()
                .unwrap()
                .as_ptr(),
            before_pointer
        );
        assert_eq!(
            images
                .current_entries()
                .map(|(k, v)| (*k, v.as_str()))
                .collect::<Vec<_>>(),
            [(1, "successor"), (4, "equal"), (5, "new"), (8, "untouched")]
        );
        assert_eq!(
            images
                .undo_entries()
                .map(|(k, v)| (*k, v.as_deref()))
                .collect::<Vec<_>>(),
            [
                (
                    1,
                    Some(if mode == BlockMode::Ordinary {
                        "tip"
                    } else {
                        "base"
                    })
                ),
                (2, Some("deleted")),
                (3, None),
                (4, Some("equal")),
                (5, None)
            ]
        );
        assert_eq!(
            images
                .undo_entries()
                .rev()
                .map(|(k, _)| *k)
                .collect::<Vec<_>>(),
            [5, 4, 3, 2, 1]
        );
        // A new current generation can neither replace this retained cut nor authorize it.
        let foreign: Storage<u64, String> = [(1, "successor".into())].into_iter().collect();
        assert!(!images.belongs_to(&foreign));
        target.block().commit();
        assert_eq!(images.publication_identity(), identity);
        assert_eq!(
            images.current_entries().next().unwrap().1.as_ptr(),
            current_pointer
        );
        assert!(!original.matches_current(&target));
    }
}

thread_local! {
    static OBSERVE: Cell<bool> = const { Cell::new(false) };
    static COMPARISONS: Cell<usize> = const { Cell::new(0) };
    static COPIES: Cell<usize> = const { Cell::new(0) };
}
#[derive(Debug, Eq, PartialEq)]
struct ObservedKey(u64);
impl Clone for ObservedKey {
    fn clone(&self) -> Self {
        if OBSERVE.get() {
            COPIES.set(COPIES.get() + 1);
        }
        Self(self.0)
    }
}
impl Ord for ObservedKey {
    fn cmp(&self, other: &Self) -> Ordering {
        if OBSERVE.get() {
            COMPARISONS.set(COMPARISONS.get() + 1);
        }
        self.0.cmp(&other.0)
    }
}
impl PartialOrd for ObservedKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
struct Observation;
impl Drop for Observation {
    fn drop(&mut self) {
        OBSERVE.set(false);
    }
}

#[test]
fn raw_iteration_never_hides_key_comparison_clone_or_successor_lookup() {
    let target: Storage<ObservedKey, u64> =
        (0..96).map(|key| (ObservedKey(key), key + 100)).collect();
    let mut block = target.block();
    for key in 0..96 {
        if key % 2 == 0 {
            block.remove(ObservedKey(key));
        } else {
            block.insert(ObservedKey(key), key + 100);
        }
    }
    block.remove(ObservedKey(999));
    let original = block.try_detach(|_| Ok::<_, ()>(())).unwrap();
    COMPARISONS.set(0);
    COPIES.set(0);
    OBSERVE.set(true);
    let guard = Observation;
    let images = original.original_images();
    assert_eq!(images.current_entries().len(), 48);
    assert_eq!(images.undo_entries().len(), 97);
    let current_sum: u64 = images
        .current_entries()
        .map(|(key, value)| key.0 + value)
        .sum();
    let undo_sum: u64 = images
        .undo_entries()
        .rev()
        .map(|(key, before)| key.0 + before.unwrap_or(0))
        .sum();
    assert_eq!(
        current_sum,
        (0..96)
            .filter(|key| key % 2 != 0)
            .map(|key| key * 2 + 100)
            .sum::<u64>()
    );
    assert_eq!(
        undo_sum,
        (0..96).map(|key| key * 2 + 100).sum::<u64>() + 999
    );
    assert_eq!(COMPARISONS.get(), 0);
    assert_eq!(COPIES.get(), 0);
    // Demonstrate that the existing touched-entry traversal really would trip this observer.
    let mut touched = 0;
    for entry in original.touched_entries() {
        std::hint::black_box(entry.after);
        touched += 1;
    }
    assert_eq!(touched, 97);
    assert!(COMPARISONS.get() > 0);
    drop(guard);
}
