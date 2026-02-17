//! Module with queue actor
//!
//! Handles transaction admission, TEU accounting, and per-lane telemetry
//! updates. Lane/dataspace routing is delegated to a pluggable router so the
//! queue can expose the actual Nexus assignments instead of single-lane
//! placeholders.

mod router;
pub(crate) mod routing_ledger;

use core::time::Duration;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    num::NonZeroUsize,
    ops::Deref,
    str::FromStr,
    sync::{
        Arc, LazyLock, OnceLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
        mpsc,
    },
};

use crossbeam_queue::ArrayQueue;
use dashmap::{DashMap, mapref::entry::Entry};
use eyre::Result;
use indexmap::IndexSet;
use iroha_config::parameters::actual::{GovernanceCatalog, LaneRegistry, Nexus, Queue as Config};
use iroha_crypto::HashOf;
#[allow(unused_imports)]
use iroha_data_model::nexus::{
    DataSpaceCatalog, DataSpaceId, LaneCatalog, LaneId, LaneLifecyclePlan, LanePrivacyProof,
    LaneStorageProfile, LaneVisibility, UniversalAccountId,
};
use iroha_data_model::{
    Encode,
    account::AccountId,
    events::pipeline::{TransactionEvent, TransactionStatus},
    isi::{
        runtime_upgrade::{ActivateRuntimeUpgrade, CancelRuntimeUpgrade, ProposeRuntimeUpgrade},
        smart_contract_code::{
            ActivateContractInstance, DeactivateContractInstance, RegisterSmartContractBytes,
            RegisterSmartContractCode, RemoveSmartContractBytes,
        },
    },
    name::Name,
    transaction::Executable,
};
use iroha_logger::{trace, warn};
use iroha_primitives::time::TimeSource;
#[cfg(feature = "telemetry")]
use iroha_telemetry::metrics::NexusLaneTeuBuckets;
use ivm::ProgramMetadata;
use mv::storage::StorageReadOnly;
use norito::core::NoritoSerialize;
use parking_lot::RwLock;
pub use router::{
    ConfigLaneRouter, LaneRouter, RoutingDecision, SingleLaneRouter, evaluate_policy,
    evaluate_policy_with_catalog,
};
use thiserror::Error;
use tokio::{
    sync::watch,
    time::{MissedTickBehavior, interval},
};

#[cfg(feature = "telemetry")]
use crate::telemetry::{DataspaceTeuGaugeUpdate, LaneTeuGaugeUpdate};
use crate::{
    EventsSender,
    compliance::{LaneComplianceContext, LaneComplianceEngine, LaneComplianceEvaluation},
    gas,
    governance::manifest::{
        GovernanceGuardError, GovernanceRules, LaneManifestRegistry, LaneManifestRegistryHandle,
    },
    interlane::{LanePrivacyRegistry, LanePrivacyRegistryHandle, verify_lane_privacy_proofs},
    prelude::*,
    state::{LaneLifecycleError, State},
    sumeragi::status,
    telemetry::StateTelemetry,
    tx::CheckedTransaction,
};

type SignedTxHash = HashOf<iroha_data_model::transaction::SignedTransaction>;

/// Nexus-derived limits that influence queue telemetry and scheduling defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaneSchedulingLimits {
    /// TEU capacity per lane for telemetry snapshots.
    pub teu_capacity: u64,
    /// Starvation bound (slots) applied to dataspace metrics.
    pub starvation_bound_slots: u64,
}

impl LaneSchedulingLimits {
    /// Construct scheduling limits from explicit capacity and starvation configuration.
    pub const fn new(teu_capacity: u64, starvation_bound_slots: u64) -> Self {
        Self {
            teu_capacity,
            starvation_bound_slots,
        }
    }
}

/// Nexus-derived limits that influence queue telemetry and scheduling defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueLimits {
    fallback: LaneSchedulingLimits,
    per_lane: BTreeMap<LaneId, LaneSchedulingLimits>,
}

impl QueueLimits {
    /// Build limits using values from the runtime Nexus configuration.
    #[must_use]
    pub fn from_nexus(nexus: &Nexus) -> Self {
        let fallback = LaneSchedulingLimits::new(
            u64::from(nexus.fusion.exit_teu),
            u64::from(nexus.da.rotation.window_slots.get()),
        );
        let mut per_lane = BTreeMap::new();
        if nexus.enabled {
            for lane in nexus.lane_catalog.lanes() {
                let limits = Self::lane_limits_from_metadata(lane, fallback);
                per_lane.insert(lane.id, limits);
            }
        }
        Self { fallback, per_lane }
    }

    /// Derive lane-specific limits from metadata overrides or fall back to the global defaults.
    pub(crate) fn lane_limits_from_metadata(
        lane: &iroha_data_model::nexus::LaneConfig,
        fallback: LaneSchedulingLimits,
    ) -> LaneSchedulingLimits {
        let mut limits = fallback;
        if let Some(raw) = lane.metadata.get("scheduler.teu_capacity") {
            match raw.trim().parse::<u64>() {
                Ok(value) => limits.teu_capacity = value,
                Err(err) => warn!(
                    lane = %lane.alias,
                    value = raw,
                    %err,
                    "invalid scheduler.teu_capacity metadata; using fallback value"
                ),
            }
        }
        if let Some(raw) = lane.metadata.get("scheduler.starvation_bound_slots") {
            match raw.trim().parse::<u64>() {
                Ok(value) => limits.starvation_bound_slots = value,
                Err(err) => warn!(
                    lane = %lane.alias,
                    value = raw,
                    %err,
                    "invalid scheduler.starvation_bound_slots metadata; using fallback value"
                ),
            }
        }
        limits
    }

    /// Return the scheduling limits associated with `lane`, falling back to the global defaults.
    #[must_use]
    pub fn for_lane(&self, lane: LaneId) -> LaneSchedulingLimits {
        self.per_lane.get(&lane).copied().unwrap_or(self.fallback)
    }
}

impl Default for QueueLimits {
    fn default() -> Self {
        let nexus_defaults = Nexus::default();
        let fallback = LaneSchedulingLimits::new(
            u64::from(nexus_defaults.fusion.exit_teu),
            u64::from(nexus_defaults.da.rotation.window_slots.get()),
        );
        Self {
            fallback,
            per_lane: BTreeMap::new(),
        }
    }
}

static GOV_NAMESPACE_METADATA_KEY: LazyLock<Name> =
    LazyLock::new(|| Name::from_str("gov_namespace").expect("static governance metadata key"));
static GOV_CONTRACT_ID_METADATA_KEY: LazyLock<Name> =
    LazyLock::new(|| Name::from_str("gov_contract_id").expect("static governance metadata key"));
static GOV_APPROVERS_METADATA_KEY: LazyLock<Name> = LazyLock::new(|| {
    Name::from_str("gov_manifest_approvers").expect("static governance metadata key")
});
static CONTRACT_NAMESPACE_METADATA_KEY: LazyLock<Name> =
    LazyLock::new(|| Name::from_str("contract_namespace").expect("static contract metadata key"));
static CONTRACT_ID_METADATA_KEY: LazyLock<Name> =
    LazyLock::new(|| Name::from_str("contract_id").expect("static contract metadata key"));

/// Lockfree queue for transactions
///
/// Multiple producers, single consumer
pub struct Queue {
    events_sender: EventsSender,
    /// Resolves lane/dataspace assignments for queued transactions.
    router: RwLock<Arc<dyn LaneRouter>>,
    /// Optional lane compliance engine.
    lane_compliance: RwLock<Option<Arc<crate::compliance::LaneComplianceEngine>>>,
    /// Cached lane catalog for routing/telemetry.
    lane_catalog: RwLock<Arc<LaneCatalog>>,
    /// Cached dataspace catalog for routing/telemetry.
    dataspace_catalog: RwLock<Arc<DataSpaceCatalog>>,
    /// The queue for transactions
    tx_hashes: ArrayQueue<SignedTxHash>,
    /// Accepted transactions addressed by `Hash`.
    /// Stored behind `Arc` to avoid deep cloning heavy transactions
    /// (including instruction payloads) during queue operations.
    txs: DashMap<SignedTxHash, Arc<CheckedTransaction<'static>>>,
    /// Cached routing decision per transaction hash.
    routing_decisions: DashMap<SignedTxHash, RoutingDecision>,
    /// Cached encoded length per queued transaction hash.
    tx_encoded_len: DashMap<SignedTxHash, usize>,
    /// Cached proposal gas cost per queued transaction hash.
    tx_gas_cost: DashMap<SignedTxHash, u64>,
    /// Cached Norito-encoded transaction payloads for gossip retransmit.
    tx_gossip_payloads: DashMap<SignedTxHash, Arc<Vec<u8>>>,
    /// Hashes of transactions removed from `txs` but still present in `tx_hashes`
    removed_hashes: DashMap<SignedTxHash, ()>,
    /// Amount of transactions per user in the queue
    txs_per_user: DashMap<AccountId, usize>,
    /// Lock to synchronize push and remove operations
    push_remove_lock: parking_lot::Mutex<()>,
    /// Monotonic counter tagging guards with their queue order to keep TEU gating fair.
    guard_sequence: AtomicU64,
    /// Active guards holding queued transactions (used to avoid resyncing during in-flight reads).
    inflight_guards: AtomicUsize,
    /// The maximum number of transactions in the queue
    capacity: NonZeroUsize,
    /// The maximum number of transactions in the queue per user. Used to apply throttling
    capacity_per_user: NonZeroUsize,
    /// The time source used to check transaction against
    ///
    /// A mock time source is used in tests for determinism
    time_source: TimeSource,
    /// Length of time after which transactions are dropped.
    pub tx_time_to_live: Duration,
    /// Minimum interval between expired-transaction sweeps.
    expired_cull_interval: Duration,
    /// Maximum number of entries scanned per expired-transaction sweep.
    expired_cull_batch: NonZeroUsize,
    /// Last time (unix ms) we swept expired transactions.
    last_expired_cull_ms: AtomicU64,
    /// Round-robin ring of queued transaction hashes used for TTL sweeps.
    expiry_ring: parking_lot::Mutex<VecDeque<SignedTxHash>>,
    /// Membership guard for the expiry ring to prevent unbounded growth.
    expiry_ring_members: DashMap<SignedTxHash, ()>,
    /// Queue to gossip transactions
    tx_gossip: ArrayQueue<SignedTxHash>,
    /// Broadcast queue load so producers can observe backpressure.
    backpressure_tx: watch::Sender<BackpressureState>,
    /// Optional wake handle for the Sumeragi worker when new transactions are enqueued.
    sumeragi_wake: OnceLock<mpsc::SyncSender<()>>,
    /// Limits derived from Nexus configuration (TEU capacity, starvation bounds).
    nexus_limits: RwLock<QueueLimits>,
    /// Cached TEU metadata for queued transactions keyed by hash.
    #[cfg(feature = "telemetry")]
    tx_teu: DashMap<SignedTxHash, TxTeuInfo>,
    /// Aggregated TEU per lane for queued transactions.
    #[cfg(feature = "telemetry")]
    lane_teu_pending: DashMap<LaneId, PendingTeu>,
    /// Aggregated TEU per (lane, dataspace) for queued transactions.
    #[cfg(feature = "telemetry")]
    dataspace_teu_pending: DashMap<(LaneId, DataSpaceId), PendingTeu>,
    /// Governance manifest registry for Nexus lanes.
    lane_manifests: parking_lot::RwLock<LaneManifestRegistryHandle>,
    /// Privacy commitments advertised by lane manifests.
    lane_privacy_registry: parking_lot::RwLock<LanePrivacyRegistryHandle>,
    #[cfg(test)]
    vacant_entry_warnings: AtomicUsize,
}

impl fmt::Debug for Queue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Queue")
            .field("capacity", &self.capacity)
            .field("capacity_per_user", &self.capacity_per_user)
            .field("tx_time_to_live", &self.tx_time_to_live)
            .field("expired_cull_interval", &self.expired_cull_interval)
            .field("expired_cull_batch", &self.expired_cull_batch)
            .finish_non_exhaustive()
    }
}

/// Snapshot of queue occupancy used to coordinate backpressure across Torii,
/// the gossiper, and consensus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackpressureState {
    /// Queue has room for new transactions.
    Healthy {
        /// Number of transactions tracked by the queue (queued + in-flight).
        queued: usize,
        /// Maximum queue capacity configured for the peer.
        capacity: NonZeroUsize,
    },
    /// Queue reached capacity; callers should defer submissions.
    Saturated {
        /// Number of transactions tracked by the queue when saturation triggered.
        queued: usize,
        /// Maximum queue capacity configured for the peer.
        capacity: NonZeroUsize,
    },
}

impl BackpressureState {
    #[must_use]
    /// Whether the queue snapshot represents a saturated state.
    pub const fn is_saturated(self) -> bool {
        matches!(self, Self::Saturated { .. })
    }

    #[must_use]
    /// Number of transactions tracked by the queue in the snapshot.
    pub const fn queued(self) -> usize {
        match self {
            Self::Healthy { queued, .. } | Self::Saturated { queued, .. } => queued,
        }
    }

    #[must_use]
    /// Queue capacity recorded in the snapshot.
    pub const fn capacity(self) -> NonZeroUsize {
        match self {
            Self::Healthy { capacity, .. } | Self::Saturated { capacity, .. } => capacity,
        }
    }
}

impl Default for BackpressureState {
    fn default() -> Self {
        Self::Healthy {
            queued: 0,
            capacity: NonZeroUsize::new(1).expect("capacity must be non-zero"),
        }
    }
}

/// Gossip payload paired with its routing metadata.
#[derive(Clone)]
pub struct GossipBatchEntry {
    /// Accepted transaction to gossip.
    pub tx: AcceptedTransaction<'static>,
    /// Lane/dataspace routing decision cached at admission time.
    pub routing: RoutingDecision,
    /// Pre-serialized transaction payload for retransmit.
    pub payload: Arc<Vec<u8>>,
}

#[cfg(feature = "telemetry")]
#[derive(Clone, Copy, Debug)]
struct TxTeuInfo {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    teu: u64,
}

#[cfg(feature = "telemetry")]
#[derive(Clone, Copy, Debug, Default)]
struct PendingTeu {
    teu: u64,
    tx_count: usize,
}

const IVM_TEU_FALLBACK: u64 = 5_000;

/// Handle that observers can clone to subscribe to queue backpressure updates.
#[derive(Clone, Debug)]
pub struct BackpressureHandle {
    rx: watch::Receiver<BackpressureState>,
}

impl BackpressureHandle {
    #[must_use]
    /// Subscribe to backpressure state updates.
    pub fn subscribe(&self) -> watch::Receiver<BackpressureState> {
        self.rx.clone()
    }

    #[must_use]
    /// Return the most recent backpressure snapshot without subscribing.
    pub fn snapshot(&self) -> BackpressureState {
        *self.rx.borrow()
    }
}

/// Queue push error
#[derive(Error, Clone, Debug, displaydoc::Display)]
#[allow(variant_size_differences)]
pub enum Error {
    /// Queue is full
    Full,
    /// Transaction expired
    Expired,
    /// Transaction is already applied
    InBlockchain,
    /// User reached maximum number of transactions in the queue
    MaximumTransactionsPerUser,
    /// The transaction is already in the queue
    IsInQueue,
    /// Lane governance manifest is missing or invalid: {0}
    Governance(GovernanceGuardError),
    /// Transaction not permitted by lane governance manifest: {alias} ({reason})
    GovernanceNotPermitted {
        /// Lane alias that triggered the governance rejection.
        alias: String,
        /// Explanation describing why the manifest rejected the transaction.
        reason: String,
    },
    /// Transaction violates lane compliance policy for {alias}: {reason}
    LaneComplianceDenied {
        /// Lane alias configured for the policy.
        alias: String,
        /// Reason describing why the policy denied the transaction.
        reason: String,
    },
    /// Lane privacy proof rejected for {alias}: {reason}
    LanePrivacyProofRejected {
        /// Lane alias configured for the policy.
        alias: String,
        /// Reason describing why the privacy proof failed.
        reason: String,
    },
    /// UAID {uaid} has no active manifest for dataspace {dataspace}
    UaidNotBound {
        /// UAID inferred from the authority account.
        uaid: UniversalAccountId,
        /// Dataspace targeted by the transaction.
        dataspace: DataSpaceId,
    },
}

/// Failure that can pop up when pushing transaction into the queue
#[derive(Debug)]
pub struct Failure {
    /// Transaction failed to be pushed into the queue
    pub tx: Box<AcceptedTransaction<'static>>,
    /// Push failure reason
    pub err: Error,
}

/// Will remove transaction from the queue on drop.
/// See `Queue::remove_transaction` for details.
pub struct TransactionGuard {
    tx: Arc<CheckedTransaction<'static>>,
    queue: Arc<Queue>,
    routing: RoutingDecision,
    queue_position: u64,
    encoded_len: usize,
    gas_cost: u64,
    #[cfg(feature = "telemetry")]
    telemetry: Option<crate::telemetry::StateTelemetry>,
}

impl Deref for TransactionGuard {
    type Target = CheckedTransaction<'static>;

    fn deref(&self) -> &Self::Target {
        &self.tx
    }
}

impl TransactionGuard {
    #[must_use]
    /// Clone the accepted transaction protected by this guard.
    pub fn clone_accepted(&self) -> AcceptedTransaction<'static> {
        self.tx.as_accepted().clone()
    }

    #[must_use]
    /// Routing decision associated with the transaction.
    pub fn routing(&self) -> RoutingDecision {
        self.routing
    }

    #[must_use]
    /// Encoded payload size (bytes) used for queue budgeting.
    pub fn encoded_len(&self) -> usize {
        self.encoded_len
    }

    #[must_use]
    /// Gas cost associated with the queued transaction.
    pub fn gas_cost(&self) -> u64 {
        self.gas_cost
    }

    #[must_use]
    pub(crate) fn teu_weight(&self) -> u64 {
        Queue::compute_teu_weight(self.tx.as_accepted())
    }

    #[allow(clippy::unused_self)]
    pub(crate) fn record_lane_teu_deferral(&self, lane_id: LaneId, reason: &'static str, teu: u64) {
        #[cfg(feature = "telemetry")]
        if let Some(tel) = self.telemetry.as_ref() {
            tel.inc_nexus_scheduler_lane_teu_deferral(lane_id, reason, teu);
        }
        #[cfg(not(feature = "telemetry"))]
        {
            let _ = (lane_id, reason, teu);
        }
    }
}

impl Drop for TransactionGuard {
    fn drop(&mut self) {
        #[cfg(feature = "telemetry")]
        let telemetry_ref: Option<&StateTelemetry> = self.telemetry.as_ref();
        #[cfg(not(feature = "telemetry"))]
        let telemetry_ref: Option<&StateTelemetry> = None;

        self.queue.remove_transaction(&self.tx, telemetry_ref);
        self.queue.release_inflight_guard();
    }
}

impl Queue {
    fn collect_lane_privacy_proofs(tx: &CheckedTransaction<'_>) -> Vec<LanePrivacyProof> {
        tx.as_ref()
            .as_ref()
            .attachments()
            .into_iter()
            .flat_map(|list| list.0.iter())
            .filter_map(|attachment| attachment.lane_privacy.clone())
            .collect()
    }

    fn compute_tx_encoded_len(tx: &AcceptedTransaction<'_>) -> usize {
        let signed = tx.as_ref();
        signed
            .encoded_len_exact()
            .unwrap_or_else(|| signed.encoded_len())
    }

    pub(crate) fn compute_proposal_gas_cost(tx: &AcceptedTransaction<'_>) -> u64 {
        match tx.as_ref().instructions() {
            Executable::Instructions(batch) => gas::meter_instructions(batch.as_ref()),
            Executable::IvmProved(proved) => gas::meter_instructions(proved.overlay.as_ref()),
            Executable::Ivm(_) => match crate::executor::parse_gas_limit(tx.as_ref().metadata()) {
                Ok(Some(limit)) => limit,
                Ok(None) => {
                    warn!(
                        tx = %tx.as_ref().hash(),
                        "missing gas_limit metadata while deriving proposal gas cost"
                    );
                    0
                }
                Err(err) => {
                    warn!(
                        ?err,
                        tx = %tx.as_ref().hash(),
                        "invalid gas_limit metadata while deriving proposal gas cost"
                    );
                    0
                }
            },
        }
    }

    fn extract_lane_identity_metadata(
        state_view: &StateView<'_>,
        authority: &AccountId,
        dataspace_id: DataSpaceId,
        lane_alias: &str,
    ) -> Result<(Option<UniversalAccountId>, Vec<String>), Error> {
        let account_entry = match state_view.world.account(authority) {
            Ok(entry) => entry,
            Err(_) => return Ok((None, Vec::new())),
        };
        let Some(uaid) = account_entry.value().uaid().copied() else {
            return Ok((None, Vec::new()));
        };

        let bindings = state_view.world.uaid_dataspaces().get(&uaid).ok_or_else(|| {
            Error::LaneComplianceDenied {
                alias: lane_alias.to_string(),
                reason: format!(
                    "account {authority} carries UAID {uaid} but has no Space Directory bindings for dataspace {}",
                    dataspace_id.as_u64()
                ),
            }
        })?;

        let is_bound = bindings.iter().any(|(dataspace, accounts)| {
            *dataspace == dataspace_id && accounts.contains(authority)
        });
        if !is_bound {
            return Err(Error::LaneComplianceDenied {
                alias: lane_alias.to_string(),
                reason: format!(
                    "account {authority} is not bound to dataspace {} for UAID {uaid}",
                    dataspace_id.as_u64()
                ),
            });
        }

        let manifest_set = state_view
            .world
            .space_directory_manifests()
            .get(&uaid)
            .ok_or_else(|| Error::LaneComplianceDenied {
                alias: lane_alias.to_string(),
                reason: format!(
                    "UAID {uaid} has no manifest registry for dataspace {}",
                    dataspace_id.as_u64()
                ),
            })?;
        let record =
            manifest_set
                .get(&dataspace_id)
                .ok_or_else(|| Error::LaneComplianceDenied {
                    alias: lane_alias.to_string(),
                    reason: format!(
                        "UAID {uaid} is missing a manifest for dataspace {}",
                        dataspace_id.as_u64()
                    ),
                })?;

        if !record.is_active() {
            return Err(Error::LaneComplianceDenied {
                alias: lane_alias.to_string(),
                reason: format!(
                    "UAID {uaid} manifest for dataspace {} is not active",
                    dataspace_id.as_u64()
                ),
            });
        }

        let mut tags = BTreeSet::new();
        for entry in &record.manifest.entries {
            if let Some(note) = &entry.notes {
                let trimmed = note.trim();
                if !trimmed.is_empty() {
                    tags.insert(trimmed.to_string());
                }
            }
        }

        Ok((Some(uaid), tags.into_iter().collect()))
    }

    /// Install governance manifests for Nexus lanes (idempotent).
    pub fn install_lane_manifests(&self, manifests: &LaneManifestRegistryHandle) {
        let mut guard = self.lane_manifests.write();
        let previous_missing = guard.missing_aliases();
        *guard = Arc::clone(manifests);
        drop(guard);

        let current_missing = manifests.missing_aliases();
        for alias in current_missing.difference(&previous_missing) {
            iroha_logger::warn!(
                lane = alias,
                "governance manifest missing; lane is sealed until a manifest is installed"
            );
        }
        for alias in previous_missing.difference(&current_missing) {
            iroha_logger::info!(lane = alias, "governance manifest loaded");
        }

        let statuses_snapshot = manifests.statuses();
        status::update_lane_governance_from_statuses(&statuses_snapshot);
        let mut privacy_guard = self.lane_privacy_registry.write();
        *privacy_guard = Arc::new(LanePrivacyRegistry::from_statuses(&statuses_snapshot));
    }

    /// Snapshot of lane privacy commitments derived from manifests.
    #[must_use]
    pub fn lane_privacy_registry(&self) -> LanePrivacyRegistryHandle {
        self.lane_privacy_registry.read().clone()
    }

    /// Background task that reloads lane manifests on the configured schedule.
    pub async fn watch_lane_manifests_task(
        self: Arc<Self>,
        telemetry: Option<StateTelemetry>,
        governance: Arc<GovernanceCatalog>,
        registry_cfg: LaneRegistry,
        state: Option<Arc<State>>,
    ) {
        let lane_catalog = self.lane_catalog.read().clone();
        let initial = Arc::new(LaneManifestRegistry::from_config(
            &lane_catalog,
            &governance,
            &registry_cfg,
        ));
        self.install_lane_manifests(&initial);
        if let Some(state_handle) = state.as_ref() {
            state_handle.install_lane_manifests(&initial);
        }
        if let Some(telemetry_handle) = telemetry.as_ref() {
            telemetry_handle.set_lane_manifest_registry(Arc::clone(&initial));
        }

        if registry_cfg.poll_interval.is_zero() {
            return;
        }

        let mut ticker = interval(registry_cfg.poll_interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            let lane_catalog = self.lane_catalog.read().clone();
            let registry = Arc::new(LaneManifestRegistry::from_config(
                &lane_catalog,
                &governance,
                &registry_cfg,
            ));
            self.install_lane_manifests(&registry);
            if let Some(state_handle) = state.as_ref() {
                state_handle.install_lane_manifests(&registry);
            }
            if let Some(telemetry_handle) = telemetry.as_ref() {
                telemetry_handle.set_lane_manifest_registry(Arc::clone(&registry));
            }
        }
    }

    fn ensure_lane_governance(&self, lane_id: LaneId) -> Result<(), GovernanceGuardError> {
        let guard = self.lane_manifests.read();
        guard.ensure_lane_ready(lane_id)
    }

    fn enforcement_error(alias: &str, reason: impl Into<String>) -> Error {
        Error::GovernanceNotPermitted {
            alias: alias.to_string(),
            reason: reason.into(),
        }
    }

