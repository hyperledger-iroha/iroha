//! Supervised daemon lifecycle for the committed SoraFS reputation projector.
//!
//! This module owns scheduling and payload-free health projection only. It
//! never fabricates ledger pages, signatures, DAG acknowledgements, or native
//! journal transactions: those operations remain behind deployment-injected,
//! identity-pinned runtime adapters.

use std::{
    collections::BTreeMap,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, bail};
use iroha_config::parameters::actual::SorafsReputationRuntime;
use iroha_core::{
    queue::{Error as QueueError, Queue},
    state::{State, StateReadOnly, WorldReadOnly},
    tx::AcceptedTransaction,
};
use iroha_crypto::KeyPair;
use iroha_data_model::{
    ChainId,
    account::AccountId,
    isi::InstructionBox,
    query::sorafs::prelude::FindSorafsReputationJournalAuthorityPolicy,
    sorafs::{
        capacity::ProviderId,
        reputation::{
            PorTerminalOutcomeV1, ReputationJournalFinalizedCursorV1,
            StreamTokenValidationOutcomeV1,
        },
    },
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use sorafs_manifest::{
    ReputationSnapshotTrustPolicyV1, ReputationWeightsV1, reputation::REPUTATION_WEIGHTS_VERSION_V1,
};
use sorafs_node::reputation::{
    ReputationIngestMetricsSnapshot, ReputationIngestPolicyV1, ReputationIngestService,
    ReputationIngestStatusV1,
    runtime::{
        ReputationCommittedProjectorRuntimeV1, ReputationCommittedReadApiV1,
        ReputationCommittedReadProjectionV1, ReputationExternalFailureV1,
        ReputationFinalizedAnchorV1, ReputationFinalizedQueryPolicyV1, ReputationFinalizedQueryV1,
        ReputationGovernanceDagClientV1, ReputationJournalAppendInstructionV1,
        ReputationJournalDeliveryMetricsV1, ReputationJournalDeliveryPolicyV1,
        ReputationJournalDeliveryWorkerV1, ReputationJournalProducerOutboxV1,
        ReputationJournalProducerPolicyV1, ReputationJournalTransactionRequestV1,
        ReputationJournalTransactionSubmitOutcomeV1, ReputationJournalTransactionSubmitterV1,
        ReputationPublicationPolicyV1, ReputationPublicationReconcilerV1, ReputationRuntimeError,
        ReputationRuntimeStatusV1, ReputationRuntimeSupervisorV1,
        ReputationThresholdSignerClientV1,
    },
};

const SHUTDOWN_WAIT: Duration = Duration::from_secs(2);
const MIN_RECONCILIATION_TIMEOUT: Duration = Duration::from_secs(30);
const RECONCILIATION_TIMEOUT_POLL_MULTIPLIER: u32 = 3;
const RECONCILIATION_FRESHNESS_GRACE_POLLS: u32 = 2;

type ReputationReconciliationResult = std::result::Result<
    sorafs_node::reputation::runtime::ReputationRuntimeTickOutcomeV1,
    ReputationRuntimeError,
>;
type ReputationJoinedReconciliationResult =
    std::result::Result<ReputationReconciliationResult, tokio::task::JoinError>;

const fn journal_cursor(
    anchor: &ReputationFinalizedAnchorV1,
) -> ReputationJournalFinalizedCursorV1 {
    ReputationJournalFinalizedCursorV1 {
        height: anchor.identity.height,
        block_hash: anchor.identity.block_hash,
        finalized_at_unix_ms: anchor.finalized_at_unix_ms,
    }
}

fn reputation_external_failure(marker: u8) -> ReputationExternalFailureV1 {
    ReputationExternalFailureV1::try_new([marker; 32])
        .expect("fixed non-zero reputation failure marker")
}

/// Runtime-only signer map and normal-queue journal transaction submitter.
pub(crate) struct QueuedReputationJournalTransactionSubmitterV1 {
    handle: String,
    chain_id: Arc<ChainId>,
    queue: Arc<Queue>,
    state: Arc<State>,
    signers: BTreeMap<AccountId, KeyPair>,
}

impl fmt::Debug for QueuedReputationJournalTransactionSubmitterV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QueuedReputationJournalTransactionSubmitterV1")
            .field("handle", &self.handle)
            .field("signer_count", &self.signers.len())
            .finish_non_exhaustive()
    }
}

impl QueuedReputationJournalTransactionSubmitterV1 {
    /// Bind runtime-only identities to the normal transaction queue.
    ///
    /// # Errors
    ///
    /// Rejects an unsafe handle, empty signer set, duplicate authority, or a
    /// signer whose public key does not derive the supplied account.
    pub(crate) fn try_new(
        handle: String,
        chain_id: Arc<ChainId>,
        queue: Arc<Queue>,
        state: Arc<State>,
        signers: impl IntoIterator<Item = (AccountId, KeyPair)>,
    ) -> Result<Self> {
        if !is_production_handle(&handle) {
            bail!("reputation transaction-submitter handle is not production-safe");
        }
        let mut signer_map = BTreeMap::new();
        for (authority, signer) in signers {
            if AccountId::new(signer.public_key().clone()) != authority
                || signer_map.insert(authority, signer).is_some()
            {
                bail!("reputation transaction signer identity is invalid or duplicated");
            }
        }
        if signer_map.is_empty() {
            bail!("reputation transaction submitter requires a runtime-only signer");
        }
        Ok(Self {
            handle,
            chain_id,
            queue,
            state,
            signers: signer_map,
        })
    }
}

impl ReputationJournalTransactionSubmitterV1 for QueuedReputationJournalTransactionSubmitterV1 {
    fn handle(&self) -> &str {
        &self.handle
    }

    fn check_readiness(&self) -> Result<(), ReputationExternalFailureV1> {
        let view = self.state.query_view();
        if self.signers.is_empty()
            || view.latest_block().is_none()
            || &view.chain_id != self.chain_id.as_ref()
        {
            Err(reputation_external_failure(0x41))
        } else {
            Ok(())
        }
    }

    fn supports_authority(&self, authority: &AccountId) -> bool {
        self.signers.contains_key(authority)
    }

