//! Global, in-memory `NEW_VIEW` receipt tracker for operator introspection.
//! Not consensus-critical. Used by Torii SSE to stream (height, view) counts.
//! Retains a bounded window to avoid unbounded memory growth.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Mutex, MutexGuard, OnceLock},
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

fn lock_store() -> MutexGuard<'static, Store> {
    match global().lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            iroha_logger::warn!(
                "NEW_VIEW stats mutex was poisoned; recovering operator receipt tracker"
            );
            poisoned.into_inner()
        }
    }
}

/// Note a `NEW_VIEW` receipt from `sender` for (height, view). Returns the current count.
pub fn note_receipt(height: u64, view: u64, sender: &PeerId) -> u64 {
    let mut g = lock_store();
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
    let g = lock_store();
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
        let mut guard = lock_store();
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

    #[test]
    fn new_view_stats_recovers_poisoned_store() {
        let _guard = test_guard();
        reset_store();

        let peer = PeerId::new(KeyPair::random().public_key().clone());
        let _ = std::panic::catch_unwind(|| {
            let mut guard = global().lock().expect("new view stats lock should be held");
            guard.by_hv.insert((1, 0), BTreeSet::new());
            panic!("poison NEW_VIEW stats store for recovery test");
        });

        assert_eq!(note_receipt(12, 3, &peer), 1);
        assert!(snapshot_counts().contains(&(12, 3, 1)));

        reset_store();
    }
}
