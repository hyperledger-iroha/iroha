//! Typed Rust session ownership over the sole existing confidential payment operation.
//!
//! File sources and typed Rust originals share the same admitted supervisor and worker.
use crate::{
    OneCallPrivateSettlementError, OneCallPrivateSettlementOperation,
    PrivateSettlementReconciliationPending,
};
use iroha::{
    client::{
        PrivateSettlementRequest, PrivateSettlementRunOptions, VerifiedPrivateSettlementOutcome,
        native_worker_command_scope, run_native_worker,
    },
    workflow::{WorkflowProgressV1, contract::MAX_WORKFLOW_JOURNAL_BYTES_V1},
};
use iroha_data_model::{
    nexus::PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1,
    sumeragi_finality::{MAX_EXECUTED_BLOCK_WIRE_BYTES, MAX_FINALITY_BLOCK_BYTES},
};
use std::{
    collections::BTreeMap,
    num::{NonZeroU16, NonZeroUsize},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::{
    runtime::Handle,
    sync::{Notify, OwnedSemaphorePermit, Semaphore, mpsc},
    task::JoinSet,
};

mod bounds;
mod files;
pub use files::NATIVE_PRIVATE_SETTLEMENT_DESCRIPTOR_MAX_BYTES;
mod opening;
mod supervisor;
pub use opening::{NativePrivateSettlementOriginals, NativePrivateSettlementStores};

// Selected execution plus its signed input and two Merkle paths fit within these original
// native framing ceilings. Receipt/progress are separately retained, not replacement authority.
/// Canonical retained-value allowance reserved for a terminal Run or Recover result before admission.
///
/// This includes the native executed block, signed block framing, public receipt and bounded
/// workflow progress. It is not an RSS bound or a serializer for verified receipt authority.
pub const NATIVE_PRIVATE_SETTLEMENT_TERMINAL_ALLOWANCE: usize = MAX_EXECUTED_BLOCK_WIRE_BYTES
    as usize
    + MAX_FINALITY_BLOCK_BYTES
    + PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1
    + MAX_WORKFLOW_JOURNAL_BYTES_V1;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
fn next_id() -> Result<u64, NativePrivateSettlementSessionError> {
    NEXT_ID
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .map_err(|_| NativePrivateSettlementSessionError::Capacity)
}

/// Host-local retained-value limits, separate from committed payment validity.
///
/// Bounds cover canonical original/prepared values, the fixed terminal-result allowance,
/// and one additional result-sized export buffer per permitted concurrent export. Taken
/// result objects belong to their callers; in-flight export buffers retain this host's lease.
/// They do not claim process RSS or accounting for arbitrary custom provider/error allocations.
#[derive(Clone, Copy, Debug)]
pub struct NativePrivateSettlementLimits {
    /// Maximum admitted sessions, including unopened tokens, pending commands and closing owners.
    pub maximum_sessions: NonZeroU16,
    /// Maximum canonical original request, including private terms and endpoint strings.
    /// `open_source` also counts its retained genesis, native pins and original custody path.
    pub maximum_original_bytes: NonZeroUsize,
    /// Maximum aggregate artifact allowance accepted from the original operation.
    /// `open_source` includes the concrete wallet's byte allowance in this same total.
    pub maximum_artifact_bytes: NonZeroUsize,
    /// Maximum canonical prepared value, including every repeated manifest and endpoint.
    pub maximum_prepared_bytes: NonZeroUsize,
}
impl NativePrivateSettlementLimits {
    fn validate(self) -> Result<Self, NativePrivateSettlementSessionError> {
        let result_allowance = self
            .maximum_prepared_bytes
            .get()
            .max(NATIVE_PRIVATE_SETTLEMENT_TERMINAL_ALLOWANCE);
        self.maximum_original_bytes
            .get()
            .checked_add(self.maximum_artifact_bytes.get())
            .and_then(|n| n.checked_add(result_allowance))
            // The same host separately admits at most maximum_sessions result exports.
            .and_then(|n| n.checked_add(result_allowance))
            .and_then(|n| n.checked_mul(usize::from(self.maximum_sessions.get())))
            .ok_or(NativePrivateSettlementSessionError::Capacity)?;
        Ok(self)
    }
}

/// A process-local session identity that is never reused, including across host instances.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct NativePrivateSettlementSessionId(u64);
impl NativePrivateSettlementSessionId {
    /// Numeric native handle. Decoding this number cannot construct a session or payment.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}
/// Exact process-local identity of one accepted command in its original session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativePrivateSettlementTicket {
    session: NativePrivateSettlementSessionId,
    command: u64,
}
impl NativePrivateSettlementTicket {
    /// Original owning session.
    #[must_use]
    pub const fn session(self) -> NativePrivateSettlementSessionId {
        self.session
    }
    /// Unique command number; old tickets never select a later result.
    #[must_use]
    pub const fn command(self) -> u64 {
        self.command
    }
}
/// Deliberate operation over the existing workflow, with no separate driver or retry policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativePrivateSettlementCommand {
    /// Authorize original wallet reservation/proving, without signing or submission.
    Prepare,
    /// Continue the original preparation and monetary settlement.
    Run,
    /// Reopen prepared originals without reserving or proving.
    Prepared,
    /// Independently recover existing effects and verified terminal local reconciliation.
    Recover,
}
/// Redacted lifecycle state. Ready says only that a result exists, never that payment succeeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativePrivateSettlementStatus {
    /// Admitted original has not yet been opened.
    Reserved,
    /// One accepted command is queued or running.
    Pending,
    /// The exact command's single result is ready for take.
    Ready,
    /// Previous result was taken; another command may be accepted.
    Idle,
    /// New work is refused; accepted work and original native cleanup are still owned.
    Closing,
    /// Original owner and native cleanup have completed.
    Closed,
    /// An unexpected supervisor failure prevents a completion claim.
    Unavailable,
}
/// Local session/admission failure; never a monetary outcome.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum NativePrivateSettlementSessionError {
    /// Finite original session/value allowance is unavailable.
    #[error("native private settlement capacity unavailable")]
    Capacity,
    /// Host or original session is closing.
    #[error("native private settlement owner is closing")]
    Closing,
    /// Unknown session or stale command identity.
    #[error("native private settlement handle is unavailable")]
    Handle,
    /// An accepted command or unconsumed result already owns the session.
    #[error("native private settlement command is busy")]
    Busy,
    /// Original operation did not open successfully.
    #[error("native private settlement original is not open")]
    NotOpen,
    /// This typed owner has no original concrete-wallet staging capability.
    #[error("native private settlement staging capability is unavailable")]
    StagingUnavailable,
    /// Exact result is not ready or was already taken.
    #[error("native private settlement result is unavailable")]
    NotReady,
    /// Host/runtime stopped unexpectedly; this does not certify completed native cleanup.
    #[error("native private settlement supervisor unavailable")]
    Unavailable,
}
/// Original command failure with no provider diagnostics in its Debug or Display projection.
pub enum NativePrivateSettlementCommandError {
    /// Existing typed operation error; access remains local to the Rust owner.
    Operation(OneCallPrivateSettlementError),
    /// Command panicked; its original native jobs and panic payload have been cleaned up.
    Panicked,
    /// Unexpected task/runtime completion failure; no ledger outcome is inferred.
    Unavailable,
}
impl std::fmt::Debug for NativePrivateSettlementCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Operation(_) => "Operation(<redacted>)",
            Self::Panicked => "Panicked",
            Self::Unavailable => "Unavailable",
        })
    }
}
impl std::fmt::Display for NativePrivateSettlementCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("native private settlement command failed")
    }
}
impl std::error::Error for NativePrivateSettlementCommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Operation(error) => Some(error),
            _ => None,
        }
    }
}
/// One owned typed result. Taking it transfers custody; it has no decoder or verified flag.
pub enum NativePrivateSettlementOutcome {
    /// Original operation was admitted and can accept commands.
    Opened,
    /// Explicit original wallet staging completed; no proof, signing or submission is implied.
    Staged,
    /// Original public/proved prepared form, not a successful settlement receipt.
    Prepared(PrivateSettlementRequest),
    /// Read-only prepared lookup found no completed preparation; this says nothing about effects.
    NotPrepared,
    /// Recovery has not independently established a terminal outcome; original progress is preserved.
    Pending(Option<WorkflowProgressV1>),
    /// Independently verified original terminal outcome and exact optional carrier progress.
    Verified {
        /// Original native settlement or successful Abort capability.
        outcome: VerifiedPrivateSettlementOutcome,
        /// Exact progress from Run or Recover.
        progress: Option<WorkflowProgressV1>,
    },
    /// Verified terminal outcome with original local wallet reconciliation still pending.
    ReconciliationPending(Box<PrivateSettlementReconciliationPending>),
    /// Original typed local failure. It never authorizes replacement spending.
    Failed(NativePrivateSettlementCommandError),
}
impl std::fmt::Debug for NativePrivateSettlementOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Opened => "Opened",
            Self::Staged => "Staged",
            Self::Prepared(_) => "Prepared(<retained>)",
            Self::NotPrepared => "NotPrepared",
            Self::Pending(_) => "Pending(<redacted>)",
            Self::Verified { .. } => "Verified(<authenticated terminal outcome>)",
            Self::ReconciliationPending(_) => "ReconciliationPending(<verified terminal outcome>)",
            Self::Failed(_) => "Failed(<redacted>)",
        })
    }
}
impl NativePrivateSettlementOutcome {
    fn error(error: OneCallPrivateSettlementError) -> Self {
        match error {
            OneCallPrivateSettlementError::ReconciliationPending(error) => {
                Self::ReconciliationPending(error)
            }
            error => Self::Failed(NativePrivateSettlementCommandError::Operation(error)),
        }
    }
}

