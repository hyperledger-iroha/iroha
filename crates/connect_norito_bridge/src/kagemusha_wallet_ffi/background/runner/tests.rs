//! Deterministic staged-worker controls; no provider, proof, SDK or wall-clock authority.

use super::*;
use crate::kagemusha_wallet_ffi::tests::TestWallet;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc,
};

type Step<T> = Box<dyn FnMut() -> Result<T> + Send>;
struct Script {
    base: TestWallet,
    view: Step<View>,
    receipt: Step<maintenance::ReceiptStep>,
    plan: Step<maintenance::DrainPlan>,
    hint: Step<maintenance::WakeHint>,
    resume: Step<Response>,
    fold: Step<ControlFlow<Response, FoldProof>>,
    drain: Option<
        Box<dyn FnMut(state::ArchiveTurnV1) -> Result<state::DeferredArchiveDrainV1> + Send>,
    >,
    log: Arc<Mutex<Vec<&'static str>>>,
    publication: Option<Failure>,
}
impl Script {
    fn new() -> Self {
        Self {
            base: TestWallet {
                calls: Arc::default(),
                drops: Arc::default(),
                expected_request: None,
            },
            view: Box::new(|| Ok(View::Released)),
            receipt: Box::new(|| Ok(maintenance::ReceiptStep::Empty)),
            plan: Box::new(|| {
                Ok(maintenance::DrainPlan::Finished(
                    state::DeferredArchiveDrainV1::NoWork,
                ))
            }),
            hint: Box::new(|| {
                Ok(maintenance::WakeHint::Age(
                    state::DeferredArchiveWakeHintV1::NoAgeWake,
                ))
            }),
            resume: Box::new(|| completion(Some(state::Completion::Pending))),
            fold: Box::new(|| {
                Ok(ControlFlow::Break(Response {
                    kind: 7,
                    backlog: Some(0),
                    ..Response::default()
                }))
            }),
            drain: None,
            log: Arc::default(),
            publication: None,
        }
    }
}
impl Wallet for Script {
    fn maintenance(&mut self) -> Result<View> {
        self.log.lock().unwrap().push("view");
        (self.view)()
    }
    fn deferred_receipt(&mut self) -> Result<maintenance::ReceiptStep> {
        self.log.lock().unwrap().push("receipt");
        (self.receipt)()
    }
    fn observe_receipt(
        &mut self,
        send: &[u8; 32],
        original: &[u8],
    ) -> Result<maintenance::ReceiptStep> {
        assert_eq!(*send, [7; 32]);
        assert_eq!(original, [0, 255, 7]);
        self.deferred_receipt()
    }
    fn drain_archive(
        &mut self,
        turn: state::ArchiveTurnV1,
    ) -> Result<state::DeferredArchiveDrainV1> {
        self.log.lock().unwrap().push("drain");
        self.drain
            .as_mut()
            .expect("only the actual Ready fixture may dispatch")(turn)
    }
    fn drain_plan(&mut self) -> Result<maintenance::DrainPlan> {
        self.log.lock().unwrap().push("plan");
        (self.plan)()
    }
    fn deferred_wake_hint(&mut self) -> Result<maintenance::WakeHint> {
        self.log.lock().unwrap().push("hint");
        (self.hint)()
    }
    fn close_archive_call(&mut self, succeeded: bool) -> Result<()> {
        self.log
            .lock()
            .unwrap()
            .push(if succeeded { "publish" } else { "discard" });
        self.publication.map_or(Ok(()), Err)
    }
    fn resume(&mut self) -> Result<Response> {
        self.log.lock().unwrap().push("resume");
        (self.resume)()
    }
    fn fold(&mut self) -> Result<ControlFlow<Response, FoldProof>> {
        self.log.lock().unwrap().push("fold");
        (self.fold)()
    }
    fn setup(&mut self, origin: u64, input: setup::Setup) -> Result<Response> {
        self.base.setup(origin, input)
    }
    fn snapshot(&mut self) -> Result<state::Snapshot> {
        self.base.snapshot()
    }
    fn execute(&mut self, input: state::OperationRequestV1) -> Result<Response> {
        self.base.execute(input)
    }
    fn request_status(&mut self, id: &[u8; 32]) -> Result<Response> {
        self.base.request_status(id)
    }
    fn retry(&mut self, id: &[u8; 32]) -> Result<Response> {
        self.base.retry(id)
    }
    fn has_pending_operation(&mut self) -> Result<bool> {
        self.base.has_pending_operation()
    }
    fn credit(&mut self, id: &[u8; 32], payment: &[u8; 32]) -> Result<Response> {
        self.base.credit(id, payment)
    }
}
fn fixture(script: Script) -> (u64, Arc<Owner>, Arc<Mutex<Vec<&'static str>>>) {
    let log = Arc::clone(&script.log);
    let id = install(Box::new(script), state::Scheduler::new()).unwrap();
    (id, owner(id).unwrap(), log)
}
fn stages(log: &Arc<Mutex<Vec<&'static str>>>) -> Vec<&'static str> {
    log.lock()
        .unwrap()
        .iter()
        .copied()
        .filter(|value| !matches!(*value, "publish" | "discard"))
        .collect()
}
fn wait(owner: &Owner, predicate: impl Fn(&Control) -> bool) {
    let control = owner.background.inner.lock();
    let (control, timeout) = owner
        .background
        .inner
        .ready
        .wait_timeout_while(control, Duration::from_secs(5), |value| !predicate(value))
        .unwrap();
    assert!(
        !timeout.timed_out() || predicate(&control),
        "worker boundary not reached"
    );
}

#[test]
fn selected_recovery_precedes_receipt_even_inactive_with_an_open_exchange() {
    let mut script = Script::new();
    let selected = Arc::new(AtomicUsize::new(0));
    let observed = selected.clone();
    script.view = Box::new(move || {
        Ok(if observed.load(Ordering::SeqCst) == 0 {
            View::Pending
        } else {
            View::Released
        })
    });
    script.resume = Box::new(move || {
        selected.store(1, Ordering::SeqCst);
        completion(Some(state::Completion::Complete(vec![7])))
    });
    let (id, owner, log) = fixture(script);
    let exchange = exchange::begin(id).unwrap().delivered();
    let pass = bounded_pass(&owner, &owner.background.inner).unwrap();
    assert_eq!(
        stages(&log),
        ["view", "resume", "view", "receipt", "plan", "hint"]
    );
    assert!(!pass.pending && !pass.again && pass.fold.is_none());
    assert!(!owner.background.inner.lock().enabled);
    assert!(matches!(
        exchange::end(id, exchange).unwrap(),
        state::ExchangeEndV1::Ended
    ));
    close(id).unwrap();
}

#[test]
fn selected_or_racing_pending_stops_later_stages_and_uses_bounded_backoff() {
    for receipt_race in [false, true] {
        let mut script = Script::new();
        if receipt_race {
            script.receipt = Box::new(|| Ok(maintenance::ReceiptStep::Pending));
        } else {
            script.view = Box::new(|| Ok(View::Pending));
        }
        let (id, owner, log) = fixture(script);
        let pass = bounded_pass(&owner, &owner.background.inner).unwrap();
        assert!(pass.pending && !pass.again && pass.fold.is_none());
        assert_eq!(
            stages(&log),
            if receipt_race {
                vec!["view", "receipt"]
            } else {
                vec!["view", "resume", "view"]
            }
        );
        let mut control = Control::default();
        let now = Instant::now();
        for expected in [1, 2, 4, 8, 16, 32, 60, 60] {
            complete(&mut control, Ok(Pass::pending()), now);
            assert_eq!(control.deadline, Some(now + Duration::from_secs(expected)));
            assert!(!control.wake);
        }
        close(id).unwrap();
    }
}

#[test]
fn each_receipt_pass_reaches_one_fold_and_one_drain_without_draining_the_fifo() {
    let mut script = Script::new();
    script.receipt = Box::new(|| Ok(maintenance::ReceiptStep::Rejected));
    let (id, owner, log) = fixture(script);
    owner.background.inner.lock().enabled = true;
    for _ in 0..3 {
        log.lock().unwrap().clear();
        let pass = bounded_pass(&owner, &owner.background.inner).unwrap();
        assert!(pass.again);
        assert_eq!(pass.fold, Some((7, Some(0))));
        assert_eq!(stages(&log), ["view", "receipt", "fold", "plan", "hint"]);
    }
    close(id).unwrap();
}

#[test]
fn only_fold_stage_cancellation_is_continuation_and_other_failures_stay_exact() {
    for stage in ["receipt", "fold", "plan", "hint"] {
        let mut script = Script::new();
        let error = Failure {
            status: CANCELLED,
            reason: 41,
            platform_code: 77,
        };
        match stage {
            "receipt" => script.receipt = Box::new(move || Err(error)),
            "fold" => script.fold = Box::new(move || Err(error)),
            "plan" => script.plan = Box::new(move || Err(error)),
            "hint" => script.hint = Box::new(move || Err(error)),
            _ => unreachable!(),
        }
        let (id, owner, _) = fixture(script);
        owner.background.inner.lock().enabled = true;
        let result = bounded_pass(&owner, &owner.background.inner);
        if stage == "fold" {
            assert!(result.unwrap().again);
        } else {
            assert_eq!(result.err().unwrap(), error);
        }
        close(id).unwrap();
    }
}

#[test]
fn authenticated_before_bootstrap_parks_but_source_loss_does_not_become_idle() {
    let mut script = Script::new();
    script.view = Box::new(|| Ok(View::BeforeBootstrap));
    let (id, owner, log) = fixture(script);
    owner.background.start(&owner).unwrap();
    wait(&owner, |control| !control.running && !control.wake);
    assert_eq!(stages(&log), ["view"]);
    assert!(owner.background.inner.lock().deadline.is_none());
    close(id).unwrap();
    let mut script = Script::new();
    script.view = Box::new(|| Err(Failure::code(CUSTODY_LOST)));
    let (id, owner, _) = fixture(script);
    owner.background.start(&owner).unwrap();
    wait(&owner, |control| control.error.is_some());
    assert_eq!(owner.background.status().unwrap_err().status, CUSTODY_LOST);
    owner.background.notify();
    assert_eq!(owner.background.status().unwrap_err().status, CUSTODY_LOST);
    close(id).unwrap();
}

#[test]
fn age_hint_bounds_and_not_due_to_due_gap_do_not_spin_or_lose_the_threshold() {
    use state::DeferredArchiveWakeHintV1 as H;
    for (input, expected) in [
        (Duration::ZERO, AGE_RECHECK_MIN),
        (Duration::from_millis(1), AGE_RECHECK_MIN),
        (Duration::from_secs(61), Duration::from_secs(61)),
        (Duration::MAX, AGE_RECHECK_MAX),
    ] {
        assert_eq!(delay(H::RecheckAfter(input), 0), Some(expected));
    }
    assert_eq!(delay(H::NoAgeWake, 0), None);
    assert_eq!(delay(H::Due, 0), None);
    for wait in [
        state::DeferredArchiveWaitV1::NotDue,
        state::DeferredArchiveWaitV1::ExchangeOpen,
        state::DeferredArchiveWaitV1::ArchiveFold,
        state::DeferredArchiveWaitV1::Busy,
    ] {
        let mut script = Script::new();
        script.plan = Box::new(move || {
            Ok(maintenance::DrainPlan::Finished(
                state::DeferredArchiveDrainV1::Blocked(wait),
            ))
        });
        script.hint = Box::new(|| Ok(maintenance::WakeHint::Age(H::Due)));
        let (id, owner, _) = fixture(script);
        let pass = bounded_pass(&owner, &owner.background.inner).unwrap();
        assert!(!pass.again);
        let mut control = Control::default();
        let now = Instant::now();
        complete(&mut control, Ok(pass), now);
        assert_eq!(
            control.deadline,
            if wait == state::DeferredArchiveWaitV1::NotDue {
                Some(now + AGE_RECHECK_MIN)
            } else {
                None
            }
        );
        assert!(!control.wake);
        close(id).unwrap();
    }
}

#[test]
fn event_during_last_source_read_survives_completion_and_parking() {
    let mut script = Script::new();
    let (entered, entry) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let reads = Arc::new(AtomicUsize::new(0));
    let counted = reads.clone();
    script.hint = Box::new(move || {
        if counted.fetch_add(1, Ordering::SeqCst) == 0 {
            entered.send(()).unwrap();
            released.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        Ok(maintenance::WakeHint::Age(
            state::DeferredArchiveWakeHintV1::NoAgeWake,
        ))
    });
    let (id, owner, _) = fixture(script);
    owner.background.start(&owner).unwrap();
    entry.recv_timeout(Duration::from_secs(5)).unwrap();
    owner.background.notify();
    release.send(()).unwrap();
    wait(&owner, |control| {
        reads.load(Ordering::SeqCst) >= 2 && !control.running && !control.wake
    });
    assert_eq!(reads.load(Ordering::SeqCst), 2);
    assert_eq!(Arc::strong_count(&owner), 2);
    close(id).unwrap();
}

#[test]
fn timer_expiry_requeries_source_without_accumulating_elapsed_age_authority() {
    let mut script = Script::new();
    let reads = Arc::new(AtomicUsize::new(0));
    let counted = reads.clone();
    script.hint = Box::new(move || {
        counted.fetch_add(1, Ordering::SeqCst);
        Ok(maintenance::WakeHint::Age(
            state::DeferredArchiveWakeHintV1::RecheckAfter(Duration::from_secs(90)),
        ))
    });
    let (id, owner, _) = fixture(script);
    owner.background.start(&owner).unwrap();
    wait(&owner, |control| {
        control.deadline.is_some() && !control.running
    });
    {
        let mut control = owner.background.inner.lock();
        assert!(!control.wake);
        control.deadline = Some(Instant::now());
        owner.background.inner.ready.notify_all();
    }
    wait(&owner, |control| {
        reads.load(Ordering::SeqCst) >= 2 && control.deadline.is_some() && !control.running
    });
    assert_eq!(reads.load(Ordering::SeqCst), 2);
    assert!(!owner.background.inner.lock().wake);
    close(id).unwrap();
}

#[test]
fn receipt_rejection_is_progress_only_after_publication_and_final_fence() {
    let mut script = Script::new();
    script.receipt = Box::new(|| Ok(maintenance::ReceiptStep::Rejected));
    let (id, owner, log) = fixture(script);
    let receipt = maintenance::receipt_step(&owner).unwrap();
    assert!(matches!(receipt, maintenance::ReceiptStep::Rejected));
    assert_eq!(*log.lock().unwrap(), ["receipt", "publish"]);
    assert_eq!(
        maintenance::receipt_response(receipt).unwrap_err().status,
        PROOF_REJECTED
    );
    close(id).unwrap();
    let mut script = Script::new();
    script.receipt = Box::new(|| Ok(maintenance::ReceiptStep::Rejected));
    script.publication = Some(Failure::code(UNCERTAIN));
    let (id, owner, _) = fixture(script);
    assert_eq!(
        maintenance::receipt_step(&owner).err().unwrap().status,
        UNCERTAIN
    );
    close(id).unwrap();
    let mut script = Script::new();
    script.receipt = Box::new(|| Ok(maintenance::ReceiptStep::Rejected));
    let (id, owner, _) = fixture(script);
    let result = run_with_fence(
        || {
            with_wallet_owner(owner.clone(), Priority::Ingest, |wallet| {
                wallet.deferred_receipt()
            })
        },
        || Err(std::io::Error::other("receipt final fence")),
    );
    assert_eq!(result.err().unwrap().status, UNAVAILABLE);
    close(id).unwrap();
}

#[test]
fn explicit_receipt_error_notifies_after_the_owner_action_even_when_intake_failed_late() {
    for selector in [56, 57] {
        let mut script = Script::new();
        let selected = Arc::new(Mutex::new(None::<Weak<Owner>>));
        let selected_in_action = selected.clone();
        let expected =
            Failure::unavailable(UNAVAILABLE, advance::KagemushaWalletUnavailableV1::Io(5));
        script.receipt = Box::new(move || {
            // Isolate the post-action event from the earlier explicit retry event. A held
            // Payment keeps the actual worker from consuming either event during this call.
            let owner = selected_in_action
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .upgrade()
                .unwrap();
            owner.background.inner.lock().wake = false;
            Err(expected)
        });
        let (id, owner, _) = fixture(script);
        *selected.lock().unwrap() = Some(Arc::downgrade(&owner));
        let payment = owner
            .background
            .payment(&owner.scheduler, state::FoldCallClassV1::Payment);
        let invocation = if selector == 56 {
            test_invocation(id).unwrap()
        } else {
            exchange::Invocation::setup(id, 0, 57).unwrap()
        };
        let input = if selector == 56 {
            setup::Setup::ObserveCredited {
                send_request: [7; 32],
                original: vec![0, 255, 7],
            }
        } else {
            setup::Setup::ResumeDeferredReceipt
        };
        let mut delivery = review::CapabilityDelivery::default();
        assert_eq!(
            setup(&invocation, input, &mut delivery).unwrap_err(),
            expected
        );
        assert!(owner.background.inner.lock().wake);
        drop(payment);
        close(id).unwrap();
    }
}

#[test]
fn explicit_rejected_receipt_publishes_and_fences_before_returning_numeric_rejection() {
    let mut script = Script::new();
    script.receipt = Box::new(|| Ok(maintenance::ReceiptStep::Rejected));
    let (id, owner, log) = fixture(script);
    let payment = owner
        .background
        .payment(&owner.scheduler, state::FoldCallClassV1::Payment);
    let invocation = exchange::Invocation::setup(id, 0, 57).unwrap();
    let before = DURABILITY_FENCES.with(|fences| fences.get());
    let mut delivery = review::CapabilityDelivery::default();
    let result = setup(
        &invocation,
        setup::Setup::ResumeDeferredReceipt,
        &mut delivery,
    );
    assert_eq!(result.unwrap_err().status, PROOF_REJECTED);
    assert_eq!(*log.lock().unwrap(), ["receipt", "publish"]);
    assert_eq!(DURABILITY_FENCES.with(|fences| fences.get()), before + 1);
    drop(payment);
    close(id).unwrap();
}

#[test]
fn pending_arriving_at_final_hint_retries_recovery_without_latching_an_error() {
    let mut script = Script::new();
    script.hint = Box::new(|| Ok(maintenance::WakeHint::Pending));
    let (id, owner, log) = fixture(script);
    let pass = bounded_pass(&owner, &owner.background.inner).unwrap();
    assert!(pass.pending && !pass.again);
    assert_eq!(stages(&log), ["view", "receipt", "plan", "hint"]);
    let mut control = Control::default();
    let now = Instant::now();
    complete(&mut control, Ok(pass), now);
    assert_eq!(control.deadline, Some(now + Duration::from_secs(1)));
    assert!(control.error.is_none());
    close(id).unwrap();
}

#[test]
fn ready_stage_executes_one_real_turn_and_releases_it_before_followup() {
    for pending in [false, true] {
        let mut script = Script::new();
        script.plan = Box::new(|| Ok(maintenance::DrainPlan::Ready));
        let selected = Arc::new(Mutex::new(None::<state::Scheduler>));
        let checked = selected.clone();
        script.drain = Some(Box::new(move |turn| {
            assert_eq!(
                checked
                    .lock()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .prepare_archive_turn()
                    .err()
                    .unwrap(),
                state::ArchiveAdmissionErrorV1::Busy
            );
            drop(turn);
            Ok(state::DeferredArchiveDrainV1::Executed(if pending {
                state::Completion::Pending
            } else {
                state::Completion::Complete(vec![7])
            }))
        }));
        let (id, owner, log) = fixture(script);
        *selected.lock().unwrap() = Some(owner.scheduler.clone());
        let pass = bounded_pass(&owner, &owner.background.inner).unwrap();
        assert_eq!(pass.pending, pending);
        assert_eq!(pass.again, !pending);
        assert_eq!(
            stages(&log),
            if pending {
                vec!["view", "receipt", "plan", "drain"]
            } else {
                vec!["view", "receipt", "plan", "drain", "hint"]
            }
        );
        drop(owner.scheduler.prepare_archive_turn().unwrap());
        close(id).unwrap();
    }
}

#[test]
fn close_joins_recovery_proof_publication_and_archive_preparation_without_priority_deadlock() {
    for stage in [
        "recovery",
        "proof-publication",
        "before-turn",
        "inside-turn",
    ] {
        let mut script = Script::new();
        let (entered, entry) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let released = Arc::new(Mutex::new(released));
        let publications = Arc::new(AtomicUsize::new(0));
        match stage {
            "recovery" => {
                script.view = Box::new(|| Ok(View::Pending));
                script.resume = Box::new(move || {
                    entered.send(()).unwrap();
                    released
                        .lock()
                        .unwrap()
                        .recv_timeout(Duration::from_secs(5))
                        .unwrap();
                    completion(Some(state::Completion::Pending))
                });
            }
            "proof-publication" => {
                let published = publications.clone();
                script.fold = Box::new(move || {
                    let entered = entered.clone();
                    let released = released.clone();
                    let published = published.clone();
                    Ok(ControlFlow::Continue(Box::new(move || {
                        entered.send(()).unwrap();
                        released
                            .lock()
                            .unwrap()
                            .recv_timeout(Duration::from_secs(5))
                            .unwrap();
                        Ok(Box::new(move |_: &mut dyn Wallet| {
                            published.fetch_add(1, Ordering::SeqCst);
                            Ok(Response {
                                kind: 7,
                                backlog: Some(0),
                                ..Response::default()
                            })
                        }) as FoldPublication)
                    })))
                });
            }
            "before-turn" => {
                script.plan = Box::new(move || {
                    entered.send(()).unwrap();
                    released
                        .lock()
                        .unwrap()
                        .recv_timeout(Duration::from_secs(5))
                        .unwrap();
                    Ok(maintenance::DrainPlan::Ready)
                });
            }
            "inside-turn" => {
                script.plan = Box::new(|| Ok(maintenance::DrainPlan::Ready));
                script.drain = Some(Box::new(move |turn| {
                    entered.send(()).unwrap();
                    released
                        .lock()
                        .unwrap()
                        .recv_timeout(Duration::from_secs(5))
                        .unwrap();
                    drop(turn);
                    Ok(state::DeferredArchiveDrainV1::Executed(
                        state::Completion::Pending,
                    ))
                }));
            }
            _ => unreachable!(),
        }
        let (id, owner, _) = fixture(script);
        owner.background.inner.lock().enabled = stage == "proof-publication";
        owner.background.start(&owner).unwrap();
        entry.recv_timeout(Duration::from_secs(5)).unwrap();
        let (sent, done) = mpsc::channel();
        let closing = std::thread::spawn(move || sent.send(close(id)).unwrap());
        wait(&owner, |control| control.stopped);
        assert!(
            matches!(done.try_recv(), Err(mpsc::TryRecvError::Empty)),
            "close released while its worker still owned a stage"
        );
        release.send(()).unwrap();
        done.recv_timeout(Duration::from_secs(5))
            .expect("close retained a guard needed by its worker")
            .unwrap();
        closing.join().unwrap();
        assert!(owner.wallet.lock().unwrap().is_none());
        assert_eq!(
            publications.load(Ordering::SeqCst),
            0,
            "a stopped proof cannot publish after close"
        );
    }
}

#[test]
fn non_complete_archive_outputs_remain_visible_errors_instead_of_silently_parking() {
    use advance::KagemushaWalletNotPerformedV1 as N;
    use state::{Completion as C, DeferredArchiveDrainV1 as D};
    for (value, expected) in [
        (C::Archived, CUSTODY_LOST),
        (C::DeliveryDataLoss, CUSTODY_LOST),
        (C::CreditStatus(vec![7]), INTERNAL),
        (C::NotPerformed(N::CapacityWait), NOT_ENOUGH_STORAGE),
        (C::NotPerformed(N::StaleHead), INVALID),
        (
            C::NotPerformed(N::Invalid {
                field: "exact archive",
            }),
            INVALID,
        ),
    ] {
        for executed in [false, true] {
            let outcome = if executed {
                D::Executed(value.clone())
            } else {
                D::Reconciled(value.clone())
            };
            assert_eq!(archive_outcome(&outcome).unwrap_err().status, expected);
            let mut script = Script::new();
            script.plan = Box::new(move || Ok(maintenance::DrainPlan::Finished(outcome.clone())));
            let (id, owner, log) = fixture(script);
            let result = bounded_pass(&owner, &owner.background.inner);
            assert_eq!(result.as_ref().err().unwrap().status, expected);
            assert_eq!(stages(&log), ["view", "receipt", "plan"]);
            let mut control = Control::default();
            complete(&mut control, result, Instant::now());
            assert_eq!(control.error, Some((Failure::code(expected), false)));
            assert!(!control.wake);
            assert!(control.deadline.is_none());
            close(id).unwrap();
        }
    }
}

#[test]
fn every_explicit_fold_completion_notifies_without_acknowledging_a_sticky_failure() {
    let background = Background::default();
    let expected = Failure::unavailable(UNAVAILABLE, advance::KagemushaWalletUnavailableV1::Io(5));
    for (kind, backlog) in [(6, None), (7, Some(0)), (8, Some(1)), (9, Some(0))] {
        background.inner.lock().error = Some((expected, true));
        background.after_explicit_fold(&Ok(Response {
            kind,
            backlog,
            ..Response::default()
        }));
        let mut control = background.inner.lock();
        assert!(control.wake);
        assert_eq!(control.error, Some((expected, true)));
        if let Some(backlog) = backlog {
            assert_eq!(control.backlog, Some(backlog));
        }
        control.wake = false;
    }
}
