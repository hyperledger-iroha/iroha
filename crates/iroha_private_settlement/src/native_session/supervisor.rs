//! Sole asynchronous owner of accepted session commands and actual native cleanup.
use super::*;
use std::future::Future;
use tokio::task::Id;

struct Owner {
    control: Arc<Control>,
    operation: Option<Arc<OneCallPrivateSettlementOperation>>,
    staging: Option<Arc<opening::Staging>>,
    admission: Option<OwnedSemaphorePermit>,
    native: Arc<Semaphore>,
    busy: bool,
    disposing: bool,
}

enum Completion {
    Opened(opening::Opened),
    Outcome(NativePrivateSettlementOutcome),
    Closed(Option<OwnedSemaphorePermit>),
}

struct SupervisorGuard(Arc<Shared>);
impl Drop for SupervisorGuard {
    fn drop(&mut self) {
        let mut registry = self.0.registry();
        if !registry.finished {
            registry.closed = true;
            registry.failed = true;
            for control in registry.sessions.values() {
                control.state().phase = NativePrivateSettlementStatus::Unavailable;
                control.changed.notify_waiters();
            }
        }
        drop(registry);
        self.0.changed.notify_waiters();
    }
}

// The outer supervisor-owned task never exposes its inner command handle. It joins the
// command, then its independent native frontier. JoinError is not a cleanup certificate.
async fn tracked<F, Fut>(runtime: Handle, native: Arc<Semaphore>, make: F) -> Completion
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Completion> + Send + 'static,
{
    let (command, drain) = native_worker_command_scope();
    let task = runtime.spawn(command.run(make));
    let outcome = task.await;
    drain.wait().await;
    match outcome {
        Ok(outcome) => outcome,
        Err(error) => {
            let panicked = error.is_panic();
            // JoinError may own arbitrary panic payload disposal. Use the same canonical
            // native worker and await it, rather than running its destructor on the executor.
            let permit = match native.acquire_owned().await {
                Ok(permit) => permit,
                Err(_) => {
                    return Completion::Outcome(NativePrivateSettlementOutcome::Failed(
                        NativePrivateSettlementCommandError::Unavailable,
                    ));
                }
            };
            let cleaned = run_native_worker(permit, (), move || drop(error)).await;
            Completion::Outcome(NativePrivateSettlementOutcome::Failed(
                if cleaned.is_err() || !panicked {
                    NativePrivateSettlementCommandError::Unavailable
                } else {
                    NativePrivateSettlementCommandError::Panicked
                },
            ))
        }
    }
}

async fn execute(
    operation: Arc<OneCallPrivateSettlementOperation>,
    native: Arc<Semaphore>,
    limits: NativePrivateSettlementLimits,
    command: NativePrivateSettlementCommand,
    options: PrivateSettlementRunOptions,
) -> Completion {
    let outcome = match command {
        NativePrivateSettlementCommand::Prepare => match operation.prepare(&options).await {
            Ok(prepared) => prepared_result(operation, native, limits, prepared).await,
            Err(error) => NativePrivateSettlementOutcome::error(error),
        },
        NativePrivateSettlementCommand::Prepared => {
            match operation.prepared_request(&options).await {
                Ok(Some(prepared)) => prepared_result(operation, native, limits, prepared).await,
                Ok(None) => NativePrivateSettlementOutcome::NotPrepared,
                Err(error) => NativePrivateSettlementOutcome::error(error),
            }
        }
        NativePrivateSettlementCommand::Run => match operation.run(&options).await {
            Ok(result) => NativePrivateSettlementOutcome::Verified {
                outcome: result.outcome,
                progress: Some(result.progress),
            },
            Err(error) => NativePrivateSettlementOutcome::error(error),
        },
        NativePrivateSettlementCommand::Recover => match operation.recover(&options).await {
            Ok(result) => match result.outcome {
                Some(outcome) => NativePrivateSettlementOutcome::Verified {
                    outcome,
                    progress: result.progress,
                },
                None => NativePrivateSettlementOutcome::Pending(result.progress),
            },
            Err(error) => NativePrivateSettlementOutcome::error(error),
        },
    };
    Completion::Outcome(outcome)
}

async fn prepared_result(
    operation: Arc<OneCallPrivateSettlementOperation>,
    native: Arc<Semaphore>,
    limits: NativePrivateSettlementLimits,
    prepared: PrivateSettlementRequest,
) -> NativePrivateSettlementOutcome {
    let permit = match native.try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            return NativePrivateSettlementOutcome::error(OneCallPrivateSettlementError::Capacity);
        }
    };
    let checked = run_native_worker(permit, (), move || {
        bounds::prepared(&operation, &prepared, limits)?;
        Ok::<_, OneCallPrivateSettlementError>(prepared)
    })
    .await;
    match checked {
        Ok((Ok(prepared), ())) => NativePrivateSettlementOutcome::Prepared(prepared),
        Ok((Err(error), ())) => NativePrivateSettlementOutcome::error(error),
        Err(error) => NativePrivateSettlementOutcome::error(error.into()),
    }
}

