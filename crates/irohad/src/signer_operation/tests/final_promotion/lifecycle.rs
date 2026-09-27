//! Whole-call exclusion over actual signing/recovery entry points and the private receipt journal.

use super::*;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::mpsc,
    time::Duration,
};

struct SourcePause {
    phase: Option<SignerCommittedObservationPhaseV1>,
    remaining: usize,
    action: Box<dyn FnOnce() + Send>,
}

struct CountedSource {
    inner: Arc<Source>,
    calls: AtomicUsize,
    pause: Mutex<Option<SourcePause>>,
}
impl CountedSource {
    fn entering(&self, phase: Option<SignerCommittedObservationPhaseV1>) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut slot = self.pause.lock().unwrap();
        let ready = slot.as_mut().is_some_and(|pause| {
            if pause.phase.is_none() || pause.phase == phase {
                pause.remaining -= 1;
            }
            pause.remaining == 0
        });
        let pause = if ready { slot.take() } else { None };
        drop(slot);
        if let Some(pause) = pause {
            (pause.action)();
        }
    }
}
impl SignerOperationStateSourceV1 for CountedSource {
    fn observe_signing_state(
        &self,
        binding: &SignerCustodyBindingV1,
    ) -> Result<SignerOperationSigningStateV1, SignerOperationErrorV1> {
        self.entering(None);
        self.inner.observe_signing_state(binding)
    }
    fn observe(
        &self,
        binding: &SignerCustodyBindingV1,
    ) -> Result<SignerCustodyUseContextV1, SignerOperationErrorV1> {
        self.entering(None);
        self.inner.observe(binding)
    }
    fn reserve(
        &self,
        request: &SignerOperationReservationRequestV1<'_>,
    ) -> Result<SignerOperationReservationV1, SignerOperationErrorV1> {
        self.entering(None);
        self.inner.reserve(request)
    }
    fn reserve_stream_token(
        &self,
        request: &SignerOperationReservationRequestV1<'_>,
        review: &SignerStreamTokenReservationReviewV1<'_>,
    ) -> Result<SignerOperationReservationV1, SignerOperationErrorV1> {
        self.entering(None);
        self.inner.reserve_stream_token(request, review)
    }
    fn observe_reserved(
        &self,
        check: &SignerOperationReservationCheckV1<'_>,
        phase: SignerReservedObservationPhaseV1,
    ) -> Result<SignerCustodyUseContextV1, SignerOperationErrorV1> {
        self.entering(None);
        self.inner.observe_reserved(check, phase)
    }
    fn commit(
        &self,
        request: &SignerOperationCommitRequestV1<'_>,
    ) -> Result<SignerCustodyUseContextV1, SignerOperationErrorV1> {
        self.entering(None);
        self.inner.commit(request)
    }
    fn observe_committed(
        &self,
        request: &SignerOperationCommitRequestV1<'_>,
        phase: SignerCommittedObservationPhaseV1,
    ) -> Result<SignerCustodyUseContextV1, SignerOperationErrorV1> {
        self.entering(Some(phase));
        self.inner.observe_committed(request, phase)
    }
}

fn counted_ceremony(
    path: &Path,
) -> (
    SignerFinalPromotionServiceV1,
    Arc<CountedSource>,
    Arc<Provider>,
) {
    let mut fixture = promotion_fixture();
    let source = Arc::new(CountedSource {
        inner: fixture.source.clone(),
        calls: AtomicUsize::new(0),
        pause: Mutex::new(None),
    });
    fixture.coordinator.source = source.clone();
    fixture.source.state.lock().unwrap().expected_journal = Some(path.to_owned());
    let statement = signed_fixture::statement_message(&fixture.source.binding);
    let service = SignerFinalPromotionServiceV1::new(
        fixture.coordinator,
        reviewed(&statement),
        Arc::from(statement),
        SignerReceiptJournalV1::open_test(path, SignerReceiptPurposeV1::FinalPromotionProvenance)
            .unwrap(),
    )
    .unwrap();
    (service, source, fixture.provider)
}

fn journal_entries(path: &Path) -> Vec<(std::ffi::OsString, Vec<u8>)> {
    let mut entries: Vec<_> = fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name(), fs::read(entry.path()).unwrap())
        })
        .collect();
    entries.sort();
    entries
}

