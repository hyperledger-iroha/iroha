//! Bridge ordering and closed result geometry; monetary/source semantics use Native tests.

use super::*;
use crate::kagemusha_wallet_ffi::tests::TestWallet;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;

#[test]
fn receipt_classification_uses_exact_native_verdict_before_abi_error_projection() {
    assert!(matches!(
        receipt_result(Ok(None)).unwrap(),
        ReceiptStep::Empty
    ));
    let rejected = receipt_result(Err(state::Error::ReceiptRejected(
        "completed receipt verdict",
    )))
    .unwrap();
    assert!(rejected.progressed());
    assert_eq!(
        receipt_response(rejected).unwrap_err().status,
        PROOF_REJECTED
    );
    let pending = receipt_result(Err(state::Error::Pending)).unwrap();
    assert!(!pending.progressed());
    assert_eq!(receipt_response(pending).unwrap_err().status, UNCERTAIN);
    // These can share an ABI code with a receipt verdict without sharing its provenance.
    for error in [
        state::Error::Proof("installed context"),
        state::Error::Cancelled,
        state::Error::NotEnoughMemory,
        state::Error::WitnessLost("source"),
    ] {
        assert!(receipt_result(Err(error)).is_err());
    }
}

// Isolate foreground drain ordering from automatic discovery; the worker tests exercise
// the external start/notification wrapper together with the bounded typed helper.
fn bounded_drain(invocation: &exchange::Invocation) -> Result<Response> {
    invocation.validate()?;
    drain_response(drain_step(&invocation.owner(), true)?)
}

#[test]
fn drain_envelope_keeps_each_actual_completion_and_disposition() {
    use state::{Completion as C, DeferredArchiveDrainV1 as D};
    for completion_value in [
        C::Complete(vec![0, 255, 7]),
        C::Pending,
        C::NotPerformed(advance::KagemushaWalletNotPerformedV1::StaleHead),
        C::Archived,
        C::DeliveryDataLoss,
        C::CreditStatus(vec![7, 0, 255]),
    ] {
        for disposition in [1, 2] {
            let expected = completion(Some(completion_value.clone())).unwrap();
            let value = drain_response(if disposition == 1 {
                D::Reconciled(completion_value.clone())
            } else {
                D::Executed(completion_value.clone())
            })
            .unwrap();
            assert_eq!(
                (value.kind, value.sequence, value.detail),
                (64, 0, disposition)
            );
            assert_eq!(
                u32::from_le_bytes(value.bytes[..4].try_into().unwrap()),
                expected.kind as u32
            );
            assert_eq!(
                u32::from_le_bytes(value.bytes[4..8].try_into().unwrap()),
                expected.detail
            );
            assert_eq!(
                u128::from_le_bytes(value.bytes[8..24].try_into().unwrap()),
                expected.sequence
            );
            assert_eq!(
                u32::from_le_bytes(value.bytes[24..28].try_into().unwrap()) as usize,
                expected.bytes.len()
            );
            assert_eq!(&value.bytes[28..], expected.bytes);
        }
    }
}

#[test]
fn no_work_and_all_policy_waits_have_no_completion_or_original() {
    use state::{DeferredArchiveDrainV1 as D, DeferredArchiveWaitV1 as W};
    let value = drain_response(D::NoWork).unwrap();
    assert_eq!(
        (value.kind, value.sequence, value.detail, value.bytes.len()),
        (62, 0, 0, 0)
    );
    for (wait, tag) in [
        (W::SelectedPending, 1),
        (W::ArchiveFold, 2),
        (W::FoldBacklog, 3),
        (W::ExchangeOpen, 4),
        (W::NotDue, 5),
        (W::Busy, 6),
    ] {
        let value = drain_response(D::Blocked(wait)).unwrap();
        assert_eq!(
            (value.kind, value.sequence, value.detail, value.bytes.len()),
            (63, 0, tag, 0)
        );
    }
}

#[test]
fn nested_payload_bound_does_not_relax_original_or_storage_failures() {
    use state::{Completion as C, DeferredArchiveDrainV1 as D};
    let value = drain_response(D::Executed(C::Complete(vec![7; 10_000]))).unwrap();
    assert_eq!(value.bytes.len(), 10_028);
    assert_eq!(
        drain_response(D::Executed(C::Complete(vec![7; 10_001])))
            .unwrap_err()
            .status,
        INTERNAL
    );
    assert_eq!(
        drain_response(D::Executed(C::NotPerformed(
            advance::KagemushaWalletNotPerformedV1::CapacityWait
        )))
        .unwrap_err()
        .status,
        NOT_ENOUGH_STORAGE
    );
}

type FoldScript = Box<dyn FnMut() -> Result<ControlFlow<Response, FoldProof>> + Send>;