    fn submit(
        &self,
        request: &ReputationJournalTransactionRequestV1,
    ) -> ReputationJournalTransactionSubmitOutcomeV1 {
        let not_queued = |marker| ReputationJournalTransactionSubmitOutcomeV1::NotQueued {
            receipt: reputation_submission_receipt(request.idempotency_key, marker),
        };
        if request.validate().is_err() {
            return not_queued(0x51);
        }
        if request.chain_id != *self.chain_id {
            return not_queued(0x52);
        }
        let Some(signer) = self.signers.get(&request.authority) else {
            return not_queued(0x53);
        };
        let instruction = match &request.instruction {
            ReputationJournalAppendInstructionV1::Por(instruction) => {
                if instruction.entry.event_id != request.event_id
                    || instruction.entry.source_id != request.source_id
                    || instruction.entry.recorded_by != request.authority
                {
                    return not_queued(0x54);
                }
                InstructionBox::from(instruction.clone())
            }
            ReputationJournalAppendInstructionV1::StreamToken(instruction) => {
                if instruction.entry.event_id != request.event_id
                    || instruction.entry.source_id != request.source_id
                    || instruction.entry.recorded_by != request.authority
                {
                    return not_queued(0x55);
                }
                InstructionBox::from(instruction.clone())
            }
        };
        let transaction = match TransactionBuilder::new(
            request.chain_id.clone(),
            request.authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction])
        .try_sign(signer.private_key())
        {
            Ok(transaction) => transaction,
            Err(_) => return not_queued(0x56),
        };
        let view = self.state.view();
        let parameters = view.world().parameters();
        let accepted = AcceptedTransaction::accept(
            transaction,
            self.chain_id.as_ref(),
            parameters.sumeragi().max_clock_drift(),
            parameters.transaction(),
            self.state.crypto().as_ref(),
        );
        drop(view);
        let accepted = match accepted {
            Ok(accepted) => accepted,
            Err(_) => return not_queued(0x57),
        };
        match self
            .queue
            .push_with_lane_with_state(accepted, self.state.as_ref())
        {
            Ok(_) => ReputationJournalTransactionSubmitOutcomeV1::Queued {
                receipt: reputation_submission_receipt(request.idempotency_key, 0x58),
            },
            Err(failure)
                if matches!(
                    failure.err,
                    QueueError::InBlockchain
                        | QueueError::IsInQueue
                        | QueueError::PlanJournalDurabilityIndeterminate { .. }
                ) =>
            {
                ReputationJournalTransactionSubmitOutcomeV1::Ambiguous {
                    receipt: reputation_submission_receipt(request.idempotency_key, 0x59),
                }
            }
            Err(_) => not_queued(0x5A),
        }
    }
}

fn reputation_submission_receipt(mut idempotency_key: [u8; 32], marker: u8) -> [u8; 32] {
    idempotency_key[0] ^= marker;
    if idempotency_key == [0; 32] {
        idempotency_key[31] = marker.max(1);
    }
    idempotency_key
}

/// Runtime-only dependencies for the committed reputation worker.
#[derive(Clone)]
pub(crate) struct ReputationRuntimeDependenciesV1 {
    pub(crate) finalized_query: Arc<dyn ReputationFinalizedQueryV1>,
    pub(crate) journal_transaction_submitter: Arc<dyn ReputationJournalTransactionSubmitterV1>,
    pub(crate) threshold_signer: Arc<dyn ReputationThresholdSignerClientV1>,
    pub(crate) governance_dag: Arc<dyn ReputationGovernanceDagClientV1>,
}

impl ReputationRuntimeDependenciesV1 {
    /// Require every runtime-only production adapter before daemon assembly.
    pub(crate) fn require(
        finalized_query: Option<Arc<dyn ReputationFinalizedQueryV1>>,
        journal_transaction_submitter: Option<Arc<dyn ReputationJournalTransactionSubmitterV1>>,
        threshold_signer: Option<Arc<dyn ReputationThresholdSignerClientV1>>,
        governance_dag: Option<Arc<dyn ReputationGovernanceDagClientV1>>,
    ) -> Result<Self> {
        let finalized_query = finalized_query.ok_or_else(|| {
            eyre::eyre!("missing immutable historical exact-anchor finalized-query adapter")
        })?;
        let journal_transaction_submitter = journal_transaction_submitter.ok_or_else(|| {
            eyre::eyre!("missing runtime-only reputation journal transaction submitter")
        })?;
        let threshold_signer = threshold_signer
            .ok_or_else(|| eyre::eyre!("missing external threshold-signer adapter"))?;
        let governance_dag = governance_dag.ok_or_else(|| {
            eyre::eyre!("missing authenticated Governance DAG publication/readback adapter")
        })?;
        Ok(Self {
            finalized_query,
            journal_transaction_submitter,
            threshold_signer,
            governance_dag,
        })
    }
}

/// Payload-free daemon health projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReputationDaemonStatusV1 {
    /// Status of committed ingest and external publication.
    pub runtime: ReputationRuntimeStatusV1,
    /// Detailed committed projector status.
    pub ingest: ReputationIngestStatusV1,
    /// Whether all four runtime-bound external adapters passed the latest
    /// completed supervised tick. This is false before the first tick.
    pub external_dependencies_healthy: bool,
    /// Whether a reconciliation tick completed within the monotonic freshness
    /// bound. This is false before the first completed tick.
    pub last_tick_fresh: bool,
    /// Whether one bounded reconciliation attempt is currently running.
    pub tick_in_flight: bool,
    /// Whether the supervised native journal worker is fully caught up with
    /// no pending or dead-lettered rows.
    pub journal_transaction_submitter_ready: bool,
    /// Overall daemon readiness, including the native journal producer path.
    pub ready: bool,
}

/// Payload-free supervised-worker counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReputationDaemonMetricsV1 {
    /// Reconciliation ticks that completed without error.
    pub successful_ticks: u64,
    /// Reconciliation ticks that returned a typed error.
    pub failed_ticks: u64,
    /// Spawn-blocking tasks that panicked or were cancelled.
    pub panicked_ticks: u64,
    /// Reconciliation ticks that exceeded their monotonic deadline.
    pub timed_out_ticks: u64,
    /// Deterministic projector counters.
    pub ingest: ReputationIngestMetricsSnapshot,
    /// Native journal delivery counters, or `None` when the required worker
    /// was not attached.
    pub journal_delivery: Option<ReputationJournalDeliveryMetricsV1>,
}

#[derive(Debug, Default)]
struct ReputationDaemonCounters {
    successful_ticks: AtomicU64,
    failed_ticks: AtomicU64,
    panicked_ticks: AtomicU64,
    timed_out_ticks: AtomicU64,
}

#[derive(Debug)]
struct ReputationDaemonLiveness {
    last_completed_at: Mutex<Option<Instant>>,
    external_dependencies_healthy: AtomicBool,
    tick_in_flight: AtomicBool,
    shutting_down: AtomicBool,
    tick_timeout: Duration,
    freshness_timeout: Duration,
}