struct State {
    ticket: Option<NativePrivateSettlementTicket>,
    phase: NativePrivateSettlementStatus,
    initialized: bool,
    staging_input_remaining: Option<usize>,
    closing: bool,
    result: Option<NativePrivateSettlementOutcome>,
}
struct Control {
    state: Mutex<State>,
    changed: Notify,
}
impl Control {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    fn status(state: &State) -> NativePrivateSettlementStatus {
        if state.closing
            && state.phase != NativePrivateSettlementStatus::Closed
            && state.phase != NativePrivateSettlementStatus::Unavailable
        {
            NativePrivateSettlementStatus::Closing
        } else {
            state.phase
        }
    }
}
struct Registry {
    closed: bool,
    finished: bool,
    failed: bool,
    exports: usize,
    sessions: BTreeMap<NativePrivateSettlementSessionId, Arc<Control>>,
}
struct Shared {
    registry: Mutex<Registry>,
    changed: Notify,
    wake: Notify,
    admission: Arc<Semaphore>,
    limits: NativePrivateSettlementLimits,
}
impl Shared {
    fn registry(&self) -> std::sync::MutexGuard<'_, Registry> {
        self.registry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    fn control(
        &self,
        id: NativePrivateSettlementSessionId,
    ) -> Result<Arc<Control>, NativePrivateSettlementSessionError> {
        let registry = self.registry();
        if registry.failed {
            return Err(NativePrivateSettlementSessionError::Unavailable);
        }
        registry
            .sessions
            .get(&id)
            .cloned()
            .ok_or(NativePrivateSettlementSessionError::Handle)
    }
    fn close_all(&self) {
        let mut registry = self.registry();
        registry.closed = true;
        for control in registry.sessions.values() {
            control.state().closing = true;
            control.changed.notify_waiters();
        }
        drop(registry);
        self.wake.notify_one();
    }
}

