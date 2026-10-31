//! Request-owned native interval production and exact last-byte response custody.
//!
//! One actual query reservation funds the original Core owner, cooperative stop
//! control and encoded response. No response DTO is copied into another graph.
//! Core retains a prepaid bounded live-graph working set while walking history;
//! the complete original reservation survives physical work and the last response byte.

use super::*;
use iroha_allocation::ChargedShared;
use iroha_core::{
    state::{FinalityProofIntervalReadError, StateViewError, StateViewWaiter},
    sumeragi::finality::{
        NativeFinalityProofInterval, NativeFinalityProofIntervalError,
        NativeFinalityProofIntervalLimits, ProofDestinationError,
    },
};
use iroha_data_model::sumeragi_finality::{MAX_FINALITY_BLOCK_BYTES, SumeragiFinalityProof};
use std::{num::NonZeroUsize, sync::atomic::Ordering};

/// First pause before a refused ladder step is tried again. Each consecutive refusal
/// doubles it up to `LADDER_STEP_RETRY_CAP`; any retained step resets it.
const LADDER_STEP_RETRY: Duration = Duration::from_secs(5);
/// Longest pause between attempts at a ladder step that keeps being refused.
const LADDER_STEP_RETRY_CAP: Duration = Duration::from_secs(300);

/// Original outer HTTP deadline, captured before route admission or physical work.
#[derive(Clone, Copy, Debug)]
pub struct RouteExecutionDeadline(pub(crate) Instant);

// The physical worker while it runs; a stop unparks it from a busy State wait.
type WorkerThread = parking_lot::Mutex<Option<std::thread::Thread>>;

struct CancelOnDrop {
    // Retire the charged control before the last original query credit, including
    // an unpolled HTTP future whose native worker has already returned an error.
    flag: ChargedShared<AtomicBool>,
    // A worker parked on a busy State release wakes when its request stops,
    // instead of at that release or the original deadline.
    worker: ChargedShared<WorkerThread>,
    #[cfg(not(all(test, sumeragi_torii_mutation = "TOR3")))]
    _owner: history_producer::HistoryProducerOwner,
}
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.flag.store(true, Ordering::Release);
        if let Some(worker) = self.worker.lock().take() {
            worker.unpark();
        }
    }
}
impl CancelOnDrop {
    fn new(owner: &history_producer::HistoryProducerOwner) -> Result<Self, Error> {
        let mut reservation = owner
            .response_metadata()
            .try_reserve_layouts([
                ChargedShared::<AtomicBool>::allocation_layout(),
                ChargedShared::<WorkerThread>::allocation_layout(),
            ])
            .map_err(|_| native_projection_response::capacity())?;
        let flag = ChargedShared::from_reservation(AtomicBool::new(false), &mut reservation)
            .map_err(|_| native_projection_response::capacity())?;
        ChargedShared::from_reservation(WorkerThread::new(None), &mut reservation)
            .map(|worker| Self {
                flag,
                worker,
                #[cfg(not(all(test, sumeragi_torii_mutation = "TOR3")))]
                _owner: owner.clone(),
            })
            .map_err(|_| native_projection_response::capacity())
    }
}

// Names the worker to its stop control only while `work` runs, so a late stop
// never unparks the next task of the same pooled thread.
struct RunningWorker(ChargedShared<WorkerThread>);
impl RunningWorker {
    fn enter(worker: ChargedShared<WorkerThread>) -> Self {
        *worker.lock() = Some(std::thread::current());
        Self(worker)
    }
}
impl Drop for RunningWorker {
    fn drop(&mut self) {
        self.0.lock().take();
    }
}

/// Current single-proof and interval responses share one actual native producer.
#[derive(Clone, Copy, Debug)]
pub enum Selection {
    /// Preserve the existing standalone canonical proof response shape.
    Single(u64),
    /// Retain at most 64 consecutive canonical proofs without another response graph.
    Interval { from: u64, to: u64 },
}
impl Selection {
    /// Exact requested first and final heights for the shared producer.
    pub(crate) fn range(self) -> (u64, u64) {
        match self {
            Self::Single(height) => (height, height),
            Self::Interval { from, to } => (from, to),
        }
    }
    /// Preserve the existing independent admission key for standalone reads.
    pub(crate) fn rate_hint(self) -> &'static str {
        match self {
            Self::Single(_) => "/v1/bridge/finality/{height}",
            Self::Interval { .. } => "/v1/bridge/finality/interval/{from}/{to}",
        }
    }
    /// Charge the actual response against its route's existing egress key.
    pub(crate) fn egress_hint(self) -> &'static str {
        match self {
            Self::Single(_) => "v1/bridge/finality",
            Self::Interval { .. } => "v1/bridge/finality/interval",
        }
    }
}