#[derive(Debug)]
struct ReputationDaemonShutdownGuard {
    liveness: Arc<ReputationDaemonLiveness>,
}

impl Drop for ReputationDaemonShutdownGuard {
    fn drop(&mut self) {
        self.liveness.mark_shutdown();
    }
}

impl ReputationDaemonLiveness {
    fn new(poll_interval: Duration) -> Self {
        let tick_timeout = poll_interval
            .saturating_mul(RECONCILIATION_TIMEOUT_POLL_MULTIPLIER)
            .max(MIN_RECONCILIATION_TIMEOUT);
        let freshness_timeout = tick_timeout
            .saturating_add(poll_interval.saturating_mul(RECONCILIATION_FRESHNESS_GRACE_POLLS));
        Self {
            last_completed_at: Mutex::new(None),
            external_dependencies_healthy: AtomicBool::new(false),
            tick_in_flight: AtomicBool::new(false),
            shutting_down: AtomicBool::new(false),
            tick_timeout,
            freshness_timeout,
        }
    }

    fn begin_tick(&self) {
        self.tick_in_flight.store(true, Ordering::Release);
    }

    fn finish_tick(&self, healthy: bool) -> Result<(), ReputationRuntimeError> {
        let last_completed_at = self.last_completed_at.lock();
        let Ok(mut last_completed_at) = last_completed_at else {
            self.tick_in_flight.store(false, Ordering::Release);
            self.external_dependencies_healthy
                .store(false, Ordering::Release);
            return Err(ReputationRuntimeError::RuntimePoisoned);
        };
        *last_completed_at = Some(Instant::now());
        self.tick_in_flight.store(false, Ordering::Release);
        self.external_dependencies_healthy
            .store(healthy, Ordering::Release);
        Ok(())
    }

    fn mark_timeout(&self) {
        self.external_dependencies_healthy
            .store(false, Ordering::Release);
    }

    fn mark_late_tick_finished(&self) {
        self.tick_in_flight.store(false, Ordering::Release);
    }

    fn mark_shutdown(&self) {
        self.shutting_down.store(true, Ordering::Release);
        self.tick_in_flight.store(false, Ordering::Release);
        self.external_dependencies_healthy
            .store(false, Ordering::Release);
    }

    fn status(&self) -> Result<(bool, bool, bool), ReputationRuntimeError> {
        let last_completed_at = self
            .last_completed_at
            .lock()
            .map_err(|_| ReputationRuntimeError::RuntimePoisoned)?;
        let external_dependencies_healthy =
            self.external_dependencies_healthy.load(Ordering::Acquire);
        let tick_in_flight = self.tick_in_flight.load(Ordering::Acquire);
        let shutting_down = self.shutting_down.load(Ordering::Acquire);
        let last_tick_fresh = !shutting_down
            && last_completed_at.is_some_and(|completed| {
                Instant::now().saturating_duration_since(completed) <= self.freshness_timeout
            });
        Ok((
            external_dependencies_healthy,
            last_tick_fresh,
            tick_in_flight,
        ))
    }
}

/// Cloneable status/metrics handle retained by `irohad`.
#[derive(Debug, Clone)]
pub struct ReputationRuntimeHandleV1 {
    runtime: Arc<ReputationRuntimeSupervisorV1>,
    counters: Arc<ReputationDaemonCounters>,
    liveness: Arc<ReputationDaemonLiveness>,
}

impl ReputationRuntimeHandleV1 {
    /// Return payload-free health and readiness without performing work.
    ///
    pub fn status(&self) -> Result<ReputationDaemonStatusV1, ReputationRuntimeError> {
        let runtime = self.runtime.status()?;
        let ingest = self.runtime.ingest_status()?;
        let (external_dependencies_healthy, last_tick_fresh, tick_in_flight) =
            self.liveness.status()?;
        let journal_transaction_submitter_ready =
            runtime.journal_delivery.is_some_and(|status| status.ready);
        let ready = runtime.ready && external_dependencies_healthy && last_tick_fresh;
        Ok(ReputationDaemonStatusV1 {
            runtime,
            ingest,
            external_dependencies_healthy,
            last_tick_fresh,
            tick_in_flight,
            journal_transaction_submitter_ready,
            ready,
        })
    }

    /// Return payload-free supervised and deterministic counters.
    #[must_use]
    pub fn metrics(&self) -> ReputationDaemonMetricsV1 {
        ReputationDaemonMetricsV1 {
            successful_ticks: self.counters.successful_ticks.load(Ordering::Relaxed),
            failed_ticks: self.counters.failed_ticks.load(Ordering::Relaxed),
            panicked_ticks: self.counters.panicked_ticks.load(Ordering::Relaxed),
            timed_out_ticks: self.counters.timed_out_ticks.load(Ordering::Relaxed),
            ingest: self.runtime.ingest_metrics(),
            journal_delivery: self.runtime.journal_delivery_metrics().ok().flatten(),
        }
    }

    /// Return the exact durable committed reputation read projection.
    ///
    /// Threshold-signer output and submitter-side Governance DAG success are
    /// never exposed before authenticated readback and projector
    /// acknowledgement complete.
    ///
    /// # Errors
    ///
    /// Returns a durable-state or runtime-lock error.
    pub fn committed_read_projection(
        &self,
    ) -> Result<ReputationCommittedReadProjectionV1, ReputationRuntimeError> {
        self.runtime.committed_read_projection()
    }

    /// Return only the retained committed-event suffix after `sequence`.
    pub fn committed_events_after(
        &self,
        sequence: u64,
    ) -> Result<Vec<sorafs_manifest::ReputationSnapshotEventV1>, ReputationRuntimeError> {
        self.runtime.committed_events_after(sequence)
    }

    /// Durably enqueue one actual native PoR terminal callback.
    ///
    /// This is a fail-closed injection point for a PoR terminal owner and
    /// returns only after the producer checkpoint is durable. The daemon does
    /// not automatically discover or attach that owner.
    pub fn record_por_terminal(
        &self,
        provider_id: ProviderId,
        outcome: PorTerminalOutcomeV1,
    ) -> Result<
        sorafs_node::reputation::runtime::ReputationJournalEnqueueOutcomeV1,
        ReputationRuntimeError,
    > {
        self.runtime
            .por_journal_producer()
            .ok_or(ReputationRuntimeError::RuntimeBindingMismatch)?
            .enqueue_terminal(provider_id, outcome)
    }

