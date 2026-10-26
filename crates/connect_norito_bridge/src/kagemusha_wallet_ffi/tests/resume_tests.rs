//! Recovery inspection preserves fold work and the existing custody/publication fences.
//! These explicit owner doubles test scheduling, never proof or monetary authority.

use super::*;
use std::{
    collections::VecDeque,
    sync::{atomic::AtomicBool, mpsc},
    thread,
    time::{Duration, Instant},
};

struct RecoveryWallet {
    pending: VecDeque<Result<bool>>,
    events: Arc<Mutex<Vec<&'static str>>>,
    custody_calls: AtomicUsize,
    custody_failure_at: Option<usize>,
    close_calls: usize,
    close_failure_at: Option<usize>,
}

// These exact custody-order fixtures exercise the production selected-recovery helper;
// worker-start and retry notification behavior has separate whole-worker coverage.
fn selected_resume(id: u64) -> Result<Response> {
    resume_selected(owner(id)?)
}

impl Wallet for RecoveryWallet {
    fn deferred_receipt(&mut self) -> Result<maintenance::ReceiptStep> {
        Ok(maintenance::ReceiptStep::Empty)
    }
    fn deferred_wake_hint(&mut self) -> Result<maintenance::WakeHint> {
        Ok(maintenance::WakeHint::Age(
            state::DeferredArchiveWakeHintV1::NoAgeWake,
        ))
    }
    fn drain_plan(&mut self) -> Result<maintenance::DrainPlan> {
        Ok(maintenance::DrainPlan::Finished(
            state::DeferredArchiveDrainV1::NoWork,
        ))
    }

    fn maintenance(&mut self) -> Result<maintenance::View> {
        Ok(maintenance::View::Released)
    }
    fn require_custody_operations(&self) -> Result<()> {
        self.events.lock().unwrap().push("custody");
        let call = self.custody_calls.fetch_add(1, Ordering::SeqCst) + 1;
        if self.custody_failure_at == Some(call) {
            return Err(Failure::code(CUSTODY_LOST));
        }
        Ok(())
    }
    fn open_archive_call(&mut self) {
        self.events.lock().unwrap().push("open");
    }
    fn close_archive_call(&mut self, succeeded: bool) -> Result<()> {
        self.events
            .lock()
            .unwrap()
            .push(if succeeded { "publish" } else { "discard" });
        self.close_calls += 1;
        if self.close_failure_at == Some(self.close_calls) {
            return Err(Failure::code(UNCERTAIN));
        }
        Ok(())
    }
    fn has_pending_operation(&mut self) -> Result<bool> {
        self.events.lock().unwrap().push("inspect");
        self.pending
            .pop_front()
            .expect("explicit selected-state observation")
    }
    fn resume(&mut self) -> Result<Response> {
        self.events.lock().unwrap().push("resume");
        completion(Some(state::Completion::Complete(vec![0, 255, 0, 7])))
    }
    fn snapshot(&mut self) -> Result<state::Snapshot> {
        unreachable!()
    }
    fn setup(&mut self, _exchange_id: u64, _: setup::Setup) -> Result<Response> {
        unreachable!()
    }
    fn execute(&mut self, _: state::OperationRequestV1) -> Result<Response> {
        unreachable!()
    }
    fn request_status(&mut self, _: &[u8; 32]) -> Result<Response> {
        unreachable!()
    }
    fn retry(&mut self, _: &[u8; 32]) -> Result<Response> {
        unreachable!()
    }
    fn fold(&mut self) -> Result<ControlFlow<Response, FoldProof>> {
        unreachable!()
    }
    fn credit(&mut self, _: &[u8; 32], _: &[u8; 32]) -> Result<Response> {
        unreachable!()
    }
}

fn fixture(pending: impl IntoIterator<Item = Result<bool>>) -> RecoveryWallet {
    RecoveryWallet {
        pending: pending.into_iter().collect(),
        events: Arc::default(),
        custody_calls: AtomicUsize::new(0),
        custody_failure_at: None,
        close_calls: 0,
        close_failure_at: None,
    }
}

struct ActiveFold {
    finish: Arc<AtomicBool>,
    worker: thread::JoinHandle<std::result::Result<bool, state::Error>>,
}
impl ActiveFold {
    fn start(scheduler: state::Scheduler) -> Self {
        scheduler.set_activity(true, false);
        let finish = Arc::new(AtomicBool::new(false));
        let finishing = finish.clone();
        let (entered_tx, entered) = mpsc::channel();
        let worker = thread::spawn(move || {
            scheduler.sub_proof_for_test(0, |token| {
                entered_tx.send(()).unwrap();
                let deadline = Instant::now() + Duration::from_secs(5);
                loop {
                    token.check()?;
                    if finishing.load(Ordering::SeqCst) {
                        return Ok(());
                    }
                    assert!(Instant::now() < deadline, "fold test was not released");
                    thread::yield_now();
                }
            })
        });
        entered.recv_timeout(Duration::from_secs(5)).unwrap();
        Self { finish, worker }
    }
    fn finish(self) -> std::result::Result<bool, state::Error> {
        self.finish.store(true, Ordering::SeqCst);
        self.worker.join().unwrap()
    }
}

