//! Exact-artifact qualification and nonblocking concurrency controls.

use super::*;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Barrier},
};

const ARTIFACT: PtxArtifact = PtxArtifact::new(c"qualified PTX bytes");
const OTHER: PtxArtifact = PtxArtifact::new(c"different PTX bytes");

#[test]
fn only_the_same_exact_artifact_reuses_a_successful_qualification() {
    let entry = KernelAdmission::default();
    assert!(entry.admit(ARTIFACT, || Ok(true)));
    assert!(entry.admitted(ARTIFACT));
    assert!(!entry.admitted(OTHER));
    assert!(entry.admit(ARTIFACT, || panic!("same artifact revalidated")));
    assert!(!entry.admit(OTHER, || panic!("different artifact reused qualification")));
    assert!(!entry.can_attempt(OTHER));
    assert!(entry.can_attempt(ARTIFACT));
}

#[test]
fn failure_is_sticky_and_does_not_modify_another_kernel() {
    let failed = KernelAdmission::default();
    let healthy = KernelAdmission::default();
    assert!(!failed.admit(ARTIFACT, || Ok(false)));
    assert!(!failed.admit(ARTIFACT, || panic!("failed artifact retried")));
    assert!(!failed.can_attempt(ARTIFACT));
    assert!(healthy.admit(ARTIFACT, || Ok(true)));
}

#[test]
fn quarantine_during_a_successful_validation_wins() {
    let entry = KernelAdmission::default();
    assert!(!entry.admit(ARTIFACT, || {
        entry.quarantine();
        Ok(true)
    }));
    assert!(!entry.can_attempt(ARTIFACT));
}

#[test]
fn recursive_admission_declines_without_waiting_or_poisoning_the_owner() {
    let entry = KernelAdmission::default();
    assert!(entry.admit(ARTIFACT, || {
        assert!(!entry.admit(ARTIFACT, || panic!("recursive validator")));
        assert!(entry.can_attempt(ARTIFACT));
        Ok(true)
    }));
    assert!(entry.admit(ARTIFACT, || panic!("successful validation lost")));
}

#[test]
fn concurrent_validation_is_local_busy_then_reuses_the_completed_result() {
    let entry = Arc::new(KernelAdmission::default());
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let worker = {
        let entry = Arc::clone(&entry);
        let entered = Arc::clone(&entered);
        let release = Arc::clone(&release);
        std::thread::spawn(move || {
            entry.admit(ARTIFACT, || {
                entered.wait();
                release.wait();
                Ok(true)
            })
        })
    };
    entered.wait();
    assert!(!entry.admit(ARTIFACT, || panic!("parallel validator")));
    assert!(entry.can_attempt(ARTIFACT));
    release.wait();
    assert!(worker.join().unwrap());
    assert!(entry.admit(ARTIFACT, || panic!("completed result not reused")));
}

#[test]
fn validator_unwind_quarantines_the_poisoned_entry() {
    let entry = KernelAdmission::default();
    assert!(
        catch_unwind(AssertUnwindSafe(
            || entry.admit(ARTIFACT, || panic!("validator"))
        ))
        .is_err()
    );
    assert!(!entry.can_attempt(ARTIFACT));
    assert!(!entry.admitted(ARTIFACT));
    assert!(!entry.admit(ARTIFACT, || panic!("poison retried")));
    assert!(!entry.can_attempt(ARTIFACT));
}

#[test]
fn local_refusal_preserves_exact_artifact_for_later_qualification() {
    for refusal in [
        CudaFailure::Capacity,
        CudaFailure::Busy,
        CudaFailure::Unavailable,
    ] {
        let entry = KernelAdmission::default();
        assert!(!entry.admit(ARTIFACT, || Err(refusal)));
        assert!(!entry.admitted(ARTIFACT));
        assert!(entry.can_attempt(ARTIFACT));
        assert!(!entry.can_attempt(OTHER));
        assert!(!entry.admit(OTHER, || panic!("refusal lost artifact binding")));
        // Repeated pressure neither fabricates qualification nor makes the
        // refusal sticky. Only the completed parity result admits this artifact.
        assert!(!entry.admit(ARTIFACT, || Err(refusal)));
        assert!(entry.admit(ARTIFACT, || Ok(true)));
        assert!(entry.admitted(ARTIFACT));
        assert!(entry.admit(ARTIFACT, || panic!("qualified artifact revalidated")));
    }
}

#[test]
fn backend_faults_remain_quarantined_after_resources_return() {
    for failure in [
        CudaFailure::Quarantined,
        CudaFailure::InvalidRequest,
        CudaFailure::Driver(2),
        CudaFailure::Timeout,
    ] {
        let entry = KernelAdmission::default();
        assert!(!entry.admit(ARTIFACT, || Err(CudaFailure::Capacity)));
        assert!(!entry.admit(ARTIFACT, || Err(failure)));
        assert!(!entry.can_attempt(ARTIFACT));
        assert!(!entry.admit(ARTIFACT, || panic!("backend fault was forgotten")));
    }
}

#[test]
fn concurrent_quarantine_wins_over_retryable_refusal() {
    for refusal in [
        CudaFailure::Capacity,
        CudaFailure::Busy,
        CudaFailure::Unavailable,
    ] {
        let entry = Arc::new(KernelAdmission::default());
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let worker = {
            let entry = Arc::clone(&entry);
            let entered = Arc::clone(&entered);
            let release = Arc::clone(&release);
            std::thread::spawn(move || {
                entry.admit(ARTIFACT, || {
                    entered.wait();
                    release.wait();
                    Err(refusal)
                })
            })
        };
        entered.wait();
        entry.quarantine();
        release.wait();
        assert!(!worker.join().unwrap());
        assert!(!entry.can_attempt(ARTIFACT));
        assert!(!entry.admit(ARTIFACT, || panic!("quarantine cleared by refusal")));
    }
}

#[test]
fn device_selection_falls_through_refusal_then_retries_the_original_device() {
    use crate::cuda::policy::select_admitted_device;
    let entries = [KernelAdmission::default(), KernelAdmission::default()];
    let mut attempted = Vec::new();
    assert_eq!(
        select_admitted_device(2, 0, None, |index| {
            attempted.push(index);
            entries[index].admit(ARTIFACT, || {
                if index == 0 {
                    Err(CudaFailure::Capacity)
                } else {
                    Ok(true)
                }
            })
        }),
        Some(1)
    );
    assert_eq!(attempted, [0, 1]);
    assert!(!entries[0].admitted(ARTIFACT));
    assert!(entries[1].admitted(ARTIFACT));
    assert_eq!(
        select_admitted_device(2, 0, None, |index| {
            entries[index].admit(ARTIFACT, || Ok(true))
        }),
        Some(0)
    );
    assert!(entries[0].admitted(ARTIFACT));
}