    /// Durably enqueue one provider-attributable stream-token callback.
    ///
    /// Unattributable valid outcomes return `NotCounted`; malformed material
    /// or unavailable durable state fails closed. The stream-token owner must
    /// be explicitly constructed with this callback by deployment wiring.
    pub fn record_stream_token_outcome(
        &self,
        provider_id: ProviderId,
        outcome: StreamTokenValidationOutcomeV1,
    ) -> Result<
        sorafs_node::reputation::runtime::CountedStreamTokenProducerOutcomeV1,
        ReputationRuntimeError,
    > {
        self.runtime
            .counted_stream_token_journal_producer()
            .ok_or(ReputationRuntimeError::RuntimeBindingMismatch)?
            .enqueue_counted(provider_id, outcome)
    }
}

impl ReputationCommittedReadApiV1 for ReputationRuntimeHandleV1 {
    fn committed_read_projection(
        &self,
    ) -> Result<ReputationCommittedReadProjectionV1, ReputationRuntimeError> {
        self.runtime.committed_read_projection()
    }

    fn committed_events_after(
        &self,
        sequence: u64,
    ) -> Result<Vec<sorafs_manifest::ReputationSnapshotEventV1>, ReputationRuntimeError> {
        self.runtime.committed_events_after(sequence)
    }
}

/// Assemble and start the committed reputation runtime.
///
/// Missing, null/test-marked, or identity-mismatched adapters fail startup
/// before any worker is spawned.
pub(crate) fn start(
    config: SorafsReputationRuntime,
    chain_id: ChainId,
    trust_policy: Arc<ReputationSnapshotTrustPolicyV1>,
    dependencies: ReputationRuntimeDependenciesV1,
    shutdown_signal: ShutdownSignal,
) -> Result<(ReputationRuntimeHandleV1, Child)> {
    let poll_interval = config.poll_interval;
    let handle = assemble(config, chain_id, trust_policy, dependencies)?;
    record_status_metrics(&handle);

    let worker = handle.clone();
    let task = tokio::task::spawn(async move {
        let _shutdown_guard = ReputationDaemonShutdownGuard {
            liveness: Arc::clone(&worker.liveness),
        };
        let mut interval = tokio::time::interval(poll_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        'worker: loop {
            tokio::select! {
                _ = interval.tick() => {
                    let runtime = Arc::clone(&worker.runtime);
                    worker.liveness.begin_tick();
                    let mut reconciliation =
                        tokio::task::spawn_blocking(move || runtime.reconcile_once());
                    let timely_result = tokio::select! {
                        result = &mut reconciliation => Some(result),
                        _ = tokio::time::sleep(worker.liveness.tick_timeout) => None,
                        () = shutdown_signal.receive() => {
                            worker.liveness.mark_shutdown();
                            reconciliation.abort();
                            record_status_metrics(&worker);
                            iroha_logger::debug!(
                                "committed SoraFS reputation runtime shut down during reconciliation"
                            );
                            break 'worker;
                        }
                    };
                    if let Some(result) = timely_result {
                        record_reputation_tick_result(&worker, result);
                        record_status_metrics(&worker);
                        continue;
                    }

                    worker.liveness.mark_timeout();
                    worker
                        .counters
                        .timed_out_ticks
                        .fetch_add(1, Ordering::Relaxed);
                    record_tick_metric("failure");
                    record_status_metrics(&worker);
                    iroha_logger::error!(
                        timeout_ms = u64::try_from(worker.liveness.tick_timeout.as_millis())
                            .unwrap_or(u64::MAX),
                        "committed SoraFS reputation reconciliation exceeded its deadline"
                    );

                    let late_result = tokio::select! {
                        result = &mut reconciliation => Some(result),
                        () = shutdown_signal.receive() => {
                            worker.liveness.mark_shutdown();
                            reconciliation.abort();
                            record_status_metrics(&worker);
                            iroha_logger::warn!(
                                "committed SoraFS reputation runtime shut down with a timed-out blocking reconciliation still outstanding"
                            );
                            break 'worker;
                        }
                    };
                    worker.liveness.mark_late_tick_finished();
                    if let Some(Err(error)) = late_result {
                        worker
                            .counters
                            .panicked_ticks
                            .fetch_add(1, Ordering::Relaxed);
                        iroha_logger::error!(
                            cancelled = error.is_cancelled(),
                            panicked = error.is_panic(),
                            "timed-out SoraFS reputation reconciliation later failed"
                        );
                    } else {
                        iroha_logger::warn!(
                            "timed-out SoraFS reputation reconciliation finished late; its result was not used for readiness"
                        );
                    }
                    record_status_metrics(&worker);
                }
                () = shutdown_signal.receive() => {
                    worker.liveness.mark_shutdown();
                    record_status_metrics(&worker);
                    iroha_logger::debug!(
                        "committed SoraFS reputation runtime is being shut down"
                    );
                    break;
                }
                else => break,
            }
        }
    });
    Ok((handle, Child::new(task, OnShutdown::Wait(SHUTDOWN_WAIT))))
}

fn record_reputation_tick_result(
    worker: &ReputationRuntimeHandleV1,
    result: ReputationJoinedReconciliationResult,
) {
    match result {
        Ok(Ok(_)) => {
            if let Err(error) = worker.liveness.finish_tick(true) {
                worker.liveness.mark_timeout();
                iroha_logger::error!(
                    error = %error,
                    "committed SoraFS reputation liveness state is poisoned"
                );
                return;
            }
            worker
                .counters
                .successful_ticks
                .fetch_add(1, Ordering::Relaxed);
            record_tick_metric("success");
        }
        Ok(Err(error)) => {
            if let Err(liveness_error) = worker.liveness.finish_tick(false) {
                worker.liveness.mark_timeout();
                iroha_logger::error!(
                    error = %liveness_error,
                    "committed SoraFS reputation liveness state is poisoned"
                );
            }
            worker.counters.failed_ticks.fetch_add(1, Ordering::Relaxed);
            record_tick_metric("failure");
            iroha_logger::warn!(
                error = %error,
                "committed SoraFS reputation reconciliation failed"
            );
        }
        Err(error) => {
            if let Err(liveness_error) = worker.liveness.finish_tick(false) {
                worker.liveness.mark_timeout();
                iroha_logger::error!(
                    error = %liveness_error,
                    "committed SoraFS reputation liveness state is poisoned"
                );
            }
            worker
                .counters
                .panicked_ticks
                .fetch_add(1, Ordering::Relaxed);
            record_tick_metric("panic");
            iroha_logger::error!(
                cancelled = error.is_cancelled(),
                panicked = error.is_panic(),
                "committed SoraFS reputation worker task failed"
            );
        }
    }
}