#[test]
fn concurrent_sign_and_recover_fail_before_io_and_success_releases_the_gate() {
    for (running_recovery, final_release) in
        [(false, false), (false, true), (true, false), (true, true)]
    {
        let directory = private_directory();
        let path = directory.path().canonicalize().unwrap();
        let (service, source, provider) = counted_ceremony(&path);
        let original = running_recovery.then(|| service.sign().unwrap());
        let (entered_tx, entered_rx) = mpsc::sync_channel(0);
        let (resume_tx, resume_rx) = mpsc::sync_channel(0);
        *source.pause.lock().unwrap() = Some(SourcePause {
            phase: final_release.then_some(SignerCommittedObservationPhaseV1::BeforeRelease),
            // The producer's final receipt recheck follows the coordinator's release check.
            remaining: if final_release { 2 } else { 1 },
            action: Box::new(move || {
                entered_tx.send(()).unwrap();
                // A broken blocking gate fails this test instead of leaving the suite hung.
                resume_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            }),
        });
        std::thread::scope(|scope| {
            let worker = scope.spawn(|| {
                if running_recovery {
                    service.recover()
                } else {
                    service.sign()
                }
            });
            entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            let calls = source.calls.load(Ordering::SeqCst);
            let signatures = provider.calls.load(Ordering::SeqCst);
            let journal = journal_entries(&path);
            for error in [service.sign().unwrap_err(), service.recover().unwrap_err()] {
                assert_eq!(error, SignerFinalPromotionErrorV1::Busy);
                assert_eq!(
                    error.to_string(),
                    "final promotion operation already in progress"
                );
            }
            assert_eq!(source.calls.load(Ordering::SeqCst), calls);
            assert_eq!(provider.calls.load(Ordering::SeqCst), signatures);
            assert_eq!(journal_entries(&path), journal);
            resume_tx.send(()).unwrap();
            let receipt = worker.join().unwrap().unwrap();
            if let Some(original) = original {
                assert_eq!(receipt, original);
            }
            // This is a fresh complete recovery, so success did not retain lifecycle ownership.
            assert_eq!(service.recover().unwrap(), receipt);
            assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
            assert_eq!(source.inner.state.lock().unwrap().commits, 1);
        });
    }
}

#[test]
fn early_errors_release_the_gate_without_consuming_an_operation() {
    let directory = private_directory();
    let path = directory.path().canonicalize().unwrap();
    let (service, source, provider) = counted_ceremony(&path);
    source.inner.state.lock().unwrap().fail_observe = true;
    assert_eq!(
        service.sign().unwrap_err(),
        SignerFinalPromotionErrorV1::Operation(SignerOperationErrorV1::StateUnavailable)
    );
    source.inner.state.lock().unwrap().fail_observe = false;
    assert_eq!(
        service.recover().unwrap_err(),
        SignerFinalPromotionErrorV1::Journal
    );
    assert!(source.inner.state.lock().unwrap().used_ids.is_empty());
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert!(journal_entries(&path).is_empty());
    let receipt = service.sign().unwrap();
    assert_eq!(service.recover().unwrap(), receipt);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
}

#[test]
fn late_errors_release_the_gate_but_preserve_the_uncompleted_tombstone() {
    let directory = private_directory();
    let path = directory.path().canonicalize().unwrap();
    let (service, source, provider) = counted_ceremony(&path);
    source.inner.state.lock().unwrap().fail_commit = true;
    let conflict =
        SignerFinalPromotionErrorV1::Operation(SignerOperationErrorV1::ReservationConflict);
    assert_eq!(service.sign().unwrap_err(), conflict);
    let retained = journal_entries(&path);
    assert_eq!(retained.len(), 1);
    let calls = source.calls.load(Ordering::SeqCst);
    assert_eq!(service.recover().unwrap_err(), conflict);
    assert!(source.calls.load(Ordering::SeqCst) > calls);
    assert_eq!(
        service.sign().unwrap_err(),
        SignerFinalPromotionErrorV1::Journal
    );
    assert_eq!(journal_entries(&path), retained);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    assert_eq!(source.inner.state.lock().unwrap().commits, 0);
}

