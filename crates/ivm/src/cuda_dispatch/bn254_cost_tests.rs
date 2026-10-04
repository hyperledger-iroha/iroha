//! Driverless tests exercise the exact production profile and scheduling state.

use super::*;
use std::cell::Cell;

fn key() -> ProfileKey {
    ProfileKey {
        artifact: PtxArtifact::new(c"cost-test-ptx"),
        cpu: TypeId::of::<u64>(),
    }
}

fn profile(cpu: u64, gpu: u64) -> CostProfile {
    CostProfile::from_trials(
        [TrialSample {
            cpu_ns: [cpu; TRIALS],
            gpu_ns: [gpu; TRIALS],
        }; SIZES.len()],
    )
    .unwrap()
}

#[test]
fn complete_costs_need_stable_clear_win_and_bounded_geometry() {
    let fast = profile(100, 80);
    for items in [64, 200, 256, 1_024, 4_096] {
        assert_eq!(fast.estimate(items), Some(80));
    }
    for items in [0, 1, 63, 4_097, usize::MAX] {
        assert_eq!(fast.estimate(items), None);
    }
    assert_eq!(profile(100, 91).estimate(64), None);
    assert_eq!(profile(u64::MAX, u64::MAX - 1).estimate(64), None);
    assert_eq!(
        profile(u64::MAX, u64::MAX / 2).estimate(64),
        Some(u64::MAX / 2)
    );
    for trial in [
        TrialSample {
            cpu_ns: [0, 10, 10],
            gpu_ns: [1; TRIALS],
        },
        TrialSample {
            cpu_ns: [10, 41, 10],
            gpu_ns: [1; TRIALS],
        },
        TrialSample {
            cpu_ns: [100; TRIALS],
            gpu_ns: [1, 5, 1],
        },
    ] {
        assert!(CostProfile::from_trials([trial; SIZES.len()]).is_none());
    }
}

#[test]
fn conservative_bounds_use_fastest_cpu_and_slowest_native_trial() {
    let trials = [TrialSample {
        cpu_ns: [100, 120, 180],
        gpu_ns: [50, 70, 91],
    }; SIZES.len()];
    assert_eq!(CostProfile::from_trials(trials).unwrap().estimate(64), None);
}

#[test]
fn interpolated_device_ranking_can_cross_without_extrapolating() {
    let make = |gpus: [u64; 4]| {
        CostProfile::from_trials(std::array::from_fn(|i| TrialSample {
            cpu_ns: [1_000; TRIALS],
            gpu_ns: [gpus[i]; TRIALS],
        }))
        .unwrap()
    };
    let a = make([40, 100, 200, 400]);
    let b = make([80, 90, 100, 110]);
    assert!(a.estimate(64) < b.estimate(64));
    assert!(a.estimate(256) > b.estimate(256));
    assert_eq!(a.estimate(160), Some(70));
    assert_eq!(a.estimate(4_097), None);
}

#[test]
fn profiles_belong_to_exact_artifact_cpu_and_original_cell() {
    let cell = ProfileCell::default();
    let other_owner = ProfileCell::default();
    let scheduler = CalibrationScheduler::new();
    let now = Instant::now();
    let mut pass = scheduler.try_pass(2, now).unwrap();
    assert_eq!(
        cell.estimate(key(), 64, now, Some(&mut pass), |_| Ok(profile(100, 20))),
        Some(20)
    );
    assert_eq!(
        cell.estimate(key(), 64, now, None, |_| panic!("cached")),
        Some(20)
    );
    assert_eq!(
        other_owner.estimate(key(), 64, now, None, |_| panic!("not scheduled")),
        None
    );
    let wrong_cpu = ProfileKey {
        cpu: TypeId::of::<u32>(),
        ..key()
    };
    let wrong_artifact = ProfileKey {
        artifact: PtxArtifact::new(c"different"),
        ..key()
    };
    for wrong in [wrong_cpu, wrong_artifact] {
        assert_eq!(
            cell.estimate(wrong, 64, now, None, |_| panic!("not scheduled")),
            None
        );
    }
    assert_eq!(
        cell.estimate(key(), 64, now, None, |_| panic!("original retained")),
        Some(20)
    );
}

#[test]
fn local_refusal_cools_down_without_poisoning_a_later_profile() {
    for failure in [
        CalibrationFailure::Deferred,
        CalibrationFailure::Deadline,
        CalibrationFailure::Backend,
        CalibrationFailure::Parity,
    ] {
        let cell = ProfileCell::default();
        let scheduler = CalibrationScheduler::new();
        let now = Instant::now();
        {
            let mut pass = scheduler.try_pass(1, now).unwrap();
            assert_eq!(
                cell.estimate(key(), 64, now, Some(&mut pass), |_| Err(failure)),
                None
            );
        }
        let mut pass = scheduler.try_pass(1, now).unwrap();
        assert_eq!(
            cell.estimate(key(), 64, now, Some(&mut pass), |_| panic!("cooldown")),
            None
        );
        let later = now + RETRY_DELAY;
        drop(pass);
        let mut pass = scheduler.try_pass(1, later).unwrap();
        assert_eq!(
            cell.estimate(key(), 64, later, Some(&mut pass), |_| Ok(profile(100, 20))),
            Some(20)
        );
    }
}

#[test]
fn one_global_pass_is_nonblocking_bounded_and_round_robin() {
    let scheduler = CalibrationScheduler::new();
    let now = Instant::now();
    assert!(scheduler.try_pass(0, now).is_none());
    for expected in [0, 1, 2, 0] {
        let mut pass = scheduler.try_pass(3, now).unwrap();
        assert_eq!(pass.start, expected);
        assert!(scheduler.try_pass(3, now).is_none());
        assert_eq!(pass.begin(now), Some(now + PASS_BUDGET));
        assert_eq!(pass.begin(now), None);
    }
    let mut pass = scheduler.try_pass(3, now).unwrap();
    assert_eq!(pass.begin(now + PASS_BUDGET), None);
}

#[test]
fn concurrent_borrow_and_out_of_range_never_execute_calibration() {
    let cell = ProfileCell::default();
    let scheduler = CalibrationScheduler::new();
    let now = Instant::now();
    let mut pass = scheduler.try_pass(1, now).unwrap();
    let borrowed = cell.0.lock();
    assert_eq!(
        cell.estimate(key(), 64, now, Some(&mut pass), |_| panic!("busy")),
        None
    );
    drop(borrowed);
    assert_eq!(
        cell.estimate(key(), 4_097, now, Some(&mut pass), |_| panic!(
            "outside measured span"
        )),
        None
    );
    let ran = Cell::new(false);
    assert_eq!(
        cell.estimate(key(), 64, now, Some(&mut pass), |_| {
            ran.set(true);
            Ok(profile(100, 20))
        }),
        Some(20)
    );
    assert!(ran.get());
}

#[test]
fn unwind_releases_locks_and_keeps_partial_profile_unpublished() {
    let cell = ProfileCell::default();
    let scheduler = CalibrationScheduler::new();
    let now = Instant::now();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut pass = scheduler.try_pass(1, now).unwrap();
            cell.estimate(key(), 64, now, Some(&mut pass), |_| panic!("native unwind"));
        }))
        .is_err()
    );
    let mut pass = scheduler.try_pass(1, now).unwrap();
    assert_eq!(
        cell.estimate(key(), 64, now, Some(&mut pass), |_| panic!("cooldown")),
        None
    );
}
