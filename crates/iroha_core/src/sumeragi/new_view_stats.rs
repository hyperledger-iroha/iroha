//! Global, in-memory `NEW_VIEW` receipt tracker for operator introspection.
//! Not consensus-critical. Used by Torii SSE to stream (height, view) counts.
//! Retains a bounded window to avoid unbounded memory growth.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Mutex, OnceLock},
};

use iroha_data_model::prelude::PeerId;

type Key = (u64, u64); // (height, view)

const NEW_VIEW_STATS_CAP: usize = 1024;

#[derive(Default)]
struct Store {
    by_hv: BTreeMap<Key, BTreeSet<PeerId>>, // sender peers per (h,v)
}

static GLOBAL: OnceLock<Mutex<Store>> = OnceLock::new();

fn global() -> &'static Mutex<Store> {
    GLOBAL.get_or_init(|| Mutex::new(Store::default()))
}

/// Note a `NEW_VIEW` receipt from `sender` for (height, view). Returns the current count.
pub fn note_receipt(height: u64, view: u64, sender: &PeerId) -> u64 {
    let mut g = global().lock().unwrap();
    let key = (height, view);
    {
        let set = g.by_hv.entry(key).or_default();
        set.insert(sender.clone());
    }
    while g.by_hv.len() > NEW_VIEW_STATS_CAP {
        g.by_hv.pop_first();
    }
    g.by_hv.get(&key).map_or(0, |set| set.len() as u64)
}

/// Snapshot deduplicated counts as a flat vector of (height, view, count).
pub fn snapshot_counts() -> Vec<(u64, u64, u64)> {
    let g = global().lock().unwrap();
    g.by_hv
        .iter()
        .map(|(&(h, v), set)| (h, v, set.len() as u64))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::KeyPair;
    use std::sync::{Mutex, OnceLock};

    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("new_view_stats test lock poisoned")
    }

    fn reset_store() {
        let mut guard = global().lock().unwrap();
        guard.by_hv.clear();
    }

    #[test]
    fn note_receipt_deduplicates_senders() {
        let _guard = test_guard();
        reset_store();

        let peer_a = PeerId::new(KeyPair::random().public_key().clone());
        let peer_b = PeerId::new(KeyPair::random().public_key().clone());
        assert_eq!(note_receipt(10, 0, &peer_a), 1);
        assert_eq!(note_receipt(10, 0, &peer_a), 1);
        assert_eq!(note_receipt(10, 0, &peer_b), 2);
    }

    #[test]
    fn note_receipt_prunes_old_entries() {
        let _guard = test_guard();
        reset_store();

        let peer = PeerId::new(KeyPair::random().public_key().clone());
        let total = NEW_VIEW_STATS_CAP + 4;
        for view in 0..total {
            note_receipt(7, view as u64, &peer);
        }

        let snapshot = snapshot_counts();
        assert_eq!(snapshot.len(), NEW_VIEW_STATS_CAP);
        let first = snapshot.first().expect("snapshot should not be empty");
        assert_eq!(first.0, 7);
        assert_eq!(first.1, (total - NEW_VIEW_STATS_CAP) as u64);
        let last = snapshot.last().expect("snapshot should not be empty");
        assert_eq!(last.1, (total - 1) as u64);
    }
}