    fn enforce_manifest_quorum(
        alias: &str,
        rules: &GovernanceRules,
        tx: &CheckedTransaction<'_>,
    ) -> Result<(), Error> {
        let Some(quorum) = rules.quorum else {
            return Ok(());
        };
        if quorum <= 1 {
            return Ok(());
        }
        if rules.validators.is_empty() {
            return Ok(());
        }

        let approvals = Self::collect_manifest_approvals(alias, tx)?;
        let validators = Self::canonical_manifest_validators(alias, rules)?;
        let approved = approvals
            .iter()
            .filter(|account| validators.contains(*account))
            .count();
        let required = usize::try_from(quorum).unwrap_or(usize::MAX);
        if approved < required {
            return Err(Self::enforcement_error(
                alias,
                format!(
                    "lane manifest quorum requires {quorum} validator approvals but {approved} were provided"
                ),
            ));
        }
        Ok(())
    }

    fn collect_manifest_approvals(
        alias: &str,
        tx: &CheckedTransaction<'_>,
    ) -> Result<BTreeSet<String>, Error> {
        let mut approvals = BTreeSet::new();
        let signed = tx.as_ref().as_ref();
        let authority = signed.authority();
        let authority_ih58 = authority.canonical_ih58().map_err(|err| {
            Self::enforcement_error(
                alias,
                format!("failed to encode authority `{authority}` as ih58: {err}"),
            )
        })?;
        approvals.insert(authority_ih58);

        let metadata = signed.metadata();
        let Some(raw) = metadata.get(&*GOV_APPROVERS_METADATA_KEY) else {
            return Ok(approvals);
        };
        let entries = raw.try_into_any_norito::<Vec<String>>().map_err(|_| {
            Self::enforcement_error(
                alias,
                "`gov_manifest_approvers` metadata must be an array of account identifiers",
            )
        })?;
        for entry in entries {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                return Err(Self::enforcement_error(
                    alias,
                    "`gov_manifest_approvers` metadata entries must not be blank",
                ));
            }
            let canonical = if let Ok(account) = AccountId::from_str(trimmed) {
                account.canonical_ih58().map_err(|err| {
                    Self::enforcement_error(
                        alias,
                        format!(
                            "invalid account id `{trimmed}` in `gov_manifest_approvers`: {err}"
                        ),
                    )
                })?
            } else {
                let prefix = iroha_data_model::account::address::chain_discriminant();
                let (address, _) = iroha_data_model::account::address::AccountAddress::parse_any(
                    trimmed,
                    Some(prefix),
                )
                .map_err(|err| {
                    Self::enforcement_error(
                        alias,
                        format!(
                            "invalid account id `{trimmed}` in `gov_manifest_approvers`: {err}"
                        ),
                    )
                })?;
                address.to_ih58(prefix).map_err(|err| {
                    Self::enforcement_error(
                        alias,
                        format!(
                            "invalid account id `{trimmed}` in `gov_manifest_approvers`: {err}"
                        ),
                    )
                })?
            };
            approvals.insert(canonical);
        }
        Ok(approvals)
    }

    fn canonical_manifest_validators(
        alias: &str,
        rules: &GovernanceRules,
    ) -> Result<BTreeSet<String>, Error> {
        let mut validators = BTreeSet::new();
        for validator in &rules.validators {
            let ih58 = validator.canonical_ih58().map_err(|err| {
                Self::enforcement_error(
                    alias,
                    format!("failed to encode validator `{validator}` as ih58: {err}"),
                )
            })?;
            validators.insert(ih58);
        }
        Ok(validators)
    }

    #[allow(clippy::too_many_lines)]
    fn enforce_manifest_protected_namespaces(
        alias: &str,
        rules: &GovernanceRules,
        tx: &CheckedTransaction<'_>,
        state_view: &StateView<'_>,
    ) -> Result<(), Error> {
        if rules.protected_namespaces.is_empty() {
            return Ok(());
        }

        let signed = tx.as_ref().as_ref();
        let metadata = signed.metadata();
        let metadata_namespace = metadata
            .get(&*GOV_NAMESPACE_METADATA_KEY)
            .map(|value| {
                let raw = value.try_into_any_norito::<String>().map_err(|_| {
                    Self::enforcement_error(
                        alias,
                        "`gov_namespace` metadata must be a string value",
                    )
                })?;
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    return Err(Self::enforcement_error(
                        alias,
                        "`gov_namespace` metadata must not be blank",
                    ));
                }
                Name::from_str(trimmed).map_err(|err| {
                    Self::enforcement_error(
                        alias,
                        format!("`gov_namespace` metadata `{trimmed}` is not a valid Name: {err}"),
                    )
                })
            })
            .transpose()?;

        let metadata_contract_id = metadata
            .get(&*GOV_CONTRACT_ID_METADATA_KEY)
            .map(|value| {
                let raw = value.try_into_any_norito::<String>().map_err(|_| {
                    Self::enforcement_error(
                        alias,
                        "`gov_contract_id` metadata must be a string value",
                    )
                })?;
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    return Err(Self::enforcement_error(
                        alias,
                        "`gov_contract_id` metadata must not be blank",
                    ));
                }
                Ok(trimmed.to_string())
            })
            .transpose()?;

        let metadata_contract_namespace = metadata
            .get(&*CONTRACT_NAMESPACE_METADATA_KEY)
            .map(|value| {
                let raw = value.try_into_any_norito::<String>().map_err(|_| {
                    Self::enforcement_error(
                        alias,
                        "`contract_namespace` metadata must be a string value",
                    )
                })?;
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    return Err(Self::enforcement_error(
                        alias,
                        "`contract_namespace` metadata must not be blank",
                    ));
                }
                Name::from_str(trimmed).map_err(|err| {
                    Self::enforcement_error(
                        alias,
                        format!(
                            "`contract_namespace` metadata `{trimmed}` is not a valid Name: {err}"
                        ),
                    )
                })
            })
            .transpose()?;

        let metadata_contract_id_hint = metadata
            .get(&*CONTRACT_ID_METADATA_KEY)
            .map(|value| {
                let raw = value.try_into_any_norito::<String>().map_err(|_| {
                    Self::enforcement_error(alias, "`contract_id` metadata must be a string value")
                })?;
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    return Err(Self::enforcement_error(
                        alias,
                        "`contract_id` metadata must not be blank",
                    ));
                }
                Ok(trimmed.to_string())
            })
            .transpose()?;

        let mut namespaces_from_instructions = BTreeSet::new();
        let mut contract_bindings = BTreeSet::new();
        let mut register_code_seen = false;
        if let Executable::Instructions(instructions) = signed.instructions() {
            for instruction in instructions {
                if let Some(activate) = instruction
                    .as_any()
                    .downcast_ref::<ActivateContractInstance>()
                {
                    let ns = Name::from_str(activate.namespace.trim()).map_err(|err| {
                        Self::enforcement_error(
                            alias,
                            format!(
                                "instruction namespace `{}` is not valid: {err}",
                                activate.namespace
                            ),
                        )
                    })?;
                    namespaces_from_instructions.insert(ns.clone());
                    let contract_id = activate.contract_id.trim();
                    if contract_id.is_empty() {
                        return Err(Self::enforcement_error(
                            alias,
                            "contract_id in ActivateContractInstance must not be blank",
                        ));
                    }
                    contract_bindings.insert((ns, contract_id.to_string()));
                } else if let Some(deactivate) = instruction
                    .as_any()
                    .downcast_ref::<DeactivateContractInstance>()
                {
                    let ns = Name::from_str(deactivate.namespace.trim()).map_err(|err| {
                        Self::enforcement_error(
                            alias,
                            format!(
                                "instruction namespace `{}` is not valid: {err}",
                                deactivate.namespace
                            ),
                        )
                    })?;
                    namespaces_from_instructions.insert(ns.clone());
                    let contract_id = deactivate.contract_id.trim();
                    if contract_id.is_empty() {
                        return Err(Self::enforcement_error(
                            alias,
                            "contract_id in DeactivateContractInstance must not be blank",
                        ));
                    }
                    contract_bindings.insert((ns, contract_id.to_string()));
                } else {
                    let modifies_contract_code = {
                        let any = instruction.as_any();
                        any.is::<RegisterSmartContractCode>()
                            || any.is::<RegisterSmartContractBytes>()
                            || any.is::<RemoveSmartContractBytes>()
                    };
                    if modifies_contract_code {
                        register_code_seen = true;
                    }
                }
            }
        }

        if let Some(ns) = metadata_namespace.clone() {
            if let Some(cid) = metadata_contract_id
                .clone()
                .or_else(|| metadata_contract_id_hint.clone())
            {
                namespaces_from_instructions.insert(ns.clone());
                contract_bindings.insert((ns, cid));
            }
        } else if let Some(ns) = metadata_contract_namespace.clone() {
            if let Some(cid) = metadata_contract_id_hint.clone() {
                namespaces_from_instructions.insert(ns.clone());
                contract_bindings.insert((ns, cid));
            }
        }

        let ivm_with_contract_metadata = matches!(signed.instructions(), Executable::Ivm(_))
            && (metadata_namespace.is_some()
                || metadata_contract_namespace.is_some()
                || metadata_contract_id_hint.is_some());

        let contract_instr_seen =
            register_code_seen || !contract_bindings.is_empty() || ivm_with_contract_metadata;

        if contract_instr_seen && metadata_namespace.is_none() {
            return Err(Self::enforcement_error(
                alias,
                "transactions with contract namespace operations must set `gov_namespace` metadata when lane governance protects namespaces",
            ));
        }

        if (contract_instr_seen || metadata_namespace.is_some()) && metadata_contract_id.is_none() {
            return Err(Self::enforcement_error(
                alias,
                "metadata key `gov_contract_id` is required when `gov_namespace` is provided",
            ));
        }

        if let (Some(ns_hint), Some(ns_meta)) = (
            metadata_contract_namespace.as_ref(),
            metadata_namespace.as_ref(),
        ) {
            if ns_hint != ns_meta {
                return Err(Self::enforcement_error(
                    alias,
                    "`contract_namespace` metadata must match `gov_namespace` for protected operations",
                ));
            }
        }

        if let (Some(cid_hint), Some(cid_meta)) = (
            metadata_contract_id_hint.as_ref(),
            metadata_contract_id.as_ref(),
        ) {
            if cid_hint != cid_meta {
                return Err(Self::enforcement_error(
                    alias,
                    "`contract_id` metadata must match `gov_contract_id` for protected operations",
                ));
            }
        }

        if let Some(meta_cid) = metadata_contract_id.as_ref()
            && let Some(target_ns) = metadata_namespace
                .clone()
                .or_else(|| namespaces_from_instructions.iter().next().cloned())
        {
            let cross_namespace = state_view
                .world()
                .contract_instances()
                .iter()
                .filter(|((_ns, cid), _)| cid == meta_cid)
                .filter_map(|((ns, _), _)| Name::from_str(ns).ok())
                .any(|existing_ns| existing_ns != target_ns);
            if cross_namespace {
                return Err(Self::enforcement_error(
                    alias,
                    format!(
                        "contract `{meta_cid}` is already bound to a different namespace; cross-namespace rebinding is not allowed"
                    ),
                ));
            }
        }

        let mut namespaces_to_check = namespaces_from_instructions.clone();
        if let Some(ns) = metadata_namespace.clone() {
            namespaces_to_check.insert(ns);
        }
        if let Some(ns) = metadata_contract_namespace.clone() {
            namespaces_to_check.insert(ns);
        }

        for namespace in &namespaces_to_check {
            if !rules.protected_namespaces.contains(namespace) {
                return Err(Self::enforcement_error(
                    alias,
                    format!(
                        "namespace `{namespace}` is not declared in lane governance protected set"
                    ),
                ));
            }
        }

        if let Some(ns) = metadata_namespace
            && !namespaces_from_instructions.is_empty()
            && namespaces_from_instructions
                .iter()
                .any(|other| other != &ns)
        {
            return Err(Self::enforcement_error(
                alias,
                "`gov_namespace` metadata does not match namespaces referenced by contract instructions",
            ));
        }

        if let Some(meta_contract_id) = metadata_contract_id
            && !contract_bindings.is_empty()
            && contract_bindings
                .iter()
                .any(|(_, cid)| cid != &meta_contract_id)
        {
            return Err(Self::enforcement_error(
                alias,
                "`gov_contract_id` metadata does not match contract ids referenced by contract instructions",
            ));
        }

        Ok(())
    }

    fn enforce_runtime_upgrade_hook(
        alias: &str,
        rules: &GovernanceRules,
        tx: &CheckedTransaction<'_>,
    ) -> Result<bool, Error> {
        let signed = tx.as_ref().as_ref();
        let mut contains_runtime_upgrade = false;
        if let Executable::Instructions(instructions) = signed.instructions() {
            for instruction in instructions {
                if instruction
                    .as_any()
                    .downcast_ref::<ProposeRuntimeUpgrade>()
                    .is_some()
                    || instruction
                        .as_any()
                        .downcast_ref::<ActivateRuntimeUpgrade>()
                        .is_some()
                    || instruction
                        .as_any()
                        .downcast_ref::<CancelRuntimeUpgrade>()
                        .is_some()
                {
                    contains_runtime_upgrade = true;
                    break;
                }
            }
        }
        if !contains_runtime_upgrade {
            return Ok(false);
        }

        let Some(hook) = rules.hooks.runtime_upgrade.as_ref() else {
            return Ok(false);
        };

        if !hook.allow {
            return Err(Self::enforcement_error(
                alias,
                "runtime upgrade hook prohibits runtime upgrade instructions".to_string(),
            ));
        }

        if hook.require_metadata || hook.allowed_ids.is_some() {
            let Some(key) = hook.metadata_key.as_ref() else {
                return Err(Self::enforcement_error(
                    alias,
                    "runtime upgrade hook missing metadata_key despite requiring metadata"
                        .to_string(),
                ));
            };
            let metadata = signed.metadata();
            let Some(raw_value) = metadata.get(key) else {
                return Err(Self::enforcement_error(
                    alias,
                    format!("runtime upgrade hook requires metadata `{}`", key.as_ref()),
                ));
            };
            let value = raw_value.try_into_any_norito::<String>().map_err(|_| {
                Self::enforcement_error(
                    alias,
                    format!(
                        "runtime upgrade metadata `{}` must be a string",
                        key.as_ref()
                    ),
                )
            })?;
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(Self::enforcement_error(
                    alias,
                    format!(
                        "runtime upgrade metadata `{}` must not be blank",
                        key.as_ref()
                    ),
                ));
            }
            if let Some(ids) = hook.allowed_ids.as_ref()
                && !ids.contains(trimmed)
            {
                return Err(Self::enforcement_error(
                    alias,
                    format!(
                        "runtime upgrade metadata `{}` value `{trimmed}` not permitted by lane manifest",
                        key.as_ref()
                    ),
                ));
            }
        }

        Ok(true)
    }

    /// Makes queue from configuration
    pub fn from_config(config: Config, events_sender: EventsSender) -> Self {
        let router: Arc<dyn LaneRouter> = Arc::new(SingleLaneRouter::new());
        let lane_catalog = Arc::new(LaneCatalog::default());
        let dataspace_catalog = Arc::new(DataSpaceCatalog::default());
        Self::from_config_with_router_limits_and_catalogs(
            config,
            events_sender,
            router,
            QueueLimits::default(),
            &lane_catalog,
            &dataspace_catalog,
            None,
        )
    }

    /// Build the queue using the provided router implementation.
    pub fn from_config_with_router(
        Config {
            capacity,
            capacity_per_user,
            transaction_time_to_live,
            expired_cull_interval,
            expired_cull_batch,
        }: Config,
        events_sender: EventsSender,
        router: Arc<dyn LaneRouter>,
    ) -> Self {
        let lane_catalog = Arc::new(LaneCatalog::default());
        let dataspace_catalog = Arc::new(DataSpaceCatalog::default());
        Self::from_config_with_router_limits_and_catalogs(
            Config {
                capacity,
                capacity_per_user,
                transaction_time_to_live,
                expired_cull_interval,
                expired_cull_batch,
            },
            events_sender,
            router,
            QueueLimits::default(),
            &lane_catalog,
            &dataspace_catalog,
            None,
        )
    }

    /// Build the queue using the provided router and explicit Nexus-derived limits.
    pub fn from_config_with_router_and_limits(
        Config {
            capacity,
            capacity_per_user,
            transaction_time_to_live,
            expired_cull_interval,
            expired_cull_batch,
        }: Config,
        events_sender: EventsSender,
        router: Arc<dyn LaneRouter>,
        limits: QueueLimits,
        lane_compliance: Option<Arc<LaneComplianceEngine>>,
    ) -> Self {
        let lane_catalog = Arc::new(LaneCatalog::default());
        let dataspace_catalog = Arc::new(DataSpaceCatalog::default());
        Self::from_config_with_router_limits_and_catalogs(
            Config {
                capacity,
                capacity_per_user,
                transaction_time_to_live,
                expired_cull_interval,
                expired_cull_batch,
            },
            events_sender,
            router,
            limits,
            &lane_catalog,
            &dataspace_catalog,
            lane_compliance,
        )
    }

    /// Build the queue using the provided router, limits, and Nexus catalogs.
    pub fn from_config_with_router_limits_and_catalogs(
        Config {
            capacity,
            capacity_per_user,
            transaction_time_to_live,
            expired_cull_interval,
            expired_cull_batch,
        }: Config,
        events_sender: EventsSender,
        router: Arc<dyn LaneRouter>,
        limits: QueueLimits,
        lane_catalog: &Arc<LaneCatalog>,
        dataspace_catalog: &Arc<DataSpaceCatalog>,
        lane_compliance: Option<Arc<LaneComplianceEngine>>,
    ) -> Self {
        let (backpressure_tx, _) = watch::channel(BackpressureState::Healthy {
            queued: 0,
            capacity,
        });
        let queue = {
            let queue = Self {
                events_sender,
                router: RwLock::new(router),
                lane_compliance: RwLock::new(lane_compliance),
                lane_catalog: RwLock::new(Arc::clone(lane_catalog)),
                dataspace_catalog: RwLock::new(Arc::clone(dataspace_catalog)),
                tx_hashes: ArrayQueue::new(capacity.get()),
                txs: DashMap::new(),
                removed_hashes: DashMap::new(),
                txs_per_user: DashMap::new(),
                routing_decisions: DashMap::new(),
                tx_encoded_len: DashMap::new(),
                tx_gas_cost: DashMap::new(),
                tx_gossip_payloads: DashMap::new(),
                push_remove_lock: parking_lot::Mutex::new(()),
                guard_sequence: AtomicU64::new(0),
                inflight_guards: AtomicUsize::new(0),
                capacity,
                capacity_per_user,
                time_source: TimeSource::new_system(),
                tx_time_to_live: transaction_time_to_live,
                expired_cull_interval,
                expired_cull_batch,
                last_expired_cull_ms: AtomicU64::new(0),
                expiry_ring: parking_lot::Mutex::new(VecDeque::new()),
                expiry_ring_members: DashMap::new(),
                tx_gossip: ArrayQueue::new(capacity.get()),
                backpressure_tx,
                sumeragi_wake: OnceLock::new(),
                nexus_limits: RwLock::new(limits),
                #[cfg(feature = "telemetry")]
                tx_teu: DashMap::new(),
                #[cfg(feature = "telemetry")]
                lane_teu_pending: DashMap::new(),
                #[cfg(feature = "telemetry")]
                dataspace_teu_pending: DashMap::new(),
                lane_manifests: parking_lot::RwLock::new(Arc::new(LaneManifestRegistry::empty())),
                lane_privacy_registry: parking_lot::RwLock::new(Arc::new(
                    LanePrivacyRegistry::empty(),
                )),
                #[cfg(test)]
                vacant_entry_warnings: AtomicUsize::new(0),
            };
            #[cfg(feature = "telemetry")]
            {
                for lane in lane_catalog.lanes() {
                    queue.lane_teu_pending.entry(lane.id).or_default();
                    for dataspace in dataspace_catalog.entries() {
                        queue
                            .dataspace_teu_pending
                            .entry((lane.id, dataspace.id))
                            .or_default();
                    }
                }
            }
            queue
        };
        #[cfg(not(feature = "telemetry"))]
        let _ = limits;
        #[cfg(not(feature = "telemetry"))]
        {
            let _ = lane_catalog;
            let _ = dataspace_catalog;
        }
        queue.publish_backpressure_state(0, None);
        queue
    }

    pub(crate) fn set_sumeragi_wake(&self, wake: mpsc::SyncSender<()>) {
        let _ = self.sumeragi_wake.set(wake);
    }

    fn wake_sumeragi(&self) {
        if let Some(wake) = self.sumeragi_wake.get() {
            let _ = wake.try_send(());
        }
    }

    fn is_pending(&self, tx: &CheckedTransaction<'static>, state_view: &StateView) -> bool {
        !self.is_expired(tx.as_accepted()) && !tx.is_in_blockchain(state_view)
    }

    fn is_pending_with_state(
        &self,
        hash: SignedTxHash,
        tx: &CheckedTransaction<'static>,
        state: &State,
    ) -> bool {
        !self.is_expired(tx.as_accepted()) && !state.has_committed_transaction(hash)
    }

    /// Checks if the transaction is waiting longer than its TTL or than the TTL from [`Config`].
    pub fn is_expired(&self, tx: &AcceptedTransaction<'static>) -> bool {
        self.is_expired_at(tx, self.time_source.get_unix_time())
    }

    /// Checks if the transaction is expired at a specific time.
    fn is_expired_at(&self, tx: &AcceptedTransaction<'static>, now: Duration) -> bool {
        let tx_creation_time = tx.as_ref().creation_time();

        let time_limit = tx.as_ref().time_to_live().map_or_else(
            || self.tx_time_to_live,
            |tx_time_to_live| core::cmp::min(self.tx_time_to_live, tx_time_to_live),
        );

        now.saturating_sub(tx_creation_time) > time_limit
    }

    /// Returns all pending transactions.
    pub fn all_transactions<'state>(
        &'state self,
        state_view: &'state StateView,
    ) -> impl Iterator<Item = AcceptedTransaction<'static>> + 'state {
        self.txs.iter().filter_map(|tx| {
            let tx_arc = tx.value();
            let tx_ref = tx_arc.as_ref();
            if self.is_pending(tx_ref, state_view) {
                // Deep-clone underlying transaction to preserve queue semantics
                return Some(tx_ref.as_accepted().clone());
            }

            None
        })
    }

    /// Returns `n` transactions in a batch for gossiping
    pub fn gossip_batch(&self, n: u32, state_view: &StateView) -> Vec<GossipBatchEntry> {
        self.gossip_batch_inner(n, |_, tx_ref| self.is_pending(tx_ref, state_view))
    }

    /// Returns `n` transactions in a batch for gossiping using narrow state accessors.
    pub fn gossip_batch_with_state(&self, n: u32, state: &State) -> Vec<GossipBatchEntry> {
        self.gossip_batch_inner(n, |hash, tx_ref| {
            self.is_pending_with_state(hash, tx_ref, state)
        })
    }

    fn gossip_batch_inner<F>(&self, n: u32, mut is_pending: F) -> Vec<GossipBatchEntry>
    where
        F: FnMut(SignedTxHash, &CheckedTransaction<'static>) -> bool,
    {
        let mut batch = Vec::with_capacity(n as usize);
        while let Some(hash) = self.tx_gossip.pop() {
            let Some(tx) = self.txs.get(&hash) else {
                // NOTE: Transaction already in the blockchain
                continue;
            };
            let tx_ref = tx.value().as_ref();
            if is_pending(hash, tx_ref) {
                let routing = if let Some(decision) = self.routing_decisions.get(&hash) {
                    *decision.value()
                } else {
                    warn!(
                        tx = %hash,
                        "missing routing decision for queued transaction, skipping gossip"
                    );
                    continue;
                };
                let payload = if let Some(entry) = self.tx_gossip_payloads.get(&hash) {
                    Arc::clone(entry.value())
                } else {
                    let signed = tx_ref.as_accepted().as_ref();
                    let encoded_len = self
                        .tx_encoded_len
                        .get(&hash)
                        .map(|entry| *entry.value())
                        .unwrap_or_else(|| Self::compute_tx_encoded_len(tx_ref.as_accepted()));
                    let mut buf = Vec::with_capacity(encoded_len);
                    signed.encode_to(&mut buf);
                    let payload = Arc::new(buf);
                    self.tx_gossip_payloads.insert(hash, Arc::clone(&payload));
                    payload
                };
                // Deep clone to produce an owned AcceptedTransaction for gossip
                batch.push(GossipBatchEntry {
                    tx: tx_ref.as_accepted().clone(),
                    routing,
                    payload,
                });
                if batch.len() >= n as usize {
                    break;
                }
            }
        }
        batch
    }

    /// Resolve routing for an inbound gossip transaction using the active router.
    pub(crate) fn route_for_gossip(
        &self,
        tx: &AcceptedTransaction<'_>,
        state_view: &StateView<'_>,
    ) -> RoutingDecision {
        self.router.read().route(tx, state_view)
    }

    /// Returns whether the queue currently tracks the transaction hash.
    ///
    /// This is used by gossip fast paths to skip expensive re-validation for
    /// entries that are already known locally.
    pub(crate) fn contains_transaction_hash(&self, hash: SignedTxHash) -> bool {
        self.txs.contains_key(&hash)
    }

    /// Returns whether the queue still tracks `hash` as pending (not expired/committed).
    #[must_use]
    pub fn contains_pending_hash(&self, hash: SignedTxHash, state: &State) -> bool {
        let Some(entry) = self.txs.get(&hash) else {
            return false;
        };
        let tx = entry.value().as_ref();
        !self.is_expired(tx.as_accepted()) && !state.has_committed_transaction(hash)
    }

    /// Return transactions back to the gossip backlog by their hashes.
    pub fn requeue_gossip_hashes(&self, hashes: impl IntoIterator<Item = SignedTxHash>) {
        for hash in hashes {
            if !self.txs.contains_key(&hash) {
                continue;
            }
            if let Err(err_hash) = self.tx_gossip.push(hash) {
                warn!(
                    tx = %err_hash,
                    "Gossiper is lagging behind, not able to requeue tx for gossiping"
                );
                break;
            }
        }
    }

    fn check_tx(
        &self,
        tx: &CheckedTransaction<'static>,
        state_view: &StateView,
    ) -> Result<(), Error> {
        if tx.is_in_blockchain(state_view) {
            Err(Error::InBlockchain)
        } else if self.is_expired(tx.as_accepted()) {
            Err(Error::Expired)
        } else {
            Ok(())
        }
    }

    /// Push transaction into queue.
    ///
    /// # Errors
    /// See [`enum@Error`]
    #[allow(clippy::too_many_lines)]
    fn push_with_lane_internal(
        &self,
        tx: AcceptedTransaction<'static>,
        state_view: &StateView<'_>,
        gossip_payload: Option<Arc<Vec<u8>>>,
    ) -> Result<RoutingDecision, Failure> {
        let routing_decision = self.router.read().route(&tx, state_view);
        let lane_id = routing_decision.lane_id;
        let dataspace_id = routing_decision.dataspace_id;

        trace!(
            lane_id = %lane_id,
            dataspace_id = %dataspace_id,
            tx = %tx.as_ref().hash(),
            "Pushing to the queue"
        );
        let checked = match tx.into_checked(state_view) {
            Ok(checked) => checked,
            Err((original, _)) => {
                return Err(Failure {
                    tx: original.into(),
                    err: Error::InBlockchain,
                });
            }
        };
        let authority = checked.as_ref().authority();
        if self.is_expired(checked.as_accepted()) {
            return Err(Failure {
                tx: Box::new(checked.into_accepted()),
                err: Error::Expired,
            });
        }
        #[cfg(feature = "telemetry")]
        let telemetry_handle = state_view.telemetry;

        #[cfg(feature = "telemetry")]
        let mut manifest_allowed = false;
        let manifest_status = {
            let guard = self.lane_manifests.read();
            guard.status(lane_id).cloned()
        };
        let lane_alias = manifest_status.as_ref().map_or_else(
            || format!("lane-{}", lane_id.as_u32()),
            |status| status.alias.clone(),
        );
        if let Err(err) = self.ensure_lane_governance(lane_id) {
            let reason = err.message();
            iroha_logger::warn!(
                lane = %lane_id.as_u32(),
                reason,
                "rejecting transaction while governance manifest is missing"
            );
            #[cfg(feature = "telemetry")]
            telemetry_handle.record_manifest_admission("missing_manifest");
            return Err(Failure {
                tx: Box::new(checked.as_accepted().clone()),
                err: Error::Governance(err),
            });
        }
        if let Some(status) = manifest_status {
            if let Some(rules) = status.rules() {
                let alias = status.alias.clone();
                if !rules.validators.is_empty()
                    && !rules
                        .validators
                        .iter()
                        .any(|validator| validator == checked.as_ref().authority())
                {
                    iroha_logger::warn!(
                        lane = %alias,
                        authority = %checked.as_ref().authority(),
                        "rejecting transaction not signed by governance validator"
                    );
                    #[cfg(feature = "telemetry")]
                    telemetry_handle.record_manifest_admission("non_validator_authority");
                    return Err(Failure {
                        tx: Box::new(checked.as_accepted().clone()),
                        err: Error::GovernanceNotPermitted {
                            alias,
                            reason: "authority not part of lane validator set".to_string(),
                        },
                    });
                }
                let quorum_required =
                    rules.quorum.unwrap_or(0).saturating_sub(1) > 0 && !rules.validators.is_empty();
                let quorum_result = Self::enforce_manifest_quorum(&alias, rules, &checked);
                if quorum_required {
                    match quorum_result {
                        Ok(()) => {
                            #[cfg(feature = "telemetry")]
                            telemetry_handle.record_manifest_quorum_enforcement("satisfied");
                        }
                        Err(err) => {
                            #[cfg(feature = "telemetry")]
                            telemetry_handle.record_manifest_quorum_enforcement("rejected");
                            #[cfg(feature = "telemetry")]
                            telemetry_handle.record_manifest_admission("quorum_rejected");
                            return Err(Failure {
                                tx: Box::new(checked.as_accepted().clone()),
                                err,
                            });
                        }
                    }
                } else if let Err(err) = quorum_result {
                    #[cfg(feature = "telemetry")]
                    telemetry_handle.record_manifest_admission("quorum_rejected");
                    return Err(Failure {
                        tx: Box::new(checked.as_accepted().clone()),
                        err,
                    });
                }
                if let Err(err) =
                    Self::enforce_manifest_protected_namespaces(&alias, rules, &checked, state_view)
                {
                    #[cfg(feature = "telemetry")]
                    if !rules.protected_namespaces.is_empty() {
                        telemetry_handle.record_protected_namespace_enforcement("rejected");
                    }
                    #[cfg(feature = "telemetry")]
                    telemetry_handle.record_manifest_admission("protected_namespace_rejected");
                    return Err(Failure {
                        tx: Box::new(checked.as_accepted().clone()),
                        err,
                    });
                }
                #[cfg(feature = "telemetry")]
                if !rules.protected_namespaces.is_empty() {
                    telemetry_handle.record_protected_namespace_enforcement("allowed");
                }
                let runtime_hook_result =
                    Self::enforce_runtime_upgrade_hook(&alias, rules, &checked);
                let runtime_hook_applied = match runtime_hook_result {
                    Ok(applied) => applied,
                    Err(err) => {
                        #[cfg(feature = "telemetry")]
                        telemetry_handle
                            .record_manifest_hook_enforcement("runtime_upgrade", "rejected");
                        #[cfg(feature = "telemetry")]
                        telemetry_handle.record_manifest_admission("runtime_hook_rejected");
                        return Err(Failure {
                            tx: Box::new(checked.as_accepted().clone()),
                            err,
                        });
                    }
                };
                #[cfg(feature = "telemetry")]
                if runtime_hook_applied {
                    telemetry_handle.record_manifest_hook_enforcement("runtime_upgrade", "allowed");
                }
                #[cfg(not(feature = "telemetry"))]
                let _ = runtime_hook_applied;
                #[cfg(feature = "telemetry")]
                {
                    manifest_allowed = true;
                }
            }
        }

        if let Ok(account_entry) = state_view.world.account(authority) {
            if let Some(uaid) = account_entry.value().uaid().copied() {
                let bound = state_view
                    .world
                    .uaid_dataspaces()
                    .get(&uaid)
                    .is_some_and(|bindings| bindings.is_bound_to(dataspace_id, authority));
                if !bound {
                    #[cfg(feature = "telemetry")]
                    telemetry_handle.record_manifest_admission("uaid_not_bound");
                    warn!(
                        uaid = %uaid,
                        dataspace = %dataspace_id.as_u64(),
                        authority = %authority,
                        "rejecting transaction: UAID not bound to dataspace"
                    );
                    return Err(Failure {
                        tx: Box::new(checked.as_accepted().clone()),
                        err: Error::UaidNotBound {
                            uaid,
                            dataspace: dataspace_id,
                        },
                    });
                }
            }
        }

        let lane_privacy_registry_handle = self.lane_privacy_registry();
        let privacy_proofs = Self::collect_lane_privacy_proofs(&checked);
        let verified_privacy_commitments = if privacy_proofs.is_empty() {
            BTreeSet::new()
        } else {
            match verify_lane_privacy_proofs(
                lane_privacy_registry_handle.as_ref(),
                lane_id,
                &privacy_proofs,
            ) {
                Ok(verified) => verified,
                Err(err) => {
                    return Err(Failure {
                        tx: Box::new(checked.as_accepted().clone()),
                        err: Error::LanePrivacyProofRejected {
                            alias: lane_alias.clone(),
                            reason: err.to_string(),
                        },
                    });
                }
            }
        };
        let lane_privacy_registry = if lane_privacy_registry_handle.is_empty() {
            None
        } else {
            Some(lane_privacy_registry_handle)
        };

        let lane_identity = Self::extract_lane_identity_metadata(
            state_view,
            checked.as_ref().authority(),
            dataspace_id,
            &lane_alias,
        )
        .map_err(|err| Failure {
            tx: Box::new(checked.as_accepted().clone()),
            err,
        })?;

        let lane_compliance = self.lane_compliance.read().clone();
        if let Some(engine) = lane_compliance.as_ref() {
            let (uaid_value, capability_tags) = lane_identity;
            let ctx = LaneComplianceContext {
                lane_id,
                dataspace_id,
                authority: checked.as_ref().authority(),
                uaid: uaid_value.as_ref(),
                capability_tags: capability_tags.as_slice(),
                lane_privacy_registry,
                verified_privacy_commitments: &verified_privacy_commitments,
            };
            let evaluation = engine.evaluate(&ctx);
            match evaluation {
                LaneComplianceEvaluation::NotConfigured => {}
                LaneComplianceEvaluation::Allowed(record) => {
                    record.log(engine.audit_only());
                }
                LaneComplianceEvaluation::Denied(record) => {
                    record.log(engine.audit_only());
                    if !engine.audit_only() {
                        let reason = record
                            .reason
                            .clone()
                            .unwrap_or_else(|| "lane compliance policy denied".to_string());
                        return Err(Failure {
                            tx: Box::new(checked.as_accepted().clone()),
                            err: Error::LaneComplianceDenied {
                                alias: lane_alias.clone(),
                                reason,
                            },
                        });
                    }
                }
            }
        }
        #[cfg(feature = "telemetry")]
        if manifest_allowed {
            telemetry_handle.record_manifest_admission("allowed");
        }
        #[cfg(feature = "telemetry")]
        let pending_teu = Self::compute_teu_weight(checked.as_accepted());
        #[cfg(feature = "telemetry")]
        let backpressure_telemetry: Option<&StateTelemetry> = Some(telemetry_handle);
        #[cfg(not(feature = "telemetry"))]
        let backpressure_telemetry: Option<&StateTelemetry> = None;
        let gossip_payload = gossip_payload.filter(|payload| !payload.is_empty());
        let encoded_len = gossip_payload
            .as_ref()
            .map(|payload| payload.len())
            .unwrap_or_else(|| Self::compute_tx_encoded_len(checked.as_accepted()));
        let proposal_gas_cost = Self::compute_proposal_gas_cost(checked.as_accepted());
        let hash = checked.as_ref().hash();
        let _guard = self.push_remove_lock.lock();
        let txs_len = self.txs.len();
        let entry = match self.txs.entry(hash) {
            Entry::Occupied(_) => {
                return Err(Failure {
                    tx: checked.as_accepted().clone().into(),
                    err: Error::IsInQueue,
                });
            }
            Entry::Vacant(entry) => entry,
        };

        if txs_len >= self.capacity.get() {
            warn!(
                lane_id = %lane_id,
                dataspace_id = %dataspace_id,
                max = self.capacity,
                "Achieved maximum amount of transactions"
            );
            self.publish_backpressure_state(txs_len, backpressure_telemetry);
            return Err(Failure {
                tx: checked.as_accepted().clone().into(),
                err: Error::Full,
            });
        }

        if let Err(err) = self.check_and_increase_per_user_tx_count(checked.as_ref().authority()) {
            return Err(Failure {
                tx: checked.as_accepted().clone().into(),
                err,
            });
        }

        // Insert entry first so that the `tx` popped from `queue` will always have a `(hash, tx)` record in `txs`.
        let tx_arc = Arc::new(checked);
        entry.insert(Arc::clone(&tx_arc));
        self.routing_decisions.insert(hash, routing_decision);
        routing_ledger::record(hash, routing_decision);
        // Drop the local holder before attempting to unwrap on push failure.
        drop(tx_arc);
        let mut pushed = self.tx_hashes.push(hash).is_ok();
        if !pushed {
            let compacted = self.compact_hash_queue_locked();
            if compacted > 0 {
                pushed = self.tx_hashes.push(hash).is_ok();
            }
        }
        if !pushed {
            warn!("Queue is full");
            let (_, err_tx) = self.txs.remove(&hash).expect("Inserted just before match");
            if let Some((_, decision)) = self.routing_decisions.remove(&hash) {
                routing_ledger::discard_if_matches(&hash, decision);
            }
            self.decrease_per_user_tx_count(err_tx.as_ref().as_ref().authority());
            self.publish_backpressure_state(self.capacity.get(), backpressure_telemetry);
            return Err(Failure {
                tx: Box::new(
                    Arc::try_unwrap(err_tx)
                        .unwrap_or_else(|_| panic!("no other Arc holders during push failure"))
                        .into_accepted(),
                ),
                err: Error::Full,
            });
        }
        self.tx_encoded_len.insert(hash, encoded_len);
        if let Some(payload) = gossip_payload {
            self.tx_gossip_payloads.insert(hash, payload);
        }
        self.tx_gas_cost.insert(hash, proposal_gas_cost);
        self.track_expiry_hash(hash);
        #[cfg(feature = "telemetry")]
        self.record_teu_enqueue(
            hash,
            TxTeuInfo {
                lane_id,
                dataspace_id,
                teu: pending_teu,
            },
            telemetry_handle,
        );
        iroha_logger::debug!(
            tx = %hash,
            lane_id = %lane_id,
            dataspace_id = %dataspace_id,
            queued = self.tx_hashes.len(),
            "transaction enqueued"
        );
        self.publish_backpressure_state(self.active_len(), backpressure_telemetry);
        if let Err(err_hash) = self.tx_gossip.push(hash) {
            warn!(
                lane_id = %lane_id,
                dataspace_id = %dataspace_id,
                tx = %err_hash,
                "Gossiper is lagging behind, not able to queue tx for gossiping"
            );
        }
        let _ = self.events_sender.send(
            TransactionEvent {
                hash,
                block_height: None,
                lane_id,
                dataspace_id,
                status: TransactionStatus::Queued,
            }
            .into(),
        );
        trace!(
            lane_id = %lane_id,
            dataspace_id = %dataspace_id,
            len = self.tx_hashes.len(),
            "Transaction queue length"
        );
        self.wake_sumeragi();
        Ok(routing_decision)
    }

    /// Pushes an accepted transaction into the queue using a cached gossip payload.
    ///
    /// # Errors
    /// Propagates [`Failure`] when the queue rejects the transaction (for example, when it is full
    /// or violates lane limits).
    pub fn push_with_gossip_payload_in_view(
        &self,
        tx: AcceptedTransaction<'static>,
        state_view: &StateView<'_>,
        gossip_payload: Option<Arc<Vec<u8>>>,
    ) -> Result<(), Failure> {
        self.push_with_lane_internal(tx, state_view, gossip_payload)
            .map(|_| ())
    }

    /// Pushes an accepted transaction into the queue using a cached gossip payload.
    ///
    /// # Errors
    /// Propagates [`Failure`] when the queue rejects the transaction (for example, when it is full
    /// or violates lane limits).
    pub fn push_with_gossip_payload(
        &self,
        tx: AcceptedTransaction<'static>,
        state_view: StateView,
        gossip_payload: Option<Arc<Vec<u8>>>,
    ) -> Result<(), Failure> {
        self.push_with_gossip_payload_in_view(tx, &state_view, gossip_payload)
    }

    /// Push transaction into queue.
    ///
    /// # Errors
    /// See [`enum@Error`]
    pub fn push_with_lane(
        &self,
        tx: AcceptedTransaction<'static>,
        state_view: StateView,
    ) -> Result<RoutingDecision, Failure> {
        self.push_with_lane_in_view(tx, &state_view)
    }

    /// Push transaction into queue with a shared [`StateView`] snapshot.
    ///
    /// # Errors
    /// See [`enum@Error`]
    pub fn push_with_lane_in_view(
        &self,
        tx: AcceptedTransaction<'static>,
        state_view: &StateView<'_>,
    ) -> Result<RoutingDecision, Failure> {
        self.push_with_lane_internal(tx, state_view, None)
    }

    /// Pushes an accepted transaction into the queue, routing it to the lane resolved from the
    /// supplied [`StateView`].
    ///
    /// # Errors
    /// Propagates [`Failure`] when the queue rejects the transaction (for example, when it is full
    /// or violates lane limits).
    pub fn push(
        &self,
        tx: AcceptedTransaction<'static>,
        state_view: StateView,
    ) -> Result<(), Failure> {
        self.push_in_view(tx, &state_view)
    }

    /// Pushes an accepted transaction into the queue with a shared [`StateView`] snapshot.
    ///
    /// # Errors
    /// Propagates [`Failure`] when the queue rejects the transaction (for example, when it is full
    /// or violates lane limits).
    pub fn push_in_view(
        &self,
        tx: AcceptedTransaction<'static>,
        state_view: &StateView<'_>,
    ) -> Result<(), Failure> {
        self.push_with_lane_in_view(tx, state_view).map(|_| ())
    }

    /// Drop all queued transactions and gossip buffers.
    ///
    /// Used when a peer is removed from the topology so it stops advertising stale transactions.
    pub fn clear_all(&self) {
        while let Some(hash) = self.tx_hashes.pop() {
            self.txs.remove(&hash);
            self.routing_decisions.remove(&hash);
            self.removed_hashes.remove(&hash);
        }
        while self.tx_gossip.pop().is_some() {}
        self.txs_per_user.clear();
        self.expiry_ring_members.clear();
        self.expiry_ring.lock().clear();
        self.tx_encoded_len.clear();
        self.tx_gas_cost.clear();
        self.tx_gossip_payloads.clear();
        #[cfg(feature = "telemetry")]
        {
            self.tx_teu.clear();
            self.lane_teu_pending.clear();
            self.dataspace_teu_pending.clear();
        }
        self.publish_backpressure_state(0, None);
    }

    fn record_inflight_guard(&self) {
        self.inflight_guards.fetch_add(1, Ordering::Relaxed);
    }

    fn release_inflight_guard(&self) {
        let prev = self.inflight_guards.fetch_sub(1, Ordering::Relaxed);
        debug_assert!(prev > 0, "inflight guard counter underflow");
    }

    /// Pop single transaction from the queue. Removes all transactions that fail the `tx_check`.
    fn pop_from_queue(
        self: &Arc<Self>,
        state_view: &StateView,
        expired_transactions: &mut Vec<AcceptedTransaction<'static>>,
    ) -> Option<TransactionGuard> {
        #[cfg(feature = "telemetry")]
        let backpressure_telemetry: Option<&StateTelemetry> = Some(state_view.telemetry);
        #[cfg(not(feature = "telemetry"))]
        let backpressure_telemetry: Option<&StateTelemetry> = None;
        loop {
            let hash = if let Some(hash) = self.tx_hashes.pop() {
                hash
            } else {
                if self.resync_hash_queue_if_needed(state_view) {
                    continue;
                }
                return None;
            };
            if self.removed_hashes.remove(&hash).is_some() {
                continue;
            }

            let tx_arc = if let Some(entry) = self.txs.get(&hash) {
                Arc::clone(entry.value())
            } else {
                self.untrack_expiry_hash(&hash);
                #[cfg(test)]
                self.vacant_entry_warnings.fetch_add(1, Ordering::Relaxed);
                warn!("Looks like we're experiencing a high load");
                continue;
            };

            if let Err(e) = self.check_tx(tx_arc.as_ref(), state_view) {
                iroha_logger::warn!(
                    tx = %hash,
                    ?e,
                    "dropping transaction during queue pop (check_tx)"
                );
                iroha_logger::trace!(?hash, ?e, "dropping transaction during queue pop");
                // Drop the cloned arc before removing to keep expiration recovery effective.
                drop(tx_arc);
                if let Some((_, removed_tx)) = self.txs.remove(&hash) {
                    self.untrack_expiry_hash(&hash);
                    self.decrease_per_user_tx_count(removed_tx.as_ref().as_ref().authority());
                    if let Some((_, decision)) = self.routing_decisions.remove(&hash) {
                        routing_ledger::discard_if_matches(&hash, decision);
                    } else {
                        // Ensure we do not leak entries when the queue-side cache missed the remove.
                        let _ = routing_ledger::take(&hash);
                    }
                    #[cfg(feature = "telemetry")]
                    self.record_teu_dequeue(&hash, Some(state_view.telemetry));
                    if let Error::Expired = e
                        && let Ok(tx) = Arc::try_unwrap(removed_tx)
                    {
                        expired_transactions.push(tx.into_accepted());
                    }
                }
                self.tx_encoded_len.remove(&hash);
                self.tx_gas_cost.remove(&hash);
                self.tx_gossip_payloads.remove(&hash);
                continue;
            }

            let routing = self
                .routing_decisions
                .get(&hash)
                .map(|entry| *entry.value())
                .unwrap_or_default();
            let queue_position = self.guard_sequence.fetch_add(1, Ordering::Relaxed);
            let encoded_len = self
                .tx_encoded_len
                .get(&hash)
                .map(|entry| *entry.value())
                .unwrap_or_else(|| Self::compute_tx_encoded_len(tx_arc.as_accepted()));
            let gas_cost = self
                .tx_gas_cost
                .get(&hash)
                .map(|entry| *entry.value())
                .unwrap_or_else(|| Self::compute_proposal_gas_cost(tx_arc.as_accepted()));
            #[cfg(feature = "telemetry")]
            let telemetry_clone = state_view.telemetry.clone();
            self.record_inflight_guard();
            let guard = TransactionGuard {
                tx: tx_arc,
                queue: Arc::clone(self),
                routing,
                queue_position,
                encoded_len,
                gas_cost,
                #[cfg(feature = "telemetry")]
                telemetry: Some(telemetry_clone),
            };
            self.publish_backpressure_state(self.active_len(), backpressure_telemetry);
            return Some(guard);
        }
    }

    /// Rebuild the hash queue when it is empty but transactions remain in the map.
    /// Skip resync if there are in-flight guards to avoid re-enqueuing active selections.
    fn resync_hash_queue_if_needed(&self, state_view: &StateView) -> bool {
        if !self.tx_hashes.is_empty() || self.txs.is_empty() {
            return false;
        }
        if self.inflight_guards.load(Ordering::Relaxed) > 0 {
            return false;
        }
        #[cfg(feature = "telemetry")]
        let backpressure_telemetry: Option<&StateTelemetry> = Some(state_view.telemetry);
        #[cfg(not(feature = "telemetry"))]
        let backpressure_telemetry: Option<&StateTelemetry> = None;
        #[cfg(not(feature = "telemetry"))]
        let _ = state_view;

        let _guard = self.push_remove_lock.lock();
        if !self.tx_hashes.is_empty() || self.txs.is_empty() {
            return false;
        }

        let total = self.txs.len();
        // Sort by hash to keep the recovery order deterministic across peers.
        let mut hashes: Vec<SignedTxHash> = self.txs.iter().map(|entry| *entry.key()).collect();
        hashes.sort();

        let mut inserted = 0usize;
        for hash in hashes {
            if self.tx_hashes.push(hash).is_err() {
                warn!(
                    queued = self.tx_hashes.len(),
                    total,
                    "queue hash resync reached capacity before re-enqueuing all transactions"
                );
                break;
            }
            self.removed_hashes.remove(&hash);
            inserted = inserted.saturating_add(1);
        }

        if inserted > 0 {
            self.removed_hashes.clear();
            self.publish_backpressure_state(self.active_len(), backpressure_telemetry);
            warn!(
                queued = self.tx_hashes.len(),
                total, "queue hash index was empty; rebuilt from queued transactions"
            );
            return true;
        }

        false
    }

    /// Return the number of transactions tracked by the queue (queued + in-flight).
    pub fn active_len(&self) -> usize {
        self.txs.len()
    }

    /// Return the number of transactions still awaiting selection from the queue.
    pub fn queued_len(&self) -> usize {
        let queued = self.tx_hashes.len();
        queued.saturating_sub(self.removed_hashes.len())
    }

    /// Track a queued transaction hash for TTL sweeps.
    fn track_expiry_hash(&self, hash: SignedTxHash) {
        if self.expiry_ring_members.insert(hash, ()).is_none() {
            let mut ring = self.expiry_ring.lock();
            ring.push_back(hash);
        }
    }

    /// Drop a transaction hash from TTL sweep tracking.
    fn untrack_expiry_hash(&self, hash: &SignedTxHash) {
        self.expiry_ring_members.remove(hash);
    }

    /// Remove expired transactions if the configured sweep interval has elapsed.
    pub(crate) fn cull_expired_entries_if_due(&self) -> usize {
        let interval_ms = Self::duration_to_millis(self.expired_cull_interval);
        if interval_ms == 0 {
            return 0;
        }
        let now = self.time_source.get_unix_time();
        let now_ms = Self::duration_to_millis(now);
        let last_ms = self.last_expired_cull_ms.load(Ordering::Relaxed);
        if now_ms.saturating_sub(last_ms) < interval_ms {
            return 0;
        }
        self.last_expired_cull_ms.store(now_ms, Ordering::Relaxed);
        self.cull_expired_entries(now)
    }

    /// Gets transactions till they fill whole block or till the end of queue.
    ///
    /// BEWARE: Shouldn't be called in parallel with itself.
    #[cfg(test)]
    fn collect_transactions_for_block(
        self: &Arc<Self>,
        state_view: &StateView,
        max_txs_in_block: NonZeroUsize,
    ) -> Vec<TransactionGuard> {
        let mut transactions = Vec::with_capacity(max_txs_in_block.get());
        self.get_transactions_for_block(state_view, max_txs_in_block, &mut transactions);
        transactions
    }

    /// Put transactions into provided vector until they fill the whole block or there are no more transactions in the queue.
    ///
    /// BEWARE: Shouldn't be called in parallel with itself.
    pub fn get_transactions_for_block(
        self: &Arc<Self>,
        state_view: &StateView,
        max_txs_in_block: NonZeroUsize,
        transactions: &mut Vec<TransactionGuard>,
    ) {
        if transactions.len() >= max_txs_in_block.get() {
            return;
        }

        let mut expired_transactions = Vec::new();

        let txs_from_queue =
            core::iter::from_fn(|| self.pop_from_queue(state_view, &mut expired_transactions));

        let transactions_hashes: IndexSet<SignedTxHash> =
            transactions.iter().map(|tx| tx.as_ref().hash()).collect();
        let txs = txs_from_queue
            .filter(|tx| !transactions_hashes.contains(&tx.as_ref().hash()))
            .take(max_txs_in_block.get() - transactions.len());
        for tx in txs {
            iroha_logger::debug!(tx = %tx.as_ref().hash(), "queue pop selected transaction");
            transactions.push(tx);
        }

        expired_transactions
            .into_iter()
            .map(|tx| {
                let hash = tx.as_ref().hash();
                let routing = self
                    .routing_decisions
                    .remove(&hash)
                    .map(|(_, decision)| decision)
                    .or_else(|| routing_ledger::take(&hash))
                    .unwrap_or_default();
                TransactionEvent {
                    hash,
                    block_height: None,
                    lane_id: routing.lane_id,
                    dataspace_id: routing.dataspace_id,
                    status: TransactionStatus::Expired,
                }
            })
            .for_each(|e| {
                let _ = self.events_sender.send(e.into());
            });

        // Safety net: if the queue hashes got out of sync (e.g., under high load in tests)
        // and some expired transactions remain only in the `txs` map, cull them now so
        // that expiration events are not missed, preventing tests from hanging on recv().
        if transactions.is_empty() {
            let _ = self.cull_expired_entries(self.time_source.get_unix_time());
        }
    }

    /// Apply lane-level TEU limits to the provided transaction guards, returning any transactions
    /// that need to be re-queued because their lane would exceed the configured capacity.
    pub fn enforce_lane_teu_limits(
        &self,
        guards: &mut Vec<TransactionGuard>,
    ) -> Vec<AcceptedTransaction<'static>> {
        let mut consumed_teu = BTreeMap::new();
        self.enforce_lane_teu_limits_with_consumption(guards, &mut consumed_teu)
    }

    /// Apply lane-level TEU limits to the provided transaction guards, taking into account any
    /// previously consumed TEU recorded in `consumed_teu`. Returns transactions that must be
    /// deferred because serving them would exceed the configured lane capacity.
    pub fn enforce_lane_teu_limits_with_consumption(
        &self,
        guards: &mut Vec<TransactionGuard>,
        consumed_teu: &mut BTreeMap<LaneId, u64>,
    ) -> Vec<AcceptedTransaction<'static>> {
        if guards.is_empty() {
            return Vec::new();
        }

        let mut retained: Vec<TransactionGuard> = Vec::with_capacity(guards.len());
        let mut deferred: Vec<AcceptedTransaction<'static>> = Vec::new();

        let mut drained = std::mem::take(guards);
        drained.sort_by(|left, right| {
            let left_route = left.routing();
            let right_route = right.routing();
            left_route
                .lane_id
                .cmp(&right_route.lane_id)
                .then_with(|| left_route.dataspace_id.cmp(&right_route.dataspace_id))
                .then_with(|| left.queue_position.cmp(&right.queue_position))
                .then_with(|| left.tx.as_ref().hash().cmp(&right.tx.as_ref().hash()))
        });
        for guard in drained {
            let routing = guard.routing();
            let lane_id = routing.lane_id;
            let limits = self.nexus_limits.read().for_lane(lane_id);
            let teu = guard.teu_weight();
            let used = consumed_teu.entry(lane_id).or_insert(0);
            let new_total = used.saturating_add(teu);
            // Admit overweight transactions when the configured TEU cap is zero (disabled) or
            // when the first transaction of the batch already exceeds the cap. Otherwise the
            // pacemaker would livelock, constantly re-queuing an unservable transaction.
            let is_first_for_lane = *used == 0;
            if limits.teu_capacity == 0 || (new_total > limits.teu_capacity && is_first_for_lane) {
                iroha_logger::warn!(
                    lane_id = %lane_id,
                    teu,
                    configured_teu_capacity = limits.teu_capacity,
                    "admitting transaction that exceeds lane TEU capacity to avoid starvation"
                );
                *used = new_total;
                retained.push(guard);
                continue;
            }
            if new_total > limits.teu_capacity {
                guard.record_lane_teu_deferral(lane_id, "cap_exceeded", teu);
                deferred.push(guard.clone_accepted());
                // guard drops here, removing the transaction from the queue.
                continue;
            }
            *used = new_total;
            retained.push(guard);
        }

        *guards = retained;
        deferred
    }

    fn duration_to_millis(duration: Duration) -> u64 {
        u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
    }

    /// Remove any entries from `txs` that have expired, emitting expiration
    /// events for TTL-elapsed transactions.
    fn cull_expired_entries(&self, now: Duration) -> usize {
        const CULL_WARN_MS: u64 = 1_000;
        let scan_start = std::time::Instant::now();
        let tracked_before = self.txs.len();
        let mut to_remove = Vec::new();
        let mut expired = 0usize;
        let mut scanned = 0usize;
        let max_scan = self.expired_cull_batch.get();
        let mut ring = self.expiry_ring.lock();
        while scanned < max_scan {
            let Some(hash) = ring.pop_front() else {
                break;
            };
            scanned = scanned.saturating_add(1);
            if !self.expiry_ring_members.contains_key(&hash) {
                continue;
            }
            let Some(entry) = self.txs.get(&hash) else {
                self.expiry_ring_members.remove(&hash);
                continue;
            };
            if self.is_expired_at(entry.value().as_accepted(), now) {
                expired = expired.saturating_add(1);
                to_remove.push(hash);
                self.expiry_ring_members.remove(&hash);
            } else {
                ring.push_back(hash);
            }
        }
        let ring_len = ring.len();
        drop(ring);
        let scan_ms = Self::duration_to_millis(scan_start.elapsed());
        if to_remove.is_empty() {
            if scan_ms >= CULL_WARN_MS {
                warn!(
                    scan_ms,
                    tracked_before,
                    scanned,
                    ring_len,
                    expired,
                    "queue expiry sweep slow without removals"
                );
            }
            return 0;
        }
        let remove_start = std::time::Instant::now();
        let _guard = self.push_remove_lock.lock();
        let mut removed = 0usize;
        for hash in to_remove {
            if let Some((_, tx_arc)) = self.txs.remove(&hash) {
                let routing = self
                    .routing_decisions
                    .remove(&hash)
                    .map(|(_, decision)| decision)
                    .or_else(|| routing_ledger::take(&hash))
                    .unwrap_or_default();
                self.removed_hashes.insert(hash, ());
                self.untrack_expiry_hash(&hash);
                self.decrease_per_user_tx_count(tx_arc.as_ref().as_ref().authority());
                #[cfg(feature = "telemetry")]
                self.record_teu_dequeue(&hash, None);
                if let Ok(tx) = Arc::try_unwrap(tx_arc) {
                    let accepted = tx.into_accepted();
                    if self.is_expired_at(&accepted, now) {
                        let _ = self.events_sender.send(
                            TransactionEvent {
                                hash,
                                block_height: None,
                                lane_id: routing.lane_id,
                                dataspace_id: routing.dataspace_id,
                                status: TransactionStatus::Expired,
                            }
                            .into(),
                        );
                    }
                }
                removed = removed.saturating_add(1);
            }
            self.tx_encoded_len.remove(&hash);
            self.tx_gas_cost.remove(&hash);
            self.tx_gossip_payloads.remove(&hash);
        }
        let remove_ms = Self::duration_to_millis(remove_start.elapsed());
        if removed > 0 && !self.removed_hashes.is_empty() && !self.tx_hashes.is_empty() {
            let _ = self.compact_hash_queue_locked();
        }
        if removed > 0 {
            self.publish_backpressure_state(self.active_len(), None);
        }
        let total_ms = scan_ms.saturating_add(remove_ms);
        if total_ms >= CULL_WARN_MS {
            warn!(
                scan_ms,
                remove_ms,
                total_ms,
                tracked_before,
                tracked_after = self.txs.len(),
                scanned,
                ring_len,
                expired,
                removed,
                "queue expiry sweep slow"
            );
        }
        removed
    }

    /// Rebuild the hash queue, dropping entries that are no longer tracked in `txs`.
    ///
    /// Caller must hold `push_remove_lock` to exclude concurrent enqueue operations.
    fn compact_hash_queue_locked(&self) -> usize {
        let mut retained = Vec::with_capacity(self.txs.len());
        let mut dropped = 0usize;
        while let Some(hash) = self.tx_hashes.pop() {
            self.removed_hashes.remove(&hash);
            if self.txs.contains_key(&hash) {
                retained.push(hash);
            } else {
                dropped = dropped.saturating_add(1);
            }
        }
        for hash in retained {
            if self.tx_hashes.push(hash).is_err() {
                warn!(
                    queued = self.tx_hashes.len(),
                    tracked = self.txs.len(),
                    "queue hash compaction reached capacity before re-enqueuing all transactions"
                );
                break;
            }
        }
        self.removed_hashes.clear();
        dropped
    }

    /// Overview:
    /// 1. Transaction is added to queue using [`Queue::push`] method.
    /// 2. Transaction is moved to [`Sumeragi::transaction_cache`] using [`Queue::pop_from_queue`] method.
    ///    Note that transaction is removed from [`Queue::tx_hashes`], but kept in [`Queue::accepted_tx`],
    ///    this is needed to return `Error::IsInQueue` when adding same transaction twice.
    /// 3. When transaction is removed from [`Sumeragi::transaction_cache`]
    ///    (either because it was expired, or because transaction is committed to blockchain),
    ///    we should remove transaction from [`Queue::accepted_tx`].
    #[cfg_attr(not(feature = "telemetry"), allow(unused_variables))]
    fn remove_transaction(
        &self,
        tx: &CheckedTransaction<'static>,
        telemetry: Option<&StateTelemetry>,
    ) {
        let hash = tx.as_ref().hash();
        let _guard = self.push_remove_lock.lock();
        if self.txs.remove(&hash).is_some() {
            self.untrack_expiry_hash(&hash);
            let decision = self
                .routing_decisions
                .remove(&hash)
                .map(|(_, decision)| decision);
            self.decrease_per_user_tx_count(tx.as_ref().authority());
            if let Some(decision) = decision {
                routing_ledger::record(hash, decision);
            }
        }
        self.tx_encoded_len.remove(&hash);
        self.tx_gas_cost.remove(&hash);
        self.tx_gossip_payloads.remove(&hash);
        // Transaction guards always represent popped hashes; clear any stale marker even if
        // the entry was culled while the guard was in-flight.
        self.removed_hashes.remove(&hash);
        #[cfg(feature = "telemetry")]
        self.record_teu_dequeue(&hash, telemetry);
        self.publish_backpressure_state(self.active_len(), telemetry);
    }

    /// Remove committed transactions from the queue by hash, preserving routing metadata.
    #[cfg_attr(not(feature = "telemetry"), allow(unused_variables))]
    pub(crate) fn remove_committed_hashes(
        &self,
        hashes: impl IntoIterator<Item = SignedTxHash>,
        telemetry: Option<&StateTelemetry>,
    ) -> usize {
        let _guard = self.push_remove_lock.lock();
        let mut removed = 0usize;
        for hash in hashes {
            let tx_arc = self.txs.remove(&hash).map(|(_, tx)| tx);
            self.untrack_expiry_hash(&hash);
            let _ = self.routing_decisions.remove(&hash);
            let _ = routing_ledger::take(&hash);
            self.tx_encoded_len.remove(&hash);
            self.tx_gas_cost.remove(&hash);
            self.tx_gossip_payloads.remove(&hash);
            if let Some(tx_arc) = tx_arc {
                self.removed_hashes.insert(hash, ());
                self.decrease_per_user_tx_count(tx_arc.as_ref().as_ref().authority());
                #[cfg(feature = "telemetry")]
                self.record_teu_dequeue(&hash, telemetry);
                removed = removed.saturating_add(1);
            }
        }
        // Keep removed markers until the consumer drains stale hashes or a push triggers
        // compaction, so committed removals stay observable to in-flight guards.
        if removed > 0 {
            self.publish_backpressure_state(self.active_len(), telemetry);
        }
        removed
    }

    /// Check that the user adhered to the maximum transaction per user limit and increment their transaction count.
    fn check_and_increase_per_user_tx_count(&self, account_id: &AccountId) -> Result<(), Error> {
        match self.txs_per_user.entry(account_id.clone()) {
            Entry::Vacant(vacant) => {
                vacant.insert(1);
            }
            Entry::Occupied(mut occupied) => {
                let txs = *occupied.get();
                if txs >= self.capacity_per_user.get() {
                    warn!(
                        max_txs_per_user = self.capacity_per_user,
                        %account_id,
                        "Account reached maximum allowed number of transactions in the queue per user"
                    );
                    return Err(Error::MaximumTransactionsPerUser);
                }
                *occupied.get_mut() += 1;
            }
        }

        Ok(())
    }

    fn decrease_per_user_tx_count(&self, account_id: &AccountId) {
        let Entry::Occupied(mut occupied) = self.txs_per_user.entry(account_id.clone()) else {
            panic!("Call to decrease always should be paired with increase count. This is a bug.")
        };

        let count = occupied.get_mut();
        if *count > 1 {
            *count -= 1;
        } else {
            occupied.remove_entry();
        }
    }

    #[cfg_attr(not(feature = "telemetry"), allow(unused_variables))]
    fn publish_backpressure_state(&self, queued: usize, telemetry: Option<&StateTelemetry>) {
        let capacity = self.capacity;
        let state = if queued >= capacity.get() {
            BackpressureState::Saturated { queued, capacity }
        } else {
            BackpressureState::Healthy { queued, capacity }
        };

        let _ = self.backpressure_tx.send_if_modified(|current| {
            if *current == state {
                false
            } else {
                *current = state;
                true
            }
        });

        #[cfg(feature = "telemetry")]
        if let Some(tel) = telemetry {
            crate::telemetry::record_state_tx_queue_backpressure(
                tel,
                queued as u64,
                capacity.get() as u64,
                state.is_saturated(),
            );
        }
        #[cfg(not(feature = "telemetry"))]
        let _ = telemetry;
    }

    fn compute_teu_weight(tx: &AcceptedTransaction<'static>) -> u64 {
        use iroha_data_model::transaction::Executable;

        match tx.as_ref().instructions() {
            Executable::Instructions(batch) => {
                let instructions: Vec<_> = batch.iter().map(Clone::clone).collect();
                gas::meter_instructions(&instructions)
            }
            Executable::IvmProved(proved) => gas::meter_instructions(proved.overlay.as_ref()),
            Executable::Ivm(bytecode) => Self::compute_ivm_teu_weight(bytecode.as_ref()),
        }
    }

    fn compute_ivm_teu_weight(bytecode: &[u8]) -> u64 {
        match ProgramMetadata::parse(bytecode) {
            Ok(parsed) => {
                let max_cycles = parsed.metadata.max_cycles;
                if max_cycles == 0 {
                    IVM_TEU_FALLBACK
                } else {
                    max_cycles
                }
            }
            Err(err) => {
                warn!(
                    ?err,
                    "Failed to parse IVM metadata while deriving TEU weight; using fallback"
                );
                IVM_TEU_FALLBACK
            }
        }
    }

    #[cfg(feature = "telemetry")]
    fn record_teu_enqueue(
        &self,
        hash: SignedTxHash,
        info: TxTeuInfo,
        telemetry: &crate::telemetry::StateTelemetry,
    ) {
        self.tx_teu.insert(hash, info);

        self.lane_teu_pending
            .entry(info.lane_id)
            .and_modify(|agg| {
                agg.teu = agg.teu.saturating_add(info.teu);
                agg.tx_count += 1;
            })
            .or_insert(PendingTeu {
                teu: info.teu,
                tx_count: 1,
            });

        self.dataspace_teu_pending
            .entry((info.lane_id, info.dataspace_id))
            .and_modify(|agg| {
                agg.teu = agg.teu.saturating_add(info.teu);
                agg.tx_count += 1;
            })
            .or_insert(PendingTeu {
                teu: info.teu,
                tx_count: 1,
            });

        self.publish_teu_backlog_metrics(Some(telemetry));
    }

    #[cfg(feature = "telemetry")]
    fn record_teu_dequeue(
        &self,
        hash: &SignedTxHash,
        telemetry: Option<&crate::telemetry::StateTelemetry>,
    ) {
        let Some((_, info)) = self.tx_teu.remove(hash) else {
            return;
        };

        if let Entry::Occupied(mut occ) = self.lane_teu_pending.entry(info.lane_id) {
            let agg = occ.get_mut();
            agg.teu = agg.teu.saturating_sub(info.teu);
            agg.tx_count = agg.tx_count.saturating_sub(1);
        }

        if let Entry::Occupied(mut occ) = self
            .dataspace_teu_pending
            .entry((info.lane_id, info.dataspace_id))
        {
            let agg = occ.get_mut();
            agg.teu = agg.teu.saturating_sub(info.teu);
            agg.tx_count = agg.tx_count.saturating_sub(1);
        }

        self.publish_teu_backlog_metrics(telemetry);
    }

    #[cfg(feature = "telemetry")]
    fn publish_teu_backlog_metrics(&self, telemetry: Option<&crate::telemetry::StateTelemetry>) {
        let Some(telemetry) = telemetry else {
            return;
        };

        let lane_snapshot: Vec<(LaneId, PendingTeu)> = self
            .lane_teu_pending
            .iter()
            .map(|entry| (*entry.key(), *entry.value()))
            .collect();

        for (lane_id, aggregate) in lane_snapshot {
            let limits = self.nexus_limits.read().for_lane(lane_id);
            let committed = aggregate.teu.min(limits.teu_capacity);
            let headroom = limits.teu_capacity.saturating_sub(committed);
            telemetry.record_nexus_scheduler_lane_teu(
                lane_id,
                LaneTeuGaugeUpdate {
                    capacity: limits.teu_capacity,
                    committed,
                    buckets: NexusLaneTeuBuckets {
                        floor: 0,
                        headroom,
                        must_serve: 0,
                        circuit_breaker: 0,
                    },
                    trigger_level: 0,
                    starvation_bound_slots: limits.starvation_bound_slots,
                },
            );
        }

        let dataspace_snapshot: Vec<((LaneId, DataSpaceId), PendingTeu)> = self
            .dataspace_teu_pending
            .iter()
            .map(|entry| (*entry.key(), *entry.value()))
            .collect();

        for ((lane_id, dataspace_id), aggregate) in dataspace_snapshot {
            telemetry.record_nexus_scheduler_dataspace_teu(
                lane_id,
                dataspace_id,
                DataspaceTeuGaugeUpdate {
                    backlog: aggregate.teu,
                    age_slots: 0,
                    virtual_finish: 0,
                },
            );
        }
    }

    #[cfg(not(feature = "telemetry"))]
    #[allow(dead_code)]
    fn publish_teu_backlog_metrics(&self, _telemetry: Option<&()>) {
        let _ = self;
    }

    #[cfg_attr(not(feature = "telemetry"), allow(unused_variables))]
    fn reroute_pending_transactions(
        &self,
        router: &Arc<dyn LaneRouter>,
        state_view: &StateView<'_>,
        lane_catalog: &LaneCatalog,
        dataspace_catalog: &DataSpaceCatalog,
    ) {
        #[cfg(not(feature = "telemetry"))]
        let _ = (lane_catalog, dataspace_catalog);
        #[cfg(feature = "telemetry")]
        {
            self.tx_teu.clear();
            self.lane_teu_pending.clear();
            self.dataspace_teu_pending.clear();
            for lane in lane_catalog.lanes() {
                self.lane_teu_pending.insert(lane.id, PendingTeu::default());
                for dataspace in dataspace_catalog.entries() {
                    self.dataspace_teu_pending
                        .insert((lane.id, dataspace.id), PendingTeu::default());
                }
            }
        }

        for entry in &self.txs {
            let tx = entry.value();
            if !self.is_pending(tx.as_ref(), state_view) {
                #[cfg(feature = "telemetry")]
                {
                    self.tx_teu.remove(entry.key());
                }
                continue;
            }
            let routing = router.route(tx.as_accepted(), state_view);
            self.routing_decisions.insert(*entry.key(), routing);
            #[cfg(feature = "telemetry")]
            {
                let teu = Self::compute_teu_weight(tx.as_accepted());
                let info = TxTeuInfo {
                    lane_id: routing.lane_id,
                    dataspace_id: routing.dataspace_id,
                    teu,
                };
                self.tx_teu.insert(*entry.key(), info);
                self.lane_teu_pending
                    .entry(routing.lane_id)
                    .and_modify(|agg| {
                        agg.teu = agg.teu.saturating_add(teu);
                        agg.tx_count += 1;
                    })
                    .or_insert(PendingTeu { teu, tx_count: 1 });
                self.dataspace_teu_pending
                    .entry((routing.lane_id, routing.dataspace_id))
                    .and_modify(|agg| {
                        agg.teu = agg.teu.saturating_add(teu);
                        agg.tx_count += 1;
                    })
                    .or_insert(PendingTeu { teu, tx_count: 1 });
            }
        }

        #[cfg(feature = "telemetry")]
        self.publish_teu_backlog_metrics(Some(state_view.telemetry));
    }

    /// Expose a handle for observing queue load.
    #[must_use]
    pub fn backpressure_handle(&self) -> BackpressureHandle {
        BackpressureHandle {
            rx: self.backpressure_tx.subscribe(),
        }
    }

    /// Snapshot current load without subscribing to updates.
    #[must_use]
    pub fn current_backpressure(&self) -> BackpressureState {
        *self.backpressure_tx.borrow()
    }

    pub(crate) fn estimate_teu(tx: &AcceptedTransaction<'static>) -> u64 {
        Self::compute_teu_weight(tx)
    }

    /// Returns the queue capacity limits enforced when admitting transactions.
    #[must_use]
    pub fn queue_limits(&self) -> QueueLimits {
        self.nexus_limits.read().clone()
    }

    /// Return the currently configured lane compliance engine, if any.
    #[must_use]
    pub fn lane_compliance_engine(&self) -> Option<Arc<LaneComplianceEngine>> {
        self.lane_compliance.read().clone()
    }

    /// Refresh routing, limits, manifests, and telemetry after a Nexus catalog update.
    pub fn reconfigure_nexus(
        &self,
        nexus: &Nexus,
        state_view: &StateView<'_>,
        lane_compliance: Option<Arc<LaneComplianceEngine>>,
    ) {
        let lane_catalog = Arc::new(nexus.lane_catalog.clone());
        let dataspace_catalog = Arc::new(nexus.dataspace_catalog.clone());
        let router: Arc<dyn LaneRouter> = Arc::new(ConfigLaneRouter::new(
            nexus.routing_policy.clone(),
            (*dataspace_catalog).clone(),
            (*lane_catalog).clone(),
        ));
        *self.router.write() = Arc::clone(&router);
        *self.nexus_limits.write() = QueueLimits::from_nexus(nexus);
        *self.lane_catalog.write() = Arc::clone(&lane_catalog);
        *self.dataspace_catalog.write() = Arc::clone(&dataspace_catalog);
        *self.lane_compliance.write() = lane_compliance;

        let registry = Arc::new(LaneManifestRegistry::from_config(
            &lane_catalog,
            &nexus.governance,
            &nexus.registry,
        ));
        self.install_lane_manifests(&registry);

        #[cfg(feature = "telemetry")]
        {
            state_view
                .telemetry
                .set_nexus_catalogs(&lane_catalog, &dataspace_catalog);
        }

        self.reroute_pending_transactions(&router, state_view, &lane_catalog, &dataspace_catalog);
    }

    /// Apply a lane lifecycle plan to the WSV and refresh queue routing/limits.
    ///
    /// This helper keeps queue routing, manifests, and telemetry aligned with
    /// the latest Nexus catalogs after lanes are added or retired at runtime.
    ///
    /// # Errors
    /// Returns an error if updating the lane lifecycle or reconfiguring Nexus metadata fails.
    pub fn apply_lane_lifecycle(
        &self,
        state: &mut State,
        plan: &LaneLifecyclePlan,
    ) -> Result<(), LaneLifecycleError> {
        let lane_compliance = self.lane_compliance.read().clone();
        state.apply_lane_lifecycle(plan)?;
        let view = state.view();
        self.reconfigure_nexus(&view.nexus, &view, lane_compliance);
        Ok(())
    }
}