fn assemble(
    config: SorafsReputationRuntime,
    chain_id: ChainId,
    trust_policy: Arc<ReputationSnapshotTrustPolicyV1>,
    dependencies: ReputationRuntimeDependenciesV1,
) -> Result<ReputationRuntimeHandleV1> {
    validate_actual_config(&config)?;
    let poll_interval = config.poll_interval;
    validate_dependency_handle(
        "finalized query",
        &config.finalized_query_handle,
        dependencies.finalized_query.handle(),
    )?;
    validate_dependency_handle(
        "journal transaction submitter",
        &config.journal_transaction_submitter_handle,
        dependencies.journal_transaction_submitter.handle(),
    )?;
    validate_dependency_handle(
        "threshold signer",
        &config.threshold_signer_handle,
        dependencies.threshold_signer.handle(),
    )?;
    validate_dependency_handle(
        "Governance DAG",
        &config.governance_dag_handle,
        dependencies.governance_dag.handle(),
    )?;
    dependencies
        .finalized_query
        .check_readiness()
        .wrap_err("committed reputation finalized-query adapter is not ready")?;
    dependencies
        .journal_transaction_submitter
        .check_readiness()
        .wrap_err("reputation journal transaction submitter is not ready")?;
    dependencies
        .threshold_signer
        .check_readiness()
        .wrap_err("committed reputation threshold-signer adapter is not ready")?;
    dependencies
        .governance_dag
        .check_readiness()
        .wrap_err("committed reputation Governance DAG adapter is not ready")?;

    let bootstrap_delivery_view = dependencies
        .finalized_query
        .reputation_journal_delivery_view(
            &chain_id,
            u64::MAX,
            FindSorafsReputationJournalAuthorityPolicy,
            None,
            1,
        )
        .wrap_err("read exact finalized reputation journal authority policy")?;
    bootstrap_delivery_view
        .authority_policy
        .validate()
        .wrap_err("validate exact finalized reputation journal authority policy")?;
    if bootstrap_delivery_view.anchor.chain_id != chain_id
        || bootstrap_delivery_view
            .authority_policy
            .activated_at_unix_ms
            > bootstrap_delivery_view.anchor.finalized_at_unix_ms
        || bootstrap_delivery_view.journal_page.finalized_cursor
            != journal_cursor(&bootstrap_delivery_view.anchor)
    {
        bail!("exact finalized reputation journal bootstrap view is inconsistent");
    }
    for authority in [
        &bootstrap_delivery_view
            .authority_policy
            .policy
            .por_recorder_authority,
        &bootstrap_delivery_view
            .authority_policy
            .policy
            .token_recorder_authority,
    ] {
        if !dependencies
            .journal_transaction_submitter
            .supports_authority(authority)
        {
            bail!("reputation journal submitter does not own a governed recorder identity");
        }
    }

    let trust_policy_digest = trust_policy
        .canonical_digest()
        .wrap_err("derive configured reputation trust-policy digest")?;
    let weights = ReputationWeightsV1 {
        version: REPUTATION_WEIGHTS_VERSION_V1,
        por_success_bps: config.por_success_bps,
        pdp_success_bps: config.pdp_success_bps,
        potr_success_bps: config.potr_success_bps,
        latency_bps: config.latency_bps,
        dispute_bps: config.dispute_bps,
        token_violation_bps: config.token_violation_bps,
        repair_breach_bps: config.repair_breach_bps,
    };
    weights
        .validate()
        .wrap_err("validate configured reputation weights")?;
    let mut ingest_policy = ReputationIngestPolicyV1::strict_v1(
        chain_id.clone(),
        config.window_start_height,
        config.window_end_height,
        trust_policy_digest,
        weights,
    );
    ingest_policy.max_providers = config.max_providers;
    ingest_policy.max_pending_events = config.max_pending_events;
    ingest_policy.max_replay_receipts = config.max_replay_receipts;
    ingest_policy.max_pages_per_batch = config.max_pages_per_batch;
    ingest_policy.max_material_delivery_failures = config.max_material_delivery_failures;
    ingest_policy.checkpoint_max_bytes = config.ingest_checkpoint_max_bytes.0;
    ingest_policy
        .validate()
        .wrap_err("validate configured committed reputation ingest policy")?;

    let query_policy = ReputationFinalizedQueryPolicyV1::try_new(
        &ingest_policy,
        config.finalized_query_handle.clone(),
        config.page_items,
        config.max_pages_per_batch,
    )
    .wrap_err("construct committed reputation finalized-query policy")?;
    let projector = Arc::new(
        ReputationIngestService::open(&config.state_dir, ingest_policy.clone())
            .wrap_err("open committed reputation projector")?,
    );
    let finalized = ReputationCommittedProjectorRuntimeV1::new(
        Arc::clone(&projector),
        &ingest_policy,
        query_policy,
        Arc::clone(&dependencies.finalized_query),
    )
    .wrap_err("bind committed reputation finalized-query runtime")?;
    let producer_policy = ReputationJournalProducerPolicyV1::strict_v1(
        chain_id.clone(),
        bootstrap_delivery_view.authority_policy.policy,
    )
    .wrap_err("construct reputation journal producer policy")?;
    let journal_outbox = Arc::new(
        ReputationJournalProducerOutboxV1::open(&config.state_dir, producer_policy)
            .wrap_err("open durable reputation journal producer outbox")?,
    );
    let journal_delivery_policy = ReputationJournalDeliveryPolicyV1::strict_v1(
        chain_id,
        dependencies.finalized_query.handle().to_owned(),
        config.journal_transaction_submitter_handle,
    )
    .wrap_err("construct reputation journal delivery policy")?;
    let journal_delivery = ReputationJournalDeliveryWorkerV1::new(
        journal_outbox,
        journal_delivery_policy,
        Arc::clone(&dependencies.finalized_query),
        Arc::clone(&dependencies.journal_transaction_submitter),
    )
    .wrap_err("bind reputation journal delivery worker")?;
    let publication_policy = ReputationPublicationPolicyV1::try_new(
        trust_policy.as_ref(),
        config.threshold_signer_handle,
        config.governance_dag_handle,
        config.governance_publisher_peer_id,
        config.governance_publisher_public_key,
        config.publication_checkpoint_max_bytes.0,
    )
    .wrap_err("construct committed reputation publication policy")?;
    let publication = ReputationPublicationReconcilerV1::open(
        &config.state_dir,
        Arc::clone(&projector),
        trust_policy.as_ref().clone(),
        publication_policy,
        dependencies.threshold_signer,
        dependencies.governance_dag,
    )
    .wrap_err("open committed reputation publication reconciler")?;
    let runtime = Arc::new(
        ReputationRuntimeSupervisorV1::new(projector, finalized, publication)
            .and_then(|runtime| runtime.with_journal_delivery(journal_delivery))
            .wrap_err("assemble committed reputation runtime")?,
    );
    let handle = ReputationRuntimeHandleV1 {
        runtime,
        counters: Arc::new(ReputationDaemonCounters::default()),
        liveness: Arc::new(ReputationDaemonLiveness::new(poll_interval)),
    };
    Ok(handle)
}

