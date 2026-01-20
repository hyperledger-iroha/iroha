//! SoraFS node scaffolding.

#![deny(missing_docs)]

pub mod capacity;
pub mod config;
pub mod deal;
pub mod gateway;
mod governance;
pub mod metering;
pub mod por;
pub mod potr;
pub mod repair;
pub mod scheduler;
pub mod store;
pub mod telemetry;

pub use deal::{
    ClientSnapshot, DealEngine, DealEngineError, DealSettlementOutcome, DealSnapshot,
    ProviderSnapshot, UsageOutcome,
};
pub use por::{
    ManifestVrfBundle, PlannedChallenge, PorChallengePlannerError, PorRandomness, PorTracker,
    PorTrackerError, PorVerdictStats, build_por_challenge_for_manifest,
};

/// Outcome returned when recording a PoR verdict.
#[derive(Debug, Clone)]
pub struct PorVerdictOutcome {
    /// Statistics extracted from the verdict.
    pub stats: PorVerdictStats,
    /// Identifier that can be referenced by repair reports (present on failure).
    pub repair_history_id: Option<u64>,
    /// Current consecutive failure streak for this provider/manifest.
    pub consecutive_failures: u64,
    /// Recommended slash derived from the configured policy and provider bond state.
    pub slash: Option<SlashRecommendation>,
}

/// Slash recommendation produced after PoR verification failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashRecommendation {
    /// Provider identifier targeted by the slash.
    pub provider_id: ProviderId,
    /// Manifest digest associated with the failed proof.
    pub manifest_digest: [u8; 32],
    /// Proposed penalty (nano-XOR) to be debited from the provider bond.
    pub penalty_nano: u128,
    /// Failure streak length that triggered the slash.
    pub strikes: u32,
    /// Reason recorded for the recommendation.
    pub reason: String,
}

/// Aggregated PoR ingestion status for a manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PorIngestionStatus {
    /// Manifest digest covered by the snapshot.
    pub manifest_digest: [u8; 32],
    /// Provider-specific status entries.
    pub providers: Vec<PorIngestionProviderStatus>,
}

/// Provider-level PoR ingestion state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PorIngestionProviderStatus {
    /// Manifest digest served by the provider.
    pub manifest_digest: [u8; 32],
    /// Provider identifier.
    pub provider_id: [u8; 32],
    /// Outstanding challenge count.
    pub pending_challenges: u64,
    /// Oldest epoch identifier recorded in the backlog.
    pub oldest_epoch_id: Option<u64>,
    /// Earliest pending response deadline.
    pub oldest_response_deadline_unix: Option<u64>,
    /// Unix timestamp for the most recent success verdict.
    pub last_success_unix: Option<u64>,
    /// Unix timestamp for the most recent failure verdict.
    pub last_failure_unix: Option<u64>,
    /// Total failure count recorded for the manifest/provider pair.
    pub failures_total: u64,
    /// Consecutive failure streak length.
    pub consecutive_failures: u64,
}
use std::{
    collections::HashMap,
    io::Read,
    sync::{Arc, RwLock},
};

use capacity::{
    CapacityError, CapacityManager, CapacityUsageSnapshot, DeclarationWindow, ReplicationPlan,
    ReplicationRelease,
};
use config::{GcConfig, RepairConfig, StorageConfig};
use iroha_data_model::{
    da::ingest::DaStripeLayout,
    sorafs::{
        capacity::{CapacityDeclarationRecord, ProviderId},
        deal::{ClientId, DealId, DealProposal, DealRecord, DealUsageReport},
    },
};
use iroha_telemetry::metrics::global_sorafs_node_otel;
use norito::codec::Encode;
pub use repair::{RepairManager, RepairSchedulerError, RepairTaskFilters, RepairTaskSnapshot};
use sorafs_car::{CarBuildPlan, PorProof};
use sorafs_manifest::{
    ManifestV1,
    capacity::{CapacityTelemetryV1, ReplicationOrderV1},
    deal::DealSettlementV1,
    por::{AuditOutcomeV1, AuditVerdictV1, PorChallengeV1, PorProofV1},
    potr::{PotrReceiptV1, PotrReceiptValidationError},
    proof_stream::ProofStreamTier,
    repair::{RepairReportV1, RepairSlashProposalV1, RepairTaskRecordV1, RepairTicketId},
};
use thiserror::Error;

use crate::{
    governance::FilesystemGovernancePublisher,
    metering::{CapacityMeter, MeteringSnapshot, ReplicationUsageSample},
    potr::PotrTracker,
    scheduler::{StorageSchedulerConfig, StorageSchedulersRuntime},
    store::{ChunkFileRecord, ChunkRoleMetadata, StorageBackend, StorageError, StoredManifest},
    telemetry::{TelemetryAccumulator, TelemetryError},
};

/// Interface for emitting settlement artefacts to the governance DAG.
pub trait GovernancePublisher: Send + Sync + std::fmt::Debug {
    /// Persist the supplied settlement NORITO payload to the governance pipeline.
    fn publish_deal_settlement(
        &self,
        settlement: &DealSettlementV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
}

/// Errors surfaced when publishing governance artefacts fails.
#[derive(Debug, Error)]
pub enum GovernancePublishError {
    /// Underlying IO error while writing the artefact.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Generic publish failure with human-readable context.
    #[error("{0}")]
    Other(String),
}

impl GovernancePublishError {
    /// Construct a generic publish failure.
    #[must_use]
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other(message.into())
    }
}

/// Lightweight handle representing the embedded SoraFS storage worker.
#[derive(Debug, Clone)]
pub struct NodeHandle {
    config: StorageConfig,
    repair_config: RepairConfig,
    gc_config: GcConfig,
    capacity: Arc<CapacityManager>,
    meter: CapacityMeter,
    telemetry: Arc<RwLock<Option<TelemetryAccumulator>>>,
    schedulers: StorageSchedulersRuntime,
    por: PorTracker,
    potr: PotrTracker,
    por_history: Arc<RwLock<HashMap<PorHistoryKey, PorHistoryEntry>>>,
    storage: Option<Arc<StorageBackend>>,
    deal_engine: DealEngine,
    repair: RepairManager,
    governance_publisher: Arc<RwLock<Option<Arc<dyn GovernancePublisher>>>>,
}

type PorHistoryKey = ([u8; 32], [u8; 32]);

#[derive(Debug, Default, Clone)]
struct PorHistoryEntry {
    last_success_unix: Option<u64>,
    last_failure_unix: Option<u64>,
    failures_total: u64,
    consecutive_failures: u64,
    last_slash_unix: Option<u64>,
}

/// Error type returned by storage-related operations on [`NodeHandle`].
#[derive(Debug, Error)]
pub enum NodeStorageError {
    /// Storage subsystem is disabled in the configuration.
    #[error("SoraFS storage is disabled for this node")]
    Disabled,
    /// Underlying storage backend reported an error.
    #[error(transparent)]
    Storage(#[from] StorageError),
}

impl NodeHandle {
    /// Construct a new handle for the embedded storage worker.
    #[must_use]
    pub fn new(config: StorageConfig) -> Self {
        Self::new_with_policies(config, RepairConfig::default(), GcConfig::default())
    }

    /// Construct a new handle with explicit repair/GC policies.
    #[must_use]
    pub fn new_with_policies(
        config: StorageConfig,
        repair_config: RepairConfig,
        gc_config: GcConfig,
    ) -> Self {
        let scheduler_config = StorageSchedulerConfig::from_storage_config(&config);
        let schedulers = StorageSchedulersRuntime::new(scheduler_config);
        let capacity_limit = config.max_capacity_bytes().0;

        let storage = if config.enabled() {
            let backend = Arc::new(StorageBackend::new(config.clone()).unwrap_or_else(|err| {
                panic!("failed to initialise SoraFS storage backend: {err}")
            }));
            schedulers.update_storage_bytes(backend.total_bytes(), capacity_limit);
            Some(backend)
        } else {
            schedulers.update_storage_bytes(0, capacity_limit);
            None
        };

        let smoothing = config.smoothing_config();
        let deal_engine = DealEngine::new();
        let governance_dir = config.governance_dir().cloned();

        let repair = RepairManager::new_with_config(repair_config.clone());
        let node = Self {
            config,
            repair_config,
            gc_config,
            capacity: Arc::new(CapacityManager::new()),
            meter: CapacityMeter::with_smoothing(smoothing),
            telemetry: Arc::new(RwLock::new(None)),
            schedulers,
            por: PorTracker::default(),
            potr: PotrTracker::default(),
            por_history: Arc::new(RwLock::new(HashMap::new())),
            storage,
            deal_engine,
            repair,
            governance_publisher: Arc::new(RwLock::new(None)),
        };

        if node.storage.is_some() {
            if let Some(dir) = governance_dir.clone() {
                match FilesystemGovernancePublisher::try_new(dir.clone()) {
                    Ok(publisher) => {
                        iroha_logger::info!(
                            path = ?dir,
                            "SoraFS governance publisher initialised"
                        );
                        node.set_governance_publisher(Arc::new(publisher));
                    }
                    Err(err) => {
                        iroha_logger::error!(
                            ?err,
                            path = ?dir,
                            "failed to initialise SoraFS governance publisher"
                        );
                    }
                }
            }
        } else if governance_dir.is_some() {
            iroha_logger::warn!(
                "skipping governance publisher initialisation: storage backend disabled"
            );
        }

        node
    }

    /// Returns a reference to the storage configuration.
    #[must_use]
    pub fn config(&self) -> &StorageConfig {
        &self.config
    }

    /// Returns a reference to the repair scheduler configuration.
    #[must_use]
    pub fn repair_config(&self) -> &RepairConfig {
        &self.repair_config
    }

    /// Returns a reference to the GC scheduler configuration.
    #[must_use]
    pub fn gc_config(&self) -> &GcConfig {
        &self.gc_config
    }

    /// Returns a clone of the embedded deal engine handle.
    #[must_use]
    pub fn deal_engine(&self) -> DealEngine {
        self.deal_engine.clone()
    }

