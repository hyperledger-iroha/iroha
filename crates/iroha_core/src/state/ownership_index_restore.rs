//! Restore grouped ownership indexes from both authoritative MV images.

use super::*;
use mv::storage::History;

/// Project source membership without consuming its retained block predecessor.
///
/// Each touched record touches its old and new buckets, even when membership is
/// unchanged. Buckets are derived from complete images so untouched members of a
/// touched bucket remain present when the latest block is replaced.
pub(super) fn grouped<K: mv::Key, V: mv::Value, B: mv::Key>(
    history: &History<'_, K, V>,
    bucket: impl Fn(&K, &V) -> B,
) -> Storage<B, BTreeSet<K>> {
    let mut current = BTreeMap::<B, BTreeSet<K>>::new();
    for (key, value) in history.current().iter() {
        current
            .entry(bucket(key, value))
            .or_default()
            .insert(key.clone());
    }
    let mut previous = BTreeMap::<B, BTreeSet<K>>::new();
    for (key, value) in history.iter_before_block() {
        previous
            .entry(bucket(key, value))
            .or_default()
            .insert(key.clone());
    }
    let mut touched = BTreeSet::new();
    for (key, prior) in history.revert_map() {
        for value in [prior.as_ref(), history.current().get(key)]
            .into_iter()
            .flatten()
        {
            touched.insert(bucket(key, value));
        }
    }
    let undo = touched
        .into_iter()
        .map(|key| {
            let prior = previous.remove(&key);
            (key, prior)
        })
        .collect();
    Storage::from_snapshot_parts(current, undo)
}

#[cfg(test)]
#[path = "ownership_index_restore_tests.rs"]
mod tests;