fn validate_actual_config(config: &SorafsReputationRuntime) -> Result<()> {
    if !config.state_dir.is_absolute()
        || config.state_dir.file_name().is_none()
        || config.state_dir.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
        || config.window_start_height == 0
        || config.window_end_height < config.window_start_height
        || config.poll_interval < Duration::from_millis(100)
        || config.poll_interval > Duration::from_secs(60)
    {
        bail!("committed reputation runtime configuration is invalid");
    }
    for handle in [
        &config.finalized_query_handle,
        &config.journal_transaction_submitter_handle,
        &config.threshold_signer_handle,
        &config.governance_dag_handle,
    ] {
        if !is_production_handle(handle) {
            bail!("committed reputation runtime dependency handle is not production-safe");
        }
    }
    Ok(())
}

fn validate_dependency_handle(label: &str, expected: &str, actual: &str) -> Result<()> {
    if !is_production_handle(actual) || actual != expected {
        bail!("{label} adapter identity does not match committed reputation configuration");
    }
    Ok(())
}

fn is_production_handle(handle: &str) -> bool {
    if handle.is_empty()
        || handle.len() > 256
        || !handle.is_ascii()
        || handle
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return false;
    }
    let lowercase = handle.to_ascii_lowercase();
    !lowercase
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(|component| {
            matches!(
                component,
                "null" | "mock" | "test" | "dev" | "fake" | "placeholder"
            )
        })
}

#[cfg(feature = "telemetry")]
fn record_status_metrics(handle: &ReputationRuntimeHandleV1) {
    let Some(metrics) = iroha_telemetry::metrics::global() else {
        return;
    };
    match handle.status() {
        Ok(status) => metrics.record_sorafs_reputation_runtime_status(
            status.runtime.finalized.live,
            status.ready,
            status.external_dependencies_healthy,
            status.journal_transaction_submitter_ready,
            status
                .ingest
                .latest_finalized
                .map_or(0, |identity| identity.height),
            status.runtime.finalized.consecutive_failures,
            status.runtime.material_acknowledged,
            status.ingest.providers,
        ),
        Err(_) => metrics
            .record_sorafs_reputation_runtime_status(false, false, false, false, 0, 1, false, 0),
    }
}

#[cfg(not(feature = "telemetry"))]
fn record_status_metrics(_handle: &ReputationRuntimeHandleV1) {}

#[cfg(feature = "telemetry")]
fn record_tick_metric(result: &str) {
    if let Some(metrics) = iroha_telemetry::metrics::global() {
        metrics.inc_sorafs_reputation_runtime_tick(result);
    }
}