    /// Deposit provider bond collateral (nano-XOR units).
    pub fn deposit_provider_bond(
        &self,
        provider_id: ProviderId,
        amount_nano: u128,
    ) -> ProviderSnapshot {
        self.deal_engine
            .deposit_provider_bond(provider_id, amount_nano)
    }

    /// Deposit client credit balance (nano-XOR units).
    pub fn deposit_client_credit(&self, client_id: ClientId, amount_nano: u128) -> ClientSnapshot {
        self.deal_engine
            .deposit_client_credit(client_id, amount_nano)
    }

    /// Open a deal using the supplied proposal and activation epoch.
    pub fn open_deal(
        &self,
        proposal: DealProposal,
        activation_epoch: u64,
    ) -> Result<DealRecord, DealEngineError> {
        self.deal_engine.open_deal(proposal, activation_epoch)
    }

    /// Record usage attributed to a deal and evaluate probabilistic micropayments.
    pub fn record_deal_usage(
        &self,
        report: DealUsageReport,
    ) -> Result<UsageOutcome, DealEngineError> {
        self.deal_engine.record_usage(report)
    }

    /// Register the governance publisher used to surface settlement artefacts.
    pub fn set_governance_publisher(&self, publisher: Arc<dyn GovernancePublisher>) {
        if let Ok(mut guard) = self.governance_publisher.write() {
            *guard = Some(publisher);
        }
    }

    /// Remove any configured governance publisher.
    pub fn clear_governance_publisher(&self) {
        if let Ok(mut guard) = self.governance_publisher.write() {
            *guard = None;
        }
    }

    /// Finalise a deal settlement for the supplied epoch.
    pub fn settle_deal(
        &self,
        deal_id: DealId,
        settlement_epoch: u64,
    ) -> Result<DealSettlementOutcome, DealEngineError> {
        let outcome = self.deal_engine.settle(deal_id, settlement_epoch)?;
        let publisher = self
            .governance_publisher
            .read()
            .ok()
            .and_then(|guard| guard.clone());
        let provider_hex = hex::encode(outcome.record.provider_id.as_bytes());
        if let Some(publisher) = publisher {
            let encoded = outcome.governance.encode();
            match publisher.publish_deal_settlement(&outcome.governance, &encoded) {
                Ok(()) => {
                    global_sorafs_node_otel().record_settlement_publish(&provider_hex, "success");
                }
                Err(err) => {
                    global_sorafs_node_otel().record_settlement_publish(&provider_hex, "failure");
                    let deal_hex = hex::encode(outcome.record.deal_id.as_bytes());
                    iroha_logger::error!(
                        %deal_hex,
                        %provider_hex,
                        error = %err,
                        "failed to publish SoraFS settlement artefact to governance DAG"
                    );
                }
            }
        }
        Ok(outcome)
    }

    /// Capture a snapshot of the deal ledger.
    #[must_use]
    pub fn deal_snapshot(&self, deal_id: DealId) -> Option<DealSnapshot> {
        self.deal_engine.deal_snapshot(deal_id)
    }

    /// Returns a clone of the repair manager.
    #[must_use]
    pub fn repair_manager(&self) -> RepairManager {
        self.repair.clone()
    }

    /// Enqueue a repair report submitted by an auditor.
    pub fn enqueue_repair_report(
        &self,
        report: &RepairReportV1,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        self.repair.enqueue_report(report.clone())
    }

    /// Fetch repair tasks with optional filters applied.
    #[must_use]
    pub fn repair_tasks(&self, filters: RepairTaskFilters) -> Vec<RepairTaskRecordV1> {
        self.repair.list_tasks(filters)
    }

    /// Fetch repair task snapshots with optional filters applied.
    #[must_use]
    pub fn repair_task_snapshots(&self, filters: RepairTaskFilters) -> Vec<RepairTaskSnapshot> {
        self.repair.list_task_snapshots(filters)
    }

    /// Fetch repair tasks associated with the supplied manifest digest.
    #[must_use]
    pub fn repair_tasks_for_manifest(&self, manifest_digest: &[u8; 32]) -> Vec<RepairTaskRecordV1> {
        self.repair_tasks(RepairTaskFilters::for_manifest(*manifest_digest))
    }

    /// Fetch repair task snapshots associated with the supplied manifest digest.
    #[must_use]
    pub fn repair_task_snapshots_for_manifest(
        &self,
        manifest_digest: &[u8; 32],
    ) -> Vec<RepairTaskSnapshot> {
        self.repair_task_snapshots(RepairTaskFilters::for_manifest(*manifest_digest))
    }

    /// Fetch a repair task record by ticket id.
    #[must_use]
    pub fn repair_task_record(&self, ticket_id: &RepairTicketId) -> Option<RepairTaskRecordV1> {
        self.repair.task_record(ticket_id)
    }

    /// Fetch a repair task snapshot by ticket id.
    #[must_use]
    pub fn repair_task_snapshot(&self, ticket_id: &RepairTicketId) -> Option<RepairTaskSnapshot> {
        self.repair.task_snapshot(ticket_id)
    }

    /// Submit a slash proposal tied to an escalated repair ticket.
    pub fn submit_repair_slash_proposal(
        &self,
        proposal: &RepairSlashProposalV1,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        self.repair.submit_slash_proposal(proposal.clone())
    }

    /// Mark the specified repair ticket as in progress.
    pub fn mark_repair_in_progress(
        &self,
        ticket_id: &RepairTicketId,
        started_at_unix: u64,
        repair_agent: Option<String>,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        self.repair
            .mark_in_progress(ticket_id, started_at_unix, repair_agent)
    }

    /// Mark the specified repair ticket as completed.
    pub fn mark_repair_completed(
        &self,
        ticket_id: &RepairTicketId,
        completed_at_unix: u64,
        resolution_notes: Option<String>,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        self.repair
            .mark_completed(ticket_id, completed_at_unix, resolution_notes)
    }

    /// Mark the specified repair ticket as failed.
    pub fn mark_repair_failed(
        &self,
        ticket_id: &RepairTicketId,
        failed_at_unix: u64,
        reason: String,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        self.repair.mark_failed(ticket_id, failed_at_unix, reason)
    }

    /// Claim a repair ticket for a worker.
    pub fn claim_repair_ticket(
        &self,
        ticket_id: &RepairTicketId,
        worker_id: &str,
        claimed_at_unix: u64,
        idempotency_key: &str,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        self.repair
            .claim_ticket(ticket_id, worker_id, claimed_at_unix, idempotency_key)
    }

    /// Record a heartbeat for an active repair lease.
    pub fn heartbeat_repair_ticket(
        &self,
        ticket_id: &RepairTicketId,
        worker_id: &str,
        heartbeat_at_unix: u64,
        idempotency_key: &str,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        self.repair
            .heartbeat_ticket(ticket_id, worker_id, heartbeat_at_unix, idempotency_key)
    }

    /// Mark a claimed repair ticket as completed.
    pub fn complete_repair_ticket(
        &self,
        ticket_id: &RepairTicketId,
        worker_id: &str,
        completed_at_unix: u64,
        resolution_notes: Option<String>,
        idempotency_key: &str,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        self.repair.complete_ticket(
            ticket_id,
            worker_id,
            completed_at_unix,
            resolution_notes,
            idempotency_key,
        )
    }

    /// Mark a claimed repair ticket as failed.
    pub fn fail_repair_ticket(
        &self,
        ticket_id: &RepairTicketId,
        worker_id: &str,
        failed_at_unix: u64,
        reason: String,
        idempotency_key: &str,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        self.repair.fail_ticket(
            ticket_id,
            worker_id,
            failed_at_unix,
            reason,
            idempotency_key,
        )
    }

    /// Whether the storage worker is currently enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.enabled()
    }

    /// Record a capacity declaration captured by Torii.
    pub fn record_capacity_declaration(
        &self,
        record: &CapacityDeclarationRecord,
    ) -> Result<(), CapacityError> {
        self.capacity.record_declaration(record)?;
        let window = DeclarationWindow {
            registered_epoch: record.registered_epoch,
            valid_from_epoch: record.valid_from_epoch,
            valid_until_epoch: record.valid_until_epoch,
        };
        self.meter
            .reset_for_declaration(record.committed_capacity_gib, window);
        self.seed_telemetry_accumulator(record);
        Ok(())
    }

    /// Return a snapshot describing the currently tracked capacity usage.
    #[must_use]
    pub fn capacity_usage(&self) -> CapacityUsageSnapshot {
        self.capacity.usage_snapshot()
    }

    /// Schedule a replication order if the active declaration matches the provider.
    pub fn schedule_replication_order(
        &self,
        order: &ReplicationOrderV1,
    ) -> Result<Option<ReplicationPlan>, CapacityError> {
        let maybe_plan = self.capacity.schedule_order(order)?;
        if let Some(plan) = maybe_plan.as_ref() {
            self.meter.on_order_scheduled(plan);
        }
        Ok(maybe_plan)
    }

    /// Mark a replication order as completed and release its reserved capacity.
    pub fn complete_replication_order(
        &self,
        order_id: [u8; 32],
    ) -> Result<ReplicationRelease, CapacityError> {
        let release = self.capacity.complete_order(order_id)?;
        let usage_sample = self.meter.on_order_completed(&release);
        self.record_replication_success(order_id, usage_sample);
        Ok(release)
    }

    /// Expose the underlying capacity manager for advanced integrations.
    #[must_use]
    pub fn capacity_manager(&self) -> Arc<CapacityManager> {
        self.capacity.clone()
    }

    /// Expose the capacity meter so callers can populate telemetry windows.
    #[must_use]
    pub fn capacity_meter(&self) -> CapacityMeter {
        self.meter.clone()
    }

    /// Access the telemetry accumulator backing the current declaration.
    #[must_use]
    pub fn telemetry_handle(&self) -> Arc<RwLock<Option<TelemetryAccumulator>>> {
        self.telemetry.clone()
    }