enum Work {
    Open {
        ticket: NativePrivateSettlementTicket,
        operation: OneCallPrivateSettlementOperation,
        permit: OwnedSemaphorePermit,
    },
    OpenSource {
        ticket: NativePrivateSettlementTicket,
        source: opening::OpenSource,
        permit: OwnedSemaphorePermit,
    },
    OpenFiles {
        ticket: NativePrivateSettlementTicket,
        source: files::FileInput,
        permit: OwnedSemaphorePermit,
    },
    Stage {
        ticket: NativePrivateSettlementTicket,
        source: files::FileInput,
    },
    Command {
        ticket: NativePrivateSettlementTicket,
        command: NativePrivateSettlementCommand,
        options: PrivateSettlementRunOptions,
    },
}

enum OpenInput {
    Operation(OneCallPrivateSettlementOperation),
    Source(opening::OpenSource),
    Files(files::FileInput),
}

/// Shared bounded host owner over typed Rust operations.
///
/// Acquire admission before constructing or copying large boundary inputs. Accepted commands
/// belong to the supervisor, not their waiters. Keep the supplied runtime alive until shutdown
/// completes; destroying a runtime is not a payment cancellation or a native cleanup guarantee.
/// No per-operation runtime, new prover, language codec or language wrapper is created here.
#[derive(Clone)]
pub struct NativePrivateSettlementHost {
    shared: Arc<Shared>,
    sender: mpsc::Sender<Work>,
}
impl NativePrivateSettlementHost {
    /// Start the single supervisor on the caller-owned runtime with explicit finite local bounds.
    /// # Errors
    /// Refuses overflow of the total retained-value allowance.
    pub fn new(
        runtime: Handle,
        limits: NativePrivateSettlementLimits,
    ) -> Result<Self, NativePrivateSettlementSessionError> {
        let limits = limits.validate()?;
        let (sender, receiver) = mpsc::channel(usize::from(limits.maximum_sessions.get()));
        let shared = Arc::new(Shared {
            registry: Mutex::new(Registry {
                closed: false,
                finished: false,
                failed: false,
                exports: 0,
                sessions: BTreeMap::new(),
            }),
            changed: Notify::new(),
            wake: Notify::new(),
            admission: Arc::new(Semaphore::new(usize::from(limits.maximum_sessions.get()))),
            limits,
        });
        let _supervisor = runtime.spawn(supervisor::run(shared.clone(), receiver, runtime.clone()));
        Ok(Self { shared, sender })
    }
    /// Reserve capacity before native boundary decoding/copying or original construction.
    /// # Errors
    /// Closing, exhausted capacity or exhausted process-local handle namespace.
    pub fn admit(
        &self,
    ) -> Result<NativePrivateSettlementAdmission, NativePrivateSettlementSessionError> {
        let mut registry = self.shared.registry();
        if registry.failed {
            return Err(NativePrivateSettlementSessionError::Unavailable);
        }
        if registry.closed {
            return Err(NativePrivateSettlementSessionError::Closing);
        }
        if registry.sessions.len() >= usize::from(self.shared.limits.maximum_sessions.get()) {
            return Err(NativePrivateSettlementSessionError::Capacity);
        }
        let permit = self
            .shared
            .admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| NativePrivateSettlementSessionError::Capacity)?;
        let id = NativePrivateSettlementSessionId(next_id()?);
        let control = Arc::new(Control {
            state: Mutex::new(State {
                ticket: None,
                phase: NativePrivateSettlementStatus::Reserved,
                initialized: false,
                staging_input_remaining: None,
                closing: false,
                result: None,
            }),
            changed: Notify::new(),
        });
        registry.sessions.insert(id, control);
        Ok(NativePrivateSettlementAdmission {
            host: self.clone(),
            id,
            permit: Some(permit),
        })
    }
    /// Admit one bounded export of an already taken native result under this original host.
    ///
    /// Hold the returned lease as canonical native-worker custody, outside its work closure.
    /// Shutdown refuses new exports and waits for their real output/capture/TLS cleanup.
    /// This grants no payment or result authority and creates no new workflow.
    /// # Errors
    /// Closing/failed host or all `maximum_sessions` export slots already occupied.
    pub fn admit_export(
        &self,
    ) -> Result<NativePrivateSettlementExportLease, NativePrivateSettlementSessionError> {
        let mut registry = self.shared.registry();
        if registry.failed {
            return Err(NativePrivateSettlementSessionError::Unavailable);
        }
        if registry.closed {
            return Err(NativePrivateSettlementSessionError::Closing);
        }
        if registry.exports >= usize::from(self.shared.limits.maximum_sessions.get()) {
            return Err(NativePrivateSettlementSessionError::Capacity);
        }
        registry.exports += 1;
        Ok(NativePrivateSettlementExportLease {
            shared: self.shared.clone(),
        })
    }
    /// Accept exactly one original command, without waiting for capacity or replacing a result.
    /// # Errors
    /// Original not open, busy, closing, stale handle or bounded queue unavailable.
    pub fn start(
        &self,
        id: NativePrivateSettlementSessionId,
        command: NativePrivateSettlementCommand,
        options: PrivateSettlementRunOptions,
    ) -> Result<NativePrivateSettlementTicket, NativePrivateSettlementSessionError> {
        let registry = self.shared.registry();
        if registry.failed {
            return Err(NativePrivateSettlementSessionError::Unavailable);
        }
        if registry.closed {
            return Err(NativePrivateSettlementSessionError::Closing);
        }
        let control = registry
            .sessions
            .get(&id)
            .cloned()
            .ok_or(NativePrivateSettlementSessionError::Handle)?;
        let mut state = control.state();
        if state.closing {
            return Err(NativePrivateSettlementSessionError::Closing);
        }
        if !state.initialized {
            return Err(NativePrivateSettlementSessionError::NotOpen);
        }
        if state.phase != NativePrivateSettlementStatus::Idle {
            return Err(NativePrivateSettlementSessionError::Busy);
        }
        let slot = self.sender.try_reserve().map_err(|error| match error {
            mpsc::error::TrySendError::Full(()) => NativePrivateSettlementSessionError::Capacity,
            mpsc::error::TrySendError::Closed(()) => {
                NativePrivateSettlementSessionError::Unavailable
            }
        })?;
        let ticket = NativePrivateSettlementTicket {
            session: id,
            command: next_id()?,
        };
        state.phase = NativePrivateSettlementStatus::Pending;
        state.ticket = Some(ticket);
        slot.send(Work::Command {
            ticket,
            command,
            options,
        });
        drop(state);
        drop(registry);
        control.changed.notify_waiters();
        Ok(ticket)
    }
    /// Explicitly stage one original native reservation and governed opening from private files.
    /// The closed descriptor is bounded before copy. Reopen, Prepared and Recover never call this.
    /// # Errors
    /// Closed, busy or unavailable original owner, or descriptor capacity refusal. Source and
    /// binding failures are retained as the accepted command's redacted typed result.
    pub fn stage_files(
        &self,
        id: NativePrivateSettlementSessionId,
        descriptor: &[u8],
    ) -> Result<NativePrivateSettlementTicket, NativePrivateSettlementSessionError> {
        let registry = self.shared.registry();
        if registry.failed {
            return Err(NativePrivateSettlementSessionError::Unavailable);
        }
        if registry.closed {
            return Err(NativePrivateSettlementSessionError::Closing);
        }
        let control = registry
            .sessions
            .get(&id)
            .cloned()
            .ok_or(NativePrivateSettlementSessionError::Handle)?;
        let mut state = control.state();
        if state.closing {
            return Err(NativePrivateSettlementSessionError::Closing);
        }
        if !state.initialized {
            return Err(NativePrivateSettlementSessionError::NotOpen);
        }
        if state.phase != NativePrivateSettlementStatus::Idle {
            return Err(NativePrivateSettlementSessionError::Busy);
        }
        let remaining = state
            .staging_input_remaining
            .ok_or(NativePrivateSettlementSessionError::StagingUnavailable)?;
        let source = files::FileInput::capture(descriptor, remaining)?;
        let slot = self.sender.try_reserve().map_err(|error| match error {
            mpsc::error::TrySendError::Full(()) => NativePrivateSettlementSessionError::Capacity,
            mpsc::error::TrySendError::Closed(()) => {
                NativePrivateSettlementSessionError::Unavailable
            }
        })?;
        let ticket = NativePrivateSettlementTicket {
            session: id,
            command: next_id()?,
        };
        state.phase = NativePrivateSettlementStatus::Pending;
        state.ticket = Some(ticket);
        slot.send(Work::Stage { ticket, source });
        drop(state);
        drop(registry);
        control.changed.notify_waiters();
        Ok(ticket)
    }
    /// Read a closed redacted lifecycle state for the exact accepted command.
    /// # Errors
    /// Unknown session or stale command identity.
    pub fn status(
        &self,
        ticket: NativePrivateSettlementTicket,
    ) -> Result<NativePrivateSettlementStatus, NativePrivateSettlementSessionError> {
        let control = self.shared.control(ticket.session)?;
        let state = control.state();
        if state.ticket != Some(ticket) {
            return Err(NativePrivateSettlementSessionError::Handle);
        }
        Ok(Control::status(&state))
    }
    /// Wait for result readiness or closing completion; dropping this waiter never cancels work.
    /// # Errors
    /// Unknown/stale handle or unexpected supervisor failure.
    pub async fn wait(
        &self,
        ticket: NativePrivateSettlementTicket,
    ) -> Result<NativePrivateSettlementStatus, NativePrivateSettlementSessionError> {
        let control = self.shared.control(ticket.session)?;
        loop {
            let changed = control.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let status = {
                let state = control.state();
                if state.ticket != Some(ticket) {
                    return Err(NativePrivateSettlementSessionError::Handle);
                }
                Control::status(&state)
            };
            if !matches!(
                status,
                NativePrivateSettlementStatus::Pending | NativePrivateSettlementStatus::Closing
            ) {
                return Ok(status);
            }
            changed.await;
        }
    }
    /// Take the original result once. Its byte/secret custody then belongs to the caller.
    /// # Errors
    /// Closing, stale handle or result absent/already taken.
    pub fn take(
        &self,
        ticket: NativePrivateSettlementTicket,
    ) -> Result<NativePrivateSettlementOutcome, NativePrivateSettlementSessionError> {
        let control = self.shared.control(ticket.session)?;
        let mut state = control.state();
        if state.ticket != Some(ticket) {
            return Err(NativePrivateSettlementSessionError::Handle);
        }
        if state.closing {
            return Err(NativePrivateSettlementSessionError::Closing);
        }
        let result = state
            .result
            .take()
            .ok_or(NativePrivateSettlementSessionError::NotReady)?;
        state.phase = NativePrivateSettlementStatus::Idle;
        drop(state);
        control.changed.notify_waiters();
        Ok(result)
    }
    /// Mark Closing immediately and wait for accepted work, native cleanup and result disposal.
    ///
    /// Dropping this returned future keeps the supervisor draining. Close discards an unclaimed
    /// result; a payment may already be settled, so reopen its original durable operation to recover.
    pub fn close(
        &self,
        id: NativePrivateSettlementSessionId,
    ) -> impl Future<Output = Result<(), NativePrivateSettlementSessionError>> + Send + 'static
    {
        let control = self.shared.control(id);
        if let Ok(control) = &control {
            control.state().closing = true;
            control.changed.notify_waiters();
            self.shared.wake.notify_one();
        }
        async move {
            let control = control?;
            loop {
                let changed = control.changed.notified();
                tokio::pin!(changed);
                changed.as_mut().enable();
                match control.state().phase {
                    NativePrivateSettlementStatus::Closed => return Ok(()),
                    NativePrivateSettlementStatus::Unavailable => {
                        return Err(NativePrivateSettlementSessionError::Unavailable);
                    }
                    _ => {}
                }
                changed.await;
            }
        }
    }
    /// Close admission immediately, then drain all accepted work before runtime destruction.
    ///
    /// Dropping the waiter does not stop draining. Unopened tokens are invalidated; they cannot
    /// subsequently hand off a newly constructed operation. A failed shutdown is not Closed.
    pub fn shutdown(
        &self,
    ) -> impl Future<Output = Result<(), NativePrivateSettlementSessionError>> + Send + 'static
    {
        self.shared.close_all();
        let shared = self.shared.clone();
        async move {
            loop {
                let changed = shared.changed.notified();
                tokio::pin!(changed);
                changed.as_mut().enable();
                {
                    let registry = shared.registry();
                    if registry.failed {
                        return Err(NativePrivateSettlementSessionError::Unavailable);
                    }
                    if registry.finished {
                        return Ok(());
                    }
                }
                changed.await;
            }
        }
    }
}
/// Unique original-host export registration. Keep it outside native work as worker custody.
/// Its destruction acknowledges actual cleanup; it grants no native result authority.
pub struct NativePrivateSettlementExportLease {
    shared: Arc<Shared>,
}
impl Drop for NativePrivateSettlementExportLease {
    fn drop(&mut self) {
        let mut registry = self.shared.registry();
        registry.exports -= 1;
        drop(registry);
        self.shared.wake.notify_one();
        self.shared.changed.notify_waiters();
    }
}