#[cfg(not(feature = "telemetry"))]
fn record_tick_metric(_result: &str) {}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use iroha_config::base::util::Bytes;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::sorafs::{
        capacity::ProviderId,
        moderation_ledger::{RepairFinalizedEventCursorV1, RepairFinalizedEventPageV1},
        orderbook::{OrderbookFinalizedEventCursorV1, OrderbookFinalizedEventPageV1},
        proof_ledger::{ProofOutcomeFinalizedEventCursorV1, ProofOutcomeFinalizedEventPageV1},
        reputation::{
            REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
            ReputationJournalAuthorityPolicyRecordV1, ReputationJournalAuthorityPolicyV1,
            ReputationJournalFinalizedCursorV1, ReputationJournalFinalizedEventCursorV1,
            ReputationJournalFinalizedEventPageV1,
        },
        reserve::{
            ReserveFinalizedEventCursorV1, ReserveFinalizedEventPageV1,
            ReserveProviderAccountPageV1,
        },
    };
    use sorafs_manifest::{
        GovernanceDagBlockV1, SignedReputationSnapshotV1,
        reputation::signed::{
            REPUTATION_SNAPSHOT_TRUST_POLICY_VERSION_V1, REPUTATION_TRUSTED_SIGNER_VERSION_V1,
            ReputationTrustedSignerV1,
        },
    };
    use sorafs_node::reputation::runtime::{
        ReputationExternalFailureV1, ReputationFinalizedAnchorV1,
        ReputationGovernanceDagPublicationRequestV1, ReputationJournalDeliveryFinalizedViewV1,
        ReputationThresholdSigningRequestV1,
    };
    use tempfile::TempDir;

    use super::*;

    #[derive(Debug)]
    struct UnavailableQuery {
        handle: String,
        ready: bool,
    }

    impl ReputationFinalizedQueryV1 for UnavailableQuery {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn check_readiness(&self) -> Result<(), ReputationExternalFailureV1> {
            if self.ready {
                Ok(())
            } else {
                Err(ReputationExternalFailureV1::try_new([0x91; 32])
                    .expect("non-zero readiness failure receipt"))
            }
        }

        fn finalized_at_or_before(
            &self,
            _chain_id: &ChainId,
            _maximum_height: u64,
        ) -> Result<ReputationFinalizedAnchorV1, ReputationExternalFailureV1> {
            unreachable!("assembly must not query external state")
        }

        fn reputation_journal_delivery_view(
            &self,
            chain_id: &ChainId,
            _maximum_height: u64,
            _policy_query: FindSorafsReputationJournalAuthorityPolicy,
            _after: Option<ReputationJournalFinalizedEventCursorV1>,
            _limit: u32,
        ) -> Result<ReputationJournalDeliveryFinalizedViewV1, ReputationExternalFailureV1> {
            let anchor = ReputationFinalizedAnchorV1 {
                chain_id: chain_id.clone(),
                identity: sorafs_node::reputation::ReputationFinalizedIdentityV1 {
                    height: 1,
                    block_hash: [0x81; 32],
                },
                finalized_at_unix_ms: 1_800_000_000_000,
            };
            let policy = ReputationJournalAuthorityPolicyV1 {
                version: REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
                revision: 1,
                predecessor_policy_digest: None,
                por_recorder_authority: account(1),
                dispute_recorder_authority: account(2),
                token_recorder_authority: account(3),
                max_source_age_ms: 86_400_000,
            };
            Ok(ReputationJournalDeliveryFinalizedViewV1 {
                authority_policy: ReputationJournalAuthorityPolicyRecordV1::try_new(
                    policy,
                    account(4),
                    anchor.finalized_at_unix_ms,
                )
                .expect("authority policy record"),
                journal_page: ReputationJournalFinalizedEventPageV1 {
                    finalized_cursor: ReputationJournalFinalizedCursorV1 {
                        height: anchor.identity.height,
                        block_hash: anchor.identity.block_hash,
                        finalized_at_unix_ms: anchor.finalized_at_unix_ms,
                    },
                    events: Vec::new(),
                    has_more: false,
                    next_after: None,
                },
                anchor,
            })
        }

        fn proof_outcome_page(
            &self,
            _anchor: &ReputationFinalizedAnchorV1,
            _after: Option<ProofOutcomeFinalizedEventCursorV1>,
            _limit: u32,
        ) -> Result<ProofOutcomeFinalizedEventPageV1, ReputationExternalFailureV1> {
            unreachable!("assembly must not query external state")
        }

        fn reputation_journal_page(
            &self,
            _anchor: &ReputationFinalizedAnchorV1,
            _after: Option<ReputationJournalFinalizedEventCursorV1>,
            _limit: u32,
        ) -> Result<ReputationJournalFinalizedEventPageV1, ReputationExternalFailureV1> {
            unreachable!("assembly must not query external state")
        }

        fn repair_page(
            &self,
            _anchor: &ReputationFinalizedAnchorV1,
            _after: Option<RepairFinalizedEventCursorV1>,
            _limit: u32,
        ) -> Result<RepairFinalizedEventPageV1, ReputationExternalFailureV1> {
            unreachable!("assembly must not query external state")
        }

        fn orderbook_page(
            &self,
            _anchor: &ReputationFinalizedAnchorV1,
            _after: Option<OrderbookFinalizedEventCursorV1>,
            _limit: u32,
        ) -> Result<OrderbookFinalizedEventPageV1, ReputationExternalFailureV1> {
            unreachable!("assembly must not query external state")
        }

        fn reserve_page(
            &self,
            _anchor: &ReputationFinalizedAnchorV1,
            _after: Option<ReserveFinalizedEventCursorV1>,
            _limit: u32,
        ) -> Result<ReserveFinalizedEventPageV1, ReputationExternalFailureV1> {
            unreachable!("assembly must not query external state")
        }

        fn reserve_provider_page(
            &self,
            _anchor: &ReputationFinalizedAnchorV1,
            _after_provider_id: Option<ProviderId>,
            _limit: u32,
        ) -> Result<ReserveProviderAccountPageV1, ReputationExternalFailureV1> {
            unreachable!("assembly must not query external state")
        }
    }

    #[derive(Debug)]
    struct PendingThresholdSigner {
        handle: String,
    }

    #[derive(Debug)]
    struct PendingJournalSubmitter {
        handle: String,
    }

    impl ReputationJournalTransactionSubmitterV1 for PendingJournalSubmitter {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn check_readiness(&self) -> Result<(), ReputationExternalFailureV1> {
            Ok(())
        }

        fn supports_authority(&self, _authority: &AccountId) -> bool {
            true
        }

        fn submit(
            &self,
            request: &ReputationJournalTransactionRequestV1,
        ) -> ReputationJournalTransactionSubmitOutcomeV1 {
            ReputationJournalTransactionSubmitOutcomeV1::Ambiguous {
                receipt: request.idempotency_key,
            }
        }
    }

    impl ReputationThresholdSignerClientV1 for PendingThresholdSigner {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn check_readiness(&self) -> Result<(), ReputationExternalFailureV1> {
            Ok(())
        }

        fn reconcile_signature(
            &self,
            _request: &ReputationThresholdSigningRequestV1,
        ) -> Result<Option<SignedReputationSnapshotV1>, ReputationExternalFailureV1> {
            Ok(None)
        }
    }

    #[derive(Debug)]
    struct PendingGovernanceDag {
        handle: String,
    }

    impl ReputationGovernanceDagClientV1 for PendingGovernanceDag {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn check_readiness(&self) -> Result<(), ReputationExternalFailureV1> {
            Ok(())
        }

        fn reconcile_publication(
            &self,
            _request: &ReputationGovernanceDagPublicationRequestV1,
        ) -> Result<Option<GovernanceDagBlockV1>, ReputationExternalFailureV1> {
            Ok(None)
        }
    }

    fn public_key(seed: u8) -> [u8; 32] {
        let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("deterministic Ed25519 key");
        key.public_key()
            .to_bytes()
            .1
            .try_into()
            .expect("Ed25519 public key is 32 bytes")
    }

    fn account(seed: u8) -> AccountId {
        let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("deterministic account");
        AccountId::new(key.public_key().clone())
    }

    fn trust_policy() -> Arc<ReputationSnapshotTrustPolicyV1> {
        Arc::new(ReputationSnapshotTrustPolicyV1 {
            version: REPUTATION_SNAPSHOT_TRUST_POLICY_VERSION_V1,
            policy_id: [0x72; 32],
            valid_from_unix: 1_800_000_000,
            valid_until_unix: 1_900_000_000,
            max_snapshot_age_secs: 600,
            max_future_skew_secs: 30,
            min_signatures: 1,
            signers: vec![ReputationTrustedSignerV1 {
                version: REPUTATION_TRUSTED_SIGNER_VERSION_V1,
                signer_id: "threshold-a".to_owned(),
                public_key: public_key(0x71),
            }],
            revoked_signer_ids: Vec::new(),
        })
    }

    fn config(state_dir: PathBuf) -> SorafsReputationRuntime {
        SorafsReputationRuntime {
            state_dir,
            window_start_height: 1,
            window_end_height: 10,
            finalized_query_handle: "ledger.finalized.primary".to_owned(),
            journal_transaction_submitter_handle: "queue.reputation.journal".to_owned(),
            threshold_signer_handle: "hsm.reputation.threshold".to_owned(),
            governance_dag_handle: "governance.dag.publisher".to_owned(),
            governance_publisher_peer_id: b"12D3KooWProductionPublisher".to_vec(),
            governance_publisher_public_key: public_key(0x73),
            poll_interval: Duration::from_secs(1),
            page_items: 64,
            max_pages_per_batch: 4_096,
            max_providers: 65_536,
            max_pending_events: 65_536,
            max_replay_receipts: 262_144,
            max_material_delivery_failures: 64,
            ingest_checkpoint_max_bytes: Bytes(64 * 1024 * 1024),
            publication_checkpoint_max_bytes: Bytes(32 * 1024 * 1024),
            por_success_bps: 2_200,
            pdp_success_bps: 2_000,
            potr_success_bps: 1_800,
            latency_bps: 1_500,
            dispute_bps: 1_000,
            token_violation_bps: 500,
            repair_breach_bps: 1_000,
        }
    }

    fn dependencies(query_handle: &str) -> ReputationRuntimeDependenciesV1 {
        ReputationRuntimeDependenciesV1 {
            finalized_query: Arc::new(UnavailableQuery {
                handle: query_handle.to_owned(),
                ready: true,
            }),
            journal_transaction_submitter: Arc::new(PendingJournalSubmitter {
                handle: "queue.reputation.journal".to_owned(),
            }),
            threshold_signer: Arc::new(PendingThresholdSigner {
                handle: "hsm.reputation.threshold".to_owned(),
            }),
            governance_dag: Arc::new(PendingGovernanceDag {
                handle: "governance.dag.publisher".to_owned(),
            }),
        }
    }

    #[test]
    fn production_config_rejects_null_test_handles_and_unsafe_paths() {
        let temp = TempDir::new().expect("tempdir");
        let mut invalid_handle_config = config(temp.path().to_path_buf());
        invalid_handle_config.finalized_query_handle = "null-query.test".to_owned();
        assert!(validate_actual_config(&invalid_handle_config).is_err());

        let mut unsafe_path_config = config(PathBuf::from("/var/lib/iroha/../reputation"));
        unsafe_path_config.finalized_query_handle = "ledger.finalized.primary".to_owned();
        assert!(validate_actual_config(&unsafe_path_config).is_err());
    }

    #[test]
    fn daemon_liveness_is_fail_closed_freshness_bounded_and_shutdown_safe() {
        let liveness = Arc::new(ReputationDaemonLiveness::new(Duration::from_secs(1)));
        assert_eq!(
            liveness.status().expect("initial liveness"),
            (false, false, false)
        );

        liveness.begin_tick();
        assert_eq!(
            liveness.status().expect("in-flight liveness"),
            (false, false, true)
        );
        liveness.finish_tick(true).expect("complete healthy tick");
        assert_eq!(
            liveness.status().expect("healthy liveness"),
            (true, true, false)
        );

        *liveness
            .last_completed_at
            .lock()
            .expect("liveness timestamp lock") = Instant::now().checked_sub(
            liveness
                .freshness_timeout
                .saturating_add(Duration::from_millis(1)),
        );
        assert_eq!(
            liveness.status().expect("stale liveness"),
            (true, false, false)
        );

        drop(ReputationDaemonShutdownGuard {
            liveness: Arc::clone(&liveness),
        });
        assert_eq!(
            liveness.status().expect("shutdown liveness"),
            (false, false, false)
        );
    }

    #[test]
    fn assembly_rejects_adapter_identity_substitution_before_external_calls() {
        let temp = TempDir::new().expect("tempdir");
        let error = assemble(
            config(temp.path().to_path_buf()),
            ChainId::from("reputation-runtime-test"),
            trust_policy(),
            dependencies("ledger.finalized.substituted"),
        )
        .expect_err("mismatched query identity must fail startup");
        assert!(error.to_string().contains("identity"));
    }

    #[test]
    fn startup_rejects_each_missing_runtime_adapter() {
        let complete = dependencies("ledger.finalized.primary");
        assert!(
            ReputationRuntimeDependenciesV1::require(
                None,
                Some(Arc::clone(&complete.journal_transaction_submitter)),
                Some(Arc::clone(&complete.threshold_signer)),
                Some(Arc::clone(&complete.governance_dag)),
            )
            .is_err()
        );
        assert!(
            ReputationRuntimeDependenciesV1::require(
                Some(Arc::clone(&complete.finalized_query)),
                None,
                Some(Arc::clone(&complete.threshold_signer)),
                Some(Arc::clone(&complete.governance_dag)),
            )
            .is_err()
        );
        assert!(
            ReputationRuntimeDependenciesV1::require(
                Some(Arc::clone(&complete.finalized_query)),
                Some(Arc::clone(&complete.journal_transaction_submitter)),
                None,
                Some(Arc::clone(&complete.governance_dag)),
            )
            .is_err()
        );
        assert!(
            ReputationRuntimeDependenciesV1::require(
                Some(Arc::clone(&complete.finalized_query)),
                Some(Arc::clone(&complete.journal_transaction_submitter)),
                Some(Arc::clone(&complete.threshold_signer)),
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn assembly_rejects_unready_adapter_before_state_open() {
        let temp = TempDir::new().expect("tempdir");
        let state_dir = temp.path().join("must-not-exist");
        let mut dependencies = dependencies("ledger.finalized.primary");
        dependencies.finalized_query = Arc::new(UnavailableQuery {
            handle: "ledger.finalized.primary".to_owned(),
            ready: false,
        });

        let error = assemble(
            config(state_dir.clone()),
            ChainId::from("reputation-runtime-test"),
            trust_policy(),
            dependencies,
        )
        .expect_err("unready query adapter must fail startup");

        assert!(error.to_string().contains("not ready"));
        assert!(
            !state_dir.exists(),
            "adapter readiness must be verified before state is opened"
        );
    }

    #[test]
    fn checkpoint_runtime_reopens_without_claiming_journal_readiness() {
        let temp = TempDir::new().expect("tempdir");
        let config = config(temp.path().to_path_buf());
        let first = assemble(
            config.clone(),
            ChainId::from("reputation-runtime-test"),
            trust_policy(),
            dependencies("ledger.finalized.primary"),
        )
        .expect("first assembly");
        let first_status = first.status().expect("first status");
        assert!(!first_status.ready);
        assert!(!first_status.external_dependencies_healthy);
        assert!(!first_status.last_tick_fresh);
        assert!(!first_status.journal_transaction_submitter_ready);
        let committed_reader: &dyn ReputationCommittedReadApiV1 = &first;
        assert!(
            committed_reader
                .committed_read_projection()
                .expect("object-safe committed read")
                .latest
                .is_none()
        );
        drop(first);

        let restarted = assemble(
            config,
            ChainId::from("reputation-runtime-test"),
            trust_policy(),
            dependencies("ledger.finalized.primary"),
        )
        .expect("restart assembly");
        let restarted_status = restarted.status().expect("restart status");
        assert_eq!(restarted_status.ingest.latest_finalized, None);
        assert!(!restarted_status.ready);
        assert!(!restarted_status.external_dependencies_healthy);
        assert!(!restarted_status.last_tick_fresh);
        assert!(!restarted_status.journal_transaction_submitter_ready);
        assert!(
            restarted
                .committed_read_projection()
                .expect("restarted committed read")
                .latest
                .is_none()
        );
    }
}