    /// Expose the scheduler runtime for telemetry and queue inspection.
    #[must_use]
    pub fn schedulers(&self) -> StorageSchedulersRuntime {
        self.schedulers.clone()
    }

    /// Record a governance-issued PoR challenge.
    pub fn record_por_challenge(&self, challenge: &PorChallengeV1) -> Result<(), PorTrackerError> {
        self.por.record_challenge(challenge)
    }

    /// Record a provider PoR proof response.
    pub fn record_por_proof(&self, proof: &PorProofV1) -> Result<(), PorTrackerError> {
        self.por.record_proof(proof)
    }

    /// Record an audit verdict and update telemetry counters accordingly.
    pub fn record_por_verdict(
        &self,
        verdict: &AuditVerdictV1,
    ) -> Result<PorVerdictOutcome, PorTrackerError> {
        let stats = self.por.record_verdict(verdict)?;
        if stats.success_samples > 0 {
            self.meter.record_por_samples(stats.success_samples, 0);
        }
        if stats.failed_samples > 0 {
            self.meter.record_por_samples(0, stats.failed_samples);
        }
        self.schedulers
            .record_por_samples(stats.success_samples, stats.failed_samples);
        let consecutive_failures = self.update_por_history_entry(verdict);
        let slash = self.evaluate_por_penalty(verdict, &stats, consecutive_failures);
        let repair_history_id = self
            .repair
            .register_por_verdict(verdict, stats.failed_samples);
        Ok(PorVerdictOutcome {
            stats,
            repair_history_id,
            consecutive_failures: slash.as_ref().map_or(consecutive_failures, |_| 0),
            slash,
        })
    }

    /// Attach stripe layout and chunk-role metadata to a stored manifest.
    pub fn attach_stripe_layout(
        &self,
        manifest_id: &str,
        stripe_layout: DaStripeLayout,
        chunk_roles: Vec<ChunkRoleMetadata>,
    ) -> Result<(), NodeStorageError> {
        let storage = self.storage.as_ref().ok_or(NodeStorageError::Disabled)?;
        storage.attach_stripe_layout(manifest_id, stripe_layout, chunk_roles)?;
        Ok(())
    }

    /// Generate PoR challenges for all stored manifests using the supplied randomness inputs.
    pub fn plan_por_challenges(
        &self,
        randomness: PorRandomness,
        vrf_records: &HashMap<[u8; 32], ManifestVrfBundle>,
    ) -> Result<Vec<PlannedChallenge>, PorChallengePlannerError> {
        let storage = self
            .storage
            .as_ref()
            .ok_or(PorChallengePlannerError::StorageDisabled)?;
        let usage = self.capacity_usage();
        let provider_id = usage
            .provider_id
            .ok_or(PorChallengePlannerError::ProviderUnavailable)?;
        let sample_policy = por::PorSamplePolicy::from_metadata(provider_id, &usage.metadata)?;

        let manifests = storage.manifests();
        let mut challenges = Vec::with_capacity(manifests.len());

        for manifest in manifests {
            let digest = *manifest.manifest_digest();
            let vrf = vrf_records.get(&digest);
            let planned = build_por_challenge_for_manifest(
                &manifest,
                provider_id,
                &randomness,
                vrf,
                &sample_policy,
            )?;
            challenges.push(planned);
        }

        Ok(challenges)
    }

    /// Record a PoTR receipt captured by the gateway.
    pub fn record_potr_receipt(
        &self,
        receipt: PotrReceiptV1,
    ) -> Result<(), PotrReceiptValidationError> {
        receipt.validate()?;
        self.potr.record_receipt(receipt);
        Ok(())
    }

    /// Retrieve PoTR receipts matching the manifest/provider filters.
    #[must_use]
    pub fn potr_receipts(
        &self,
        manifest_digest: &[u8; 32],
        provider_id: &[u8; 32],
        tier: Option<ProofStreamTier>,
    ) -> Vec<PotrReceiptV1> {
        self.potr.receipts_for(manifest_digest, provider_id, tier)
    }

    /// Returns a clone of the persistent storage backend when enabled.
    #[must_use]
    pub fn storage(&self) -> Option<Arc<StorageBackend>> {
        self.storage.clone()
    }

    /// Ingest a manifest payload into the local storage backend.
    pub fn ingest_manifest<R: Read>(
        &self,
        manifest: &ManifestV1,
        plan: &CarBuildPlan,
        reader: &mut R,
    ) -> Result<String, NodeStorageError> {
        let layout = Self::derive_layout_and_roles(plan);
        let (stripe_layout, chunk_roles) = match layout {
            Some((layout, roles)) => (Some(layout), Some(roles)),
            None => (None, None),
        };
        self.ingest_manifest_with_layout(manifest, plan, reader, stripe_layout, chunk_roles)
    }

    /// Ingest a manifest payload with optional stripe layout and chunk-role annotations.
    pub fn ingest_manifest_with_layout<R: Read>(
        &self,
        manifest: &ManifestV1,
        plan: &CarBuildPlan,
        reader: &mut R,
        stripe_layout: Option<DaStripeLayout>,
        chunk_roles: Option<Vec<ChunkRoleMetadata>>,
    ) -> Result<String, NodeStorageError> {
        let storage = self.storage_backend()?;
        let result = self.schedulers.with_pin(|| {
            storage.ingest_manifest_with_layout(manifest, plan, reader, stripe_layout, chunk_roles)
        });
        match result {
            Ok(manifest_id) => {
                self.schedulers.update_storage_bytes(
                    storage.total_bytes(),
                    self.config.max_capacity_bytes().0,
                );
                Ok(manifest_id)
            }
            Err(err) => Err(NodeStorageError::from(err)),
        }
    }

    /// Read a byte range from a stored manifest payload.
    pub fn read_payload_range(
        &self,
        manifest_id: &str,
        offset: u64,
        len: usize,
    ) -> Result<Vec<u8>, NodeStorageError> {
        let storage = self.storage_backend()?;
        self.schedulers
            .run_fetch(len as u64, None, || {
                storage.read_payload_range(manifest_id, offset, len)
            })
            .map_err(NodeStorageError::from)
    }

    /// Retrieve stored manifest metadata.
    pub fn manifest_metadata(&self, manifest_id: &str) -> Result<StoredManifest, NodeStorageError> {
        let storage = self.storage_backend()?;
        storage.manifest(manifest_id).ok_or_else(|| {
            NodeStorageError::from(StorageError::ManifestNotFound {
                manifest_id: manifest_id.to_owned(),
            })
        })
    }

    /// Retrieve stored manifest metadata by digest.
    pub fn manifest_metadata_by_digest(
        &self,
        digest: &[u8; 32],
    ) -> Result<StoredManifest, NodeStorageError> {
        let storage = self.storage_backend()?;
        storage.manifest_by_digest(digest).ok_or_else(|| {
            NodeStorageError::from(StorageError::ManifestNotFound {
                manifest_id: hex::encode(digest),
            })
        })
    }

    fn derive_layout_and_roles(
        plan: &CarBuildPlan,
    ) -> Option<(DaStripeLayout, Vec<ChunkRoleMetadata>)> {
        if plan.chunks.is_empty() {
            return None;
        }
        let layout = DaStripeLayout {
            total_stripes: 1,
            shards_per_stripe: plan.chunks.len() as u32,
            row_parity_stripes: 0,
        };
        let roles = plan
            .chunks
            .iter()
            .enumerate()
            .map(|(idx, _)| ChunkRoleMetadata {
                role: iroha_data_model::da::manifest::ChunkRole::Data,
                group_id: idx as u32,
            })
            .collect();
        Some((layout, roles))
    }

    /// Retrieve chunk metadata identified by its digest.
    pub fn chunk_by_digest(
        &self,
        manifest_id: &str,
        digest: &[u8; 32],
    ) -> Result<ChunkFileRecord, NodeStorageError> {
        let storage = self.storage_backend()?;
        storage
            .chunk_by_digest(manifest_id, digest)
            .map_err(NodeStorageError::from)
    }

    /// Read chunk bytes identified by digest, returning metadata alongside the payload.
    pub fn read_chunk_by_digest(
        &self,
        manifest_id: &str,
        digest: &[u8; 32],
    ) -> Result<(ChunkFileRecord, Vec<u8>), NodeStorageError> {
        let storage = self.storage_backend()?;
        let record = storage
            .chunk_by_digest(manifest_id, digest)
            .map_err(NodeStorageError::from)?;
        let bytes = self
            .schedulers
            .run_fetch(record.length as u64, None, || {
                storage.read_chunk(manifest_id, digest)
            })
            .map_err(NodeStorageError::from)?;
        Ok((record, bytes))
    }

    /// Sample Proof-of-Retrievability leaves for a stored manifest.
    pub fn sample_por(
        &self,
        manifest_id: &str,
        count: usize,
        seed: u64,
    ) -> Result<Vec<(usize, PorProof)>, NodeStorageError> {
        let storage = self.storage_backend()?;
        let result = self
            .schedulers
            .with_por(|| storage.sample_por(manifest_id, count, seed));
        match result {
            Ok(samples) => {
                self.schedulers.record_por_samples(samples.len() as u64, 0);
                Ok(samples)
            }
            Err(err) => {
                self.schedulers.record_por_samples(0, count as u64);
                Err(NodeStorageError::from(err))
            }
        }
    }

    /// Produce a snapshot of the current metering counters.
    #[must_use]
    pub fn metering_snapshot(&self) -> MeteringSnapshot {
        self.meter.snapshot()
    }

    /// Record an uptime observation for the current declaration.
    pub fn record_uptime_observation(&self, uptime_secs: u64, observed_secs: u64) {
        let success = uptime_secs >= observed_secs && observed_secs > 0;
        self.meter.record_uptime_sample(success);
        if let Ok(mut guard) = self.telemetry.write()
            && let Some(acc) = guard.as_mut()
        {
            let _ = acc.record_uptime_sample(uptime_secs.min(observed_secs), observed_secs);
        }
    }