/// Exact body extent accompanies the already prepaid HTTP source; no collection follows.
pub struct PreparedResponse {
    /// Already admitted canonical body retaining its original query owner.
    pub(crate) response: AxResponse,
    /// Complete encoded HTTP content extent, including canonical outer framing.
    pub(crate) bytes: usize,
}
impl PreparedResponse {
    fn deadline(format: ResponseFormat) -> Self {
        Self {
            response: route_timeout_error_response(format),
            bytes: 0,
        }
    }
}

fn invalid_interval() -> Error {
    Error::AppQueryValidation {
        code: "invalid_finality_interval",
        message: "Finality intervals require 1 to 64 consecutive positive heights.".to_owned(),
    }
}
// Prepay the worker's busy-State waiter from the request's own control corridor.
fn state_view_waiter(
    owner: &history_producer::HistoryProducerOwner,
) -> Result<StateViewWaiter, Error> {
    let mut reservation = owner
        .response_metadata()
        .try_reserve(StateViewWaiter::allocation_layout())
        .map_err(|_| native_projection_response::capacity())?;
    StateViewWaiter::for_current_thread(&mut reservation)
        .map_err(|_| native_projection_response::capacity())
}
fn source_error(error: FinalityProofIntervalReadError) -> Error {
    use iroha_core::sumeragi::finality::NativeFinalityProofRefusal;

    match error {
        // A waiting read returns Busy only once the request's whole deadline passed.
        FinalityProofIntervalReadError::StateView(StateViewError::Busy(_)) => {
            Error::AppServiceUnavailable {
                code: "finality_history_busy",
                message: "The original State publication has not completed.".to_owned(),
            }
        }
        FinalityProofIntervalReadError::Proof(
            NativeFinalityProofIntervalError::Backing(_)
            | NativeFinalityProofIntervalError::Destination(
                ProofDestinationError::Admission(_)
                | ProofDestinationError::Buffer(_)
                | ProofDestinationError::Key(_),
            ),
        ) => native_projection_response::capacity(),
        FinalityProofIntervalReadError::Proof(NativeFinalityProofIntervalError::Deferred(
            original,
        )) => canonical_history::query_attempt_error(
            iroha_core::execution_attempt::ExecutionAttemptError::Deferred(original),
        ),
        FinalityProofIntervalReadError::Proof(NativeFinalityProofIntervalError::Native(
            NativeFinalityProofRefusal::NotCommitted { .. }
            | NativeFinalityProofRefusal::NotInView { .. }
            | NativeFinalityProofRefusal::MissingCertificate { .. },
        )) => Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::NotFound,
        )),
        error => Error::Query(iroha_data_model::ValidationFail::InternalError(
            error.to_string(),
        )),
    }
}

// Canonical Vec framing borrows the original immutable proofs. Both binary and
// JSON emission use the existing codec's sequence/leaf kernels, never reencoding
// a proof through a copied Vec or constructing an unchecked response authority.
#[derive(Clone, Copy)]
struct BorrowedInterval<'a>(&'a NativeFinalityProofInterval);
impl BorrowedInterval<'_> {
    fn proofs(&self) -> impl ExactSizeIterator<Item = &SumeragiFinalityProof> {
        (0..self.0.len()).map(move |index| self.0.proof(index).expect("immutable interval member"))
    }
}
impl norito::NoritoSchema for BorrowedInterval<'_> {
    fn nominal_name() -> String {
        "iroha_torii::finality_interval::BorrowedInterval<'_>".to_owned()
    }
    fn frame_name() -> String {
        <Vec<SumeragiFinalityProof> as norito::NoritoSchema>::frame_name()
    }
}
impl norito::core::SerializePayload for BorrowedInterval<'_> {
    fn serialize(&self, out: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        norito::core::write_element_sequence::<SumeragiFinalityProof, _>(out, self.proofs())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        norito::core::sequence_encoded_len_hint(self.proofs())
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        norito::core::sequence_encoded_len_exact(self.proofs())
    }
}
impl norito::json::JsonSerialize for BorrowedInterval<'_> {
    fn json_serialize(&self, out: &mut String) {
        out.push('[');
        for (index, proof) in self.proofs().enumerate() {
            if index != 0 {
                out.push(',');
            }
            norito::json::JsonSerialize::json_serialize(proof, out);
        }
        out.push(']');
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        out.begin_container()?;
        let result = (|| {
            out.push('[')?;
            for (index, proof) in self.proofs().enumerate() {
                if index != 0 {
                    out.push(',')?;
                }
                norito::json::JsonSerialize::json_serialize_to(proof, out)?;
            }
            out.push(']')
        })();
        out.end_container();
        result
    }
}

