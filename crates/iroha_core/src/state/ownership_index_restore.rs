//! Restore ownership membership-row indexes from both authoritative MV images.

use super::*;
use mv::storage::History;

/// Project one row per membership while retaining both authoritative MV images.
///
/// Source writes retain their old and new membership preimages, including a
/// remove/insert of an unchanged membership. Unrelated members never share a row.
#[expect(
    clippy::zero_sized_map_values,
    reason = "MV snapshot restoration requires complete current and undo key/value maps"
)]
pub(super) fn members<K: mv::Key, V: mv::Value, B: mv::Key>(
    history: &History<'_, K, V>,
    bucket: impl Fn(&K, &V) -> Option<B>,
) -> Storage<(B, K), ()> {
    let current = history
        .current()
        .iter()
        .filter_map(|(key, value)| bucket(key, value).map(|bucket| ((bucket, key.clone()), ())))
        .collect();
    let previous: BTreeMap<_, _> = history
        .iter_before_block()
        .filter_map(|(key, value)| bucket(key, value).map(|bucket| ((bucket, key.clone()), ())))
        .collect();
    let mut touched = BTreeSet::new();
    for (key, prior) in history.revert_map().iter() {
        for value in [prior.as_ref(), history.current().get(key)]
            .into_iter()
            .flatten()
        {
            if let Some(bucket) = bucket(key, value) {
                touched.insert((bucket, key.clone()));
            }
        }
    }
    let undo = touched
        .into_iter()
        .map(|key| {
            let prior = previous.get(&key).copied();
            (key, prior)
        })
        .collect();
    Storage::from_snapshot_parts(current, undo)
}

#[cfg(test)]
#[path = "ownership_index_restore_tests.rs"]
mod tests;