    /// Record a proof-of-retrievability observation for the current declaration.
    pub fn record_por_observation(&self, success: bool) {
        self.meter.record_por_sample(success);
        if let Ok(mut guard) = self.telemetry.write()
            && let Some(acc) = guard.as_mut()
        {
            acc.record_por_sample(success);
        }
    }

    /// Return the PoR ingestion status for the supplied manifest digest.
    pub fn por_ingestion_status(
        &self,
        manifest_digest: &[u8; 32],
    ) -> Result<PorIngestionStatus, NodeStorageError> {
        if !self.is_enabled() {
            return Err(NodeStorageError::Disabled);
        }
        let backlog = self.por.backlog_for_manifest(manifest_digest);
        let mut statuses = Self::build_ingestion_status_map(backlog);
        self.apply_por_history(&mut statuses, Some(manifest_digest));
        let mut providers: Vec<_> = statuses.into_values().collect();
        providers.sort_by_key(|entry| entry.provider_id);
        Ok(PorIngestionStatus {
            manifest_digest: *manifest_digest,
            providers,
        })
    }

    /// Return the PoR ingestion status for all manifests tracked by the node.
    #[must_use]
    pub fn por_ingestion_overview(&self) -> Vec<PorIngestionProviderStatus> {
        if !self.is_enabled() {
            return Vec::new();
        }
        let backlog = self.por.backlog_entries();
        let mut statuses = Self::build_ingestion_status_map(backlog);
        self.apply_por_history(&mut statuses, None);
        let mut entries: Vec<_> = statuses.into_values().collect();
        entries.sort_by(|left, right| {
            left.manifest_digest
                .cmp(&right.manifest_digest)
                .then(left.provider_id.cmp(&right.provider_id))
        });
        entries
    }

    /// Record that a replication order ultimately failed and release the reservation.
    ///
    /// Returns `true` when the order was tracked and the failure counters were updated.
    pub fn record_replication_failure(&self, order_id: [u8; 32]) -> bool {
        let Some(sample) = self.meter.record_replication_failure(order_id) else {
            return false;
        };
        if let Ok(mut guard) = self.telemetry.write()
            && let Some(acc) = guard.as_mut()
        {
            acc.record_replication_failure();
            if sample.slice_gib > 0 && sample.duration_secs > 0 {
                let _ = acc.record_utilisation(sample.slice_gib, sample.duration_secs);
            }
        }
        true
    }

    /// Build the current Norito telemetry payload, if the accumulator is initialised.
    pub fn build_capacity_telemetry(&self) -> Option<Result<CapacityTelemetryV1, TelemetryError>> {
        let guard = self.telemetry.read().ok()?;
        guard.as_ref().map(TelemetryAccumulator::build_payload)
    }

