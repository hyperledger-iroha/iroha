//! Global, in-memory `NEW_VIEW` receipt tracker for operator introspection.
//! Not consensus-critical. Used by Torii SSE to stream (height, view) counts.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Mutex, OnceLock},
};

type Key = (u64, u64); // (height, view)

#[derive(Default)]
struct Store {
    by_hv: BTreeMap<Key, BTreeSet<u32>>, // sender indices per (h,v)
}

static GLOBAL: OnceLock<Mutex<Store>> = OnceLock::new();

fn global() -> &'static Mutex<Store> {
    GLOBAL.get_or_init(|| Mutex::new(Store::default()))
}

/// Note a `NEW_VIEW` receipt from `sender` for (height, view). Returns the current count.
pub fn note_receipt(height: u64, view: u64, sender: u32) -> u64 {
    let mut g = global().lock().unwrap();
    let set = g.by_hv.entry((height, view)).or_default();
    set.insert(sender);
    set.len() as u64
}

/// Snapshot deduplicated counts as a flat vector of (height, view, count).
pub fn snapshot_counts() -> Vec<(u64, u64, u64)> {
    let g = global().lock().unwrap();
    g.by_hv
        .iter()
        .map(|(&(h, v), set)| (h, v, set.len() as u64))
        .collect()
}