struct Script {
    base: TestWallet,
    view: Box<dyn FnMut() -> Result<View> + Send>,
    plan: Box<dyn FnMut() -> Result<DrainPlan> + Send>,
    resumes: Arc<AtomicUsize>,
    drains: Arc<AtomicUsize>,
    scheduler: state::Scheduler,
    fold_override: Option<FoldScript>,
}
impl Wallet for Script {
    fn deferred_receipt(&mut self) -> Result<ReceiptStep> {
        Ok(ReceiptStep::Empty)
    }
    fn deferred_wake_hint(&mut self) -> Result<maintenance::WakeHint> {
        Ok(maintenance::WakeHint::Age(
            state::DeferredArchiveWakeHintV1::NoAgeWake,
        ))
    }

    fn maintenance(&mut self) -> Result<View> {
        (self.view)()
    }
    fn drain_plan(&mut self) -> Result<DrainPlan> {
        (self.plan)()
    }
    fn drain_archive(
        &mut self,
        turn: state::ArchiveTurnV1,
    ) -> Result<state::DeferredArchiveDrainV1> {
        assert!(matches!(
            self.scheduler.prepare_archive_turn(),
            Err(state::ArchiveAdmissionErrorV1::Busy)
        ));
        self.drains.fetch_add(1, Ordering::SeqCst);
        let response = Ok(state::DeferredArchiveDrainV1::Executed(
            state::Completion::Pending,
        ));
        drop(turn);
        response
    }
    fn snapshot(&mut self) -> Result<state::Snapshot> {
        self.base.snapshot()
    }
    fn setup(&mut self, exchange: u64, input: setup::Setup) -> Result<Response> {
        self.base.setup(exchange, input)
    }
    fn execute(&mut self, input: state::OperationRequestV1) -> Result<Response> {
        self.base.execute(input)
    }
    fn request_status(&mut self, request: &[u8; 32]) -> Result<Response> {
        self.base.request_status(request)
    }
    fn retry(&mut self, operation: &[u8; 32]) -> Result<Response> {
        self.base.retry(operation)
    }
    fn has_pending_operation(&mut self) -> Result<bool> {
        Ok(true)
    }
    fn resume(&mut self) -> Result<Response> {
        self.resumes.fetch_add(1, Ordering::SeqCst);
        completion(Some(state::Completion::Pending))
    }
    fn fold(&mut self) -> Result<ControlFlow<Response, FoldProof>> {
        match self.fold_override.as_mut() {
            Some(fold) => fold(),
            None => self.base.fold(),
        }
    }
    fn credit(&mut self, credit: &[u8; 32], payment: &[u8; 32]) -> Result<Response> {
        self.base.credit(credit, payment)
    }
}
fn fixture(
    view: impl FnMut() -> Result<View> + Send + 'static,
    plan: impl FnMut() -> Result<DrainPlan> + Send + 'static,
) -> (u64, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    fixture_with_fold(view, plan, None)
}
fn fixture_with_fold(
    view: impl FnMut() -> Result<View> + Send + 'static,
    plan: impl FnMut() -> Result<DrainPlan> + Send + 'static,
    fold_override: Option<FoldScript>,
) -> (u64, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let scheduler = state::Scheduler::new();
    let resumes = Arc::new(AtomicUsize::new(0));
    let drains = Arc::new(AtomicUsize::new(0));
    let id = install(
        Box::new(Script {
            base: TestWallet {
                calls: Arc::new(AtomicUsize::new(0)),
                drops: Arc::new(AtomicUsize::new(0)),
                expected_request: None,
            },
            view: Box::new(view),
            plan: Box::new(plan),
            resumes: Arc::clone(&resumes),
            drains: Arc::clone(&drains),
            scheduler: scheduler.clone(),
            fold_override,
        }),
        scheduler,
    )
    .unwrap();
    (id, resumes, drains)
}

#[test]
fn archive_fold_wait_releases_wallet_and_priority_so_close_can_finish() {
    let (seen_tx, seen) = mpsc::channel();
    let mut first = true;
    let (id, _, drains) = fixture(
        move || {
            if first {
                first = false;
                seen_tx.send(()).unwrap();
            }
            Ok(View::ArchiveFold)
        },
        || Ok(DrainPlan::Finished(state::DeferredArchiveDrainV1::NoWork)),
    );
    let invocation = test_invocation(id).unwrap();
    let waiting = std::thread::spawn(move || await_fresh(&invocation));
    seen.recv_timeout(Duration::from_secs(5)).unwrap();
    let (closed_tx, closed) = mpsc::channel();
    let closing = std::thread::spawn(move || {
        let result = close(id);
        closed_tx.send(result).unwrap();
    });
    closed
        .recv_timeout(Duration::from_secs(5))
        .expect("wait held a wallet/priority owner")
        .unwrap();
    assert_eq!(waiting.join().unwrap().unwrap_err().status, CLOSED);
    closing.join().unwrap();
    assert_eq!(drains.load(Ordering::SeqCst), 0);
}

#[test]
fn selected_pending_recovery_precedes_idle_exchange_and_activity_gates() {
    let (id, resumes, drains) = fixture(|| Ok(View::Pending), || Ok(DrainPlan::Pending));
    let held = exchange::begin(id).unwrap().delivered();
    let invocation = exchange::Invocation::setup(id, 0, 58).unwrap();
    let response = bounded_drain(&invocation).unwrap();
    assert_eq!((response.kind, response.detail), (63, 1));
    assert_eq!(resumes.load(Ordering::SeqCst), 1);
    assert_eq!(drains.load(Ordering::SeqCst), 0);
    assert!(matches!(
        exchange::end(id, held).unwrap(),
        state::ExchangeEndV1::Ended
    ));
    close(id).unwrap();
}