    /// Mutate the telemetry accumulator if it has been initialised.
    pub fn update_telemetry<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut TelemetryAccumulator) -> R,
    {
        let mut guard = self.telemetry.write().ok()?;
        let acc = guard.as_mut()?;
        Some(f(acc))
    }

    fn update_por_history_entry(&self, verdict: &AuditVerdictV1) -> u64 {
        let mut history = self.por_history.write().expect("por history poisoned");
        let entry = history
            .entry((verdict.manifest_digest, verdict.provider_id))
            .or_default();
        match verdict.outcome {
            AuditOutcomeV1::Success | AuditOutcomeV1::Repaired => {
                entry.last_success_unix = Some(verdict.decided_at);
                entry.consecutive_failures = 0;
            }
            AuditOutcomeV1::Failed => {
                entry.last_failure_unix = Some(verdict.decided_at);
                entry.failures_total = entry.failures_total.saturating_add(1);
                entry.consecutive_failures = entry.consecutive_failures.saturating_add(1);
            }
        }
        entry.consecutive_failures
    }

    fn evaluate_por_penalty(
        &self,
        verdict: &AuditVerdictV1,
        stats: &PorVerdictStats,
        consecutive_failures: u64,
    ) -> Option<SlashRecommendation> {
        if stats.failed_samples == 0 {
            return None;
        }
        let policy = self.config.penalty();
        if consecutive_failures < u64::from(policy.strike_threshold) {
            return None;
        }

        let mut history = self.por_history.write().expect("por history poisoned");
        let entry = history
            .entry((verdict.manifest_digest, verdict.provider_id))
            .or_default();
        if let Some(last_slash) = entry.last_slash_unix {
            let elapsed = verdict.decided_at.saturating_sub(last_slash);
            if elapsed < policy.cooldown_secs {
                return None;
            }
        }

        let provider_id = ProviderId::new(verdict.provider_id);
        let snapshot = self.deal_engine.provider_snapshot(provider_id)?;
        let bonded_total = snapshot
            .bond_available_nano
            .saturating_add(snapshot.bond_locked_nano);
        let penalty = bonded_total
            .saturating_mul(u128::from(policy.penalty_bond_bps))
            .saturating_div(10_000);
        if penalty == 0 {
            return None;
        }

        entry.last_slash_unix = Some(verdict.decided_at);
        entry.consecutive_failures = 0;

        Some(SlashRecommendation {
            provider_id,
            manifest_digest: verdict.manifest_digest,
            penalty_nano: penalty,
            strikes: policy.strike_threshold,
            reason: format!(
                "PoR failure streak reached {} (threshold {}), slashing {} bps of bonded collateral",
                consecutive_failures, policy.strike_threshold, policy.penalty_bond_bps
            ),
        })
    }

    fn build_ingestion_status_map(
        backlog: Vec<por::PorBacklogEntry>,
    ) -> HashMap<PorHistoryKey, PorIngestionProviderStatus> {
        let mut statuses = HashMap::new();
        for entry in backlog {
            let slot = Self::ensure_ingestion_entry(
                &mut statuses,
                entry.manifest_digest,
                entry.provider_id,
            );
            slot.pending_challenges = entry.pending_challenges;
            slot.oldest_epoch_id = entry.oldest_epoch_id;
            slot.oldest_response_deadline_unix = entry.oldest_response_deadline_unix;
        }
        statuses
    }

    fn apply_por_history(
        &self,
        statuses: &mut HashMap<PorHistoryKey, PorIngestionProviderStatus>,
        manifest_filter: Option<&[u8; 32]>,
    ) {
        let history = self.por_history.read().expect("por history poisoned");
        for ((manifest, provider), entry) in history.iter() {
            if manifest_filter.is_some_and(|filter| manifest != filter) {
                continue;
            }
            let slot = Self::ensure_ingestion_entry(statuses, *manifest, *provider);
            slot.last_success_unix = entry.last_success_unix;
            slot.last_failure_unix = entry.last_failure_unix;
            slot.failures_total = entry.failures_total;
            slot.consecutive_failures = entry.consecutive_failures;
        }
    }

    fn ensure_ingestion_entry(
        statuses: &mut HashMap<PorHistoryKey, PorIngestionProviderStatus>,
        manifest_digest: [u8; 32],
        provider_id: [u8; 32],
    ) -> &mut PorIngestionProviderStatus {
        statuses
            .entry((manifest_digest, provider_id))
            .or_insert_with(|| PorIngestionProviderStatus {
                manifest_digest,
                provider_id,
                pending_challenges: 0,
                oldest_epoch_id: None,
                oldest_response_deadline_unix: None,
                last_success_unix: None,
                last_failure_unix: None,
                failures_total: 0,
                consecutive_failures: 0,
            })
    }

    fn storage_backend(&self) -> Result<&StorageBackend, NodeStorageError> {
        self.storage
            .as_ref()
            .map(|arc| arc.as_ref())
            .ok_or(NodeStorageError::Disabled)
    }

    fn seed_telemetry_accumulator(&self, record: &CapacityDeclarationRecord) {
        let mut provider_bytes = [0_u8; 32];
        provider_bytes.copy_from_slice(record.provider_id.as_bytes());
        let mut accumulator = TelemetryAccumulator::new(
            provider_bytes,
            record.committed_capacity_gib,
            record.valid_from_epoch,
        );

        let window_end = if record.valid_until_epoch <= record.valid_from_epoch {
            record.valid_from_epoch.saturating_add(1)
        } else {
            record.valid_until_epoch
        };
        let _ = accumulator.set_window_end_epoch(window_end);

        if let Ok(mut guard) = self.telemetry.write() {
            *guard = Some(accumulator);
        }
    }

    fn record_replication_success(&self, _order_id: [u8; 32], usage: ReplicationUsageSample) {
        if let Ok(mut guard) = self.telemetry.write()
            && let Some(acc) = guard.as_mut()
        {
            acc.record_replication_success();
            if usage.slice_gib > 0 && usage.duration_secs > 0 {
                let _ = acc.record_utilisation(usage.slice_gib, usage.duration_secs);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        str::FromStr,
        sync::{Arc, Mutex},
    };

    use iroha_data_model::{
        metadata::Metadata,
        name::Name,
        sorafs::{
            capacity::{CapacityDeclarationRecord, ProviderId},
            deal::{
                BYTES_PER_GIB, ClientId, DealProposal, DealStatus, DealTerms, DealUsageReport,
                GIB_HOURS_PER_MONTH, MicropaymentTicket, TicketId,
            },
            pin_registry::StorageClass,
        },
    };
    use norito::{codec::Decode, to_bytes};
    use sorafs_car::CarBuildPlan;
    use sorafs_manifest::{
        DagCodecId, ManifestBuilder, PinPolicy,
        capacity::{
            CAPACITY_DECLARATION_VERSION_V1, CapacityDeclarationV1, CapacityMetadataEntry,
            ChunkerCommitmentV1, LaneCommitmentV1, REPLICATION_ORDER_VERSION_V1,
            ReplicationAssignmentV1, ReplicationOrderSlaV1, ReplicationOrderV1,
        },
        deal::{DealSettlementStatusV1, DealSettlementV1},
        repair::{
            CompletedRepairStateV1, EscalatedRepairStateV1, FailedRepairStateV1,
            InProgressRepairStateV1, QueuedRepairStateV1, REPAIR_EVIDENCE_VERSION_V1,
            REPAIR_REPORT_VERSION_V1, REPAIR_SLASH_PROPOSAL_VERSION_V1, RepairCauseV1,
            RepairEvidenceV1, RepairReportV1, RepairSlashProposalV1, RepairTaskStateV1,
            RepairTicketId,
        },
    };
    use tempfile::TempDir;

    use super::*;
    use crate::por::test_support::{
        sample_challenge as por_sample_challenge, sample_proof as por_sample_proof,
        sample_verdict as por_sample_verdict,
    };

    fn storage_config_with_temp_dir() -> (StorageConfig, TempDir) {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(temp_dir.path().join("storage"))
            .build();
        (cfg, temp_dir)
    }

    #[test]
    fn manifest_metadata_resolves_by_digest() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let payload = b"digest-lookup-fixture";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let manifest = ManifestBuilder::new()
            .root_cid(vec![0xAA; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = payload.as_slice();
        let manifest_id = handle
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest manifest");
        let manifest_digest: [u8; 32] = manifest.digest().expect("manifest digest").into();

        let by_id = handle
            .manifest_metadata(&manifest_id)
            .expect("lookup by id");
        let by_digest = handle
            .manifest_metadata_by_digest(&manifest_digest)
            .expect("lookup by digest");

        assert_eq!(by_digest.manifest_id(), manifest_id);
        assert_eq!(by_digest.manifest_digest(), &manifest_digest);
        assert_eq!(by_id.manifest_digest(), by_digest.manifest_digest());
    }

    #[test]
    fn node_handle_registers_and_settles_deal() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let provider_id = ProviderId::new([0xDD; 32]);
        let client_id = ClientId::new([0xCC; 32]);

        handle.deposit_provider_bond(provider_id, 3_000_000_000);
        handle.deposit_client_credit(client_id, 1_000_000_000);

        let terms = DealTerms {
            storage_price_nano_per_gib_month: 200_000_000,
            egress_price_nano_per_gib: 50_000_000,
            settlement_window_epochs: 7,
            micropayment_probability_bps: 10_000,
            micropayment_payout_nano: 50_000_000,
        };

        let activation_epoch = 1_700_000_000;
        let proposal = DealProposal {
            provider_id,
            client_id,
            storage_class: StorageClass::Hot,
            capacity_gib: 4,
            start_epoch: activation_epoch,
            end_epoch: activation_epoch + 14,
            terms,
            metadata: Metadata::default(),
        };

        let record = handle
            .open_deal(proposal, activation_epoch)
            .expect("open deal");

        let usage = DealUsageReport {
            deal_id: record.deal_id,
            epoch: activation_epoch + 1,
            storage_gib_hours: (4u128 * GIB_HOURS_PER_MONTH) as u64,
            egress_bytes: BYTES_PER_GIB as u64,
            tickets: vec![
                MicropaymentTicket {
                    ticket_id: TicketId([1; 32]),
                    issued_epoch: activation_epoch + 1,
                    storage_gib_hours: 0,
                    egress_bytes: 0,
                },
                MicropaymentTicket {
                    ticket_id: TicketId([2; 32]),
                    issued_epoch: activation_epoch + 1,
                    storage_gib_hours: 0,
                    egress_bytes: 0,
                },
                MicropaymentTicket {
                    ticket_id: TicketId([3; 32]),
                    issued_epoch: activation_epoch + 1,
                    storage_gib_hours: 0,
                    egress_bytes: 0,
                },
                MicropaymentTicket {
                    ticket_id: TicketId([4; 32]),
                    issued_epoch: activation_epoch + 1,
                    storage_gib_hours: 0,
                    egress_bytes: 0,
                },
                MicropaymentTicket {
                    ticket_id: TicketId([5; 32]),
                    issued_epoch: activation_epoch + 1,
                    storage_gib_hours: 0,
                    egress_bytes: 0,
                },
            ],
        };

        let usage_outcome = handle.record_deal_usage(usage).expect("record usage");
        assert_eq!(usage_outcome.tickets_processed, 5);

        let outcome = handle
            .settle_deal(record.deal_id, activation_epoch + 7)
            .expect("settle deal");
        let settlement = &outcome.record;
        assert_eq!(settlement.provider_id, provider_id);
        assert_eq!(settlement.client_id, client_id);
        assert_eq!(settlement.deal_id, record.deal_id);
        assert_eq!(settlement.settlement_index, 1);
        assert_eq!(settlement.expected_charge_nano, 850_000_000);
        assert_eq!(settlement.micropayment_credit_nano, 250_000_000);
        assert_eq!(settlement.client_credit_debit_nano, 600_000_000);
        assert_eq!(settlement.bond_slash_nano, 0);
        assert_eq!(settlement.outstanding_nano, 0);

        let governance = &outcome.governance;
        assert_eq!(governance.deal_id, *record.deal_id.as_bytes());
        assert_eq!(governance.status, DealSettlementStatusV1::Completed);
        assert_eq!(governance.settled_at, activation_epoch + 7);
        let ledger = &governance.ledger;
        assert_eq!(ledger.deal_id, *record.deal_id.as_bytes());
        assert_eq!(ledger.provider_id, *provider_id.as_bytes());
        assert_eq!(ledger.client_id, *client_id.as_bytes());
        assert_eq!(ledger.provider_accrual.as_micro(), 850_000);
        assert_eq!(ledger.client_liability.as_micro(), 850_000);
        assert_eq!(ledger.bond_locked.as_micro(), 2_400_000);
        assert_eq!(ledger.bond_slashed.as_micro(), 0);
        assert_eq!(ledger.captured_at, activation_epoch + 7);

        let snapshot = handle.deal_snapshot(record.deal_id).expect("snapshot");
        assert!(matches!(snapshot.status, DealStatus::Active(_)));

        let provider_snapshot = handle
            .deal_engine()
            .provider_snapshot(provider_id)
            .expect("provider snapshot");
        assert!(provider_snapshot.bond_locked_nano >= 2_400_000_000);
    }

    #[test]
    fn settle_deal_publishes_governance_artifact() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);

        let provider_id = ProviderId::new([0xAB; 32]);
        let client_id = ClientId::new([0xBC; 32]);

        handle.deposit_provider_bond(provider_id, 3_000_000_000);
        handle.deposit_client_credit(client_id, 1_000_000_000);

        let terms = DealTerms {
            storage_price_nano_per_gib_month: 200_000_000,
            egress_price_nano_per_gib: 50_000_000,
            settlement_window_epochs: 7,
            micropayment_probability_bps: 10_000,
            micropayment_payout_nano: 50_000_000,
        };

        let activation_epoch = 1_650_000_000;
        let proposal = DealProposal {
            provider_id,
            client_id,
            storage_class: StorageClass::Hot,
            capacity_gib: 2,
            start_epoch: activation_epoch,
            end_epoch: activation_epoch + 14,
            terms,
            metadata: Metadata::default(),
        };

        let record = handle
            .open_deal(proposal, activation_epoch)
            .expect("open deal");

        let usage = DealUsageReport {
            deal_id: record.deal_id,
            epoch: activation_epoch + 1,
            storage_gib_hours: GIB_HOURS_PER_MONTH as u64,
            egress_bytes: BYTES_PER_GIB as u64,
            tickets: vec![],
        };

        handle
            .record_deal_usage(usage)
            .expect("record usage should succeed");

        let outcome = handle
            .settle_deal(record.deal_id, activation_epoch + 7)
            .expect("settlement succeeds");

        let published = publisher.take();
        assert_eq!(published.len(), 1, "expected one governance publish");
        let mut cursor = &published[0][..];
        let decoded = DealSettlementV1::decode(&mut cursor).expect("governance payload decodes");
        assert_eq!(decoded.deal_id, *record.deal_id.as_bytes());
        assert_eq!(decoded.ledger.provider_id, *provider_id.as_bytes());
        assert_eq!(decoded.ledger.client_id, *client_id.as_bytes());
        assert_eq!(decoded.status, outcome.governance.status);
        assert_eq!(decoded.settled_at, outcome.governance.settled_at);
    }

    #[test]
    fn settle_deal_writes_filesystem_governance_payloads() {
        let temp = tempfile::tempdir().expect("temp dir");
        let governance_dir = temp.path().join("governance");
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(temp.path().join("storage"))
            .governance_dir(Some(governance_dir.clone()))
            .build();
        let handle = NodeHandle::new(cfg);

        let provider_id = ProviderId::new([0x10; 32]);
        let client_id = ClientId::new([0x20; 32]);

        handle.deposit_provider_bond(provider_id, 1_000_000_000);
        handle.deposit_client_credit(client_id, 1_000_000_000);

        let terms = DealTerms {
            storage_price_nano_per_gib_month: 100_000_000,
            egress_price_nano_per_gib: 25_000_000,
            settlement_window_epochs: 5,
            micropayment_probability_bps: 0,
            micropayment_payout_nano: 0,
        };

        let activation_epoch = 1_680_000_000;
        let proposal = DealProposal {
            provider_id,
            client_id,
            storage_class: StorageClass::Hot,
            capacity_gib: 1,
            start_epoch: activation_epoch,
            end_epoch: activation_epoch + 10,
            terms,
            metadata: Metadata::default(),
        };

        let record = handle
            .open_deal(proposal, activation_epoch)
            .expect("open deal");

        let usage = DealUsageReport {
            deal_id: record.deal_id,
            epoch: activation_epoch + 1,
            storage_gib_hours: (GIB_HOURS_PER_MONTH / 2) as u64,
            egress_bytes: (BYTES_PER_GIB / 4) as u64,
            tickets: vec![],
        };

        handle
            .record_deal_usage(usage)
            .expect("record usage succeeds");

        let outcome = handle
            .settle_deal(record.deal_id, activation_epoch + 5)
            .expect("settlement");

        let deal_hex = hex::encode(record.deal_id.as_bytes());
        let output_dir = governance_dir.join("settlements").join(deal_hex);
        let entries = std::fs::read_dir(&output_dir)
            .expect("settlement artefacts directory exists")
            .map(|entry| entry.expect("dir entry").path())
            .collect::<Vec<_>>();
        assert!(
            entries
                .iter()
                .any(|path| path.extension().map(|ext| ext == "to").unwrap_or(false)),
            "encoded artefact missing"
        );

        let encoded_path = entries
            .iter()
            .find(|path| path.extension().map(|ext| ext == "to").unwrap_or(false))
            .expect("encoded artefact present");
        let mut cursor = &std::fs::read(encoded_path).expect("read encoded artefact")[..];
        let decoded = DealSettlementV1::decode(&mut cursor).expect("decode artefact");
        assert_eq!(decoded.deal_id, *record.deal_id.as_bytes());
        assert_eq!(decoded.status, outcome.governance.status);

        let json_path = entries
            .iter()
            .find(|path| path.extension().map(|ext| ext == "json").unwrap_or(false))
            .expect("json artefact present");
        let json_bytes = std::fs::read(json_path).expect("read json artefact");
        let value: norito::json::Value =
            norito::json::from_slice(&json_bytes).expect("json parses");
        let status = value
            .get("metadata")
            .and_then(|meta| meta.get("status"))
            .and_then(norito::json::Value::as_str)
            .expect("status present");
        assert_eq!(status, "completed");
    }

    #[test]
    fn settlement_publish_failure_is_best_effort() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let publisher = Arc::new(FailingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);

        let provider_id = ProviderId::new([0xDE; 32]);
        let client_id = ClientId::new([0xEF; 32]);

        handle.deposit_provider_bond(provider_id, 3_000_000_000);
        handle.deposit_client_credit(client_id, 1_000_000_000);

        let terms = DealTerms {
            storage_price_nano_per_gib_month: 200_000_000,
            egress_price_nano_per_gib: 50_000_000,
            settlement_window_epochs: 7,
            micropayment_probability_bps: 10_000,
            micropayment_payout_nano: 50_000_000,
        };

        let activation_epoch = 1_700_100_000;
        let proposal = DealProposal {
            provider_id,
            client_id,
            storage_class: StorageClass::Hot,
            capacity_gib: 3,
            start_epoch: activation_epoch,
            end_epoch: activation_epoch + 14,
            terms,
            metadata: Metadata::default(),
        };

        let record = handle
            .open_deal(proposal, activation_epoch)
            .expect("open deal");

        let usage = DealUsageReport {
            deal_id: record.deal_id,
            epoch: activation_epoch + 1,
            storage_gib_hours: (2 * GIB_HOURS_PER_MONTH) as u64,
            egress_bytes: (BYTES_PER_GIB / 2) as u64,
            tickets: vec![],
        };

        handle
            .record_deal_usage(usage)
            .expect("record usage should succeed");

        let outcome = handle
            .settle_deal(record.deal_id, activation_epoch + 7)
            .expect("settlement succeeds despite publish failure");
        assert_eq!(publisher.attempts(), 1);
        assert_eq!(outcome.record.deal_id, record.deal_id);
    }

    #[test]
    fn por_ingestion_status_tracks_backlog_and_history() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let challenge = por_sample_challenge();

        handle
            .record_por_challenge(&challenge)
            .expect("record challenge");

        let initial = handle
            .por_ingestion_status(&challenge.manifest_digest)
            .expect("status before verdict");
        assert_eq!(initial.providers.len(), 1);
        assert_eq!(initial.providers[0].pending_challenges, 1);
        assert_eq!(initial.providers[0].last_success_unix, None);

        let proof = por_sample_proof(&challenge);
        handle
            .record_por_proof(&proof)
            .expect("record proof succeeds");
        let verdict = por_sample_verdict(&challenge, proof.proof_digest());
        handle
            .record_por_verdict(&verdict)
            .expect("record verdict succeeds");

        let after = handle
            .por_ingestion_status(&challenge.manifest_digest)
            .expect("status after verdict");
        assert_eq!(after.providers.len(), 1);
        let provider = &after.providers[0];
        assert_eq!(provider.pending_challenges, 0);
        assert_eq!(provider.last_success_unix, Some(verdict.decided_at));
        assert_eq!(provider.failures_total, 0);
        assert_eq!(provider.consecutive_failures, 0);
    }

    #[test]
    fn por_ingestion_status_tracks_failures() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let challenge = por_sample_challenge();
        handle
            .record_por_challenge(&challenge)
            .expect("record challenge");

        let mut verdict = por_sample_verdict(&challenge, [0; 32]);
        verdict.outcome = AuditOutcomeV1::Failed;
        verdict.failure_reason = Some("timeout".to_string());
        verdict.proof_digest = None;
        handle
            .record_por_verdict(&verdict)
            .expect("record failure verdict");

        let status = handle
            .por_ingestion_status(&challenge.manifest_digest)
            .expect("status after failure");
        assert_eq!(status.providers.len(), 1);
        let provider = &status.providers[0];
        assert_eq!(provider.pending_challenges, 0);
        assert_eq!(provider.failures_total, 1);
        assert_eq!(provider.consecutive_failures, 1);
        assert_eq!(provider.last_failure_unix, Some(verdict.decided_at));
        assert!(provider.last_success_unix.is_none());
    }

    #[test]
    fn por_ingestion_overview_reports_pending_and_failures() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let challenge = por_sample_challenge();
        handle
            .record_por_challenge(&challenge)
            .expect("record challenge");

        let overview = handle.por_ingestion_overview();
        assert_eq!(overview.len(), 1);
        assert_eq!(overview[0].pending_challenges, 1);
        assert_eq!(overview[0].failures_total, 0);

        let mut verdict = por_sample_verdict(&challenge, [0; 32]);
        verdict.outcome = AuditOutcomeV1::Failed;
        verdict.failure_reason = Some("missed".to_string());
        verdict.proof_digest = None;
        handle
            .record_por_verdict(&verdict)
            .expect("record failure verdict");

        let overview_after = handle.por_ingestion_overview();
        assert_eq!(overview_after.len(), 1);
        assert_eq!(overview_after[0].pending_challenges, 0);
        assert_eq!(overview_after[0].failures_total, 1);
        assert_eq!(
            overview_after[0].last_failure_unix,
            Some(verdict.decided_at)
        );
    }

    #[test]
    fn por_failures_trigger_slash_after_threshold() {
        let (base_cfg, _dir) = storage_config_with_temp_dir();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(base_cfg.data_dir().clone())
            .penalty_strike_threshold(2)
            .penalty_bond_bps(5_000)
            .penalty_cooldown_secs(0)
            .build();
        let handle = NodeHandle::new(cfg);
        let challenge = por_sample_challenge();
        let provider = ProviderId::new(challenge.provider_id);

        handle.deposit_provider_bond(provider, 10_000);

        let mut verdict = por_sample_verdict(&challenge, [0; 32]);
        verdict.outcome = AuditOutcomeV1::Failed;
        verdict.failure_reason = Some("proof missing".to_string());
        verdict.proof_digest = None;

        handle
            .record_por_challenge(&challenge)
            .expect("record first challenge");
        let first = handle
            .record_por_verdict(&verdict)
            .expect("record first failure");
        assert_eq!(first.consecutive_failures, 1);
        assert!(first.slash.is_none());

        // Reuse the same challenge identifier after the tracker clears it.
        let mut second_verdict = verdict.clone();
        second_verdict.decided_at += 10;
        handle
            .record_por_challenge(&challenge)
            .expect("record second challenge");
        let second = handle
            .record_por_verdict(&second_verdict)
            .expect("record second failure");

        let slash = second.slash.expect("slash recommendation expected");
        assert_eq!(slash.provider_id, provider);
        assert_eq!(slash.manifest_digest, challenge.manifest_digest);
        assert_eq!(slash.penalty_nano, 5_000);
        assert_eq!(second.consecutive_failures, 0);
    }

    #[test]
    fn por_slash_respects_cooldown() {
        let (base_cfg, _dir) = storage_config_with_temp_dir();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(base_cfg.data_dir().clone())
            .penalty_strike_threshold(1)
            .penalty_bond_bps(10_000)
            .penalty_cooldown_secs(300)
            .build();
        let handle = NodeHandle::new(cfg);
        let challenge = por_sample_challenge();
        let provider = ProviderId::new(challenge.provider_id);
        handle.deposit_provider_bond(provider, 2_000);

        let mut verdict = por_sample_verdict(&challenge, [0; 32]);
        verdict.outcome = AuditOutcomeV1::Failed;
        verdict.failure_reason = Some("timeout".to_string());
        verdict.proof_digest = None;

        handle
            .record_por_challenge(&challenge)
            .expect("record challenge");
        let first = handle.record_por_verdict(&verdict).expect("record verdict");
        assert!(first.slash.is_some());

        // Cooldown prevents an immediate second slash even though the strike threshold is 1.
        let mut later_verdict = verdict.clone();
        later_verdict.decided_at += 120;
        handle
            .record_por_challenge(&challenge)
            .expect("record challenge after cooldown start");
        let second = handle
            .record_por_verdict(&later_verdict)
            .expect("record verdict during cooldown");
        assert!(second.slash.is_none());
        assert_eq!(second.consecutive_failures, 1);
    }

    #[test]
    fn node_handle_manages_repair_queue() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-351".into()),
            auditor_account: "auditor#sora".into(),
            submitted_at_unix: 1_700_100_000,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest: [0x44; 32],
                provider_id: [0x77; 32],
                por_history_id: None,
                cause: RepairCauseV1::PorFailure {
                    challenge_id: [0xAA; 32],
                    failed_samples: 4,
                    proof_digest: None,
                },
                evidence_json: None,
                notes: Some("PoR sample failed twice".into()),
            },
            notes: Some("auto-generated".into()),
        };

        let queued = handle
            .enqueue_repair_report(&report)
            .expect("queue repair report");
        assert!(matches!(
            queued.state,
            RepairTaskStateV1::Queued(QueuedRepairStateV1 { .. })
        ));

        let in_progress = handle
            .mark_repair_in_progress(
                &report.ticket_id,
                report.submitted_at_unix + 45,
                Some("repairer#sora".into()),
            )
            .expect("mark repair in progress");
        assert!(matches!(
            in_progress.state,
            RepairTaskStateV1::InProgress(InProgressRepairStateV1 { .. })
        ));

        let completed = handle
            .mark_repair_completed(
                &report.ticket_id,
                report.submitted_at_unix + 600,
                Some("reseeded manifest".into()),
            )
            .expect("mark repair completed");
        assert!(matches!(
            completed.state,
            RepairTaskStateV1::Completed(CompletedRepairStateV1 { .. })
        ));

        let tasks = handle.repair_tasks_for_manifest(&report.evidence.manifest_digest);
        assert_eq!(tasks.len(), 1);
        let provider_tasks = handle.repair_tasks(RepairTaskFilters {
            provider_id: Some(report.evidence.provider_id),
            ..RepairTaskFilters::default()
        });
        assert_eq!(provider_tasks.len(), 1);

        let escalated_report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-352".into()),
            auditor_account: "auditor#sora".into(),
            submitted_at_unix: 1_700_200_000,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest: [0x45; 32],
                provider_id: [0x88; 32],
                por_history_id: None,
                cause: RepairCauseV1::PorFailure {
                    challenge_id: [0xAB; 32],
                    failed_samples: 6,
                    proof_digest: None,
                },
                evidence_json: None,
                notes: None,
            },
            notes: None,
        };
        handle
            .enqueue_repair_report(&escalated_report)
            .expect("queue second report");

        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: escalated_report.ticket_id.clone(),
            provider_id: escalated_report.evidence.provider_id,
            manifest_digest: escalated_report.evidence.manifest_digest,
            auditor_account: "auditor#sora".into(),
            proposed_penalty_nano: 5_000_000_000,
            submitted_at_unix: escalated_report.submitted_at_unix + 1_200,
            rationale: "Repeated PoR failures without acknowledgement".into(),
        };

        let escalated = handle
            .submit_repair_slash_proposal(&proposal)
            .expect("submit slash proposal");
        assert!(matches!(
            escalated.state,
            RepairTaskStateV1::Escalated(EscalatedRepairStateV1 { .. })
        ));
    }

    #[test]
    fn node_handle_tracks_repair_worker_actions() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-451".into()),
            auditor_account: "auditor#sora".into(),
            submitted_at_unix: 1_700_300_000,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest: [0x10; 32],
                provider_id: [0x20; 32],
                por_history_id: None,
                cause: RepairCauseV1::Manual {
                    reason: "manual".into(),
                },
                evidence_json: None,
                notes: None,
            },
            notes: None,
        };

        handle
            .enqueue_repair_report(&report)
            .expect("queue repair report");
        let claimed = handle
            .claim_repair_ticket(
                &report.ticket_id,
                "worker-1",
                report.submitted_at_unix + 10,
                "claim-1",
            )
            .expect("claim repair ticket");
        assert!(matches!(
            claimed.state,
            RepairTaskStateV1::InProgress(InProgressRepairStateV1 { .. })
        ));

        handle
            .heartbeat_repair_ticket(
                &report.ticket_id,
                "worker-1",
                report.submitted_at_unix + 20,
                "hb-1",
            )
            .expect("heartbeat repair ticket");

        let completed = handle
            .complete_repair_ticket(
                &report.ticket_id,
                "worker-1",
                report.submitted_at_unix + 30,
                Some("repaired".into()),
                "complete-1",
            )
            .expect("complete repair ticket");
        assert!(matches!(
            completed.state,
            RepairTaskStateV1::Completed(CompletedRepairStateV1 { .. })
        ));

        let failed_report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-452".into()),
            auditor_account: "auditor#sora".into(),
            submitted_at_unix: 1_700_400_000,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest: [0x11; 32],
                provider_id: [0x21; 32],
                por_history_id: None,
                cause: RepairCauseV1::Manual {
                    reason: "manual".into(),
                },
                evidence_json: None,
                notes: None,
            },
            notes: None,
        };
        handle
            .enqueue_repair_report(&failed_report)
            .expect("queue second report");
        handle
            .claim_repair_ticket(
                &failed_report.ticket_id,
                "worker-2",
                failed_report.submitted_at_unix + 5,
                "claim-2",
            )
            .expect("claim second ticket");

        let failed = handle
            .fail_repair_ticket(
                &failed_report.ticket_id,
                "worker-2",
                failed_report.submitted_at_unix + 15,
                "retry later".into(),
                "fail-1",
            )
            .expect("fail repair ticket");
        assert!(matches!(
            failed.state,
            RepairTaskStateV1::Failed(FailedRepairStateV1 { .. })
        ));
    }

    #[test]
    fn node_handle_reflects_config() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg.clone());

        assert!(handle.is_enabled());
        let observed = handle.config();
        assert_eq!(observed.enabled(), cfg.enabled());
        assert_eq!(observed.data_dir(), cfg.data_dir());
        assert_eq!(observed.max_capacity_bytes().0, cfg.max_capacity_bytes().0);
        assert_eq!(observed.max_parallel_fetches(), cfg.max_parallel_fetches());
        assert_eq!(observed.max_pins(), cfg.max_pins());
        assert_eq!(
            observed.por_sample_interval_secs(),
            cfg.por_sample_interval_secs()
        );
        assert_eq!(observed.alias(), cfg.alias());
        assert_eq!(observed.adverts().topics(), cfg.adverts().topics());
        assert!(handle.storage().is_some());
    }

    #[test]
    fn node_handle_threads_repair_and_gc_config() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let actual_repair = iroha_config::parameters::actual::SorafsRepair {
            enabled: true,
            claim_ttl_secs: 900,
            heartbeat_interval_secs: 45,
            max_attempts: 6,
            worker_concurrency: 9,
            default_slash_penalty_nano: 42_000,
            ..Default::default()
        };

        let actual_gc = iroha_config::parameters::actual::SorafsGc {
            enabled: true,
            interval_secs: 300,
            max_deletions_per_run: 2_000,
            retention_grace_secs: 86_400,
            pre_admission_sweep: false,
            ..Default::default()
        };

        let repair_cfg = RepairConfig::from(&actual_repair);
        let gc_cfg = GcConfig::from(&actual_gc);
        let handle = NodeHandle::new_with_policies(cfg, repair_cfg.clone(), gc_cfg.clone());

        assert!(handle.repair_config().enabled());
        assert_eq!(handle.repair_config().claim_ttl_secs(), 900);
        assert_eq!(handle.repair_config().heartbeat_interval_secs(), 45);
        assert_eq!(handle.repair_config().max_attempts(), 6);
        assert_eq!(handle.repair_config().worker_concurrency(), 9);
        assert_eq!(handle.repair_config().default_slash_penalty_nano(), 42_000);

        assert!(handle.gc_config().enabled());
        assert_eq!(handle.gc_config().interval_secs(), 300);
        assert_eq!(handle.gc_config().max_deletions_per_run(), 2_000);
        assert_eq!(handle.gc_config().retention_grace_secs(), 86_400);
        assert!(!handle.gc_config().pre_admission_sweep());

        let manager = handle.repair_manager();
        assert_eq!(manager.claim_ttl_secs(), repair_cfg.claim_ttl_secs());
        assert_eq!(
            manager.heartbeat_interval_secs(),
            repair_cfg.heartbeat_interval_secs()
        );
    }

    #[test]
    fn node_handle_records_capacity_declaration() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: [0x11; 32],
            stake: sorafs_manifest::provider_advert::StakePointer {
                pool_id: [0xAA; 32],
                stake_amount: 1,
            },
            committed_capacity_gib: 100,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".into(),
                profile_aliases: None,
                committed_gib: 100,
                capability_refs: Vec::new(),
            }],
            lane_commitments: vec![LaneCommitmentV1 {
                lane_id: "default".into(),
                max_gib: 100,
            }],
            pricing: None,
            valid_from: 1,
            valid_until: 2,
            metadata: vec![],
        };
        let payload = to_bytes(&declaration).expect("encode declaration");
        let record = CapacityDeclarationRecord::new(
            ProviderId::new(declaration.provider_id),
            payload,
            declaration.committed_capacity_gib,
            1,
            1,
            2,
            Metadata::default(),
        );

        handle
            .record_capacity_declaration(&record)
            .expect("record declaration");

        let usage = handle.capacity_usage();
        assert_eq!(usage.provider_id, Some([0x11; 32]));
        assert_eq!(usage.committed_total_gib, 100);
        assert_eq!(usage.available_total_gib, 100);

        let telemetry = handle
            .build_capacity_telemetry()
            .expect("telemetry accumulator present")
            .expect("telemetry payload");
        assert_eq!(telemetry.declared_capacity_gib, 100);
        assert_eq!(telemetry.utilised_capacity_gib, 0);
        assert_eq!(telemetry.successful_replications, 0);
    }

    #[test]
    fn node_handle_completes_replication_order() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: [0x22; 32],
            stake: sorafs_manifest::provider_advert::StakePointer {
                pool_id: [0xAA; 32],
                stake_amount: 1,
            },
            committed_capacity_gib: 200,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".into(),
                profile_aliases: None,
                committed_gib: 200,
                capability_refs: Vec::new(),
            }],
            lane_commitments: vec![LaneCommitmentV1 {
                lane_id: "default".into(),
                max_gib: 200,
            }],
            pricing: None,
            valid_from: 1,
            valid_until: 100,
            metadata: vec![],
        };
        let payload = norito::to_bytes(&declaration).expect("encode declaration");
        let record = CapacityDeclarationRecord::new(
            ProviderId::new(declaration.provider_id),
            payload,
            declaration.committed_capacity_gib,
            1,
            1,
            100,
            Metadata::default(),
        );

        handle
            .record_capacity_declaration(&record)
            .expect("record declaration");

        let order = ReplicationOrderV1 {
            version: sorafs_manifest::capacity::REPLICATION_ORDER_VERSION_V1,
            order_id: [0x99; 32],
            manifest_cid: vec![0x55; 32],
            manifest_digest: [0x77; 32],
            chunking_profile: "sorafs.sf1@1.0.0".into(),
            target_replicas: 1,
            assignments: vec![sorafs_manifest::capacity::ReplicationAssignmentV1 {
                provider_id: [0x22; 32],
                slice_gib: 50,
                lane: Some("default".into()),
            }],
            issued_at: 10,
            deadline_at: 20,
            sla: sorafs_manifest::capacity::ReplicationOrderSlaV1 {
                ingest_deadline_secs: 600,
                min_availability_percent_milli: 99_000,
                min_por_success_percent_milli: 99_000,
            },
            metadata: Vec::new(),
        };

        let plan = handle
            .schedule_replication_order(&order)
            .expect("schedule order")
            .expect("plan produced");
        assert_eq!(plan.assigned_slice_gib, 50);

        let release = handle
            .complete_replication_order(order.order_id)
            .expect("complete order");
        assert_eq!(release.released_gib, 50);
        assert_eq!(release.remaining_total_gib, 200);
    }

    #[test]
    fn node_handle_meter_tracks_replication_flow() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: [0x55; 32],
            stake: sorafs_manifest::provider_advert::StakePointer {
                pool_id: [0xAA; 32],
                stake_amount: 1,
            },
            committed_capacity_gib: 256,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".into(),
                profile_aliases: None,
                committed_gib: 256,
                capability_refs: Vec::new(),
            }],
            lane_commitments: vec![LaneCommitmentV1 {
                lane_id: "default".into(),
                max_gib: 256,
            }],
            pricing: None,
            valid_from: 1,
            valid_until: 10,
            metadata: vec![],
        };
        let payload = norito::to_bytes(&declaration).expect("encode declaration");
        let record = CapacityDeclarationRecord::new(
            ProviderId::new(declaration.provider_id),
            payload,
            declaration.committed_capacity_gib,
            0,
            1,
            10,
            Metadata::default(),
        );
        handle
            .record_capacity_declaration(&record)
            .expect("record declaration");

        let meter = handle.capacity_meter();
        let snapshot = meter.snapshot();
        assert_eq!(snapshot.declared_gib, 256);
        assert_eq!(snapshot.orders_issued, 0);
        assert_eq!(snapshot.outstanding_orders, 0);

        let order = ReplicationOrderV1 {
            version: REPLICATION_ORDER_VERSION_V1,
            order_id: [0x44; 32],
            manifest_cid: vec![0xDE, 0xAD],
            manifest_digest: [0xCD; 32],
            chunking_profile: "sorafs.sf1@1.0.0".into(),
            target_replicas: 1,
            assignments: vec![ReplicationAssignmentV1 {
                provider_id: declaration.provider_id,
                slice_gib: 64,
                lane: Some("default".into()),
            }],
            issued_at: 100,
            deadline_at: 400,
            sla: ReplicationOrderSlaV1 {
                ingest_deadline_secs: 600,
                min_availability_percent_milli: 99_000,
                min_por_success_percent_milli: 98_000,
            },
            metadata: vec![CapacityMetadataEntry {
                key: "priority".into(),
                value: "standard".into(),
            }],
        };

        let plan = handle
            .schedule_replication_order(&order)
            .expect("schedule ok")
            .expect("plan expected");
        assert_eq!(plan.assigned_slice_gib, 64);

        let snapshot_after_schedule = meter.snapshot();
        assert_eq!(snapshot_after_schedule.orders_issued, 1);
        assert_eq!(snapshot_after_schedule.outstanding_orders, 1);
        assert_eq!(snapshot_after_schedule.outstanding_total_gib, 64);

        handle
            .complete_replication_order(order.order_id)
            .expect("complete order");

        let snapshot_after_complete = meter.snapshot();
        assert_eq!(snapshot_after_complete.orders_completed, 1);
        assert_eq!(snapshot_after_complete.utilised_gib, 64);
        assert_eq!(snapshot_after_complete.outstanding_orders, 0);

        handle.update_telemetry(|acc| {
            acc.record_uptime_sample(540, 600).expect("uptime sample");
            acc.record_por_sample(true);
            acc.record_por_sample(false);
        });

        let telemetry = handle
            .build_capacity_telemetry()
            .expect("telemetry accumulator present")
            .expect("payload");
        assert_eq!(telemetry.successful_replications, 1);
        assert_eq!(telemetry.failed_replications, 0);
        assert_eq!(telemetry.uptime_percent_milli, 90_000);
        assert_eq!(telemetry.por_success_percent_milli, 50_000);
    }

    #[test]
    fn node_handle_storage_ingest_and_fetch_range() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let payload = b"node handle storage fetch test";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let manifest = ManifestBuilder::new()
            .root_cid(vec![0xAA; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        let manifest_id = handle
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");

        let bytes = handle
            .read_payload_range(&manifest_id, 5, 6)
            .expect("read range");
        assert_eq!(bytes, b"handle"[..]);
    }

    #[test]
    fn node_handle_storage_sample_por() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let payload = b"SoraFS node handle PoR sampling payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let manifest = ManifestBuilder::new()
            .root_cid(vec![0xBB; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        let manifest_id = handle
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");

        let storage = handle.storage().expect("storage backend");
        let stored = storage.manifest(&manifest_id).expect("stored manifest");
        let expected = stored.por_tree().leaf_count().min(3);

        let samples = handle.sample_por(&manifest_id, 3, 99).expect("sample por");
        assert_eq!(samples.len(), expected);
        let root = *stored.por_tree().root();

        for (_idx, proof) in samples {
            assert!(proof.verify(&root));
        }
    }

    #[test]
    fn node_handle_plan_por_challenges_handles_vrf_and_forced() {
        use std::collections::HashMap;

        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: [0x11; 32],
            stake: sorafs_manifest::provider_advert::StakePointer {
                pool_id: [0xAA; 32],
                stake_amount: 1,
            },
            committed_capacity_gib: 128,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".into(),
                profile_aliases: None,
                committed_gib: 128,
                capability_refs: Vec::new(),
            }],
            lane_commitments: vec![LaneCommitmentV1 {
                lane_id: "default".into(),
                max_gib: 128,
            }],
            pricing: None,
            valid_from: 1,
            valid_until: 2,
            metadata: vec![],
        };
        let payload = to_bytes(&declaration).expect("encode declaration");
        let provider_metadata = {
            let mut metadata = Metadata::default();
            metadata.insert(
                Name::from_str("profile.sample_multiplier").expect("valid metadata key"),
                2u64,
            );
            metadata
        };
        let record = CapacityDeclarationRecord::new(
            ProviderId::new(declaration.provider_id),
            payload,
            declaration.committed_capacity_gib,
            1,
            1,
            2,
            provider_metadata,
        );
        handle
            .record_capacity_declaration(&record)
            .expect("record declaration");

        let payload = vec![0xEE; 128 * 1024];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");
        let manifest = ManifestBuilder::new()
            .root_cid(vec![0xDD; 8])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(&payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        handle
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");

        let randomness = PorRandomness {
            epoch_id: 42,
            issued_at_unix: 1_700_000_000,
            response_window_secs: 900,
            drand_round: 12345,
            drand_randomness: [0x33; 32],
            drand_signature: vec![0x44; 96],
        };

        let plans = handle
            .plan_por_challenges(randomness.clone(), &HashMap::new())
            .expect("forced challenge");
        assert_eq!(plans.len(), 1);
        let forced = &plans[0].challenge;
        assert!(forced.forced);
        assert!(forced.vrf_output.is_none());
        assert!(forced.sample_count > 0);
        assert_eq!(forced.sample_count, 128);

        let mut vrf_records = HashMap::new();
        vrf_records.insert(
            forced.manifest_digest,
            ManifestVrfBundle {
                output: [0x55; 32],
                proof: vec![0x66; 80],
            },
        );

        let plans_with_vrf = handle
            .plan_por_challenges(randomness, &vrf_records)
            .expect("vrf-backed challenge");
        let satisfied = &plans_with_vrf[0].challenge;
        assert!(!satisfied.forced);
        assert_eq!(satisfied.vrf_output, Some([0x55; 32]));
        assert_eq!(satisfied.sample_count, 128);
        assert!(
            satisfied
                .vrf_proof
                .as_ref()
                .is_some_and(|proof| proof.len() == 80)
        );
    }

    #[test]
    fn node_handle_storage_methods_error_when_disabled() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);

        let payload = b"disabled storage payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let manifest = ManifestBuilder::new()
            .root_cid(vec![0xCC; 8])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        let err = handle
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect_err("storage disabled");
        assert!(matches!(err, NodeStorageError::Disabled));
    }

    #[derive(Debug, Default)]
    struct RecordingPublisher {
        payloads: Mutex<Vec<Vec<u8>>>,
    }

    impl RecordingPublisher {
        fn take(&self) -> Vec<Vec<u8>> {
            let mut guard = self.payloads.lock().expect("publisher lock poisoned");
            guard.drain(..).collect()
        }
    }

    impl GovernancePublisher for RecordingPublisher {
        fn publish_deal_settlement(
            &self,
            _settlement: &DealSettlementV1,
            encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.payloads.lock().expect("publisher lock poisoned");
            guard.push(encoded.to_vec());
            Ok(())
        }
    }

    #[derive(Debug, Default)]
    struct FailingPublisher {
        attempts: Mutex<usize>,
    }

    impl FailingPublisher {
        fn attempts(&self) -> usize {
            *self.attempts.lock().expect("publisher lock poisoned")
        }
    }

    impl GovernancePublisher for FailingPublisher {
        fn publish_deal_settlement(
            &self,
            _settlement: &DealSettlementV1,
            _encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.attempts.lock().expect("publisher lock poisoned");
            *guard += 1;
            Err(GovernancePublishError::other("simulated publish failure"))
        }
    }
}