/// Capacity acquired before caller construction. Dropping an unopened token starts no work.
pub struct NativePrivateSettlementAdmission {
    host: NativePrivateSettlementHost,
    id: NativePrivateSettlementSessionId,
    permit: Option<OwnedSemaphorePermit>,
}
impl NativePrivateSettlementAdmission {
    /// Original process-local session identity, reserved once even if open later fails.
    #[must_use]
    pub const fn id(&self) -> NativePrivateSettlementSessionId {
        self.id
    }
    /// Hand off the existing typed operation for supervised native budget validation.
    ///
    /// Acquire this token before constructing the operation. Boundary adapters must also check
    /// their input lengths before copying/decode; this typed Rust method is not that decoder.
    /// # Errors
    /// Closing/failed host or full queue. Refused ownership remains ordinary caller disposal.
    pub fn open(
        self,
        operation: OneCallPrivateSettlementOperation,
    ) -> Result<NativePrivateSettlementTicket, NativePrivateSettlementSessionError> {
        self.enqueue_open(OpenInput::Operation(operation))
    }

    /// Accept original native construction under this token's existing supervisor and worker.
    ///
    /// Checks the borrowed genesis extent before its sole native copy. The accepted command
    /// authenticates independent genesis/schema/account bindings and counts the complete
    /// originals before opening any private directory. Opening never provisions wallet inputs,
    /// signs or submits a payment. A discarded waiter does not cancel accepted construction.
    ///
    /// Acquire this admission before copying or decoding the caller's typed originals. This
    /// method supplies no language decoder and never obtains trust pins from a remote source.
    /// # Errors
    /// Closed host, unavailable admission, or original genesis exceeding the retained allowance.
    /// Authentication and custody failures after acceptance remain the ticket's typed outcome.
    pub fn open_source(
        self,
        originals: NativePrivateSettlementOriginals,
        genesis_wire: &[u8],
    ) -> Result<NativePrivateSettlementTicket, NativePrivateSettlementSessionError> {
        // Refuse already-closed admission before counting or copying any source. Recheck
        // at enqueue for a concurrent close; no caller-owned destructor runs under this lock.
        {
            let registry = self.host.shared.registry();
            if registry.failed {
                return Err(NativePrivateSettlementSessionError::Unavailable);
            }
            if registry.closed {
                return Err(NativePrivateSettlementSessionError::Closing);
            }
            let control = registry
                .sessions
                .get(&self.id)
                .ok_or(NativePrivateSettlementSessionError::Handle)?;
            if control.state().closing {
                return Err(NativePrivateSettlementSessionError::Closing);
            }
        }
        let source =
            opening::OpenSource::capture(originals, genesis_wire, self.host.shared.limits)?;
        self.enqueue_open(OpenInput::Source(source))
    }

