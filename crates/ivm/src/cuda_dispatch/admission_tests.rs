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
    assert!(entry.admit(ARTIFACT, || true));
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
    assert!(!failed.admit(ARTIFACT, || false));
    assert!(!failed.admit(ARTIFACT, || panic!("failed artifact retried")));
    assert!(!failed.can_attempt(ARTIFACT));
    assert!(healthy.admit(ARTIFACT, || true));
}

#[test]
fn quarantine_during_a_successful_validation_wins() {
    let entry = KernelAdmission::default();
    assert!(!entry.admit(ARTIFACT, || {
        entry.quarantine();
        true
    }));
    assert!(!entry.can_attempt(ARTIFACT));
}

#[test]
fn recursive_admission_declines_without_waiting_or_poisoning_the_owner() {
    let entry = KernelAdmission::default();
    assert!(entry.admit(ARTIFACT, || {
        assert!(!entry.admit(ARTIFACT, || panic!("recursive validator")));
        assert!(entry.can_attempt(ARTIFACT));
        true
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
                true
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