#[cfg(test)]
/// Test helpers and cases for `Queue` and related logic.
pub mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        thread,
        time::Duration,
    };

    use crossbeam_queue::ArrayQueue;
    use dashmap::DashMap;
    use iroha_crypto::{
        Hash, KeyPair, MerkleTree,
        privacy::{LaneCommitmentId, LanePrivacyCommitment, MerkleCommitment},
    };
    use iroha_data_model::{
        block::SignedBlock,
        events::pipeline::PipelineEventBox,
        isi::runtime_upgrade::ProposeRuntimeUpgrade,
        metadata::Metadata,
        name::Name,
        nexus::{
            AssetPermissionManifest, AuditControls, DataSpaceCatalog, DataSpaceId, JurisdictionSet,
            LaneCatalog, LaneCompliancePolicy, LaneCompliancePolicyId, LaneComplianceRule,
            LaneConfig, LaneId, LaneLifecyclePlan, LanePrivacyMerkleWitness, LanePrivacyProof,
            LanePrivacyWitness, ManifestVersion, ParticipantSelector,
        },
        parameter::TransactionParameters,
        prelude::*,
        proof::{ProofAttachment, ProofAttachmentList, ProofBox, VerifyingKeyBox},
        runtime::RuntimeUpgradeManifest,
    };
    use iroha_primitives::json::Json;
    use iroha_schema::Ident;
    #[cfg(feature = "telemetry")]
    use iroha_telemetry::metrics::Metrics;
    use iroha_test_samples::{ALICE_KEYPAIR, gen_account_in};
    use nonzero_ext::nonzero;
    use rand::Rng as _;

    #[allow(unused_imports)]
    use super::*;
    use crate::{
        block::{BlockBuilder, EventProducer, ValidBlock},
        compliance::LaneComplianceEngine,
        governance::manifest::{
            GovernanceHooks, GovernanceRules, LaneManifestRegistry, LaneManifestStatus,
            RuntimeUpgradeHook,
        },
        kura::Kura,
        nexus::space_directory::{
            SpaceDirectoryManifestRecord, SpaceDirectoryManifestSet, UaidDataspaceBindings,
        },
        query::store::LiveQueryStore,
        state::{State, World},
    };

    impl Queue {
        /// Construct a `Queue` instance suitable for unit tests.
        ///
        /// Initializes bounded queues and counters using the provided configuration
        /// and a controllable `time_source`.
        pub fn test(cfg: Config, time_source: &TimeSource) -> Self {
            let router: Arc<dyn LaneRouter> = Arc::new(SingleLaneRouter::new());
            Self::test_with_router(cfg, time_source, router)
        }

        /// Construct a `Queue` instance for unit tests with a custom router.
        pub fn test_with_router(
            cfg: Config,
            time_source: &TimeSource,
            router: Arc<dyn LaneRouter>,
        ) -> Self {
            let (backpressure_tx, _) = watch::channel(BackpressureState::Healthy {
                queued: 0,
                capacity: cfg.capacity,
            });
            let queue = {
                let lane_catalog = Arc::new(LaneCatalog::default());
                let dataspace_catalog = Arc::new(DataSpaceCatalog::default());
                let queue = Self {
                    events_sender: tokio::sync::broadcast::Sender::new(1),
                    router: parking_lot::RwLock::new(router),
                    lane_compliance: parking_lot::RwLock::new(None),
                    lane_catalog: parking_lot::RwLock::new(Arc::clone(&lane_catalog)),
                    dataspace_catalog: parking_lot::RwLock::new(Arc::clone(&dataspace_catalog)),
                    tx_hashes: ArrayQueue::new(cfg.capacity.get()),
                    tx_gossip: ArrayQueue::new(cfg.capacity.get()),
                    txs: DashMap::new(),
                    routing_decisions: DashMap::new(),
                    tx_encoded_len: DashMap::new(),
                    tx_gas_cost: DashMap::new(),
                    tx_gossip_payloads: DashMap::new(),
                    removed_hashes: DashMap::new(),
                    txs_per_user: DashMap::new(),
                    push_remove_lock: parking_lot::Mutex::new(()),
                    guard_sequence: AtomicU64::new(0),
                    inflight_guards: AtomicUsize::new(0),
                    capacity: cfg.capacity,
                    capacity_per_user: cfg.capacity_per_user,
                    time_source: time_source.clone(),
                    tx_time_to_live: cfg.transaction_time_to_live,
                    expired_cull_interval: cfg.expired_cull_interval,
                    expired_cull_batch: cfg.expired_cull_batch,
                    last_expired_cull_ms: AtomicU64::new(0),
                    expiry_ring: parking_lot::Mutex::new(VecDeque::new()),
                    expiry_ring_members: DashMap::new(),
                    backpressure_tx,
                    sumeragi_wake: OnceLock::new(),
                    nexus_limits: parking_lot::RwLock::new(QueueLimits::default()),
                    #[cfg(feature = "telemetry")]
                    tx_teu: DashMap::new(),
                    #[cfg(feature = "telemetry")]
                    lane_teu_pending: DashMap::new(),
                    #[cfg(feature = "telemetry")]
                    dataspace_teu_pending: DashMap::new(),
                    lane_manifests: parking_lot::RwLock::new(Arc::new(
                        LaneManifestRegistry::empty(),
                    )),
                    lane_privacy_registry: parking_lot::RwLock::new(Arc::new(
                        LanePrivacyRegistry::empty(),
                    )),
                    #[cfg(test)]
                    vacant_entry_warnings: AtomicUsize::new(0),
                };
                #[cfg(feature = "telemetry")]
                {
                    for lane in lane_catalog.lanes() {
                        queue
                            .lane_teu_pending
                            .insert(lane.id, PendingTeu::default());
                        for dataspace in dataspace_catalog.entries() {
                            queue
                                .dataspace_teu_pending
                                .insert((lane.id, dataspace.id), PendingTeu::default());
                        }
                    }
                }
                queue
            };
            queue.publish_backpressure_state(0, None);
            queue
        }
    }

    struct StaticRouter {
        lane: LaneId,
        dataspace: DataSpaceId,
    }

    #[test]
    fn queue_limits_default_matches_nexus_defaults() {
        let defaults = QueueLimits::default();
        let expected = QueueLimits::from_nexus(&Nexus::default());

        assert_eq!(
            defaults.fallback.teu_capacity,
            expected.fallback.teu_capacity
        );
        assert_eq!(
            defaults.fallback.starvation_bound_slots,
            expected.fallback.starvation_bound_slots
        );
        assert_eq!(defaults.per_lane, expected.per_lane);
        assert!(
            defaults.fallback.teu_capacity > 0,
            "fallback TEU capacity should remain non-zero without telemetry"
        );
    }

    #[test]
    fn queue_limits_use_configured_fusion_teu_when_nexus_disabled() {
        let nexus = Nexus {
            enabled: false,
            fusion: iroha_config::parameters::actual::Fusion {
                exit_teu: 120_000,
                ..Default::default()
            },
            ..Default::default()
        };

        let limits = QueueLimits::from_nexus(&nexus);

        assert_eq!(limits.fallback.teu_capacity, 120_000);
        assert!(
            limits.per_lane.is_empty(),
            "disabled nexus should not apply lane overrides"
        );
    }

    #[test]
    fn apply_lane_lifecycle_reconfigures_router_and_limits() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let mut state = State::new_for_testing(World::default(), kura, query_handle);
        let lane_catalog =
            LaneCatalog::new(nonzero!(1_u32), vec![LaneConfig::default()]).expect("lane catalog");
        let nexus = Nexus {
            enabled: true,
            lane_catalog: lane_catalog.clone(),
            ..Nexus::default()
        };
        state
            .set_nexus(nexus.clone())
            .expect("apply initial Nexus config");

        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let router: Arc<dyn LaneRouter> = Arc::new(ConfigLaneRouter::new(
            nexus.routing_policy.clone(),
            nexus.dataspace_catalog.clone(),
            nexus.lane_catalog.clone(),
        ));
        let lane_catalog = Arc::new(nexus.lane_catalog.clone());
        let dataspace_catalog = Arc::new(nexus.dataspace_catalog.clone());
        let mut queue = Queue::from_config_with_router_limits_and_catalogs(
            Config {
                transaction_time_to_live: Duration::from_secs(60),
                capacity: nonzero!(8_usize),
                capacity_per_user: nonzero!(4_usize),
                ..Config::default()
            },
            tokio::sync::broadcast::Sender::new(1),
            router,
            QueueLimits::from_nexus(&nexus),
            &lane_catalog,
            &dataspace_catalog,
            None,
        );
        queue.time_source = time_source.clone();

        let (account_id, keypair) = gen_account_in("wonderland");
        let tx = accepted_tx_by(account_id, &keypair, &time_source);
        let tx_hash = tx.as_ref().hash();
        queue.push(tx, state.view()).expect("push");

        let mut metadata = BTreeMap::new();
        metadata.insert("scheduler.teu_capacity".to_string(), "123".to_string());
        let lane_b = LaneConfig {
            id: LaneId::new(1),
            alias: "beta".to_string(),
            metadata,
            ..LaneConfig::default()
        };
        let plan = LaneLifecyclePlan {
            additions: vec![lane_b.clone()],
            retire: Vec::new(),
        };
        state.nexus.get_mut().routing_policy.default_lane = lane_b.id;

        queue
            .apply_lane_lifecycle(&mut state, &plan)
            .expect("plan applied");

        let routing = queue
            .routing_decisions
            .get(&tx_hash)
            .expect("routing decision");
        assert_eq!(routing.lane_id, lane_b.id);
        assert_eq!(queue.queue_limits().for_lane(lane_b.id).teu_capacity, 123);
        assert_eq!(queue.lane_catalog.read().lanes().len(), 2);
    }

    impl LaneRouter for StaticRouter {
        fn route(
            &self,
            _tx: &AcceptedTransaction<'_>,
            _state_view: &StateView<'_>,
        ) -> RoutingDecision {
            RoutingDecision::new(self.lane, self.dataspace)
        }
    }

    #[tokio::test]
    async fn governance_manifest_validators_gate_admission() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        #[cfg(feature = "telemetry")]
        let metrics = Arc::new(Metrics::default());
        #[cfg(feature = "telemetry")]
        let state = Arc::new(State::with_telemetry(
            world_with_test_domains(),
            kura.clone(),
            query_handle.clone(),
            StateTelemetry::new(metrics.clone(), true),
        ));
        #[cfg(not(feature = "telemetry"))]
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let time_source = TimeSource::new_system();

        let queue = Arc::new(Queue::test(config_factory(), &time_source));

        let (validator_id, validator_keypair) = gen_account_in("wonderland");
        let (other_id, other_keypair) = gen_account_in("wonderland");

        let mut statuses = BTreeMap::new();
        let rules = GovernanceRules {
            validators: vec![validator_id.clone()],
            ..GovernanceRules::default()
        };
        let status = LaneManifestStatus {
            lane: LaneId::SINGLE,
            alias: "default".to_string(),
            dataspace: DataSpaceId::GLOBAL,
            visibility: LaneVisibility::Public,
            storage: LaneStorageProfile::FullReplica,
            governance: Some("parliament".to_string()),
            manifest_path: Some(PathBuf::from("/tmp/manifest.json")),
            governance_rules: Some(rules),
            privacy_commitments: Vec::new(),
        };
        statuses.insert(LaneId::SINGLE, status);
        let manifests = Arc::new(LaneManifestRegistry::from_statuses(statuses));
        queue.install_lane_manifests(&manifests);

        let validator_tx = accepted_tx_by(validator_id.clone(), &validator_keypair, &time_source);
        queue
            .push(validator_tx, state.view())
            .expect("validator should be admitted");
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_manifest_admission_total
                .with_label_values(&["allowed"])
                .get(),
            1
        );

        let other_tx = accepted_tx_by(other_id.clone(), &other_keypair, &time_source);
        let err = queue
            .push(other_tx, state.view())
            .expect_err("non-validator should be rejected");
        assert!(matches!(err.err, Error::GovernanceNotPermitted { .. }));
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_manifest_admission_total
                .with_label_values(&["non_validator_authority"])
                .get(),
            1
        );
    }

    #[test]
    fn installing_manifests_populates_privacy_registry() {
        use iroha_crypto::privacy::{LaneCommitmentId, LanePrivacyCommitment, MerkleCommitment};

        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let queue = Queue::test(config_factory(), &time_source);

        let mut statuses = BTreeMap::new();
        statuses.insert(
            LaneId::SINGLE,
            LaneManifestStatus {
                lane: LaneId::SINGLE,
                alias: "default".to_string(),
                dataspace: DataSpaceId::GLOBAL,
                visibility: LaneVisibility::Public,
                storage: LaneStorageProfile::CommitmentOnly,
                governance: None,
                manifest_path: Some(PathBuf::from("/tmp/privacy.json")),
                governance_rules: None,
                privacy_commitments: vec![LanePrivacyCommitment::merkle(
                    LaneCommitmentId::new(7),
                    MerkleCommitment::from_root_bytes([0x11; 32], 4),
                )],
            },
        );
        let manifests = Arc::new(LaneManifestRegistry::from_statuses(statuses));
        queue.install_lane_manifests(&manifests);

        let registry = queue.lane_privacy_registry();
        assert!(
            registry
                .lane(LaneId::SINGLE)
                .expect("lane registry entry")
                .get(LaneCommitmentId::new(7))
                .is_some()
        );
    }

    #[allow(clippy::too_many_lines)]
    #[tokio::test]
    async fn lane_compliance_policy_blocks_transactions() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        #[cfg(feature = "telemetry")]
        let metrics = Arc::new(Metrics::default());
        #[cfg(feature = "telemetry")]
        let state = Arc::new(State::with_telemetry(
            world_with_test_domains(),
            kura.clone(),
            query_handle.clone(),
            StateTelemetry::new(metrics.clone(), true),
        ));
        #[cfg(not(feature = "telemetry"))]
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let (allowed_id, allowed_keypair) = gen_account_in("wonderland");
        let (denied_id, denied_keypair) = gen_account_in("wonderland");
        let (confidential_id, confidential_keypair) = gen_account_in("wonderland");

        // Build a Merkle commitment + witness for privacy-gated policies
        let mut leaves = Vec::new();
        leaves.extend_from_slice(&[0x01_u8; 32]);
        leaves.extend_from_slice(&[0x02_u8; 32]);
        let tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&leaves, 32).expect("valid chunk");
        let merkle_root = tree.root().expect("merkle root");
        let proof = tree.get_proof(0).expect("merkle proof");
        let first_leaf_hash: [u8; 32] = tree
            .leaves()
            .next()
            .expect("leaf hash")
            .as_ref()
            .as_ref()
            .try_into()
            .expect("hash length");
        let merkle_commitment = LanePrivacyCommitment::merkle(
            LaneCommitmentId::new(9),
            MerkleCommitment::new(merkle_root, 8),
        );

        let queue_inner = Queue::test(config_factory(), &time_source);
        let policy = LaneCompliancePolicy {
            id: LaneCompliancePolicyId::new(Hash::prehashed([0xAA; 32])),
            version: 1,
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
            jurisdiction: JurisdictionSet::default(),
            deny: vec![LaneComplianceRule {
                selector: ParticipantSelector {
                    account: Some(denied_id.clone()),
                    ..ParticipantSelector::default()
                },
                reason_code: Some("denied account".to_string()),
                jurisdiction_override: JurisdictionSet::default(),
            }],
            allow: vec![
                LaneComplianceRule {
                    selector: ParticipantSelector {
                        account: Some(allowed_id.clone()),
                        ..ParticipantSelector::default()
                    },
                    reason_code: Some("allowed account".to_string()),
                    jurisdiction_override: JurisdictionSet::default(),
                },
                LaneComplianceRule {
                    selector: ParticipantSelector {
                        account: Some(confidential_id.clone()),
                        privacy_commitments_any_of: vec![LaneCommitmentId::new(9)],
                        ..ParticipantSelector::default()
                    },
                    reason_code: Some("confidential allowed".to_string()),
                    jurisdiction_override: JurisdictionSet::default(),
                },
            ],
            transfer_limits: Vec::new(),
            audit_controls: AuditControls::default(),
            metadata: Metadata::default(),
        };
        let engine = LaneComplianceEngine::from_policies(vec![policy], false).expect("engine");
        *queue_inner.lane_compliance.write() = Some(Arc::new(engine));
        let queue = Arc::new(queue_inner);
        let mut statuses = BTreeMap::new();
        statuses.insert(
            LaneId::SINGLE,
            LaneManifestStatus {
                lane: LaneId::SINGLE,
                alias: "confidential".to_string(),
                dataspace: DataSpaceId::GLOBAL,
                visibility: LaneVisibility::Public,
                storage: LaneStorageProfile::CommitmentOnly,
                governance: None,
                manifest_path: Some(PathBuf::from("/tmp/privacy.json")),
                governance_rules: None,
                privacy_commitments: vec![merkle_commitment],
            },
        );
        let manifests = Arc::new(LaneManifestRegistry::from_statuses(statuses));
        queue.install_lane_manifests(&manifests);

        let denied_tx = accepted_tx_by(denied_id.clone(), &denied_keypair, &time_source);
        let err = queue
            .push(denied_tx, state.view())
            .expect_err("denied account should be rejected");
        assert!(matches!(err.err, Error::LaneComplianceDenied { .. }));

        let allowed_tx = accepted_tx_by(allowed_id.clone(), &allowed_keypair, &time_source);
        queue
            .push(allowed_tx, state.view())
            .expect("allowed account should pass");

        // Confidential account is denied without a privacy witness.
        let confidential_tx =
            accepted_tx_by(confidential_id.clone(), &confidential_keypair, &time_source);
        let err = queue
            .push(confidential_tx, state.view())
            .expect_err("missing privacy proof should be rejected");
        assert!(matches!(
            err.err,
            Error::LanePrivacyProofRejected { .. } | Error::LaneComplianceDenied { .. }
        ));

        // Attach a valid privacy proof to satisfy the policy.
        let proof_attachment = ProofAttachment {
            backend: Ident::from_str("halo2/ipa").expect("ident"),
            proof: ProofBox {
                backend: Ident::from_str("halo2/ipa").expect("ident"),
                bytes: vec![0xAA],
            },
            vk_ref: None,
            vk_inline: Some(VerifyingKeyBox::new(
                Ident::from_str("halo2/ipa").expect("ident"),
                vec![0xBB],
            )),
            vk_commitment: None,
            envelope_hash: None,
            lane_privacy: Some(LanePrivacyProof {
                commitment_id: LaneCommitmentId::new(9),
                witness: LanePrivacyWitness::Merkle(LanePrivacyMerkleWitness {
                    leaf: first_leaf_hash,
                    proof,
                }),
            }),
        };
        let attachments = ProofAttachmentList(vec![proof_attachment]);
        let confidential_tx = accepted_tx_with_attachments(
            confidential_id,
            &confidential_keypair,
            &time_source,
            vec![sample_unregister_instruction()],
            Metadata::default(),
            Some(attachments),
        );
        queue
            .push(confidential_tx, state.view())
            .expect("privacy proof should satisfy lane policy");
    }

    #[tokio::test]
    async fn governance_manifest_enforces_quorum_metadata() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        #[cfg(feature = "telemetry")]
        let metrics = Arc::new(Metrics::default());
        #[cfg(feature = "telemetry")]
        let state = Arc::new(State::with_telemetry(
            world_with_test_domains(),
            kura.clone(),
            query_handle.clone(),
            StateTelemetry::new(metrics.clone(), true),
        ));
        #[cfg(not(feature = "telemetry"))]
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Arc::new(Queue::test(config_factory(), &time_source));

        let (validator_primary, primary_keypair) = gen_account_in("wonderland");
        let (validator_secondary, _secondary_keypair) = gen_account_in("wonderland");

        let mut statuses = BTreeMap::new();
        let rules = GovernanceRules {
            validators: vec![validator_primary.clone(), validator_secondary.clone()],
            quorum: Some(2),
            ..GovernanceRules::default()
        };
        let status = LaneManifestStatus {
            lane: LaneId::SINGLE,
            alias: "gov".to_string(),
            dataspace: DataSpaceId::GLOBAL,
            visibility: LaneVisibility::Public,
            storage: LaneStorageProfile::FullReplica,
            governance: Some("parliament".to_string()),
            manifest_path: Some(PathBuf::from("/tmp/manifest.json")),
            governance_rules: Some(rules),
            privacy_commitments: Vec::new(),
        };
        statuses.insert(LaneId::SINGLE, status);
        let manifests = Arc::new(LaneManifestRegistry::from_statuses(statuses));
        queue.install_lane_manifests(&manifests);

        // Without additional approvals the quorum rule must reject the transaction.
        let tx = accepted_tx_by(validator_primary.clone(), &primary_keypair, &time_source);
        let err = queue
            .push(tx, state.view())
            .expect_err("quorum without approvals should reject");
        assert!(matches!(err.err, Error::GovernanceNotPermitted { .. }));
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_manifest_quorum_total
                .with_label_values(&["rejected"])
                .get(),
            1
        );
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_manifest_admission_total
                .with_label_values(&["quorum_rejected"])
                .get(),
            1
        );

        // Attach metadata listing the secondary validator so the quorum threshold is satisfied.
        let mut metadata = Metadata::default();
        metadata.insert(
            (*super::GOV_APPROVERS_METADATA_KEY).clone(),
            Json::new(vec![validator_secondary.to_string()]),
        );
        let tx = accepted_tx_with(
            validator_primary.clone(),
            &primary_keypair,
            &time_source,
            vec![sample_unregister_instruction()],
            metadata,
        );
        queue
            .push(tx, state.view())
            .expect("quorum satisfied via metadata approvals");
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_manifest_quorum_total
                .with_label_values(&["satisfied"])
                .get(),
            1
        );
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_manifest_admission_total
                .with_label_values(&["allowed"])
                .get(),
            1
        );
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn governance_manifest_enforces_protected_namespace_metadata() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        #[cfg(feature = "telemetry")]
        let metrics = Arc::new(Metrics::default());
        #[cfg(feature = "telemetry")]
        let state = Arc::new(State::with_telemetry(
            world_with_test_domains(),
            kura.clone(),
            query_handle.clone(),
            StateTelemetry::new(metrics.clone(), true),
        ));
        #[cfg(not(feature = "telemetry"))]
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Arc::new(Queue::test(config_factory(), &time_source));

        let (validator, keypair) = gen_account_in("wonderland");

        let mut protected = BTreeSet::new();
        protected.insert(Name::from_str("apps").expect("static namespace"));

        let mut statuses = BTreeMap::new();
        let rules = GovernanceRules {
            validators: vec![validator.clone()],
            protected_namespaces: protected,
            ..GovernanceRules::default()
        };
        let status = LaneManifestStatus {
            lane: LaneId::SINGLE,
            alias: "gov".to_string(),
            dataspace: DataSpaceId::GLOBAL,
            visibility: LaneVisibility::Public,
            storage: LaneStorageProfile::FullReplica,
            governance: Some("parliament".to_string()),
            manifest_path: Some(PathBuf::from("/tmp/manifest.json")),
            governance_rules: Some(rules),
            privacy_commitments: Vec::new(),
        };
        statuses.insert(LaneId::SINGLE, status);
        let manifests = Arc::new(LaneManifestRegistry::from_statuses(statuses));
        queue.install_lane_manifests(&manifests);

        // Metadata referencing an unknown namespace must be rejected.
        let mut metadata = Metadata::default();
        metadata.insert(
            (*super::GOV_NAMESPACE_METADATA_KEY).clone(),
            Json::new("system"),
        );
        metadata.insert(
            (*super::GOV_CONTRACT_ID_METADATA_KEY).clone(),
            Json::new("calc.v1"),
        );
        let tx = accepted_tx_with(
            validator.clone(),
            &keypair,
            &time_source,
            vec![sample_unregister_instruction()],
            metadata,
        );
        let err = queue
            .push(tx, state.view())
            .expect_err("namespace outside manifest must reject");
        assert!(matches!(err.err, Error::GovernanceNotPermitted { .. }));
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_protected_namespace_total
                .with_label_values(&["rejected"])
                .get(),
            1
        );
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_manifest_admission_total
                .with_label_values(&["protected_namespace_rejected"])
                .get(),
            1
        );

        // Namespace within the manifest but missing contract id must be rejected.
        let mut metadata_missing_cid = Metadata::default();
        metadata_missing_cid.insert(
            (*super::GOV_NAMESPACE_METADATA_KEY).clone(),
            Json::new("apps"),
        );
        let tx = accepted_tx_with(
            validator.clone(),
            &keypair,
            &time_source,
            vec![sample_unregister_instruction()],
            metadata_missing_cid,
        );
        let err = queue
            .push(tx, state.view())
            .expect_err("missing contract id metadata must reject");
        assert!(matches!(err.err, Error::GovernanceNotPermitted { .. }));
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_protected_namespace_total
                .with_label_values(&["rejected"])
                .get(),
            2
        );
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_manifest_admission_total
                .with_label_values(&["protected_namespace_rejected"])
                .get(),
            2
        );

        // Correct namespace metadata with contract id should be accepted.
        let mut valid_metadata = Metadata::default();
        valid_metadata.insert(
            (*super::GOV_NAMESPACE_METADATA_KEY).clone(),
            Json::new("apps"),
        );
        valid_metadata.insert(
            (*super::GOV_CONTRACT_ID_METADATA_KEY).clone(),
            Json::new("calc.v1"),
        );
        let tx = accepted_tx_with(
            validator.clone(),
            &keypair,
            &time_source,
            vec![sample_unregister_instruction()],
            valid_metadata,
        );
        queue
            .push(tx, state.view())
            .expect("protected namespace metadata satisfied");
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_protected_namespace_total
                .with_label_values(&["allowed"])
                .get(),
            1
        );
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_manifest_admission_total
                .with_label_values(&["allowed"])
                .get(),
            1
        );
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn governance_manifest_requires_metadata_for_contract_namespace_ops() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        #[cfg(feature = "telemetry")]
        let metrics = Arc::new(Metrics::default());
        #[cfg(feature = "telemetry")]
        let state = Arc::new(State::with_telemetry(
            world_with_test_domains(),
            kura.clone(),
            query_handle.clone(),
            StateTelemetry::new(metrics.clone(), true),
        ));
        #[cfg(not(feature = "telemetry"))]
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        let (validator, keypair) = gen_account_in("wonderland");

        let mut protected = BTreeSet::new();
        protected.insert(Name::from_str("apps").expect("static namespace"));

        let mut statuses = BTreeMap::new();
        let rules = GovernanceRules {
            validators: vec![validator.clone()],
            protected_namespaces: protected,
            ..GovernanceRules::default()
        };
        let status = LaneManifestStatus {
            lane: LaneId::SINGLE,
            alias: "gov".to_string(),
            dataspace: DataSpaceId::GLOBAL,
            visibility: LaneVisibility::Public,
            storage: LaneStorageProfile::FullReplica,
            governance: Some("parliament".to_string()),
            manifest_path: Some(PathBuf::from("/tmp/manifest.json")),
            governance_rules: Some(rules),
            privacy_commitments: Vec::new(),
        };
        statuses.insert(LaneId::SINGLE, status);
        let manifests = Arc::new(LaneManifestRegistry::from_statuses(statuses));
        queue.install_lane_manifests(&manifests);

        let code_hash = iroha_crypto::Hash::new(b"demo");
        let activate = InstructionBox::from(ActivateContractInstance {
            namespace: "apps".to_string(),
            contract_id: "calc.v1".to_string(),
            code_hash,
        });

        // Missing metadata must reject when touching a protected namespace.
        let tx = accepted_tx_with(
            validator.clone(),
            &keypair,
            &time_source,
            vec![activate.clone()],
            Metadata::default(),
        );
        let err = queue
            .push(tx, state.view())
            .expect_err("protected namespace operations require governance metadata");
        assert!(matches!(err.err, Error::GovernanceNotPermitted { .. }));
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_protected_namespace_total
                .with_label_values(&["rejected"])
                .get(),
            1
        );

        // Metadata namespace present but contract_id mismatched should reject.
        let mut metadata_mismatch = Metadata::default();
        metadata_mismatch.insert(
            (*super::GOV_NAMESPACE_METADATA_KEY).clone(),
            Json::new("apps"),
        );
        metadata_mismatch.insert(
            (*super::GOV_CONTRACT_ID_METADATA_KEY).clone(),
            Json::new("other"),
        );
        let tx = accepted_tx_with(
            validator.clone(),
            &keypair,
            &time_source,
            vec![activate.clone()],
            metadata_mismatch,
        );
        let err = queue
            .push(tx, state.view())
            .expect_err("contract_id mismatch must reject");
        assert!(matches!(err.err, Error::GovernanceNotPermitted { .. }));
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_protected_namespace_total
                .with_label_values(&["rejected"])
                .get(),
            2
        );

        // Matching metadata should allow the transaction.
        let mut metadata_ok = Metadata::default();
        metadata_ok.insert(
            (*super::GOV_NAMESPACE_METADATA_KEY).clone(),
            Json::new("apps"),
        );
        metadata_ok.insert(
            (*super::GOV_CONTRACT_ID_METADATA_KEY).clone(),
            Json::new("calc.v1"),
        );
        let tx = accepted_tx_with(
            validator.clone(),
            &keypair,
            &time_source,
            vec![activate],
            metadata_ok,
        );
        queue
            .push(tx, state.view())
            .expect("matching metadata must allow");
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_protected_namespace_total
                .with_label_values(&["allowed"])
                .get(),
            1
        );

        // Contract artifact instructions must also carry governance metadata.
        let (code_hash, code) = minimal_contract_bytes();
        let register_bytes = InstructionBox::from(RegisterSmartContractBytes {
            code_hash,
            code: code.clone(),
        });

        let tx = accepted_tx_with(
            validator.clone(),
            &keypair,
            &time_source,
            vec![register_bytes.clone()],
            Metadata::default(),
        );
        let err = queue
            .push(tx, state.view())
            .expect_err("contract artifact registration requires governance metadata");
        assert!(matches!(err.err, Error::GovernanceNotPermitted { .. }));

        let mut metadata_bytes = Metadata::default();
        metadata_bytes.insert(
            (*super::GOV_NAMESPACE_METADATA_KEY).clone(),
            Json::new("apps"),
        );
        metadata_bytes.insert(
            (*super::GOV_CONTRACT_ID_METADATA_KEY).clone(),
            Json::new("calc.v1"),
        );
        let tx = accepted_tx_with(
            validator.clone(),
            &keypair,
            &time_source,
            vec![register_bytes],
            metadata_bytes,
        );
        queue
            .push(tx, state.view())
            .expect("contract artifact registration metadata satisfied");
    }

    #[tokio::test]
    async fn governance_manifest_rejects_cross_namespace_rebind() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let mut world = world_with_test_domains();
        world.contract_instances.insert(
            ("apps".to_string(), "calc.v1".to_string()),
            Hash::new(b"demo"),
        );
        let state = Arc::new(State::new(world, kura.clone(), query_handle.clone()));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        let (validator, keypair) = gen_account_in("wonderland");

        let mut protected = BTreeSet::new();
        protected.insert(Name::from_str("apps").expect("static namespace"));
        protected.insert(Name::from_str("ops").expect("static namespace"));

        let mut statuses = BTreeMap::new();
        let rules = GovernanceRules {
            validators: vec![validator.clone()],
            protected_namespaces: protected,
            ..GovernanceRules::default()
        };
        let status = LaneManifestStatus {
            lane: LaneId::SINGLE,
            alias: "gov".to_string(),
            dataspace: DataSpaceId::GLOBAL,
            visibility: LaneVisibility::Public,
            storage: LaneStorageProfile::FullReplica,
            governance: Some("parliament".to_string()),
            manifest_path: Some(PathBuf::from("/tmp/manifest.json")),
            governance_rules: Some(rules),
            privacy_commitments: Vec::new(),
        };
        statuses.insert(LaneId::SINGLE, status);
        let manifests = Arc::new(LaneManifestRegistry::from_statuses(statuses));
        queue.install_lane_manifests(&manifests);

        let code_hash = iroha_crypto::Hash::new(b"demo");
        let activate = InstructionBox::from(ActivateContractInstance {
            namespace: "ops".to_string(),
            contract_id: "calc.v1".to_string(),
            code_hash,
        });

        let mut metadata = Metadata::default();
        metadata.insert(
            (*super::GOV_NAMESPACE_METADATA_KEY).clone(),
            Json::new("ops"),
        );
        metadata.insert(
            (*super::GOV_CONTRACT_ID_METADATA_KEY).clone(),
            Json::new("calc.v1"),
        );

        let tx = accepted_tx_with(
            validator.clone(),
            &keypair,
            &time_source,
            vec![activate],
            metadata,
        );
        let err = queue
            .push(tx, state.view())
            .expect_err("cross-namespace rebinding must be rejected");
        assert!(matches!(err.err, Error::GovernanceNotPermitted { .. }));
    }

    #[tokio::test]
    async fn governance_manifest_runtime_upgrade_hook_blocks_when_disabled() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        #[cfg(feature = "telemetry")]
        let metrics = Arc::new(Metrics::default());
        #[cfg(feature = "telemetry")]
        let state = Arc::new(State::with_telemetry(
            world_with_test_domains(),
            kura.clone(),
            query_handle.clone(),
            StateTelemetry::new(metrics.clone(), true),
        ));
        #[cfg(not(feature = "telemetry"))]
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        let (validator, keypair) = gen_account_in("wonderland");

        let mut statuses = BTreeMap::new();
        let rules = GovernanceRules {
            hooks: GovernanceHooks {
                runtime_upgrade: Some(RuntimeUpgradeHook {
                    allow: false,
                    require_metadata: false,
                    metadata_key: None,
                    allowed_ids: None,
                }),
                ..GovernanceHooks::default()
            },
            ..GovernanceRules::default()
        };
        let status = LaneManifestStatus {
            lane: LaneId::SINGLE,
            alias: "upgrade".to_string(),
            dataspace: DataSpaceId::GLOBAL,
            visibility: LaneVisibility::Public,
            storage: LaneStorageProfile::FullReplica,
            governance: Some("parliament".to_string()),
            manifest_path: Some(PathBuf::from("/tmp/manifest.json")),
            governance_rules: Some(rules),
            privacy_commitments: Vec::new(),
        };
        statuses.insert(LaneId::SINGLE, status);
        let manifests = Arc::new(LaneManifestRegistry::from_statuses(statuses));
        queue.install_lane_manifests(&manifests);

        let tx = accepted_tx_with(
            validator.clone(),
            &keypair,
            &time_source,
            vec![runtime_upgrade_instruction()],
            Metadata::default(),
        );
        let err = queue
            .push(tx, state.view())
            .expect_err("runtime upgrade instructions must be rejected when hook is disabled");
        assert!(matches!(err.err, Error::GovernanceNotPermitted { .. }));
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_manifest_hook_total
                .with_label_values(&["runtime_upgrade", "rejected"])
                .get(),
            1
        );
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_manifest_admission_total
                .with_label_values(&["runtime_hook_rejected"])
                .get(),
            1
        );
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn governance_manifest_runtime_upgrade_hook_requires_metadata() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        #[cfg(feature = "telemetry")]
        let metrics = Arc::new(Metrics::default());
        #[cfg(feature = "telemetry")]
        let state = Arc::new(State::with_telemetry(
            world_with_test_domains(),
            kura.clone(),
            query_handle.clone(),
            StateTelemetry::new(metrics.clone(), true),
        ));
        #[cfg(not(feature = "telemetry"))]
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        let (validator, keypair) = gen_account_in("wonderland");

        let mut statuses = BTreeMap::new();
        let metadata_key = Name::from_str("gov_upgrade_id").expect("static metadata key");
        let mut allowed_ids = BTreeSet::new();
        allowed_ids.insert(RUNTIME_UPGRADE_ALLOWED_ID.to_string());
        let rules = GovernanceRules {
            hooks: GovernanceHooks {
                runtime_upgrade: Some(RuntimeUpgradeHook {
                    allow: true,
                    require_metadata: true,
                    metadata_key: Some(metadata_key.clone()),
                    allowed_ids: Some(allowed_ids),
                }),
                ..GovernanceHooks::default()
            },
            ..GovernanceRules::default()
        };
        let status = LaneManifestStatus {
            lane: LaneId::SINGLE,
            alias: "upgrade".to_string(),
            dataspace: DataSpaceId::GLOBAL,
            visibility: LaneVisibility::Public,
            storage: LaneStorageProfile::FullReplica,
            governance: Some("parliament".to_string()),
            manifest_path: Some(PathBuf::from("/tmp/manifest.json")),
            governance_rules: Some(rules),
            privacy_commitments: Vec::new(),
        };
        statuses.insert(LaneId::SINGLE, status);
        let manifests = Arc::new(LaneManifestRegistry::from_statuses(statuses));
        queue.install_lane_manifests(&manifests);

        let tx_missing_metadata = accepted_tx_with(
            validator.clone(),
            &keypair,
            &time_source,
            vec![runtime_upgrade_instruction()],
            Metadata::default(),
        );
        let err = queue
            .push(tx_missing_metadata, state.view())
            .expect_err("hook requires metadata and should reject when absent");
        assert!(matches!(err.err, Error::GovernanceNotPermitted { .. }));
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_manifest_hook_total
                .with_label_values(&["runtime_upgrade", "rejected"])
                .get(),
            1
        );
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_manifest_admission_total
                .with_label_values(&["runtime_hook_rejected"])
                .get(),
            1
        );

        let mut wrong_metadata = Metadata::default();
        wrong_metadata.insert(metadata_key.clone(), Json::new("not-allowed"));
        let tx_wrong = accepted_tx_with(
            validator.clone(),
            &keypair,
            &time_source,
            vec![runtime_upgrade_instruction()],
            wrong_metadata,
        );
        let err = queue
            .push(tx_wrong, state.view())
            .expect_err("hook must reject metadata values outside allowlist");
        assert!(matches!(err.err, Error::GovernanceNotPermitted { .. }));
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_manifest_hook_total
                .with_label_values(&["runtime_upgrade", "rejected"])
                .get(),
            2
        );
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_manifest_admission_total
                .with_label_values(&["runtime_hook_rejected"])
                .get(),
            2
        );

        let mut valid_metadata = Metadata::default();
        valid_metadata.insert(metadata_key.clone(), Json::new(RUNTIME_UPGRADE_ALLOWED_ID));
        let tx_valid = accepted_tx_with(
            validator.clone(),
            &keypair,
            &time_source,
            vec![runtime_upgrade_instruction()],
            valid_metadata,
        );
        queue
            .push(tx_valid, state.view())
            .expect("hook should allow runtime upgrade with approved metadata");
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_manifest_hook_total
                .with_label_values(&["runtime_upgrade", "allowed"])
                .get(),
            1
        );
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_manifest_admission_total
                .with_label_values(&["allowed"])
                .get(),
            1
        );
    }

    fn accepted_tx_by_someone(time_source: &TimeSource) -> AcceptedTransaction<'static> {
        let (account_id, key_pair) = gen_account_in("wonderland");
        accepted_tx_by(account_id, &key_pair, time_source)
    }

    #[test]
    fn compute_tx_encoded_len_matches_payload() {
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let tx = accepted_tx_by_someone(&time_source);
        let expected = tx.as_ref().encode().len();
        assert_eq!(Queue::compute_tx_encoded_len(&tx), expected);
    }

    #[test]
    fn push_wakes_sumeragi_when_configured() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Queue::test(config_factory(), &time_source);
        let (wake_tx, wake_rx) = std::sync::mpsc::sync_channel(1);
        queue.set_sumeragi_wake(wake_tx);

        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("push should succeed");

        assert!(matches!(wake_rx.try_recv(), Ok(())));
    }

    #[test]
    fn queued_tx_metadata_cached_in_guard_and_cleared_on_drop() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world_with_test_domains(), kura, query_handle);
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        let tx = accepted_tx_by_someone(&time_source);
        let encoded_len = tx.as_ref().encoded_len();
        let expected_gas = match tx.as_ref().instructions() {
            iroha_data_model::transaction::Executable::Instructions(batch) => {
                crate::gas::meter_instructions(batch.as_ref())
            }
            iroha_data_model::transaction::Executable::Ivm(_) => {
                panic!("expected ISI transaction for gas test")
            }
            iroha_data_model::transaction::Executable::IvmProved(_) => {
                panic!("expected ISI transaction for gas test")
            }
        };

        queue.push(tx, state.view()).expect("push tx");

        let state_view = state.view();
        let mut expired = Vec::new();
        let guard = queue
            .pop_from_queue(&state_view, &mut expired)
            .expect("guard");
        assert_eq!(guard.encoded_len(), encoded_len);
        assert_eq!(guard.gas_cost(), expected_gas);
        drop(guard);

        assert!(
            queue.tx_encoded_len.is_empty(),
            "encoded length cache should clear after guard drop"
        );
        assert!(
            queue.tx_gas_cost.is_empty(),
            "gas cost cache should clear after guard drop"
        );
    }

    #[test]
    fn queue_metadata_cleared_on_commit_and_clear_all() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world_with_test_domains(), kura, query_handle);
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Queue::test(config_factory(), &time_source);
        let tx = accepted_tx_by_someone(&time_source);
        let hash = tx.as_ref().hash();
        queue.push(tx, state.view()).expect("push tx");
        let _ = queue.gossip_batch(1, &state.view());
        assert!(!queue.tx_gossip_payloads.is_empty());

        let removed = queue.remove_committed_hashes(std::iter::once(hash), None);
        assert_eq!(removed, 1);
        assert!(queue.tx_encoded_len.is_empty());
        assert!(queue.tx_gas_cost.is_empty());
        assert!(queue.tx_gossip_payloads.is_empty());

        let tx = accepted_tx_by_someone(&time_source);
        queue.push(tx, state.view()).expect("push tx");
        assert!(!queue.tx_encoded_len.is_empty());
        let _ = queue.gossip_batch(1, &state.view());
        assert!(!queue.tx_gossip_payloads.is_empty());
        queue.clear_all();
        assert!(queue.tx_encoded_len.is_empty());
        assert!(queue.tx_gas_cost.is_empty());
        assert!(queue.tx_gossip_payloads.is_empty());
    }

    #[test]
    fn queue_accepts_gossip_payload_cache() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world_with_test_domains(), kura, query_handle);
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Queue::test(config_factory(), &time_source);
        let tx = accepted_tx_by_someone(&time_source);
        let hash = tx.as_ref().hash();
        let payload = Arc::new(tx.as_ref().encode());

        queue
            .push_with_gossip_payload(tx, state.view(), Some(Arc::clone(&payload)))
            .expect("push tx with payload");

        let stored_payload = queue.tx_gossip_payloads.get(&hash).expect("payload stored");
        assert_eq!(stored_payload.as_slice(), payload.as_slice());

        let encoded_len = queue
            .tx_encoded_len
            .get(&hash)
            .map(|entry| *entry.value())
            .expect("encoded len stored");
        assert_eq!(encoded_len, payload.len());

        let batch = queue.gossip_batch(1, &state.view());
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].payload.as_slice(), payload.as_slice());
    }

    #[test]
    fn queue_accepts_gossip_payload_cache_in_shared_view() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world_with_test_domains(), kura, query_handle);
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Queue::test(config_factory(), &time_source);
        let tx = accepted_tx_by_someone(&time_source);
        let hash = tx.as_ref().hash();
        let payload = Arc::new(tx.as_ref().encode());
        let state_view = state.view();

        queue
            .push_with_gossip_payload_in_view(tx, &state_view, Some(Arc::clone(&payload)))
            .expect("push tx with payload through shared view");

        let stored_payload = queue.tx_gossip_payloads.get(&hash).expect("payload stored");
        assert_eq!(stored_payload.as_slice(), payload.as_slice());
    }

    #[test]
    fn push_in_view_accepts_multiple_transactions_with_shared_snapshot() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world_with_test_domains(), kura, query_handle);
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Queue::test(config_factory(), &time_source);
        let state_view = state.view();
        queue
            .push_in_view(accepted_tx_by_someone(&time_source), &state_view)
            .expect("first push");
        queue
            .push_in_view(accepted_tx_by_someone(&time_source), &state_view)
            .expect("second push");

        assert_eq!(queue.queued_len(), 2);
    }

    #[test]
    fn contains_pending_hash_ignores_committed_entries() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world_with_test_domains(), kura, query_handle);
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Queue::test(config_factory(), &time_source);
        let tx = accepted_tx_by_someone(&time_source);
        let hash = tx.as_ref().hash();
        queue.push(tx, state.view()).expect("push tx");
        assert!(queue.contains_pending_hash(hash, &state));

        {
            let mut transactions = state.transactions.block();
            transactions.insert_block_with_single_tx(hash, nonzero!(1_usize));
            transactions
                .commit()
                .expect("transactions block should commit");
        }

        assert!(!queue.contains_pending_hash(hash, &state));
    }

    #[test]
    fn gossip_batch_with_state_skips_committed_entries() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world_with_test_domains(), kura, query_handle);
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Queue::test(config_factory(), &time_source);
        let tx = accepted_tx_by_someone(&time_source);
        let hash = tx.as_ref().hash();
        queue.push(tx, state.view()).expect("push tx");
        {
            let mut transactions = state.transactions.block();
            transactions.insert_block_with_single_tx(hash, nonzero!(1_usize));
            transactions
                .commit()
                .expect("transactions block should commit");
        }

        let batch = queue.gossip_batch_with_state(1, &state);
        assert!(
            batch.is_empty(),
            "committed transaction must not be selected for gossip"
        );
    }

    #[tokio::test]
    async fn push_rejects_without_governance_manifest() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        #[cfg(feature = "telemetry")]
        let metrics = Arc::new(Metrics::default());
        #[cfg(feature = "telemetry")]
        let state = Arc::new(State::with_telemetry(
            world_with_test_domains(),
            kura.clone(),
            query_handle.clone(),
            StateTelemetry::new(metrics.clone(), true),
        ));
        #[cfg(not(feature = "telemetry"))]
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        let mut statuses = BTreeMap::new();
        statuses.insert(
            LaneId::SINGLE,
            LaneManifestStatus {
                lane: LaneId::SINGLE,
                alias: "default".to_string(),
                dataspace: DataSpaceId::GLOBAL,
                visibility: LaneVisibility::Public,
                storage: LaneStorageProfile::FullReplica,
                governance: Some("parliament".to_string()),
                manifest_path: None,
                governance_rules: None,
                privacy_commitments: Vec::new(),
            },
        );
        let manifests = Arc::new(LaneManifestRegistry::from_statuses(statuses));
        queue.install_lane_manifests(&manifests);

        let result = queue.push(accepted_tx_by_someone(&time_source), state.view());
        assert!(matches!(
            result,
            Err(Failure {
                err: Error::Governance(_),
                ..
            })
        ));
        #[cfg(feature = "telemetry")]
        assert_eq!(
            metrics
                .governance_manifest_admission_total
                .with_label_values(&["missing_manifest"])
                .get(),
            1
        );
    }

    #[tokio::test]
    async fn uaid_without_dataspace_binding_is_rejected() {
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::missing-binding"));
        let dataspace = DataSpaceId::new(7);
        let (world, account_id, key_pair) = world_with_uaid_account(uaid, dataspace, false);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        #[cfg(feature = "telemetry")]
        let metrics = Arc::new(Metrics::default());
        #[cfg(feature = "telemetry")]
        let state = Arc::new(State::with_telemetry(
            world,
            kura.clone(),
            query_handle.clone(),
            StateTelemetry::new(metrics.clone(), true),
        ));
        #[cfg(not(feature = "telemetry"))]
        let state = Arc::new(State::new(world, kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let router: Arc<dyn LaneRouter> = Arc::new(StaticRouter {
            lane: LaneId::SINGLE,
            dataspace,
        });
        let queue = Arc::new(Queue::test_with_router(
            config_factory(),
            &time_source,
            router.clone(),
        ));

        let mut statuses = BTreeMap::new();
        statuses.insert(
            LaneId::SINGLE,
            LaneManifestStatus {
                lane: LaneId::SINGLE,
                alias: "uaid-enforcement".to_string(),
                dataspace,
                visibility: LaneVisibility::Public,
                storage: LaneStorageProfile::FullReplica,
                governance: None,
                manifest_path: None,
                governance_rules: None,
                privacy_commitments: Vec::new(),
            },
        );
        let manifests = Arc::new(LaneManifestRegistry::from_statuses(statuses));
        queue.install_lane_manifests(&manifests);

        let err = queue
            .push(
                accepted_tx_by(account_id.clone(), &key_pair, &time_source),
                state.view(),
            )
            .expect_err("UAID without a dataspace binding must be rejected");
        match err.err {
            Error::UaidNotBound {
                uaid: rejected,
                dataspace: rejected_ds,
            } => {
                assert_eq!(rejected, uaid);
                assert_eq!(rejected_ds, dataspace);
            }
            other => panic!("expected UaidNotBound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn uaid_binding_allows_lane_identity_extraction() {
        let dataspace = DataSpaceId::new(11);
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::bound"));
        let (world, account_id, key_pair) = world_with_uaid_account(uaid, dataspace, true);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        #[cfg(feature = "telemetry")]
        let metrics = Arc::new(Metrics::default());
        #[cfg(feature = "telemetry")]
        let state = Arc::new(State::with_telemetry(
            world,
            kura.clone(),
            query_handle.clone(),
            StateTelemetry::new(metrics.clone(), true),
        ));
        #[cfg(not(feature = "telemetry"))]
        let state = Arc::new(State::new(world, kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let router: Arc<dyn LaneRouter> = Arc::new(StaticRouter {
            lane: LaneId::SINGLE,
            dataspace,
        });
        let queue = Arc::new(Queue::test_with_router(
            config_factory(),
            &time_source,
            router.clone(),
        ));

        let mut statuses = BTreeMap::new();
        statuses.insert(
            LaneId::SINGLE,
            LaneManifestStatus {
                lane: LaneId::SINGLE,
                alias: "uaid-binding".to_string(),
                dataspace,
                visibility: LaneVisibility::Public,
                storage: LaneStorageProfile::FullReplica,
                governance: None,
                manifest_path: None,
                governance_rules: None,
                privacy_commitments: Vec::new(),
            },
        );
        let manifests = Arc::new(LaneManifestRegistry::from_statuses(statuses));
        queue.install_lane_manifests(&manifests);

        queue
            .push(
                accepted_tx_by(account_id.clone(), &key_pair, &time_source),
                state.view(),
            )
            .expect("UAID with active dataspace binding should be admitted");
    }

    #[tokio::test]
    async fn uaid_binding_enforced_for_target_dataspace() {
        let bound = DataSpaceId::new(42);
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::rebind"));
        let (world, account_id, key_pair) = world_with_uaid_account(uaid, bound, true);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world, kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let target = DataSpaceId::GLOBAL;
        let queue = Queue::test_with_router(
            config_factory(),
            &time_source,
            Arc::new(StaticRouter {
                lane: LaneId::SINGLE,
                dataspace: target,
            }),
        );

        let result = queue.push(
            accepted_tx_by(account_id.clone(), &key_pair, &time_source),
            state.view(),
        );

        match result {
            Err(Failure {
                err:
                    Error::UaidNotBound {
                        uaid: rejected,
                        dataspace,
                    },
                ..
            }) => {
                assert_eq!(rejected, uaid);
                assert_eq!(dataspace, target);
            }
            other => panic!("expected UAID binding rejection, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn uaid_binding_allows_matching_dataspace() {
        let dataspace = DataSpaceId::new(24);
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::aligned"));
        let (world, account_id, key_pair) = world_with_uaid_account(uaid, dataspace, true);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world, kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Queue::test_with_router(
            config_factory(),
            &time_source,
            Arc::new(StaticRouter {
                lane: LaneId::SINGLE,
                dataspace,
            }),
        );

        queue
            .push(
                accepted_tx_by(account_id.clone(), &key_pair, &time_source),
                state.view(),
            )
            .expect("UAID bound to dataspace should be admitted");
    }

    fn minimal_contract_bytes() -> (iroha_crypto::Hash, Vec<u8>) {
        let mut program = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: 0,
            vector_length: 0,
            max_cycles: 1,
            abi_version: 1,
        }
        .encode();
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let parsed = ProgramMetadata::parse(&program).expect("parse minimal program");
        let code_hash = iroha_crypto::Hash::new(&program[parsed.header_len..]);
        (code_hash, program)
    }

    fn sample_unregister_instruction() -> InstructionBox {
        let domain_name = format!("dummy{}", rand::random::<u64>());
        InstructionBox::from(Unregister::domain(domain_name.parse().unwrap()))
    }

    const RUNTIME_UPGRADE_ALLOWED_ID: &str = "upgrade-q1";

    fn sample_runtime_upgrade_manifest_bytes() -> Vec<u8> {
        RuntimeUpgradeManifest {
            name: "upgrade.v1.test".to_string(),
            description: "test upgrade for runtime hook enforcement (v1)".to_string(),
            abi_version: 1,
            abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
            added_syscalls: vec![],
            added_pointer_types: vec![],
            start_height: 42,
            end_height: 84,
            sbom_digests: Vec::new(),
            slsa_attestation: Vec::new(),
            provenance: Vec::new(),
        }
        .canonical_bytes()
    }

    fn runtime_upgrade_instruction() -> InstructionBox {
        InstructionBox::from(ProposeRuntimeUpgrade {
            manifest_bytes: sample_runtime_upgrade_manifest_bytes(),
        })
    }

    fn accepted_tx_by(
        account_id: AccountId,
        key_pair: &KeyPair,
        time_source: &TimeSource,
    ) -> AcceptedTransaction<'static> {
        let instructions = vec![sample_unregister_instruction()];
        accepted_tx_with(
            account_id,
            key_pair,
            time_source,
            instructions,
            Metadata::default(),
        )
    }

    fn accepted_tx_with(
        account_id: AccountId,
        key_pair: &KeyPair,
        time_source: &TimeSource,
        instructions: Vec<InstructionBox>,
        metadata: Metadata,
    ) -> AcceptedTransaction<'static> {
        accepted_tx_with_attachments(
            account_id,
            key_pair,
            time_source,
            instructions,
            metadata,
            None,
        )
    }

    fn accepted_tx_with_attachments(
        account_id: AccountId,
        key_pair: &KeyPair,
        time_source: &TimeSource,
        instructions: Vec<InstructionBox>,
        metadata: Metadata,
        attachments: Option<ProofAttachmentList>,
    ) -> AcceptedTransaction<'static> {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
        let mut builder =
            TransactionBuilder::new_with_time_source(chain_id.clone(), account_id, time_source)
                .with_instructions(instructions)
                .with_metadata(metadata);
        if let Some(att) = attachments {
            builder = builder.with_attachments(att);
        }
        let tx = builder.sign(key_pair.private_key());
        let default_limits = TransactionParameters::default();
        let tx_limits = TransactionParameters::with_max_signatures(
            nonzero!(16_u64),
            nonzero!(4096_u64),
            nonzero!(1024_u64),
            default_limits.max_tx_bytes(),
            default_limits.max_decompressed_bytes(),
            default_limits.max_metadata_depth(),
        );
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        AcceptedTransaction::accept_with_time_source(
            tx,
            &chain_id,
            Duration::from_millis(10),
            tx_limits,
            &crypto_cfg,
            time_source,
        )
        .expect("Failed to accept Transaction.")
    }

    #[cfg(feature = "telemetry")]
    fn accepted_ivm_tx_by(
        account_id: AccountId,
        key_pair: &KeyPair,
        time_source: &TimeSource,
        max_cycles: u64,
    ) -> AcceptedTransaction<'static> {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
        let program = minimal_ivm_program_with_max_cycles(1, max_cycles);
        let mut metadata = Metadata::default();
        metadata.insert(
            "gas_limit".parse().expect("gas_limit key"),
            iroha_primitives::json::Json::new(crate::smartcontracts::ivm::gas_limit_for_cycles(
                max_cycles,
            )),
        );
        let tx =
            TransactionBuilder::new_with_time_source(chain_id.clone(), account_id, time_source)
                .with_metadata(metadata)
                .with_executable(Executable::Ivm(IvmBytecode::from_compiled(program)))
                .sign(key_pair.private_key());
        let default_limits = TransactionParameters::default();
        let tx_limits = TransactionParameters::with_max_signatures(
            nonzero!(16_u64),
            nonzero!(4096_u64),
            nonzero!(1024_u64),
            default_limits.max_tx_bytes(),
            default_limits.max_decompressed_bytes(),
            default_limits.max_metadata_depth(),
        );
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        AcceptedTransaction::accept_with_time_source(
            tx,
            &chain_id,
            Duration::from_millis(10),
            tx_limits,
            &crypto_cfg,
            time_source,
        )
        .expect("Failed to accept IVM transaction.")
    }

    #[cfg(feature = "telemetry")]
    fn minimal_ivm_program_with_max_cycles(abi_version: u8, max_cycles: u64) -> Vec<u8> {
        const IVM_MAGIC: [u8; 4] = *b"IVM\0";
        const HEADER_SUFFIX: [u8; 4] = [1, 0, 0, 4];

        let mut program = Vec::new();
        program.extend_from_slice(&IVM_MAGIC);
        program.extend_from_slice(&HEADER_SUFFIX);
        program.extend_from_slice(&max_cycles.to_le_bytes());
        program.push(abi_version);
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        program
    }

    /// Build a minimal world with a single domain and account for tests.
    pub fn world_with_test_domains() -> World {
        let domain_id = "wonderland".parse().expect("Valid");
        let (account_id, _account_keypair) = gen_account_in("wonderland");
        let domain = Domain::new(domain_id).build(&account_id);
        let account = Account::new(account_id.clone()).build(&account_id);
        World::with([domain], [account], [])
    }

    #[test]
    fn gossip_batch_returns_routing_metadata() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let queue = Queue::test(config_factory(), &time_source);

        let tx = accepted_tx_by_someone(&time_source);
        let hash = tx.as_ref().hash();
        queue
            .push(tx, state.view())
            .expect("enqueue accepted transaction");

        let batch = queue.gossip_batch(1, &state.view());
        assert_eq!(batch.len(), 1);
        let entry = &batch[0];
        assert_eq!(entry.tx.as_ref().hash(), hash);
        let expected_payload = entry.tx.as_ref().encode();
        assert_eq!(entry.payload.as_slice(), expected_payload.as_slice());
        assert_eq!(entry.routing.lane_id, LaneId::SINGLE);
        assert_eq!(entry.routing.dataspace_id, DataSpaceId::GLOBAL);
    }

    #[test]
    fn route_for_gossip_uses_router_decision() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let expected_lane = LaneId::new(2);
        let expected_dataspace = DataSpaceId::new(7);
        let queue = Queue::test_with_router(
            config_factory(),
            &time_source,
            Arc::new(StaticRouter {
                lane: expected_lane,
                dataspace: expected_dataspace,
            }),
        );

        let tx = accepted_tx_by_someone(&time_source);
        let state_view = state.view();
        let routing = queue.route_for_gossip(&tx, &state_view);

        assert_eq!(routing.lane_id, expected_lane);
        assert_eq!(routing.dataspace_id, expected_dataspace);
    }

    fn config_factory() -> Config {
        Config {
            transaction_time_to_live: Duration::from_secs(100),
            capacity: 100.try_into().unwrap(),
            ..Config::default()
        }
    }

    fn world_with_uaid_account(
        uaid: UniversalAccountId,
        dataspace: DataSpaceId,
        bind_manifest: bool,
    ) -> (World, AccountId, KeyPair) {
        let (account_id, key_pair) = gen_account_in("wonderland");
        let domain = Domain::new(account_id.domain().clone()).build(&account_id);
        let account = Account::new(account_id.clone())
            .with_uaid(Some(uaid))
            .build(&account_id);

        let mut world = World::with([domain], [account], []);
        if bind_manifest {
            let manifest = AssetPermissionManifest {
                version: ManifestVersion::V1,
                uaid,
                dataspace,
                issued_ms: 1,
                activation_epoch: 1,
                expiry_epoch: None,
                entries: Vec::new(),
            };
            let mut record = SpaceDirectoryManifestRecord::new(manifest);
            record.lifecycle.mark_activated(1);
            let mut set = SpaceDirectoryManifestSet::default();
            set.upsert(record);
            world.space_directory_manifests.insert(uaid, set);
            let mut bindings = UaidDataspaceBindings::default();
            bindings.bind_account(dataspace, account_id.clone());
            world.uaid_dataspaces.insert(uaid, bindings);
        }

        (world, account_id, key_pair)
    }

    #[tokio::test]
    async fn push_tx() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));

        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Queue::test(config_factory(), &time_source);

        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("Failed to push tx into queue");
    }

    #[test]
    fn enforce_lane_teu_limits_defers_when_capacity_exceeded() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        // Prepare a representative transaction so we can size the lane capacity to its TEU weight.
        let first_tx = accepted_tx_by_someone(&time_source);
        let lane_capacity = Queue::compute_teu_weight(&first_tx);
        assert!(
            lane_capacity > 0,
            "expected positive TEU weight for sample transaction"
        );

        let test_lane = LaneId::new(7);
        let test_dataspace = DataSpaceId::new(1);
        let router: Arc<dyn LaneRouter> = Arc::new(StaticRouter {
            lane: test_lane,
            dataspace: test_dataspace,
        });
        let scheduling = LaneSchedulingLimits::new(lane_capacity, 0);
        let queue_inner = Queue::test_with_router(config_factory(), &time_source, router);
        *queue_inner.nexus_limits.write() = QueueLimits {
            fallback: scheduling,
            per_lane: BTreeMap::from([(test_lane, scheduling)]),
        };
        let queue = Arc::new(queue_inner);

        let first_hash = first_tx.as_ref().hash();
        queue
            .push(first_tx, state.view())
            .expect("first push should succeed");

        let second_tx = accepted_tx_by_someone(&time_source);
        let second_hash = second_tx.as_ref().hash();
        queue
            .push(second_tx, state.view())
            .expect("second push should succeed");

        let mut guards = queue.collect_transactions_for_block(&state.view(), nonzero!(2usize));
        assert_eq!(
            guards.len(),
            2,
            "expected both transactions before TEU gating"
        );

        let deferred = queue.enforce_lane_teu_limits(&mut guards);
        assert_eq!(
            guards.len(),
            1,
            "only one transaction should remain after enforcement"
        );
        assert_eq!(deferred.len(), 1, "one transaction should be deferred");
        assert_eq!(
            guards[0].as_ref().hash(),
            first_hash,
            "first transaction should remain executable"
        );
        assert_eq!(
            deferred[0].as_ref().hash(),
            second_hash,
            "second transaction should be deferred"
        );
        assert_eq!(guards[0].routing().lane_id, test_lane);

        drop(guards);
        let deferred_tx = deferred.into_iter().next().expect("deferred tx present");
        queue
            .push(deferred_tx, state.view())
            .expect("re-queueing deferred transaction should succeed once capacity resets");

        let next = queue.collect_transactions_for_block(&state.view(), nonzero!(1usize));
        assert_eq!(
            next.len(),
            1,
            "deferred transaction should be available next slot"
        );
        assert_eq!(next[0].as_ref().hash(), second_hash);
    }

    #[test]
    fn overweight_lane_not_starved_when_not_first_in_batch() {
        struct SequenceRouter {
            decisions: parking_lot::Mutex<Vec<RoutingDecision>>,
        }

        impl LaneRouter for SequenceRouter {
            fn route(
                &self,
                _tx: &AcceptedTransaction<'_>,
                _state_view: &crate::state::StateView<'_>,
            ) -> RoutingDecision {
                let mut decisions = self.decisions.lock();
                decisions.remove(0)
            }
        }

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let lane_a = LaneId::new(1);
        let lane_b = LaneId::new(2);
        let dataspace_a = DataSpaceId::new(11);
        let dataspace_b = DataSpaceId::new(12);

        let first_tx = accepted_tx_by_someone(&time_source);
        let second_tx = accepted_tx_by_someone(&time_source);
        let overweight_teu = Queue::compute_teu_weight(&second_tx);
        assert!(overweight_teu > 1, "expected non-trivial TEU weight");

        let lane_a_limits = LaneSchedulingLimits::new(overweight_teu.saturating_mul(2), 0);
        let lane_b_bounds = LaneSchedulingLimits::new(overweight_teu.saturating_sub(1), 0);

        let router: Arc<dyn LaneRouter> = Arc::new(SequenceRouter {
            decisions: parking_lot::Mutex::new(vec![
                RoutingDecision::new(lane_a, dataspace_a),
                RoutingDecision::new(lane_b, dataspace_b),
            ]),
        });

        let queue_inner = Queue::test_with_router(config_factory(), &time_source, router);
        *queue_inner.nexus_limits.write() = QueueLimits {
            fallback: lane_b_bounds,
            per_lane: BTreeMap::from([(lane_a, lane_a_limits), (lane_b, lane_b_bounds)]),
        };
        let queue = Arc::new(queue_inner);

        queue
            .push(first_tx, state.view())
            .expect("lane A push should succeed");
        queue
            .push(second_tx, state.view())
            .expect("lane B push should succeed");

        let mut guards = queue.collect_transactions_for_block(&state.view(), nonzero!(2usize));
        assert_eq!(guards.len(), 2, "expected both transactions before gating");

        let mut consumed = BTreeMap::new();
        let deferred = queue.enforce_lane_teu_limits_with_consumption(&mut guards, &mut consumed);

        assert!(
            deferred.is_empty(),
            "overweight transaction for lane B must not be starved"
        );
        assert!(
            guards.iter().any(|guard| guard.routing().lane_id == lane_b),
            "lane B transaction should be retained"
        );
    }

    #[test]
    fn enforce_lane_teu_limits_with_consumption_respects_existing_usage() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let first_tx = accepted_tx_by_someone(&time_source);
        let lane_capacity = Queue::compute_teu_weight(&first_tx);
        assert!(lane_capacity > 0, "expected positive TEU weight");

        let test_lane = LaneId::new(11);
        let test_dataspace = DataSpaceId::new(5);
        let router: Arc<dyn LaneRouter> = Arc::new(StaticRouter {
            lane: test_lane,
            dataspace: test_dataspace,
        });
        let scheduling = LaneSchedulingLimits::new(lane_capacity, 0);
        let queue_inner = Queue::test_with_router(config_factory(), &time_source, router);
        *queue_inner.nexus_limits.write() = QueueLimits {
            fallback: scheduling,
            per_lane: BTreeMap::from([(test_lane, scheduling)]),
        };
        let queue = Arc::new(queue_inner);

        queue
            .push(first_tx, state.view())
            .expect("first push should succeed");

        let second_tx = accepted_tx_by_someone(&time_source);
        queue
            .push(second_tx, state.view())
            .expect("second push should succeed");

        let mut guards = queue.collect_transactions_for_block(&state.view(), nonzero!(2usize));
        assert_eq!(guards.len(), 2, "expected both transactions before gating");

        let mut consumed = BTreeMap::new();
        consumed.insert(test_lane, lane_capacity);
        let deferred = queue.enforce_lane_teu_limits_with_consumption(&mut guards, &mut consumed);

        assert!(guards.is_empty(), "all transactions should be deferred");
        assert_eq!(deferred.len(), 2, "both transactions should defer");
        assert_eq!(
            consumed.get(&test_lane),
            Some(&lane_capacity),
            "lane consumption should remain capped at the configured capacity"
        );
    }

    #[test]
    fn enforce_lane_teu_limits_is_deterministic_across_guard_order() {
        fn run_with_order<F>(
            mut reorder: F,
            first_tx: &AcceptedTransaction<'static>,
            second_tx: &AcceptedTransaction<'static>,
        ) -> (Vec<SignedTxHash>, Vec<SignedTxHash>)
        where
            F: FnMut(&mut Vec<TransactionGuard>),
        {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
            let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

            let lane_capacity = Queue::compute_teu_weight(first_tx);
            assert!(lane_capacity > 0, "expected positive TEU weight");

            let test_lane = LaneId::new(17);
            let test_dataspace = DataSpaceId::new(4);
            let router: Arc<dyn LaneRouter> = Arc::new(StaticRouter {
                lane: test_lane,
                dataspace: test_dataspace,
            });
            let scheduling = LaneSchedulingLimits::new(lane_capacity, 0);
            let queue_inner = Queue::test_with_router(config_factory(), &time_source, router);
            *queue_inner.nexus_limits.write() = QueueLimits {
                fallback: scheduling,
                per_lane: BTreeMap::from([(test_lane, scheduling)]),
            };
            let queue = Arc::new(queue_inner);

            queue
                .push(first_tx.clone(), state.view())
                .expect("first push should succeed");
            queue
                .push(second_tx.clone(), state.view())
                .expect("second push should succeed");

            let mut guards = queue.collect_transactions_for_block(&state.view(), nonzero!(2usize));
            assert_eq!(guards.len(), 2, "expected both transactions before gating");
            reorder(&mut guards);

            let mut consumed = BTreeMap::new();
            let deferred =
                queue.enforce_lane_teu_limits_with_consumption(&mut guards, &mut consumed);

            let retained_hashes = guards
                .iter()
                .map(|guard| guard.tx.as_ref().hash())
                .collect::<Vec<_>>();
            let deferred_hashes = deferred
                .iter()
                .map(|tx| tx.as_ref().hash())
                .collect::<Vec<_>>();
            (retained_hashes, deferred_hashes)
        }

        let (_seed_handle, seed_time_source) = TimeSource::new_mock(Duration::default());
        let (account_id, key_pair) = gen_account_in("wonderland");
        let first_template = accepted_tx_by(account_id.clone(), &key_pair, &seed_time_source);
        let second_template = accepted_tx_by(account_id, &key_pair, &seed_time_source);

        let (retained_normal, deferred_normal) =
            run_with_order(|_| {}, &first_template, &second_template);
        let (retained_reversed, deferred_reversed) =
            run_with_order(|guards| guards.reverse(), &first_template, &second_template);
        assert_eq!(
            retained_normal, retained_reversed,
            "lane gating should retain the same transactions regardless of guard order"
        );
        assert_eq!(
            deferred_normal, deferred_reversed,
            "lane gating should defer the same transactions regardless of guard order"
        );
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn enforce_lane_teu_limits_updates_telemetry_counters() {
        use std::num::NonZeroU32;

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);

        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let first_tx = accepted_tx_by_someone(&time_source);
        let lane_capacity = Queue::compute_teu_weight(&first_tx);
        assert!(lane_capacity > 0, "expected positive TEU weight");

        let test_lane = LaneId::new(11);
        let test_dataspace = DataSpaceId::new(3);
        let lane_metadata = LaneConfig {
            id: test_lane,
            dataspace_id: test_dataspace,
            alias: "lane11".to_string(),
            metadata: BTreeMap::from([(
                "scheduler.teu_capacity".to_string(),
                lane_capacity.to_string(),
            )]),
            ..LaneConfig::default()
        };
        let dataspace_metadata = DataSpaceMetadata {
            id: test_dataspace,
            alias: "dataspace3".to_string(),
            description: None,
            fault_tolerance: 1,
        };
        let lane_catalog = LaneCatalog::new(
            NonZeroU32::new(16).expect("nonzero lane count"),
            vec![lane_metadata.clone()],
        )
        .expect("valid lane catalog");
        let dataspace_catalog = DataSpaceCatalog::new(vec![dataspace_metadata.clone()])
            .expect("valid dataspace catalog");
        telemetry.set_nexus_catalogs(&lane_catalog, &dataspace_catalog);

        let state = Arc::new(State::with_telemetry(
            world_with_test_domains(),
            kura.clone(),
            query_handle.clone(),
            telemetry.clone(),
        ));

        let router: Arc<dyn LaneRouter> = Arc::new(StaticRouter {
            lane: test_lane,
            dataspace: test_dataspace,
        });
        let scheduling = LaneSchedulingLimits::new(lane_capacity, 0);
        let queue_inner = Queue::test_with_router(config_factory(), &time_source, router);
        *queue_inner.nexus_limits.write() = QueueLimits {
            fallback: scheduling,
            per_lane: BTreeMap::from([(test_lane, scheduling)]),
        };
        queue_inner
            .lane_teu_pending
            .insert(test_lane, PendingTeu::default());
        queue_inner
            .dataspace_teu_pending
            .insert((test_lane, test_dataspace), PendingTeu::default());
        let queue = Arc::new(queue_inner);

        let first_hash = first_tx.as_ref().hash();
        queue
            .push(first_tx, state.view())
            .expect("first push should succeed");
        let second_tx = accepted_tx_by_someone(&time_source);
        let second_hash = second_tx.as_ref().hash();
        queue
            .push(second_tx, state.view())
            .expect("second push should succeed");

        let mut guards = queue.collect_transactions_for_block(&state.view(), nonzero!(2usize));
        assert_eq!(guards.len(), 2);

        let deferred = queue.enforce_lane_teu_limits(&mut guards);
        assert_eq!(guards.len(), 1);
        assert_eq!(deferred.len(), 1);
        assert_eq!(guards[0].as_ref().hash(), first_hash);
        assert_eq!(deferred[0].as_ref().hash(), second_hash);

        let lane_label = test_lane.as_u32().to_string();
        let recorded = metrics
            .nexus_scheduler_lane_teu_deferral_total
            .with_label_values(&[lane_label.as_str(), "cap_exceeded"])
            .get();
        assert_eq!(recorded, lane_capacity);

        let lane_snapshots = metrics
            .nexus_scheduler_lane_teu_status
            .read()
            .expect("lane TEU cache poisoned");
        let snapshot = lane_snapshots
            .get(&test_lane.as_u32())
            .expect("lane snapshot missing");
        assert_eq!(snapshot.deferrals.cap_exceeded, lane_capacity);
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn queue_backlog_reports_available_lane_headroom() {
        use std::num::NonZeroU32;

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);

        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let first_tx = accepted_tx_by_someone(&time_source);
        let second_tx = accepted_tx_by_someone(&time_source);
        let first_teu = Queue::compute_teu_weight(&first_tx);
        let second_teu = Queue::compute_teu_weight(&second_tx);
        let lane_capacity = first_teu.saturating_mul(10);
        assert!(
            lane_capacity > first_teu,
            "expected lane capacity to exceed TEU"
        );

        let test_lane = LaneId::new(13);
        let test_dataspace = DataSpaceId::new(9);
        let lane_metadata = LaneConfig {
            id: test_lane,
            dataspace_id: test_dataspace,
            alias: "lane13".to_string(),
            metadata: BTreeMap::from([(
                "scheduler.teu_capacity".to_string(),
                lane_capacity.to_string(),
            )]),
            ..LaneConfig::default()
        };
        let dataspace_metadata = DataSpaceMetadata {
            id: test_dataspace,
            alias: "dataspace9".to_string(),
            description: None,
            fault_tolerance: 1,
        };
        let lane_catalog = LaneCatalog::new(
            NonZeroU32::new(16).expect("nonzero lane count"),
            vec![lane_metadata.clone()],
        )
        .expect("valid lane catalog");
        let dataspace_catalog = DataSpaceCatalog::new(vec![dataspace_metadata.clone()])
            .expect("valid dataspace catalog");
        telemetry.set_nexus_catalogs(&lane_catalog, &dataspace_catalog);

        let state = Arc::new(State::with_telemetry(
            world_with_test_domains(),
            kura.clone(),
            query_handle.clone(),
            telemetry.clone(),
        ));

        let router: Arc<dyn LaneRouter> = Arc::new(StaticRouter {
            lane: test_lane,
            dataspace: test_dataspace,
        });
        let scheduling = LaneSchedulingLimits::new(lane_capacity, 0);
        let queue_inner = Queue::test_with_router(config_factory(), &time_source, router);
        *queue_inner.nexus_limits.write() = QueueLimits {
            fallback: scheduling,
            per_lane: BTreeMap::from([(test_lane, scheduling)]),
        };
        queue_inner
            .lane_teu_pending
            .insert(test_lane, PendingTeu::default());
        queue_inner
            .dataspace_teu_pending
            .insert((test_lane, test_dataspace), PendingTeu::default());
        let queue = Arc::new(queue_inner);

        queue
            .push(first_tx, state.view())
            .expect("first push should succeed");
        queue
            .push(second_tx, state.view())
            .expect("second push should succeed");

        let pending_teu = first_teu.saturating_add(second_teu);
        let expected_committed = pending_teu.min(lane_capacity);
        let expected_headroom = lane_capacity.saturating_sub(expected_committed);

        let lane_label = test_lane.as_u32().to_string();
        let headroom_events = metrics
            .nexus_scheduler_lane_headroom_events_total
            .with_label_values(&[lane_label.as_str()])
            .get();
        assert_eq!(
            headroom_events, 0,
            "backlog snapshots should not emit warnings"
        );

        let lane_snapshots = metrics
            .nexus_scheduler_lane_teu_status
            .read()
            .expect("lane TEU cache poisoned");
        let snapshot = lane_snapshots
            .get(&test_lane.as_u32())
            .expect("lane snapshot missing");
        assert_eq!(snapshot.committed, expected_committed);
        assert_eq!(snapshot.buckets.headroom, expected_headroom);
    }

    #[cfg(feature = "telemetry")]
    #[tokio::test]
    async fn push_records_teu_using_router_assignment() {
        struct StaticRouter {
            lane: LaneId,
            dataspace: DataSpaceId,
        }

        impl LaneRouter for StaticRouter {
            fn route(
                &self,
                _tx: &AcceptedTransaction<'_>,
                _state_view: &crate::state::StateView<'_>,
            ) -> RoutingDecision {
                RoutingDecision::new(self.lane, self.dataspace)
            }
        }

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let (events_sender, _) = tokio::sync::broadcast::channel(8);
        let router = Arc::new(StaticRouter {
            lane: LaneId::new(7),
            dataspace: DataSpaceId::new(42),
        });
        let queue = Arc::new(Queue::from_config_with_router(
            config_factory(),
            events_sender,
            router,
        ));

        let (account_id, key_pair) = gen_account_in("wonderland");
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
        let domain_name = format!("tagged{}", rand::random::<u64>());
        let unregister = Unregister::domain(domain_name.parse().unwrap());
        let tx =
            TransactionBuilder::new_with_time_source(chain_id.clone(), account_id, &time_source)
                .with_instructions([unregister])
                .sign(key_pair.private_key());
        let default_limits = TransactionParameters::default();
        let tx_limits = TransactionParameters::with_max_signatures(
            nonzero!(16_u64),
            nonzero!(4096_u64),
            nonzero!(1024_u64),
            default_limits.max_tx_bytes(),
            default_limits.max_decompressed_bytes(),
            default_limits.max_metadata_depth(),
        );
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            Duration::from_secs(60),
            tx_limits,
            &crypto_cfg,
        )
        .expect("Failed to accept transaction.");
        let hash = tx.as_ref().hash();
        queue
            .push(tx, state.view())
            .expect("Failed to push tx into queue");

        let teu_info = queue
            .tx_teu
            .get(&hash)
            .expect("TEU info missing for routed transaction");
        assert_eq!(teu_info.lane_id, LaneId::new(7));
        assert_eq!(teu_info.dataspace_id, DataSpaceId::new(42));
    }

    #[cfg(feature = "telemetry")]
    #[tokio::test]
    async fn push_records_teu_from_ivm_metadata() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Arc::new(Queue::test(config_factory(), &time_source));

        let (account_id, key_pair) = gen_account_in("wonderland");
        let max_cycles = 42_000_u64;
        let tx = accepted_ivm_tx_by(account_id, &key_pair, &time_source, max_cycles);
        let hash = tx.as_ref().hash();

        queue
            .push(tx, state.view())
            .expect("Failed to enqueue IVM transaction");

        let info = queue
            .tx_teu
            .get(&hash)
            .expect("TEU info missing for IVM transaction");
        assert_eq!(info.teu, max_cycles);
    }

    #[tokio::test]
    async fn block_events_carry_lane_metadata_from_queue() {
        struct TaggedRouter {
            lane: LaneId,
            dataspace: DataSpaceId,
        }

        impl LaneRouter for TaggedRouter {
            fn route(
                &self,
                _tx: &AcceptedTransaction<'_>,
                _state_view: &crate::state::StateView<'_>,
            ) -> RoutingDecision {
                RoutingDecision::new(self.lane, self.dataspace)
            }
        }

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let expected_lane = LaneId::new(5);
        let expected_dataspace = DataSpaceId::new(13);
        let queue = Arc::new(Queue::test_with_router(
            config_factory(),
            &time_source,
            Arc::new(TaggedRouter {
                lane: expected_lane,
                dataspace: expected_dataspace,
            }),
        ));

        let tx = accepted_tx_by_someone(&time_source);
        let hash = tx.as_ref().hash();
        let routing = queue
            .push_with_lane(tx, state.view())
            .expect("Failed to enqueue transaction");
        assert_eq!(routing.lane_id, expected_lane);
        assert_eq!(routing.dataspace_id, expected_dataspace);
        assert!(
            queue.routing_decisions.contains_key(&hash),
            "routing decision missing from queue cache"
        );

        let state_view = state.view();
        let mut guards = Vec::new();
        queue.get_transactions_for_block(&state_view, nonzero!(1_usize), &mut guards);
        drop(state_view);

        assert_eq!(guards.len(), 1);
        let cached = queue
            .routing_decisions
            .get(&hash)
            .map(|entry| *entry.value())
            .expect("routing cached");
        assert_eq!(cached.lane_id, expected_lane);
        assert_eq!(cached.dataspace_id, expected_dataspace);
        let transactions: Vec<_> = guards
            .iter()
            .map(TransactionGuard::clone_accepted)
            .collect();
        let ledger_entry =
            crate::queue::routing_ledger::take(&hash).expect("routing entry missing from ledger");
        assert_eq!(ledger_entry.lane_id, expected_lane);
        assert_eq!(ledger_entry.dataspace_id, expected_dataspace);
        crate::queue::routing_ledger::record(hash, ledger_entry);

        let new_block = BlockBuilder::new(transactions)
            .chain(0, None)
            .sign(ALICE_KEYPAIR.private_key())
            .unpack(|_| {});
        let header = new_block.header();
        let signed_block: SignedBlock = new_block.into();
        let mut state_block = state.block(header);
        let valid_block =
            ValidBlock::validate_unchecked(signed_block, &mut state_block).unpack(|_| {});

        drop(guards);

        let ledger_before_event = crate::queue::routing_ledger::take(&hash)
            .expect("routing entry removed before event emission");
        assert_eq!(ledger_before_event.lane_id, expected_lane);
        assert_eq!(ledger_before_event.dataspace_id, expected_dataspace);
        crate::queue::routing_ledger::record(hash, ledger_before_event);

        let tx_event = valid_block
            .produce_events()
            .find_map(|event| match event {
                PipelineEventBox::Transaction(event) if event.hash() == &hash => Some(event),
                _ => None,
            })
            .expect("missing transaction event for routed transaction");

        assert_eq!(tx_event.lane_id(), expected_lane);
        assert_eq!(tx_event.dataspace_id(), expected_dataspace);
    }

    #[tokio::test]
    async fn dropping_transaction_guard_clears_removed_hashes() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));

        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Arc::new(Queue::test(config_factory(), &time_source));

        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("Failed to push tx into queue");

        let mut expired_transactions = Vec::new();
        let state_view = state.view();
        let guard = queue
            .pop_from_queue(&state_view, &mut expired_transactions)
            .expect("Expected a transaction guard");
        assert!(expired_transactions.is_empty());

        drop(guard);

        assert!(queue.removed_hashes.is_empty());
    }

    #[tokio::test]
    async fn push_tx_overflow() {
        let capacity = nonzero!(10_usize);

        let kura: Arc<Kura> = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));

        let (time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Queue::test(
            Config {
                transaction_time_to_live: Duration::from_secs(100),
                capacity,
                ..Config::default()
            },
            &time_source,
        );

        for _ in 0..capacity.get() {
            queue
                .push(accepted_tx_by_someone(&time_source), state.view())
                .expect("Failed to push tx into queue");
            time_handle.advance(Duration::from_millis(10));
        }

        assert!(matches!(
            queue.push(accepted_tx_by_someone(&time_source), state.view()),
            Err(Failure {
                err: Error::Full,
                ..
            })
        ));
    }

    #[test]
    fn lane_limits_respect_metadata_overrides() {
        let fallback = LaneSchedulingLimits::new(6_000, 120);
        let mut lane = LaneConfig {
            id: LaneId::new(3),
            alias: "custom".to_string(),
            metadata: BTreeMap::from([
                ("scheduler.teu_capacity".to_string(), "8192".to_string()),
                (
                    "scheduler.starvation_bound_slots".to_string(),
                    "42".to_string(),
                ),
            ]),
            ..LaneConfig::default()
        };
        let limits = QueueLimits::lane_limits_from_metadata(&lane, fallback);
        assert_eq!(limits.teu_capacity, 8_192);
        assert_eq!(limits.starvation_bound_slots, 42);

        lane.metadata.insert(
            "scheduler.teu_capacity".to_string(),
            "not-a-number".to_string(),
        );
        lane.metadata.insert(
            "scheduler.starvation_bound_slots".to_string(),
            "NaN".to_string(),
        );
        let limits = QueueLimits::lane_limits_from_metadata(&lane, fallback);
        assert_eq!(limits, fallback);
    }

    #[tokio::test]
    async fn overweight_tx_not_starved_by_lane_teu_limit() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let time_source = TimeSource::new_system();

        let tx = accepted_tx_by_someone(&time_source);
        let teu = Queue::compute_teu_weight(&tx);
        assert!(
            teu > 1,
            "baseline transaction should carry a non-trivial TEU weight (got {teu})"
        );
        let lane_cap = teu.saturating_sub(1);
        let limits = QueueLimits {
            fallback: LaneSchedulingLimits::new(lane_cap, 0),
            per_lane: BTreeMap::new(),
        };
        let queue = Queue::test(config_factory(), &time_source);
        *queue.nexus_limits.write() = limits;
        let queue = Arc::new(queue);

        queue
            .push(tx, state.view())
            .expect("overweight tx should still be enqueued");

        let mut guards = Vec::new();
        queue.get_transactions_for_block(&state.view(), nonzero!(1_usize), &mut guards);
        assert_eq!(
            guards.len(),
            1,
            "first overweight transaction must not be indefinitely deferred"
        );
    }

    #[tokio::test]
    async fn backpressure_state_tracks_queue_load() {
        let capacity = nonzero!(2_usize);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Arc::new(Queue::test(
            Config {
                capacity,
                ..config_factory()
            },
            &time_source,
        ));
        let mut rx = queue.backpressure_handle().subscribe();

        assert!(!rx.borrow().is_saturated());

        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("first push succeeds");
        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("second push reaches capacity");

        rx.changed().await.expect("backpressure update to saturate");
        assert!(rx.borrow().is_saturated());

        let mut expired = Vec::new();
        let guard = queue
            .pop_from_queue(&state.view(), &mut expired)
            .expect("transaction available");
        drop(guard);

        rx.changed().await.expect("backpressure update to healthy");
        assert!(!rx.borrow().is_saturated());
    }

    #[tokio::test]
    async fn resync_rebuilds_hash_queue_when_empty() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        for _ in 0..3 {
            queue
                .push(accepted_tx_by_someone(&time_source), state.view())
                .expect("push succeeds");
        }

        while queue.tx_hashes.pop().is_some() {}
        assert_eq!(queue.queued_len(), 0, "hash queue should be empty");
        assert_eq!(queue.txs.len(), 3, "tx map retains queued entries");

        let mut guards = Vec::new();
        queue.get_transactions_for_block(&state.view(), nonzero!(2_usize), &mut guards);
        assert_eq!(guards.len(), 2, "resync should repopulate hashes");
        drop(guards);

        assert_eq!(
            queue.queued_len(),
            1,
            "remaining queue size should be tracked"
        );
    }

    #[tokio::test]
    async fn resync_skips_when_guards_inflight() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("push succeeds");

        let mut guards = Vec::new();
        queue.get_transactions_for_block(&state.view(), nonzero!(1_usize), &mut guards);
        assert_eq!(guards.len(), 1, "expected one in-flight guard");
        assert_eq!(
            queue.queued_len(),
            0,
            "hash queue should be empty while guard is held"
        );

        let mut extra = Vec::new();
        queue.get_transactions_for_block(&state.view(), nonzero!(1_usize), &mut extra);
        assert!(
            extra.is_empty(),
            "resync should be skipped while guard is in flight"
        );

        drop(guards);
    }

    #[tokio::test]
    async fn get_available_txs() {
        let max_txs_in_block = nonzero!(2_usize);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));

        let (time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Queue::test(
            Config {
                transaction_time_to_live: Duration::from_secs(100),
                ..config_factory()
            },
            &time_source,
        );
        let queue = Arc::new(queue);
        for _ in 0..5 {
            queue
                .push(accepted_tx_by_someone(&time_source), state.view())
                .expect("Failed to push tx into queue");
            time_handle.advance(Duration::from_millis(10));
        }

        let available = queue.collect_transactions_for_block(&state.view(), max_txs_in_block);
        assert_eq!(available.len(), max_txs_in_block.get());
    }

    #[tokio::test]
    async fn transaction_guard_clone_accepted_returns_owned_transaction() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let queue = Arc::new(Queue::test(config_factory(), &time_source));

        let tx = accepted_tx_by_someone(&time_source);
        let expected_hash = tx.as_ref().hash();
        queue
            .push(tx, state.view())
            .expect("Failed to push tx into queue");

        let mut expired = Vec::new();
        let guard = queue
            .pop_from_queue(&state.view(), &mut expired)
            .expect("Expected guard from queue");
        assert!(expired.is_empty());

        let guard_hash = guard.as_ref().hash();
        assert_eq!(guard_hash, expected_hash);

        let cloned = guard.clone_accepted();
        assert_eq!(cloned.as_ref().hash(), guard_hash);

        drop(guard);

        assert_eq!(queue.queued_len(), 0);
        assert_eq!(cloned.as_ref().hash(), guard_hash);
    }

    #[tokio::test]
    async fn push_tx_already_in_blockchain() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world_with_test_domains(), kura, query_handle);
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let tx = accepted_tx_by_someone(&time_source);
        let (_, private_key) = KeyPair::random().into_parts();
        let unverified_block: SignedBlock =
            ValidBlock::new_dummy_and_modify_header(&private_key, |header| {
                header.height = nonzero!(1_u64);
            })
            .into();
        let mut state_block = state.block(unverified_block.header());
        let block_height: NonZeroUsize = unverified_block
            .header()
            .height()
            .try_into()
            .expect("block height should fit into usize");
        state_block
            .transactions
            .insert_block_with_single_tx(tx.as_ref().hash(), block_height);
        state_block.commit().unwrap();
        let queue = Queue::test(config_factory(), &time_source);
        assert!(matches!(
            queue.push(tx, state.view()),
            Err(Failure {
                err: Error::InBlockchain,
                ..
            })
        ));
        assert_eq!(queue.txs.len(), 0);
    }

    #[tokio::test]
    async fn push_expired_tx_already_in_blockchain() {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        let (alice_id, alice_keypair) = gen_account_in("wonderland");

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world_with_test_domains(), kura, query_handle);
        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let (time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let ok_instruction = Log::new(iroha_logger::Level::INFO, "pass".into());
        let mut tx =
            TransactionBuilder::new_with_time_source(chain_id.clone(), alice_id, &time_source)
                .with_instructions([ok_instruction]);
        tx.set_ttl(Duration::from_millis(100));
        let tx = tx.sign(alice_keypair.private_key());
        let tx = {
            let crypto_cfg = state.crypto();
            AcceptedTransaction::accept_with_time_source(
                tx,
                &chain_id,
                max_clock_drift,
                tx_limits,
                &crypto_cfg,
                &time_source,
            )
            .expect("Failed to accept Transaction.")
        };

        let (_, private_key) = KeyPair::random().into_parts();
        let unverified_block: SignedBlock =
            ValidBlock::new_dummy_and_modify_header(&private_key, |header| {
                header.height = nonzero!(1_u64);
            })
            .into();
        let mut state_block = state.block(unverified_block.header());
        let block_height: NonZeroUsize = unverified_block
            .header()
            .height()
            .try_into()
            .expect("block height should fit into usize");
        state_block
            .transactions
            .insert_block_with_single_tx(tx.as_ref().hash(), block_height);
        state_block.commit().unwrap();
        let queue = Queue::test(config_factory(), &time_source);
        time_handle.advance(Duration::from_secs(100));
        assert!(matches!(
            queue.push(tx, state.view()),
            Err(Failure {
                err: Error::InBlockchain,
                ..
            })
        ));
        assert_eq!(queue.txs.len(), 0);
    }

    #[tokio::test]
    async fn get_tx_drop_if_in_blockchain() {
        let max_txs_in_block = nonzero!(2_usize);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world_with_test_domains(), kura, query_handle);
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let tx = accepted_tx_by_someone(&time_source);
        let tx_hash = tx.as_ref().hash();
        let queue = Queue::test(config_factory(), &time_source);
        let queue = Arc::new(queue);
        queue.push(tx, state.view()).unwrap();
        let (_, private_key) = KeyPair::random().into_parts();
        let unverified_block: SignedBlock =
            ValidBlock::new_dummy_and_modify_header(&private_key, |header| {
                header.height = nonzero!(1_u64);
            })
            .into();
        let mut state_block = state.block(unverified_block.header());
        let block_height: NonZeroUsize = unverified_block
            .header()
            .height()
            .try_into()
            .expect("block height should fit into usize");
        state_block
            .transactions
            .insert_block_with_single_tx(tx_hash, block_height);
        state_block.commit().unwrap();
        assert_eq!(
            queue
                .collect_transactions_for_block(&state.view(), max_txs_in_block)
                .len(),
            0
        );
        assert_eq!(queue.txs.len(), 0);
    }

    #[tokio::test]
    async fn get_available_txs_with_timeout() {
        let max_txs_in_block = nonzero!(6_usize);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));

        let (time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Queue::test(
            Config {
                transaction_time_to_live: Duration::from_millis(200),
                ..config_factory()
            },
            &time_source,
        );
        let queue = Arc::new(queue);
        for _ in 0..(max_txs_in_block.get() - 1) {
            queue
                .push(accepted_tx_by_someone(&time_source), state.view())
                .expect("Failed to push tx into queue");
            time_handle.advance(Duration::from_millis(100));
        }

        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("Failed to push tx into queue");
        time_handle.advance(Duration::from_millis(101));
        assert_eq!(
            queue
                .collect_transactions_for_block(&state.view(), max_txs_in_block)
                .len(),
            1
        );

        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("Failed to push tx into queue");
        time_handle.advance(Duration::from_millis(210));
        assert_eq!(
            queue
                .collect_transactions_for_block(&state.view(), max_txs_in_block)
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn custom_expired_transaction_is_rejected() {
        const TTL_MS: u64 = 200;

        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        let max_txs_in_block = nonzero!(2_usize);
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };

        let (time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let mut queue = Queue::test(config_factory(), &time_source);
        let (event_sender, mut event_receiver) = tokio::sync::broadcast::channel(1);
        queue.events_sender = event_sender;
        // Use a simple instruction to avoid exercising heavy decode paths
        // unrelated to queue TTL behavior.
        let instructions = [Log::new(iroha_logger::Level::INFO, "ttl".into())];
        let mut tx =
            TransactionBuilder::new_with_time_source(chain_id.clone(), alice_id, &time_source)
                .with_instructions(instructions);
        tx.set_ttl(Duration::from_millis(TTL_MS));
        let tx = tx.sign(alice_keypair.private_key());
        let tx_hash = tx.hash();
        let tx = {
            let crypto_cfg = state.crypto();
            AcceptedTransaction::accept_with_time_source(
                tx,
                &chain_id,
                max_clock_drift,
                tx_limits,
                &crypto_cfg,
                &time_source,
            )
            .expect("Failed to accept Transaction.")
        };
        queue
            .push(tx, state.view())
            .expect("Failed to push tx into queue");
        // Avoid indefinite hang if events are not delivered
        let queued_tx_event = tokio::time::timeout(Duration::from_secs(2), event_receiver.recv())
            .await
            .expect("timed out waiting for queued event")
            .unwrap();

        assert_eq!(
            queued_tx_event,
            TransactionEvent {
                hash: tx_hash,
                block_height: None,
                lane_id: LaneId::SINGLE,
                dataspace_id: DataSpaceId::GLOBAL,
                status: TransactionStatus::Queued,
            }
            .into()
        );

        let mut txs = Vec::new();
        time_handle.advance(Duration::from_millis(TTL_MS + 1));
        let queue = Arc::new(queue);
        queue.get_transactions_for_block(&state.view(), max_txs_in_block, &mut txs);
        let expired_tx_event = tokio::time::timeout(Duration::from_secs(2), event_receiver.recv())
            .await
            .expect("timed out waiting for expired event")
            .unwrap();
        assert!(txs.is_empty());

        assert_eq!(
            expired_tx_event,
            TransactionEvent {
                hash: tx_hash,
                block_height: None,
                lane_id: LaneId::SINGLE,
                dataspace_id: DataSpaceId::GLOBAL,
                status: TransactionStatus::Expired,
            }
            .into()
        )
    }

    #[test]
    fn expired_cull_sweeps_reduce_active_len() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Queue::test(
            Config {
                transaction_time_to_live: Duration::from_secs(1),
                expired_cull_interval: Duration::from_secs(1),
                ..config_factory()
            },
            &time_source,
        );

        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("push succeeds");
        assert_eq!(queue.active_len(), 1, "tx tracked before expiration");

        time_handle.advance(Duration::from_secs(2));
        let culled = queue.cull_expired_entries_if_due();
        assert_eq!(culled, 1, "expired transaction should be culled");
        assert_eq!(
            queue.active_len(),
            0,
            "expired tx removed from active count"
        );
    }

    #[test]
    fn expired_cull_compacts_hash_queue() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let mut cfg = config_factory();
        cfg.capacity = nonzero!(2_usize);
        cfg.capacity_per_user = nonzero!(2_usize);
        cfg.transaction_time_to_live = Duration::from_secs(1);
        cfg.expired_cull_interval = Duration::from_secs(1);

        let queue = Queue::test(cfg, &time_source);
        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("push succeeds");
        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("push succeeds");
        assert_eq!(queue.queued_len(), 2, "hash queue tracks queued txs");

        time_handle.advance(Duration::from_secs(2));
        let culled = queue.cull_expired_entries_if_due();
        assert_eq!(culled, 2, "expired transactions should be culled");
        assert_eq!(queue.queued_len(), 0, "hash queue compacted after cull");

        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("push after compaction succeeds");
        assert_eq!(queue.queued_len(), 1, "hash queue accepts new txs");
    }

    #[test]
    fn expired_cull_respects_batch_limit() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let mut cfg = config_factory();
        cfg.transaction_time_to_live = Duration::from_millis(1);
        cfg.expired_cull_interval = Duration::from_millis(1);
        cfg.expired_cull_batch = nonzero!(1_usize);

        let queue = Queue::test(cfg, &time_source);
        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("push succeeds");
        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("push succeeds");

        time_handle.advance(Duration::from_millis(2));
        let culled = queue.cull_expired_entries_if_due();
        assert_eq!(culled, 1, "batch-limited sweep culls one tx");
        assert_eq!(queue.active_len(), 1, "one tx remains after first sweep");

        time_handle.advance(Duration::from_millis(2));
        let culled = queue.cull_expired_entries_if_due();
        assert_eq!(culled, 1, "second sweep culls remaining tx");
        assert_eq!(queue.active_len(), 0, "all expired txs removed");
    }

    #[test]
    fn remove_committed_hashes_clears_expiry_tracking() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Queue::test(config_factory(), &time_source);
        let tx = accepted_tx_by_someone(&time_source);
        let hash = tx.as_ref().hash();
        queue.push(tx, state.view()).expect("push succeeds");
        assert!(
            queue.expiry_ring_members.contains_key(&hash),
            "expiry tracking is set on push"
        );

        let removed = queue.remove_committed_hashes([hash], None);
        assert_eq!(removed, 1, "committed hash should be removed");
        assert_eq!(queue.active_len(), 0, "queue no longer tracks tx");
        assert!(
            !queue.expiry_ring_members.contains_key(&hash),
            "expiry tracking cleared on commit removal"
        );
        assert!(
            queue.removed_hashes.contains_key(&hash),
            "removed hash marker set for committed tx"
        );
    }

    #[test]
    fn queued_len_excludes_inflight_transactions() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let mut cfg = config_factory();
        cfg.capacity = nonzero!(2_usize);
        cfg.capacity_per_user = nonzero!(2_usize);
        cfg.transaction_time_to_live = Duration::from_secs(100);

        let queue = Arc::new(Queue::test(cfg, &time_source));
        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("push succeeds");
        assert_eq!(queue.queued_len(), 1, "queued count before pop");

        let mut expired = Vec::new();
        let guard = queue
            .pop_from_queue(&state.view(), &mut expired)
            .expect("pop should return a transaction guard");
        assert!(expired.is_empty());
        assert_eq!(queue.queued_len(), 0, "hash queue empty after pop");
        assert_eq!(queue.active_len(), 1, "active count includes in-flight");
        assert_eq!(queue.queued_len(), 0, "queued count excludes in-flight");
        drop(guard);
        assert_eq!(
            queue.active_len(),
            0,
            "active count clears after guard drop"
        );
    }

    #[test]
    fn inflight_cull_clears_removed_hash_marker_on_drop() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
        let (time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let mut cfg = config_factory();
        cfg.transaction_time_to_live = Duration::from_millis(1);
        cfg.expired_cull_interval = Duration::from_millis(1);

        let queue = Arc::new(Queue::test(cfg, &time_source));
        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("push succeeds");

        let mut expired = Vec::new();
        let guard = queue
            .pop_from_queue(&state.view(), &mut expired)
            .expect("pop should return a transaction guard");
        assert!(expired.is_empty());

        time_handle.advance(Duration::from_millis(2));
        let culled = queue.cull_expired_entries_if_due();
        assert_eq!(culled, 1, "expired in-flight tx should be culled");
        assert_eq!(
            queue.active_len(),
            0,
            "active count reflects the culled in-flight entry"
        );
        assert!(
            !queue.removed_hashes.is_empty(),
            "removed hash marker should be set after cull"
        );

        drop(guard);
        assert!(
            queue.removed_hashes.is_empty(),
            "removed hash marker should be cleared on guard drop"
        );
    }

    #[tokio::test]
    async fn concurrent_stress_test() {
        let max_txs_in_block = nonzero!(10_usize);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));

        let (time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Arc::new(Queue::test(
            Config {
                transaction_time_to_live: Duration::from_secs(100),
                capacity: 100_000_000.try_into().unwrap(),
                ..Config::default()
            },
            &time_source,
        ));

        let start_time = std::time::Instant::now();
        let run_for = Duration::from_secs(5);

        let push_handles: Vec<_> = (0..4)
            .map(|_| {
                let queue_arc_clone = Arc::clone(&queue);
                let state = state.clone();
                let time_source = time_source.clone();
                thread::spawn(move || {
                    while start_time.elapsed() < run_for {
                        let tx = accepted_tx_by_someone(&time_source);
                        match queue_arc_clone.push(tx, state.view()) {
                            Ok(())
                            | Err(Failure {
                                err: Error::Full | Error::MaximumTransactionsPerUser,
                                ..
                            }) => (),
                            Err(Failure { err, .. }) => panic!("{err}"),
                        }
                    }
                })
            })
            .collect();

        // Spawn a thread where we get_transactions_for_block and add them to state
        let get_txs_handle = {
            let queue = Arc::clone(&queue);
            let state = Arc::clone(&state);

            thread::spawn(move || {
                let mut height = nonzero!(1usize);
                while start_time.elapsed() < run_for {
                    {
                        let state_view = state.view();
                        let transactions =
                            queue.collect_transactions_for_block(&state_view, max_txs_in_block);
                        drop(transactions);
                    }
                    height = height.checked_add(1).unwrap();

                    // Simulate random small delays
                    let mut rng = rand::rng();
                    let delay = Duration::from_millis(rng.random_range(0..25));
                    thread::sleep(delay);
                    time_handle.advance(delay);
                }
            })
        };

        for handle in push_handles {
            handle.join().unwrap();
        }
        get_txs_handle.join().unwrap();

        // Validate the queue state.
        let array_queue: Vec<_> = core::iter::from_fn(|| queue.tx_hashes.pop()).collect();

        assert_eq!(array_queue.len(), queue.txs.len());
        for tx in array_queue {
            assert!(queue.txs.contains_key(&tx));
        }
        assert_eq!(queue.vacant_entry_warnings.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn queue_throttling() {
        let kura = Kura::blank_kura_for_testing();
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, bob_keypair) = gen_account_in("wonderland");
        let world = {
            let domain_id = "wonderland".parse().expect("Valid");
            let domain = Domain::new(domain_id).build(&alice_id);
            let alice_account = Account::new(alice_id.clone()).build(&alice_id);
            let bob_account = Account::new(bob_id.clone()).build(&bob_id);
            World::with([domain], [alice_account, bob_account], [])
        };
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);

        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

        let queue = Queue::test(
            Config {
                transaction_time_to_live: Duration::from_secs(100),
                capacity: 100.try_into().unwrap(),
                capacity_per_user: 1.try_into().unwrap(),
                ..Config::default()
            },
            &time_source,
        );
        let queue = Arc::new(queue);

        // First push by Alice should be fine
        queue
            .push(
                accepted_tx_by(alice_id.clone(), &alice_keypair, &time_source),
                state.view(),
            )
            .expect("Failed to push tx into queue");

        // Second push by Alice excide limit and will be rejected
        let result = queue.push(
            accepted_tx_by(alice_id.clone(), &alice_keypair, &time_source),
            state.view(),
        );
        assert!(
            matches!(
                result,
                Err(Failure {
                    tx: _,
                    err: Error::MaximumTransactionsPerUser
                }),
            ),
            "Failed to match: {result:?}",
        );

        // First push by Bob should be fine despite previous Alice error
        queue
            .push(
                accepted_tx_by(bob_id.clone(), &bob_keypair, &time_source),
                state.view(),
            )
            .expect("Failed to push tx into queue");

        let transactions = queue.collect_transactions_for_block(&state.view(), nonzero!(10_usize));
        assert_eq!(transactions.len(), 2);
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        // Put transaction hashes into state as if they were in the blockchain
        let transaction_hashes = transactions
            .into_iter()
            .map(|tx| tx.as_ref().hash())
            .collect();
        state_block
            .transactions
            .insert_block(transaction_hashes, nonzero!(1_usize));
        state_block.commit().unwrap();
        // Cleanup transactions
        let transactions = queue.collect_transactions_for_block(&state.view(), nonzero!(10_usize));
        assert!(transactions.is_empty());

        // After cleanup Alice and Bob pushes should work fine
        queue
            .push(
                accepted_tx_by(alice_id, &alice_keypair, &time_source),
                state.view(),
            )
            .expect("Failed to push tx into queue");

        queue
            .push(
                accepted_tx_by(bob_id, &bob_keypair, &time_source),
                state.view(),
            )
            .expect("Failed to push tx into queue");
    }
}