#[test]
fn source_plan_runs_before_turn_and_open_exchange_blocks_only_fresh_dispatch() {
    let (id, _, drains) = fixture(|| Ok(View::Released), || Ok(DrainPlan::Ready));
    let held = exchange::begin(id).unwrap().delivered();
    let invocation = exchange::Invocation::setup(id, 0, 58).unwrap();
    let response = bounded_drain(&invocation).unwrap();
    assert_eq!((response.kind, response.detail), (63, 4));
    assert_eq!(drains.load(Ordering::SeqCst), 0);
    exchange::end(id, held).unwrap();
    let response = bounded_drain(&invocation).unwrap();
    assert_eq!((response.kind, response.detail), (64, 2));
    assert_eq!(drains.load(Ordering::SeqCst), 1);
    // The consumed turn cannot keep its Payment priority after returning.
    let next = owner(id).unwrap().scheduler.prepare_archive_turn().unwrap();
    drop(next);
    close(id).unwrap();
}

#[test]
fn unavailable_plan_never_becomes_no_work_and_reconciliation_needs_no_idle_gate() {
    let (id, _, drains) = fixture(|| Ok(View::Released), || Err(Failure::code(UNAVAILABLE)));
    assert_eq!(
        bounded_drain(&exchange::Invocation::setup(id, 0, 58).unwrap())
            .unwrap_err()
            .status,
        UNAVAILABLE
    );
    assert_eq!(drains.load(Ordering::SeqCst), 0);
    close(id).unwrap();
    let (id, _, drains) = fixture(
        || Ok(View::Released),
        || {
            Ok(DrainPlan::Finished(
                state::DeferredArchiveDrainV1::Reconciled(state::Completion::Complete(vec![7])),
            ))
        },
    );
    let held = exchange::begin(id).unwrap().delivered();
    let response = bounded_drain(&exchange::Invocation::setup(id, 0, 58).unwrap()).unwrap();
    assert_eq!((response.kind, response.detail), (64, 1));
    assert_eq!(drains.load(Ordering::SeqCst), 0);
    exchange::end(id, held).unwrap();
    close(id).unwrap();
}

#[test]
fn archive_fold_worker_failure_is_returned_and_explicit_retry_requeries_source() {
    let published = Arc::new(AtomicBool::new(false));
    let source_reads = Arc::new(AtomicUsize::new(0));
    let folds = Arc::new(AtomicUsize::new(0));
    let (source, reads, folded, attempts) = (
        published.clone(),
        source_reads.clone(),
        published.clone(),
        folds.clone(),
    );
    let expected = Failure::unavailable(UNAVAILABLE, advance::KagemushaWalletUnavailableV1::Io(5));
    let (id, resumes, drains) = fixture_with_fold(
        move || {
            reads.fetch_add(1, Ordering::SeqCst);
            Ok(if source.load(Ordering::SeqCst) {
                View::Released
            } else {
                View::ArchiveFold
            })
        },
        || Ok(DrainPlan::Finished(state::DeferredArchiveDrainV1::NoWork)),
        Some(Box::new(move || {
            // A release notification may probe an already covered head. Count only the
            // actual attempted fold, as Native distinguishes caught-up from proof work.
            if folded.load(Ordering::SeqCst) {
                return Ok(ControlFlow::Break(Response {
                    kind: 7,
                    backlog: Some(0),
                    ..Response::default()
                }));
            }
            if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                return Err(expected);
            }
            folded.store(true, Ordering::SeqCst);
            Ok(ControlFlow::Break(Response {
                kind: 7,
                backlog: Some(0),
                ..Response::default()
            }))
        })),
    );
    activity(id, true, false).unwrap();
    let invocation = test_invocation(id).unwrap();
    let (returned, result) = mpsc::channel();
    let first = std::thread::spawn(move || returned.send(await_fresh(&invocation)).unwrap());
    let failure = result.recv_timeout(Duration::from_secs(5));
    if failure.is_err() {
        close(id).unwrap();
    }
    first.join().unwrap();
    assert_eq!(
        failure
            .expect("worker failure must escape its own serial wait")
            .unwrap_err(),
        expected
    );
    assert!(!published.load(Ordering::SeqCst));
    // No separate BackgroundStatus call is needed. This retry clears the now-observed
    // failure, runs the same worker and must read Released from the source again.
    let retry_invocation = test_invocation(id).unwrap();
    assert!(await_fresh(&retry_invocation).unwrap().is_none());
    assert!(published.load(Ordering::SeqCst));
    assert_eq!(folds.load(Ordering::SeqCst), 2);
    assert!(source_reads.load(Ordering::SeqCst) >= 3);
    assert_eq!(resumes.load(Ordering::SeqCst), 0);
    assert_eq!(drains.load(Ordering::SeqCst), 0);
    close(id).unwrap();
}
