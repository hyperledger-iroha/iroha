//! In-memory snapshot store for per-epoch VRF penalties.
//! Not consensus-critical. Used by operator endpoints.

use core::sync::atomic::{AtomicU64, Ordering};
use std::{
    collections::BTreeMap,
    sync::{Mutex, MutexGuard, OnceLock},
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

fn lock_reports() -> MutexGuard<'static, BTreeMap<u64, VrfPenaltiesReport>> {
    match reports().lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            iroha_logger::warn!(
                "VRF penalties epoch-report mutex was poisoned; recovering operator report store"
            );
            poisoned.into_inner()
        }
    }
}

/// Insert or replace the report for an epoch.
pub fn update(report: VrfPenaltiesReport) {
    let mut g = lock_reports();
    LAST_EPOCH.store(report.epoch, Ordering::Relaxed);
    g.insert(report.epoch, report);
}

/// Fetch the report for a specific epoch, if present.
pub fn get(epoch: u64) -> Option<VrfPenaltiesReport> {
    lock_reports().get(&epoch).cloned()
}

/// Return the latest epoch index for which a report was stored (best-effort).
pub fn last_epoch_index() -> u64 {
    LAST_EPOCH.load(Ordering::Relaxed)
}

/// Clear all reports (tests only).
#[cfg(test)]
pub fn clear() {
    lock_reports().clear();
    LAST_EPOCH.store(0, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    use super::*;

    fn report_test_guard() -> MutexGuard<'static, ()> {
        static REPORT_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        REPORT_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("report test mutex poisoned")
    }

    fn assert_report(
        report: VrfPenaltiesReport,
        epoch: u64,
        committed_no_reveal: Vec<u32>,
        no_participation: Vec<u32>,
        roster_len: u32,
    ) {
        assert_eq!(report.epoch, epoch);
        assert_eq!(report.committed_no_reveal, committed_no_reveal);
        assert_eq!(report.no_participation, no_participation);
        assert_eq!(report.roster_len, roster_len);
    }

    #[test]
    fn update_and_get_roundtrip() {
        let _guard = report_test_guard();
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

    #[test]
    fn vrf_penalties_report_formal_gate_update_replace_and_latest_write() {
        let _guard = report_test_guard();
        clear();
        assert_eq!(last_epoch_index(), 0);
        assert!(get(0).is_none());
        assert!(get(7).is_none());

        update(VrfPenaltiesReport {
            epoch: 7,
            committed_no_reveal: vec![1, 3],
            no_participation: vec![2],
            roster_len: 5,
        });
        assert_eq!(last_epoch_index(), 7);
        assert_report(
            get(7).expect("epoch 7 report should be stored"),
            7,
            vec![1, 3],
            vec![2],
            5,
        );

        update(VrfPenaltiesReport {
            epoch: 7,
            committed_no_reveal: vec![4],
            no_participation: vec![0, 6],
            roster_len: 8,
        });
        assert_eq!(last_epoch_index(), 7);
        assert_report(
            get(7).expect("epoch 7 replacement should be stored"),
            7,
            vec![4],
            vec![0, 6],
            8,
        );

        update(VrfPenaltiesReport {
            epoch: 9,
            committed_no_reveal: vec![5],
            no_participation: Vec::new(),
            roster_len: 10,
        });
        assert_eq!(last_epoch_index(), 9);
        assert_report(
            get(7).expect("earlier epoch report should be retained"),
            7,
            vec![4],
            vec![0, 6],
            8,
        );
        assert_report(
            get(9).expect("later epoch report should be retained"),
            9,
            vec![5],
            Vec::new(),
            10,
        );

        update(VrfPenaltiesReport {
            epoch: 4,
            committed_no_reveal: Vec::new(),
            no_participation: vec![1],
            roster_len: 3,
        });
        assert_eq!(
            last_epoch_index(),
            4,
            "latest epoch follows the latest write, not max(epoch)"
        );
        assert_report(
            get(9).expect("higher epoch report should still be retained"),
            9,
            vec![5],
            Vec::new(),
            10,
        );
        assert_report(
            get(4).expect("backward latest write should be stored"),
            4,
            Vec::new(),
            vec![1],
            3,
        );
    }

    #[test]
    fn vrf_penalties_report_formal_gate_clear_missing_and_get_is_read_only() {
        let _guard = report_test_guard();
        clear();
        assert!(get(42).is_none());

        update(VrfPenaltiesReport {
            epoch: 11,
            committed_no_reveal: vec![2],
            no_participation: vec![0, 1],
            roster_len: 4,
        });
        let first = get(11).expect("report should exist before read-only check");
        let second = get(11).expect("get must not remove the report");
        assert_report(first, 11, vec![2], vec![0, 1], 4);
        assert_report(second, 11, vec![2], vec![0, 1], 4);

        clear();
        assert_eq!(last_epoch_index(), 0);
        assert!(get(11).is_none());

        update(VrfPenaltiesReport {
            epoch: 12,
            committed_no_reveal: Vec::new(),
            no_participation: Vec::new(),
            roster_len: 6,
        });
        assert_eq!(last_epoch_index(), 12);
        assert_report(
            get(12).expect("post-clear update should be stored"),
            12,
            Vec::new(),
            Vec::new(),
            6,
        );
    }

    #[test]
    fn vrf_penalties_report_recovers_poisoned_store() {
        let _guard = report_test_guard();
        clear();

        let _ = std::panic::catch_unwind(|| {
            let mut guard = reports()
                .lock()
                .expect("VRF penalties report lock should be held");
            guard.insert(
                2,
                VrfPenaltiesReport {
                    epoch: 2,
                    committed_no_reveal: vec![0],
                    no_participation: Vec::new(),
                    roster_len: 1,
                },
            );
            panic!("poison VRF penalties report store for recovery test");
        });

        update(VrfPenaltiesReport {
            epoch: 13,
            committed_no_reveal: vec![1, 4],
            no_participation: vec![2],
            roster_len: 5,
        });
        assert_eq!(last_epoch_index(), 13);
        assert_report(
            get(13).expect("post-poison report should be stored"),
            13,
            vec![1, 4],
            vec![2],
            5,
        );

        clear();
        assert_eq!(last_epoch_index(), 0);
        assert!(get(13).is_none());
    }
}
