//! Physical EBR acquisition, retained refusal, and unlocked reclamation.

use super::*;
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
        Arc, OnceLock, Weak,
    },
};

struct Observations {
    target: OnceLock<Weak<EbrCell<Payload, Charge>>>,
    clones: AtomicUsize,
    clone_panics: AtomicBool,
    payload_drops: AtomicUsize,
    charge_drops: AtomicUsize,
    payload_locked: AtomicBool,
    charge_locked: AtomicBool,
}

struct Payload {
    observations: Arc<Observations>,
    private: bool,
}

impl Clone for Payload {
    fn clone(&self) -> Self {
        self.observations.clones.fetch_add(1, SeqCst);
        assert!(!self.observations.clone_panics.load(SeqCst), "clone fault");
        Self {
            observations: Arc::clone(&self.observations),
            private: true,
        }
    }
}

impl Drop for Payload {
    fn drop(&mut self) {
        if self.private {
            let locked = self
                .observations
                .target
                .get()
                .unwrap()
                .upgrade()
                .is_none_or(|target| target.try_acquire_writer().is_none());
            self.observations.payload_locked.store(locked, SeqCst);
            self.observations.payload_drops.fetch_add(1, SeqCst);
        }
    }
}

struct Charge(Option<Arc<Observations>>);

impl Drop for Charge {
    fn drop(&mut self) {
        if let Some(observations) = &self.0 {
            let locked = observations
                .target
                .get()
                .unwrap()
                .upgrade()
                .is_none_or(|target| target.try_acquire_writer().is_none());
            observations.charge_locked.store(locked, SeqCst);
            observations.charge_drops.fetch_add(1, SeqCst);
        }
    }
}

fn fixture() -> (Arc<EbrCell<Payload, Charge>>, Arc<Observations>) {
    let observations = Arc::new(Observations {
        target: OnceLock::new(),
        clones: AtomicUsize::new(0),
        clone_panics: AtomicBool::new(false),
        payload_drops: AtomicUsize::new(0),
        charge_drops: AtomicUsize::new(0),
        payload_locked: AtomicBool::new(false),
        charge_locked: AtomicBool::new(false),
    });
    let target = Arc::new(EbrCell::new_charged(
        Payload {
            observations: Arc::clone(&observations),
            private: false,
        },
        Charge(None),
    ));
    assert!(observations.target.set(Arc::downgrade(&target)).is_ok());
    (target, observations)
}

#[test]
fn raw_acquisition_and_refusal_retain_the_exact_writer_without_cloning() {
    let (target, observations) = fixture();
    let acquired = target.acquire_writer();
    assert!(target.try_acquire_writer().is_none());
    assert!(!acquired.is_poisoned());
    assert_eq!(observations.clones.load(SeqCst), 0);
    let (acquired, error) = match acquired.try_clone_charged(|_, _| Err("capacity")) {
        Err(refusal) => refusal,
        Ok(_) => panic!("refused before constructing a generation"),
    };
    assert_eq!(error, EbrCellWriterAdmissionError::Refused("capacity"));
    assert!(target.try_acquire_writer().is_none());
    assert_eq!(observations.clones.load(SeqCst), 0);
    drop(acquired);
    assert!(target.try_acquire_writer().is_some());
}

#[test]
fn acquired_clone_and_attachment_keep_the_original_allocation() {
    let (target, observations) = fixture();
    let (acquired, owned) = target
        .acquire_writer()
        .try_clone_charged(|_, _| Ok::<_, ()>(Charge(Some(Arc::clone(&observations)))))
        .unwrap_or_else(|_| panic!("healthy original writer"));
    let original = std::ptr::from_ref(&*owned);
    assert!(target.try_acquire_writer().is_none());
    let writer = acquired
        .try_write_owned(owned)
        .unwrap_or_else(|_| panic!("same healthy physical owner"));
    assert_eq!(std::ptr::from_ref(&*writer), original);
    assert_eq!(observations.clones.load(SeqCst), 1);
    drop(writer);
    assert_eq!(observations.payload_drops.load(SeqCst), 1);
    assert_eq!(observations.charge_drops.load(SeqCst), 1);
    assert!(!observations.payload_locked.load(SeqCst));
    assert!(!observations.charge_locked.load(SeqCst));
}

#[test]
fn consumed_clone_panic_releases_and_poisons_before_caller_recovery() {
    let (target, observations) = fixture();
    observations.clone_panics.store(true, SeqCst);
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _ = target
            .acquire_writer()
            .try_clone_charged(|_, _| Ok::<_, ()>(Charge(Some(Arc::clone(&observations)))));
    }))
    .is_err());
    assert_eq!(observations.clones.load(SeqCst), 1);
    assert_eq!(observations.charge_drops.load(SeqCst), 0);
    let acquired = target
        .try_acquire_writer()
        .expect("poison is acquired, not contention");
    assert!(acquired.is_poisoned());
    let result = acquired.try_clone_charged::<()>(|_, _| panic!("poison precedes admission"));
    assert!(matches!(
        result,
        Err((_, EbrCellWriterAdmissionError::Poisoned))
    ));
    assert_eq!(observations.clones.load(SeqCst), 1);
}

#[test]
fn poisoned_attachment_returns_both_original_owners() {
    let source = EbrCell::new(41_u64);
    let owner = source.write().detach();
    let original = std::ptr::from_ref(&*owner);
    let target = EbrCell::new(7_u64);
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _acquired = target.acquire_writer();
        panic!("poison raw original writer");
    }))
    .is_err());
    let acquired = target.try_acquire_writer().expect("retain poisoned guard");
    let (acquired, owner) = match acquired.try_write_owned(owner) {
        Err(owners) => owners,
        Ok(_) => panic!("poison refuses attachment"),
    };
    assert_eq!(std::ptr::from_ref(&*owner), original);
    assert_eq!(*owner, 41);
    assert_eq!(*target.read(), 7);
    assert!(target.try_acquire_writer().is_none());
    drop(acquired);
    assert!(target.try_acquire_writer().is_some());
    drop(owner);
}
