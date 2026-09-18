//! Borrowed current and undo maps for generation-fenced snapshot capture.

use super::{Storage, View};
use crate::{Key, Value};
use concread::ebrcell::{EbrCell, EbrCellReadTxn};
use std::collections::BTreeMap;

/// Read guards retaining both serialized maps without consuming undo history.
///
/// Acquisition is not atomic across the two maps. The owner must fence capture
/// with its publication generation and discard it if that generation changes.
pub struct Snapshot<'a, K: Key, V: Value> {
    current: View<'a, K, V>,
    revert: EbrCellReadTxn<BTreeMap<K, Option<V>>>,
}

impl<K: Key, V: Value> Storage<K, V> {
    /// Acquire current and undo guards for an externally generation-fenced capture.
    pub fn snapshot(&self) -> Snapshot<'_, K, V> {
        let revert = self.revert.read();
        let current = self.view();
        Snapshot { current, revert }
    }

    /// Restore exact current entries and predecessor preimages from a snapshot.
    ///
    /// A `None` preimage records prior absence even when the key is also absent
    /// now. Callers validate their schema and canonical key ordering before this
    /// constructor; neither undo nor deleted entries are inferred from current data.
    pub fn from_snapshot_parts(current: BTreeMap<K, V>, revert: BTreeMap<K, Option<V>>) -> Self {
        Self {
            publication: crate::publication::Publication::new(),
            blocks: current.into_iter().collect(),
            revert: EbrCell::new(revert),
        }
    }
}

impl<'a, K: Key, V: Value> Snapshot<'a, K, V> {
    /// Borrow the committed entries retained at acquisition.
    pub fn current(&self) -> &View<'a, K, V> {
        &self.current
    }

    /// Borrow exact touched keys, including deleted values and prior absence.
    pub fn revert_map(&self) -> &BTreeMap<K, Option<V>> {
        &self.revert
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::StorageReadOnly;

    #[test]
    fn snapshot_preserves_deletions_insertions_and_redundant_absence() {
        let storage: Storage<u64, u64> = [(1, 10), (2, 20)].into_iter().collect();
        let mut block = storage.block();
        block.insert(1, 11);
        block.remove(2);
        block.insert(3, 30);
        block.remove(4);
        block.commit();
        let snapshot = storage.snapshot();
        assert_eq!(
            snapshot.revert_map(),
            &BTreeMap::from([(1, Some(10)), (2, Some(20)), (3, None), (4, None),])
        );
        let restored = Storage::from_snapshot_parts(
            snapshot.current().iter().map(|(k, v)| (*k, *v)).collect(),
            snapshot.revert_map().clone(),
        );
        assert_eq!(restored.snapshot().revert_map(), snapshot.revert_map());
        let previous = restored.block_and_revert();
        assert_eq!(
            previous.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>(),
            [(1, 10), (2, 20)]
        );
        drop(previous);
        assert_eq!(storage.view().get(&1), Some(&11));
        assert_eq!(snapshot.current().get(&3), Some(&30));
    }
}