/// Produce and encode under the original physical worker, response and deadline owners.
pub async fn prepare(
    state: Arc<CoreState>,
    selection: Selection,
    format: ResponseFormat,
    admission: QueryAdmissionPermit,
    response_limit: usize,
    deadline: Instant,
) -> Result<PreparedResponse, Error> {
    let (from, to) = selection.range();
    let from = NonZeroU64::new(from).ok_or_else(invalid_interval)?;
    let to = NonZeroU64::new(to).ok_or_else(invalid_interval)?;
    let owner = history_producer::HistoryProducerOwner::from_admission(&admission)?;
    // Keep both configured HTTP content and the actual response corridor; neither
    // the interval count nor a native frame-length receipt grants more bytes.
    let response_limit = response_limit.min(owner.response_bytes());
    let limits = NativeFinalityProofIntervalLimits::new(
        from,
        to,
        NonZeroUsize::new(MAX_FINALITY_BLOCK_BYTES).expect("positive canonical block ceiling"),
        NonZeroUsize::new(response_limit).ok_or_else(native_projection_response::capacity)?,
        deadline,
    )
    .map_err(|_| invalid_interval())?;
    if Instant::now() >= deadline {
        return Ok(PreparedResponse::deadline(format));
    }
    run_with_stop(admission, owner, move |owner, physical_cancel| {
        if Instant::now() >= deadline {
            return Ok(PreparedResponse::deadline(format));
        }
        if physical_cancel.load(Ordering::Acquire) {
            return Err(native_projection_response::capacity());
        }
        // Admit the worker's waiter before the first State capture: a busy publication
        // or reader is waited out within this same deadline and stop control.
        let mut waiter = state_view_waiter(&owner)?;
        let interval = match state.read_finality_proof_interval_waiting(
            &owner.finality_budget()?,
            &limits,
            &physical_cancel,
            &mut waiter,
        ) {
            Ok(interval) => interval,
            Err(FinalityProofIntervalReadError::Proof(
                NativeFinalityProofIntervalError::Deadline
                | NativeFinalityProofIntervalError::Destination(ProofDestinationError::Deadline),
            )) => return Ok(PreparedResponse::deadline(format)),
            Err(error) => return Err(source_error(error)),
        };
        drop(waiter);
        let encoded = match selection {
            Selection::Single(_) => native_projection_response::encode(
                interval.proof(0).expect("successful singleton interval"),
                format,
                response_limit,
                owner.cold_frames(),
            ),
            Selection::Interval { .. } => native_projection_response::encode(
                &BorrowedInterval(&interval),
                format,
                response_limit,
                owner.cold_frames(),
            ),
        }?;
        // The full outer header and sequence framing were counted before
        // allocation. Core's individual-frame sum is never the HTTP extent.
        let bytes = encoded.as_ref().len();
        drop(interval);
        if Instant::now() >= deadline {
            return Ok(PreparedResponse::deadline(format));
        }
        if physical_cancel.load(Ordering::Acquire) {
            return Err(native_projection_response::capacity());
        }
        let encoded = response_memory_custody::owned(encoded, &owner)
            .map_err(|_| native_projection_response::capacity())?;
        let mut response = response_memory_custody::json(encoded, &owner)
            .map_err(|_| native_projection_response::capacity())?;
        if matches!(format, ResponseFormat::Norito) {
            response.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static("application/x-norito"),
            );
        }
        Ok(PreparedResponse { response, bytes })
    })
    .await
}

/// Keep this node's verified finality prefixes within one ladder step of its tip.
///
/// Retained prefixes are process-local and never persisted: each carries the native
/// authority that its own walk from signed genesis verified. A restarted node starts
/// cold, so its first read near the tip would walk the whole chain, which can outlast
/// the route deadline. From startup on, this worker reads the proof one ladder step
/// above the nearest retained prefix, under its own heavy-query admission and route
/// deadline, until the tip is within one step; then it follows the tip. Every step is
/// an ordinary authenticated read, and client reads resume from each completed one.
pub(crate) async fn keep_prefix_ladder_warm(
    app: SharedAppState,
    shutdown: ShutdownSignal,
) -> ToriiCriticalWorkerExit {
    keep_prefix_ladder_warm_in_steps(app, shutdown, CoreState::FINALITY_LADDER_STEP).await
}

