//! Borrowed committed and prior images for validation and derived storage.

use super::{Iter, Storage, StorageReadOnly, View};
use crate::{Key, Value};
use concread::ebrcell::{EbrCell, EbrCellReadTxn};
use std::{
    borrow::Borrow,
    cmp::Ordering,
    collections::{BTreeMap, btree_map},
    iter::Peekable,
    marker::PhantomData,
};

/// Read-only guard over a storage's current state and retained undo history.
///
/// The exclusive source borrow prevents writes while both images are acquired
/// and inspected. Reading history does not begin a block or consume the undo log.
pub struct History<'storage, K: Key, V: Value> {
    current: View<'storage, K, V>,
    revert: EbrCellReadTxn<BTreeMap<K, Option<V>>>,
    _exclusive: PhantomData<&'storage mut Storage<K, V>>,
}

impl<K: Key, V: Value> Storage<K, V> {
    /// Borrow the current image and the state before the latest committed block.
    ///
    /// This operation does not mutate storage. Exclusive access keeps acquisition
    /// of the two read guards coherent and excludes writes for their lifetime.
    pub fn history(&mut self) -> History<'_, K, V> {
        let revert = self.revert.read();
        let current = self.view();
        History {
            current,
            revert,
            _exclusive: PhantomData,
        }
    }
}

impl<'storage, K: Key, V: Value> History<'storage, K, V> {
    /// Borrow the committed storage image.
    pub fn current(&self) -> &View<'storage, K, V> {
        &self.current
    }

    /// Borrow touched keys and their values before the last committed block.
    ///
    /// `None` records prior absence; it is not a missing undo entry. Untouched
    /// keys inherit their current values in the prior image.
    pub fn revert_map(&self) -> &BTreeMap<K, Option<V>> {
        &self.revert
    }

    /// Read a value in the image before the latest committed block.
    pub fn get_before_block<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        match self.revert.get(key) {
            Some(previous) => previous.as_ref(),
            None => self.current.get(key),
        }
    }

    /// Iterate over the prior image in canonical key order without cloning values.
    ///
    /// The iterator merges unchanged current entries with retained prior values;
    /// prior-absence tombstones suppress keys inserted by the latest block.
    pub fn iter_before_block(&self) -> impl Iterator<Item = (&K, &V)> {
        BeforeBlockIter {
            current: self.current.iter().peekable(),
            revert: self.revert.iter().peekable(),
        }
    }

    /// Derive another storage while preserving both images and touched-key history.
    ///
    /// Apply the same pure projection to current and retained prior values. `None`
    /// excludes a value from the derived image. Source values remain borrowed;
    /// only destination keys are cloned. The original undo map is never consumed.
    /// Redundant tombstones are retained so no source touch is erased.
    pub fn project<U: Value>(&self, project: impl Fn(&V) -> Option<U>) -> Storage<K, U> {
        let blocks = self
            .current
            .iter()
            .filter_map(|(key, value)| project(value).map(|value| (key.clone(), value)))
            .collect();
        let revert = self
            .revert
            .iter()
            .map(|(key, previous)| (key.clone(), previous.as_ref().and_then(&project)))
            .collect();
        Storage {
            publication: crate::publication::Publication::new(),
            revert: EbrCell::new(revert),
            blocks,
        }
    }
}

struct BeforeBlockIter<'a, K: Key, V: Value> {
    current: Peekable<Iter<'a, K, V>>,
    revert: Peekable<btree_map::Iter<'a, K, Option<V>>>,
}

impl<'a, K: Key, V: Value> Iterator for BeforeBlockIter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match (self.current.peek(), self.revert.peek()) {
                (None, None) => return None,
                (Some(_), None) => return self.current.next(),
                (None, Some(_)) => {}
                (Some((current_key, _)), Some((previous_key, _))) => {
                    match current_key.cmp(previous_key) {
                        Ordering::Less => return self.current.next(),
                        Ordering::Equal => {
                            self.current.next();
                        }
                        Ordering::Greater => {}
                    }
                }
            }
            if let Some((key, Some(value))) = self.revert.next() {
                return Some((key, value));
            }
        }
    }
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