#[test]
fn empty_resume_preserves_an_active_fold_and_returns_the_existing_no_operation_result() {
    let scheduler = state::Scheduler::new();
    let wallet = fixture([Ok(false)]);
    let events = wallet.events.clone();
    let id = install(Box::new(wallet), scheduler.clone()).unwrap();
    let fold = ActiveFold::start(scheduler);
    let result = selected_resume(id);
    let ended = fold.finish();
    let result = result.unwrap();
    assert_eq!(result.kind, 0);
    assert!(result.bytes.is_empty());
    assert!(
        matches!(ended, Ok(true)),
        "an empty poll cancelled the fold"
    );
    assert_eq!(
        *events.lock().unwrap(),
        ["custody", "open", "inspect", "publish"]
    );
    close(id).unwrap();
}

#[test]
fn selected_resume_takes_payment_priority_and_returns_exact_completion() {
    let scheduler = state::Scheduler::new();
    let wallet = fixture([Ok(true)]);
    let events = wallet.events.clone();
    let id = install(Box::new(wallet), scheduler.clone()).unwrap();
    let fold = ActiveFold::start(scheduler);
    let result = selected_resume(id);
    let ended = fold.finish();
    let result = result.unwrap();
    assert_eq!((result.kind, result.bytes), (1, vec![0, 255, 0, 7]));
    assert!(matches!(ended, Err(state::Error::Cancelled)));
    assert_eq!(
        *events.lock().unwrap(),
        [
            "custody", "open", "inspect", "publish", "custody", "open", "resume", "publish",
        ]
    );
    close(id).unwrap();
}

#[test]
fn custody_inspection_and_publication_failures_never_become_empty_success_or_payment() {
    for boundary in ["custody", "inspect", "publish"] {
        let scheduler = state::Scheduler::new();
        let mut wallet = fixture([if boundary == "inspect" {
            Err(Failure::code(UNAVAILABLE))
        } else {
            Ok(false)
        }]);
        wallet.custody_failure_at = (boundary == "custody").then_some(1);
        wallet.close_failure_at = (boundary == "publish").then_some(1);
        let events = wallet.events.clone();
        let id = install(Box::new(wallet), scheduler.clone()).unwrap();
        let fold = ActiveFold::start(scheduler);
        let result = selected_resume(id);
        let ended = fold.finish();
        let expected = match boundary {
            "custody" => CUSTODY_LOST,
            "inspect" => UNAVAILABLE,
            _ => UNCERTAIN,
        };
        assert_eq!(result.unwrap_err(), Failure::code(expected));
        assert!(
            matches!(ended, Ok(true)),
            "{boundary} refusal cancelled a fold"
        );
        let expected_events: &[&str] = match boundary {
            "custody" => &["custody"],
            "inspect" => &["custody", "open", "inspect", "discard"],
            _ => &["custody", "open", "inspect", "publish"],
        };
        assert_eq!(events.lock().unwrap().as_slice(), expected_events);
        close(id).unwrap();
    }
}

#[test]
fn failed_probe_publication_does_not_enter_selected_recovery() {
    let scheduler = state::Scheduler::new();
    let mut wallet = fixture([Ok(true)]);
    wallet.close_failure_at = Some(1);
    let events = wallet.events.clone();
    let id = install(Box::new(wallet), scheduler.clone()).unwrap();
    let fold = ActiveFold::start(scheduler);
    let result = selected_resume(id);
    let ended = fold.finish();
    assert_eq!(result.unwrap_err(), Failure::code(UNCERTAIN));
    assert!(matches!(ended, Ok(true)));
    assert_eq!(
        *events.lock().unwrap(),
        ["custody", "open", "inspect", "publish"]
    );
    close(id).unwrap();
}

#[test]
fn selected_recovery_reenters_custody_and_keeps_final_publication_failures() {
    for custody_failure in [true, false] {
        let mut wallet = fixture([Ok(true)]);
        wallet.custody_failure_at = custody_failure.then_some(2);
        wallet.close_failure_at = (!custody_failure).then_some(2);
        let events = wallet.events.clone();
        let id = install(Box::new(wallet), state::Scheduler::new()).unwrap();
        let expected = if custody_failure {
            CUSTODY_LOST
        } else {
            UNCERTAIN
        };
        assert_eq!(selected_resume(id).unwrap_err(), Failure::code(expected));
        let expected_events: &[&str] = if custody_failure {
            &["custody", "open", "inspect", "publish", "custody"]
        } else {
            &[
                "custody", "open", "inspect", "publish", "custody", "open", "resume", "publish",
            ]
        };
        assert_eq!(events.lock().unwrap().as_slice(), expected_events);
        close(id).unwrap();
    }
}

#[test]
fn selection_after_an_empty_observation_is_recovered_only_by_a_later_poll() {
    let wallet = fixture([Ok(false), Ok(true)]);
    let events = wallet.events.clone();
    let id = install(Box::new(wallet), state::Scheduler::new()).unwrap();
    assert_eq!(selected_resume(id).unwrap().kind, 0);
    assert!(!events.lock().unwrap().contains(&"resume"));
    assert_eq!(selected_resume(id).unwrap().bytes, [0, 255, 0, 7]);
    assert_eq!(
        events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| **event == "resume")
            .count(),
        1
    );
    close(id).unwrap();
    assert_eq!(selected_resume(id).unwrap_err(), Failure::code(CLOSED));
}
