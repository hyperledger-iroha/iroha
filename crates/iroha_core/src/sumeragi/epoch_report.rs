//! In-memory snapshot store for per-epoch VRF penalties.
//! Not consensus-critical. Used by operator endpoints.

use core::sync::atomic::{AtomicU64, Ordering};
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};

/// Report for VRF penalties at a given epoch.
#[derive(Clone, Debug)]
pub struct VrfPenaltiesReport {
    /// Epoch index this report describes.
    pub epoch: u64,
    /// Validators that committed without a valid reveal in the epoch.
    pub committed_no_reveal: Vec<u32>,
    /// Validators that neither committed nor revealed in the epoch.
    pub no_participation: Vec<u32>,
    /// Roster length (validators in the epoch roster snapshot).
    pub roster_len: u32,
}

static REPORTS: OnceLock<Mutex<BTreeMap<u64, VrfPenaltiesReport>>> = OnceLock::new();
static LAST_EPOCH: AtomicU64 = AtomicU64::new(0);

fn reports() -> &'static Mutex<BTreeMap<u64, VrfPenaltiesReport>> {
    REPORTS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Insert or replace the report for an epoch.
pub fn update(report: VrfPenaltiesReport) {
    let mut g = reports().lock().unwrap();
    LAST_EPOCH.store(report.epoch, Ordering::Relaxed);
    g.insert(report.epoch, report);
}

/// Fetch the report for a specific epoch, if present.
pub fn get(epoch: u64) -> Option<VrfPenaltiesReport> {
    reports().lock().unwrap().get(&epoch).cloned()
}

/// Return the latest epoch index for which a report was stored (best-effort).
pub fn last_epoch_index() -> u64 {
    LAST_EPOCH.load(Ordering::Relaxed)
}

/// Clear all reports (tests only).
#[cfg(test)]
pub fn clear() {
    reports().lock().unwrap().clear();
    LAST_EPOCH.store(0, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_and_get_roundtrip() {
        clear();
        let r = VrfPenaltiesReport {
            epoch: 7,
            committed_no_reveal: vec![1, 3],
            no_participation: vec![2],
            roster_len: 5,
        };
        update(r.clone());
        assert_eq!(last_epoch_index(), 7);
        let got = get(7).expect("report present");
        assert_eq!(got.epoch, 7);
        assert_eq!(got.roster_len, 5);
        assert_eq!(got.committed_no_reveal, vec![1, 3]);
        assert_eq!(got.no_participation, vec![2]);
    }
}