    /// Accept a closed, bounded Norito-JSON file descriptor on the original supervisor.
    /// Native reads, config parsing, independent genesis authentication and custody opening occur
    /// on its canonical worker. This method copies no source file and never provisions notes.
    /// # Errors
    /// Closing/unavailable original admission or excessive descriptor length before its copy.
    pub fn open_files(
        self,
        descriptor: &[u8],
    ) -> Result<NativePrivateSettlementTicket, NativePrivateSettlementSessionError> {
        self.check_open()?;
        let source = files::FileInput::capture(
            descriptor,
            self.host.shared.limits.maximum_original_bytes.get(),
        )?;
        self.enqueue_open(OpenInput::Files(source))
    }
    fn check_open(&self) -> Result<(), NativePrivateSettlementSessionError> {
        let registry = self.host.shared.registry();
        if registry.failed {
            return Err(NativePrivateSettlementSessionError::Unavailable);
        }
        if registry.closed {
            return Err(NativePrivateSettlementSessionError::Closing);
        }
        let control = registry
            .sessions
            .get(&self.id)
            .ok_or(NativePrivateSettlementSessionError::Handle)?;
        if control.state().closing {
            return Err(NativePrivateSettlementSessionError::Closing);
        }
        Ok(())
    }

    fn enqueue_open(
        mut self,
        input: OpenInput,
    ) -> Result<NativePrivateSettlementTicket, NativePrivateSettlementSessionError> {
        let registry = self.host.shared.registry();
        if registry.failed {
            return Err(NativePrivateSettlementSessionError::Unavailable);
        }
        if registry.closed {
            return Err(NativePrivateSettlementSessionError::Closing);
        }
        let control = registry
            .sessions
            .get(&self.id)
            .cloned()
            .ok_or(NativePrivateSettlementSessionError::Handle)?;
        let mut state = control.state();
        if state.closing {
            return Err(NativePrivateSettlementSessionError::Closing);
        }
        let slot = self
            .host
            .sender
            .try_reserve()
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(()) => {
                    NativePrivateSettlementSessionError::Capacity
                }
                mpsc::error::TrySendError::Closed(()) => {
                    NativePrivateSettlementSessionError::Unavailable
                }
            })?;
        let ticket = NativePrivateSettlementTicket {
            session: self.id,
            command: next_id()?,
        };
        state.ticket = Some(ticket);
        state.phase = NativePrivateSettlementStatus::Pending;
        let permit = self
            .permit
            .take()
            .ok_or(NativePrivateSettlementSessionError::Handle)?;
        slot.send(match input {
            OpenInput::Operation(operation) => Work::Open {
                ticket,
                operation,
                permit,
            },
            OpenInput::Source(source) => Work::OpenSource {
                ticket,
                source,
                permit,
            },
            OpenInput::Files(source) => Work::OpenFiles {
                ticket,
                source,
                permit,
            },
        });
        drop(state);
        drop(registry);
        Ok(ticket)
    }
}
impl Drop for NativePrivateSettlementAdmission {
    fn drop(&mut self) {
        if self.permit.is_some() {
            let removed = self.host.shared.registry().sessions.remove(&self.id);
            if let Some(control) = removed {
                control.state().phase = NativePrivateSettlementStatus::Closed;
                control.changed.notify_waiters();
            }
            self.host.shared.wake.notify_one();
        }
    }
}

#[cfg(test)]
mod tests;