pub(super) fn run(
    shared: Arc<Shared>,
    receiver: mpsc::Receiver<Work>,
    runtime: Handle,
) -> impl Future<Output = ()> + Send {
    // Eager custody also covers a stopped runtime discarding this future unpolled.
    let guard = SupervisorGuard(shared.clone());
    async move { run_owned(shared, receiver, runtime, guard).await }
}

async fn run_owned(
    shared: Arc<Shared>,
    mut receiver: mpsc::Receiver<Work>,
    runtime: Handle,
    _guard: SupervisorGuard,
) {
    let mut owners = BTreeMap::<NativePrivateSettlementSessionId, Owner>::new();
    let mut tasks = JoinSet::new();
    let mut running = std::collections::HashMap::<Id, NativePrivateSettlementSessionId>::new();
    let mut sender_closed = false;
    loop {
        // Close unopened reservations without waiting for a caller to drop its unused token.
        // A pending queued Open has Pending state and must still be accepted and drained.
        {
            let mut registry = shared.registry();
            registry.sessions.retain(|_, control| {
                let mut state = control.state();
                if state.closing && state.phase == NativePrivateSettlementStatus::Reserved {
                    state.phase = NativePrivateSettlementStatus::Closed;
                    control.changed.notify_waiters();
                    false
                } else {
                    true
                }
            });
        }
        for (id, owner) in &mut owners {
            if !owner.busy && !owner.disposing && {
                let state = owner.control.state();
                state.closing && state.phase != NativePrivateSettlementStatus::Pending
            } {
                owner.disposing = true;
                owner.busy = true;
                let output = owner.control.state().result.take();
                let operation = owner.operation.take();
                let staging = owner.staging.take();
                let admission = owner.admission.take();
                let native = owner.native.clone();
                let cleanup_native = native.clone();
                let future = tracked(runtime.clone(), native, move || async move {
                    let Ok(permit) = cleanup_native.acquire_owned().await else {
                        // This semaphore is private and never closed by any caller.
                        return Completion::Outcome(NativePrivateSettlementOutcome::Failed(
                            NativePrivateSettlementCommandError::Unavailable,
                        ));
                    };
                    // Keep session admission outside work until actual TLS and discarded value
                    // cleanup ends. Cancellation of a close waiter cannot drop this future.
                    let completed = run_native_worker(permit, admission, move || {
                        drop((output, operation, staging))
                    })
                    .await;
                    match completed {
                        Ok(((), admission)) => Completion::Closed(admission),
                        Err(_) => Completion::Closed(None), // canonical worker already disposed original custody
                    }
                });
                let task = tasks.spawn_on(future, &runtime);
                running.insert(task.id(), *id);
            }
        }
        let finished = {
            let mut registry = shared.registry();
            if registry.closed
                && registry.exports == 0
                && registry.sessions.is_empty()
                && owners.is_empty()
                && tasks.is_empty()
                && receiver.is_empty()
            {
                registry.finished = true;
                true
            } else {
                false
            }
        };
        if finished {
            shared.changed.notify_waiters();
            return;
        }
        tokio::select! {
            work = receiver.recv(), if !sender_closed => {
                let Some(work) = work else { sender_closed = true; shared.close_all(); continue; };
                let (id, future): (_, std::pin::Pin<Box<dyn Future<Output = Completion> + Send>>) = match work {
                    work @ (Work::Open { .. } | Work::OpenSource { .. } | Work::OpenFiles { .. }) => {
                        let (ticket, input, permit) = match work {
                            Work::Open { ticket, operation, permit } => (ticket, OpenInput::Operation(operation), permit),
                            Work::OpenSource { ticket, source, permit } => (ticket, OpenInput::Source(source), permit),
                            Work::OpenFiles { ticket, source, permit } => (ticket, OpenInput::Files(source), permit),
                            Work::Command { .. } | Work::Stage { .. } => unreachable!(),
                        };
                        let Some(control) = shared.registry().sessions.get(&ticket.session).cloned() else {
                            // Only supervisor failure could invalidate a previously accepted Open.
                            shared.registry().failed = true;
                            return;
                        };
                        let native = Arc::new(Semaphore::new(1));
                        owners.insert(ticket.session, Owner { control, operation: None, staging: None, admission: Some(permit), native: native.clone(), busy: true, disposing: false });
                        let limits = shared.limits;
                        let worker = native.clone();
                        let future = tracked(runtime.clone(), native, move || async move {
                            let Ok(permit) = worker.acquire_owned().await else {
                                return Completion::Outcome(NativePrivateSettlementOutcome::error(OneCallPrivateSettlementError::Capacity));
                            };
                            match run_native_worker(permit, (), move || {
                                let opened = match input {
                                    OpenInput::Operation(operation) => opening::Opened { operation, staging: None },
                                    OpenInput::Source(source) => source.construct_with_staging(limits)?,
                                    OpenInput::Files(source) => source.open(limits)?,
                                };
                                bounds::original(&opened.operation, limits)?;
                                Ok::<_, OneCallPrivateSettlementError>(opened)
                            }).await {
                                Ok((Ok(operation), ())) => Completion::Opened(operation),
                                Ok((Err(error), ())) => Completion::Outcome(NativePrivateSettlementOutcome::error(error)),
                                Err(error) => Completion::Outcome(NativePrivateSettlementOutcome::error(error.into())),
                            }
                        });
                        (ticket.session, Box::pin(future))
                    }
                    Work::Stage { ticket, source } => {
                        let Some(owner) = owners.get_mut(&ticket.session) else { shared.registry().failed = true; return; };
                        let staging = owner.staging.clone();
                        owner.busy = true;
                        let native = owner.native.clone();
                        let worker = native.clone();
                        let maximum = shared.limits.maximum_original_bytes.get();
                        let future = tracked(runtime.clone(), native, move || async move {
                            let Ok(permit) = worker.acquire_owned().await else {
                                return Completion::Outcome(NativePrivateSettlementOutcome::error(OneCallPrivateSettlementError::Capacity));
                            };
                            let result = run_native_worker(permit, (), move || {
                                let staging = staging.ok_or(OneCallPrivateSettlementError::Binding)?;
                                source.stage(&staging, maximum)
                            }).await;
                            Completion::Outcome(match result {
                                Ok((Ok(()), ())) => NativePrivateSettlementOutcome::Staged,
                                Ok((Err(error), ())) => NativePrivateSettlementOutcome::error(error),
                                Err(error) => NativePrivateSettlementOutcome::error(error.into()),
                            })
                        });
                        (ticket.session, Box::pin(future))
                    }
                    Work::Command { ticket, command, options } => {
                        let Some(owner) = owners.get_mut(&ticket.session) else { shared.registry().failed = true; return; };
                        let Some(operation) = owner.operation.clone() else { shared.registry().failed = true; return; };
                        owner.busy = true;
                        let native = owner.native.clone();
                        let worker = native.clone();
                        let limits = shared.limits;
                        (ticket.session, Box::pin(tracked(runtime.clone(), native, move || execute(operation, worker, limits, command, options))))
                    }
                };
                let task = tasks.spawn_on(future, &runtime);
                running.insert(task.id(), id);
            },
            joined = tasks.join_next_with_id(), if !tasks.is_empty() => {
                let Some(joined) = joined else { continue; };
                let (task_id, completion) = match joined {
                    Ok((id, completion)) => (id, completion),
                    Err(error) => {
                        // Unexpected failure of the join/drain owner cannot certify closure.
                        // Guard marks every live handle unavailable; no new admission is allowed.
                        shared.registry().failed = true;
                        drop(error);
                        return;
                    }
                };
                let Some(id) = running.remove(&task_id) else { shared.registry().failed = true; return; };
                if let Completion::Closed(admission) = completion {
                    let owner = owners.remove(&id);
                    let control = shared.registry().sessions.remove(&id);
                    // Nothing caller-controlled remains in this cleaned owner. Refund admission
                    // before publishing Closed so a woken waiter can immediately admit again.
                    drop(owner);
                    drop(admission);
                    if let Some(control) = control {
                        control.state().phase = NativePrivateSettlementStatus::Closed;
                        control.changed.notify_waiters();
                    }
                    shared.changed.notify_waiters();
                    continue;
                }
                let Some(owner) = owners.get_mut(&id) else { shared.registry().failed = true; return; };
                owner.busy = false;
                let outcome = match completion {
                    Completion::Opened(opened) => {
                        owner.operation = Some(Arc::new(opened.operation));
                        owner.staging = opened.staging;
                        let mut state = owner.control.state();
                        state.initialized = true;
                        state.staging_input_remaining = owner.staging.as_ref().map(|staging| shared.limits.maximum_original_bytes.get().saturating_sub(staging.original_bytes));
                        NativePrivateSettlementOutcome::Opened
                    }
                    Completion::Outcome(outcome) => outcome,
                    Completion::Closed(_) => unreachable!(),
                };
                let mut state = owner.control.state();
                let previous = state.result.replace(outcome);
                state.phase = NativePrivateSettlementStatus::Ready;
                drop(state);
                drop(previous);
                owner.control.changed.notify_waiters();
            },
            () = shared.wake.notified() => {},
        }
    }
}