async fn keep_prefix_ladder_warm_in_steps(
    app: SharedAppState,
    shutdown: ShutdownSignal,
    step: u64,
) -> ToriiCriticalWorkerExit {
    let started = Instant::now();
    let mut reported = false;
    let mut pause = LADDER_STEP_RETRY;
    loop {
        let tip = u64::try_from(app.state.committed_height()).unwrap_or(u64::MAX);
        let floor = app.state.finality_ladder_floor(tip);
        if tip.saturating_sub(floor) <= step {
            if !reported {
                reported = true;
                iroha_logger::info!(
                    tip,
                    floor,
                    elapsed_ms = started.elapsed().as_millis(),
                    "finality prefix ladder is within one step of the committed tip"
                );
            }
            tokio::select! {
                () = shutdown.receive() => return ToriiCriticalWorkerExit::StoppedByShutdown,
                () = app.state.wait_for_committed_height(floor + step + 1) => continue,
            }
        }
        let result = tokio::select! {
            () = shutdown.receive() => return ToriiCriticalWorkerExit::StoppedByShutdown,
            result = ladder_step(&app, floor + step) => result,
        };
        if result.is_ok() && app.state.finality_ladder_floor(tip) > floor {
            pause = LADDER_STEP_RETRY;
            continue;
        }
        let refusal = result.err().map(|error| error.to_string());
        iroha_logger::warn!(
            height = floor + step,
            retry_ms = pause.as_millis(),
            refusal = refusal.as_deref().unwrap_or("its prefix was not retained"),
            "finality prefix ladder step kept no prefix"
        );
        tokio::select! {
            () = shutdown.receive() => return ToriiCriticalWorkerExit::StoppedByShutdown,
            () = tokio::time::sleep(pause) => {}
        }
        pause = (pause * 2).min(LADDER_STEP_RETRY_CAP);
    }
}

// One ordinary authenticated single-proof read; its lasting effect is the retained prefix.
async fn ladder_step(app: &SharedAppState, height: u64) -> Result<(), Error> {
    let selection = Selection::Single(height);
    let height = NonZeroU64::new(height).ok_or_else(invalid_interval)?;
    let deadline = Instant::now() + route_timeout_for_path(selection.rate_hint());
    let mut admission = acquire_query_admission(app.as_ref(), true).await?;
    if admission.fanout_memory.is_none() {
        admission.fanout_memory = Some(
            try_acquire_bodyless_finality_memory(app)
                .map_err(|_| native_projection_response::capacity())?,
        );
    }
    let owner = history_producer::HistoryProducerOwner::from_admission(&admission)?;
    let response_limit = app
        .torii_proxy_max_response_bytes
        .min(owner.response_bytes());
    let limits = NativeFinalityProofIntervalLimits::new(
        height,
        height,
        NonZeroUsize::new(MAX_FINALITY_BLOCK_BYTES).expect("positive canonical block ceiling"),
        NonZeroUsize::new(response_limit).ok_or_else(native_projection_response::capacity)?,
        deadline,
    )
    .map_err(|_| invalid_interval())?;
    let state = Arc::clone(&app.state);
    run_with_stop(admission, owner, move |owner, cancelled| {
        let mut waiter = state_view_waiter(&owner)?;
        state
            .read_finality_proof_interval_waiting(
                &owner.finality_budget()?,
                &limits,
                &cancelled,
                &mut waiter,
            )
            .map(drop)
            .map_err(source_error)
    })
    .await
}

/// Run finite native proof production under one physical admission and cancellation owner.
pub async fn run_with_stop<T, F>(
    admission: QueryAdmissionPermit,
    owner: history_producer::HistoryProducerOwner,
    work: F,
) -> Result<T, Error>
where
    T: Send + 'static,
    F: FnOnce(
            history_producer::HistoryProducerOwner,
            ChargedShared<AtomicBool>,
        ) -> Result<T, Error>
        + Send
        + 'static,
{
    let cancelled = CancelOnDrop::new(&owner)?;
    let physical_cancel = cancelled.flag.clone();
    let running = cancelled.worker.clone();
    let worker = crate::panic_recovery::spawn_blocking_recoverable(move || {
        // The blocking task retains every original permit through actual completion,
        // even if the HTTP future disappears. Core owns its prepaid graph phases;
        // entering the generic cumulative history scope here would debit freed
        // ancestors forever. Physical admission and inherited scopes remain intact.
        let _admission = admission;
        let _running = RunningWorker::enter(running);
        owner.physical_scope(|| work(owner.clone(), physical_cancel))
    });
    let result = crate::panic_recovery::join_recoverable(worker)
        .await
        .map_err(|_| {
            Error::Query(iroha_data_model::ValidationFail::InternalError(
                "finite finality interval worker failed".to_owned(),
            ))
        })?;
    // On cancellation this guard is dropped with the HTTP future. The physical
    // worker keeps the original flag and every admission permit until it retires.
    drop(cancelled);
    result
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use tests::maximum_paid_chain_for_testing;