#[test]
fn a_panicked_lifecycle_permanently_rejects_sign_and_recover_before_io() {
    let directory = private_directory();
    let path = directory.path().canonicalize().unwrap();
    let (service, source, provider) = counted_ceremony(&path);
    *source.pause.lock().unwrap() = Some(SourcePause {
        phase: None,
        remaining: 1,
        action: Box::new(|| {
            panic!("injected lifecycle panic before the native observation");
        }),
    });
    assert!(catch_unwind(AssertUnwindSafe(|| service.sign())).is_err());
    let calls = source.calls.load(Ordering::SeqCst);
    for error in [service.sign().unwrap_err(), service.recover().unwrap_err()] {
        assert_eq!(error, SignerFinalPromotionErrorV1::Poisoned);
        assert_eq!(error.to_string(), "final promotion lifecycle unavailable");
    }
    assert_eq!(source.calls.load(Ordering::SeqCst), calls);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert!(source.inner.state.lock().unwrap().used_ids.is_empty());
    assert!(journal_entries(&path).is_empty());
}

#[test]
fn recovery_only_view_outlives_every_protected_provider_and_retains_the_exact_journal_lease() {
    let directory = private_directory();
    let path = directory.path().canonicalize().unwrap();
    let (service, source, provider) = counted_ceremony(&path);
    let original = service.sign().unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    let provider_lifetime = Arc::downgrade(&provider);
    let recovery = service.recovery_service();
    let before = journal_entries(&path);
    drop(service);
    drop(provider);
    assert!(provider_lifetime.upgrade().is_none());
    assert!(
        SignerReceiptJournalV1::open_test(&path, SignerReceiptPurposeV1::FinalPromotionProvenance)
            .is_err()
    );
    let reads = source.calls.load(Ordering::SeqCst);
    let reserved = source.inner.state.lock().unwrap().reserved_reads;
    assert_eq!(recovery.recover().unwrap(), original);
    assert!(source.calls.load(Ordering::SeqCst) > reads);
    assert_eq!(source.inner.state.lock().unwrap().reserved_reads, reserved);
    assert_eq!(source.inner.state.lock().unwrap().commits, 1);
    assert_eq!(journal_entries(&path), before);
    assert!(provider_lifetime.upgrade().is_none());
    drop(recovery);
    let _new_lease =
        SignerReceiptJournalV1::open_test(&path, SignerReceiptPurposeV1::FinalPromotionProvenance)
            .unwrap();
}

#[test]
fn recovery_only_view_shares_signing_gate_and_releases_it_after_success() {
    let directory = private_directory();
    let path = directory.path().canonicalize().unwrap();
    let (service, source, provider) = counted_ceremony(&path);
    let recovery = service.recovery_service();
    let (entered_tx, entered_rx) = mpsc::sync_channel(0);
    let (resume_tx, resume_rx) = mpsc::sync_channel(0);
    *source.pause.lock().unwrap() = Some(SourcePause {
        phase: Some(SignerCommittedObservationPhaseV1::BeforeRelease),
        remaining: 2,
        action: Box::new(move || {
            entered_tx.send(()).unwrap();
            resume_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        }),
    });
    std::thread::scope(|scope| {
        let worker = scope.spawn(|| service.sign());
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let calls = source.calls.load(Ordering::SeqCst);
        let signatures = provider.calls.load(Ordering::SeqCst);
        let before = journal_entries(&path);
        assert_eq!(
            recovery.recover().unwrap_err(),
            SignerFinalPromotionErrorV1::Busy
        );
        assert_eq!(source.calls.load(Ordering::SeqCst), calls);
        assert_eq!(provider.calls.load(Ordering::SeqCst), signatures);
        assert_eq!(journal_entries(&path), before);
        resume_tx.send(()).unwrap();
        let receipt = worker.join().unwrap().unwrap();
        assert_eq!(recovery.recover().unwrap(), receipt);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
        assert_eq!(source.inner.state.lock().unwrap().commits, 1);
    });
}

#[test]
fn recovery_only_view_preserves_shared_poison_without_any_observation_or_journal_io() {
    let directory = private_directory();
    let path = directory.path().canonicalize().unwrap();
    let (service, source, provider) = counted_ceremony(&path);
    let recovery = service.recovery_service();
    *source.pause.lock().unwrap() = Some(SourcePause {
        phase: None,
        remaining: 1,
        action: Box::new(|| panic!("injected shared receipt lifecycle failure")),
    });
    assert!(catch_unwind(AssertUnwindSafe(|| service.sign())).is_err());
    let calls = source.calls.load(Ordering::SeqCst);
    assert_eq!(
        recovery.recover().unwrap_err(),
        SignerFinalPromotionErrorV1::Poisoned
    );
    assert_eq!(source.calls.load(Ordering::SeqCst), calls);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert!(source.inner.state.lock().unwrap().used_ids.is_empty());
    assert!(journal_entries(&path).is_empty());
}
