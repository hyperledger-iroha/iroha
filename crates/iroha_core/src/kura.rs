//! Translates to warehouse. File-system and persistence-related
//! logic.  [`Kura`] is the main entity which should be used to store
//! new [`Block`](iroha_data_model::block::SignedBlock)s on the
//! blockchain.
mod fastpq_artifact_store;
mod membership_storage;
mod snapshot_hash_journal;
use crate::lane_consensus::{
    CommittedLaneBlockSession, DurableLaneBlockNewViewCertificateV1,
    DurableLaneBlockViewCheckpointV1, DurableLanePayloadAvailabilityCertificateV1,
    LaneExecutablePayloadV1, MAX_LANE_NEW_VIEW_CERTIFICATES,
    decode_autonomous_lane_payload_envelope, lane_payload_availability_body,
};
#[cfg(test)]
use crate::merge::reduce_merge_hint_roots;
use crate::telemetry::StateTelemetry;
use crate::zk::kagemusha_v1_recursion::KagemushaMintAuthorityCheckpointV1;
use crate::{
    block::CommittedBlock,
    queue::{
        RoutingPlan, },
    secure_file_metadata::{self, SecureMetadata},
    sumeragi::{
        lane_planner::autonomous_lane_reservation_identity_hashes_for_proposal,
        message::{
            KuraReplicaAdvertV1, LaneHistoricalRecoveryKindV1, LaneHistoricalRecoveryPayloadV1,
            LaneHistoricalRecoveryRequestV1, LaneHistoricalRecoveryResponseV1,
        },
        output_guard::ConsensusOutputGuard,
        v2_apply::PostCarrierEvidenceRepairAuthorization,
        v2_core::{
            CanonicalIdentityProjection, CheckedProductionTransition,
            IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA,
            IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASE_PENDING,
            IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASED,
            IN_FLIGHT_FIRST_RELEASE_ACTION_AUTHORIZE_READY,
            IN_FLIGHT_FIRST_RELEASE_ACTION_COMPLETE_RESERVATION_RELEASE,
            IN_FLIGHT_FIRST_RELEASE_ACTION_FORGET_RESERVATION_RELEASE,
            IN_FLIGHT_FIRST_RELEASE_ACTION_LANE_COMMIT,
            IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_EXECUTION_INPUT,
            IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_KURA_RETIREMENT,
            IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_READY_QC,
            IN_FLIGHT_FIRST_RELEASE_ACTION_PREPARE_RESERVATION_RELEASE,
            IN_FLIGHT_FIRST_RELEASE_ACTION_RELEASE_RESERVATION_DIRECT,
            IN_FLIGHT_FIRST_RELEASE_ACTION_RESTORE_RELEASED_FIFO,
            IN_FLIGHT_FIRST_RELEASE_ACTION_SIGN_READY, IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_DIRECT_RELEASED,
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_COMPLETED,
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_FORGOTTEN,
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_PREPARED,
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_REPLICA_QUEUE_ABSENT,
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_REPLICA_QUEUE_FIFO_PRESERVED,
            ProductionInFlightFirstReleaseCarrierProjection,
            ProductionInFlightFirstReleaseDecisionProjection,
            ProductionInFlightFirstReleaseHistoryProjection,
            ProductionInFlightFirstReleaseQueueProjection,
            ProductionInFlightFirstReleaseReleaseProjection,
            ProductionInFlightFirstReleaseSessionProjection,
            ProductionInFlightFirstReleaseStateProjection,
            ProductionInFlightFirstReleaseTransitionProjection,
            check_production_in_flight_first_release_observe_replica_queue_release_transition,
            check_production_in_flight_first_release_transition,
            production_in_flight_first_release_state_kernel,
            production_in_flight_first_release_terminal_owner,
        },
    },
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
pub use fastpq_artifact_store::{FastpqDurableArtifactReceipt, FastpqStoredArtifactReference};
use iroha_config::{
    base::WithOrigin,
    kura::{FsyncMode, InitMode},
    parameters::{
        actual::{
            Fastpq as FastpqConfig, Kura as Config, LaneConfig, SnapshotBootstrapPolicy,
            },
        defaults::{
            kura::{
                BLOCKS_IN_MEMORY, FSYNC_INTERVAL,
                MAX_DISK_USAGE_BYTES, },
            zk::fastpq as FASTPQ_DEFAULTS,
        },
    },
};
use iroha_crypto::KeyPair;
use iroha_crypto::{
    Algorithm, Hash, HashOf, MerkleProof, MerkleTree, MerkleTreeCommitment, PublicKey, Signature,
};
#[cfg(test)]
use iroha_data_model::block::decode_versioned_signed_block;
use iroha_data_model::{
    AccountId, NetworkId,
    block::{
        BlockHeader, SignedBlock,
        consensus::{
            ExecWitness, },
        consensus_v2::{
            BlockSubject, ConsensusMode, DataAvailabilityLayout, DualQuorum, ExecutionCommitment,
            HeightContext, HeightContextId, MAX_EXECUTED_BLOCK_WIRE_BYTES,
            QuorumCertificate, QuorumCertificateRef,
            SnapshotBootstrapAnchor, SnapshotV2BootstrapRecord, ValidatorPower,
            finality::{
                V2FinalityArtifact, V2FinalityValidationError, V2QuorumCertificateVerificationError,
            },
        },
        decode_framed_signed_block,
    },
    isi::kagemusha_v1::{
        KAGEMUSHA_CHAIN_VERSION_V1, KagemushaFinalityTrustAnchorV1, KagemushaOperationFinalityV1,
        KagemushaOperationKindV1, KagemushaReserveReceiptWitnessV1, KagemushaTopUpResultV1,
    },
    kaigi::KaigiId,
    merge::{
        LaneDrainNativeFrontierEvidenceV1, MAX_MERGE_EXECUTION_CERTIFIED_SOURCE_BYTES,
        MAX_MERGE_LEDGER_ENTRY_BYTES, MergeExecutionBatch, MergeLaneExecution, MergeLedgerEntry,
    },
    nexus::LaneCatalog,
    parliament_casting::{
        ParliamentTimedOvnCastingContextBindingV1,
        ParliamentTimedOvnCastingContextMembershipProofV1,
        ParliamentTimedOvnCastingSnapshotCommitmentV1, ParliamentTimedOvnCastingWitnessProofV1,
        ParliamentTimedOvnFinalizedCastingProofV1,
    },
    parliament_types::BallotAttemptId,
    transaction::signed::{TransactionEntrypoint, TransactionResult},
    validation_fee::ValidationFeePolicyWitnessProofV1,
};
use iroha_file_mmap::ReadOnlyMmap;
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal, spawn_os_thread_as_future};
use iroha_logger::prelude::*;
use iroha_model_base::domain::DomainId;
use iroha_model_base::name::Name;
use iroha_model_base::peer::PeerId;
use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
pub(crate) use lane_geometry::{
    RawGeometryAttempt, RawGeometryPhase, ReplayGeometryBindingRequest,
    StartupReplayGeometryTransition,
};
pub(crate) use membership_storage::MEMBERSHIP_RECORD_BYTES;
pub use membership_storage::{
    MembershipAppendCleanup, MembershipAppendRange, MembershipStorageError,
};
#[cfg(test)]
use norito::core::{Header, MAGIC};
use norito::{
    codec::{Decode, DecodeAll, Encode},
    json::Value as JsonValue,
};
use parking_lot::{Condvar, Mutex};
use snapshot_hash_journal::{checked_block_hash_read_range, verified_snapshot_hash_journal_digest};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    fmt::Debug,
    io::{BufWriter, ErrorKind, Read, Seek, SeekFrom, Write},
    num::{NonZeroU64, NonZeroUsize},
    ops::Bound,
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, RecvTimeoutError},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
impl From<CommittedBlock> for Arc<SignedBlock> {
    fn from(value: CommittedBlock) -> Self {
        Arc::new(value.into())
    }
}
const INDEX_FILE_NAME: &str = "blocks.index";
const DATA_FILE_NAME: &str = "blocks.data";
const HASHES_FILE_NAME: &str = "blocks.hashes";
const COUNT_FILE_NAME: &str = "blocks.count.norito";
const BOUND_PROGRESS_APPEND_INTENT_MAX_BYTES: usize = 128 * 1024;
// Ordinary append/replacement keeps its existing envelope. A bounded prepend
// owns both complete index images in the same durable intent.
const BOUND_PROGRESS_PREPEND_INDEX_MAX_BYTES: usize = INDEXED_SIDECAR_BASE_HEADER_SIZE
    + MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES * PIPELINE_INDEX_ENTRY_SIZE;
const BOUND_PROGRESS_APPEND_INTENT_DECODE_MAX_BYTES: usize =
    BOUND_PROGRESS_APPEND_INTENT_MAX_BYTES + 2 * BOUND_PROGRESS_PREPEND_INDEX_MAX_BYTES;

const MAX_BLOCK_COMMIT_MARKER_BYTES: usize = 1024;
const VERIFIED_SNAPSHOT_TAIL_FILE_NAME: &str = "verified_snapshot_tail.norito";
const MAX_VERIFIED_SNAPSHOT_TAIL_MARKER_BYTES: usize = 1024;
const STORE_ROOT_LOCK_FILE_NAME: &str = ".kura.lock";
const VERIFIED_SNAPSHOT_TAIL_DIGEST_DOMAIN: &[u8] = b"iroha:kura:verified-snapshot-tail:v1\0";
const PIPELINE_DIR_NAME: &str = "pipeline";

include!("kura/startup_finality_support.rs");
include!("kura/bound_progress_and_retained_support.rs");
#[cfg(test)]
std::thread_local! {
    static SIDECAR_DIRECTORY_CANONICALIZATIONS: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}
const PIPELINE_SIDECARS_DATA_FILE: &str = "sidecars.norito";
const PIPELINE_SIDECARS_INDEX_FILE: &str = "sidecars.index";
const PIPELINE_INDEX_ENTRY_SIZE: usize = core::mem::size_of::<u64>() * 2;
const PIPELINE_INDEX_ENTRY_SIZE_U64: u64 = PIPELINE_INDEX_ENTRY_SIZE as u64;
// Every present V1 index starts with `(MAX, MAX)` (invalid as a payload entry),
// followed by `(base_height, base_height ^ mask)`.
const INDEXED_SIDECAR_BASE_HEADER_SIZE: usize = PIPELINE_INDEX_ENTRY_SIZE * 2;
const INDEXED_SIDECAR_BASE_HEADER_SIZE_U64: u64 = INDEXED_SIDECAR_BASE_HEADER_SIZE as u64;
const INDEXED_SIDECAR_BASE_CHECK_MASK: u64 = 0x6B75_7261_2D69_6478;
/// Keep a single sparse append bounded even when its height came from untrusted metadata.
const MAX_INDEXED_SIDECAR_GAP_ENTRIES: u64 = 4_096;
const DISK_USAGE_TOTAL_REFRESH_INTERVAL: Duration = Duration::from_secs(60 * 60);
const BLOCK_NOTIFY_CHANNEL_CAPACITY: usize = 1;
const SIZE_OF_BLOCK_HASH: u64 = Hash::LENGTH as u64;
pub(crate) const STRICT_INIT_MAX_BLOCK_BYTES: u64 = MAX_EXECUTED_BLOCK_WIRE_BYTES;
const EVICTED_BLOCK_START: u64 = u64::MAX;
#[cfg(any(test, feature = "bench", feature = "iroha-core-tests"))]
fn checked_keypair() -> KeyPair {
    KeyPair::try_random().expect("kura fixture key generation should succeed")
}
#[cfg(test)]
fn checked_keypair_with_algorithm(algorithm: Algorithm) -> KeyPair {
    KeyPair::try_random_with_algorithm(algorithm)
        .expect("kura algorithm-specific fixture key generation should succeed")
}
#[cfg(any(test, feature = "bench", feature = "iroha-core-tests"))]
fn checked_peer_id() -> PeerId {
    PeerId::new(checked_keypair().public_key().clone())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HashOnlySnapshotExtensionMode {
    HardForkBootstrap,
    #[cfg(any(test, feature = "iroha-core-tests"))]
    VerifiedLocalSnapshot,
}
impl HashOnlySnapshotExtensionMode {
    fn label(self) -> &'static str {
        match self {
            Self::HardForkBootstrap => "hard-fork snapshot bootstrap",
            #[cfg(any(test, feature = "iroha-core-tests"))]
            Self::VerifiedLocalSnapshot => "verified local snapshot recovery",
        }
    }
    fn marks_hash_only_prefix(self) -> bool {
        matches!(self, Self::HardForkBootstrap)
    }
}
fn default_fastpq_proof_sidecar_queue_cap() -> usize {
    FASTPQ_DEFAULTS::PROOF_SIDECAR_QUEUE_CAP.get()
}
fn default_pipeline_sidecar_queue_cap() -> usize {
    BLOCKS_IN_MEMORY.get()
}
const EMERGENCY_FAST_RECENT_BLOCK_CACHE_CAPACITY: NonZeroUsize =
    NonZeroUsize::new(256).expect("the emergency Fast cache ceiling is non-zero");
fn default_fastpq_proof_sidecar_max_bytes() -> usize {
    usize::try_from(FASTPQ_DEFAULTS::PROOF_SIDECAR_MAX_BYTES.get())
        .unwrap_or(usize::MAX)
        .max(1)
}
fn default_fastpq_proof_sidecar_max_retries() -> usize {
    FASTPQ_DEFAULTS::PROOF_SIDECAR_MAX_RETRIES.get()
}
#[path = "kura/resource_inventory.rs"]
pub(crate) mod resource_inventory;
#[cfg(feature = "telemetry")]
mod resource_telemetry;
include!("kura/index_resource_accounting.rs");
include!("kura/evidence_resource_accounting.rs");
include!("kura/storage_resource_accounting.rs");
include!("kura/physical_resource_accounting.rs");
include!("kura/physical_resource_guard.rs");
include!("kura/canonical_physical_resource_accounting.rs");
include!("kura/physical_resource_initialization.rs");
#[cfg(test)]
#[path = "kura/physical_resource_accounting_tests.rs"]
mod physical_resource_accounting_tests;
#[cfg(test)]
#[path = "kura/physical_resource_initialization_tests.rs"]
mod physical_resource_initialization_tests;

use crate::publication_lock::{PublicationGuard, PublicationMutex};
pub(crate) use publication_lease::{
    KuraPublicationCleanup, KuraPublicationLease, KuraPublicationPreparationError,
};

/// The interface of Kura subsystem.
///
/// Merge-ledger persistence requirements are tracked in
/// `specs/merge_ledger.md`; follow that plan when wiring
/// global state checkpoints into storage.
#[derive(Debug)]
pub struct Kura {
    /// Configured finite maximum for an original native context archive record.
    native_context_archive_max_bytes: NonZeroUsize,
    /// One finite pool shared by every State hash generation using this store.
    block_hash_history_budget: mv::allocation::AllocationBudget,
    /// One finite pool shared by every State transaction-membership generation using this store.
    transaction_history_budget: mv::allocation::AllocationBudget,
    membership_storage: membership_storage::MembershipStorage,
    /// Exact owner-published resident and physical resources; never consensus authority.
    resource_inventory: Arc<resource_inventory::Inventory>,
    /// Process-local identity shared with sealed lifecycle storage authority.
    instance_identity: Arc<KuraInstanceIdentityMarker>,
    /// One anti-equivocation signer journal owner for every State using this storage instance.
    lane_drain_signing_guard:
        once_cell::sync::OnceCell<Arc<crate::lane_drain::LaneDrainSigningGuard>>,
    /// Opened canonical store-root owner used to mint descriptor-relative WAL storage.
    #[cfg(all(unix, not(target_os = "espidf")))]
    store_root_directory: BoundProgressDirectory,
    /// The block storage
    block_store: Mutex<BlockStore>,
    /// Serializes destructive canonical-chain changes with finality association and lane relabels.
    ///
    /// Operations that also hold `prune_lock` acquire that gate first. Inner locks retain their
    /// existing order: identity/path snapshots, then `block_store_write_lock`, then `block_store`.
    canonical_chain_lock: PublicationMutex,
    /// Serializes block-store writes while allowing reads during long eviction compaction.
    block_store_write_lock: Mutex<()>,
    /// Serializes canonical prune transactions from preflight through intent clearance.
    /// When combined with canonical-chain or lane locks, acquire this before
    /// `canonical_chain_lock`, `lane_geometry_lock`, and `sidecar_lock`.
    prune_lock: PublicationMutex,
    /// Rejects consensus-path sidecar enqueues while a canonical prune is active.
    ///
    /// Unlike `prune_lock`, this gate does not make enqueuers wait behind unrelated writer work
    /// that uses the same disk-mutation lock. Enqueuers check it both before and while holding
    /// their queue lock so prune start is linearizable with queue insertion.
    prune_in_progress: AtomicBool,
    /// Prevents a live Kura from serving or mutating a chain whose durable prune needs restart
    /// recovery.
    prune_recovery_required: AtomicBool,
    /// The array of block hashes and a slot for an arc of the block. This is normally recovered from the index file.
    block_data: ResidentMutex<BlockData>,
    /// Whether emergency Fast startup deliberately left historical auxiliary indexes unknown.
    auxiliary_history_deferred: bool,
    /// Number of pre-fork blocks whose body schema is intentionally not decoded in bootstrap mode.
    hard_fork_hash_only_block_count: AtomicUsize,
    /// Reverse lookup for committed block hash to block height.
    block_height_index: ResidentMutex<BlockHeightIndex>,
    /// Reverse lookup for committed transaction entrypoint hash to containing block heights.
    transaction_entrypoint_index: ResidentMutex<TransactionEntrypointIndex>,
    /// Channel for waking the writer thread when sidecars need flushing or shutdown is signalled.
    block_notify_tx: mpsc::SyncSender<BlockNotify>,
    block_notify_rx: Mutex<Option<mpsc::Receiver<BlockNotify>>>,
    /// Path to newline-delimited JSON (JSONL) block dump.
    block_plain_text_path: Mutex<Option<PathBuf>>,
    /// Serialize sidecar writes to avoid index/data races.
    sidecar_lock: PublicationMutex,
    /// Opaque permission for audited immutable reads of this exact sidecar fence.
    sidecar_read_permit: crate::publication_lock::PublicationReadPermit,
    /// Serializes the one durable process-generation claim for this Kura instance.
    autonomous_lifecycle_process_generation_lock: Mutex<()>,
    /// Process-local cache of the exact durable generation claimed after peer binding.
    autonomous_lifecycle_process_generation_claim:
        OnceLock<AutonomousLifecycleProcessGenerationClaim>,
    /// Serializes complete historical-autonomous recovery preflight/install batches.
    historical_autonomous_recovery_mutation_lock: Mutex<()>,
    /// Bounded identities of immutable v2 finality sidecars already BLS-verified.
    v2_finality_verification_cache: ResidentMutex<VecDeque<VerifiedV2FinalityCacheEntry>>,
    /// Startup-scoped identities produced by the complete finality inventory audit.
    ///
    /// The replay planner and Sumeragi recovery both rescan the same immutable
    /// artifacts before ingress opens. This inventory lets those scans reuse
    /// the audit only while the exact bytes and stable filesystem identity are
    /// unchanged. It is cleared after active-height recovery; the ordinary
    /// runtime cache remains fixed at [`V2_FINALITY_VERIFICATION_CACHE_CAPACITY`].
    v2_startup_finality_verification_inventory:
        Mutex<Option<Arc<V2StartupFinalityVerificationInventory>>>,
    /// Private publication paired with the audit under inventory-then-publication lock order.
    /// Every pair install, snapshot and clear holds the inventory lock throughout.
    v2_startup_replay_geometry_publication:
        Mutex<Option<Arc<lane_geometry::StartupReplayGeometryPublication>>>,
    /// Counts every live startup inventory allocation, including escaped Arc readers.
    startup_inventory_resident:
        Arc<ResidentMutex<resident_inventory_lifetimes::VerificationAllocations>>,
    /// Serialize sparse merge-carrier index publication and reconciliation.
    merge_carrier_lock: Mutex<()>,
    /// Validated in-memory sparse carrier maps loaded during startup reconciliation.
    merge_carrier_index: ResidentMutex<MergeCarrierIndex>,
    /// Queue of pipeline sidecar writes flushed by the Kura writer thread.
    pipeline_sidecar_queue: ResidentMutex<VecDeque<PipelineRecoverySidecar>>,
    /// Maximum queued pipeline sidecar writes.
    pipeline_sidecar_queue_cap: AtomicUsize,
    /// Queue of FASTPQ proof attachments merged into existing pipeline sidecars.
    fastpq_proof_queue: ResidentMutex<VecDeque<QueuedFastpqProofSnapshot>>,
    /// Maximum queued FASTPQ proof sidecar attachments.
    fastpq_proof_sidecar_queue_cap: AtomicUsize,
    /// Maximum encoded FASTPQ proof snapshot bytes accepted for persistence.
    fastpq_proof_sidecar_max_bytes: AtomicUsize,
    /// Maximum merge attempts for a pending FASTPQ proof sidecar.
    fastpq_proof_sidecar_max_retries: AtomicUsize,
    /// Root directory where Kura stores lane segments.
    store_root: PathBuf,
    /// Fixed canonical-chain directory, independent of every lane alias and incarnation.
    active_blocks_dir: Mutex<PathBuf>,
    /// Fixed canonical merge-ledger path, independent of lane geometry.
    active_merge_path: Mutex<PathBuf>,
    /// Current lane storage entries, keyed by lane id, used for lane-local artifact placement.
    lane_storage_entries: ResidentMutex<BTreeMap<LaneId, LaneStorageEntry>>,
    lane_storage_network: Mutex<Option<NetworkId>>,
    /// Monotonic wake-up generation for committed-lane operator status.
    committed_lane_status_revision: AtomicU64,
    /// Fail-stop latch for an ambiguous latest-certified frontier publication boundary.
    latest_certified_frontier_storage_unknown: AtomicBool,
    /// Restart-empty proof that the exact pair completed its strict barriers;
    /// artifact, pair metadata and every directory generation gate fsync reuse.
    certified_pair_durability: ResidentMutex<BTreeMap<LaneId, CertifiedPairDurabilityAttestation>>,
    /// Bounded original directory handles; never cached canonical/application authority.
    lane_receipt_namespace_durability: ResidentMutex<LaneReceiptNamespaceDurability>,
    /// Bounded restart-empty proof that exact stable frontier bytes completed
    /// full certificate validation and subsequent pair repair/readback.
    certified_frontier_artifact_validation:
        ResidentMutex<BTreeMap<LaneId, CertifiedFrontierArtifactValidationAttestation>>,
    /// Serializes lifecycle geometry moves, snapshot checkpoints, and archive garbage collection.
    /// Acquire it after `prune_lock` and before `sidecar_lock` when locks are combined.
    lane_geometry_lock: PublicationMutex,
    /// Exact in-process operation custody while physical geometry locks are released.
    raw_geometry_claim: lane_geometry::RawGeometryClaimGate,
    /// Maximum on-disk footprint for Kura block storage (0 = unlimited).
    max_disk_usage_bytes: u64,
    /// Distinct remote peers required before Kura may evict a local canonical block body.
    eviction_required_replicas: NonZeroUsize,
    /// Authoritative process-local peer identity used to pin selected keeper bodies.
    local_peer_id: OnceLock<PeerId>,
    /// Recently authenticated exact canonical block replicas.
    replica_registry: ResidentMutex<BlockReplicaRegistry>,
    /// Protected-tail plus historical-window capacity for exact canonical advert identities.
    replica_registry_key_capacity: NonZeroUsize,
    /// Number of historical advert identities retained immediately before the protected tail.
    replica_advert_evictable_window: NonZeroUsize,
    /// Lifetime of one authenticated remote replica observation.
    replica_advert_ttl: Duration,
    /// Cadence for proactively refreshing selected-keeper replica adverts.
    replica_advert_refresh_interval: Duration,
    /// Cached disk usage for budget enforcement.
    disk_usage: AtomicU64,
    /// Cached total disk usage.
    disk_usage_total: AtomicU64,
    /// Serializes total-usage scans with filesystem mutations that publish total-cache deltas.
    disk_usage_total_accounting: Mutex<TotalDiskUsageAccountingState>,
    /// Wakes total-usage scanners after the last in-flight filesystem mutation completes.
    disk_usage_total_accounting_changed: Condvar,
    /// Cached sum of budgeted bytes for in-memory blocks not yet durably indexed.
    pending_budget_bytes: AtomicU64,
    /// Marks whether `pending_budget_bytes` currently reflects in-memory block state.
    pending_budget_bytes_valid: AtomicBool,
    /// Carrier envelopes retained until every exact receipt, frontier, and Queue outcome is durable.
    post_wsv_lane_artifact_budget_reservations:
        ResidentMutex<NestedMap<HashOf<MergeLedgerEntry>, PostWsvLaneArtifactBudgetReservation>>,
    /// READY-bearing certified frontiers whose exact certified/bundle pairs
    /// have not both completed strict durable readback.
    certified_bundle_capacity_reservations: ResidentMutex<
        NestedMap<CertifiedBundleCapacityIdentity, CertifiedBundleCapacityReservation>,
    >,
    /// Full authenticated reservation reconstruction has completed for this process.
    native_amx_publication_capacity_reservations: ResidentMutex<
        NestedMap<NativeAmxPublicationCarrier, NativeAmxPublicationCapacityReservation>,
    >,
    native_amx_resident_recovery_complete: AtomicBool,
    post_wsv_resident_recovery_complete: AtomicBool,
    /// Full certified/bundle reservation reconstruction has completed for this process.
    certified_resident_recovery_complete: AtomicBool,
    /// Counts raw pending-budget scans for focused cache tests.
    #[cfg(test)]
    pending_budget_raw_scans: AtomicUsize,
    /// Coalesced reclaim request for the writer thread's storage-budget maintenance pass.
    pending_budget_eviction_bytes: AtomicU64,
    /// Cached durable block count for budget accounting.
    durable_budget_persisted_count: AtomicUsize,
    /// Cached unindexed block-store bytes for budget accounting.
    durable_budget_unindexed_bytes: AtomicU64,
    /// Marks whether the durable budget metadata cache is usable.
    durable_budget_snapshot_valid: AtomicBool,
    /// Indicates whether the budget usage cache was initialized successfully.
    disk_usage_initialized: AtomicBool,
    /// Indicates whether the total usage cache was initialized successfully.
    disk_usage_total_initialized: AtomicBool,
    /// Last successful total-usage refresh time (seconds since UNIX epoch).
    disk_usage_total_last_refresh: AtomicU64,
    /// Number of most recent non-genesis blocks stored in memory.
    /// The genesis block is always retained for metrics and replay.
    blocks_in_memory: NonZeroUsize,
    /// Number of recent lane, autonomous, and Native AMX history entries retained.
    lane_history_retention: NonZeroUsize,
    fastpq_artifact_policy: iroha_config::parameters::actual::KuraFastpqArtifactPolicy,
    /// Fingerprint-bound limits shared by pre-carrier controls and historical recovery evidence.
    pending_control_sidecar_limits: PendingControlSidecarLimits,
    /// Exact maximum encoded Native AMX pair-prune journal for the configured
    /// retained-history window.
    native_amx_evidence_prune_intent_max_bytes: usize,
    /// On-disk merge-ledger log and in-memory cache.
    merge_log: ResidentMutex<MergeLedgerLog>,
    /// Optional telemetry sink for storage and durable finality reporting.
    telemetry: OnceLock<StateTelemetry>,
    /// Last fatal writer fault observed by the background persistence loop.
    writer_fault: Mutex<Option<String>>,
    /// Fail-stop latch for an ambiguous canonical-journal publication boundary.
    canonical_storage_poisoned: AtomicBool,
    /// Hash-only opening authority still awaiting signed snapshot authentication.
    provisional_snapshot_bootstrap: Mutex<SnapshotBootstrapRuntimeState>,
    /// Serializes canonical poison publication with consensus-guard binding.
    canonical_poison_binding_lock: Mutex<()>,
    /// Consensus admission guard bound before the authoritative worker is published.
    consensus_output_guard: OnceLock<Arc<ConsensusOutputGuard>>,
    /// Test hook that pauses canonical poison after publishing its latch.
    #[cfg(test)]
    pause_canonical_poison_after_latch: AtomicBool,
    /// Test hook indicating canonical poison is paused before checking the bound guard.
    #[cfg(test)]
    canonical_poison_paused_after_latch: AtomicBool,
    /// Test hook for forcing the next synchronous block write to fail after pre-write work.
    #[cfg(test)]
    fail_next_block_write: AtomicBool,
    #[cfg(test)]
    fail_next_atomic_write_after_temporary_sync: AtomicBool,
    /// Test hook for forcing the next WSV checkpoint sidecar write to fail.
    #[cfg(test)]
    fail_next_wsv_checkpoint_write: AtomicBool,
    /// Test hook for forcing the next commit manifest sidecar write to fail.
    #[cfg(test)]
    fail_next_commit_manifest_write: AtomicBool,
    /// Test hook for forcing redundant committed-pending cleanup to fail.
    #[cfg(test)]
    fail_next_pending_merge_cleanup: AtomicBool,
    /// Test hook pausing a merge-carrier store after its pending entry is durable.
    #[cfg(test)]
    pause_store_after_pending_merge_stage: AtomicBool,
    /// Indicates that a merge-carrier store is paused after pending-entry staging.
    #[cfg(test)]
    store_paused_after_pending_merge_stage: AtomicBool,
    /// Test hook for failing association recovery after a canonical stage exists.
    #[cfg(test)]
    fail_next_canonical_association_recovery: AtomicBool,
    /// Test hook for failing removal of an existing canonical association stage.
    #[cfg(test)]
    fail_next_canonical_association_cleanup: AtomicBool,
    /// Test hook for forcing the next lane-geometry catalog publication to fail.
    #[cfg(test)]
    fail_next_lane_geometry_publication: AtomicBool,
    /// Test hook for failing catalog publication after its journal target was replaced.
    #[cfg(test)]
    fail_next_lane_geometry_publication_after_write: AtomicBool,
    /// Test hook selecting a crash boundary in lane-geometry archive garbage collection.
    #[cfg(test)]
    fail_lane_geometry_gc_stage: AtomicUsize,
    /// Test hook selecting a crash boundary in canonical prune recovery.
    #[cfg(test)]
    fail_prune_after_stage: AtomicUsize,
    /// Test hook forcing a fallible indexed-sidecar prune promotion to stop before rename.
    #[cfg(test)]
    fail_prune_sidecar_promotion_stage: AtomicUsize,
    /// Test hook that pauses a canonical prune after all mutable read locks are held but before
    /// its durable intent is published.
    #[cfg(test)]
    pause_prune_before_intent: AtomicBool,
    /// Indicates that the prune-before-intent test hook is currently paused.
    #[cfg(test)]
    prune_paused_before_intent: AtomicBool,
    /// Enables observation of canonical readers after their initial prune-poison check.
    #[cfg(test)]
    observe_canonical_reads_after_prune_check: AtomicBool,
    /// Bitmask of observed canonical reader kinds after their initial prune-poison check.
    #[cfg(test)]
    canonical_read_kinds_after_prune_check: AtomicUsize,
    /// Test hook pausing after exact instance preparation and before reference publication.
    #[cfg(test)]
    pause_geometry_reference_publication: AtomicBool,
    /// Test hook indicating that geometry and sidecar authority are held at reference publication.
    #[cfg(test)]
    geometry_reference_publication_paused: AtomicBool,
    /// Test hook for forcing the next Sumeragi v2 finality sidecar write to fail.
    #[cfg(test)]
    fail_next_v2_finality_write: AtomicBool,
    /// Test hook for forcing the next pre-WSV Native AMX evidence publication to fail.
    #[cfg(test)]
    fail_next_native_amx_prepublication: AtomicBool,
    /// Counts actual v2 finality BLS verification passes for cache tests.
    #[cfg(test)]
    v2_finality_crypto_verifications: AtomicUsize,
    /// Number of historical payload files reopened after the startup replay
    /// projection was built.
    #[cfg(test)]
    startup_replay_historical_payload_reads: AtomicUsize,
    /// Number of complete active historical-recovery inventory scans.
    #[cfg(test)]
    historical_autonomous_recovery_inventory_scans: AtomicUsize,
    #[cfg(test)]
    pub(crate) pending_queue_plan_admission_inventory_scans: AtomicUsize,
    #[cfg(test)]
    pub(crate) pending_queue_plan_admission_exact_reads: AtomicUsize,
    #[cfg(test)]
    pub(crate) pending_queue_plan_admission_batch_validations: AtomicUsize,
    /// Test hook failing retained-rewrite stage discard after a selected removal.
    #[cfg(test)]
    fail_retained_rewrite_discard_after: AtomicUsize,
    /// Test hook for forcing immediate recovery of a retained rewrite stage to fail.
    #[cfg(test)]
    fail_next_retained_rewrite_recovery: AtomicBool,
    /// Test hook failing a retired-tree purge after one deterministic file removal.
    #[cfg(test)]
    fail_next_retired_tree_purge_after_one_removal: AtomicBool,
    /// Counts raw durable-budget metadata reads for focused cache tests.
    #[cfg(test)]
    durable_budget_metadata_reads: AtomicUsize,
    /// Test hook that pauses eviction after the block-store snapshot is captured.
    #[cfg(test)]
    pause_eviction_after_snapshot: AtomicBool,
    /// Test hook indicating eviction is paused after releasing the block-store lock.
    #[cfg(test)]
    eviction_paused_after_snapshot: AtomicBool,
    /// Test hook that pauses eviction immediately before the final keeper-freshness check.
    #[cfg(test)]
    pause_eviction_before_stage_publication: AtomicBool,
    /// Test hook indicating eviction reached its last pre-publication freshness boundary.
    #[cfg(test)]
    eviction_paused_before_stage_publication: AtomicBool,
    /// Number of complete body reads performed by local replica-advert build/revalidation.
    #[cfg(test)]
    kura_replica_advert_body_reads: AtomicUsize,
    /// Test hook that pauses an inline read immediately before its cache publication recheck.
    #[cfg(test)]
    pause_block_read_before_cache_recheck: AtomicBool,
    /// Test hook indicating an inline read is paused before its cache publication recheck.
    #[cfg(test)]
    block_read_paused_before_cache_recheck: AtomicBool,
    /// Test hook forcing `durable_blocks_count` through its in-memory fallback.
    #[cfg(test)]
    force_durable_blocks_count_fallback: AtomicBool,
    /// Test hook indicating the durable-count fallback is about to acquire `block_data`.
    #[cfg(test)]
    durable_blocks_count_fallback_reached: AtomicBool,
    /// Test hook pausing hash-only snapshot extension while it owns `block_data`.
    #[cfg(test)]
    pause_hash_only_extension_before_store: AtomicBool,
    /// Test hook indicating hash-only snapshot extension is paused before block-store access.
    #[cfg(test)]
    hash_only_extension_paused_before_store: AtomicBool,
    /// Test hook that pauses a total-usage refresh after its filesystem scan.
    #[cfg(test)]
    pause_total_disk_usage_scan_after_scan: AtomicBool,
    /// Test hook indicating a total-usage refresh is paused before publication.
    #[cfg(test)]
    total_disk_usage_scan_paused: AtomicBool,
    /// Exclusive OS lock dropped after every Kura resource but before temporary-directory cleanup.
    _store_root_lock_file: Option<std::fs::File>,
    /// Retains the temporary storage directory used by isolated Kura instances.
    _temp_store_dir: Option<tempfile::TempDir>,
}
#[derive(Debug)]
struct KuraInstanceIdentityMarker;
/// Comparison-only identity for one exact live Kura instance.
///
/// This seal carries no storage path and cannot reopen Kura. Lifecycle startup
/// retains it solely to prevent a canonical owner built under one Kura from
/// launching its workers against another.
#[derive(Clone, Debug)]
pub(crate) struct KuraInstanceIdentity(Arc<KuraInstanceIdentityMarker>);
impl KuraInstanceIdentity {
    /// Return whether this seal came from the exact supplied Kura instance.
    pub(crate) fn matches(&self, kura: &Kura) -> bool {
        Arc::ptr_eq(&self.0, &kura.instance_identity)
    }
}
#[path = "kura/resident_inventory.rs"]
mod resident_inventory;
use resident_inventory::{AssociationCount, ResidentMutex};
#[path = "kura/resident_inventory_lifetimes.rs"]
mod resident_inventory_lifetimes;
#[path = "kura/resident_nested_map.rs"]
mod resident_nested_map;
use resident_nested_map::NestedMap;
#[cfg(test)]
#[path = "kura/resident_remaining_inventory_tests.rs"]
mod resident_remaining_inventory_tests;
include!("kura/resident_inventory_remaining_owners.rs");
include!("kura/prune_commit_merge_support.rs");
include!("kura/resident_inventory_owners.rs");
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BlockNotify {
    NewBlock,
    StorageBudgetEviction,
    Shutdown,
}
impl Kura {
    /// Retain the original configured pool for State history construction and edits.
    pub(crate) fn block_hash_history_budget(&self) -> mv::allocation::AllocationBudget {
        self.block_hash_history_budget.clone()
    }
    /// Retain the original configured pool through membership restore, replay, and edits.
    pub(crate) fn transaction_history_budget(&self) -> mv::allocation::AllocationBudget {
        self.transaction_history_budget.clone()
    }
    fn notify_block_writer_sender(
        sender: &mpsc::SyncSender<BlockNotify>,
        notification: BlockNotify,
        context: &'static str,
    ) {
        match sender.try_send(notification) {
            Ok(()) => {}
            Err(mpsc::TrySendError::Full(_)) => {
                debug!(
                    ?notification,
                    context, "coalesced redundant Kura writer notification"
                );
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                warn!(
                    ?notification,
                    context, "failed to notify Kura writer because channel is closed"
                );
            }
        }
    }
    fn notify_block_writer(&self, notification: BlockNotify, context: &'static str) {
        Self::notify_block_writer_sender(&self.block_notify_tx, notification, context);
    }
    fn ensure_prune_recovery_not_required(&self) -> Result<()> {
        if self.prune_recovery_required.load(Ordering::Acquire) {
            return Err(Error::PruneRecoveryRequired);
        }
        Ok(())
    }
    fn prune_recovery_is_required(&self) -> bool {
        self.prune_recovery_required.load(Ordering::Acquire)
    }
    /// Return whether a consensus-path sidecar enqueue must fail closed for pruning.
    ///
    /// Callers repeat this check while holding the queue mutex. A queue insertion whose locked
    /// check observes `false` linearizes before prune start and is covered by prune's queue
    /// truncation. Once prune start publishes `true`, no later insertion is accepted.
    fn prune_blocks_sidecar_enqueue(&self) -> bool {
        self.prune_in_progress.load(Ordering::Acquire) || self.prune_recovery_is_required()
    }
    fn build_block_height_index(block_data: &BlockData) -> BlockHeightIndex {
        let Some(entries) = block_data.dense_entries() else {
            return HashMap::new();
        };
        let mut index = HashMap::with_capacity(block_data.len());
        for (offset, (hash, _)) in entries.iter().enumerate() {
            if let Some(height) = NonZeroUsize::new(offset.saturating_add(1)) {
                index.entry(*hash).or_insert(height);
            }
        }
        index
    }
    fn build_transaction_entrypoint_index(block_data: &BlockData) -> TransactionEntrypointIndex {
        let mut index = TransactionEntrypointIndex::complete_empty();
        index.complete = false;
        let Some(entries) = block_data.dense_entries() else {
            return index;
        };
        for (offset, (expected_hash, block)) in entries.iter().enumerate() {
            let Some(height) = NonZeroUsize::new(offset.saturating_add(1)) else {
                continue;
            };
            if let Some(block) = block
                .as_ref()
                .filter(|block| block.hash() == *expected_hash)
            {
                Self::insert_transaction_entrypoint_heights(&mut index, height, block);
            } else {
                index.incomplete_heights.insert(height);
            }
        }
        index.complete =
            index.incomplete_heights.is_empty() && index.indexed_heights.len() == block_data.len();
        index
    }

    /// Publish only a complete, structurally validated Network projection. Physical
    /// canonical body/finality admission remains the caller's existing responsibility.
    fn insert_transaction_entrypoint_heights(
        index: &mut TransactionEntrypointIndex,
        height: NonZeroUsize,
        block: &SignedBlock,
    ) {
        Self::remove_transaction_entrypoint_height(index, height);
        if u64::try_from(height.get()).ok() != Some(block.header().height().get())
            || block.execution_context().is_some_and(|context| {
                !context.has_current_version() || context.merge_entry.is_some()
            })
            || block.validate_output_merkle_cache().is_err()
            || u32::try_from(block.network_entrypoint_count()).is_err()
        {
            index.incomplete_heights.insert(height);
            index.complete = false;
            return;
        }
        // Validate the entire carrier before any membership is exposed, including
        // internal outputs, source joins and every retained Merkle cache node.
        index.inventories_by_height.entry(height).or_default();
        for (input_index, entrypoint) in block.network_entrypoints().enumerate() {
            let input_index = u32::try_from(input_index).expect("validated Network index range");
            let (_, output) = block
                .network_output_at(input_index)
                .expect("validated complete Network output join");
            let hash = entrypoint.hash();
            let inserted = index
                .inventories_by_height
                .entry(height)
                .or_default()
                .entrypoint_hashes
                .insert(hash);
            index.nested_associations.inserted(inserted);
            let inserted = index
                .heights_by_entrypoint
                .entry(hash)
                .or_default()
                .insert(height);
            index.nested_associations.inserted(inserted);
            if let Some(authority) = entrypoint.authority_opt() {
                let inserted = index
                    .inventories_by_height
                    .entry(height)
                    .or_default()
                    .authorities
                    .insert(authority.clone());
                index.nested_associations.inserted(inserted);
                let inserted = index
                    .heights_by_authority
                    .entry(authority.clone())
                    .or_default()
                    .insert(height);
                index.nested_associations.inserted(inserted);
            }
            if let Some(timestamp_ms) = entrypoint.creation_time_ms() {
                let inserted = index
                    .inventories_by_height
                    .entry(height)
                    .or_default()
                    .timestamps_ms
                    .insert(timestamp_ms);
                index.nested_associations.inserted(inserted);
                let inserted = index
                    .heights_by_timestamp_ms
                    .entry(timestamp_ms)
                    .or_default()
                    .insert(height);
                index.nested_associations.inserted(inserted);
            }
            let status = output.result.is_ok();
            let inserted = index
                .inventories_by_height
                .entry(height)
                .or_default()
                .result_statuses
                .insert(status);
            index.nested_associations.inserted(inserted);
            let inserted = index
                .heights_by_result_status
                .entry(status)
                .or_default()
                .insert(height);
            index.nested_associations.inserted(inserted);
            if !Self::insert_kaigi_signal_candidate(
                index,
                height,
                block.hash(),
                input_index,
                entrypoint,
                &output.result,
            ) {
                Self::remove_transaction_entrypoint_height(index, height);
                index.incomplete_heights.insert(height);
                index.complete = false;
                return;
            }
        }
        index.indexed_heights.insert(height);
    }

    pub(crate) fn kaigi_signal_candidate_identity(
        entrypoint: &TransactionEntrypoint,
        result: &TransactionResult,
    ) -> Option<(KaigiId, AccountId)> {
        if result.as_ref().is_err() {
            return None;
        }
        let transaction = match entrypoint {
            TransactionEntrypoint::External(transaction) => transaction,
            TransactionEntrypoint::SealedReveal(reveal) => reveal.signed_transaction(),
            TransactionEntrypoint::SealedCommitment(_) => {
                return None;
            }
        };
        let key = "kaigi_signal".parse::<Name>().ok()?;
        let signal = transaction
            .metadata()
            .get(&key)?
            .try_into_any_norito::<JsonValue>()
            .ok()?;
        let object = signal.as_object()?;
        if object.get("schema")?.as_str()? != "iroha-demo-kaigi-chain-signal/v1" {
            return None;
        }
        let mut call_literal = None;
        for key in ["callId", "call_id"] {
            let Some(value) = object.get(key) else {
                continue;
            };
            let candidate = value.as_str()?;
            if call_literal.is_some_and(|existing| existing != candidate) {
                return None;
            }
            call_literal = Some(candidate);
        }
        let call_literal = call_literal?;
        let (domain_literal, call_name_literal) = call_literal.split_once(':')?;
        if call_name_literal.contains(':') {
            return None;
        }
        let call_id = KaigiId::new(
            DomainId::parse_fully_qualified(domain_literal).ok()?,
            call_name_literal.parse::<Name>().ok()?,
        );
        if call_id.to_string() != call_literal {
            return None;
        }
        Some((call_id, transaction.authority().clone()))
    }

    fn insert_kaigi_signal_candidate(
        index: &mut TransactionEntrypointIndex,
        height: NonZeroUsize,
        block_hash: HashOf<BlockHeader>,
        network_input_index: u32,
        entrypoint: &TransactionEntrypoint,
        result: &TransactionResult,
    ) -> bool {
        let Some((call_id, authority)) = Self::kaigi_signal_candidate_identity(entrypoint, result)
        else {
            return true;
        };
        let Ok(block_height) = u64::try_from(height.get()) else {
            return false;
        };
        let position = KaigiSignalCandidatePosition {
            block_height,
            network_input_index,
            block_hash,
            entrypoint_hash: entrypoint.hash(),
        };
        let locator = KaigiSignalCandidateLocator {
            position,
            authority,
        };
        let inserted = index
            .inventories_by_height
            .entry(height)
            .or_default()
            .kaigi_calls
            .insert(call_id.clone());
        index.nested_associations.inserted(inserted);
        match index
            .kaigi_signal_candidates
            .entry(call_id)
            .or_default()
            .entry(height)
            .or_default()
            .insert(network_input_index, locator.clone())
        {
            None => {
                index.nested_associations.inserted(true);
                true
            }
            Some(existing) => existing == locator,
        }
    }
    fn remove_transaction_entrypoint_height(
        index: &mut TransactionEntrypointIndex,
        height: NonZeroUsize,
    ) {
        let inventory = index
            .inventories_by_height
            .remove(&height)
            .unwrap_or_default();
        index.nested_associations.removed(
            resident_inventory::lengths([
                inventory.entrypoint_hashes.len(),
                inventory.authorities.len(),
                inventory.timestamps_ms.len(),
                inventory.result_statuses.len(),
                inventory.kaigi_calls.len(),
            ])
            .ok(),
        );
        let removed = Self::remove_transaction_height_for_keys(
            &mut index.heights_by_entrypoint,
            inventory.entrypoint_hashes,
            height,
        );
        index.nested_associations.removed(removed);
        let removed = Self::remove_transaction_height_for_keys(
            &mut index.heights_by_authority,
            inventory.authorities,
            height,
        );
        index.nested_associations.removed(removed);
        let removed = Self::remove_transaction_height_for_keys(
            &mut index.heights_by_timestamp_ms,
            inventory.timestamps_ms,
            height,
        );
        index.nested_associations.removed(removed);
        let removed = Self::remove_transaction_height_for_keys(
            &mut index.heights_by_result_status,
            inventory.result_statuses,
            height,
        );
        index.nested_associations.removed(removed);
        for call_id in inventory.kaigi_calls {
            let remove_call =
                index
                    .kaigi_signal_candidates
                    .get_mut(&call_id)
                    .is_some_and(|heights| {
                        if let Some(candidates) = heights.remove(&height) {
                            index
                                .nested_associations
                                .removed(u64::try_from(candidates.len()).ok());
                        }
                        heights.is_empty()
                    });
            if remove_call {
                index.kaigi_signal_candidates.remove(&call_id);
            }
        }
        index.indexed_heights.remove(&height);
        index.incomplete_heights.remove(&height);
    }

    fn remove_transaction_height_for_keys<K: Ord>(
        indexed: &mut BTreeMap<K, BTreeSet<NonZeroUsize>>,
        keys: impl IntoIterator<Item = K>,
        height: NonZeroUsize,
    ) -> Option<u64> {
        let mut removed = Some(0_u64);
        for key in keys {
            let remove_key = indexed.get_mut(&key).is_some_and(|heights| {
                if heights.remove(&height) {
                    removed = removed.and_then(|count| count.checked_add(1));
                }
                heights.is_empty()
            });
            if remove_key {
                indexed.remove(&key);
            }
        }
        removed
    }
    fn set_transaction_entrypoint_index_entry(
        &self,
        height: usize,
        block: &SignedBlock,
        chain_len: usize,
    ) {
        let Some(height) = NonZeroUsize::new(height) else {
            return;
        };
        let mut index = self.transaction_entrypoint_index.lock();
        Self::insert_transaction_entrypoint_heights(&mut index, height, block);
        index.complete =
            index.incomplete_heights.is_empty() && index.indexed_heights.len() == chain_len;
    }

    fn mark_transaction_entrypoint_index_incomplete(&self, height: usize, chain_len: usize) {
        let Some(height) = NonZeroUsize::new(height) else {
            return;
        };
        let mut index = self.transaction_entrypoint_index.lock();
        Self::remove_transaction_entrypoint_height(&mut index, height);
        index.incomplete_heights.insert(height);
        index.complete =
            index.incomplete_heights.is_empty() && index.indexed_heights.len() == chain_len;
    }

    fn set_block_height_index_entry(&self, height: usize, hash: HashOf<BlockHeader>) {
        let Some(height) = NonZeroUsize::new(height) else {
            return;
        };
        let mut index = self.block_height_index.lock();
        index.retain(|_, indexed_height| *indexed_height != height);
        index.insert(hash, height);
    }
}
impl Kura {
    /// Open a crate test fixture through the production authenticated-catalog boundary.
    ///
    /// This helper exists only in unit-test builds. It reconstructs the
    /// canonical storage-bearing catalog projection supplied by the fixture and
    /// delegates to [`Self::new_with_configured_lane_catalog`]; it never calls
    /// [`Self::new_inner`] or weakens any shipping validation. Fixtures that
    /// exercise a full catalog commitment use the production constructor
    /// directly with that exact catalog.
    #[cfg(test)]
    pub(crate) fn open_test_kura_with_configured_lane_config(
        config: &Config,
        lane_config: &LaneConfig,
    ) -> Result<(Arc<Self>, BlockCount)> {
        let store_root = config.store_dir.resolve_relative_path();
        let maximum_lane_id = lane_config
            .entries()
            .iter()
            .map(|entry| entry.lane_id.as_u32())
            .max()
            .ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidInput,
                        "test lane configuration must contain a primary lane",
                    ),
                    store_root.clone(),
                )
            })?;
        let lane_count = maximum_lane_id
            .checked_add(1)
            .and_then(std::num::NonZeroU32::new)
            .ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidInput,
                        "test lane configuration exceeds the lane identifier namespace",
                    ),
                    store_root.clone(),
                )
            })?;
        let configured_lanes = lane_config
            .entries()
            .iter()
            .map(|entry| iroha_data_model::nexus::LaneConfig {
                id: entry.lane_id,
                shard_id: (entry.shard_id != entry.lane_id.as_u32())
                    .then_some(iroha_model_base::topology::ShardId::new(entry.shard_id)),
                dataspace_id: entry.dataspace_id,
                alias: entry.alias.clone(),
                description: None,
                visibility: entry.visibility,
                lane_type: None,
                governance: None,
                settlement: None,
                storage: entry.storage_profile,
                proof_scheme: entry.proof_scheme,
                manifest_policy: entry.manifest_policy,
                confidential_compute: entry.confidential_compute.clone(),
                scheduler: entry.scheduler,
                settlement_buffer: entry.settlement_buffer.clone(),
                metadata: BTreeMap::new(),
            })
            .collect();
        let configured_lane_catalog =
            LaneCatalog::new(lane_count, configured_lanes).map_err(|error| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidInput,
                        format!("invalid configured lane catalog for Kura test fixture: {error}"),
                    ),
                    store_root,
                )
            })?;
        Self::new_with_configured_lane_catalog(config, lane_config, &configured_lane_catalog)
    }
    /// Initialize authenticated Kura with exact fingerprint-bound Sumeragi v2
    /// pending-control persistence limits.
    ///
    /// This is the production constructor. It validates the limits before any
    /// Kura-owned path is created or opened, then applies them to crash
    /// recovery and every subsequent pending-sidecar operation.
    ///
    /// # Errors
    ///
    /// Returns an error when snapshot policy, lane geometry, or pending-control
    /// limits are invalid, or when authenticated Kura startup fails.
    pub fn new_with_configured_lane_catalog_and_snapshot_bootstrap_and_sumeragi_limits(
        config: &Config,
        lane_config: &LaneConfig,
        configured_lane_catalog: &LaneCatalog,
        bootstrap_policy: &SnapshotBootstrapPolicy,
        sumeragi_limits: &SumeragiV2RuntimeLimits,
    ) -> Result<(Arc<Self>, BlockCount)> {
        bootstrap_policy.validate().map_err(|message| {
            Error::IO(
                std::io::Error::new(ErrorKind::InvalidInput, message),
                config.store_dir.resolve_relative_path(),
            )
        })?;
        let pending_control_sidecar_limits = if config.init_mode == InitMode::Fast {
            // Consensus and every sidecar writer remain disabled for the whole Fast process.
            // Do not let their unused production bounds delay or reject an emergency read boot.
            PendingControlSidecarLimits::default()
        } else {
            PendingControlSidecarLimits::from_config(
                sumeragi_limits,
                &config.store_dir.resolve_relative_path(),
            )?
        };
        let provisional_hash_only_prefix = bootstrap_policy
            .enabled
            .then_some(bootstrap_policy.audited_height)
            .flatten()
            .map(usize::try_from)
            .transpose()?;
        Self::new_with_configured_lane_catalog_inner(
            config,
            lane_config,
            configured_lane_catalog,
            provisional_hash_only_prefix,
            !bootstrap_policy.enabled,
            pending_control_sidecar_limits,
        )
    }
    /// Initialize an authenticated Kura in an isolated temporary directory.
    ///
    /// This applies the same configured-catalog authentication as
    /// [`Self::new_with_configured_lane_catalog`] while binding the directory lifetime to the
    /// returned Kura. It is intended for non-committing genesis validation and other transient
    /// staging that must exercise production lane-storage invariants without touching node data.
    ///
    /// # Errors
    ///
    /// Returns an error if the temporary directory cannot be created or the configured lane
    /// catalog cannot initialize an authenticated Kura inside it.
    pub fn new_temporary_with_configured_lane_catalog(
        config: &Config,
        lane_config: &LaneConfig,
        configured_lane_catalog: &LaneCatalog,
    ) -> Result<Arc<Self>> {
        let temp_store_dir = tempfile::Builder::new()
            .prefix("iroha-staging-kura-")
            .tempdir()
            .map_err(|error| Error::IO(error, std::env::temp_dir()))?;
        let mut temporary_config = config.clone();
        temporary_config.store_dir = WithOrigin::inline(temp_store_dir.path().to_path_buf());
        let (mut kura, _) = Self::new_with_configured_lane_catalog(
            &temporary_config,
            lane_config,
            configured_lane_catalog,
        )?;
        let store_root = temporary_config.store_dir.resolve_relative_path();
        let kura_inner = Arc::get_mut(&mut kura).ok_or_else(|| {
            Error::IO(
                std::io::Error::other(
                    "newly initialized temporary Kura unexpectedly acquired another owner",
                ),
                store_root,
            )
        })?;
        kura_inner._temp_store_dir = Some(temp_store_dir);
        Ok(kura)
    }
    fn acquire_store_root_lock(
        store_root: &Path,
        create_if_missing: bool,
    ) -> Result<std::fs::File> {
        let canonical_root = std::fs::canonicalize(store_root)
            .map_err(|error| Error::IO(error, store_root.to_path_buf()))?;
        let root_before = secure_file_metadata::from_path(&canonical_root)
            .map_err(|error| Error::IO(error, canonical_root.clone()))?;
        if root_before.file_type().is_symlink() || !root_before.is_dir() {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "canonical Kura store root is not a direct directory",
                ),
                canonical_root,
            ));
        }
        let lock_path = canonical_root.join(STORE_ROOT_LOCK_FILE_NAME);
        if let Some(metadata) = match secure_file_metadata::from_path(&lock_path) {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == ErrorKind::NotFound => None,
            Err(error) => return Err(Error::IO(error, lock_path)),
        } && (metadata.file_type().is_symlink()
            || !metadata.file_type().is_file()
            || !Self::sidecar_is_single_link(&metadata))
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "Kura store-root lock path is not a single-link regular file",
                ),
                lock_path,
            ));
        }
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true).create(create_if_missing);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options
                .mode(0o600)
                .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
        }
        let file = options
            .open(&lock_path)
            .map_err(|error| Error::IO(error, lock_path.clone()))?;
        let opened_metadata = secure_file_metadata::from_file(&file)
            .map_err(|error| Error::IO(error, lock_path.clone()))?;
        let path_metadata = secure_file_metadata::from_path(&lock_path)
            .map_err(|error| Error::IO(error, lock_path.clone()))?;
        if !opened_metadata.file_type().is_file()
            || !path_metadata.file_type().is_file()
            || path_metadata.file_type().is_symlink()
            || !Self::sidecar_is_single_link(&opened_metadata)
            || !Self::sidecar_is_single_link(&path_metadata)
            || !Self::sidecar_metadata_same_object(&opened_metadata, &path_metadata)
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "Kura store-root lock path changed while opening",
                ),
                lock_path,
            ));
        }
        match file.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => return Err(Error::Locked(lock_path)),
            Err(std::fs::TryLockError::Error(error)) => {
                return Err(Error::IO(error, lock_path));
            }
        }
        let root_after = secure_file_metadata::from_path(&canonical_root)
            .map_err(|error| Error::IO(error, canonical_root.clone()))?;
        let path_after = secure_file_metadata::from_path(&lock_path)
            .map_err(|error| Error::IO(error, lock_path.clone()))?;
        if root_after.file_type().is_symlink()
            || !root_after.is_dir()
            || !Self::sidecar_metadata_same_object(&root_before, &root_after)
            || path_after.file_type().is_symlink()
            || !path_after.is_file()
            || !Self::sidecar_file_metadata_unchanged(&path_metadata, &path_after)
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "Kura store root or lock path changed while acquiring the OS lock",
                ),
                lock_path,
            ));
        }
        Ok(file)
    }
}
impl Kura {
    fn new_inner(
        config: &Config,
        _lane_config: &LaneConfig,
        configured_catalog_hash: Option<Hash>,
        provisional_hash_only_prefix: Option<usize>,
        discover_signed_lineage_marker: bool,
        pending_control_sidecar_limits: PendingControlSidecarLimits,
    ) -> Result<(Arc<Self>, BlockCount)> {
        let init_started_at = Instant::now();
        let configured_store_dir = config.store_dir.resolve_relative_path();
        let history_bytes = usize::try_from(config.block_hash_history_bytes.get())
            .ok()
            .filter(|bytes| *bytes != 0)
            .ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidInput,
                        "kura.block_hash_history_bytes must be nonzero and representable as usize",
                    ),
                    configured_store_dir.clone(),
                )
            })?;
        let transaction_history_bytes = usize::try_from(config.transaction_history_bytes.get())
            .ok()
            .filter(|bytes| *bytes != 0)
            .ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidInput,
                        "kura.transaction_history_bytes must be nonzero and representable as usize",
                    ),
                    configured_store_dir.clone(),
                )
            })?;
        let membership_storage = membership_storage::MembershipStorage::new(
            config.membership_storage,
        )
        .map_err(|error| {
            Error::IO(
                std::io::Error::new(ErrorKind::InvalidInput, error),
                configured_store_dir.clone(),
            )
        })?;
        config.fastpq_artifacts.validate().map_err(|error| {
            Error::IO(
                std::io::Error::new(ErrorKind::InvalidInput, error.to_string()),
                configured_store_dir.clone(),
            )
        })?;
        if configured_store_dir.as_os_str().is_empty() {
            return Err(Error::EmptyStoreRoot);
        }
        let replica_registry_key_capacity = if config.init_mode == InitMode::Fast {
            NonZeroUsize::MIN
        } else {
            config
                .replica_advert
                .validate(config.blocks_in_memory)
                .map_err(|error| Error::InvalidKuraReplicaAdvertConfiguration(error.to_string()))?
        };
        let native_amx_evidence_prune_intent_max_bytes = if config.init_mode == InitMode::Fast {
            0
        } else {
            Self::native_amx_evidence_prune_intent_max_bytes_for_retention(
                config.lane_history_retention,
                pending_control_sidecar_limits.aggregate_bytes,
            )?
        };
        if config.init_mode == InitMode::Strict {
            create_dir_all_with_context(&configured_store_dir)?;
        }
        // Resolve aliases once, before taking the lock, and use the same stable
        // absolute root for every subsequent Kura path. Fast deliberately does
        // not create a missing root; it is valid only for Strict-initialized
        // storage.
        let store_dir = std::fs::canonicalize(&configured_store_dir)
            .map_err(|error| Error::IO(error, configured_store_dir))?;
        let store_root = store_dir.clone();
        let store_root_lock_file =
            Self::acquire_store_root_lock(&store_dir, config.init_mode == InitMode::Strict)?;
        #[cfg(all(unix, not(target_os = "espidf")))]
        let store_root_directory =
            Self::open_safety_wal_store_root_directory(&store_root, &store_root_lock_file)?;
        if config.init_mode == InitMode::Strict {
            Self::reject_retired_commit_roster_artifacts(&store_root)?;
        }
        let blocks_in_memory = if config.init_mode == InitMode::Fast {
            NonZeroUsize::new(
                config
                    .blocks_in_memory
                    .get()
                    .min(EMERGENCY_FAST_RECENT_BLOCK_CACHE_CAPACITY.get()),
            )
            .expect("the emergency Fast cache ceiling is non-zero")
        } else {
            config.blocks_in_memory
        };
        let lane_history_retention = if config.init_mode == InitMode::Fast {
            NonZeroUsize::MIN
        } else {
            config.lane_history_retention
        };
        let authenticated_configured_catalog = configured_catalog_hash.is_some();
        let mut provisional_open = provisional_hash_only_prefix.is_some();
        if provisional_open && config.init_mode == InitMode::Fast {
            return Err(Error::InvalidSnapshotBootstrapMarker {
                path: store_root,
                reason: "emergency Fast mode cannot authenticate or finalize an imported snapshot; restart in strict mode"
                    .to_owned(),
            });
        }
        if let Some(configured_catalog_hash) = configured_catalog_hash {
            if config.init_mode == InitMode::Fast {
                warn!(
                    "emergency Fast startup skipped configured-catalog and lane-geometry journal decoding"
                );
            } else if provisional_open {
                Self::verify_configured_lane_catalog_baseline_read_only(
                    &store_dir,
                    configured_catalog_hash,
                    &store_root_lock_file,
                )?;
            } else if discover_signed_lineage_marker {
                match Self::verify_configured_lane_catalog_baseline_read_only(
                    &store_dir,
                    configured_catalog_hash,
                    &store_root_lock_file,
                ) {
                    Ok(()) => {
                        let marker_path = Self::canonical_storage_paths(&store_dir)
                            .0
                            .join(VERIFIED_SNAPSHOT_TAIL_FILE_NAME);
                        provisional_open = match std::fs::symlink_metadata(&marker_path) {
                            Ok(_) => true,
                            Err(error) if error.kind() == ErrorKind::NotFound => false,
                            Err(error) => return Err(Error::IO(error, marker_path)),
                        };
                    }
                    Err(Error::IO(error, _)) if error.kind() == ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
            if !provisional_open && config.init_mode == InitMode::Strict {
                Self::establish_or_verify_configured_lane_catalog_baseline_with_lock(
                    &store_dir,
                    configured_catalog_hash,
                    &store_root_lock_file,
                )?;
                #[cfg(test)]
                Self::configured_catalog_preflight_crash_boundary(&store_dir)?;
            }
        }
        if config.init_mode == InitMode::Strict {
            Self::recover_autonomous_lifecycle_process_generation_atomic_temporary_on_startup(
                &store_root,
                &store_root_lock_file,
                authenticated_configured_catalog && !provisional_open,
            )?;
            Self::read_autonomous_lifecycle_process_generation_record_for(&store_root)?;
        } else {
            warn!(
                "emergency Fast startup deferred process-generation and autonomous bootstrap artifact audits until a Strict restart"
            );
        }
        // Canonical bodies do not depend on a current LaneId or lane namespace.
        // State installs the authenticated instance catalog after genesis/snapshot
        // authentication; this open guesses and provisions no lane path.
        let (blocks_root, merge_log_path) = Self::canonical_storage_paths(&store_dir);
        let mut canonical_preflight = (config.init_mode == InitMode::Strict)
            .then(|| Self::preflight_canonical_storage(&store_dir))
            .transpose()?;
        if let Some(preflight) = canonical_preflight.as_mut() {
            Self::reverify_canonical_storage_parents(preflight, true)?;
        }
        if blocks_root.as_os_str().is_empty() || merge_log_path.as_os_str().is_empty() {
            return Err(Error::EmptyStoreRoot);
        }
        if config.init_mode == InitMode::Strict {
            Self::reject_retired_pipeline_artifacts(&blocks_root)?;
            Self::reject_retired_rollback_intents(&blocks_root)?;
        }
        let mut canonical_replica_terminal_carrier_pins =
            if config.init_mode == InitMode::Strict && !provisional_open {
                Self::canonical_replica_terminal_carrier_pins_for_store(&store_root, &blocks_root)?
            } else {
                BTreeMap::new()
            };
        let merge_cache_capacity =
            sanitize_merge_cache_capacity(config.merge_ledger_cache_capacity);
        if let Some(preflight) = canonical_preflight.as_mut() {
            #[cfg(test)]
            {
                configured_primary_open_identity_swap_boundary(&store_dir)?;
                configured_primary_open_identity_swap_boundary(&blocks_root)?;
                configured_primary_open_identity_swap_boundary(&merge_log_path)?;
            }
            Self::reverify_canonical_blocks_open(preflight, &blocks_root, false)?;
            Self::reverify_canonical_merge_open(preflight, &merge_log_path, false)?;
        }
        let mut block_store =
            BlockStore::with_fsync(&blocks_root, config.fsync_mode, config.fsync_interval);
        if config.init_mode == InitMode::Strict && !provisional_open {
            // Both independent operation owners protect the exact selected canonical
            // body before strict initialization can repair or truncate its suffix.
            let native_pins =
                block_store.native_amx_publication_pins_before_storage_recovery(&store_root)?;
            for (height, hash) in native_pins {
                if canonical_replica_terminal_carrier_pins
                    .insert(height, hash)
                    .is_some_and(|old| old != hash)
                {
                    return Err(Error::PruneIntentConflict(
                        "Native publication and terminal recovery pin different canonical bodies"
                            .to_owned(),
                    ));
                }
            }
        }
        let mut provisional_snapshot_bootstrap = None;
        let durable_height_bound;
        let mut fast_preflight_height = None;
        if provisional_open {
            block_store.require_existing_journal_bound_canonical_files()?;
            let logical_count = block_store.read_index_count()?;
            let hashes_count = block_store.read_hashes_count()?;
            let durable_marker = block_store
                .validated_verified_snapshot_tail_read_only(logical_count, hashes_count)?;
            let prefix = if let Some(configured_prefix) = provisional_hash_only_prefix {
                configured_prefix
            } else {
                let marker = durable_marker.as_ref().ok_or_else(|| {
                    Error::InvalidSnapshotBootstrapMarker {
                        path: blocks_root.join(VERIFIED_SNAPSHOT_TAIL_FILE_NAME),
                        reason:
                            "signed-lineage provisional opening requires a durable snapshot marker"
                                .to_owned(),
                    }
                })?;
                if marker.body_prefix_count != marker.snapshot_height
                    || marker.bootstrap_lineage_hash.is_none()
                {
                    return Err(Error::InvalidSnapshotBootstrapMarker {
                        path: blocks_root.join(VERIFIED_SNAPSHOT_TAIL_FILE_NAME),
                        reason: "marker is not an imported-prefix lineage binding".to_owned(),
                    });
                }
                usize::try_from(marker.snapshot_height)?
            };
            durable_height_bound =
                block_store.initialize_provisional_snapshot_bootstrap_read_only(prefix)?;
            provisional_snapshot_bootstrap = Some(ProvisionalSnapshotBootstrap {
                hash_only_prefix_height: prefix,
                bootstrap_lineage_hash: durable_marker
                    .as_ref()
                    .and_then(|marker| marker.bootstrap_lineage_hash),
                hash_journal_digest: durable_marker
                    .as_ref()
                    .map(|marker| marker.hash_journal_digest),
            });
        } else {
            let snapshot_marker_path = blocks_root.join(VERIFIED_SNAPSHOT_TAIL_FILE_NAME);
            if std::fs::symlink_metadata(&snapshot_marker_path).is_ok() {
                return Err(Error::InvalidSnapshotBootstrapMarker {
                    path: snapshot_marker_path,
                    reason: "hash-only imported history must be opened provisionally and reauthenticated from a signed snapshot lineage"
                        .to_owned(),
                });
            }
            match config.init_mode {
                InitMode::Fast => {
                    if canonical_preflight
                        .as_ref()
                        .is_some_and(|preflight| preflight.requires_existing_files)
                    {
                        block_store.require_existing_journal_bound_canonical_files()?;
                    }
                    let height = block_store.preflight_fast_durable_prefix()?;
                    fast_preflight_height = Some(height);
                    durable_height_bound = height;
                }
                InitMode::Strict => {
                    block_store.recover_canonical_storage_stages_with_carrier_pins(
                        &canonical_replica_terminal_carrier_pins,
                    )?;
                    if canonical_preflight
                        .as_ref()
                        .is_some_and(|preflight| preflight.requires_existing_files)
                    {
                        block_store.require_existing_journal_bound_canonical_files()?;
                    }
                    durable_height_bound = block_store
                        .read_commit_marker()?
                        .map_or(0, |marker| marker.count);
                }
            }
        }
        let v2_finality_floor = if provisional_open || config.init_mode == InitMode::Strict {
            Self::highest_v2_finality_artifact_height_for(
                &store_root,
                &blocks_root,
                durable_height_bound,
            )?
        } else {
            None
        };
        if !provisional_open {
            if let Some(finalized_height) = v2_finality_floor {
                block_store.preflight_v2_finalized_prefix(finalized_height)?;
            }
            match config.init_mode {
                InitMode::Fast => block_store.open_fast_prevalidated_files_read_only()?,
                InitMode::Strict => block_store.create_files_if_they_do_not_exist()?,
            }
            if let Some(expected_height) = fast_preflight_height {
                let actual_height = block_store.read_exact_durable_index_count()?;
                if actual_height != expected_height {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "Kura fast init recovery changed the committed block boundary",
                        ),
                        blocks_root.clone(),
                    ));
                }
            }
        }
        if let Some(preflight) = canonical_preflight.as_mut() {
            Self::reverify_canonical_blocks_open(preflight, &blocks_root, true)?;
        }
        let prune_intent = if config.init_mode == InitMode::Fast {
            None
        } else {
            Self::read_prune_intent_for_startup(&store_root, provisional_open)?
        };
        if let Some(intent) = prune_intent.as_ref() {
            let configured_limit = config.max_disk_usage_bytes.get();
            if configured_limit > 0 && intent.capacity.admitted_peak_bytes > configured_limit {
                return Err(Error::StorageBudgetExceeded {
                    limit: configured_limit,
                    used: intent.capacity.source_physical_bytes,
                    required: intent.capacity.admitted_peak_bytes,
                });
            }
        }
        if let Some(preflight) = canonical_preflight.as_mut() {
            Self::reverify_canonical_merge_open(preflight, &merge_log_path, false)?;
        }
        let mut merge_log = if config.init_mode == InitMode::Fast {
            warn!(
                "emergency Fast startup deferred merge-ledger decoding, indexing, and tail repair until a Strict restart"
            );
            MergeLedgerLog::deferred(merge_cache_capacity)
        } else {
            MergeLedgerLog::startup(&merge_log_path, merge_cache_capacity, provisional_open)?
        };
        if let Some(preflight) = canonical_preflight.as_mut() {
            Self::reverify_canonical_merge_open(preflight, &merge_log_path, true)?;
        }
        if let Some(intent) = prune_intent.as_ref() {
            if let Some(finalized_height) = v2_finality_floor
                && finalized_height > intent.target_height
            {
                return Err(Error::FinalizedV2BlockMutation {
                    rewrite_from_height: intent.target_height.saturating_add(1),
                    finalized_height,
                });
            }
            Self::validate_prune_intent_merge_prefix(&mut merge_log, intent)?;
            Self::apply_prune_intent_to_block_store(&mut block_store, intent)?;
        }
        let (block_notify_tx, block_notify_rx) = mpsc::sync_channel(BLOCK_NOTIFY_CHANNEL_CAPACITY);
        let block_plain_text_path = config
            .debug_output_new_blocks
            .then(|| blocks_root.join("blocks.jsonl"));
        let mut chain_validation = Kura::init(
            &mut block_store,
            config.init_mode,
            v2_finality_floor,
            provisional_snapshot_bootstrap
                .as_ref()
                .map(|bootstrap| bootstrap.hash_only_prefix_height),
            &canonical_replica_terminal_carrier_pins,
        )?;
        if let Some(preflight) = canonical_preflight.as_mut() {
            Self::reverify_canonical_blocks_open(preflight, &blocks_root, true)?;
        }
        if !provisional_open && config.init_mode == InitMode::Strict {
            let manifest_reconciliation = Self::reconcile_commit_manifests(
                &mut block_store,
                &blocks_root,
                &mut chain_validation.hashes,
            )?;
            if manifest_reconciliation.manifests_present
                || manifest_reconciliation.pruned_manifests
                || manifest_reconciliation.pruned_checkpoints
            {
                if manifest_reconciliation.pruned_manifests {
                    warn!(
                        retained_height = manifest_reconciliation.retained_height,
                        "Kura pruned commit manifests outside the durable block log"
                    );
                }
                if manifest_reconciliation.pruned_checkpoints {
                    warn!(
                        retained_height = manifest_reconciliation.retained_height,
                        "Kura pruned WSV checkpoints outside the durable block log"
                    );
                }
            }
        }
        let block_count = usize::try_from(block_store.read_exact_durable_index_count()?)?;
        let block_data = if config.init_mode == InitMode::Fast && !provisional_open {
            BlockData::deferred(block_count)
        } else {
            std::mem::take(&mut chain_validation.hashes)
                .into_iter()
                .map(|hash| (hash, None))
                .collect()
        };
        let block_height_index = Self::build_block_height_index(&block_data);
        let transaction_entrypoint_index = Self::build_transaction_entrypoint_index(&block_data);
        let hard_fork_hash_only_block_count = chain_validation
            .hard_fork_hash_only_block_count
            .min(block_count);
        if hard_fork_hash_only_block_count > 0 {
            warn!(
                hard_fork_hash_only_block_count,
                block_count,
                "hard-fork snapshot bootstrap: treating pre-fork block bodies as unavailable"
            );
        }
        info!(
            mode = ?config.init_mode,
            block_count,
            "Kura block journal init complete"
        );
        if !provisional_open && let Some(preflight) = canonical_preflight.as_mut() {
            Self::reverify_canonical_merge_open(preflight, &merge_log_path, false)?;
        }
        if !provisional_open && let Some(preflight) = canonical_preflight.as_mut() {
            Self::reverify_canonical_merge_open(preflight, &merge_log_path, true)?;
        }
        let startup_lane_storage_entries = BTreeMap::new();
        if config.init_mode == InitMode::Strict
            && !provisional_open
            && merge_log.total_entries > block_count
        {
            let trimmed = merge_log.total_entries - block_count;
            if chain_validation.truncated {
                info!(
                    trimmed,
                    block_count, "Pruning merge-ledger entries to match truncated block store"
                );
            } else {
                warn!(
                    trimmed,
                    block_count, "Merge-ledger log longer than block store; truncating tail"
                );
            }
            merge_log.truncate_to_len(block_count)?;
        }
        let resource_inventory = Arc::new(resource_inventory::Inventory::default());
        let (sidecar_lock, sidecar_read_permit) = PublicationMutex::with_read_permit();
        let kura = Arc::new(Self {
            block_hash_history_budget: mv::allocation::AllocationBudget::new(history_bytes),
            transaction_history_budget: mv::allocation::AllocationBudget::new(
                transaction_history_bytes,
            ),
            membership_storage,
            resource_inventory: Arc::clone(&resource_inventory),
            instance_identity: Arc::new(KuraInstanceIdentityMarker),
            #[cfg(all(unix, not(target_os = "espidf")))]
            store_root_directory,
            _store_root_lock_file: Some(store_root_lock_file),
            block_store: Mutex::new(block_store),
            canonical_chain_lock: PublicationMutex::default(),
            block_store_write_lock: Mutex::new(()),
            prune_lock: PublicationMutex::default(),
            prune_in_progress: AtomicBool::new(false),
            prune_recovery_required: AtomicBool::new(false),
            block_data: ResidentMutex::new(block_data, &resource_inventory),
            auxiliary_history_deferred: config.init_mode == InitMode::Fast && !provisional_open,
            hard_fork_hash_only_block_count: AtomicUsize::new(hard_fork_hash_only_block_count),
            block_height_index: ResidentMutex::new(block_height_index, &resource_inventory),
            transaction_entrypoint_index: ResidentMutex::new(
                transaction_entrypoint_index,
                &resource_inventory,
            ),
            block_notify_tx,
            block_notify_rx: Mutex::new(Some(block_notify_rx)),
            block_plain_text_path: Mutex::new(block_plain_text_path),
            sidecar_lock,
            sidecar_read_permit,
            autonomous_lifecycle_process_generation_lock: Mutex::new(()),
            autonomous_lifecycle_process_generation_claim: OnceLock::new(),
            lane_drain_signing_guard: once_cell::sync::OnceCell::new(),
            historical_autonomous_recovery_mutation_lock: Mutex::new(()),
            v2_finality_verification_cache: ResidentMutex::new(
                VecDeque::new(),
                &resource_inventory,
            ),
            v2_startup_finality_verification_inventory: Mutex::new(None),
            v2_startup_replay_geometry_publication: Mutex::new(None),
            startup_inventory_resident: Arc::new(ResidentMutex::new(
                resident_inventory_lifetimes::VerificationAllocations::default(),
                &resource_inventory,
            )),
            merge_carrier_lock: Mutex::new(()),
            merge_carrier_index: ResidentMutex::new(
                MergeCarrierIndex::default(),
                &resource_inventory,
            ),
            pipeline_sidecar_queue: ResidentMutex::new(VecDeque::new(), &resource_inventory),
            pipeline_sidecar_queue_cap: AtomicUsize::new(blocks_in_memory.get()),
            fastpq_proof_queue: ResidentMutex::new(VecDeque::new(), &resource_inventory),
            fastpq_proof_sidecar_queue_cap: AtomicUsize::new(
                default_fastpq_proof_sidecar_queue_cap(),
            ),
            fastpq_proof_sidecar_max_bytes: AtomicUsize::new(
                default_fastpq_proof_sidecar_max_bytes(),
            ),
            fastpq_proof_sidecar_max_retries: AtomicUsize::new(
                default_fastpq_proof_sidecar_max_retries(),
            ),
            store_root,
            active_blocks_dir: Mutex::new(blocks_root.clone()),
            active_merge_path: Mutex::new(merge_log_path.clone()),
            lane_storage_entries: ResidentMutex::new(
                startup_lane_storage_entries,
                &resource_inventory,
            ),
            lane_storage_network: Mutex::new(None),
            committed_lane_status_revision: AtomicU64::new(0),
            latest_certified_frontier_storage_unknown: AtomicBool::new(
                config.init_mode == InitMode::Fast && !provisional_open,
            ),
            certified_pair_durability: ResidentMutex::new(BTreeMap::new(), &resource_inventory),
            lane_receipt_namespace_durability: ResidentMutex::new(
                LaneReceiptNamespaceDurability::default(),
                &resource_inventory,
            ),
            certified_frontier_artifact_validation: ResidentMutex::new(
                BTreeMap::new(),
                &resource_inventory,
            ),
            lane_geometry_lock: PublicationMutex::default(),
            raw_geometry_claim: lane_geometry::RawGeometryClaimGate::default(),
            max_disk_usage_bytes: if config.init_mode == InitMode::Fast {
                0
            } else {
                config.max_disk_usage_bytes.get()
            },
            eviction_required_replicas: config.replica_advert.eviction_required_replicas,
            local_peer_id: OnceLock::new(),
            replica_registry: ResidentMutex::new(NestedMap::default(), &resource_inventory),
            replica_registry_key_capacity,
            replica_advert_evictable_window: config.replica_advert.evictable_window,
            replica_advert_ttl: config.replica_advert.ttl,
            replica_advert_refresh_interval: config.replica_advert.refresh_interval,
            disk_usage: AtomicU64::new(0),
            disk_usage_total: AtomicU64::new(0),
            disk_usage_total_accounting: Mutex::new(TotalDiskUsageAccountingState::default()),
            disk_usage_total_accounting_changed: Condvar::new(),
            pending_budget_bytes: AtomicU64::new(0),
            pending_budget_bytes_valid: AtomicBool::new(false),
            post_wsv_lane_artifact_budget_reservations: ResidentMutex::new(
                NestedMap::default(),
                &resource_inventory,
            ),
            certified_bundle_capacity_reservations: ResidentMutex::new(
                NestedMap::default(),
                &resource_inventory,
            ),
            native_amx_publication_capacity_reservations: ResidentMutex::new(
                NestedMap::default(),
                &resource_inventory,
            ),
            native_amx_resident_recovery_complete: AtomicBool::new(false),
            post_wsv_resident_recovery_complete: AtomicBool::new(false),
            certified_resident_recovery_complete: AtomicBool::new(false),
            #[cfg(test)]
            pending_budget_raw_scans: AtomicUsize::new(0),
            pending_budget_eviction_bytes: AtomicU64::new(0),
            durable_budget_persisted_count: AtomicUsize::new(block_count),
            durable_budget_unindexed_bytes: AtomicU64::new(0),
            durable_budget_snapshot_valid: AtomicBool::new(true),
            disk_usage_initialized: AtomicBool::new(false),
            disk_usage_total_initialized: AtomicBool::new(false),
            disk_usage_total_last_refresh: AtomicU64::new(0),
            blocks_in_memory,
            lane_history_retention,
            fastpq_artifact_policy: config.fastpq_artifacts,
            native_context_archive_max_bytes: config.native_context_archive_max_bytes,
            pending_control_sidecar_limits,
            native_amx_evidence_prune_intent_max_bytes,
            merge_log: ResidentMutex::new(merge_log, &resource_inventory),
            telemetry: OnceLock::new(),
            writer_fault: Mutex::new(None),
            canonical_storage_poisoned: AtomicBool::new(false),
            provisional_snapshot_bootstrap: Mutex::new(provisional_snapshot_bootstrap.map_or(
                SnapshotBootstrapRuntimeState::Authenticated,
                SnapshotBootstrapRuntimeState::Pending,
            )),
            canonical_poison_binding_lock: Mutex::new(()),
            consensus_output_guard: OnceLock::new(),
            #[cfg(test)]
            pause_canonical_poison_after_latch: AtomicBool::new(false),
            #[cfg(test)]
            canonical_poison_paused_after_latch: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_block_write: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_atomic_write_after_temporary_sync: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_wsv_checkpoint_write: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_commit_manifest_write: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_pending_merge_cleanup: AtomicBool::new(false),
            #[cfg(test)]
            pause_store_after_pending_merge_stage: AtomicBool::new(false),
            #[cfg(test)]
            store_paused_after_pending_merge_stage: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_canonical_association_recovery: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_canonical_association_cleanup: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_lane_geometry_publication: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_lane_geometry_publication_after_write: AtomicBool::new(false),
            #[cfg(test)]
            fail_lane_geometry_gc_stage: AtomicUsize::new(0),
            #[cfg(test)]
            fail_prune_after_stage: AtomicUsize::new(0),
            #[cfg(test)]
            fail_prune_sidecar_promotion_stage: AtomicUsize::new(0),
            #[cfg(test)]
            pause_prune_before_intent: AtomicBool::new(false),
            #[cfg(test)]
            prune_paused_before_intent: AtomicBool::new(false),
            #[cfg(test)]
            observe_canonical_reads_after_prune_check: AtomicBool::new(false),
            #[cfg(test)]
            canonical_read_kinds_after_prune_check: AtomicUsize::new(0),
            #[cfg(test)]
            pause_geometry_reference_publication: AtomicBool::new(false),
            #[cfg(test)]
            geometry_reference_publication_paused: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_v2_finality_write: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_native_amx_prepublication: AtomicBool::new(false),
            #[cfg(test)]
            v2_finality_crypto_verifications: AtomicUsize::new(0),
            #[cfg(test)]
            startup_replay_historical_payload_reads: AtomicUsize::new(0),
            #[cfg(test)]
            historical_autonomous_recovery_inventory_scans: AtomicUsize::new(0),
            #[cfg(test)]
            pending_queue_plan_admission_inventory_scans: AtomicUsize::new(0),
            #[cfg(test)]
            pending_queue_plan_admission_exact_reads: AtomicUsize::new(0),
            #[cfg(test)]
            pending_queue_plan_admission_batch_validations: AtomicUsize::new(0),
            #[cfg(test)]
            fail_retained_rewrite_discard_after: AtomicUsize::new(usize::MAX),
            #[cfg(test)]
            fail_next_retained_rewrite_recovery: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_retired_tree_purge_after_one_removal: AtomicBool::new(false),
            #[cfg(test)]
            durable_budget_metadata_reads: AtomicUsize::new(0),
            #[cfg(test)]
            pause_eviction_after_snapshot: AtomicBool::new(false),
            #[cfg(test)]
            eviction_paused_after_snapshot: AtomicBool::new(false),
            #[cfg(test)]
            pause_eviction_before_stage_publication: AtomicBool::new(false),
            #[cfg(test)]
            eviction_paused_before_stage_publication: AtomicBool::new(false),
            #[cfg(test)]
            kura_replica_advert_body_reads: AtomicUsize::new(0),
            #[cfg(test)]
            pause_block_read_before_cache_recheck: AtomicBool::new(false),
            #[cfg(test)]
            block_read_paused_before_cache_recheck: AtomicBool::new(false),
            #[cfg(test)]
            force_durable_blocks_count_fallback: AtomicBool::new(false),
            #[cfg(test)]
            durable_blocks_count_fallback_reached: AtomicBool::new(false),
            #[cfg(test)]
            pause_hash_only_extension_before_store: AtomicBool::new(false),
            #[cfg(test)]
            hash_only_extension_paused_before_store: AtomicBool::new(false),
            #[cfg(test)]
            pause_total_disk_usage_scan_after_scan: AtomicBool::new(false),
            #[cfg(test)]
            total_disk_usage_scan_paused: AtomicBool::new(false),
            _temp_store_dir: None,
        });
        if let Some(intent) = prune_intent.as_ref() {
            kura.preflight_recovered_prune_capacity_before_mutation(intent)?;
        }
        if config.init_mode == InitMode::Strict {
            if !provisional_open {
                kura.recover_journal_owned_lane_instances_on_startup()?;
            }
            // Bootstrap recovery needs the exact retained instance journal; a
            // configured LaneId cannot identify an original publication target.
            kura.recover_autonomous_lifecycle_bootstrap_atomic_temporary_on_startup(
                authenticated_configured_catalog && !provisional_open,
            )?;
            kura.audit_retained_autonomous_lifecycle_cursor_generations()?;
        }
        if !provisional_open {
            if config.init_mode == InitMode::Strict {
                kura.rebuild_native_amx_publication_capacity_on_startup()?;
                kura.cleanup_autonomous_atomic_sidecar_temps_on_startup()?;
                kura.seal_completed_autonomous_lifecycle_replica_claims_on_startup()?;
                kura.recover_retained_block_rewrite_stage_on_startup(&blocks_root)?;
                kura.recover_lane_consensus_sidecar_pairs_on_startup()?;
                kura.recover_canonical_autonomous_lane_replica_pairs_on_startup()?;
                kura.reconcile_historical_autonomous_recovery_atomic_temps_on_startup()?;
                let verified_finality = kura.validate_v2_finality_inventory_on_startup(true)?;
                kura.install_v2_startup_finality_verification_inventory(verified_finality);
                kura.prune_retained_block_records_from(
                    &blocks_root,
                    u64::try_from(block_count)?.saturating_add(1),
                )?;
                kura.validate_retained_block_inventory_on_startup()?;
                kura.recover_canonical_association_stage_before_state_geometry()?;
                kura.reconcile_merge_carriers_from_durable_blocks(prune_intent.is_some(), true)?;
                if let Some(intent) = prune_intent.as_ref() {
                    kura.complete_recovered_prune_intent(intent)?;
                }
                kura.recover_lane_histories_on_startup()?;
                kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()?;
                kura.refresh_v2_startup_replay_auxiliary_binding()?;
            } else {
                warn!(
                    "Kura emergency Fast mode skipped canonical association, historical retained, lane, merge-carrier, reservation, and capacity recovery"
                );
            }
        }
        if config.init_mode == InitMode::Strict {
            kura.validate_and_publish_configured_kura_capacity_after_startup_recovery(
                !provisional_open,
            )?;
        } else {
            warn!(
                configured_limit = config.max_disk_usage_bytes.get(),
                "Kura emergency Fast mode skipped the full disk-usage inventory and suspended its local Kura capacity cap until a Strict restart"
            );
        }
        kura.validate_fastpq_artifact_inventory_on_startup()?;
        let verified_finality_count = kura
            .v2_startup_finality_verification_inventory
            .lock()
            .as_ref()
            .map_or(0, |inventory| inventory.entries.len());
        info!(
            mode = ?config.init_mode,
            block_count,
            verified_finality_count,
            init_ms = init_started_at.elapsed().as_millis(),
            "Kura init complete"
        );
        let _ = kura.reconcile_physical_resource_inventory();
        let _ = kura.reconcile_resident_resource_inventory();
        Ok((kura, BlockCount(block_count)))
    }
    /// Create an isolated Kura instance for tests.
    ///
    /// The instance keeps blocks in memory for normal test access, while any background writer
    /// activity is redirected into a per-instance temporary directory instead of the crate root.
    /// Its empty canonical data, index, hash, and count journals match production first-boot
    /// storage so startup-boundary tests cannot accidentally rely on a fileless test-only shape.
    pub fn blank_kura_for_testing() -> Arc<Kura> {
        Self::blank_kura_for_testing_with_lane_config_and_retention(
            &LaneConfig::default(),
            BLOCKS_IN_MEMORY,
        )
    }
    /// Create an isolated empty Kura that exercises emergency Fast startup validation.
    #[cfg(test)]
    pub(crate) fn blank_kura_for_testing_in_emergency_fast_mode() -> Arc<Kura> {
        let mut kura = Self::blank_kura_for_testing();
        Arc::get_mut(&mut kura)
            .expect("a fresh test Kura has one owner")
            .auxiliary_history_deferred = true;
        kura
    }
    /// Return the number of FASTPQ proof snapshots awaiting persistence in tests.
    #[cfg(test)]
    pub(crate) fn fastpq_proof_queue_len_for_testing(&self) -> usize {
        self.fastpq_proof_queue.lock().len()
    }
    fn blank_kura_for_testing_with_lane_config_and_retention(
        _lane_config: &LaneConfig,
        blocks_in_memory: NonZeroUsize,
    ) -> Arc<Kura> {
        let (block_notify_tx, block_notify_rx) = mpsc::sync_channel(BLOCK_NOTIFY_CHANNEL_CAPACITY);
        let temp_store_dir = tempfile::Builder::new()
            .prefix("iroha-blank-kura-")
            .tempdir()
            .expect("create temporary Kura directory for tests");
        // Keep the test constructor on the same canonical-root boundary as
        // `new_inner`.  On macOS `/var` resolves through `/private/var`; retaining
        // the spelling returned by `tempfile` makes subsequently canonicalized
        // lane-geometry paths appear to escape the Kura root.
        let store_root = std::fs::canonicalize(temp_store_dir.path())
            .expect("canonicalize temporary Kura directory for tests");
        let store_root_lock_file = Self::acquire_store_root_lock(&store_root, true)
            .expect("lock temporary Kura directory for tests");
        Self::establish_or_verify_configured_lane_catalog_baseline_with_lock(
            &store_root,
            LaneLifecycleParameterV1::catalog_hash(&LaneCatalog::default()),
            &store_root_lock_file,
        )
        .expect("authenticate default configured catalog before opening test storage");
        let (blocks_root, merge_log_path) = Self::canonical_storage_paths(&store_root);
        std::fs::create_dir_all(&blocks_root)
            .expect("create temporary Kura block directory for tests");
        let mut block_store =
            BlockStore::with_fsync(&blocks_root, FsyncMode::Batched, FSYNC_INTERVAL);
        block_store
            .create_files_if_they_do_not_exist()
            .expect("initialize empty canonical Kura journal for tests");
        let merge_log = MergeLedgerLog::open_at(&merge_log_path, MERGE_LEDGER_CACHE_CAPACITY)
            .expect("create temporary Kura merge ledger for tests");
        #[cfg(all(unix, not(target_os = "espidf")))]
        let store_root_directory = Self::open_bound_progress_directory(&store_root, &store_root)
            .expect("bind temporary Kura store-root directory");
        let native_amx_evidence_prune_intent_max_bytes =
            Self::native_amx_evidence_prune_intent_max_bytes_for_retention(
                LANE_HISTORY_RETENTION,
                PendingControlSidecarLimits::default().aggregate_bytes,
            )
            .expect("default Native AMX prune-intent bound is valid");
        let resource_inventory = Arc::new(resource_inventory::Inventory::default());
        let (sidecar_lock, sidecar_read_permit) = PublicationMutex::with_read_permit();
        Arc::new(Self {
            membership_storage: membership_storage::MembershipStorage::new(
                iroha_config::parameters::defaults::kura::MEMBERSHIP_STORAGE_POLICY,
            )
            .expect("default finite membership control fits its original pool"),
            block_hash_history_budget: mv::allocation::AllocationBudget::new(
                usize::try_from(
                    iroha_config::parameters::defaults::kura::BLOCK_HASH_HISTORY_BYTES.get(),
                )
                .expect("default history budget fits supported platforms"),
            ),
            transaction_history_budget: mv::allocation::AllocationBudget::new(
                usize::try_from(
                    iroha_config::parameters::defaults::kura::TRANSACTION_HISTORY_BYTES.get(),
                )
                .expect("default transaction history budget fits supported platforms"),
            ),
            resource_inventory: Arc::clone(&resource_inventory),
            instance_identity: Arc::new(KuraInstanceIdentityMarker),
            #[cfg(all(unix, not(target_os = "espidf")))]
            store_root_directory,
            _store_root_lock_file: Some(store_root_lock_file),
            block_store: Mutex::new(block_store),
            canonical_chain_lock: PublicationMutex::default(),
            block_store_write_lock: Mutex::new(()),
            prune_lock: PublicationMutex::default(),
            prune_in_progress: AtomicBool::new(false),
            prune_recovery_required: AtomicBool::new(false),
            block_data: ResidentMutex::new(BlockData::default(), &resource_inventory),
            auxiliary_history_deferred: false,
            hard_fork_hash_only_block_count: AtomicUsize::new(0),
            block_height_index: ResidentMutex::new(HashMap::new(), &resource_inventory),
            transaction_entrypoint_index: ResidentMutex::new(
                TransactionEntrypointIndex::complete_empty(),
                &resource_inventory,
            ),
            block_notify_tx,
            block_notify_rx: Mutex::new(Some(block_notify_rx)),
            block_plain_text_path: Mutex::new(None),
            sidecar_lock,
            sidecar_read_permit,
            autonomous_lifecycle_process_generation_lock: Mutex::new(()),
            autonomous_lifecycle_process_generation_claim: OnceLock::new(),
            lane_drain_signing_guard: once_cell::sync::OnceCell::new(),
            historical_autonomous_recovery_mutation_lock: Mutex::new(()),
            v2_finality_verification_cache: ResidentMutex::new(
                VecDeque::new(),
                &resource_inventory,
            ),
            v2_startup_finality_verification_inventory: Mutex::new(None),
            v2_startup_replay_geometry_publication: Mutex::new(None),
            startup_inventory_resident: Arc::new(ResidentMutex::new(
                resident_inventory_lifetimes::VerificationAllocations::default(),
                &resource_inventory,
            )),
            merge_carrier_lock: Mutex::new(()),
            merge_carrier_index: ResidentMutex::new(
                MergeCarrierIndex::default(),
                &resource_inventory,
            ),
            pipeline_sidecar_queue: ResidentMutex::new(VecDeque::new(), &resource_inventory),
            pipeline_sidecar_queue_cap: AtomicUsize::new(default_pipeline_sidecar_queue_cap()),
            fastpq_proof_queue: ResidentMutex::new(VecDeque::new(), &resource_inventory),
            fastpq_proof_sidecar_queue_cap: AtomicUsize::new(
                default_fastpq_proof_sidecar_queue_cap(),
            ),
            fastpq_proof_sidecar_max_bytes: AtomicUsize::new(
                default_fastpq_proof_sidecar_max_bytes(),
            ),
            fastpq_proof_sidecar_max_retries: AtomicUsize::new(
                default_fastpq_proof_sidecar_max_retries(),
            ),
            store_root,
            active_blocks_dir: Mutex::new(blocks_root),
            active_merge_path: Mutex::new(merge_log_path),
            lane_storage_entries: ResidentMutex::new(BTreeMap::new(), &resource_inventory),
            lane_storage_network: Mutex::new(None),
            committed_lane_status_revision: AtomicU64::new(0),
            latest_certified_frontier_storage_unknown: AtomicBool::new(false),
            certified_pair_durability: ResidentMutex::new(BTreeMap::new(), &resource_inventory),
            lane_receipt_namespace_durability: ResidentMutex::new(
                LaneReceiptNamespaceDurability::default(),
                &resource_inventory,
            ),
            certified_frontier_artifact_validation: ResidentMutex::new(
                BTreeMap::new(),
                &resource_inventory,
            ),
            lane_geometry_lock: PublicationMutex::default(),
            raw_geometry_claim: lane_geometry::RawGeometryClaimGate::default(),
            max_disk_usage_bytes: MAX_DISK_USAGE_BYTES.get(),
            eviction_required_replicas: EVICTION_REQUIRED_REPLICAS,
            local_peer_id: OnceLock::new(),
            replica_registry: ResidentMutex::new(NestedMap::default(), &resource_inventory),
            replica_registry_key_capacity: kura_replica_advert_registry_key_capacity(
                blocks_in_memory,
                REPLICA_ADVERT_EVICTABLE_WINDOW,
            )
            .expect("default replica-advert registry geometry is representable"),
            replica_advert_evictable_window: REPLICA_ADVERT_EVICTABLE_WINDOW,
            replica_advert_ttl: REPLICA_ADVERT_TTL,
            replica_advert_refresh_interval: REPLICA_ADVERT_REFRESH_INTERVAL,
            disk_usage: AtomicU64::new(0),
            disk_usage_total: AtomicU64::new(0),
            disk_usage_total_accounting: Mutex::new(TotalDiskUsageAccountingState::default()),
            disk_usage_total_accounting_changed: Condvar::new(),
            pending_budget_bytes: AtomicU64::new(0),
            pending_budget_bytes_valid: AtomicBool::new(false),
            post_wsv_lane_artifact_budget_reservations: ResidentMutex::new(
                NestedMap::default(),
                &resource_inventory,
            ),
            certified_bundle_capacity_reservations: ResidentMutex::new(
                NestedMap::default(),
                &resource_inventory,
            ),
            native_amx_publication_capacity_reservations: ResidentMutex::new(
                NestedMap::default(),
                &resource_inventory,
            ),
            native_amx_resident_recovery_complete: AtomicBool::new(true),
            post_wsv_resident_recovery_complete: AtomicBool::new(true),
            certified_resident_recovery_complete: AtomicBool::new(true),
            #[cfg(test)]
            pending_budget_raw_scans: AtomicUsize::new(0),
            pending_budget_eviction_bytes: AtomicU64::new(0),
            durable_budget_persisted_count: AtomicUsize::new(0),
            durable_budget_unindexed_bytes: AtomicU64::new(0),
            durable_budget_snapshot_valid: AtomicBool::new(true),
            disk_usage_initialized: AtomicBool::new(true),
            disk_usage_total_initialized: AtomicBool::new(true),
            disk_usage_total_last_refresh: AtomicU64::new(0),
            blocks_in_memory,
            lane_history_retention: LANE_HISTORY_RETENTION,
            fastpq_artifact_policy:
                iroha_config::parameters::defaults::kura::FASTPQ_ARTIFACT_POLICY,
            native_context_archive_max_bytes:
                iroha_config::parameters::defaults::kura::NATIVE_CONTEXT_ARCHIVE_MAX_BYTES,
            pending_control_sidecar_limits: PendingControlSidecarLimits::default(),
            native_amx_evidence_prune_intent_max_bytes,
            merge_log: ResidentMutex::new(merge_log, &resource_inventory),
            telemetry: OnceLock::new(),
            writer_fault: Mutex::new(None),
            canonical_storage_poisoned: AtomicBool::new(false),
            provisional_snapshot_bootstrap: Mutex::new(
                SnapshotBootstrapRuntimeState::Authenticated,
            ),
            canonical_poison_binding_lock: Mutex::new(()),
            consensus_output_guard: OnceLock::new(),
            #[cfg(test)]
            pause_canonical_poison_after_latch: AtomicBool::new(false),
            #[cfg(test)]
            canonical_poison_paused_after_latch: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_block_write: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_atomic_write_after_temporary_sync: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_wsv_checkpoint_write: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_commit_manifest_write: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_pending_merge_cleanup: AtomicBool::new(false),
            #[cfg(test)]
            pause_store_after_pending_merge_stage: AtomicBool::new(false),
            #[cfg(test)]
            store_paused_after_pending_merge_stage: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_canonical_association_recovery: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_canonical_association_cleanup: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_lane_geometry_publication: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_lane_geometry_publication_after_write: AtomicBool::new(false),
            #[cfg(test)]
            fail_lane_geometry_gc_stage: AtomicUsize::new(0),
            #[cfg(test)]
            fail_prune_after_stage: AtomicUsize::new(0),
            #[cfg(test)]
            fail_prune_sidecar_promotion_stage: AtomicUsize::new(0),
            #[cfg(test)]
            pause_prune_before_intent: AtomicBool::new(false),
            #[cfg(test)]
            prune_paused_before_intent: AtomicBool::new(false),
            #[cfg(test)]
            observe_canonical_reads_after_prune_check: AtomicBool::new(false),
            #[cfg(test)]
            canonical_read_kinds_after_prune_check: AtomicUsize::new(0),
            #[cfg(test)]
            pause_geometry_reference_publication: AtomicBool::new(false),
            #[cfg(test)]
            geometry_reference_publication_paused: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_v2_finality_write: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_native_amx_prepublication: AtomicBool::new(false),
            #[cfg(test)]
            v2_finality_crypto_verifications: AtomicUsize::new(0),
            #[cfg(test)]
            startup_replay_historical_payload_reads: AtomicUsize::new(0),
            #[cfg(test)]
            historical_autonomous_recovery_inventory_scans: AtomicUsize::new(0),
            #[cfg(test)]
            pending_queue_plan_admission_inventory_scans: AtomicUsize::new(0),
            #[cfg(test)]
            pending_queue_plan_admission_exact_reads: AtomicUsize::new(0),
            #[cfg(test)]
            pending_queue_plan_admission_batch_validations: AtomicUsize::new(0),
            #[cfg(test)]
            #[cfg(test)]
            fail_retained_rewrite_discard_after: AtomicUsize::new(usize::MAX),
            #[cfg(test)]
            fail_next_retained_rewrite_recovery: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_retired_tree_purge_after_one_removal: AtomicBool::new(false),
            #[cfg(test)]
            durable_budget_metadata_reads: AtomicUsize::new(0),
            #[cfg(test)]
            pause_eviction_after_snapshot: AtomicBool::new(false),
            #[cfg(test)]
            eviction_paused_after_snapshot: AtomicBool::new(false),
            #[cfg(test)]
            pause_eviction_before_stage_publication: AtomicBool::new(false),
            #[cfg(test)]
            eviction_paused_before_stage_publication: AtomicBool::new(false),
            #[cfg(test)]
            kura_replica_advert_body_reads: AtomicUsize::new(0),
            #[cfg(test)]
            pause_block_read_before_cache_recheck: AtomicBool::new(false),
            #[cfg(test)]
            block_read_paused_before_cache_recheck: AtomicBool::new(false),
            #[cfg(test)]
            force_durable_blocks_count_fallback: AtomicBool::new(false),
            #[cfg(test)]
            durable_blocks_count_fallback_reached: AtomicBool::new(false),
            #[cfg(test)]
            pause_hash_only_extension_before_store: AtomicBool::new(false),
            #[cfg(test)]
            hash_only_extension_paused_before_store: AtomicBool::new(false),
            #[cfg(test)]
            pause_total_disk_usage_scan_after_scan: AtomicBool::new(false),
            #[cfg(test)]
            total_disk_usage_scan_paused: AtomicBool::new(false),
            _temp_store_dir: Some(temp_store_dir),
        })
    }
    /// Attach a telemetry sink for storage and durable finality reporting.
    pub fn attach_telemetry(&self, telemetry: StateTelemetry) {
        let _ = self.telemetry.set(telemetry);
        self.hydrate_v2_finality_telemetry_from_startup_inventory();
    }
    /// Configure FASTPQ proof sidecar persistence limits from runtime configuration.
    pub fn configure_fastpq_proof_sidecar_limits(&self, config: &FastpqConfig) {
        let max_bytes = usize::try_from(config.proof_sidecar_max_bytes.get())
            .unwrap_or(usize::MAX)
            .max(1);
        self.set_fastpq_proof_sidecar_limits(
            config.proof_sidecar_queue_cap.get(),
            max_bytes,
            config.proof_sidecar_max_retries.get(),
        );
    }
    fn set_fastpq_proof_sidecar_limits(
        &self,
        queue_cap: usize,
        max_bytes: usize,
        max_retries: usize,
    ) {
        self.fastpq_proof_sidecar_queue_cap
            .store(queue_cap.max(1), Ordering::Relaxed);
        self.fastpq_proof_sidecar_max_bytes
            .store(max_bytes.max(1), Ordering::Relaxed);
        self.fastpq_proof_sidecar_max_retries
            .store(max_retries.max(1), Ordering::Relaxed);
    }
    /// Record that a FASTPQ proof could not be persisted because no entry hash was available.
    pub fn record_fastpq_missing_entry_hash(&self) {
        let _ = self;
        let telemetry = FastpqProofSidecarTelemetry;
        telemetry.record_event("missing_entry_hash");
    }
    #[cfg(test)]
    fn set_fastpq_proof_sidecar_limits_for_testing(
        &self,
        queue_cap: usize,
        max_bytes: usize,
        max_retries: usize,
    ) {
        self.set_fastpq_proof_sidecar_limits(queue_cap, max_bytes, max_retries);
    }
    #[cfg(test)]
    fn set_pipeline_sidecar_queue_cap_for_testing(&self, queue_cap: usize) {
        self.pipeline_sidecar_queue_cap
            .store(queue_cap.max(1), Ordering::Relaxed);
    }
    /// Return the number of pipeline recovery sidecars pending in the test-only writer queue.
    #[cfg(test)]
    pub(crate) fn pipeline_sidecar_queue_len_for_testing(&self) -> usize {
        self.pipeline_sidecar_queue.lock().len()
    }
    /// Retain this Kura's original opened directory for native context publication.
    pub(crate) fn native_context_archive_root(&self) -> std::io::Result<std::fs::File> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            if !self.instance_identity().matches(self)
                || !self.bound_storage_directory_unchanged(&self.store_root_directory)
            {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "opened Kura root changed before native context archive binding",
                ));
            }
            self.store_root_directory.file.try_clone()
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "original Kura directory custody is required for native context publication",
            ))
        }
    }
    /// Original configured limit for each canonical native context projection.
    #[must_use]
    pub const fn native_context_archive_max_bytes(&self) -> NonZeroUsize {
        self.native_context_archive_max_bytes
    }
    /// Root directory used by this Kura instance.
    #[must_use]
    pub fn store_root(&self) -> PathBuf {
        self.store_root.clone()
    }
    /// Return cached total on-disk bytes used by Kura (active + retired segments).
    ///
    /// Local body caches and immutable retained-block/finality evidence are tracked in the total
    /// usage counter. The enforced Kura budget deliberately follows canonical/evictable storage:
    /// rejecting safety evidence could prevent finality or prevent the eviction needed to satisfy
    /// that same budget. Use [`Self::refresh_disk_usage_bytes`] to resync both counters.
    pub(crate) fn disk_usage_bytes(&self) -> Result<u64> {
        if self.emergency_fast_startup_enabled()
            && !self.disk_usage_total_initialized.load(Ordering::Acquire)
        {
            return Err(self.emergency_fast_disk_usage_unavailable());
        }
        loop {
            let generation = {
                let mut accounting = self.disk_usage_total_accounting.lock();
                while accounting.mutations_in_flight != 0 {
                    self.disk_usage_total_accounting_changed
                        .wait(&mut accounting);
                }
                accounting.generation
            };
            self.maybe_refresh_total_disk_usage_bytes()?;
            let accounting = self.disk_usage_total_accounting.lock();
            if accounting.mutations_in_flight != 0 || accounting.generation != generation {
                drop(accounting);
                continue;
            }
            if !self.disk_usage_total_initialized.load(Ordering::Acquire) {
                drop(accounting);
                continue;
            }
            return Ok(self.disk_usage_total.load(Ordering::Relaxed));
        }
    }
    /// Recompute enforced and total on-disk bytes in one scan and refresh both caches.
    pub(crate) fn refresh_disk_usage_bytes(&self) -> Result<u64> {
        if self.emergency_fast_startup_enabled() {
            return Err(self.emergency_fast_disk_usage_unavailable());
        }
        loop {
            let generation = {
                let mut accounting = self.disk_usage_total_accounting.lock();
                while accounting.mutations_in_flight != 0 {
                    self.disk_usage_total_accounting_changed
                        .wait(&mut accounting);
                }
                accounting.generation
            };
            let scanned = self.kura_disk_usage_bytes_with_total();
            let durable_budget = self.persisted_count_and_unindexed_bytes_raw();
            #[cfg(test)]
            self.maybe_pause_total_disk_usage_scan_after_scan_for_tests();
            let accounting = self.disk_usage_total_accounting.lock();
            if accounting.mutations_in_flight != 0 || accounting.generation != generation {
                drop(accounting);
                continue;
            }
            let (usage, total) = match scanned {
                Ok(usage) => usage,
                Err(error) => {
                    self.disk_usage_initialized.store(false, Ordering::Relaxed);
                    self.disk_usage_total_initialized
                        .store(false, Ordering::Relaxed);
                    self.invalidate_durable_budget_snapshot();
                    return Err(error);
                }
            };
            self.disk_usage.store(usage, Ordering::Relaxed);
            self.disk_usage_initialized.store(true, Ordering::Relaxed);
            match durable_budget {
                Ok((persisted_count, unindexed_bytes)) => {
                    self.publish_durable_budget_snapshot(persisted_count, unindexed_bytes);
                }
                Err(err) => {
                    warn!(?err, "failed to refresh Kura durable budget metadata");
                    self.invalidate_durable_budget_snapshot();
                }
            }
            self.disk_usage_total.store(total, Ordering::Relaxed);
            self.disk_usage_total_initialized
                .store(true, Ordering::Relaxed);
            self.disk_usage_total_last_refresh
                .store(Self::now_unix_secs(), Ordering::Relaxed);
            return Ok(usage);
        }
    }
    /// Recompute total on-disk bytes and refresh the cached value.
    pub(crate) fn refresh_total_disk_usage_bytes(&self) -> Result<u64> {
        if self.emergency_fast_startup_enabled() {
            return Err(self.emergency_fast_disk_usage_unavailable());
        }
        loop {
            let generation = {
                let mut accounting = self.disk_usage_total_accounting.lock();
                while accounting.mutations_in_flight != 0 {
                    self.disk_usage_total_accounting_changed
                        .wait(&mut accounting);
                }
                accounting.generation
            };
            let scanned = self.kura_total_disk_usage_bytes();
            #[cfg(test)]
            self.maybe_pause_total_disk_usage_scan_after_scan_for_tests();
            let accounting = self.disk_usage_total_accounting.lock();
            if accounting.mutations_in_flight != 0 || accounting.generation != generation {
                drop(accounting);
                continue;
            }
            let usage = match scanned {
                Ok(usage) => usage,
                Err(error) => {
                    self.disk_usage_total_initialized
                        .store(false, Ordering::Release);
                    return Err(error);
                }
            };
            self.disk_usage_total.store(usage, Ordering::Relaxed);
            self.disk_usage_total_initialized
                .store(true, Ordering::Relaxed);
            self.disk_usage_total_last_refresh
                .store(Self::now_unix_secs(), Ordering::Relaxed);
            return Ok(usage);
        }
    }
    /// Register a filesystem mutation before its first write, rename, or removal.
    pub(crate) fn begin_total_disk_usage_mutation(&self) -> TotalDiskUsageMutation<'_> {
        let mut accounting = self.disk_usage_total_accounting.lock();
        accounting.mutations_in_flight = accounting
            .mutations_in_flight
            .checked_add(1)
            .expect("total disk-usage mutation count must not overflow");
        accounting.generation = accounting.generation.wrapping_add(1);
        drop(accounting);
        TotalDiskUsageMutation {
            kura: self,
            published: false,
            physical_resources: self.begin_physical_resource_mutation(),
            physical_scope_classified: false,
            physical_children_remaining: None,
        }
    }
    fn finish_total_disk_usage_mutation(&self, published: bool) {
        let mut accounting = self.disk_usage_total_accounting.lock();
        if !published {
            self.disk_usage_initialized.store(false, Ordering::Relaxed);
            self.disk_usage_total_initialized
                .store(false, Ordering::Relaxed);
            self.invalidate_durable_budget_snapshot();
        }
        accounting.mutations_in_flight = accounting
            .mutations_in_flight
            .checked_sub(1)
            .expect("total disk-usage mutation guard must be balanced");
        accounting.generation = accounting.generation.wrapping_add(1);
        if accounting.mutations_in_flight == 0 {
            self.disk_usage_total_accounting_changed.notify_all();
        }
    }
    #[cfg(test)]
    fn maybe_pause_total_disk_usage_scan_after_scan_for_tests(&self) {
        if self
            .pause_total_disk_usage_scan_after_scan
            .swap(false, Ordering::AcqRel)
        {
            self.total_disk_usage_scan_paused
                .store(true, Ordering::Release);
            while self.total_disk_usage_scan_paused.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
        }
    }
    fn maybe_refresh_total_disk_usage_bytes(&self) -> Result<()> {
        if self.emergency_fast_startup_enabled() {
            return Err(self.emergency_fast_disk_usage_unavailable());
        }
        if !self.disk_usage_total_initialized.load(Ordering::Relaxed) {
            let _ = self.refresh_total_disk_usage_bytes()?;
            return Ok(());
        }
        let last_refresh = self.disk_usage_total_last_refresh.load(Ordering::Relaxed);
        let now = Self::now_unix_secs();
        if now.saturating_sub(last_refresh) >= DISK_USAGE_TOTAL_REFRESH_INTERVAL.as_secs() {
            let _ = self.refresh_total_disk_usage_bytes()?;
        }
        Ok(())
    }
    fn ensure_disk_usage_initialized(&self) -> Result<()> {
        if self.emergency_fast_startup_enabled() {
            return Err(self.emergency_fast_disk_usage_unavailable());
        }
        if self.disk_usage_initialized.load(Ordering::Relaxed) {
            return Ok(());
        }
        let _ = self.refresh_disk_usage_bytes()?;
        Ok(())
    }
    fn emergency_fast_disk_usage_unavailable(&self) -> Error {
        Error::EmergencyFastAuxiliaryUnavailable {
            subsystem: "disk-usage inventory",
        }
    }
    /// Update the cached disk usage by applying a before/after delta.
    pub(crate) fn update_disk_usage_delta(&self, before: u64, after: u64) {
        if before == after {
            return;
        }
        if after > before {
            self.add_disk_usage_bytes(after - before);
        } else {
            self.sub_disk_usage_bytes(before - after);
        }
    }
    fn update_total_disk_usage_delta(&self, before: u64, after: u64) {
        if before == after {
            return;
        }
        if after > before {
            self.add_total_disk_usage_bytes(after - before);
        } else {
            self.sub_total_disk_usage_bytes(before - after);
        }
    }
    fn add_disk_usage_bytes(&self, delta: u64) {
        if delta == 0 {
            return;
        }
        let _accounting_guard = self.disk_usage_total_accounting.lock();
        self.add_disk_usage_bytes_locked(delta);
    }
    fn add_disk_usage_bytes_locked(&self, delta: u64) {
        let _ = self
            .disk_usage
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                Some(current.saturating_add(delta))
            });
        self.add_total_disk_usage_bytes_locked(delta);
    }
    fn sub_disk_usage_bytes(&self, delta: u64) {
        if delta == 0 {
            return;
        }
        let _accounting_guard = self.disk_usage_total_accounting.lock();
        self.sub_disk_usage_bytes_locked(delta);
    }
    fn sub_disk_usage_bytes_locked(&self, delta: u64) {
        let _ = self
            .disk_usage
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                Some(current.saturating_sub(delta))
            });
        self.sub_total_disk_usage_bytes_locked(delta);
    }
    fn add_total_disk_usage_bytes(&self, delta: u64) {
        let _accounting_guard = self.disk_usage_total_accounting.lock();
        self.add_total_disk_usage_bytes_locked(delta);
    }
    fn add_total_disk_usage_bytes_locked(&self, delta: u64) {
        if delta == 0 || !self.disk_usage_total_initialized.load(Ordering::Relaxed) {
            return;
        }
        let _ =
            self.disk_usage_total
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                    Some(current.saturating_add(delta))
                });
    }
    fn sub_total_disk_usage_bytes(&self, delta: u64) {
        let _accounting_guard = self.disk_usage_total_accounting.lock();
        self.sub_total_disk_usage_bytes_locked(delta);
    }
    fn sub_total_disk_usage_bytes_locked(&self, delta: u64) {
        if delta == 0 || !self.disk_usage_total_initialized.load(Ordering::Relaxed) {
            return;
        }
        let _ =
            self.disk_usage_total
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                    Some(current.saturating_sub(delta))
                });
    }
    fn invalidate_pending_budget_cache(&self) {
        self.pending_budget_bytes_valid
            .store(false, Ordering::Relaxed);
    }
    fn durable_budget_snapshot(&self) -> Option<(usize, u64)> {
        if !self.durable_budget_snapshot_valid.load(Ordering::Acquire) {
            return None;
        }
        Some((
            self.durable_budget_persisted_count.load(Ordering::Relaxed),
            self.durable_budget_unindexed_bytes.load(Ordering::Relaxed),
        ))
    }
    fn publish_durable_budget_snapshot(&self, persisted_count: usize, unindexed_bytes: u64) {
        self.durable_budget_persisted_count
            .store(persisted_count, Ordering::Relaxed);
        self.durable_budget_unindexed_bytes
            .store(unindexed_bytes, Ordering::Relaxed);
        self.durable_budget_snapshot_valid
            .store(true, Ordering::Release);
    }
    fn invalidate_durable_budget_snapshot(&self) {
        self.durable_budget_snapshot_valid
            .store(false, Ordering::Release);
    }
    fn lock_block_store_for_write(&self) -> parking_lot::MutexGuard<'_, ()> {
        self.block_store_write_lock.lock()
    }
    #[cfg(test)]
    fn maybe_pause_block_read_before_cache_recheck_for_tests(&self) {
        if self
            .pause_block_read_before_cache_recheck
            .swap(false, Ordering::AcqRel)
        {
            self.block_read_paused_before_cache_recheck
                .store(true, Ordering::Release);
            while self
                .block_read_paused_before_cache_recheck
                .load(Ordering::Acquire)
            {
                std::thread::yield_now();
            }
        }
    }
    #[cfg(test)]
    fn maybe_pause_hash_only_extension_before_store_for_tests(&self) {
        if self
            .pause_hash_only_extension_before_store
            .swap(false, Ordering::AcqRel)
        {
            self.hash_only_extension_paused_before_store
                .store(true, Ordering::Release);
            while self
                .hash_only_extension_paused_before_store
                .load(Ordering::Acquire)
            {
                std::thread::yield_now();
            }
        }
    }
    #[cfg(test)]
    fn maybe_pause_canonical_poison_after_latch_for_tests(&self) {
        if self
            .pause_canonical_poison_after_latch
            .swap(false, Ordering::AcqRel)
        {
            self.canonical_poison_paused_after_latch
                .store(true, Ordering::Release);
            while self
                .canonical_poison_paused_after_latch
                .load(Ordering::Acquire)
            {
                std::thread::yield_now();
            }
        }
    }
    fn record_writer_fault(&self, context: &'static str, error: &Error) {
        {
            let mut fault = self.writer_fault.lock();
            if fault.is_none() {
                *fault = Some(format!("{context}: {error}"));
            }
        }
        if let Some(telemetry) = self.telemetry.get() {
            telemetry.inc_storage_budget_exceeded("kura_writer_fault");
        }
    }
    fn poison_canonical_storage(&self, context: &'static str, error: &Error) {
        // Publish the process-local fail-stop latch before looking for consensus.
        // The handshake lock then serializes this lookup with guard publication:
        // either poison nonblockingly closes admission on an already-bound
        // guard, or a later bind observes the latch and closes admission on the
        // newly published guard before returning. In-flight output is drained
        // by the consensus guard's ordinary fail-stop finalization paths.
        self.canonical_storage_poisoned
            .store(true, Ordering::Release);
        #[cfg(test)]
        self.maybe_pause_canonical_poison_after_latch_for_tests();
        let binding_guard = self.canonical_poison_binding_lock.lock();
        if let Some(output_guard) = self.consensus_output_guard.get() {
            output_guard.close_admission_for_restart();
        }
        drop(binding_guard);
        self.record_writer_fault(context, error);
        // Invalidate after the existing fail-stop sequence: resource accounting
        // must never delay publishing the latch or closing consensus admission.
        // The generation also rejects a reconciliation captured before poison.
        self.resource_inventory.invalidate(
            physical_resource_mask(),
            resource_inventory::Unavailable::InvalidInventory,
        );
    }
    fn committed_recovery_failure(&self, context: &'static str, error: &Error) -> Error {
        let recovery_error = Error::CanonicalBlockCommittedRecoveryRequired {
            detail: format!("{context}: {error}"),
        };
        self.poison_canonical_storage(context, &recovery_error);
        recovery_error
    }
    fn record_or_poison_fsync_fault(&self, context: &'static str, error: &Error) {
        if self.store_root.as_os_str().is_empty() {
            self.record_writer_fault(context, error);
            return;
        }
        let unresolved_stage = {
            let store = self.block_store.lock();
            [
                store.da_block_rewrite_stage_path(),
                store.eviction_compaction_stage_path(),
            ]
            .into_iter()
            .any(|path| match std::fs::symlink_metadata(path) {
                Ok(_) => true,
                Err(error) => error.kind() != ErrorKind::NotFound,
            })
        };
        if unresolved_stage
            || matches!(
                error,
                Error::DaBlockRewriteCommitStateUnknown { .. } | Error::CanonicalStoragePoisoned
            )
        {
            self.poison_canonical_storage(context, error);
        } else {
            self.record_writer_fault(context, error);
        }
    }
    fn ensure_canonical_storage_not_poisoned(&self) -> Result<()> {
        if self.canonical_storage_poisoned.load(Ordering::Acquire) {
            return Err(Error::CanonicalStoragePoisoned);
        }
        Ok(())
    }
    fn resolve_canonical_storage_before_mutation(&self) -> Result<()> {
        // The canonical fence also orders raw geometry claim acquisition.
        // Its original request must not observe a different carrier frontier
        // while State retains the operation between physical leases.
        self.raw_geometry_claim.ensure_unclaimed()?;
        self.durable_mutation_authorized()?;
        if self.store_root.as_os_str().is_empty() {
            return Ok(());
        }
        let write_guard = self.block_store_write_lock.lock();
        let mut store = self.block_store.lock();
        let resources = self
            .begin_canonical_physical_mutation(&mut store, CanonicalPhysicalOperation::Recovery);
        if let Err(error) = store.recover_canonical_storage_stages() {
            drop(store);
            self.poison_canonical_storage("unresolved canonical storage stage", &error);
            return Err(Error::CanonicalStoragePoisoned);
        }
        let deferred_da_recovery_fault = store.take_deferred_da_recovery_fault();
        let da_rewrite_stage_path = store.da_block_rewrite_stage_path();
        if deferred_da_recovery_fault.is_none() {
            resources.finish_resources_before_disk_rescan();
        }
        drop(store);
        drop(write_guard);
        if let Some(message) = deferred_da_recovery_fault {
            let recovered = Error::IO(std::io::Error::other(message), da_rewrite_stage_path);
            return Err(
                self.committed_recovery_failure("recovered committed DA block rewrite", &recovered)
            );
        }
        // A stale association stage may describe an append whose marker never committed. Its
        // cleanup is an exact-retry precondition, not evidence that this invocation crossed a
        // canonical commit point.
        self.recover_canonical_association_stage()
    }
    /// Number of newest canonical block bodies protected from Kura eviction.
    #[must_use]
    pub(crate) fn blocks_in_memory(&self) -> NonZeroUsize {
        self.blocks_in_memory
    }
    #[cfg(unix)]
    fn sidecar_metadata_same_object(left: &SecureMetadata, right: &SecureMetadata) -> bool {
        use std::os::unix::fs::MetadataExt as _;
        left.dev() == right.dev() && left.ino() == right.ino()
    }
    #[cfg(windows)]
    fn sidecar_metadata_same_object(left: &SecureMetadata, right: &SecureMetadata) -> bool {
        left.volume_serial_number() == right.volume_serial_number()
            && left.file_index() == right.file_index()
            && left.volume_serial_number().is_some()
            && left.file_index().is_some()
    }
    #[cfg(all(not(unix), not(windows)))]
    fn sidecar_metadata_same_object(_left: &SecureMetadata, _right: &SecureMetadata) -> bool {
        false
    }
    #[cfg(unix)]
    fn sidecar_file_metadata_unchanged(left: &SecureMetadata, right: &SecureMetadata) -> bool {
        use std::os::unix::fs::MetadataExt as _;
        Self::sidecar_metadata_same_object(left, right)
            && left.nlink() == 1
            && right.nlink() == 1
            && left.len() == right.len()
            && left.mtime() == right.mtime()
            && left.mtime_nsec() == right.mtime_nsec()
            && left.ctime() == right.ctime()
            && left.ctime_nsec() == right.ctime_nsec()
    }
    #[cfg(unix)]
    fn sidecar_directory_metadata_unchanged(left: &SecureMetadata, right: &SecureMetadata) -> bool {
        use std::os::unix::fs::MetadataExt as _;
        Self::sidecar_metadata_same_object(left, right)
            && left.mtime() == right.mtime()
            && left.mtime_nsec() == right.mtime_nsec()
            && left.ctime() == right.ctime()
            && left.ctime_nsec() == right.ctime_nsec()
    }
    #[cfg(windows)]
    fn sidecar_directory_metadata_unchanged(left: &SecureMetadata, right: &SecureMetadata) -> bool {
        Self::sidecar_metadata_same_object(left, right)
            && left.last_write_time() == right.last_write_time()
            && left.creation_time() == right.creation_time()
    }
    #[cfg(all(not(unix), not(windows)))]
    fn sidecar_directory_metadata_unchanged(
        _left: &SecureMetadata,
        _right: &SecureMetadata,
    ) -> bool {
        false
    }
    fn sidecar_directory_binding_unchanged(left: &SecureMetadata, right: &SecureMetadata) -> bool {
        // Directory mtime/ctime describe mutations to child entries, not replacement of the
        // directory itself. Progress sidecars are published concurrently, so descriptor binding
        // must compare object identity only. Stable inventory scans deliberately retain the
        // stronger timestamp comparison in `sidecar_directory_metadata_unchanged`.
        Self::sidecar_metadata_same_object(left, right)
    }
    #[cfg(windows)]
    fn sidecar_file_metadata_unchanged(left: &SecureMetadata, right: &SecureMetadata) -> bool {
        Self::sidecar_metadata_same_object(left, right)
            && left.number_of_links() == Some(1)
            && right.number_of_links() == Some(1)
            && left.file_size() == right.file_size()
            && left.last_write_time() == right.last_write_time()
            && left.creation_time() == right.creation_time()
    }
    #[cfg(all(not(unix), not(windows)))]
    fn sidecar_file_metadata_unchanged(left: &SecureMetadata, right: &SecureMetadata) -> bool {
        Self::sidecar_metadata_same_object(left, right)
            && left.len() == right.len()
            && left.modified().ok() == right.modified().ok()
    }
    fn stable_sidecar_metadata_unchanged(
        left: &StableSidecarMetadata,
        right: &StableSidecarMetadata,
    ) -> bool {
        left.canonical_path == right.canonical_path
            && Self::sidecar_file_metadata_unchanged(&left.file, &right.file)
            && Self::sidecar_directory_metadata_unchanged(&left.directory, &right.directory)
    }
    fn stable_sidecar_file_binding_unchanged(
        left: &StableSidecarMetadata,
        right: &StableSidecarMetadata,
    ) -> bool {
        // A sibling publication legitimately advances the directory timestamps
        // without changing this file. Single-file reads bind the directory by
        // object identity; inventory scans use the stronger timestamp check in
        // `stable_sidecar_metadata_unchanged` to detect sibling mutations.
        left.canonical_path == right.canonical_path
            && Self::sidecar_file_metadata_unchanged(&left.file, &right.file)
            && Self::sidecar_directory_binding_unchanged(&left.directory, &right.directory)
    }
    fn stable_sidecar_directory_metadata_unchanged(
        left: &StableSidecarDirectoryMetadata,
        right: &StableSidecarDirectoryMetadata,
    ) -> bool {
        if left.expected_path != right.expected_path || left.canonical_path != right.canonical_path
        {
            return false;
        }
        match (&left.metadata, &right.metadata) {
            (None, None) => true,
            (Some(left), Some(right)) => Self::sidecar_directory_metadata_unchanged(left, right),
            (None, Some(_)) | (Some(_), None) => false,
        }
    }
    #[cfg(unix)]
    fn sidecar_is_single_link(metadata: &SecureMetadata) -> bool {
        use std::os::unix::fs::MetadataExt as _;
        metadata.nlink() == 1
    }
    #[cfg(windows)]
    fn sidecar_is_single_link(metadata: &SecureMetadata) -> bool {
        metadata.number_of_links() == Some(1)
    }
    #[cfg(all(not(unix), not(windows)))]
    fn sidecar_is_single_link(_metadata: &SecureMetadata) -> bool {
        false
    }
    #[cfg(windows)]
    fn sidecar_has_link_count(metadata: &SecureMetadata, expected: u64) -> bool {
        u32::try_from(expected)
            .ok()
            .is_some_and(|expected| metadata.number_of_links() == Some(expected))
    }
    #[cfg(all(not(unix), not(windows)))]
    fn sidecar_has_link_count(_metadata: &SecureMetadata, _expected: u64) -> bool {
        false
    }
    fn canonical_sidecar_directory_for(
        store_root: &Path,
        expected_directory: &Path,
    ) -> Result<Option<(PathBuf, SecureMetadata)>> {
        #[cfg(test)]
        SIDECAR_DIRECTORY_CANONICALIZATIONS.with(|count| {
            if let Some(current) = count.get() {
                count.set(Some(current + 1));
            }
        });
        let before = match secure_file_metadata::from_path(expected_directory) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(Error::IO(error, expected_directory.to_path_buf())),
        };
        if before.file_type().is_symlink() || !before.is_dir() {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "sidecar directory is not a direct directory",
                ),
                expected_directory.to_path_buf(),
            ));
        }
        let relative = expected_directory.strip_prefix(store_root).map_err(|_| {
            Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "sidecar directory is outside the configured Kura root",
                ),
                expected_directory.to_path_buf(),
            )
        })?;
        let canonical_root = std::fs::canonicalize(store_root)
            .map_err(|error| Error::IO(error, store_root.to_path_buf()))?;
        let canonical_directory = std::fs::canonicalize(expected_directory)
            .map_err(|error| Error::IO(error, expected_directory.to_path_buf()))?;
        if canonical_directory != canonical_root.join(relative) {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "sidecar directory contains a symlink or escapes the Kura root",
                ),
                expected_directory.to_path_buf(),
            ));
        }
        let after = secure_file_metadata::from_path(expected_directory)
            .map_err(|error| Error::IO(error, expected_directory.to_path_buf()))?;
        if after.file_type().is_symlink()
            || !after.is_dir()
            || !Self::sidecar_directory_binding_unchanged(&before, &after)
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "sidecar directory changed during canonical validation",
                ),
                expected_directory.to_path_buf(),
            ));
        }
        Ok(Some((canonical_directory, after)))
    }
    fn canonical_sidecar_directory(
        &self,
        expected_directory: &Path,
    ) -> Result<Option<(PathBuf, SecureMetadata)>> {
        Self::canonical_sidecar_directory_for(&self.store_root, expected_directory)
    }
    fn stable_sidecar_directory_metadata(
        &self,
        expected_directory: &Path,
    ) -> Result<StableSidecarDirectoryMetadata> {
        let current = self.canonical_sidecar_directory(expected_directory)?;
        Ok(StableSidecarDirectoryMetadata {
            expected_path: expected_directory.to_path_buf(),
            canonical_path: current.as_ref().map(|(path, _)| path.clone()),
            metadata: current.map(|(_, metadata)| metadata),
        })
    }
    fn regular_sidecar_metadata_for(
        store_root: &Path,
        path: &Path,
        expected_directory: &Path,
    ) -> Result<Option<StableSidecarMetadata>> {
        if path.parent() != Some(expected_directory) {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "sidecar path is not an immediate child of its expected directory",
                ),
                path.to_path_buf(),
            ));
        }
        let directory = Self::canonical_sidecar_directory_for(store_root, expected_directory)?;
        let metadata = match secure_file_metadata::from_path(path) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(Error::IO(err, path.to_path_buf())),
        };
        let Some((canonical_directory, directory_metadata)) = directory else {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "sidecar exists without its expected direct directory",
                ),
                path.to_path_buf(),
            ));
        };
        if !metadata.file_type().is_file()
            || metadata.file_type().is_symlink()
            || !Self::sidecar_is_single_link(&metadata)
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "sidecar path is not a single-link regular file",
                ),
                path.to_path_buf(),
            ));
        }
        let canonical_path =
            std::fs::canonicalize(path).map_err(|err| Error::IO(err, path.to_path_buf()))?;
        if canonical_path.parent() != Some(canonical_directory.as_path()) {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "sidecar path escapes its canonical Kura directory",
                ),
                path.to_path_buf(),
            ));
        }
        Ok(Some(StableSidecarMetadata {
            canonical_path,
            file: metadata,
            directory: directory_metadata,
        }))
    }
    fn open_bound_progress_file(
        namespace: &BoundProgressNamespace,
        path: &Path,
        expected: &StableSidecarMetadata,
    ) -> Result<std::fs::File> {
        let immediate = namespace.directories.first().ok_or_else(|| {
            Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "bound progress namespace has no immediate directory",
                ),
                path.to_path_buf(),
            )
        })?;
        if path.parent() != Some(immediate.expected_path.as_path()) {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidInput,
                    "progress sidecar file is outside its bound immediate directory",
                ),
                path.to_path_buf(),
            ));
        }
        let _file_name = path.file_name().ok_or_else(|| {
            Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidInput,
                    "progress sidecar file has no entry name",
                ),
                path.to_path_buf(),
            )
        })?;
        #[cfg(unix)]
        let file = std::fs::File::from(
            rustix::fs::openat(
                &immediate.file,
                _file_name,
                rustix::fs::OFlags::RDWR
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(std::io::Error::from)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?,
        );
        #[cfg(not(unix))]
        let file = {
            let mut options = std::fs::OpenOptions::new();
            options.read(true).write(true);
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt as _;
                const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
                options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
            }
            options
                .open(path)
                .map_err(|error| Error::IO(error, path.to_path_buf()))?
        };
        let opened = secure_file_metadata::from_file(&file)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        if !opened.is_file() || !Self::sidecar_file_metadata_unchanged(&expected.file, &opened) {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "progress sidecar file changed while opening",
                ),
                path.to_path_buf(),
            ));
        }
        Ok(file)
    }
    fn open_direct_sidecar_file_in_namespace(
        path: &Path,
        create: bool,
        append: bool,
        _namespace: Option<&BoundProgressNamespace>,
    ) -> std::io::Result<std::fs::File> {
        let before = match secure_file_metadata::from_path(path) {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == ErrorKind::NotFound && create => None,
            Err(error) => return Err(error),
        };
        if before.as_ref().is_some_and(|metadata| {
            metadata.file_type().is_symlink()
                || !metadata.is_file()
                || !Self::sidecar_is_single_link(metadata)
        }) {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "sidecar path is not a direct single-link regular file",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
            let parent_path = path.parent().ok_or_else(|| {
                std::io::Error::new(ErrorKind::InvalidInput, "sidecar path has no parent")
            })?;
            let file_name = path.file_name().ok_or_else(|| {
                std::io::Error::new(ErrorKind::InvalidInput, "sidecar path has no file name")
            })?;
            let owned_parent;
            let parent = if let Some(namespace) = _namespace {
                let immediate = namespace.directories.first().ok_or_else(|| {
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "bound sidecar namespace has no immediate directory",
                    )
                })?;
                if parent_path != immediate.expected_path {
                    return Err(std::io::Error::new(
                        ErrorKind::InvalidData,
                        "sidecar path is outside its bound namespace",
                    ));
                }
                let opened = immediate.file.metadata()?;
                let current = std::fs::symlink_metadata(parent_path)?;
                if !opened.is_dir()
                    || current.file_type().is_symlink()
                    || !current.is_dir()
                    || !Self::sidecar_metadata_same_object(&immediate.metadata, &opened)
                    || !Self::sidecar_metadata_same_object(&immediate.metadata, &current)
                {
                    return Err(std::io::Error::new(
                        ErrorKind::InvalidData,
                        "bound sidecar namespace changed before mutation",
                    ));
                }
                &immediate.file
            } else {
                let parent_before = std::fs::symlink_metadata(parent_path)?;
                if parent_before.file_type().is_symlink() || !parent_before.is_dir() {
                    return Err(std::io::Error::new(
                        ErrorKind::InvalidData,
                        "sidecar parent is not a direct directory",
                    ));
                }
                let mut options = std::fs::OpenOptions::new();
                options.read(true).custom_flags(
                    (rustix::fs::OFlags::DIRECTORY
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC)
                        .bits() as i32,
                );
                owned_parent = options.open(parent_path)?;
                let parent_opened = owned_parent.metadata()?;
                let parent_after = std::fs::symlink_metadata(parent_path)?;
                if !parent_opened.is_dir()
                    || parent_after.file_type().is_symlink()
                    || !parent_after.is_dir()
                    || !Self::sidecar_metadata_same_object(&parent_before, &parent_opened)
                    || !Self::sidecar_metadata_same_object(&parent_before, &parent_after)
                {
                    return Err(std::io::Error::new(
                        ErrorKind::InvalidData,
                        "sidecar parent changed while binding it",
                    ));
                }
                &owned_parent
            };
            let mut flags = rustix::fs::OFlags::RDWR
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC;
            if create {
                flags |= rustix::fs::OFlags::CREATE;
            }
            if append {
                flags |= rustix::fs::OFlags::APPEND;
            }
            let file = std::fs::File::from(
                rustix::fs::openat(
                    parent,
                    file_name,
                    flags,
                    rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
                )
                .map_err(std::io::Error::from)?,
            );
            let opened = file.metadata()?;
            let after =
                rustix::fs::statat(parent, file_name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
                    .map_err(std::io::Error::from)?;
            if !opened.is_file()
                || !Self::sidecar_is_single_link(&opened)
                || after.st_dev as u64 != opened.dev()
                || after.st_ino as u64 != opened.ino()
                || after.st_nlink as u64 != 1
                || before
                    .as_ref()
                    .is_some_and(|metadata| !Self::sidecar_metadata_same_object(metadata, &opened))
            {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    "sidecar path identity changed while opening",
                ));
            }
            return Ok(file);
        }
        #[cfg(not(unix))]
        {
            let mut options = std::fs::OpenOptions::new();
            options.read(true).write(true).create(create).append(append);
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt as _;
                const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
                options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
            }
            let file = options.open(path)?;
            let opened = secure_file_metadata::from_file(&file)?;
            let after = secure_file_metadata::from_path(path)?;
            if !opened.is_file()
                || after.file_type().is_symlink()
                || !after.is_file()
                || !Self::sidecar_is_single_link(&opened)
                || !Self::sidecar_is_single_link(&after)
                || !Self::sidecar_metadata_same_object(&opened, &after)
                || before
                    .as_ref()
                    .is_some_and(|metadata| !Self::sidecar_metadata_same_object(metadata, &opened))
            {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    "sidecar path identity changed while opening",
                ));
            }
            Ok(file)
        }
    }
    fn remove_bound_progress_temp_if_present(
        namespace: &BoundProgressNamespace,
        path: &Path,
    ) -> std::io::Result<()> {
        let immediate = namespace.directories.first().ok_or_else(|| {
            std::io::Error::new(
                ErrorKind::InvalidData,
                "bound progress namespace has no immediate directory",
            )
        })?;
        if path.parent() != Some(immediate.expected_path.as_path()) {
            return Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                "progress temp is outside its bound namespace",
            ));
        }
        let _name = path.file_name().ok_or_else(|| {
            std::io::Error::new(ErrorKind::InvalidInput, "progress temp has no entry name")
        })?;
        #[cfg(unix)]
        {
            let entry = match rustix::fs::statat(
                &immediate.file,
                _name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            ) {
                Ok(entry) => entry,
                Err(rustix::io::Errno::NOENT) => return Ok(()),
                Err(error) => return Err(std::io::Error::from(error)),
            };
            if rustix::fs::FileType::from_raw_mode(entry.st_mode)
                != rustix::fs::FileType::RegularFile
                || entry.st_nlink as u64 != 1
            {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    "progress temp is not a direct single-link regular file",
                ));
            }
            rustix::fs::unlinkat(&immediate.file, _name, rustix::fs::AtFlags::empty())
                .map_err(std::io::Error::from)?;
            return Ok(());
        }
        #[cfg(not(unix))]
        {
            match secure_file_metadata::from_path(path) {
                Ok(metadata)
                    if metadata.is_file()
                        && !metadata.file_type().is_symlink()
                        && Self::sidecar_is_single_link(&metadata) =>
                {
                    std::fs::remove_file(path)
                }
                Ok(_) => Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    "progress temp is not a direct single-link regular file",
                )),
                Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            }
        }
    }
    fn remove_bound_progress_file_if_matches(
        namespace: &BoundProgressNamespace,
        path: &Path,
        expected: &std::fs::File,
        expected_snapshot: &StableSidecarMetadata,
    ) -> std::io::Result<()> {
        let immediate = namespace.directories.first().ok_or_else(|| {
            std::io::Error::new(
                ErrorKind::InvalidData,
                "bound progress namespace has no immediate directory",
            )
        })?;
        if path.parent() != Some(immediate.expected_path.as_path()) {
            return Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                "progress file is outside its bound namespace",
            ));
        }
        let _name = path.file_name().ok_or_else(|| {
            std::io::Error::new(ErrorKind::InvalidInput, "progress file has no entry name")
        })?;
        let expected_metadata = secure_file_metadata::from_file(expected)?;
        if !Self::sidecar_file_metadata_unchanged(&expected_snapshot.file, &expected_metadata) {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "progress file changed after exact-object verification",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            let entry = rustix::fs::statat(
                &immediate.file,
                _name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(std::io::Error::from)?;
            if rustix::fs::FileType::from_raw_mode(entry.st_mode)
                != rustix::fs::FileType::RegularFile
                || expected_metadata.nlink() != 1
                || entry.st_dev as u64 != expected_snapshot.file.dev()
                || entry.st_ino as u64 != expected_snapshot.file.ino()
                || entry.st_nlink as u64 != 1
            {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    "progress file changed before exact-object removal",
                ));
            }
            rustix::fs::unlinkat(&immediate.file, _name, rustix::fs::AtFlags::empty())
                .map_err(std::io::Error::from)?;
            return Ok(());
        }
        #[cfg(not(unix))]
        {
            let current = secure_file_metadata::from_path(path)?;
            if current.file_type().is_symlink()
                || !current.is_file()
                || !Self::sidecar_is_single_link(&current)
                || !Self::sidecar_file_metadata_unchanged(&expected_snapshot.file, &current)
            {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    "progress file changed before exact-object removal",
                ));
            }
            std::fs::remove_file(path)
        }
    }
    fn create_new_bound_progress_temp(
        namespace: &BoundProgressNamespace,
        path: &Path,
    ) -> std::io::Result<std::fs::File> {
        let immediate = namespace.directories.first().ok_or_else(|| {
            std::io::Error::new(
                ErrorKind::InvalidData,
                "bound progress namespace has no immediate directory",
            )
        })?;
        if path.parent() != Some(immediate.expected_path.as_path()) {
            return Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                "progress temp is outside its bound namespace",
            ));
        }
        let _name = path.file_name().ok_or_else(|| {
            std::io::Error::new(ErrorKind::InvalidInput, "progress temp has no entry name")
        })?;
        #[cfg(unix)]
        {
            let file = std::fs::File::from(
                rustix::fs::openat(
                    &immediate.file,
                    _name,
                    rustix::fs::OFlags::RDWR
                        | rustix::fs::OFlags::CREATE
                        | rustix::fs::OFlags::EXCL
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
                )
                .map_err(std::io::Error::from)?,
            );
            let metadata = secure_file_metadata::from_file(&file)?;
            let entry = rustix::fs::statat(
                &immediate.file,
                _name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(std::io::Error::from)?;
            use std::os::unix::fs::MetadataExt as _;
            if !metadata.is_file()
                || metadata.nlink() != 1
                || entry.st_dev as u64 != metadata.dev()
                || entry.st_ino as u64 != metadata.ino()
                || entry.st_nlink as u64 != 1
            {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    "progress temp identity changed during exclusive creation",
                ));
            }
            return Ok(file);
        }
        #[cfg(not(unix))]
        {
            let mut options = std::fs::OpenOptions::new();
            options.read(true).write(true).create_new(true);
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt as _;
                const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
                options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
            }
            let file = options.open(path)?;
            let metadata = secure_file_metadata::from_file(&file)?;
            if !metadata.is_file() || !Self::sidecar_is_single_link(&metadata) {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    "progress temp is not a direct single-link regular file",
                ));
            }
            Ok(file)
        }
    }
    fn promote_bound_progress_temp(
        namespace: &BoundProgressNamespace,
        temp_path: &Path,
        main_path: &Path,
        temp: &std::fs::File,
    ) -> std::result::Result<(), BoundProgressPromotionError> {
        let unpublished = |source| BoundProgressPromotionError {
            published: false,
            source,
        };
        let _published = |source| BoundProgressPromotionError {
            published: true,
            source,
        };
        let immediate = namespace
            .directories
            .first()
            .ok_or_else(|| {
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "bound progress namespace has no immediate directory",
                )
            })
            .map_err(unpublished)?;
        if temp_path.parent() != Some(immediate.expected_path.as_path())
            || main_path.parent() != Some(immediate.expected_path.as_path())
        {
            return Err(unpublished(std::io::Error::new(
                ErrorKind::InvalidInput,
                "progress promotion escapes its bound namespace",
            )));
        }
        let _temp_name = temp_path
            .file_name()
            .ok_or_else(|| {
                std::io::Error::new(ErrorKind::InvalidInput, "progress temp has no entry name")
            })
            .map_err(unpublished)?;
        let _main_name = main_path
            .file_name()
            .ok_or_else(|| {
                std::io::Error::new(ErrorKind::InvalidInput, "progress index has no entry name")
            })
            .map_err(unpublished)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            let metadata = temp.metadata().map_err(unpublished)?;
            let before = rustix::fs::statat(
                &immediate.file,
                _temp_name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(std::io::Error::from)
            .map_err(unpublished)?;
            if !metadata.is_file()
                || metadata.nlink() != 1
                || before.st_dev as u64 != metadata.dev()
                || before.st_ino as u64 != metadata.ino()
                || before.st_nlink as u64 != 1
            {
                return Err(unpublished(std::io::Error::new(
                    ErrorKind::InvalidData,
                    "progress temp changed before promotion",
                )));
            }
            rustix::fs::renameat(&immediate.file, _temp_name, &immediate.file, _main_name)
                .map_err(std::io::Error::from)
                .map_err(unpublished)?;
            let after = rustix::fs::statat(
                &immediate.file,
                _main_name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(std::io::Error::from)
            .map_err(_published)?;
            if after.st_dev as u64 != metadata.dev()
                || after.st_ino as u64 != metadata.ino()
                || after.st_nlink as u64 != 1
            {
                return Err(_published(std::io::Error::new(
                    ErrorKind::InvalidData,
                    "promoted progress index has the wrong identity",
                )));
            }
            return Ok(());
        }
        #[cfg(not(unix))]
        {
            let _ = (namespace, temp_path, main_path, temp);
            Err(BoundProgressPromotionError {
                published: false,
                source: std::io::Error::new(
                    ErrorKind::Unsupported,
                    "descriptor-relative progress prepend promotion is unsupported on this platform",
                ),
            })
        }
    }
    fn bound_progress_append_build_path(index_path: &Path) -> PathBuf {
        index_path.with_extension("index.append.build.tmp")
    }
    fn bound_progress_append_intent_path(index_path: &Path) -> PathBuf {
        index_path.with_extension("index.append.intent.tmp")
    }
    fn sync_bound_progress_intent_directories(
        namespace: &BoundProgressNamespace,
    ) -> std::io::Result<()> {
        #[cfg(test)]
        let fail_at = FAIL_BOUND_PROGRESS_INTENT_DIRECTORY_SYNC.with(|slot| {
            let Some(mut fault) = slot.get() else {
                return None;
            };
            if fault.calls_before_failure > 0 {
                fault.calls_before_failure -= 1;
                slot.set(Some(fault));
                None
            } else {
                Some(fault.target_index)
            }
        });
        #[cfg(not(test))]
        let fail_at: Option<usize> = None;
        for (position, directory) in namespace.directories.iter().enumerate() {
            if fail_at == Some(position) {
                #[cfg(test)]
                FAIL_BOUND_PROGRESS_INTENT_DIRECTORY_SYNC.with(|slot| slot.set(None));
                return Err(std::io::Error::other(
                    "injected bound progress append-intent directory sync failure",
                ));
            }
            directory.file.sync_all()?;
        }
        Ok(())
    }
    fn promote_bound_progress_temp_noreplace(
        namespace: &BoundProgressNamespace,
        temp_path: &Path,
        intent_path: &Path,
        temp: &std::fs::File,
    ) -> std::result::Result<(), BoundProgressPromotionError> {
        let unpublished = |source| BoundProgressPromotionError {
            published: false,
            source,
        };
        let _published = |source| BoundProgressPromotionError {
            published: true,
            source,
        };
        let immediate = namespace
            .directories
            .first()
            .ok_or_else(|| {
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "bound progress namespace has no immediate directory",
                )
            })
            .map_err(unpublished)?;
        if temp_path.parent() != Some(immediate.expected_path.as_path())
            || intent_path.parent() != Some(immediate.expected_path.as_path())
        {
            return Err(unpublished(std::io::Error::new(
                ErrorKind::InvalidInput,
                "progress append-intent promotion escapes its bound namespace",
            )));
        }
        let _temp_name = temp_path
            .file_name()
            .ok_or_else(|| {
                std::io::Error::new(ErrorKind::InvalidInput, "progress build has no entry name")
            })
            .map_err(unpublished)?;
        let _intent_name = intent_path
            .file_name()
            .ok_or_else(|| {
                std::io::Error::new(ErrorKind::InvalidInput, "progress intent has no entry name")
            })
            .map_err(unpublished)?;
        #[cfg(any(target_vendor = "apple", target_os = "linux", target_os = "android"))]
        {
            use std::os::unix::fs::MetadataExt as _;
            let metadata = temp.metadata().map_err(unpublished)?;
            let before = rustix::fs::statat(
                &immediate.file,
                _temp_name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(std::io::Error::from)
            .map_err(unpublished)?;
            if !metadata.is_file()
                || metadata.nlink() != 1
                || before.st_dev as u64 != metadata.dev()
                || before.st_ino as u64 != metadata.ino()
                || before.st_nlink as u64 != 1
            {
                return Err(unpublished(std::io::Error::new(
                    ErrorKind::InvalidData,
                    "progress append-intent build changed before promotion",
                )));
            }
            rustix::fs::renameat_with(
                &immediate.file,
                _temp_name,
                &immediate.file,
                _intent_name,
                rustix::fs::RenameFlags::NOREPLACE,
            )
            .map_err(std::io::Error::from)
            .map_err(unpublished)?;
            let after = rustix::fs::statat(
                &immediate.file,
                _intent_name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(std::io::Error::from)
            .map_err(_published)?;
            if after.st_dev as u64 != metadata.dev()
                || after.st_ino as u64 != metadata.ino()
                || after.st_nlink as u64 != 1
            {
                return Err(_published(std::io::Error::new(
                    ErrorKind::InvalidData,
                    "published progress append intent has the wrong identity",
                )));
            }
            return Ok(());
        }
        #[cfg(not(any(target_vendor = "apple", target_os = "linux", target_os = "android")))]
        {
            let _ = (namespace, temp_path, intent_path, temp);
            Err(BoundProgressPromotionError {
                published: false,
                source: std::io::Error::new(
                    ErrorKind::Unsupported,
                    "atomic descriptor-relative append-intent publication is unsupported on this platform",
                ),
            })
        }
    }
    fn open_bound_progress_directory(
        store_root: &Path,
        expected_path: &Path,
    ) -> Result<BoundProgressDirectory> {
        let (canonical_path, metadata) =
            Self::canonical_sidecar_directory_for(store_root, expected_path)?.ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::NotFound,
                        "progress sidecar directory disappeared while binding it",
                    ),
                    expected_path.to_path_buf(),
                )
            })?;
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.custom_flags(
                (rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::NOFOLLOW).bits() as i32,
            );
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt as _;
            const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
            const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
            options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
        }
        let file = options
            .open(expected_path)
            .map_err(|error| Error::IO(error, expected_path.to_path_buf()))?;
        let opened = secure_file_metadata::from_file(&file)
            .map_err(|error| Error::IO(error, expected_path.to_path_buf()))?;
        let after = Self::canonical_sidecar_directory_for(store_root, expected_path)?;
        if !opened.is_dir()
            || !Self::sidecar_directory_binding_unchanged(&metadata, &opened)
            || !after.as_ref().is_some_and(|(after_path, after_metadata)| {
                *after_path == canonical_path
                    && Self::sidecar_directory_binding_unchanged(&metadata, after_metadata)
            })
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "progress sidecar directory changed while opening",
                ),
                expected_path.to_path_buf(),
            ));
        }
        Ok(BoundProgressDirectory {
            expected_path: expected_path.to_path_buf(),
            canonical_path,
            entry_name: None,
            file,
            metadata,
        })
    }
    fn open_bound_progress_child_directory(
        store_root: &Path,
        parent: &BoundProgressDirectory,
        expected_path: &Path,
    ) -> Result<BoundProgressDirectory> {
        #[cfg(unix)]
        let _ = store_root;
        if expected_path.parent() != Some(parent.expected_path.as_path()) {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidInput,
                    "progress sidecar directory is not a direct child of its bound parent",
                ),
                expected_path.to_path_buf(),
            ));
        }
        let name = expected_path.file_name().ok_or_else(|| {
            Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidInput,
                    "progress sidecar child directory has no entry name",
                ),
                expected_path.to_path_buf(),
            )
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            let before =
                rustix::fs::statat(&parent.file, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
                    .map_err(std::io::Error::from)
                    .map_err(|error| Error::IO(error, expected_path.to_path_buf()))?;
            if rustix::fs::FileType::from_raw_mode(before.st_mode)
                != rustix::fs::FileType::Directory
            {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "progress sidecar path component is not a direct directory",
                    ),
                    expected_path.to_path_buf(),
                ));
            }
            let file = std::fs::File::from(
                rustix::fs::openat(
                    &parent.file,
                    name,
                    rustix::fs::OFlags::RDONLY
                        | rustix::fs::OFlags::DIRECTORY
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                )
                .map_err(std::io::Error::from)
                .map_err(|error| Error::IO(error, expected_path.to_path_buf()))?,
            );
            let metadata = secure_file_metadata::from_file(&file)
                .map_err(|error| Error::IO(error, expected_path.to_path_buf()))?;
            let after =
                rustix::fs::statat(&parent.file, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
                    .map_err(std::io::Error::from)
                    .map_err(|error| Error::IO(error, expected_path.to_path_buf()))?;
            let canonical_path = parent.canonical_path.join(name);
            let canonical_after = std::fs::canonicalize(expected_path)
                .map_err(|error| Error::IO(error, expected_path.to_path_buf()))?;
            if !metadata.is_dir()
                || before.st_dev as u64 != metadata.dev()
                || before.st_ino as u64 != metadata.ino()
                || after.st_dev as u64 != metadata.dev()
                || after.st_ino as u64 != metadata.ino()
                || canonical_after != canonical_path
            {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "progress sidecar directory changed during descriptor-relative binding",
                    ),
                    expected_path.to_path_buf(),
                ));
            }
            return Ok(BoundProgressDirectory {
                expected_path: expected_path.to_path_buf(),
                canonical_path,
                entry_name: Some(name.to_os_string()),
                file,
                metadata,
            });
        }
        #[cfg(not(unix))]
        {
            let mut child = Self::open_bound_progress_directory(store_root, expected_path)?;
            child.entry_name = Some(name.to_os_string());
            Ok(child)
        }
    }
    fn open_bound_progress_namespace(
        &self,
        data_path: &Path,
        index_path: &Path,
    ) -> Result<BoundProgressNamespace> {
        let sidecar_dir = data_path.parent().ok_or_else(|| {
            Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidInput,
                    "progress sidecar data path has no parent",
                ),
                data_path.to_path_buf(),
            )
        })?;
        if index_path.parent() != Some(sidecar_dir) {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidInput,
                    "progress sidecar files do not share one parent directory",
                ),
                index_path.to_path_buf(),
            ));
        }
        let mut directory_paths = vec![sidecar_dir.to_path_buf()];
        let mut ancestor = sidecar_dir;
        loop {
            if ancestor == self.store_root {
                break;
            }
            ancestor = ancestor.parent().ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "progress sidecar durability chain did not reach the Kura root",
                    ),
                    ancestor.to_path_buf(),
                )
            })?;
            directory_paths.push(ancestor.to_path_buf());
        }
        directory_paths.reverse();
        let Some(root_path) = directory_paths.first() else {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "progress sidecar durability chain is empty",
                ),
                sidecar_dir.to_path_buf(),
            ));
        };
        let root = Self::open_bound_progress_directory(&self.store_root, root_path)?;
        let mut directories = Vec::with_capacity(directory_paths.len());
        directories.push(root);
        for path in directory_paths.iter().skip(1) {
            let parent = directories
                .last()
                .expect("bound progress directory chain starts at Kura root");
            let child = Self::open_bound_progress_child_directory(&self.store_root, parent, path)?;
            directories.push(child);
        }
        directories.reverse();
        let namespace = BoundProgressNamespace {
            data_path: data_path.to_path_buf(),
            index_path: index_path.to_path_buf(),
            directories,
        };
        if !self.bound_progress_namespace_unchanged(&namespace) {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "progress sidecar namespace changed while binding durability handles",
                ),
                sidecar_dir.to_path_buf(),
            ));
        }
        Ok(namespace)
    }
    fn bound_progress_namespace_unchanged(&self, namespace: &BoundProgressNamespace) -> bool {
        namespace
            .directories
            .iter()
            .enumerate()
            .all(|(_index, directory)| {
                let Ok(opened) = secure_file_metadata::from_file(&directory.file) else {
                    return false;
                };
                if !opened.is_dir()
                    || !Self::sidecar_directory_binding_unchanged(&directory.metadata, &opened)
                {
                    return false;
                }
                #[cfg(unix)]
                if let Some(name) = directory.entry_name.as_deref() {
                    use std::os::unix::fs::MetadataExt as _;
                    let Some(parent) = namespace.directories.get(_index.saturating_add(1)) else {
                        return false;
                    };
                    if directory.expected_path.parent() != Some(parent.expected_path.as_path())
                        || directory.expected_path.file_name() != Some(name)
                        || directory.canonical_path != parent.canonical_path.join(name)
                    {
                        return false;
                    }
                    let Ok(entry) = rustix::fs::statat(
                        &parent.file,
                        name,
                        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
                    ) else {
                        return false;
                    };
                    // Every ancestor is checked by this same traversal. The
                    // terminal root (or standalone directory) below retains
                    // its full canonical path binding. A fresh no-follow link
                    // to that bound parent proves this child's path identity
                    // without resolving the whole root-to-child path again.
                    return rustix::fs::FileType::from_raw_mode(entry.st_mode)
                        == rustix::fs::FileType::Directory
                        && entry.st_dev as u64 == opened.dev()
                        && entry.st_ino as u64 == opened.ino();
                }
                Self::canonical_sidecar_directory_for(&self.store_root, &directory.expected_path)
                    .ok()
                    .flatten()
                    .is_some_and(|(canonical_path, metadata)| {
                        let path_matches = Self::sidecar_directory_binding_unchanged(
                            &directory.metadata,
                            &metadata,
                        );
                        canonical_path == directory.canonical_path && path_matches
                    })
            })
    }

    fn read_regular_sidecar_bytes_for(
        store_root: &Path,
        path: &Path,
        expected_directory: &Path,
        byte_limit: usize,
    ) -> Result<Option<Vec<u8>>> {
        Ok(Self::read_regular_sidecar_snapshot_for(
            store_root,
            path,
            expected_directory,
            byte_limit,
        )?
        .map(|snapshot| snapshot.bytes))
    }
    fn read_regular_sidecar_snapshot_for(
        store_root: &Path,
        path: &Path,
        expected_directory: &Path,
        byte_limit: usize,
    ) -> Result<Option<StableSidecarRead>> {
        Self::read_regular_sidecar_snapshot_for_with_admission_hook(
            store_root,
            path,
            expected_directory,
            byte_limit,
            || {},
        )
    }
    /// Perform one stable, no-follow bounded read after invoking a post-admission hook.
    ///
    /// Production passes a no-op hook. Tests use the seam to grow a file after
    /// its capped metadata has been admitted, proving that the reader allocates
    /// only the admitted length and rejects the max-plus-one race.
    fn read_regular_sidecar_snapshot_for_with_admission_hook<F>(
        store_root: &Path,
        path: &Path,
        expected_directory: &Path,
        byte_limit: usize,
        after_admission: F,
    ) -> Result<Option<StableSidecarRead>>
    where
        F: FnOnce(),
    {
        let directory_before =
            Self::canonical_sidecar_directory_for(store_root, expected_directory)?;
        let Some(metadata) =
            Self::regular_sidecar_metadata_for(store_root, path, expected_directory)?
        else {
            return Ok(None);
        };
        let Some((_, directory_before)) = directory_before else {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "sidecar exists without a stable direct directory",
                ),
                path.to_path_buf(),
            ));
        };
        if !Self::sidecar_directory_binding_unchanged(&directory_before, &metadata.directory) {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "sidecar directory changed before bounded read",
                ),
                path.to_path_buf(),
            ));
        }
        if metadata.file.len() > u64::try_from(byte_limit)? {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "sidecar exceeds its hard byte limit",
                ),
                path.to_path_buf(),
            ));
        }
        after_admission();
        let mut file =
            std::fs::File::open(path).map_err(|err| Error::IO(err, path.to_path_buf()))?;
        let opened_metadata = secure_file_metadata::from_file(&file)
            .map_err(|err| Error::IO(err, path.to_path_buf()))?;
        if !opened_metadata.is_file()
            || !Self::sidecar_file_metadata_unchanged(&metadata.file, &opened_metadata)
        {
            return Err(Error::IO(
                std::io::Error::new(ErrorKind::InvalidData, "sidecar file changed while opening"),
                path.to_path_buf(),
            ));
        }
        let mut bytes = Vec::new();
        let expected_len = usize::try_from(metadata.file.len())?;
        bytes.try_reserve_exact(expected_len)?;
        bytes.resize(expected_len, 0);
        file.read_exact(&mut bytes)
            .map_err(|err| Error::IO(err, path.to_path_buf()))?;
        let mut growth_probe = [0_u8; 1];
        if file
            .read(&mut growth_probe)
            .map_err(|err| Error::IO(err, path.to_path_buf()))?
            != 0
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "sidecar grew beyond its admitted length while reading",
                ),
                path.to_path_buf(),
            ));
        }
        let opened_after = secure_file_metadata::from_file(&file)
            .map_err(|err| Error::IO(err, path.to_path_buf()))?;
        let path_after = Self::regular_sidecar_metadata_for(store_root, path, expected_directory)?;
        if bytes.len() > byte_limit
            || u64::try_from(bytes.len())? != metadata.file.len()
            || !Self::sidecar_file_metadata_unchanged(&metadata.file, &opened_after)
            || !path_after.as_ref().is_some_and(|after| {
                Self::stable_sidecar_file_binding_unchanged(&metadata, after)
                    && Self::sidecar_metadata_same_object(&directory_before, &after.directory)
            })
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "sidecar changed or exceeded its hard byte limit while reading",
                ),
                path.to_path_buf(),
            ));
        }
        Ok(Some(StableSidecarRead {
            bytes_hash: Hash::new(&bytes),
            bytes,
            metadata: path_after.expect("validated stable sidecar metadata exists"),
        }))
    }
    #[cfg(not(test))]
    fn record_startup_replay_historical_payload_read(&self) {}
    /// Remove pending certificates outside the current carrier lineage.
    ///
    /// Same-parent certificates from earlier views remain available to service already-durable
    /// lifecycle Validate work. Exact-round selection still excludes them from a newer body, and
    /// finalized-height cleanup bounds their lifetime. The pending-count and aggregate-byte limits
    /// remain hard admission bounds across a view storm.
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "TODO: wire native consensus owner")
    )]
    pub(crate) fn prune_pending_certified_merge_entries_not_bound_to(
        &self,
        carrier_height: u64,
        carrier_parent_hash: HashOf<BlockHeader>,
        view: u64,
    ) -> Result<usize> {
        self.ensure_prune_recovery_not_required()?;
        self.durable_mutation_authorized()?;
        let _guard = self.sidecar_lock.lock();
        self.reconcile_pending_merge_temp_files_unlocked()?;
        self.prune_pending_certified_merge_entries_not_bound_to_unlocked(
            carrier_height,
            carrier_parent_hash,
            view,
        )
    }

    /// Return at most `limit` pending QueuePlan admission certificates in
    /// deterministic byte-hash order.
    pub(crate) fn pending_queue_plan_admission_certificates_bounded(
        &self,
        limit: usize,
    ) -> Result<Vec<(Hash, Vec<u8>)>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        self.ensure_prune_recovery_not_required()?;
        let certificates = {
            let _guard = self.sidecar_lock.lock();
            self.ensure_prune_recovery_not_required()?;
            let paths = self.pending_queue_plan_admission_inventory_paths_unlocked()?;
            let bounded = limit.min(self.pending_control_sidecar_limits.queue_plan_admissions);
            let mut certificates = Vec::with_capacity(paths.len().min(bounded));
            for path in paths.into_iter().take(bounded) {
                let Some(certificate) = self.read_pending_queue_plan_admission_path(&path, None)?
                else {
                    continue;
                };
                certificates.push(certificate);
            }
            certificates
        };
        self.ensure_prune_recovery_not_required()?;
        Ok(certificates)
    }


    /// Start the background block writer after all provisional startup authority is finalized.
    ///
    /// # Errors
    /// Returns an error in read-only emergency Fast mode, when the immutable
    /// local peer identity has not been bound, while signed snapshot
    /// authentication is pending, or canonical storage is fail-stop poisoned.
    pub fn start(kura: Arc<Self>, shutdown_signal: ShutdownSignal) -> Result<Child> {
        kura.durable_mutation_authorized()?;
        if kura.local_peer_id.get().is_none() {
            return Err(Error::KuraReplicaLocalPeerUnbound);
        }
        let shutdown_notify_tx = kura.block_notify_tx.clone();
        let shutdown_signal_clone = shutdown_signal.clone();
        tokio::spawn(async move {
            shutdown_signal_clone.receive().await;
            Self::notify_block_writer_sender(
                &shutdown_notify_tx,
                BlockNotify::Shutdown,
                "shutdown",
            );
        });
        Ok(Child::new(
            tokio::task::spawn(spawn_os_thread_as_future(
                std::thread::Builder::new().name("kura".to_owned()),
                move || {
                    kura.receive_blocks_loop(&shutdown_signal);
                },
            )),
            OnShutdown::Wait(Duration::from_secs(5)),
        ))
    }
    /// Initialize [`Kura`] after its construction to be able to work with it.
    ///
    /// # Errors
    /// Fails if:
    /// - file storage is unavailable
    /// - data in file storage is invalid or corrupted
    #[iroha_logger::log(skip_all, name = "kura_init")]
    fn init(
        block_store: &mut BlockStore,
        mode: InitMode,
        v2_finality_floor: Option<u64>,
        provisional_hash_only_prefix: Option<usize>,
        canonical_replica_terminal_carrier_pins: &BTreeMap<u64, HashOf<BlockHeader>>,
    ) -> Result<ChainValidation> {
        let block_index_count: usize = block_store
            .read_durable_index_count()?
            .try_into()
            .expect("INTERNAL BUG: block index count exceeds usize::MAX");
        let chain_validation = if let Some(audited_height) = provisional_hash_only_prefix {
            Kura::init_provisional_snapshot_bootstrap(
                block_store,
                block_index_count,
                audited_height,
                v2_finality_floor,
            )?
        } else {
            match mode {
                InitMode::Fast => {
                    warn!(
                        "Kura fast init trusts the durable local journal and defers full block validation; restart in strict mode after emergency recovery"
                    );
                    Kura::init_fast_mode(block_store, block_index_count, v2_finality_floor)
                }
                InitMode::Strict => Kura::init_canonical_chain(
                    block_store,
                    block_index_count,
                    v2_finality_floor,
                    canonical_replica_terminal_carrier_pins,
                ),
            }?
        };
        if chain_validation.truncated {
            warn!(
                validated_blocks = chain_validation.hashes.len(),
                "Kura detected corrupted storage during init and pruned to the last valid block"
            );
        }
        if chain_validation.hash_mismatch {
            warn!("Kura rewrote hashes file after detecting mismatches with on-disk blocks");
        }
        Ok(chain_validation)
    }
    fn init_provisional_snapshot_bootstrap(
        block_store: &mut BlockStore,
        block_index_count: usize,
        audited_height: usize,
        _v2_finality_floor: Option<u64>,
    ) -> Result<ChainValidation, Error> {
        let hashes_count = usize::try_from(block_store.read_hashes_count()?)?;
        if hashes_count != block_index_count || block_index_count < audited_height {
            return Err(Error::HashesFileHeightMismatch);
        }
        let hashes = block_store.read_block_hashes(0, hashes_count)?;
        let mut block_indices = vec![BlockIndex::default(); block_index_count];
        block_store.read_block_indices(0, &mut block_indices)?;
        let data_file_len = block_store.data_file_len()?;
        let mut buffer = Vec::new();
        let mut previous_hash = audited_height
            .checked_sub(1)
            .and_then(|index| hashes.get(index))
            .copied();
        for (index, block_index) in block_indices.iter().enumerate().skip(audited_height) {
            let height = u64::try_from(index)?.saturating_add(1);
            if block_index.length == 0 || block_index.length > STRICT_INIT_MAX_BLOCK_BYTES {
                return Err(Error::InvalidProvisionalSnapshotSuffix {
                    height,
                    reason: format!(
                        "block length {} is zero or exceeds the strict limit",
                        block_index.length
                    ),
                });
            }
            let header = if block_index.is_evicted() {
                block_store.verified_evicted_block_header(
                    height,
                    hashes[index],
                    block_index.length,
                )?
            } else {
                let end = block_index.start.checked_add(block_index.length).ok_or(
                    Error::CorruptedBlockRange {
                        start: block_index.start,
                        length: block_index.length,
                        data_len: data_file_len,
                    },
                )?;
                if end > data_file_len {
                    return Err(Error::CorruptedBlockRange {
                        start: block_index.start,
                        length: block_index.length,
                        data_len: data_file_len,
                    });
                }
                buffer.resize(usize::try_from(block_index.length)?, 0);
                block_store.read_block_data(block_index.start, &mut buffer)?;
                decode_framed_signed_block(&buffer)
                    .map_err(|error| Error::InvalidProvisionalSnapshotSuffix {
                        height,
                        reason: format!("canonical block body is not decodable: {error}"),
                    })?
                    .header()
            };
            if header.height().get() != height
                || header.prev_block_hash() != previous_hash
                || header.hash() != hashes[index]
            {
                return Err(Error::InvalidProvisionalSnapshotSuffix {
                    height,
                    reason: "block height, parent hash, or canonical header hash mismatches the durable journal"
                        .to_owned(),
                });
            }
            previous_hash = Some(hashes[index]);
        }
        Ok(ChainValidation {
            hashes,
            truncated: false,
            hash_mismatch: false,
            hard_fork_hash_only_block_count: audited_height,
        })
    }
    fn rewrite_validated_block_hashes(
        block_store: &mut BlockStore,
        hashes: &[HashOf<BlockHeader>],
        v2_finality_floor: Option<u64>,
    ) -> Result<()> {
        let Some(finalized_height) = v2_finality_floor else {
            return block_store.overwrite_block_hashes(hashes);
        };
        let suffix_start = usize::try_from(finalized_height)?;
        let Some(suffix) = hashes.get(suffix_start..) else {
            return Err(Error::HashesFileHeightMismatch);
        };
        block_store.overwrite_block_hash_suffix(finalized_height, suffix)
    }
    /// Open the exact durable journal without decoding historical block bodies.
    ///
    /// Read-only preflight has already bound the durable count and terminal hash.
    /// Fast mode leaves any unpublished suffix untouched and deliberately trusts the
    /// committed journal prefix. A requested block is still decoded and checked
    /// lazily before it is returned.
    fn init_fast_mode(
        block_store: &mut BlockStore,
        block_index_count: usize,
        v2_finality_floor: Option<u64>,
    ) -> Result<ChainValidation, Error> {
        if block_store.fast_prevalidated_count != Some(u64::try_from(block_index_count)?) {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "Kura fast init did not pass the read-only committed-prefix preflight",
                ),
                block_store.path_to_blockchain.clone(),
            ));
        }
        block_store.fast_prevalidated_count = None;
        if block_store.read_exact_durable_index_count()? != u64::try_from(block_index_count)? {
            return Err(Error::HashesFileHeightMismatch);
        }
        let durable_height = u64::try_from(block_index_count)?;
        if let Some(finalized_height) = v2_finality_floor
            && durable_height < finalized_height
        {
            return Err(Error::V2FinalityBeyondDurableChain {
                finalized_height,
                durable_height,
            });
        }
        Ok(ChainValidation {
            // Fast keeps only the durable count and reads exact hashes on demand. This avoids
            // startup I/O and RAM proportional to total chain height.
            hashes: Vec::new(),
            truncated: false,
            hash_mismatch: false,
            hard_fork_hash_only_block_count: 0,
        })
    }
    fn init_canonical_chain(
        block_store: &mut BlockStore,
        block_index_count: usize,
        v2_finality_floor: Option<u64>,
        canonical_replica_terminal_carrier_pins: &BTreeMap<u64, HashOf<BlockHeader>>,
    ) -> Result<ChainValidation, Error> {
        let mut block_indices = vec![BlockIndex::default(); block_index_count];
        block_store.read_block_indices(0, &mut block_indices)?;
        let hashes_count = block_store.read_hashes_count()?;
        if let Some(finalized_height) = v2_finality_floor {
            if u64::try_from(block_index_count)? < finalized_height {
                return Err(Error::FinalizedV2BlockMutation {
                    rewrite_from_height: u64::try_from(block_index_count)?.saturating_add(1),
                    finalized_height,
                });
            }
            if hashes_count < finalized_height {
                return Err(Error::FinalizedV2BlockMutation {
                    rewrite_from_height: hashes_count.saturating_add(1),
                    finalized_height,
                });
            }
        }
        let hash_journal_is_exact = hashes_count == block_index_count as u64;
        let expected_hashes = if hash_journal_is_exact {
            Some(block_store.read_block_hashes(0, block_index_count)?)
        } else if let Some(finalized_height) = v2_finality_floor {
            let finalized_count = usize::try_from(finalized_height)?;
            warn!(
                hashes_count,
                index_count = block_index_count,
                finalized_height,
                "strict Kura init is retaining the finalized hash prefix and rebuilding only its mutable suffix"
            );
            Some(block_store.read_block_hashes(0, finalized_count)?)
        } else {
            if hashes_count > 0 {
                warn!(
                    hashes_count,
                    index_count = block_index_count,
                    "strict Kura init cannot use hashes file for remote-only block metadata"
                );
            }
            None
        };
        let validation = Self::validate_block_chain(
            block_store,
            &block_indices,
            expected_hashes.as_deref(),
            0,
            v2_finality_floor,
            canonical_replica_terminal_carrier_pins,
        )?;
        if !hash_journal_is_exact || validation.truncated || validation.hash_mismatch {
            Self::rewrite_validated_block_hashes(
                block_store,
                &validation.hashes,
                v2_finality_floor,
            )?;
        }
        Ok(validation)
    }
    #[allow(clippy::too_many_lines)]
    fn validate_block_chain(
        block_store: &mut BlockStore,
        block_indices: &[BlockIndex],
        expected_hashes: Option<&[HashOf<BlockHeader>]>,
        mut hash_only_prefix: usize,
        v2_finality_floor: Option<u64>,
        canonical_replica_terminal_carrier_pins: &BTreeMap<u64, HashOf<BlockHeader>>,
    ) -> Result<ChainValidation, Error> {
        let hashes_count = block_store.read_hashes_count()?;
        let durable_height_bound = u64::try_from(block_indices.len())?;
        if canonical_replica_terminal_carrier_pins
            .keys()
            .any(|height| *height == 0 || *height > durable_height_bound)
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "canonical replica terminal carrier pin is outside the durable block journal",
                ),
                block_store.path_to_blockchain.clone(),
            ));
        }
        let verified_snapshot_tail = block_store
            .validated_verified_snapshot_tail(u64::try_from(block_indices.len())?, hashes_count)?;
        if let Some(marker) = verified_snapshot_tail.as_ref()
            && marker.body_prefix_count == marker.snapshot_height
        {
            hash_only_prefix = hash_only_prefix.max(usize::try_from(marker.snapshot_height)?);
        }
        if let Some(expected) = expected_hashes {
            if expected.len() > block_indices.len() || expected.len() < hash_only_prefix {
                return Err(Error::HashesFileHeightMismatch);
            }
        } else if hash_only_prefix > 0 {
            return Err(Error::HashesFileHeightMismatch);
        }
        let mut block_hashes = Vec::with_capacity(block_indices.len());
        let mut block_data_buffer = Vec::new();
        let mut prev_block_hash = None;
        let data_file_len = block_store.data_file_len()?;
        let mut truncated = None;
        let mut hash_mismatch = false;
        for (idx, block) in block_indices.iter().enumerate() {
            let height = idx.saturating_add(1) as u64;
            let required_carrier_hash = canonical_replica_terminal_carrier_pins
                .get(&height)
                .copied();
            if let Some(required_carrier_hash) = required_carrier_hash {
                let expected_carrier_hash = expected_hashes
                    .and_then(|hashes| hashes.get(idx))
                    .copied()
                    .ok_or(Error::HashesFileHeightMismatch)?;
                if expected_carrier_hash != required_carrier_hash {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "canonical replica terminal carrier pin conflicts with the durable hash journal",
                        ),
                        block_store.da_block_path(height),
                    ));
                }
            }
            if idx < hash_only_prefix {
                if required_carrier_hash.is_some() {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "canonical replica terminal carrier cannot be hash-only",
                        ),
                        block_store.da_block_path(height),
                    ));
                }
                let expected = expected_hashes
                    .and_then(|hashes| hashes.get(idx))
                    .copied()
                    .ok_or(Error::HashesFileHeightMismatch)?;
                prev_block_hash = Some(expected);
                block_hashes.push(expected);
                continue;
            }
            if block.length == 0
                && block.is_evicted()
                && expected_hashes.is_some_and(|hashes| hashes.get(idx).is_some())
                && verified_snapshot_tail.as_ref().is_some_and(|marker| {
                    let position = u64::try_from(idx).unwrap_or(u64::MAX);
                    let marker_start = if marker.body_prefix_count == marker.snapshot_height {
                        0
                    } else {
                        marker.body_prefix_count
                    };
                    position >= marker_start && position < marker.snapshot_height
                })
            {
                if required_carrier_hash.is_some() {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "canonical replica terminal carrier has no complete local body",
                        ),
                        block_store.da_block_path(height),
                    ));
                }
                let expected = expected_hashes
                    .and_then(|hashes| hashes.get(idx))
                    .copied()
                    .ok_or(Error::HashesFileHeightMismatch)?;
                debug!(
                    block_index = idx,
                    height,
                    block = %expected,
                    "preserving verified snapshot hash-only block metadata from hashes file"
                );
                prev_block_hash = Some(expected);
                block_hashes.push(expected);
                continue;
            }
            if block.length == 0 {
                if required_carrier_hash.is_some() {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "canonical replica terminal carrier has a zero-length block entry",
                        ),
                        block_store.da_block_path(height),
                    ));
                }
                truncated = Some(true);
                error!(
                    length = block.length,
                    limit = STRICT_INIT_MAX_BLOCK_BYTES,
                    "Encountered zero-length block entry; pruning to last valid block"
                );
                break;
            }
            if block.length > STRICT_INIT_MAX_BLOCK_BYTES {
                if required_carrier_hash.is_some() {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "canonical replica terminal carrier exceeds the strict block-size limit",
                        ),
                        block_store.da_block_path(height),
                    ));
                }
                truncated = Some(true);
                error!(
                    length = block.length,
                    limit = STRICT_INIT_MAX_BLOCK_BYTES,
                    "Encountered oversized block entry; pruning to last valid block"
                );
                break;
            }
            let decoded_block = if block.is_evicted() {
                let expected_evicted_hash =
                    expected_hashes.and_then(|hashes| hashes.get(idx)).copied();
                let expected_evicted_hash =
                    expected_evicted_hash.ok_or(Error::HashesFileHeightMismatch)?;
                let signed_wire_hash = block_store.verified_v2_finality_wire_hash(
                    height,
                    expected_evicted_hash,
                    block.length,
                )?;
                let payload = match block_store.read_optional_da_cache(height) {
                    Ok(payload) => payload,
                    Err(error) => {
                        return Err(error);
                    }
                };
                let Some(payload) = payload else {
                    if required_carrier_hash.is_some() {
                        return Err(Error::IO(
                            std::io::Error::new(
                                ErrorKind::NotFound,
                                "canonical replica terminal carrier DA body is missing",
                            ),
                            block_store.da_block_path(height),
                        ));
                    }
                    debug!(
                        block_index = idx,
                        height,
                        block = %expected_evicted_hash,
                        "preserving signed remote-only evicted block metadata from hashes file"
                    );
                    prev_block_hash = Some(expected_evicted_hash);
                    block_hashes.push(expected_evicted_hash);
                    continue;
                };
                {
                    let expected = expected_evicted_hash;
                    let retained_wire_hash = signed_wire_hash;
                    if u64::try_from(payload.len())? != block.length
                        || Hash::new(&payload) != retained_wire_hash
                    {
                        if required_carrier_hash.is_some() {
                            return Err(Error::IO(
                                std::io::Error::new(
                                    ErrorKind::InvalidData,
                                    "canonical replica terminal carrier DA body differs from signed finality",
                                ),
                                block_store.da_block_path(height),
                            ));
                        }
                        warn!(
                            block_index = idx,
                            height,
                            block = %expected,
                            "removing noncanonical DA cache and preserving remote-only metadata"
                        );
                        if let Err(remove_error) = block_store.remove_da_block_file(height) {
                            warn!(
                                ?remove_error,
                                height, "failed to remove complete-wire-mismatched DA block cache"
                            );
                        }
                        prev_block_hash = Some(expected);
                        block_hashes.push(expected);
                        continue;
                    }
                }
                match decode_framed_signed_block(&payload) {
                    Ok(decoded_block) => {
                        {
                            let expected = expected_evicted_hash;
                            let actual = decoded_block.hash();
                            if actual != expected {
                                if required_carrier_hash.is_some() {
                                    return Err(Error::IO(
                                        std::io::Error::new(
                                            ErrorKind::InvalidData,
                                            "canonical replica terminal carrier DA body has the wrong block hash",
                                        ),
                                        block_store.da_block_path(height),
                                    ));
                                }
                                warn!(
                                    expected = %expected,
                                    actual = %actual,
                                    block_index = idx,
                                    height,
                                    "removing local sidecar with mismatched block hash and preserving remote-only metadata"
                                );
                                if let Err(remove_error) = block_store.remove_da_block_file(height)
                                {
                                    warn!(
                                        ?remove_error,
                                        height, "failed to remove hash-mismatched DA block sidecar"
                                    );
                                }
                                prev_block_hash = Some(expected);
                                block_hashes.push(expected);
                                continue;
                            }
                        }
                        decoded_block
                    }
                    Err(error) => {
                        let expected = expected_evicted_hash;
                        if required_carrier_hash.is_some() {
                            return Err(Error::IO(
                                std::io::Error::new(
                                    ErrorKind::InvalidData,
                                    format!(
                                        "canonical replica terminal carrier DA body is malformed: {error}"
                                    ),
                                ),
                                block_store.da_block_path(height),
                            ));
                        }
                        warn!(
                            ?error,
                            block_index = idx,
                            height,
                            block = %expected,
                            "removing malformed local sidecar and preserving remote-only block metadata"
                        );
                        if let Err(remove_error) = block_store.remove_da_block_file(height) {
                            warn!(
                                ?remove_error,
                                height, "failed to remove malformed DA block sidecar"
                            );
                        }
                        prev_block_hash = Some(expected);
                        block_hashes.push(expected);
                        continue;
                    }
                }
            } else {
                let end =
                    block
                        .start
                        .checked_add(block.length)
                        .ok_or(Error::CorruptedBlockRange {
                            start: block.start,
                            length: block.length,
                            data_len: data_file_len,
                        })?;
                if end > data_file_len {
                    if required_carrier_hash.is_some() {
                        return Err(Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                "canonical replica terminal carrier points past the canonical data file",
                            ),
                            block_store.path_to_blockchain.clone(),
                        ));
                    }
                    truncated = Some(true);
                    error!(
                        start = block.start,
                        length = block.length,
                        data_len = data_file_len,
                        "Block index points past data file; pruning to last valid block"
                    );
                    break;
                }
                let length: usize = block.length.try_into()?;
                let additional = length.saturating_sub(block_data_buffer.len());
                if additional > 0 {
                    block_data_buffer.try_reserve(additional)?;
                }
                block_data_buffer.resize(length, 0);
                match block_store.read_block_data(block.start, &mut block_data_buffer) {
                    Ok(()) => match decode_framed_signed_block(&block_data_buffer) {
                        Ok(decoded_block) => decoded_block,
                        Err(error) => {
                            if required_carrier_hash.is_some() {
                                return Err(Error::IO(
                                    std::io::Error::new(
                                        ErrorKind::InvalidData,
                                        format!(
                                            "canonical replica terminal carrier inline body is malformed: {error}"
                                        ),
                                    ),
                                    block_store.path_to_blockchain.clone(),
                                ));
                            }
                            truncated = Some(true);
                            error!(
                                ?error,
                                block_index = idx,
                                "Malformed block payload; pruning to last valid block"
                            );
                            break;
                        }
                    },
                    Err(error) => {
                        if required_carrier_hash.is_some() {
                            let Error::IO(source, _) = error else {
                                return Err(error);
                            };
                            return Err(Error::IO(
                                std::io::Error::new(
                                    source.kind(),
                                    format!(
                                        "failed to read canonical replica terminal carrier inline body: {source}"
                                    ),
                                ),
                                block_store.path_to_blockchain.clone(),
                            ));
                        }
                        truncated = Some(true);
                        error!(
                            ?error,
                            block_index = idx,
                            "Failed to read block payload; pruning to last valid block"
                        );
                        break;
                    }
                }
            };
            if prev_block_hash != decoded_block.header().prev_block_hash() {
                truncated = Some(true);
                error!(
                    expected = ?prev_block_hash,
                    actual = ?decoded_block.header().prev_block_hash(),
                    block_index = idx,
                    "Previous block hash mismatch; pruning to last valid block"
                );
                break;
            }
            let decoded_block_hash = decoded_block.hash();
            if let Some(expected) = expected_hashes.and_then(|hashes| hashes.get(idx)).copied() {
                if expected != decoded_block_hash {
                    if required_carrier_hash.is_some() {
                        return Err(Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                "canonical replica terminal carrier inline body has the wrong block hash",
                            ),
                            block_store.path_to_blockchain.clone(),
                        ));
                    }
                    Self::ensure_startup_rewrite_respects_v2_finality(v2_finality_floor, height)?;
                    hash_mismatch = true;
                    warn!(
                        expected = ?expected,
                        actual = ?decoded_block_hash,
                        block_index = idx,
                        "Block hash file entry mismatched decoded block; rewriting hashes file"
                    );
                }
            }
            prev_block_hash = Some(decoded_block_hash);
            block_hashes.push(decoded_block_hash);
        }
        let truncated = truncated.unwrap_or(false);
        let validated_height = block_hashes.len() as u64;
        if truncated
            && canonical_replica_terminal_carrier_pins
                .keys()
                .any(|height| *height > validated_height)
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "canonical chain corruption precedes a pinned canonical replica terminal carrier",
                ),
                block_store.path_to_blockchain.clone(),
            ));
        }
        if truncated {
            Self::ensure_startup_rewrite_respects_v2_finality(
                v2_finality_floor,
                validated_height.saturating_add(1),
            )?;
            block_store.prune(validated_height)?;
            info!(
                validated_height,
                "Pruned Kura storage to last validated block after detecting corruption"
            );
        }
        let authenticated_hash_only_block_count = verified_snapshot_tail
            .as_ref()
            .filter(|marker| marker.body_prefix_count == marker.snapshot_height)
            .and_then(|marker| usize::try_from(marker.snapshot_height).ok())
            .unwrap_or(hash_only_prefix)
            .min(block_hashes.len());
        Ok(ChainValidation {
            hashes: block_hashes,
            truncated,
            hash_mismatch,
            hard_fork_hash_only_block_count: authenticated_hash_only_block_count,
        })
    }
    #[iroha_logger::log(skip_all)]
    fn receive_blocks_loop(&self, shutdown_signal: &ShutdownSignal) {
        let kura = self;
        let block_rx = kura
            .block_notify_rx
            .lock()
            .take()
            .expect("Kura writer thread already started");
        let mut should_exit = false;
        loop {
            if shutdown_signal.is_sent() {
                info!("Kura block thread is being shut down. Flushing sidecars and fsync state.");
                should_exit = true;
            }
            let prune_guard = kura.prune_lock.lock();
            if kura.prune_recovery_is_required() {
                error!("Kura writer stopped because canonical prune recovery requires restart");
                return;
            }
            kura.flush_pipeline_sidecars();
            kura.flush_fastpq_proof_snapshots();
            kura.flush_pending_budget_eviction();
            drop(prune_guard);
            if should_exit {
                let _prune_guard = kura.prune_lock.lock();
                if kura.prune_recovery_is_required() {
                    error!("Kura writer stopped because canonical prune recovery requires restart");
                    return;
                }
                let flush_result = {
                    let mut store = kura.block_store.lock();
                    kura.flush_pending_fsync_with_resources(&mut store, true)
                };
                if let Err(error) = flush_result {
                    error!(?error, "Failed to fsync pending blocks on shutdown");
                    kura.record_or_poison_fsync_fault("shutdown fsync", &error);
                    return;
                }
                info!("Kura has flushed sidecars and pending fsync state and is shutting down.");
                return;
            }
            let wait_for_fsync = {
                let guard = kura.block_store.lock();
                guard.next_fsync_wait()
            };
            match wait_for_fsync {
                Some(wait) => match block_rx.recv_timeout(wait) {
                    Ok(BlockNotify::NewBlock) => {
                        debug!("kura writer received sidecar flush signal");
                    }
                    Ok(BlockNotify::StorageBudgetEviction) => {
                        debug!("kura writer received storage-budget eviction signal");
                    }
                    Ok(BlockNotify::Shutdown) => {
                        should_exit = true;
                        debug!("kura writer received shutdown signal");
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        let _prune_guard = kura.prune_lock.lock();
                        if kura.prune_recovery_is_required() {
                            error!(
                                "Kura writer stopped because canonical prune recovery requires restart"
                            );
                            return;
                        }
                        let mut store = kura.block_store.lock();
                        if let Err(error) =
                            kura.flush_pending_fsync_with_resources(&mut store, false)
                        {
                            error!(?error, "Failed to fsync pending batch");
                            drop(store);
                            kura.record_or_poison_fsync_fault("periodic fsync", &error);
                            return;
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        info!("Block writer channel closed; exiting thread.");
                        return;
                    }
                },
                None => match block_rx.recv() {
                    Ok(BlockNotify::NewBlock) => {
                        debug!("kura writer received sidecar flush signal");
                    }
                    Ok(BlockNotify::StorageBudgetEviction) => {
                        debug!("kura writer received storage-budget eviction signal");
                    }
                    Ok(BlockNotify::Shutdown) => {
                        should_exit = true;
                        debug!("kura writer received shutdown signal");
                    }
                    Err(error) => {
                        info!(?error, "Block writer channel closed; exiting thread.");
                        return;
                    }
                },
            }
        }
    }
    /// Get the hash of the block at the provided height.
    pub fn get_block_hash(&self, block_height: NonZeroUsize) -> Option<HashOf<BlockHeader>> {
        if self.prune_recovery_is_required()
            || self.canonical_storage_poisoned.load(Ordering::Acquire)
        {
            return None;
        }
        #[cfg(test)]
        self.observe_canonical_read_after_prune_check_for_tests(CANONICAL_HASH_READER_OBSERVED);
        if self.emergency_fast_startup_enabled() {
            return self.get_durable_block_hash(block_height);
        }
        let hash_data_guard = self.block_data.lock();
        if self.prune_recovery_is_required() {
            return None;
        }
        let block_height = block_height.get();
        if hash_data_guard.len() < block_height {
            return None;
        }
        let block_index = block_height - 1;
        hash_data_guard.get(block_index).map(|(hash, _)| *hash)
    }
    /// Get the committed hash at the provided height from Kura's durable hash journal.
    pub fn get_durable_block_hash(
        &self,
        block_height: NonZeroUsize,
    ) -> Option<HashOf<BlockHeader>> {
        if self.prune_recovery_is_required()
            || self.canonical_storage_poisoned.load(Ordering::Acquire)
        {
            return None;
        }
        let height = u64::try_from(block_height.get()).ok()?;
        let mut block_store = self.block_store.lock();
        if self.prune_recovery_is_required() {
            return None;
        }
        let hash = Self::read_durable_hash_at_height(&mut block_store, height)
            .ok()
            .flatten();
        if self.prune_recovery_is_required() {
            return None;
        }
        hash
    }
    /// Resolve the height of the block with the given hash.
    ///
    /// Emergency Fast mode resolves only hashes learned after startup or by a
    /// height-based block read. It deliberately does not scan the complete
    /// durable hash journal to answer a reverse lookup.
    pub fn get_block_height_by_hash(&self, hash: HashOf<BlockHeader>) -> Option<NonZeroUsize> {
        if self.prune_recovery_is_required()
            || self.canonical_storage_poisoned.load(Ordering::Acquire)
        {
            return None;
        }
        let cached = self.block_height_index.lock().get(&hash).copied();
        if self.prune_recovery_is_required() {
            return None;
        }
        cached
    }
    /// Resolve block heights containing the given transaction entrypoint hash.
    ///
    /// Returns `None` when the in-memory index is known to be partial, so callers can fall back to
    /// scanning blocks without risking incomplete query results.
    pub fn get_block_heights_by_entrypoint_hash(
        &self,
        hash: HashOf<TransactionEntrypoint>,
    ) -> Option<BTreeSet<NonZeroUsize>> {
        if self.prune_recovery_is_required()
            || self.canonical_storage_poisoned.load(Ordering::Acquire)
        {
            return None;
        }
        let index = self.transaction_entrypoint_index.lock();
        if self.prune_recovery_is_required() {
            return None;
        }
        let heights = index.complete.then(|| {
            index
                .heights_by_entrypoint
                .get(&hash)
                .cloned()
                .unwrap_or_default()
        });
        if self.prune_recovery_is_required() {
            return None;
        }
        heights
    }
    /// Resolve block heights containing committed transactions with the given authority.
    ///
    /// Returns `None` when the in-memory transaction index is known to be partial.
    pub fn get_block_heights_by_transaction_authority(
        &self,
        authority: &AccountId,
    ) -> Option<BTreeSet<NonZeroUsize>> {
        if self.prune_recovery_is_required()
            || self.canonical_storage_poisoned.load(Ordering::Acquire)
        {
            return None;
        }
        let index = self.transaction_entrypoint_index.lock();
        if self.prune_recovery_is_required() {
            return None;
        }
        let heights = index.complete.then(|| {
            index
                .heights_by_authority
                .get(authority)
                .cloned()
                .unwrap_or_default()
        });
        if self.prune_recovery_is_required() {
            return None;
        }
        heights
    }
    /// Resolve block heights containing committed transactions with the given timestamp.
    ///
    /// Returns `None` when the in-memory transaction index is known to be partial.
    pub fn get_block_heights_by_transaction_timestamp_ms(
        &self,
        timestamp_ms: u64,
    ) -> Option<BTreeSet<NonZeroUsize>> {
        if self.prune_recovery_is_required()
            || self.canonical_storage_poisoned.load(Ordering::Acquire)
        {
            return None;
        }
        let index = self.transaction_entrypoint_index.lock();
        if self.prune_recovery_is_required() {
            return None;
        }
        let heights = index.complete.then(|| {
            index
                .heights_by_timestamp_ms
                .get(&timestamp_ms)
                .cloned()
                .unwrap_or_default()
        });
        if self.prune_recovery_is_required() {
            return None;
        }
        heights
    }
    /// Resolve block heights containing committed transactions in the timestamp range.
    ///
    /// Returns `None` when the in-memory transaction index is known to be partial.
    pub fn get_block_heights_by_transaction_timestamp_range(
        &self,
        lower_bound: Option<u64>,
        upper_bound: Option<u64>,
    ) -> Option<BTreeSet<NonZeroUsize>> {
        if self.prune_recovery_is_required()
            || self.canonical_storage_poisoned.load(Ordering::Acquire)
        {
            return None;
        }
        if let (Some(lower), Some(upper)) = (lower_bound, upper_bound)
            && lower > upper
        {
            return Some(BTreeSet::new());
        }
        let index = self.transaction_entrypoint_index.lock();
        if self.prune_recovery_is_required() {
            return None;
        }
        let heights = index.complete.then(|| {
            let lower = lower_bound.map_or(Bound::Unbounded, Bound::Included);
            let upper = upper_bound.map_or(Bound::Unbounded, Bound::Included);
            index
                .heights_by_timestamp_ms
                .range((lower, upper))
                .flat_map(|(_, heights)| heights.iter().copied())
                .collect()
        });
        if self.prune_recovery_is_required() {
            return None;
        }
        heights
    }
    /// Resolve block heights containing committed transactions with the given result status.
    ///
    /// Returns `None` when the in-memory transaction index is known to be partial.
    pub fn get_block_heights_by_transaction_result_status(
        &self,
        is_ok: bool,
    ) -> Option<BTreeSet<NonZeroUsize>> {
        if self.prune_recovery_is_required()
            || self.canonical_storage_poisoned.load(Ordering::Acquire)
        {
            return None;
        }
        let index = self.transaction_entrypoint_index.lock();
        if self.prune_recovery_is_required() {
            return None;
        }
        let heights = index.complete.then(|| {
            index
                .heights_by_result_status
                .get(&is_ok)
                .cloned()
                .unwrap_or_default()
        });
        if self.prune_recovery_is_required() {
            return None;
        }
        heights
    }

    /// Return a bounded chronological page of exact-schema Kaigi signal locators.
    ///
    /// The index stores only structural locations and transaction authorities.
    /// An exclusive `after` position must name an exact candidate for the same
    /// call. `anchor_height` excludes later appends. A partial index, pruning,
    /// or poisoned canonical storage returns `Unavailable`; callers must not
    /// fall back to a ledger scan.
    pub(crate) fn get_kaigi_signal_candidate_locators(
        &self,
        call_id: &KaigiId,
        anchor_height: usize,
        after: Option<KaigiSignalCandidatePosition>,
        limit: NonZeroUsize,
    ) -> core::result::Result<KaigiSignalCandidateLocatorPage, KaigiSignalCandidateIndexError> {
        if self.prune_recovery_is_required()
            || self.canonical_storage_poisoned.load(Ordering::Acquire)
        {
            return Err(KaigiSignalCandidateIndexError::Unavailable);
        }
        let index = self.transaction_entrypoint_index.lock();
        if self.prune_recovery_is_required()
            || self.canonical_storage_poisoned.load(Ordering::Acquire)
            || !index.complete
            || (anchor_height > 0
                && index
                    .indexed_heights
                    .last()
                    .is_none_or(|height| height.get() < anchor_height))
        {
            return Err(KaigiSignalCandidateIndexError::Unavailable);
        }
        let page = Self::collect_kaigi_signal_candidate_locators(
            &index,
            call_id,
            anchor_height,
            after,
            limit,
        )?;
        if self.prune_recovery_is_required()
            || self.canonical_storage_poisoned.load(Ordering::Acquire)
        {
            return Err(KaigiSignalCandidateIndexError::Unavailable);
        }
        Ok(page)
    }

    fn collect_kaigi_signal_candidate_locators(
        index: &TransactionEntrypointIndex,
        call_id: &KaigiId,
        anchor_height: usize,
        after: Option<KaigiSignalCandidatePosition>,
        limit: NonZeroUsize,
    ) -> core::result::Result<KaigiSignalCandidateLocatorPage, KaigiSignalCandidateIndexError> {
        let Some(by_height) = index.kaigi_signal_candidates.get(call_id) else {
            return if after.is_some() {
                Err(KaigiSignalCandidateIndexError::CursorMismatch)
            } else {
                Ok(KaigiSignalCandidateLocatorPage {
                    candidates: Vec::new(),
                    has_more: false,
                })
            };
        };
        let after_key = after.map(|position| position.network_input_index);
        let after_height = after
            .and_then(|position| usize::try_from(position.block_height).ok())
            .and_then(NonZeroUsize::new);
        if let Some(position) = after {
            let Some(height) = after_height else {
                return Err(KaigiSignalCandidateIndexError::CursorMismatch);
            };
            if height.get() > anchor_height {
                return Err(KaigiSignalCandidateIndexError::CursorMismatch);
            }
            let Some(locator) = by_height
                .get(&height)
                .and_then(|by_offset| by_offset.get(&after_key.expect("cursor key exists")))
            else {
                return Err(KaigiSignalCandidateIndexError::CursorMismatch);
            };
            if locator.position != position {
                return Err(KaigiSignalCandidateIndexError::CursorMismatch);
            }
        }
        let Some(anchor_height) = NonZeroUsize::new(anchor_height) else {
            return Ok(KaigiSignalCandidateLocatorPage {
                candidates: Vec::new(),
                has_more: false,
            });
        };
        let lower_height = after_height.map_or(Bound::Unbounded, Bound::Included);
        let mut candidates = Vec::new();
        candidates
            .try_reserve(limit.get())
            .map_err(|_| KaigiSignalCandidateIndexError::Unavailable)?;
        for (height, by_offset) in by_height.range((lower_height, Bound::Included(anchor_height))) {
            let lower_offset = if Some(*height) == after_height {
                Bound::Excluded(after_key.expect("cursor key exists"))
            } else {
                Bound::Unbounded
            };
            for locator in by_offset
                .range((lower_offset, Bound::Unbounded))
                .map(|(_, locator)| locator)
            {
                if candidates.len() == limit.get() {
                    return Ok(KaigiSignalCandidateLocatorPage {
                        candidates,
                        has_more: true,
                    });
                }
                candidates.push(locator.clone());
            }
        }
        Ok(KaigiSignalCandidateLocatorPage {
            candidates,
            has_more: false,
        })
    }
    /// Return the exact durable height and bounded encoded payload length for a
    /// known canonical block hash.
    ///
    /// This read is serialized with pruning and canonical-chain replacement. It
    /// accepts only the in-memory canonical hash-to-height binding backed by the
    /// exact durable commit-marker count, hash journal, block-index slot, and
    /// verified CommitQC execution commitment. Evicted bodies remain eligible
    /// because their index slot and retained v3 record must agree with the
    /// QC-authenticated wire length used to bound committee recovery requests.
    pub(crate) fn durable_block_payload_len_by_hash(
        &self,
        hash: HashOf<BlockHeader>,
    ) -> Result<Option<(u64, u64)>> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        self.ensure_canonical_storage_not_poisoned()?;
        let Some(height) = self.block_height_index.lock().get(&hash).copied() else {
            return Ok(None);
        };
        let height_u64 = u64::try_from(height.get())?;
        let mut store = self.block_store.lock();
        let durable_count = store.read_exact_durable_index_count()?;
        if height_u64 > durable_count {
            return Ok(None);
        }
        let durable_hash = Self::read_durable_hash_at_height(&mut store, height_u64)?
            .ok_or(Error::HashesFileHeightMismatch)?;
        if durable_hash != hash {
            return Err(Error::CanonicalBlockWireMismatch { height: height_u64 });
        }
        let index = store.read_block_index(height_u64 - 1)?;
        if index.length == 0 || index.length > STRICT_INIT_MAX_BLOCK_BYTES {
            return Err(Error::CorruptedBlockLength {
                length: index.length,
                limit: STRICT_INIT_MAX_BLOCK_BYTES,
            });
        }
        // This is size admission, so never follow retained-record validation into
        // get_block: a cold body must not allocate, decode or enter resident indexes
        // before its consumer admits the signed wire length. The existing replica
        // authority verifies the exact CommitQC/retained-record/header bindings
        // without reading the body; the bounded reader validates body bytes later.
        let wire_len = match self.verified_v2_finality_wire_hash_for_eviction(
            &store.path_to_blockchain,
            height_u64,
            hash,
        )? {
            Some((wire_len, _)) => wire_len,
            // A Sumeragi block has no v2 finality sidecar: its frame carries the commit
            // certificate the Sumeragi block store verified before writing it.
            // TODO(WP8c): Kura keeps no v2 finality.
            None => index.length,
        };
        if wire_len != index.length {
            return Err(Error::CanonicalBlockWireMismatch { height: height_u64 });
        }
        Ok(Some((height_u64, wire_len)))
    }
    /// Get a reference to block by height, loading it from disk if needed.
    pub fn get_block(&self, block_height: NonZeroUsize) -> Option<Arc<SignedBlock>> {
        self.get_block_inner(block_height, true)
    }
    /// Load a body without updating hash, height, Network query, or body caches.
    ///
    /// This historical storage helper grants no executed-wire query authority.
    /// Canonical queries use the exact finalized body reader with a precharged
    /// wire bound. No merge sidecar participates in Network indexing.
    pub(crate) fn get_block_without_merge_sidecar(
        &self,
        block_height: NonZeroUsize,
    ) -> Option<Arc<SignedBlock>> {
        self.get_block_inner(block_height, false)
    }
    fn poison_corrupt_canonical_read(&self, block_index: usize, reason: &'static str) {
        let path = self.active_blocks_dir.lock().clone();
        let error = Error::IO(std::io::Error::new(ErrorKind::InvalidData, reason), path);
        error!(
            block_index,
            reason, "Canonical Kura block read failed validation"
        );
        self.poison_canonical_storage("canonical block read validation", &error);
    }
    fn get_block_inner(
        &self,
        block_height: NonZeroUsize,
        update_transaction_index: bool,
    ) -> Option<Arc<SignedBlock>> {
        if self.prune_recovery_is_required()
            || self.canonical_storage_poisoned.load(Ordering::Acquire)
        {
            if self.canonical_storage_poisoned.load(Ordering::Relaxed) {
                error!(
                    height = block_height.get(),
                    "refusing canonical block read while Kura storage is fail-stop poisoned"
                );
            }
            return None;
        }
        #[cfg(test)]
        self.observe_canonical_read_after_prune_check_for_tests(CANONICAL_BLOCK_READER_OBSERVED);
        let (block_index, known_hash, known_previous_hash, cached_block, should_cache, chain_len) = {
            let data = self.block_data.lock();
            if self.prune_recovery_is_required() {
                return None;
            }
            if data.len() < block_height.get() {
                return None;
            }
            let idx = block_height.get() - 1;
            if self.is_hard_fork_hash_only_block(idx) {
                debug!(
                    block_index = idx,
                    "hard-fork snapshot bootstrap: hash-only block body is unavailable"
                );
                return None;
            }
            let known_hash = data.known_hash(idx);
            let known_previous_hash = idx
                .checked_sub(1)
                .and_then(|previous| data.known_hash(previous));
            let cached_block = data.cached_body(idx);
            let should_cache = idx + self.blocks_in_memory.get() >= data.len();
            (
                idx,
                known_hash,
                known_previous_hash,
                cached_block,
                should_cache,
                data.len(),
            )
        };
        let expected_hash = known_hash.or_else(|| self.get_durable_block_hash(block_height))?;
        let expected_previous_hash = if block_index == 0 {
            None
        } else {
            let previous_height = NonZeroUsize::new(block_index)
                .expect("a non-genesis block has a non-zero parent height");
            Some(known_previous_hash.or_else(|| self.get_durable_block_hash(previous_height))?)
        };
        if should_cache {
            let mut data = self.block_data.lock();
            if data.len() != chain_len || self.prune_recovery_is_required() {
                return None;
            }
            if update_transaction_index {
                data.cache_hash(block_index, expected_hash);
                if let (Some(previous_index), Some(previous_hash)) =
                    (block_index.checked_sub(1), expected_previous_hash)
                {
                    data.cache_hash(previous_index, previous_hash);
                }
                self.set_block_height_index_entry(block_height.get(), expected_hash);
            }
        }
        let (block, is_evicted, authenticated_for_index) = {
            let mut block_store = self.block_store.lock();
            if self.prune_recovery_is_required() {
                return None;
            }
            let index = match block_store.read_block_index(block_index as u64) {
                Ok(index) => index,
                Err(error) => {
                    error!(?error, block_index, "Failed to read block index from disk");
                    return None;
                }
            };
            let is_evicted = index.is_evicted();
            let BlockIndex { start, length } = index;
            if length > STRICT_INIT_MAX_BLOCK_BYTES {
                drop(block_store);
                self.poison_corrupt_canonical_read(
                    block_index,
                    "committed block length exceeds the canonical wire limit",
                );
                return None;
            }
            if length == 0 {
                debug!(
                    block_index,
                    height = block_index.saturating_add(1),
                    evicted = is_evicted,
                    "Kura block body is unavailable for hash-only metadata"
                );
                return None;
            }
            if let Some(telemetry) = self.telemetry.get() {
                let outcome = if is_evicted { "miss" } else { "hit" };
                telemetry.inc_storage_da_cache("kura", outcome);
            }
            let mut authenticated_for_index = is_evicted;
            let loaded = if is_evicted {
                let height = block_index.saturating_add(1) as u64;
                let (finality_wire_len, finality_wire_hash) = match self
                    .verified_v2_finality_wire_hash_for_eviction(
                        &block_store.path_to_blockchain,
                        height,
                        expected_hash,
                    ) {
                    Ok(Some(hash)) => hash,
                    Ok(None) => {
                        error!(
                            block_index,
                            height, "Evicted block has no signed complete-wire finality"
                        );
                        return None;
                    }
                    Err(error) => {
                        error!(
                            ?error,
                            block_index,
                            height,
                            "Failed to authenticate signed complete-wire finality"
                        );
                        return None;
                    }
                };
                let bytes = match block_store.read_optional_da_cache(height) {
                    Ok(Some(bytes)) => bytes,
                    Ok(None) => return None,
                    Err(error) => {
                        error!(
                            ?error,
                            block_index, height, "Failed to read evicted block cache"
                        );
                        return None;
                    }
                };
                if finality_wire_len != length
                    || u64::try_from(bytes.len()).ok() != Some(length)
                    || Hash::new(&bytes) != finality_wire_hash
                {
                    error!(
                        block_index,
                        height, "Evicted block cache differs from its signed complete-wire binding"
                    );
                    return None;
                }
                if let Some(telemetry) = self.telemetry.get() {
                    let actual_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
                    telemetry.add_storage_da_churn_bytes("kura", "rehydrated", actual_len);
                }
                let decoded = match decode_framed_signed_block(&bytes) {
                    Ok(decoded) => decoded,
                    Err(error) => {
                        if self.is_hard_fork_hash_only_block(block_index) {
                            debug!(
                                ?error,
                                block_index,
                                height,
                                "hard-fork snapshot bootstrap: audited block body is unavailable"
                            );
                            return None;
                        }
                        error!(
                            ?error,
                            block_index, height, "Failed to decode evicted block payload"
                        );
                        return None;
                    }
                };
                decoded
            } else {
                if let Some(block) = cached_block {
                    return Some(block);
                }
                // A decoded header alone cannot restore query membership. Authenticate
                // the exact wire before promoting an uncached body into the derived
                // index or cache. Live append-owned cached bodies retain their custody.
                let published_wire = if update_transaction_index {
                    match self.verified_v2_finality_wire_hash_for_eviction(
                        &block_store.path_to_blockchain,
                        block_height.get() as u64,
                        expected_hash,
                    ) {
                        Ok(binding) => binding,
                        Err(error) => {
                            debug!(?error, block_index, "Not promoting an unauthenticated body");
                            None
                        }
                    }
                } else {
                    None
                };
                let bytes = match block_store.block_bytes(start, length) {
                    Ok(slice) => slice,
                    Err(error) => {
                        error!(?error, block_index, "Failed to borrow block data slice");
                        drop(block_store);
                        self.poison_corrupt_canonical_read(
                            block_index,
                            "committed inline block range is unreadable",
                        );
                        return None;
                    }
                };
                authenticated_for_index = published_wire.is_some_and(|(wire_len, wire_hash)| {
                    wire_len == length && Hash::new(bytes) == wire_hash
                });
                match decode_framed_signed_block(bytes) {
                    Ok(decoded) => decoded,
                    Err(error) => {
                        if self.is_hard_fork_hash_only_block(block_index) {
                            debug!(
                                ?error,
                                block_index,
                                "hard-fork snapshot bootstrap: audited block body is unavailable"
                            );
                            return None;
                        }
                        error!(?error, block_index, "Failed to decode block from disk");
                        drop(block_store);
                        self.poison_corrupt_canonical_read(
                            block_index,
                            "committed inline block body is not decodable",
                        );
                        return None;
                    }
                }
            };
            (loaded, is_evicted, authenticated_for_index)
        };
        if self.prune_recovery_is_required() {
            return None;
        }
        if block.hash() != expected_hash {
            error!(
                expected = ?expected_hash,
                actual = ?block.hash(),
                block_index,
                "Loaded block hash mismatched the index entry"
            );
            self.poison_corrupt_canonical_read(
                block_index,
                "loaded block hash mismatches the canonical hash journal",
            );
            return None;
        }
        let header = block.header();
        let expected_height = u64::try_from(block_height.get()).ok()?;
        if header.height().get() != expected_height
            || header.prev_block_hash() != expected_previous_hash
        {
            error!(
                expected_height = block_height.get(),
                actual_height = header.height().get(),
                expected_previous_hash = ?expected_previous_hash,
                actual_previous_hash = ?header.prev_block_hash(),
                block_index,
                "Loaded block lineage mismatched the canonical journal"
            );
            self.poison_corrupt_canonical_read(
                block_index,
                "loaded block height or parent mismatches the canonical journal",
            );
            return None;
        }
        if self.prune_recovery_is_required() {
            return None;
        }
        let block_arc = Arc::new(block);
        if update_transaction_index {
            let height = NonZeroUsize::new(block_index.saturating_add(1))
                .expect("canonical block index produces a non-zero height");
            let already_indexed = self
                .transaction_entrypoint_index
                .lock()
                .indexed_heights
                .contains(&height);
            if !authenticated_for_index {
                self.mark_transaction_entrypoint_index_incomplete(height.get(), chain_len);
            } else if !already_indexed {
                // Index only the complete Network projection of this canonical body.
                // A retired merge carrier or invalid result remains incomplete.
                self.set_transaction_entrypoint_index_entry(
                    height.get(),
                    block_arc.as_ref(),
                    chain_len,
                );
            }
        }
        if should_cache && update_transaction_index && !is_evicted && authenticated_for_index {
            #[cfg(test)]
            self.maybe_pause_block_read_before_cache_recheck_for_tests();
            // Cache publication is opportunistic, but it must be ordered after the durable index
            // observation. Eviction publishes its new index and clears this slot while retaining
            // `block_store`; re-read both durable identity fields before acquiring the cache so a
            // reader that decoded the old inline body cannot reinsert it after that publication.
            // Some canonical writers already own `block_data` before taking `block_store`, so use
            // a non-blocking cache acquisition while the store guard is held to avoid introducing
            // a reverse-order deadlock. A missed cache fill is harmless.
            let mut block_store = self.block_store.lock();
            let current_index = block_store.read_block_index(block_index as u64);
            let current_hash = block_store
                .read_block_hashes(block_index as u64, 1)
                .map(|hashes| hashes.first().copied());
            match (current_index, current_hash) {
                (Ok(index), Ok(Some(hash)))
                    if !index.is_evicted() && index.length > 0 && hash == expected_hash =>
                {
                    if let Some(mut data) = self.block_data.try_lock() {
                        let _ = data.cache_body_if_hash(
                            block_index,
                            expected_hash,
                            Arc::clone(&block_arc),
                        );
                    }
                }
                (Err(error), _) | (_, Err(error)) => {
                    debug!(
                        ?error,
                        block_index, "Skipping block cache fill after durable recheck failed"
                    );
                }
                _ => {}
            }
            drop(block_store);
        }
        if self.prune_recovery_is_required() {
            return None;
        }
        Some(block_arc)
    }
    fn is_hard_fork_hash_only_block(&self, block_index: usize) -> bool {
        block_index < self.hard_fork_hash_only_block_count.load(Ordering::Relaxed)
    }
    /// Return the provisional imported-prefix boundary and lineage digest.
    ///
    /// This is classification metadata only. Presence does not authenticate
    /// either the Kura hashes or the snapshot lineage.
    pub(crate) fn provisional_snapshot_bootstrap_metadata(&self) -> Option<(usize, Option<Hash>)> {
        self.provisional_snapshot_bootstrap
            .lock()
            .pending_metadata()
            .map(|pending| {
                (
                    pending.hash_only_prefix_height,
                    pending.bootstrap_lineage_hash,
                )
            })
    }
    /// Return whether Kura is open only for provisional signed-snapshot authentication.
    #[must_use]
    pub fn provisional_snapshot_bootstrap_pending(&self) -> bool {
        !self
            .provisional_snapshot_bootstrap
            .lock()
            .is_authenticated()
    }
    /// Return whether this height belongs to the imported snapshot prefix.
    ///
    /// The result deliberately includes a provisional prefix so startup
    /// planning can classify unavailable bodies before authentication. No
    /// mutation or output may use that classification until finalization.
    pub(crate) fn is_audited_snapshot_import_height(&self, block_height: NonZeroUsize) -> bool {
        self.is_hard_fork_hash_only_block(block_height.get().saturating_sub(1))
    }
    fn ensure_snapshot_bootstrap_authenticated(&self) -> Result<()> {
        if !self
            .provisional_snapshot_bootstrap
            .lock()
            .is_authenticated()
        {
            return Err(Error::SnapshotBootstrapAuthenticationPending);
        }
        Ok(())
    }
    /// Authorize a durable sidecar or journal mutation which does not require
    /// canonical block-stage recovery.
    fn durable_mutation_authorized(&self) -> Result<()> {
        if self.emergency_fast_startup_enabled() {
            return Err(Error::EmergencyFastAuxiliaryUnavailable {
                subsystem: "canonical mutation",
            });
        }
        self.ensure_snapshot_bootstrap_authenticated()?;
        self.ensure_canonical_storage_not_poisoned()
    }
    /// Returns `true` when the canonical block is represented only by its
    /// hash from a hard-fork snapshot bootstrap and the local body is
    /// intentionally unavailable.
    pub(crate) fn is_hash_only_block_height(&self, block_height: NonZeroUsize) -> bool {
        if self.prune_recovery_is_required() {
            return false;
        }
        let idx = block_height.get().saturating_sub(1);
        if self.is_hard_fork_hash_only_block(idx) {
            return true;
        }
        let data = self.block_data.lock();
        if self.prune_recovery_is_required() {
            return false;
        }
        if data.len() <= idx || data.cached_body(idx).is_some() {
            return false;
        }
        drop(data);
        let mut store = self.block_store.lock();
        if self.prune_recovery_is_required() {
            return false;
        }
        let is_hash_only = matches!(
            store.read_block_index(idx as u64),
            Ok(index) if index.length == 0
        );
        !self.prune_recovery_is_required() && is_hash_only
    }
    /// Force a stored block height into hash-only form when constructing snapshot tests.
    #[doc(hidden)]
    #[cfg(any(test, feature = "iroha-core-tests"))]
    #[allow(dead_code)]
    pub fn force_hash_only_block_for_testing(&self, block_height: NonZeroUsize) -> Result<()> {
        let idx = block_height.get().saturating_sub(1);
        let (block_hash, block_count) = {
            let mut data = self.block_data.lock();
            let block_count = data.len();
            let Some((block_hash, block_body)) = data.get_mut(idx) else {
                return Err(Error::OutOfBoundsBlockRead {
                    start_block_height: u64::try_from(block_height.get())?,
                    block_count,
                });
            };
            *block_body = None;
            (*block_hash, block_count)
        };
        let _write_guard = self.block_store_write_lock.lock();
        let mut store = self.block_store.lock();
        store.create_files_if_they_do_not_exist()?;
        store.write_block_hash(u64::try_from(idx)?, block_hash)?;
        store.write_block_index(u64::try_from(idx)?, EVICTED_BLOCK_START, 0)?;
        store.publish_commit_marker(u64::try_from(block_count)?)?;
        // This fixture evicts exactly one body. A hard-fork prefix would also
        // hide unrelated earlier bodies that are still durably available.
        Ok(())
    }
    pub(crate) fn hash_only_unavailable_prefix_len(&self, limit: usize) -> usize {
        if self.prune_recovery_is_required() {
            return 0;
        }
        let hash_only_count = self.hard_fork_hash_only_block_count.load(Ordering::Relaxed);
        if hash_only_count == 0 || limit == 0 {
            return 0;
        }
        hash_only_count.min(limit).min(self.block_data.lock().len())
    }
}
include!("kura/retained_finality_replica_authority.rs");
include!("kura/durable_block_and_atomic_sidecar_io.rs");
include!("kura/prune_intent_publication.rs");
impl Kura {
    fn block_store_tracked_bytes(block_store: &mut BlockStore) -> Result<u64> {
        if block_store.path_to_blockchain.as_os_str().is_empty() {
            return Ok(0);
        }
        let data_len = block_store.data_file_len()?;
        let index_len = block_store.index_file_len()?;
        let hashes_len = block_store.hashes_file_len()?;
        let marker_path = block_store.commit_marker_path();
        let marker_len = Self::file_len_or_zero(&marker_path)?;
        let marker_tmp_len = Self::file_len_or_zero(&marker_path.with_extension("norito.tmp"))?;
        let data_tmp_len = Self::file_len_or_zero(
            &block_store
                .path_to_blockchain
                .join(format!("{DATA_FILE_NAME}.tmp")),
        )?;
        let index_tmp_len = Self::file_len_or_zero(
            &block_store
                .path_to_blockchain
                .join(format!("{INDEX_FILE_NAME}.tmp")),
        )?;
        let hashes_tmp_len = Self::file_len_or_zero(
            &block_store
                .path_to_blockchain
                .join(format!("{HASHES_FILE_NAME}.tmp")),
        )?;
        Ok(data_len
            .saturating_add(index_len)
            .saturating_add(hashes_len)
            .saturating_add(marker_len)
            .saturating_add(marker_tmp_len)
            .saturating_add(data_tmp_len)
            .saturating_add(index_tmp_len)
            .saturating_add(hashes_tmp_len))
    }
    fn sidecar_tracked_bytes(data_path: &Path, index_path: &Path) -> Result<u64> {
        let data_tmp = data_path.with_extension("norito.tmp");
        let index_tmp = index_path.with_extension("index.tmp");
        Ok(Self::file_len_or_zero(data_path)?
            .saturating_add(Self::file_len_or_zero(index_path)?)
            .saturating_add(Self::file_len_or_zero(&data_tmp)?)
            .saturating_add(Self::file_len_or_zero(&index_tmp)?))
    }
    fn block_required_bytes(block: &SignedBlock) -> Result<u64> {
        let wire = block.canonical_wire()?;
        let frame = wire.into_vec();
        let frame_len = u64::try_from(frame.len())?;
        Ok(frame_len
            .saturating_add(BlockIndex::SIZE)
            .saturating_add(SIZE_OF_BLOCK_HASH))
    }
    fn block_required_bytes_for_budget(
        &self,
        block: &SignedBlock,
        merge_entry: Option<&MergeLedgerEntry>,
        _limit: u64,
    ) -> Result<u64> {
        let required = Self::block_required_bytes(block)?;
        Ok(required
            .saturating_add(self.lane_artifact_required_bytes_for_block(block, merge_entry)?))
    }
    fn blocks_root_bytes(root: &Path, historical_byte_limit: u64) -> Result<u64> {
        Self::blocks_root_usage_bytes(root, historical_byte_limit).map(|(enforced, _)| enforced)
    }
    /// Stream a fixed-depth accounting namespace without following links or
    /// retaining a descriptor for every instance. This observes physical bytes;
    /// it never installs an active route or grants historical write authority.
    fn visit_disk_accounting_directory(
        directory: &Path,
        mut visit: impl FnMut(&Path, &SecureMetadata) -> Result<()>,
    ) -> Result<()> {
        let before = match secure_file_metadata::from_path(directory) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(Error::IO(err, directory.to_path_buf())),
        };
        if before.file_type().is_symlink() || !before.is_dir() {
            return Err(Self::invalid_lane_artifact_error(
                directory.to_path_buf(),
                "Kura accounting namespace is not a direct directory",
            ));
        }
        for (index, entry) in std::fs::read_dir(directory)
            .map_err(|err| Error::IO(err, directory.to_path_buf()))?
            .enumerate()
        {
            // Same ceiling as the complete retained physical inventory. The
            // nesting below blocks/merge_ledger is exactly one instance level.
            if index >= 4_000_000 {
                return Err(Self::invalid_lane_artifact_error(
                    directory.to_path_buf(),
                    "Kura accounting namespace exceeds its entry bound",
                ));
            }
            let entry = entry.map_err(|err| Error::IO(err, directory.to_path_buf()))?;
            let path = entry.path();
            let metadata = secure_file_metadata::from_path(&path)
                .map_err(|err| Error::IO(err, path.clone()))?;
            if metadata.file_type().is_symlink()
                || !(metadata.is_dir()
                    || (metadata.is_file() && Self::sidecar_is_single_link(&metadata)))
            {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "Kura accounting namespace contains a linked or non-regular entry",
                ));
            }
            visit(&path, &metadata)?;
        }
        let after = secure_file_metadata::from_path(directory)
            .map_err(|err| Error::IO(err, directory.to_path_buf()))?;
        if after.file_type().is_symlink()
            || !after.is_dir()
            || !Self::sidecar_directory_metadata_unchanged(&before, &after)
        {
            return Err(Self::invalid_lane_artifact_error(
                directory.to_path_buf(),
                "Kura accounting namespace changed during observation",
            ));
        }
        Ok(())
    }
    fn is_immutable_instance_accounting_key(name: &str) -> bool {
        name.len() == 64
            && name
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }
    fn dir_file_bytes(dir: &Path) -> Result<u64> {
        if dir.as_os_str().is_empty() {
            return Ok(0);
        }
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(0),
            Err(err) => return Err(Error::IO(err, dir.to_path_buf())),
        };
        let mut total = 0u64;
        for entry in entries {
            let entry = entry.map_err(|err| Error::IO(err, dir.to_path_buf()))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|err| Error::IO(err, path.clone()))?;
            if file_type.is_file() {
                let len = entry
                    .metadata()
                    .map_err(|err| Error::IO(err, path.clone()))?
                    .len();
                total = total.saturating_add(len);
            }
        }
        Ok(total)
    }
    fn blocks_root_usage_bytes(root: &Path, historical_byte_limit: u64) -> Result<(u64, u64)> {
        if root.as_os_str().is_empty() {
            return Ok((0, 0));
        }
        let debug_bytes = Self::blocks_root_debug_file_bytes(root)?;
        let mut enforced = debug_bytes;
        let mut total = debug_bytes;
        let mut historical_budget =
            HistoricalAutonomousRecoveryAccountingBudget::new(historical_byte_limit);
        let mut count_store = |path: &Path| -> Result<()> {
            let budgeted =
                Self::block_store_bytes_with_historical_budget(path, &mut historical_budget)?;
            enforced = enforced.saturating_add(budgeted);
            total = total
                .saturating_add(budgeted)
                .saturating_add(Self::block_store_supplemental_bytes(path)?);
            Ok(())
        };
        Self::visit_disk_accounting_directory(root, |path, metadata| {
            if path.file_name() == Some(std::ffi::OsStr::new("instances")) {
                Self::visit_disk_accounting_directory(path, |instance, metadata| {
                    if !metadata.is_dir()
                        || !instance
                            .file_name()
                            .and_then(std::ffi::OsStr::to_str)
                            .is_some_and(Self::is_immutable_instance_accounting_key)
                    {
                        return Err(Self::invalid_lane_artifact_error(
                            instance.to_path_buf(),
                            "Kura instance accounting contains an unknown physical entry",
                        ));
                    }
                    count_store(instance)
                })?;
            } else if metadata.is_dir() {
                // Count canonical custody and disposable retained stores. This
                // physical enumeration does not resolve an alias into a lane.
                count_store(path)?;
            }
            Ok(())
        })?;
        Ok((enforced, total))
    }
    /// Count regular files below `root` without following symbolic links.
    ///
    /// Lane-geometry archives are deliberately opaque to the legacy block and
    /// merge-log accounting helpers: a transition archive contains both trees,
    /// nested below a transition identifier.  They are recovery evidence and
    /// must still count against the Kura budget while they are retained.
    fn directory_tree_file_bytes(root: &Path) -> Result<u64> {
        if root.as_os_str().is_empty() {
            return Ok(0);
        }
        let root_metadata = match std::fs::symlink_metadata(root) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(0),
            Err(err) => return Err(Error::IO(err, root.to_path_buf())),
        };
        if root_metadata.file_type().is_symlink() || !root_metadata.file_type().is_dir() {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "Kura retained geometry root must be a non-symlink directory",
                ),
                root.to_path_buf(),
            ));
        }
        let mut total = 0u64;
        let mut seen = 0_usize;
        let mut pending = vec![(root.to_path_buf(), 0_usize)];
        while let Some((directory, depth)) = pending.pop() {
            if depth > 128 {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "Kura retained geometry tree exceeds the maximum directory depth",
                    ),
                    directory,
                ));
            }
            let entries =
                std::fs::read_dir(&directory).map_err(|err| Error::IO(err, directory.clone()))?;
            for entry in entries {
                let entry = entry.map_err(|err| Error::IO(err, directory.clone()))?;
                seen = seen.saturating_add(1);
                if seen > 4_000_000 {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "Kura retained geometry tree exceeds the maximum entry count",
                        ),
                        directory,
                    ));
                }
                let path = entry.path();
                let file_type = entry
                    .file_type()
                    .map_err(|err| Error::IO(err, path.clone()))?;
                if file_type.is_symlink() {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "Kura retained geometry tree contains a symbolic link",
                        ),
                        path,
                    ));
                }
                if file_type.is_file() {
                    total = total.saturating_add(
                        entry
                            .metadata()
                            .map_err(|err| Error::IO(err, path.clone()))?
                            .len(),
                    );
                } else if file_type.is_dir() {
                    pending.push((path, depth.saturating_add(1)));
                } else {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "Kura retained geometry tree contains a non-regular entry",
                        ),
                        path,
                    ));
                }
            }
        }
        Ok(total)
    }
    fn kura_shared_disk_usage_bytes(&self) -> Result<u64> {
        if self.store_root.as_os_str().is_empty() {
            return Ok(0);
        }
        let merge_root = self.store_root.join("merge_ledger");
        let retired_root = self.store_root.join("retired");
        let retired_merge_root = retired_root.join("merge_ledger");
        let retired_geometry_root = retired_root.join("lane_geometry");
        let mut used = 0u64;
        used = used.saturating_add(Self::merge_root_bytes(&merge_root)?);
        used = used.saturating_add(Self::file_len_or_zero(
            &self.store_root.join(membership_storage::SEGMENT_NAME),
        )?);
        used = used.saturating_add(Self::merge_root_bytes(&retired_merge_root)?);
        used = used.saturating_add(Self::directory_tree_file_bytes(&retired_geometry_root)?);
        used = used.saturating_add(Self::directory_tree_file_bytes(
            &self.store_root.join(MERGE_CARRIERS_DIR),
        )?);
        used = used.saturating_add(Self::directory_tree_file_bytes(
            &self.store_root.join(PENDING_MERGE_ENTRIES_DIR),
        )?);
        used = used.saturating_add(Self::directory_tree_file_bytes(
            &self.store_root.join(NATIVE_AMX_PUBLICATION_INDEX_DIRECTORY),
        )?);
        used = used.saturating_add(Self::directory_tree_file_bytes(
            &self.store_root.join(PENDING_QUEUE_PLAN_ADMISSIONS_DIR),
        )?);
        used = used.saturating_add(Self::directory_tree_file_bytes(
            &self.store_root.join(fastpq_artifact_store::DIRECTORY),
        )?);
        for journal_name in [
            crate::query::index_status::QueryIndexJournal::JOURNAL_FILE,
            crate::query::projection_checkpoint_journal::QueryProjectionCheckpointJournal::JOURNAL_FILE,
        ] {
            let path = self.store_root.join(journal_name);
            used = used.saturating_add(Self::file_len_or_zero(&path)?);
            used = used.saturating_add(Self::file_len_or_zero(
                &path.with_extension("norito.tmp"),
            )?);
        }
        used = used.saturating_add(Self::file_len_or_zero(&self.lane_geometry_journal_path())?);
        used = used.saturating_add(Self::file_len_or_zero(
            &self
                .lane_geometry_journal_path()
                .with_extension("norito.tmp"),
        )?);
        used = used.saturating_add(
            Self::canonical_prune_intent_artifact_inventory(&self.store_root)?.tracked_bytes()?,
        );
        used = used.saturating_add(Self::file_len_or_zero(
            &Self::autonomous_lifecycle_process_generation_path_for(&self.store_root),
        )?);
        used = used.saturating_add(Self::file_len_or_zero(
            &Self::autonomous_lifecycle_process_generation_temp_path_for(&self.store_root),
        )?);
        used = used.saturating_add(
            Self::autonomous_lifecycle_process_generation_publication_residue_bytes(
                &self.store_root,
            )?,
        );
        Ok(used)
    }
    fn kura_disk_usage_bytes(&self) -> Result<u64> {
        if self.store_root.as_os_str().is_empty() {
            return Ok(0);
        }
        let blocks_root = self.store_root.join("blocks");
        let retired_blocks_root = self.store_root.join("retired").join("blocks");
        let historical_byte_limit = self.historical_autonomous_recovery_aggregate_byte_limit();
        let active = Self::blocks_root_bytes(&blocks_root, historical_byte_limit)?;
        let retired = Self::blocks_root_bytes(&retired_blocks_root, historical_byte_limit)?;
        Ok(active
            .saturating_add(retired)
            .saturating_add(self.kura_shared_disk_usage_bytes()?))
    }
    fn kura_disk_usage_bytes_with_total(&self) -> Result<(u64, u64)> {
        if self.store_root.as_os_str().is_empty() {
            return Ok((0, 0));
        }
        let blocks_root = self.store_root.join("blocks");
        let retired_blocks_root = self.store_root.join("retired").join("blocks");
        let historical_byte_limit = self.historical_autonomous_recovery_aggregate_byte_limit();
        let (active_enforced, active_total) =
            Self::blocks_root_usage_bytes(&blocks_root, historical_byte_limit)?;
        let (retired_enforced, retired_total) =
            Self::blocks_root_usage_bytes(&retired_blocks_root, historical_byte_limit)?;
        let shared = self.kura_shared_disk_usage_bytes()?;
        Ok((
            active_enforced
                .saturating_add(retired_enforced)
                .saturating_add(shared),
            active_total
                .saturating_add(retired_total)
                .saturating_add(shared),
        ))
    }
    fn kura_total_disk_usage_bytes(&self) -> Result<u64> {
        self.kura_disk_usage_bytes_with_total()
            .map(|(_, total)| total)
    }
    fn pending_block_bytes_raw<E: From<Error>>(
        &self,
        persisted_count: usize,
        mut resolve_merge: impl FnMut(HashOf<MergeLedgerEntry>) -> Result<Option<MergeLedgerEntry>, E>,
    ) -> Result<u64, E> {
        if self.emergency_fast_startup_enabled() {
            return Err(Error::EmergencyFastAuxiliaryUnavailable {
                subsystem: "pending canonical block cache",
            }
            .into());
        }
        #[cfg(test)]
        self.pending_budget_raw_scans
            .fetch_add(1, Ordering::Relaxed);
        let pending_blocks = {
            let data = self.block_data.lock();
            let start = persisted_count.min(data.len());
            let mut blocks = Vec::with_capacity(data.len().saturating_sub(start));
            let entries = data
                .dense_entries()
                .ok_or(Error::EmergencyFastAuxiliaryUnavailable {
                    subsystem: "pending canonical block cache",
                })?;
            for (_, block) in entries.iter().skip(start) {
                let block = block
                    .as_ref()
                    .expect("pending block missing from Kura memory cache");
                blocks.push(Arc::clone(block));
            }
            blocks
        };
        let mut pending_bytes = 0u64;
        for block in pending_blocks {
            let merge_entry = if let Some(reference) = Self::block_merge_reference(&block) {
                Some(resolve_merge(reference.entry_hash)?.ok_or(
                    Error::MissingCertifiedMergeSidecar {
                        entry_hash: reference.entry_hash,
                    },
                )?)
            } else {
                None
            };
            pending_bytes = pending_bytes.saturating_add(self.block_required_bytes_for_budget(
                &block,
                merge_entry.as_ref(),
                self.max_disk_usage_bytes,
            )?);
        }
        Ok(pending_bytes)
    }
    fn pending_block_bytes(&self, persisted_count: usize, unindexed_bytes: u64) -> Result<u64> {
        self.pending_block_bytes_with_merge_resolver(persisted_count, unindexed_bytes, |hash| {
            self.merge_entry_by_hash(hash)
        })
    }
    fn persisted_count_and_unindexed_bytes_raw(&self) -> Result<(usize, u64)> {
        #[cfg(test)]
        self.durable_budget_metadata_reads
            .fetch_add(1, Ordering::Relaxed);
        let mut block_store = self.block_store.lock();
        let persisted = usize::try_from(block_store.read_durable_index_count()?)?;
        let persisted_u64 = persisted as u64;
        let indexed_data_len = if persisted == 0 {
            0
        } else {
            let last = block_store.read_block_index(persisted as u64 - 1)?;
            last.start.saturating_add(last.length)
        };
        let data_file_len = block_store.data_file_len()?;
        let index_file_len = block_store.index_file_len()?;
        let hashes_file_len = block_store.hashes_file_len()?;
        let indexed_index_len = persisted_u64.saturating_mul(BlockIndex::SIZE);
        let indexed_hash_len = persisted_u64.saturating_mul(SIZE_OF_BLOCK_HASH);
        let unindexed_bytes = data_file_len
            .saturating_sub(indexed_data_len)
            .saturating_add(index_file_len.saturating_sub(indexed_index_len))
            .saturating_add(hashes_file_len.saturating_sub(indexed_hash_len));
        Ok((persisted, unindexed_bytes))
    }
    fn persisted_count_and_unindexed_bytes(&self) -> Result<(usize, u64)> {
        if let Some(snapshot) = self.durable_budget_snapshot() {
            return Ok(snapshot);
        }
        let (persisted_count, unindexed_bytes) = self.persisted_count_and_unindexed_bytes_raw()?;
        self.publish_durable_budget_snapshot(persisted_count, unindexed_bytes);
        Ok((persisted_count, unindexed_bytes))
    }
    fn check_storage_budget(
        &self,
        block: &SignedBlock,
        merge_entry: Option<&MergeLedgerEntry>,
    ) -> Result<()> {
        let prepend_extra = self
            .post_wsv_prepend_admission_extra_under_prune_and_canonical_guards(
                block,
                merge_entry,
            )?;
        if self.max_disk_usage_bytes == 0 || self.store_root.as_os_str().is_empty() {
            return Ok(());
        }
        self.ensure_disk_usage_initialized()?;
        let merge_entry_bytes = match merge_entry {
            Some(entry) => self.merge_commit_required_bytes(block, entry)?,
            None => 0,
        };
        let (persisted_count, unindexed_bytes) = self.persisted_count_and_unindexed_bytes()?;
        let limit = self.max_disk_usage_bytes;
        let block_required = self
            .block_required_bytes_for_budget(block, merge_entry, limit)?
            .checked_add(prepend_extra)
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "post-WSV prepend admission accounting overflowed",
                )
            })?;
        let association_stage_bytes =
            self.canonical_association_stage_additional_bytes(block, merge_entry)?;
        let lane_publication_reservations = self.all_publication_budget_reserved_bytes()?;
        let certified_bundle_reservations = self.certified_bundle_capacity_reserved_bytes()?;
        let autonomous_terminal_reservations =
            self.autonomous_global_terminal_outcome_reserved_bytes()?;
        let prune_maintenance_headroom = Self::canonical_prune_intent_maintenance_headroom_bytes();
        let mut used = self.disk_usage.load(Ordering::Relaxed);
        let pending_bytes = self.pending_block_bytes(persisted_count, unindexed_bytes)?;
        let mut budget_used = used
            .saturating_add(pending_bytes)
            .saturating_add(lane_publication_reservations)
            .saturating_add(certified_bundle_reservations)
            .saturating_add(autonomous_terminal_reservations)
            .saturating_add(prune_maintenance_headroom);
        let mut required = budget_used
            .saturating_add(block_required)
            .saturating_add(merge_entry_bytes)
            .saturating_add(association_stage_bytes);
        if required > limit {
            if self.purge_retired_storage_under_prune_and_canonical_guards()? {
                used = self.disk_usage.load(Ordering::Relaxed);
                budget_used = used
                    .saturating_add(pending_bytes)
                    .saturating_add(lane_publication_reservations)
                    .saturating_add(certified_bundle_reservations)
                    .saturating_add(autonomous_terminal_reservations)
                    .saturating_add(prune_maintenance_headroom);
                required = used
                    .saturating_add(pending_bytes)
                    .saturating_add(lane_publication_reservations)
                    .saturating_add(certified_bundle_reservations)
                    .saturating_add(autonomous_terminal_reservations)
                    .saturating_add(prune_maintenance_headroom)
                    .saturating_add(block_required)
                    .saturating_add(merge_entry_bytes)
                    .saturating_add(association_stage_bytes);
                if required <= limit {
                    if let Some(telemetry) = self.telemetry.get() {
                        telemetry.record_storage_budget_usage("kura", required, limit);
                    }
                    return Ok(());
                }
            }
            let evict_needed = required.saturating_sub(limit);
            if evict_needed > 0 {
                self.request_background_budget_eviction(evict_needed);
            }
            if let Some(telemetry) = self.telemetry.get() {
                telemetry.record_storage_budget_usage("kura", budget_used, limit);
                telemetry.inc_storage_budget_exceeded("kura");
            }
            warn!(
                used,
                required,
                limit,
                path = %self.store_root.display(),
                "Kura storage budget exceeded"
            );
            return Err(Error::StorageBudgetExceeded {
                limit,
                used,
                required,
            });
        }
        if let Some(telemetry) = self.telemetry.get() {
            telemetry.record_storage_budget_usage("kura", required, limit);
        }
        Ok(())
    }
    /// Store a block durably in Kura's canonical block store.
    ///
    /// # Errors
    /// Returns an error if the block violates canonical height ordering or cannot be persisted.
    /// For a compact merge carrier, an error after the canonical block fsync leaves the block and
    /// exact pending sidecar durable; an exact retry repairs the merge-log/carrier suffix.
    pub fn store_block(&self, block: impl Into<Arc<SignedBlock>>) -> Result<()> {
        let block = block.into();
        let merge_entry = if let Some(reference) = Self::block_merge_reference(&block) {
            Some(self.merge_entry_by_hash(reference.entry_hash)?.ok_or(
                Error::MissingCertifiedMergeSidecar {
                    entry_hash: reference.entry_hash,
                },
            )?)
        } else {
            None
        };
        self.store_block_durable(&block, merge_entry.as_ref())?;
        self.note_committed_lane_status_change();
        Ok(())
    }
    /// Read the exact canonical framed block bytes persisted at `height`.
    #[cfg(test)]
    pub(crate) fn canonical_block_wire_bytes_for_testing(
        &self,
        height: NonZeroUsize,
    ) -> Result<Vec<u8>> {
        let mut block_store = self.block_store.lock();
        let index_position = u64::try_from(height.get().saturating_sub(1))?;
        let index = block_store.read_block_index(index_position)?;
        if index.is_evicted() {
            return block_store.read_da_block_bytes(u64::try_from(height.get())?, index.length);
        }
        let mut bytes = vec![0_u8; usize::try_from(index.length)?];
        block_store.read_block_data(index.start, &mut bytes)?;
        Ok(bytes)
    }
    fn sync_native_amx_evidence_namespace(
        &self,
        namespace: &BoundProgressNamespace,
        kind: &str,
    ) -> Result<()> {
        for (index, directory) in namespace.directories.iter().enumerate() {
            let result = if index == 0 {
                sync_indexed_sidecar_dir_handle(&directory.file)
            } else {
                sync_progress_sidecar_ancestor_dir_handle(&directory.file)
            };
            if let Err(error) = result {
                iroha_logger::warn!(
                    ?error,
                    path = ?directory.expected_path,
                    kind,
                    "failed to sync descriptor-bound Native AMX evidence namespace"
                );
                return Err(Self::invalid_lane_artifact_error(
                    namespace.data_path.clone(),
                    format!("{kind} directory durability sync failed"),
                ));
            }
        }
        // Standalone evidence publication and pair pruning necessarily change
        // the immediate directory timestamps. Retain the descriptor-bound
        // directory-object invariant without applying the indexed-pair
        // helper's pre-mutation timestamp snapshot after the mutation.
        if !Self::progress_mutation_namespace_unchanged(namespace) {
            return Err(Self::invalid_lane_artifact_error(
                namespace.data_path.clone(),
                format!("{kind} directory durability sync failed"),
            ));
        }
        Ok(())
    }
    fn read_bound_regular_file_bytes_locked(
        &self,
        namespace: &BoundProgressNamespace,
        path: &Path,
        max_bytes: usize,
        kind: &str,
    ) -> Result<Option<Vec<u8>>> {
        let directory = path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} path has no directory"),
            )
        })?;
        let Some(metadata) = Self::regular_sidecar_metadata_for(&self.store_root, path, directory)?
        else {
            if !Self::progress_mutation_namespace_unchanged(namespace) {
                return Err(Self::invalid_lane_artifact_error(
                    path.to_path_buf(),
                    format!("{kind} namespace changed while proving absence"),
                ));
            }
            return Ok(None);
        };
        let len = usize::try_from(metadata.file.len())?;
        if len == 0 || len > max_bytes {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} has an empty or oversized payload"),
            ));
        }
        let mut file = Self::open_bound_progress_file(namespace, path, &metadata)?;
        let mut bytes = Vec::with_capacity(len);
        std::io::Read::by_ref(&mut file)
            .take(u64::try_from(max_bytes)?.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let current = Self::regular_sidecar_metadata_for(&self.store_root, path, directory)?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.to_path_buf(),
                    format!("{kind} disappeared while reading"),
                )
            })?;
        if bytes.len() != len
            || !Self::stable_sidecar_metadata_unchanged(&metadata, &current)
            || !Self::progress_mutation_namespace_unchanged(namespace)
        {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} changed while reading"),
            ));
        }
        Ok(Some(bytes))
    }
    fn open_bound_regular_file_with_exact_bytes_locked(
        &self,
        namespace: &BoundProgressNamespace,
        path: &Path,
        expected_bytes: &[u8],
        max_bytes: usize,
        kind: &str,
    ) -> Result<(std::fs::File, StableSidecarMetadata)> {
        let directory = path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} path has no directory"),
            )
        })?;
        let metadata = Self::regular_sidecar_metadata_for(&self.store_root, path, directory)?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.to_path_buf(),
                    format!("{kind} disappeared before exact-object binding"),
                )
            })?;
        let len = usize::try_from(metadata.file.len())?;
        if len == 0 || len > max_bytes || len != expected_bytes.len() {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} changed size before exact-object binding"),
            ));
        }
        let mut file = Self::open_bound_progress_file(namespace, path, &metadata)?;
        self.verify_bound_open_regular_file_exact_bytes_locked(
            namespace,
            path,
            &mut file,
            &metadata,
            expected_bytes,
            max_bytes,
            kind,
        )?;
        Ok((file, metadata))
    }
    #[allow(clippy::too_many_arguments)]
    fn verify_bound_open_regular_file_exact_bytes_locked(
        &self,
        namespace: &BoundProgressNamespace,
        path: &Path,
        file: &mut std::fs::File,
        metadata: &StableSidecarMetadata,
        expected_bytes: &[u8],
        max_bytes: usize,
        kind: &str,
    ) -> Result<()> {
        let directory = path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} path has no directory"),
            )
        })?;
        file.seek(SeekFrom::Start(0))
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let mut readback = Vec::with_capacity(expected_bytes.len());
        std::io::Read::by_ref(&mut *file)
            .take(u64::try_from(max_bytes)?.saturating_add(1))
            .read_to_end(&mut readback)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let opened = secure_file_metadata::from_file(file)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let current = Self::regular_sidecar_metadata_for(&self.store_root, path, directory)?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.to_path_buf(),
                    format!("{kind} disappeared during exact-object binding"),
                )
            })?;
        if readback != expected_bytes
            || !Self::sidecar_file_metadata_unchanged(&metadata.file, &opened)
            || !Self::stable_sidecar_metadata_unchanged(&metadata, &current)
            || !Self::progress_mutation_namespace_unchanged(namespace)
        {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} changed during exact-object binding"),
            ));
        }
        file.seek(SeekFrom::Start(0))
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn verify_bound_open_regular_file_exact_bytes_after_namespace_mutation_locked(
        &self,
        namespace: &BoundProgressNamespace,
        path: &Path,
        file: &mut std::fs::File,
        metadata: &StableSidecarMetadata,
        expected_bytes: &[u8],
        max_bytes: usize,
        kind: &str,
    ) -> Result<()> {
        let directory = path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} path has no directory"),
            )
        })?;
        file.seek(SeekFrom::Start(0))
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let mut readback = Vec::with_capacity(expected_bytes.len());
        std::io::Read::by_ref(&mut *file)
            .take(u64::try_from(max_bytes)?.saturating_add(1))
            .read_to_end(&mut readback)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let opened = secure_file_metadata::from_file(file)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let current = Self::regular_sidecar_metadata_for(&self.store_root, path, directory)?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.to_path_buf(),
                    format!("{kind} disappeared after namespace mutation"),
                )
            })?;
        if readback != expected_bytes
            || metadata.canonical_path != current.canonical_path
            || !Self::sidecar_file_metadata_unchanged(&metadata.file, &opened)
            || !Self::sidecar_file_metadata_unchanged(&metadata.file, &current.file)
            || !Self::sidecar_directory_binding_unchanged(&metadata.directory, &current.directory)
            || !Self::progress_mutation_namespace_unchanged(namespace)
        {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} changed after namespace mutation"),
            ));
        }
        file.seek(SeekFrom::Start(0))
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        Ok(())
    }
    fn publish_bound_noclobber_file_locked(
        &self,
        namespace: &BoundProgressNamespace,
        path: &Path,
        temp_path: &Path,
        bytes: &[u8],
        kind: &str,
    ) -> Result<bool> {
        self.durable_mutation_authorized()?;
        if path.parent() != namespace.data_path.parent()
            || temp_path.parent() != namespace.data_path.parent()
        {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} publication escapes its descriptor-bound directory"),
            ));
        }
        if self
            .read_bound_regular_file_bytes_locked(namespace, path, bytes.len().max(1), kind)?
            .is_some()
        {
            return Ok(false);
        }
        match std::fs::symlink_metadata(temp_path) {
            Ok(_) => {
                return Err(Self::invalid_lane_artifact_error(
                    temp_path.to_path_buf(),
                    format!("{kind} publication found an unresolved temporary"),
                ));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(Error::IO(error, temp_path.to_path_buf())),
        }
        let mut temporary = Self::create_new_bound_progress_temp(namespace, temp_path)
            .map_err(|error| Error::IO(error, temp_path.to_path_buf()))?;
        #[cfg(test)]
        self.fail_native_amx_publication_temp_prefix_at_path_for_tests(
            &mut temporary,
            temp_path,
            bytes,
        )?;
        #[cfg(test)]
        if let Some(prefix_len) =
            FAIL_AFTER_NEXT_NATIVE_AMX_EVIDENCE_TEMP_PREFIX.with(|flag| flag.take())
        {
            assert!(
                prefix_len < bytes.len(),
                "crash cut must precede the complete artifact"
            );
            temporary
                .write_all(&bytes[..prefix_len])
                .map_err(|error| Error::IO(error, temp_path.to_path_buf()))?;
            // Keep this original exclusive-create descriptor's actual bytes, as
            // process loss before write_all completion would. No fabricated file.
            return Err(Error::IO(
                std::io::Error::other(
                    "injected Native evidence interruption during temporary write",
                ),
                temp_path.to_path_buf(),
            ));
        }
        if let Err(error) = temporary
            .write_all(bytes)
            .and_then(|_| temporary.flush())
            .and_then(|_| temporary.sync_all())
        {
            drop(temporary);
            let _ = Self::remove_bound_progress_temp_if_present(namespace, temp_path);
            let _ = Self::sync_bound_progress_intent_directories(namespace);
            return Err(Error::IO(error, temp_path.to_path_buf()));
        }
        #[cfg(test)]
        if FAIL_AFTER_NEXT_NATIVE_AMX_EVIDENCE_TEMP_SYNC.with(|flag| flag.replace(false)) {
            // Retain the actual synced temporary exactly as a crash between
            // write/fsync and promotion would; no synthetic artifact is installed.
            return Err(Error::IO(
                std::io::Error::other(
                    "injected Native evidence interruption after temporary fsync",
                ),
                temp_path.to_path_buf(),
            ));
        }
        if let Err(error) =
            Self::promote_bound_progress_temp_noreplace(namespace, temp_path, path, &temporary)
        {
            drop(temporary);
            if !error.published {
                let _ = Self::remove_bound_progress_temp_if_present(namespace, temp_path);
                let _ = Self::sync_bound_progress_intent_directories(namespace);
            }
            return Err(Error::IO(error.source, path.to_path_buf()));
        }
        self.sync_native_amx_evidence_namespace(namespace, kind)?;
        let persisted = self
            .read_bound_regular_file_bytes_locked(namespace, path, bytes.len().max(1), kind)?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.to_path_buf(),
                    format!("{kind} disappeared after publication"),
                )
            })?;
        if persisted != bytes {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                format!("{kind} changed during publication"),
            ));
        }
        Ok(true)
    }
    // Drop cached blocks that are already persisted and outside the retention window.
    // Keep the genesis block plus the most recent `blocks_in_memory` persisted blocks.
    fn drop_persisted_blocks(
        block_data: &mut BlockData,
        persisted_count: usize,
        blocks_in_memory: usize,
    ) {
        let drop_before = persisted_count.saturating_sub(blocks_in_memory);
        let limit = drop_before.min(block_data.len());
        if limit <= 1 {
            return;
        }
        // (Genesis block is used in metrics to get genesis timestamp.)
        match block_data {
            BlockData::Dense(entries) => {
                for entry in entries.iter_mut().take(limit).skip(1) {
                    entry.1 = None;
                }
            }
            BlockData::Deferred { entries, .. } => {
                for entry in entries.range_mut(1..limit).map(|(_, entry)| entry) {
                    entry.1 = None;
                }
            }
        }
    }
    /// Returns count of blocks Kura currently holds
    pub fn blocks_count(&self) -> usize {
        if self.prune_recovery_is_required()
            || self.canonical_storage_poisoned.load(Ordering::Acquire)
        {
            return 0;
        }
        let data = self.block_data.lock();
        if self.prune_recovery_is_required() {
            return 0;
        }
        data.len()
    }
    /// Return the exact count of blocks durably committed to disk.
    ///
    /// Unlike logical-height and telemetry accessors, this propagates commit
    /// marker/index corruption and conversion failures. Startup and every
    /// authorization decision must use this method so an in-memory height can
    /// never mask an unauthenticated durable boundary.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical storage is poisoned or the durable
    /// commit marker/index cannot be read exactly.
    pub fn exact_durable_blocks_count(&self) -> Result<usize> {
        self.ensure_canonical_storage_not_poisoned()?;
        let count = self.block_store.lock().read_exact_durable_index_count()?;
        usize::try_from(count).map_err(Error::from)
    }
    /// Bind startup replay to one exact durable hash-journal image.
    ///
    /// The returned hashes are read under the same block-store lock as the
    /// authenticated durable count. Callers re-read and compare this value
    /// after prevalidation so a same-height journal replacement cannot be
    /// consumed as though it were the prevalidated chain.
    pub(crate) fn exact_replay_boundary(&self) -> Result<ExactReplayBoundary> {
        self.ensure_canonical_storage_not_poisoned()?;
        let mut store = self.block_store.lock();
        let count = store.read_exact_durable_index_count()?;
        let hashes = store.read_block_hashes(0, usize::try_from(count)?)?;
        if store.read_exact_durable_index_count()? != count {
            return Err(Error::HashesFileHeightMismatch);
        }
        Ok(ExactReplayBoundary { count, hashes })
    }
    /// Return whether this process deliberately skipped historical startup audits.
    pub const fn emergency_fast_startup_enabled(&self) -> bool {
        self.auxiliary_history_deferred
    }
    /// Read the one canonical hash needed to bind a restored snapshot in emergency Fast mode.
    ///
    /// The durable count and requested boundary hash are captured under the canonical-chain and
    /// block-store locks. This deliberately avoids materializing or rereading the historical hash
    /// prefix; Strict startup is responsible for validating every retained height later.
    pub(crate) fn emergency_fast_snapshot_boundary(
        &self,
        snapshot_height: usize,
    ) -> Result<(usize, Option<HashOf<BlockHeader>>)> {
        if !self.emergency_fast_startup_enabled() {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidInput,
                    "emergency Fast snapshot boundary requested after Strict startup",
                ),
                self.active_blocks_dir.lock().clone(),
            ));
        }
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        self.ensure_canonical_storage_not_poisoned()?;
        let mut store = self.block_store.lock();
        let count = store.read_exact_durable_index_count()?;
        let requested_height = u64::try_from(snapshot_height)?;
        let boundary_hash = if requested_height == 0 || requested_height > count {
            None
        } else {
            store
                .read_block_hashes(requested_height.saturating_sub(1), 1)?
                .first()
                .copied()
        };
        if (requested_height > 0 && requested_height <= count && boundary_hash.is_none())
            || store.read_exact_durable_index_count()? != count
        {
            return Err(Error::HashesFileHeightMismatch);
        }
        Ok((usize::try_from(count)?, boundary_hash))
    }
    /// Map the exact durable hash prefix used by emergency Fast state reads.
    ///
    /// The mapping is copy-on-write and read-only, so startup neither copies
    /// the complete journal nor permits State to mutate it. Kura's exclusive
    /// store lock keeps the mapped file from being truncated by another node
    /// process; this Fast process never starts a writer.
    pub(crate) fn emergency_fast_snapshot_hash_mapping(
        &self,
        snapshot_height: usize,
    ) -> Result<Option<ReadOnlyMmap>> {
        if !self.emergency_fast_startup_enabled() {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidInput,
                    "emergency Fast hash mapping requested after Strict startup",
                ),
                self.active_blocks_dir.lock().clone(),
            ));
        }
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        self.ensure_canonical_storage_not_poisoned()?;
        let mut store = self.block_store.lock();
        let durable_count = store.read_exact_durable_index_count()?;
        if usize::try_from(durable_count)? != snapshot_height {
            return Err(Error::HashesFileHeightMismatch);
        }
        if snapshot_height == 0 {
            return Ok(None);
        }
        let byte_len = snapshot_height
            .checked_mul(Hash::LENGTH)
            .ok_or(Error::HashesFileHeightMismatch)?;
        let hashes_path = store.path_to_blockchain.join(HASHES_FILE_NAME);
        let hashes_file = store.hashes_file.as_mut().ok_or_else(|| {
            Error::IO(
                std::io::Error::new(
                    ErrorKind::NotFound,
                    "emergency Fast hash journal is not open read-only",
                ),
                hashes_path,
            )
        })?;
        let file_len = hashes_file.try_io(|file| file.metadata().map(|metadata| metadata.len()))?;
        let mapping = hashes_file
            .try_io(|file| ReadOnlyMmap::copy_read_only_with_file_len(file, byte_len, file_len))?;
        Ok(Some(mapping))
    }
    /// Return the canonical block hash recorded at `height` without decoding the block body.
    pub fn block_hash_at_height(&self, height: NonZeroUsize) -> Option<HashOf<BlockHeader>> {
        if self.prune_recovery_is_required()
            || self.canonical_storage_poisoned.load(Ordering::Acquire)
        {
            return None;
        }
        if self.emergency_fast_startup_enabled() {
            return self.get_durable_block_hash(height);
        }
        let data = self.block_data.lock();
        if self.prune_recovery_is_required() {
            return None;
        }
        data.get(height.get().saturating_sub(1))
            .map(|(hash, _)| *hash)
    }
    /// Reconcile an exact outer-authenticated bootstrap snapshot while Kura remains provisional.
    pub(crate) fn reconcile_exact_audited_snapshot_bootstrap(
        &self,
        payload: &crate::snapshot::AuthenticatedSnapshotBootstrapPayload,
    ) -> Result<usize> {
        if !payload.is_exact_audited_boundary() {
            return Err(Error::SnapshotBootstrapAuthenticationPending);
        }
        let snapshot_hashes = payload.block_hashes();
        self.reconcile_authenticated_hash_only_snapshot(
            snapshot_hashes,
            HashOnlySnapshotExtensionMode::HardForkBootstrap,
            Some(payload.record()),
            true,
        )
    }
    /// Extend Kura's canonical hash chain using an audited hard-fork snapshot.
    ///
    /// The snapshot payload is the source of truth for hashes above the durable block body log.
    /// Missing bodies are persisted as hash-only placeholders so the next committed block can
    /// append at the snapshot height without replaying unavailable block bodies.
    #[cfg(test)]
    pub(crate) fn extend_hash_only_prefix_from_snapshot(
        &self,
        snapshot_hashes: &[HashOf<BlockHeader>],
    ) -> Result<usize> {
        self.reconcile_authenticated_hash_only_snapshot(
            snapshot_hashes,
            HashOnlySnapshotExtensionMode::HardForkBootstrap,
            None,
            false,
        )
    }
    /// Extend Kura's canonical hash chain using a verified local state snapshot.
    ///
    /// This is used when the signed WSV snapshot is ahead of a truncated durable block-body log.
    /// Existing block bodies remain readable; only the missing suffix is persisted as hash-only
    /// entries so startup can resume from the signed state without replaying unavailable bodies.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub fn extend_hash_only_suffix_from_verified_snapshot(
        &self,
        snapshot_hashes: &[HashOf<BlockHeader>],
    ) -> Result<usize> {
        self.reconcile_authenticated_hash_only_snapshot(
            snapshot_hashes,
            HashOnlySnapshotExtensionMode::VerifiedLocalSnapshot,
            None,
            false,
        )
    }
    fn reconcile_authenticated_hash_only_snapshot(
        &self,
        snapshot_hashes: &[HashOf<BlockHeader>],
        mode: HashOnlySnapshotExtensionMode,
        bootstrap_lineage: Option<&SnapshotV2BootstrapRecord>,
        provisional_transition: bool,
    ) -> Result<usize> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        if snapshot_hashes.is_empty() {
            return Ok(0);
        }
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        if provisional_transition {
            self.raw_geometry_claim.ensure_unclaimed()?;
            if !self.provisional_snapshot_bootstrap_pending() {
                return Err(Error::SnapshotBootstrapAuthenticationPending);
            }
            self.ensure_canonical_storage_not_poisoned()?;
        } else {
            self.resolve_canonical_storage_before_mutation()?;
        }
        let blocks_dir = self.active_blocks_dir.lock().clone();
        self.resolve_retained_block_rewrite_stage_before_canonical_mutation(&blocks_dir)?;
        let mut block_data = self.block_data.lock();
        let current = block_data.len();
        let target = snapshot_hashes.len();
        let shared = current.min(target);
        let bootstrap_lineage_hash = bootstrap_lineage.map(snapshot_bootstrap_lineage_digest);
        let mut rewrite_from = current;
        let complete_entries =
            block_data
                .dense_entries()
                .ok_or(Error::EmergencyFastAuxiliaryUnavailable {
                    subsystem: "complete canonical history",
                })?;
        for (idx, (existing, _)) in complete_entries.iter().enumerate().take(shared) {
            let actual = snapshot_hashes[idx];
            if *existing == actual {
                continue;
            }
            return Err(Error::BlockHeightConflict {
                height: u64::try_from(idx.saturating_add(1))?,
                expected: *existing,
                actual,
            });
        }
        if target < current && rewrite_from == current {
            return Err(Error::HashesFileHeightMismatch);
        }
        if target == current && rewrite_from == current {
            if mode.marks_hash_only_prefix() {
                let marker_result = (|| -> Result<()> {
                    let _write_guard = self.block_store_write_lock.lock();
                    let mut block_store = self.block_store.lock();
                    let resources = self
                        .begin_total_disk_usage_mutation()
                        .with_resource_paths(Self::canonical_physical_fixed_paths(&block_store));
                    block_store.sync_target(FsyncTarget::Hashes, BlockStore::ensure_hashes_file)?;
                    block_store.sync_target(FsyncTarget::Index, BlockStore::ensure_index_file)?;
                    block_store.write_verified_snapshot_tail_marker(
                        u64::try_from(target)?,
                        snapshot_hashes,
                        bootstrap_lineage_hash,
                    )?;
                    resources.finish_resources_before_disk_rescan();
                    Ok(())
                })();
                if let Err(error) = marker_result {
                    self.poison_canonical_storage(
                        "audited snapshot prefix marker publication",
                        &error,
                    );
                    return Err(error);
                }
                let previous = self.hard_fork_hash_only_block_count.load(Ordering::Relaxed);
                if previous != target {
                    self.hard_fork_hash_only_block_count
                        .store(target, Ordering::Relaxed);
                    info!(
                        previous_hash_only_block_count = previous,
                        snapshot_height = target,
                        recovery = mode.label(),
                        "aligned existing Kura hash-only snapshot entries"
                    );
                }
            }
            return Ok(0);
        }
        rewrite_from = rewrite_from.min(shared);
        self.ensure_no_retired_rollback_intents()?;
        if rewrite_from < current {
            self.ensure_v2_finality_allows_rewrite_from(
                &blocks_dir,
                u64::try_from(rewrite_from)?.saturating_add(1),
            )?;
        }
        let start = u64::try_from(rewrite_from)?;
        let target_u64 = u64::try_from(target)?;
        #[cfg(test)]
        self.maybe_pause_hash_only_extension_before_store_for_tests();
        let publication = self.with_retained_block_records_staged_for_rewrite(
            &blocks_dir,
            u64::try_from(rewrite_from)?.saturating_add(1),
            || {
                let _write_guard = self.block_store_write_lock.lock();
                let mut block_store = self.block_store.lock();
                let before_bytes = Self::block_store_tracked_bytes(&mut block_store).ok();
                let accounting_mutation = self
                    .begin_total_disk_usage_mutation()
                    .with_resource_paths(Self::canonical_physical_fixed_paths(&block_store));
                let hashes_file = block_store.ensure_hashes_file()?;
                hashes_file.try_io(|file| {
                    file.set_len(target_u64.saturating_mul(SIZE_OF_BLOCK_HASH))?;
                    file.seek(SeekFrom::Start(start.saturating_mul(SIZE_OF_BLOCK_HASH)))?;
                    for hash in &snapshot_hashes[rewrite_from..] {
                        file.write_all(hash.as_ref())?;
                    }
                    file.flush()
                })?;
                let index_file = block_store.ensure_index_file()?;
                index_file.try_io(|file| {
                    file.set_len(target_u64.saturating_mul(BlockIndex::SIZE))?;
                    file.seek(SeekFrom::Start(start.saturating_mul(BlockIndex::SIZE)))?;
                    let entry = BlockIndex {
                        start: EVICTED_BLOCK_START,
                        length: 0,
                    }
                    .encode();
                    for _ in rewrite_from..target {
                        file.write_all(&entry)?;
                    }
                    file.flush()
                })?;
                // This marker is the durable capability proving that zero-length entries were
                // published only after snapshot authentication. Sync its hash/index inputs first
                // even when normal Kura fsync is deferred by batching, then publish the ordinary
                // count marker.
                block_store.sync_target(FsyncTarget::Hashes, BlockStore::ensure_hashes_file)?;
                block_store.sync_target(FsyncTarget::Index, BlockStore::ensure_index_file)?;
                let marker_body_prefix = if mode.marks_hash_only_prefix() {
                    target_u64
                } else {
                    start
                };
                block_store.write_verified_snapshot_tail_marker(
                    marker_body_prefix,
                    snapshot_hashes,
                    bootstrap_lineage_hash,
                )?;
                block_store.publish_commit_marker(target_u64)?;
                if let Some(before_bytes) = before_bytes
                    && let Ok(after_bytes) = Self::block_store_tracked_bytes(&mut block_store)
                {
                    self.update_disk_usage_delta(before_bytes, after_bytes);
                    accounting_mutation.finish();
                }
                Ok(())
            },
        );
        let publication = match publication {
            Ok(publication) => publication,
            Err(error) => {
                self.poison_canonical_storage("hash-only snapshot marker publication", &error);
                return Err(error);
            }
        };
        publication.into_result(self)?;
        block_data.truncate(rewrite_from);
        block_data.extend(
            snapshot_hashes[rewrite_from..]
                .iter()
                .copied()
                .map(|hash| (hash, None)),
        );
        let rebuilt_height_index = Self::build_block_height_index(&block_data);
        let rebuilt_transaction_index = Self::build_transaction_entrypoint_index(&block_data);
        let mut block_height_index = self.block_height_index.lock();
        *block_height_index = rebuilt_height_index;
        drop(block_height_index);
        let mut transaction_entrypoint_index = self.transaction_entrypoint_index.lock();
        *transaction_entrypoint_index = rebuilt_transaction_index;
        drop(transaction_entrypoint_index);
        if mode.marks_hash_only_prefix() {
            self.hard_fork_hash_only_block_count
                .store(target, Ordering::Relaxed);
        }
        self.publish_durable_budget_snapshot(target, 0);
        let added = target.saturating_sub(current);
        info!(
            previous_height = current,
            rewrite_from_height = rewrite_from.saturating_add(1),
            snapshot_height = target,
            recovery = mode.label(),
            "extended Kura with hash-only snapshot entries"
        );
        Ok(added)
    }
}
include!("kura/prune_recovery_capacity.rs");
include!("kura/block_store_definition_and_test_controls.rs");
/// Read-only mirror of the block data file backed either by a memory mapping or a heap copy.
#[derive(Clone)]
enum MemoryMirror {
    /// OS-backed memory map sharing pages with the data file (copy-on-write).
    Mapped(Arc<ReadOnlyMmap>),
    /// Heap-backed copy used as a portable fallback when memory mapping is unavailable.
    Heap(Arc<[u8]>),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MemoryMirrorKind {
    /// Mirror is backed by an OS memory map.
    MemoryMapped,
    /// Mirror contains a heap-allocated copy of the file contents.
    HeapCopy,
}
impl MemoryMirror {
    fn from_mmap(map: ReadOnlyMmap) -> Self {
        Self::Mapped(Arc::new(map))
    }
    fn from_bytes(bytes: Vec<u8>) -> Self {
        Self::Heap(Arc::from(bytes.into_boxed_slice()))
    }
    fn len(&self) -> usize {
        match self {
            Self::Mapped(map) => map.len(),
            Self::Heap(bytes) => bytes.len(),
        }
    }
    fn slice(&self, start: usize, end: usize) -> &[u8] {
        match self {
            Self::Mapped(map) => &map[start..end],
            Self::Heap(bytes) => &bytes[start..end],
        }
    }
    fn kind(&self) -> MemoryMirrorKind {
        match self {
            Self::Mapped(_) => MemoryMirrorKind::MemoryMapped,
            Self::Heap(_) => MemoryMirrorKind::HeapCopy,
        }
    }
    fn from_file(file: &mut std::fs::File, len: usize, file_len: u64) -> std::io::Result<Self> {
        match ReadOnlyMmap::copy_read_only_with_file_len(file, len, file_len) {
            Ok(map) => Ok(Self::from_mmap(map)),
            Err(map_err) => {
                iroha_logger::debug!(
                    ?map_err,
                    "failed to memory-map block data; falling back to heap mirror"
                );
                file.seek(SeekFrom::Start(0))?;
                let mut buffer = Vec::with_capacity(len);
                {
                    let mut reader = file.take(len as u64);
                    reader.read_to_end(&mut buffer)?;
                    if buffer.len() != len {
                        return Err(std::io::Error::new(
                            ErrorKind::UnexpectedEof,
                            format!(
                                "expected {len} bytes while mirroring block data, read {} bytes",
                                buffer.len()
                            ),
                        ));
                    }
                }
                Ok(Self::from_bytes(buffer))
            }
        }
    }
}
#[derive(Default, Debug, Clone, Copy)]
/// Lightweight wrapper for block indices in the block index file
pub struct BlockIndex {
    /// Start of block in bytes
    pub start: u64,
    /// Length of block section in bytes
    pub length: u64,
}
impl BlockIndex {
    fn is_evicted(&self) -> bool {
        self.start == EVICTED_BLOCK_START
    }
}
impl BlockIndex {
    const SIZE: u64 = core::mem::size_of::<Self>() as u64;
    const SIZE_USIZE: usize = core::mem::size_of::<Self>();
    fn encode(self) -> [u8; Self::SIZE_USIZE] {
        let mut out = [0u8; Self::SIZE_USIZE];
        out[..core::mem::size_of::<u64>()].copy_from_slice(&self.start.to_le_bytes());
        out[core::mem::size_of::<u64>()..].copy_from_slice(&self.length.to_le_bytes());
        out
    }
    fn read(
        file: &mut std::fs::File,
        buff: &mut [u8; core::mem::size_of::<u64>()],
    ) -> std::io::Result<Self> {
        fn read_u64(
            file: &mut std::fs::File,
            buff: &mut [u8; core::mem::size_of::<u64>()],
        ) -> std::io::Result<u64> {
            file.read_exact(buff).map(|()| u64::from_le_bytes(*buff))
        }
        Ok(Self {
            start: read_u64(file, buff)?,
            length: read_u64(file, buff)?,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::kura::BlockStoreCommitMarker")]
struct BlockStoreCommitMarker {
    /// Marker format version (v1).
    version: u32,
    /// Count of blocks that are fully durable on disk.
    count: u64,
    /// Canonical header hash at `count`, or `None` for the empty chain.
    tip_hash: Option<HashOf<BlockHeader>>,
}
impl DaBlockRewriteImageV1 {
    fn index(&self) -> BlockIndex {
        BlockIndex {
            start: self.index_start,
            length: self.index_length,
        }
    }
}
impl LaneArtifactStorageView for LaneStorageEntry {
    fn storage_identity(&self) -> LaneStorageIdentity {
        self.identity
    }
    fn lane_id(&self) -> LaneId {
        self.lane_id
    }
    fn dataspace_id(&self) -> DataSpaceId {
        self.dataspace_id
    }
    fn blocks_dir(&self, store_root: &Path) -> PathBuf {
        LaneStorageEntry::blocks_dir(self, store_root)
    }
}
impl LaneArtifactStorageView for LaneArtifactPhysicalTarget {
    fn storage_identity(&self) -> LaneStorageIdentity {
        LaneStorageIdentity {
            network_id: self.network_id,
            lane_id: self.lane_id,
            dataspace_id: self.dataspace_id,
            incarnation: self.incarnation,
            activation_height: self.activation_height,
        }
    }
    fn lane_id(&self) -> LaneId {
        self.lane_id
    }
    fn dataspace_id(&self) -> DataSpaceId {
        self.dataspace_id
    }
    fn blocks_dir(&self, _store_root: &Path) -> PathBuf {
        self.blocks_path.clone()
    }
}
impl<T: LaneArtifactStorageView + ?Sized> LaneArtifactStorageView for &T {
    fn storage_identity(&self) -> LaneStorageIdentity {
        (**self).storage_identity()
    }
    fn lane_id(&self) -> LaneId {
        (**self).lane_id()
    }
    fn dataspace_id(&self) -> DataSpaceId {
        (**self).dataspace_id()
    }
    fn blocks_dir(&self, store_root: &Path) -> PathBuf {
        (**self).blocks_dir(store_root)
    }
}
/// Authenticated metadata for a body-less Kura suffix recovered from a verified local snapshot.
#[derive(Debug, Clone, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::kura::VerifiedSnapshotTailMarkerV1")]
struct VerifiedSnapshotTailMarkerV1 {
    /// Marker format version.
    version: u32,
    /// Count of entries preceding the recovered body-less suffix.
    body_prefix_count: u64,
    /// Signed snapshot height represented by the hash journal and zero-length indices.
    snapshot_height: u64,
    /// Domain-separated digest of the canonical hash journal through `snapshot_height`.
    hash_journal_digest: Hash,
    /// Domain-separated digest of the typed, originally authenticated bootstrap lineage.
    ///
    /// This is a consistency binding only. The marker is never an authority;
    /// startup must reauthenticate the exact record from a signed snapshot (or
    /// the one-time explicitly audited digest policy) before enabling output.
    bootstrap_lineage_hash: Option<Hash>,
}
impl VerifiedSnapshotTailMarkerV1 {
    const VERSION: u32 = 1;
    fn new(
        body_prefix_count: u64,
        snapshot_height: u64,
        hash_journal_digest: Hash,
        bootstrap_lineage_hash: Option<Hash>,
    ) -> Self {
        Self {
            version: Self::VERSION,
            body_prefix_count,
            snapshot_height,
            hash_journal_digest,
            bootstrap_lineage_hash,
        }
    }
}
const SNAPSHOT_BOOTSTRAP_LINEAGE_DIGEST_DOMAIN: &[u8] =
    b"iroha:kura:snapshot-bootstrap-lineage:v1\0";
/// Exact authenticated canonical hash-journal image used by startup replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExactReplayBoundary {
    pub(crate) count: u64,
    pub(crate) hashes: Vec<HashOf<BlockHeader>>,
}
fn snapshot_bootstrap_lineage_digest(record: &SnapshotV2BootstrapRecord) -> Hash {
    let encoded = record.encode();
    Hash::new_from_chunks(&[SNAPSHOT_BOOTSTRAP_LINEAGE_DIGEST_DOMAIN, &encoded])
}
impl BlockStoreCommitMarker {
    const VERSION: u32 = 1;
    fn new(count: u64, tip_hash: Option<HashOf<BlockHeader>>) -> Self {
        debug_assert_eq!(count == 0, tip_hash.is_none());
        Self {
            version: Self::VERSION,
            count,
            tip_hash,
        }
    }
}
include!("kura/pipeline_and_lane_artifacts.rs");
impl Kura {
    fn now_unix_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|dur| dur.as_secs())
            .unwrap_or(0)
    }
    /// Return the path to the block storage directory, if configured.
    /// For in-memory test Kura, returns `None`.
    fn store_dir(&self) -> Option<PathBuf> {
        let path = self.block_store.lock().path_to_blockchain.clone();
        if path.as_os_str().is_empty() {
            None
        } else {
            Some(path)
        }
    }
    fn sidecar_fsync_mode(&self) -> FsyncMode {
        self.block_store.lock().fsync.mode
    }
}
include!("kura/autonomous_terminal_capacity.rs");
include!("kura/sidecar_physical_resource_accounting.rs");
include!("kura/indexed_sidecar_io.rs");
include!("kura/consensus_storage_reads.rs");
include!("kura/native_execution_reads.rs");
pub(crate) use lane_admission_source::{
    canonical_admission_read_decode_limits, canonical_admission_read_working_set_bytes,
};
include!("kura/indexed_sidecar_rewrite.rs");
impl BlockStore {
    /// Create a new block store in `path`.
    pub fn new(store_path: impl AsRef<Path>) -> Self {
        Self::with_fsync(store_path, FsyncMode::Always, FSYNC_INTERVAL)
    }
    /// Open an existing block store for inspection without creating or
    /// requesting write access to any canonical journal.
    ///
    /// All three journals are opened eagerly so a missing or non-regular file
    /// fails before an inspector consumes a partial store.
    ///
    /// # Errors
    /// Returns an I/O error when a canonical journal cannot be opened read-only.
    pub fn open_read_only(store_path: impl AsRef<Path>) -> Result<Self> {
        let mut store = Self::with_fsync(store_path, FsyncMode::Always, FSYNC_INTERVAL);
        store.read_only = true;
        store.data_file = Some(FileWrap::open_read_only(
            store.path_to_blockchain.join(DATA_FILE_NAME),
        )?);
        store.index_file = Some(FileWrap::open_read_only(
            store.path_to_blockchain.join(INDEX_FILE_NAME),
        )?);
        store.hashes_file = Some(FileWrap::open_read_only(
            store.path_to_blockchain.join(HASHES_FILE_NAME),
        )?);
        Ok(store)
    }
    /// Create a new block store in `path` with an explicit fsync policy.
    pub fn with_fsync(
        store_path: impl AsRef<Path>,
        fsync_mode: FsyncMode,
        fsync_interval: Duration,
    ) -> Self {
        let path_to_blockchain = store_path.as_ref().to_path_buf();
        Self {
            da_blocks_dir: path_to_blockchain.join(DA_BLOCKS_DIR_NAME),
            path_to_blockchain,
            read_only: false,
            data_file: None,
            index_file: None,
            hashes_file: None,
            fsync: FsyncState::new(fsync_mode, fsync_interval),
            fsync_telemetry: FsyncTelemetry::new(fsync_mode),
            read_scratch: Vec::new(),
            data_mmap: None,
            data_mmap_len: 0,
            #[cfg(test)]
            body_bytes_read: AtomicU64::new(0),
            #[cfg(test)]
            body_read_calls: AtomicUsize::new(0),
            fast_prevalidated_count: None,
            commit_marker_count: 0,
            commit_marker_pending: None,
            deferred_da_recovery_fault: None,
            #[cfg(test)]
            fail_next_da_rewrite_before_marker: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_da_rewrite_after_marker: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_da_rewrite_recovery: AtomicBool::new(false),
            #[cfg(test)]
            crash_next_da_rewrite_before_marker: AtomicBool::new(false),
            #[cfg(test)]
            crash_next_da_rewrite_after_marker: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_commit_marker_write: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_commit_marker_after_temp_sync: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_commit_marker_read: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_commit_marker_ack_after_persist: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_commit_marker_write_and_readback: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_commit_marker_ack_and_readback: AtomicBool::new(false),
            #[cfg(test)]
            crash_next_eviction_after_stage: AtomicBool::new(false),
            #[cfg(test)]
            crash_next_eviction_after_data_promotion: AtomicBool::new(false),
            #[cfg(test)]
            crash_next_eviction_after_index_promotion: AtomicBool::new(false),
            #[cfg(test)]
            fail_eviction_stage_syncs_remaining: AtomicUsize::new(0),
        }
    }
    /// Resolve every durable canonical-storage transaction before another mutation.
    ///
    /// Eviction compaction is recovered first because it keeps the commit marker and
    /// hash journal fixed while atomically replacing the data/index pair. A DA rewrite
    /// may change that marker, so the two stages must never be observed in the opposite
    /// order after a crash.
    fn recover_canonical_storage_stages(&mut self) -> Result<()> {
        self.recover_canonical_storage_stages_with_carrier_pins(&BTreeMap::new())
    }
    fn ensure_data_file(&mut self) -> Result<&mut FileWrap> {
        if self.data_file.is_none() {
            let path = self.path_to_blockchain.join(DATA_FILE_NAME);
            self.data_file = Some(if self.read_only {
                FileWrap::open_read_only(path)?
            } else {
                FileWrap::open_existing_read_write(path)?
            });
        }
        Ok(self.data_file.as_mut().expect("handle just initialised"))
    }
    fn ensure_index_file(&mut self) -> Result<&mut FileWrap> {
        if self.index_file.is_none() {
            let path = self.path_to_blockchain.join(INDEX_FILE_NAME);
            self.index_file = Some(if self.read_only {
                FileWrap::open_read_only(path)?
            } else {
                FileWrap::open_existing_read_write(path)?
            });
        }
        Ok(self.index_file.as_mut().expect("handle just initialised"))
    }
    fn ensure_hashes_file(&mut self) -> Result<&mut FileWrap> {
        if self.hashes_file.is_none() {
            let path = self.path_to_blockchain.join(HASHES_FILE_NAME);
            self.hashes_file = Some(if self.read_only {
                FileWrap::open_read_only(path)?
            } else {
                FileWrap::open_existing_read_write(path)?
            });
        }
        Ok(self.hashes_file.as_mut().expect("handle just initialised"))
    }
    fn commit_marker_path(&self) -> PathBuf {
        self.path_to_blockchain.join(COUNT_FILE_NAME)
    }
    fn verified_snapshot_tail_marker_path(&self) -> PathBuf {
        self.path_to_blockchain
            .join(VERIFIED_SNAPSHOT_TAIL_FILE_NAME)
    }
    fn remove_verified_snapshot_tail_marker(&self) -> Result<()> {
        if self.path_to_blockchain.as_os_str().is_empty() {
            return Ok(());
        }
        let path = self.verified_snapshot_tail_marker_path();
        let tmp_path = path.with_extension("norito.tmp");
        for candidate in [&path, &tmp_path] {
            match std::fs::remove_file(candidate) {
                Ok(()) => {}
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => return Err(Error::IO(err, candidate.clone())),
            }
        }
        if let Some(parent) = path.parent() {
            sync_dir(parent).map_err(|err| Error::IO(err, parent.to_path_buf()))?;
        }
        Ok(())
    }
    fn write_verified_snapshot_tail_marker(
        &self,
        body_prefix_count: u64,
        snapshot_hashes: &[HashOf<BlockHeader>],
        bootstrap_lineage_hash: Option<Hash>,
    ) -> Result<()> {
        if self.path_to_blockchain.as_os_str().is_empty() {
            return Ok(());
        }
        let snapshot_height = u64::try_from(snapshot_hashes.len())?;
        if body_prefix_count > snapshot_height {
            return Err(Error::NoritoFrame(norito::core::Error::Message(
                "verified snapshot marker body prefix exceeds its snapshot height".to_owned(),
            )));
        }
        let marker = VerifiedSnapshotTailMarkerV1::new(
            body_prefix_count,
            snapshot_height,
            verified_snapshot_hash_journal_digest(snapshot_hashes)
                .add_err_context(&self.path_to_blockchain.join(HASHES_FILE_NAME))?,
            bootstrap_lineage_hash,
        );
        let bytes = norito::encode_canonical(&marker).map_err(Error::NoritoFrame)?;
        if bytes.len() > MAX_VERIFIED_SNAPSHOT_TAIL_MARKER_BYTES {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidInput,
                    "verified snapshot tail marker exceeds its hard byte limit",
                ),
                self.verified_snapshot_tail_marker_path(),
            ));
        }
        let path = self.verified_snapshot_tail_marker_path();
        let parent = path.parent().ok_or_else(|| {
            Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidInput,
                    "verified snapshot tail marker has no parent",
                ),
                path.clone(),
            )
        })?;
        let Some((canonical_parent, parent_before)) =
            Kura::canonical_sidecar_directory_for(&self.path_to_blockchain, parent)?
        else {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::NotFound,
                    "verified snapshot tail marker parent is missing",
                ),
                parent.to_path_buf(),
            ));
        };
        let _ = Kura::regular_sidecar_metadata_for(&self.path_to_blockchain, &path, parent)?;
        let mut temporary = tempfile::Builder::new()
            .prefix(".verified-snapshot-tail-")
            .tempfile_in(&canonical_parent)
            .map_err(|error| Error::IO(error, canonical_parent.clone()))?;
        temporary
            .as_file_mut()
            .write_all(&bytes)
            .and_then(|()| temporary.as_file_mut().flush())
            .and_then(|()| temporary.as_file().sync_all())
            .map_err(|error| Error::IO(error, path.clone()))?;
        let persisted = temporary
            .persist(&path)
            .map_err(|error| Error::IO(error.error, path.clone()))?;
        persisted
            .sync_all()
            .map_err(|error| Error::IO(error, path.clone()))?;
        sync_dir(parent).map_err(|error| Error::IO(error, parent.to_path_buf()))?;
        let parent_after = secure_file_metadata::from_path(parent)
            .map_err(|error| Error::IO(error, parent.to_path_buf()))?;
        if !Kura::sidecar_metadata_same_object(&parent_before, &parent_after) {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "verified snapshot tail marker parent changed during publication",
                ),
                parent.to_path_buf(),
            ));
        }
        let Some(readback) = Kura::read_regular_sidecar_bytes_for(
            &self.path_to_blockchain,
            &path,
            parent,
            MAX_VERIFIED_SNAPSHOT_TAIL_MARKER_BYTES,
        )?
        else {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::NotFound,
                    "verified snapshot tail marker disappeared after publication",
                ),
                path,
            ));
        };
        if readback != bytes {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "verified snapshot tail marker readback differs from its publication",
                ),
                path,
            ));
        }
        Ok(())
    }
    fn read_verified_snapshot_tail_marker(&self) -> Result<Option<VerifiedSnapshotTailMarkerV1>> {
        if self.path_to_blockchain.as_os_str().is_empty() {
            return Ok(None);
        }
        let path = self.verified_snapshot_tail_marker_path();
        let Some(bytes) = Kura::read_regular_sidecar_bytes_for(
            &self.path_to_blockchain,
            &path,
            &self.path_to_blockchain,
            MAX_VERIFIED_SNAPSHOT_TAIL_MARKER_BYTES,
        )?
        else {
            return Ok(None);
        };
        match norito::decode_canonical::<VerifiedSnapshotTailMarkerV1>(&bytes) {
            Ok(marker) if marker.version == VerifiedSnapshotTailMarkerV1::VERSION => {
                Ok(Some(marker))
            }
            Ok(marker) => {
                warn!(
                    version = marker.version,
                    "discarding verified snapshot tail marker with unsupported version"
                );
                self.remove_verified_snapshot_tail_marker()?;
                Ok(None)
            }
            Err(err) => {
                warn!(?err, "discarding malformed verified snapshot tail marker");
                self.remove_verified_snapshot_tail_marker()?;
                Ok(None)
            }
        }
    }
    /// Read and validate the structural snapshot marker without repairing or deleting it.
    ///
    /// The returned marker is only provisional metadata.  Its self-digest does
    /// not authenticate the hash journal or the bootstrap lineage.
    fn validated_verified_snapshot_tail_read_only(
        &mut self,
        logical_count: u64,
        hashes_count: u64,
    ) -> Result<Option<VerifiedSnapshotTailMarkerV1>> {
        if self.path_to_blockchain.as_os_str().is_empty() {
            return Ok(None);
        }
        let path = self.verified_snapshot_tail_marker_path();
        let Some(bytes) = Kura::read_regular_sidecar_bytes_for(
            &self.path_to_blockchain,
            &path,
            &self.path_to_blockchain,
            MAX_VERIFIED_SNAPSHOT_TAIL_MARKER_BYTES,
        )?
        else {
            return Ok(None);
        };
        let invalid = |reason: String| Error::InvalidSnapshotBootstrapMarker {
            path: path.clone(),
            reason,
        };
        let marker = norito::decode_canonical::<VerifiedSnapshotTailMarkerV1>(&bytes)
            .map_err(|error| invalid(format!("failed to decode marker: {error}")))?;
        if marker.version != VerifiedSnapshotTailMarkerV1::VERSION {
            return Err(invalid(
                "marker version or canonical encoding is invalid".to_owned(),
            ));
        }
        if marker.body_prefix_count > marker.snapshot_height
            || marker.snapshot_height > logical_count
            || marker.snapshot_height > hashes_count
        {
            return Err(invalid(format!(
                "marker bounds prefix={} snapshot={} index={} hashes={}",
                marker.body_prefix_count, marker.snapshot_height, logical_count, hashes_count
            )));
        }
        for index_pos in marker.body_prefix_count..marker.snapshot_height {
            let index = self.read_block_index(index_pos)?;
            if !index.is_evicted() || index.length != 0 {
                return Err(invalid(format!(
                    "marker hash-only position {index_pos} is not a zero-length evicted entry"
                )));
            }
        }
        let actual_digest =
            self.verified_snapshot_hash_journal_digest_from_store(marker.snapshot_height)?;
        if actual_digest != marker.hash_journal_digest {
            return Err(invalid(
                "marker hash-journal digest does not match durable hashes".to_owned(),
            ));
        }
        Ok(Some(marker))
    }
    fn validated_verified_snapshot_tail(
        &mut self,
        logical_count: u64,
        hashes_count: u64,
    ) -> Result<Option<VerifiedSnapshotTailMarkerV1>> {
        let Some(marker) = self.read_verified_snapshot_tail_marker()? else {
            return Ok(None);
        };
        let valid_bounds = marker.body_prefix_count <= marker.snapshot_height
            && marker.snapshot_height <= logical_count
            && marker.snapshot_height <= hashes_count;
        if !valid_bounds {
            warn!(
                body_prefix_count = marker.body_prefix_count,
                snapshot_height = marker.snapshot_height,
                logical_count,
                hashes_count,
                "discarding verified snapshot tail marker with invalid bounds"
            );
            self.remove_verified_snapshot_tail_marker()?;
            return Ok(None);
        }
        for index_pos in marker.body_prefix_count..marker.snapshot_height {
            let index = self.read_block_index(index_pos)?;
            if !index.is_evicted() || index.length != 0 {
                warn!(
                    index_pos,
                    start = index.start,
                    length = index.length,
                    "discarding verified snapshot tail marker with non-placeholder index"
                );
                self.remove_verified_snapshot_tail_marker()?;
                return Ok(None);
            }
        }
        let actual_digest =
            self.verified_snapshot_hash_journal_digest_from_store(marker.snapshot_height)?;
        if actual_digest != marker.hash_journal_digest {
            warn!(
                expected = %marker.hash_journal_digest,
                actual = %actual_digest,
                "discarding verified snapshot tail marker with mismatched hash journal digest"
            );
            self.remove_verified_snapshot_tail_marker()?;
            return Ok(None);
        }
        Ok(Some(marker))
    }
    fn read_bounded_commit_marker_bytes(path: &Path) -> Result<Option<Vec<u8>>> {
        let before = match secure_file_metadata::from_path(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(Error::IO(error, path.to_path_buf())),
        };
        if before.file_type().is_symlink()
            || !before.file_type().is_file()
            || !Kura::sidecar_is_single_link(&before)
            || before.len()
                > u64::try_from(MAX_VERIFIED_SNAPSHOT_TAIL_MARKER_BYTES).unwrap_or(u64::MAX)
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "block commit marker is not a bounded single-link regular file",
                ),
                path.to_path_buf(),
            ));
        }
        let mut file =
            std::fs::File::open(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let opened_before = secure_file_metadata::from_file(&file)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        if !opened_before.is_file()
            || !Kura::sidecar_is_single_link(&opened_before)
            || !Kura::sidecar_metadata_same_object(&before, &opened_before)
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "block commit marker changed while being opened",
                ),
                path.to_path_buf(),
            ));
        }
        let mut bytes = Vec::with_capacity(usize::try_from(before.len())?);
        (&mut file)
            .take(
                u64::try_from(MAX_VERIFIED_SNAPSHOT_TAIL_MARKER_BYTES)
                    .unwrap_or(u64::MAX)
                    .saturating_add(1),
            )
            .read_to_end(&mut bytes)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let opened_after = secure_file_metadata::from_file(&file)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let after = secure_file_metadata::from_path(path)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != before.len()
            || after.file_type().is_symlink()
            || !after.file_type().is_file()
            || !Kura::sidecar_is_single_link(&opened_after)
            || !Kura::sidecar_is_single_link(&after)
            || !Kura::sidecar_metadata_same_object(&before, &opened_after)
            || !Kura::sidecar_metadata_same_object(&opened_after, &after)
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "block commit marker changed during bounded read",
                ),
                path.to_path_buf(),
            ));
        }
        Ok(Some(bytes))
    }
    fn read_commit_marker(&mut self) -> Result<Option<BlockStoreCommitMarker>> {
        let path = self.commit_marker_path();
        if path.as_os_str().is_empty() {
            return Ok(None);
        }
        #[cfg(test)]
        if self
            .fail_next_commit_marker_read
            .swap(false, Ordering::AcqRel)
        {
            return Err(Error::IO(
                std::io::Error::other("injected commit-marker readback failure"),
                path,
            ));
        }
        let tmp_path = path.with_extension("norito.tmp");
        let mut main_invalid = false;
        let mut stable_marker = None;
        match Self::read_bounded_commit_marker_bytes(&path)? {
            Some(bytes) => match norito::decode_canonical::<BlockStoreCommitMarker>(&bytes) {
                Ok(marker) => {
                    if marker.version == BlockStoreCommitMarker::VERSION {
                        if (marker.count == 0) == marker.tip_hash.is_none() {
                            stable_marker = Some(marker);
                        } else {
                            return Err(Error::IO(
                                std::io::Error::new(
                                    ErrorKind::InvalidData,
                                    "block commit marker has an invalid empty/tip invariant",
                                ),
                                path,
                            ));
                        }
                    } else {
                        warn!(
                            version = marker.version,
                            "unsupported block store marker version; ignoring"
                        );
                        main_invalid = true;
                    }
                }
                Err(err) => {
                    warn!(
                        ?err,
                        "failed to decode block store marker; ignoring corrupted marker"
                    );
                    main_invalid = true;
                }
            },
            None => {}
        }
        if main_invalid {
            remove_commit_marker_temp_and_sync(&path)?;
        }
        match Self::read_bounded_commit_marker_bytes(&tmp_path)? {
            Some(bytes) => match norito::decode_canonical::<BlockStoreCommitMarker>(&bytes) {
                Ok(marker) => {
                    if marker.version != BlockStoreCommitMarker::VERSION {
                        warn!(
                            version = marker.version,
                            "unsupported block store temp marker version; ignoring"
                        );
                        remove_commit_marker_temp_and_sync(&tmp_path)?;
                        return Ok(stable_marker);
                    }
                    if (marker.count == 0) != marker.tip_hash.is_none() {
                        remove_commit_marker_temp_and_sync(&tmp_path)?;
                        return Ok(stable_marker);
                    }
                    warn!(
                        path = %tmp_path.display(),
                        "recovered block store marker from temp file"
                    );
                    promote_commit_marker_temp_and_sync(&tmp_path, &path)?;
                    let readback = Self::read_required_bounded_commit_marker_bytes(
                        &path,
                        "recovered block commit marker disappeared before readback",
                    )?;
                    if readback != bytes {
                        return Err(Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                "recovered block commit marker differs from its temp",
                            ),
                            path,
                        ));
                    }
                    Ok(Some(marker))
                }
                Err(err) => {
                    warn!(
                        ?err,
                        path = %tmp_path.display(),
                        "failed to decode temp block store marker"
                    );
                    remove_commit_marker_temp_and_sync(&tmp_path)?;
                    Ok(stable_marker)
                }
            },
            None => Ok(stable_marker),
        }
    }
    fn commit_marker_for_count(&mut self, count: u64) -> Result<BlockStoreCommitMarker> {
        let tip_hash = if count == 0 {
            None
        } else {
            self.read_block_hashes(count.saturating_sub(1), 1)?
                .first()
                .copied()
        };
        if count > 0 && tip_hash.is_none() {
            return Err(Error::OutOfBoundsBlockRead {
                start_block_height: count.saturating_sub(1),
                block_count: 1,
            });
        }
        Ok(BlockStoreCommitMarker::new(count, tip_hash))
    }
    fn write_commit_marker(&mut self, count: u64) -> Result<()> {
        let marker = self.commit_marker_for_count(count)?;
        self.write_commit_marker_value(&marker)
    }
    fn write_commit_marker_value(&mut self, marker: &BlockStoreCommitMarker) -> Result<()> {
        let path = self.commit_marker_path();
        if path.as_os_str().is_empty() {
            return Ok(());
        }
        #[cfg(test)]
        if self
            .fail_next_commit_marker_write_and_readback
            .swap(false, Ordering::AcqRel)
        {
            self.fail_next_commit_marker_read
                .store(true, Ordering::Release);
            return Err(Error::IO(
                std::io::Error::other(
                    "injected commit-marker write and subsequent readback failure",
                ),
                path,
            ));
        }
        #[cfg(test)]
        if self
            .fail_next_commit_marker_write
            .swap(false, Ordering::AcqRel)
        {
            return Err(Error::IO(
                std::io::Error::other("injected commit-marker write failure"),
                path,
            ));
        }
        if marker.version != BlockStoreCommitMarker::VERSION
            || (marker.count == 0) != marker.tip_hash.is_none()
        {
            return Err(Error::IO(
                std::io::Error::new(ErrorKind::InvalidInput, "invalid block commit marker"),
                path,
            ));
        }
        let bytes = norito::encode_canonical(marker).map_err(Error::NoritoFrame)?;
        let parent = path.parent().ok_or_else(|| {
            Error::IO(
                std::io::Error::new(ErrorKind::InvalidInput, "commit marker has no parent"),
                path.clone(),
            )
        })?;
        std::fs::create_dir_all(parent).map_err(|err| Error::IO(err, parent.to_path_buf()))?;
        if bytes.is_empty() || bytes.len() > MAX_VERIFIED_SNAPSHOT_TAIL_MARKER_BYTES {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "encoded block commit marker exceeds its hard bound",
                ),
                path,
            ));
        }
        let temporary_path = path.with_extension("norito.tmp");
        match std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary_path)
        {
            Ok(mut temporary) => {
                temporary
                    .write_all(&bytes)
                    .and_then(|()| temporary.flush())
                    .and_then(|()| temporary.sync_all())
                    .map_err(|error| Error::IO(error, temporary_path.clone()))?;
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                let existing = Self::read_bounded_commit_marker_bytes(&temporary_path)?
                    .ok_or_else(|| {
                        Error::IO(
                            std::io::Error::new(
                                ErrorKind::NotFound,
                                "block commit marker temp disappeared during retry",
                            ),
                            temporary_path.clone(),
                        )
                    })?;
                if existing != bytes {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::AlreadyExists,
                            "block commit marker temp conflicts with the intended marker",
                        ),
                        temporary_path,
                    ));
                }
            }
            Err(error) => return Err(Error::IO(error, temporary_path)),
        }
        self.maybe_fail_commit_marker_after_temp_sync(&temporary_path)?;
        std::fs::rename(&temporary_path, &path).map_err(|error| Error::IO(error, path.clone()))?;
        let persisted = std::fs::OpenOptions::new()
            .read(true)
            .open(&path)
            .map_err(|error| Error::IO(error, path.clone()))?;
        persisted
            .sync_all()
            .map_err(|error| Error::IO(error, path.clone()))?;
        sync_dir(parent).map_err(|err| Error::IO(err, parent.to_path_buf()))?;
        let readback = Self::read_bounded_commit_marker_bytes(&path)?.ok_or_else(|| {
            Error::IO(
                std::io::Error::new(
                    ErrorKind::NotFound,
                    "published block commit marker disappeared before exact readback",
                ),
                path.clone(),
            )
        })?;
        if readback != bytes {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "published block commit marker differs from its synced temporary",
                ),
                path,
            ));
        }
        #[cfg(test)]
        if self
            .fail_next_commit_marker_ack_and_readback
            .swap(false, Ordering::AcqRel)
        {
            self.fail_next_commit_marker_read
                .store(true, Ordering::Release);
            return Err(Error::IO(
                std::io::Error::other(
                    "injected persisted marker acknowledgement and readback failure",
                ),
                path,
            ));
        }
        #[cfg(test)]
        if self
            .fail_next_commit_marker_ack_after_persist
            .swap(false, Ordering::AcqRel)
        {
            return Err(Error::IO(
                std::io::Error::other(
                    "injected post-persist commit-marker acknowledgement failure",
                ),
                path,
            ));
        }
        Ok(())
    }
    fn validate_commit_marker_tip(
        &mut self,
        marker: &BlockStoreCommitMarker,
        hashes_count: u64,
    ) -> Result<()> {
        if marker.count == 0 {
            if marker.tip_hash.is_none() {
                return Ok(());
            }
        } else if marker.count <= hashes_count {
            let journal_tip = self
                .read_block_hashes(marker.count.saturating_sub(1), 1)?
                .first()
                .copied();
            if journal_tip == marker.tip_hash {
                return Ok(());
            }
        } else {
            // `init_commit_marker` explicitly reconciles a marker beyond the available hash
            // journal to the data-backed prefix below.
            return Ok(());
        }
        Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "block commit marker tip hash mismatches the canonical hash journal",
            ),
            self.commit_marker_path(),
        ))
    }
    fn align_hashes_len(&mut self) -> Result<u64> {
        if self.path_to_blockchain.as_os_str().is_empty() {
            return Ok(0);
        }
        let hashes_file = self.ensure_hashes_file()?;
        let len = hashes_file.try_io(|file| file.metadata().map(|meta| meta.len()))?;
        let aligned = len - (len % SIZE_OF_BLOCK_HASH);
        if aligned != len {
            warn!(
                len,
                aligned, "block hashes length misaligned; truncating trailing bytes"
            );
            hashes_file.try_io(|file| file.set_len(aligned))?;
        }
        Ok(aligned / SIZE_OF_BLOCK_HASH)
    }
    fn data_backed_count(
        &mut self,
        mut candidate: u64,
        hashes_count: u64,
        trusted_hash_only_tail: Option<(u64, u64)>,
    ) -> Result<u64> {
        if candidate > hashes_count {
            warn!(
                index_count = candidate,
                hashes_count,
                "block store index exceeds hash journal; capping durable count to hashes height"
            );
            candidate = hashes_count;
        }
        if candidate == 0 {
            return Ok(0);
        }
        let data_len = self.data_file_len()?;
        if data_len == 0 {
            return Ok(
                if trusted_hash_only_tail.is_some_and(|(start, end)| start == 0 && candidate <= end)
                {
                    candidate
                } else {
                    0
                },
            );
        }
        let initial = candidate;
        while candidate > 0 {
            match self.read_block_index(candidate - 1) {
                Ok(index) => {
                    if index.is_evicted() {
                        if index.length == 0
                            && trusted_hash_only_tail.is_some_and(|(start, end)| {
                                candidate > start && candidate <= end && candidate <= hashes_count
                            })
                        {
                            break;
                        }
                        if index.length == 0
                            || index.length > STRICT_INIT_MAX_BLOCK_BYTES
                            || candidate > hashes_count
                        {
                            candidate = candidate.saturating_sub(1);
                            continue;
                        }
                        break;
                    }
                    let end = if let Some(end) = index.start.checked_add(index.length) {
                        end
                    } else {
                        candidate = candidate.saturating_sub(1);
                        continue;
                    };
                    if index.length == 0
                        || index.length > STRICT_INIT_MAX_BLOCK_BYTES
                        || end > data_len
                    {
                        candidate = candidate.saturating_sub(1);
                        continue;
                    }
                    break;
                }
                Err(err) => {
                    warn!(
                        ?err,
                        candidate,
                        "failed to read block index while reconciling data length; truncating"
                    );
                    candidate = 0;
                    break;
                }
            }
        }
        if candidate != initial {
            warn!(
                initial,
                candidate,
                data_len,
                "block store data shorter than index; truncating durable count"
            );
        }
        Ok(candidate)
    }
    /// Validate the committed journal prefix before ordinary startup is allowed to mutate it.
    ///
    /// Fast mode never initializes or repairs storage. The store must already carry an exact stable
    /// commit marker and every required canonical file. Entries and bytes beyond that marker are
    /// unpublished crash suffixes and remain untouched for the next Strict restart to reconcile.
    fn preflight_fast_durable_prefix(&mut self) -> Result<u64> {
        if self.path_to_blockchain.as_os_str().is_empty() {
            self.fast_prevalidated_count = Some(0);
            return Ok(0);
        }
        let root_metadata = std::fs::symlink_metadata(&self.path_to_blockchain)
            .map_err(|error| Error::IO(error, self.path_to_blockchain.clone()))?;
        if root_metadata.file_type().is_symlink() || !root_metadata.file_type().is_dir() {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "Kura canonical block root is not a direct directory",
                ),
                self.path_to_blockchain.clone(),
            ));
        }
        let invalid = |path: PathBuf, reason: &'static str| {
            Error::IO(std::io::Error::new(ErrorKind::InvalidData, reason), path)
        };
        // Imported hash-only history has different trust semantics and cannot be
        // authorized by an ordinary emergency manifest. All other auxiliary
        // recovery artifacts are deliberately ignored until the Strict restart.
        let snapshot_tail_marker = self.verified_snapshot_tail_marker_path();
        match std::fs::symlink_metadata(&snapshot_tail_marker) {
            Ok(_) => {
                return Err(invalid(
                    snapshot_tail_marker,
                    "Kura Fast init cannot authorize imported hash-only history",
                ));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(Error::IO(error, snapshot_tail_marker)),
        }

        let marker_path = self.commit_marker_path();
        let marker_bytes = Self::read_required_bounded_commit_marker_bytes(
            &marker_path,
            "Kura fast init requires a stable block commit marker",
        )?;
        let marker =
            norito::decode_canonical::<BlockStoreCommitMarker>(&marker_bytes).map_err(|_| {
                invalid(
                    marker_path.clone(),
                    "Kura fast init requires a canonical block commit marker",
                )
            })?;
        if marker.version != BlockStoreCommitMarker::VERSION
            || (marker.count == 0) != marker.tip_hash.is_none()
        {
            return Err(invalid(
                marker_path,
                "Kura fast init found an invalid block commit marker",
            ));
        }

        let required_index_len = marker.count.checked_mul(BlockIndex::SIZE).ok_or_else(|| {
            invalid(
                self.path_to_blockchain.join(INDEX_FILE_NAME),
                "Kura fast init index prefix length overflows",
            )
        })?;
        let required_hashes_len =
            marker
                .count
                .checked_mul(SIZE_OF_BLOCK_HASH)
                .ok_or_else(|| {
                    invalid(
                        self.path_to_blockchain.join(HASHES_FILE_NAME),
                        "Kura fast init hash prefix length overflows",
                    )
                })?;
        let required_file_len = |path: &Path, required: u64| -> Result<u64> {
            let metadata = std::fs::symlink_metadata(path)
                .map_err(|error| Error::IO(error, path.to_path_buf()))?;
            if metadata.file_type().is_symlink()
                || !metadata.file_type().is_file()
                || metadata.len() < required
            {
                return Err(invalid(
                    path.to_path_buf(),
                    "Kura fast init found a missing, linked, or truncated canonical journal",
                ));
            }
            Ok(metadata.len())
        };
        let index_path = self.path_to_blockchain.join(INDEX_FILE_NAME);
        let hashes_path = self.path_to_blockchain.join(HASHES_FILE_NAME);
        let data_path = self.path_to_blockchain.join(DATA_FILE_NAME);
        let _index_len = required_file_len(&index_path, required_index_len)?;
        let _hashes_len = required_file_len(&hashes_path, required_hashes_len)?;
        let data_len = required_file_len(&data_path, 0)?;

        if marker.count > 0 {
            let mut hashes_file = std::fs::File::open(&hashes_path)
                .map_err(|error| Error::IO(error, hashes_path.clone()))?;
            hashes_file
                .seek(SeekFrom::Start(
                    marker.count.saturating_sub(1) * SIZE_OF_BLOCK_HASH,
                ))
                .map_err(|error| Error::IO(error, hashes_path.clone()))?;
            let mut tip_bytes = [0_u8; Hash::LENGTH];
            hashes_file
                .read_exact(&mut tip_bytes)
                .map_err(|error| Error::IO(error, hashes_path.clone()))?;
            let expected_tip_bytes: &[u8] = marker
                .tip_hash
                .as_ref()
                .expect("non-empty commit marker has a tip hash")
                .as_ref();
            if expected_tip_bytes != tip_bytes {
                return Err(invalid(
                    hashes_path,
                    "Kura fast init commit marker tip mismatches the hash journal",
                ));
            }
        }

        if marker.count > 0 {
            let mut index_file = std::fs::File::open(&index_path)
                .map_err(|error| Error::IO(error, index_path.clone()))?;
            index_file
                .seek(SeekFrom::Start(
                    marker.count.saturating_sub(1) * BlockIndex::SIZE,
                ))
                .map_err(|error| Error::IO(error, index_path.clone()))?;
            let mut buffer = [0_u8; core::mem::size_of::<u64>()];
            let index = BlockIndex::read(&mut index_file, &mut buffer).map_err(|_| {
                invalid(
                    index_path.clone(),
                    "Kura fast init could not read the committed tip index",
                )
            })?;
            if index.length == 0 || index.length > STRICT_INIT_MAX_BLOCK_BYTES {
                return Err(invalid(
                    index_path,
                    "Kura fast init found a zero or oversized committed tip length",
                ));
            }
            if !index.is_evicted() {
                let end = index.start.checked_add(index.length).ok_or_else(|| {
                    invalid(
                        data_path.clone(),
                        "Kura fast init committed tip range overflows",
                    )
                })?;
                if end > data_len {
                    return Err(invalid(
                        data_path,
                        "Kura fast init committed tip range exceeds the data journal",
                    ));
                }
            }
        }
        self.commit_marker_count = marker.count;
        self.commit_marker_pending = None;
        self.fast_prevalidated_count = Some(marker.count);
        Ok(marker.count)
    }
    fn init_commit_marker(&mut self) -> Result<()> {
        if self.path_to_blockchain.as_os_str().is_empty() {
            return Ok(());
        }
        let logical_count = {
            let index_file = self.ensure_index_file()?;
            let len = index_file.try_io(|file| file.metadata().map(|meta| meta.len()))?;
            let aligned = len - (len % BlockIndex::SIZE);
            if aligned != len {
                warn!(
                    len,
                    aligned, "block index length misaligned; truncating trailing bytes"
                );
                index_file.try_io(|file| file.set_len(aligned))?;
            }
            aligned / BlockIndex::SIZE
        };
        let hashes_count = self.align_hashes_len()?;
        let existing_marker = self.read_commit_marker()?;
        // When the hash journal was durably shortened before its marker, the
        // old tip is no longer readable. Defer that one case to the
        // conservative common-prefix reconciliation below; markers whose tip
        // is still addressable remain fully validated.
        if let Some(marker) = existing_marker.as_ref()
            && marker.count <= hashes_count
        {
            self.validate_commit_marker_tip(marker, hashes_count)?;
        }
        let verified_snapshot_tail =
            self.validated_verified_snapshot_tail(logical_count, hashes_count)?;
        let trusted_hash_only_tail = verified_snapshot_tail.as_ref().map(|marker| {
            let start = if marker.body_prefix_count == marker.snapshot_height {
                0
            } else {
                marker.body_prefix_count
            };
            (start, marker.snapshot_height)
        });
        let data_backed_count =
            self.data_backed_count(logical_count, hashes_count, trusted_hash_only_tail)?;
        let mut durable_count = if let Some(marker) = existing_marker {
            marker.count
        } else {
            self.write_commit_marker(data_backed_count)?;
            data_backed_count
        };
        if let Some(marker) = &verified_snapshot_tail
            && durable_count < marker.snapshot_height
            && data_backed_count >= marker.snapshot_height
        {
            durable_count = marker.snapshot_height;
            self.write_commit_marker(durable_count)?;
        }
        if durable_count > data_backed_count {
            warn!(
                durable_count,
                data_backed_count,
                "block store marker exceeds data-backed count; truncating marker"
            );
            durable_count = data_backed_count;
            self.write_commit_marker(durable_count)?;
        }
        if logical_count < durable_count {
            warn!(
                logical_count,
                durable_count, "block store marker exceeds index length; truncating marker"
            );
            durable_count = logical_count;
            self.write_commit_marker(durable_count)?;
        }
        if logical_count > durable_count {
            warn!(
                logical_count,
                durable_count,
                discarded_count = logical_count - durable_count,
                "block store contains an uncommitted suffix; pruning to the durable commit marker"
            );
            self.commit_marker_count = durable_count;
            self.commit_marker_pending = None;
            self.prune(durable_count)?;
        }
        self.truncate_hashes_to_count(durable_count)?;
        self.truncate_data_to_index(durable_count)?;
        if verified_snapshot_tail
            .as_ref()
            .is_some_and(|marker| durable_count < marker.snapshot_height)
        {
            self.remove_verified_snapshot_tail_marker()?;
        }
        self.commit_marker_count = durable_count;
        self.commit_marker_pending = None;
        Ok(())
    }
    fn drop_cached_handles(&mut self) {
        self.data_file = None;
        self.index_file = None;
        self.hashes_file = None;
        self.invalidate_data_mmap();
    }
    fn next_fsync_wait(&self) -> Option<Duration> {
        let deadline = self.fsync.deadline()?;
        let now = Instant::now();
        if deadline <= now {
            Some(Duration::ZERO)
        } else {
            deadline.checked_duration_since(now)
        }
    }
    #[cfg(test)]
    fn fsync_pending_for_tests(&self) -> bool {
        self.fsync.pending_since.is_some()
    }
    fn flush_pending_fsync(&mut self, force: bool) -> Result<()> {
        if !self.path_to_blockchain.as_os_str().is_empty()
            && (self.da_block_rewrite_stage_path().exists()
                || self.eviction_compaction_stage_path().exists())
        {
            self.recover_canonical_storage_stages()?;
        }
        self.fsync_telemetry.update_mode(self.fsync.mode);
        let now = Instant::now();
        if !self.fsync.is_due(now, force) {
            return Ok(());
        }
        // Sync index last so the commit marker only advances after data/hashes/index are durable.
        self.sync_target(FsyncTarget::Data, Self::ensure_data_file)?;
        self.sync_target(FsyncTarget::Hashes, Self::ensure_hashes_file)?;
        self.sync_target(FsyncTarget::Index, Self::ensure_index_file)?;
        self.commit_pending_marker()?;
        self.fsync.clear();
        Ok(())
    }
    fn commit_pending_marker(&mut self) -> Result<()> {
        let Some(count) = self.commit_marker_pending else {
            return Ok(());
        };
        let old_marker = match self.read_commit_marker() {
            Ok(marker) => marker,
            Err(read_error) => {
                self.commit_marker_pending = None;
                self.fsync.clear();
                return Err(self.unknown_da_rewrite_state(
                    "failed to read the pre-publication commit marker",
                    &read_error,
                ));
            }
        };
        let expected_marker = match self.commit_marker_for_count(count) {
            Ok(marker) => marker,
            Err(marker_error) => {
                self.commit_marker_pending = None;
                self.fsync.clear();
                if old_marker
                    .as_ref()
                    .is_some_and(|marker| marker.count < count)
                    && let Err(rollback_error) = self.init_commit_marker()
                {
                    return Err(self.unknown_da_rewrite_state(
                        "failed to roll back after deriving the pending marker failed",
                        &rollback_error,
                    ));
                }
                return Err(marker_error);
            }
        };
        match self.write_commit_marker_value(&expected_marker) {
            Ok(()) => {
                self.commit_marker_count = count;
                self.commit_marker_pending = None;
                Ok(())
            }
            Err(publication_error) => {
                let readback = match self.read_commit_marker() {
                    Ok(readback) => readback,
                    Err(readback_error) => {
                        self.commit_marker_pending = None;
                        self.fsync.clear();
                        return Err(self.unknown_da_rewrite_state(
                            "commit-marker publication and readback both failed",
                            &readback_error,
                        ));
                    }
                };
                if readback.as_ref() == Some(&expected_marker) {
                    self.commit_marker_count = count;
                    self.commit_marker_pending = None;
                    return Ok(());
                }
                if let Some(old_marker) = old_marker
                    && readback.as_ref() == Some(&old_marker)
                    && old_marker.count < count
                {
                    self.commit_marker_pending = None;
                    self.fsync.clear();
                    if let Err(rollback_error) = self.init_commit_marker() {
                        return Err(self.unknown_da_rewrite_state(
                            "failed to roll back an unpublished append journal",
                            &rollback_error,
                        ));
                    }
                    return Err(publication_error);
                }
                self.commit_marker_pending = None;
                self.fsync.clear();
                Err(self.unknown_da_rewrite_state(
                    "commit marker matches neither the pre-write nor published append state",
                    &publication_error,
                ))
            }
        }
    }
    fn publish_commit_marker(&mut self, count: u64) -> Result<()> {
        self.commit_marker_pending = Some(count);
        self.mark_fsync_pending();
        self.flush_pending_fsync(true)
    }
    fn sync_target(
        &mut self,
        target: FsyncTarget,
        open: impl Fn(&mut Self) -> Result<&mut FileWrap>,
    ) -> Result<()> {
        let start = Instant::now();
        let result = open(self)?.try_io(|inner| inner.sync_data());
        match result {
            Ok(()) => {
                self.fsync_telemetry.record_success(target, start.elapsed());
                Ok(())
            }
            Err(err) => {
                self.fsync_telemetry
                    .record_failure(target, Some(start.elapsed()));
                Err(err)
            }
        }
    }
    fn schedule_fsync_after_write(&mut self) -> Result<()> {
        self.mark_fsync_pending();
        self.flush_pending_fsync(false)
    }
    fn mark_fsync_pending(&mut self) {
        let now = Instant::now();
        self.fsync.record_write(now);
    }
    fn invalidate_data_mmap(&mut self) {
        let _ = self.data_mmap.take();
        self.data_mmap_len = 0;
    }
    fn ensure_data_file_present(&mut self) -> Result<()> {
        let path = self.path_to_blockchain.join(DATA_FILE_NAME);
        match std::fs::metadata(&path) {
            Ok(_) => Ok(()),
            Err(err) => {
                if err.kind() == std::io::ErrorKind::NotFound {
                    self.drop_cached_handles();
                }
                Err(Error::IO(err, path))
            }
        }
    }
    fn ensure_data_mmap(&mut self) -> Result<()> {
        self.ensure_data_file_present()?;
        let len = {
            let data_file = self.ensure_data_file()?;
            data_file.try_io(|file| file.metadata().map(|meta| meta.len()))?
        };
        if len == 0 {
            self.invalidate_data_mmap();
            return Ok(());
        }
        if self.data_mmap.as_ref().map(|m| m.len() as u64) == Some(len) {
            self.data_mmap_len = len;
            return Ok(());
        }
        self.invalidate_data_mmap();
        let len_usize: usize = len.try_into()?;
        let mirror = {
            let data_file = self.ensure_data_file()?;
            data_file.try_io(|file| MemoryMirror::from_file(file, len_usize, len))?
        };
        self.data_mmap_len = len;
        self.data_mmap = Some(mirror);
        Ok(())
    }
    /// Read a contiguous range of bytes from the block data file.
    ///
    /// # Errors
    /// Returns an error if the requested range is out of bounds or if the
    /// underlying storage cannot be read.
    pub fn block_bytes(&mut self, start: u64, length: u64) -> Result<&[u8]> {
        if length == 0 {
            self.ensure_data_mmap()?;
            return Ok(&[]);
        }
        self.ensure_data_mmap()?;
        let end = start
            .checked_add(length)
            .ok_or(Error::CorruptedBlockRange {
                start,
                length,
                data_len: self.data_mmap_len,
            })?;
        if end > self.data_mmap_len {
            return Err(Error::CorruptedBlockRange {
                start,
                length,
                data_len: self.data_mmap_len,
            });
        }
        #[cfg(test)]
        {
            self.body_read_calls.fetch_add(1, Ordering::Relaxed);
            self.body_bytes_read.fetch_add(length, Ordering::Relaxed);
        }
        let len_usize: usize = length.try_into()?;
        let start_usize: usize = start.try_into()?;
        let end_usize = start_usize + len_usize;
        if let Some(ref mirror) = self.data_mmap {
            return Ok(mirror.slice(start_usize, end_usize));
        }
        let mut scratch = std::mem::take(&mut self.read_scratch);
        if scratch.len() < len_usize {
            scratch.resize(len_usize, 0);
        }
        self.ensure_data_file()?.try_io(|file| {
            file.seek(SeekFrom::Start(start))?;
            file.read_exact(&mut scratch[..len_usize])
        })?;
        self.read_scratch = scratch;
        Ok(&self.read_scratch[..len_usize])
    }
    /// Read a series of block indices from the block index file and
    /// attempt to fill all of `dest_buffer`.
    ///
    /// # Errors
    /// IO Error.
    pub fn read_block_indices(
        &mut self,
        start_block_height: u64,
        dest_buffer: &mut [BlockIndex],
    ) -> Result<()> {
        let block_count = dest_buffer.len();
        if block_count == 0 {
            return Ok(());
        }
        let start_location = start_block_height * BlockIndex::SIZE;
        let required = BlockIndex::SIZE * block_count as u64;
        let index_file = self.ensure_index_file()?;
        let file_len = index_file.try_io(|f| f.metadata().map(|meta| meta.len()))?;
        if start_location + required > file_len {
            return Err(Error::OutOfBoundsBlockRead {
                start_block_height,
                block_count,
            });
        }
        index_file.try_io(|file| {
            file.seek(SeekFrom::Start(start_location))?;
            let mut buffer = [0; 8];
            for current in dest_buffer.iter_mut() {
                *current = BlockIndex::read(file, &mut buffer)?;
            }
            Ok(())
        })?;
        Ok(())
    }
    /// Call `read_block_indices` with a buffer of one.
    ///
    /// # Errors
    /// IO Error.
    pub(crate) fn read_block_index(&mut self, block_height: u64) -> Result<BlockIndex> {
        let mut index = BlockIndex {
            start: 0,
            length: 0,
        };
        self.read_block_indices(block_height, std::slice::from_mut(&mut index))?;
        Ok(index)
    }
    /// Get the number of indices in the index file, which is
    /// calculated as the size of the index file in bytes divided by
    /// `2*size_of(u64)`.
    ///
    /// # Errors
    /// IO Error.
    ///
    /// The most common reason this function fails is
    /// that you did not call `create_files_if_they_do_not_exist`.
    ///
    /// Note that if there is an error, you can be quite sure all
    /// other read and write operations will also fail.
    #[allow(clippy::integer_division)]
    fn read_index_count_from_len(&mut self) -> Result<u64> {
        let index_file = self.ensure_index_file()?;
        let len = index_file.try_io(|file| file.metadata().map(|meta| meta.len()))?;
        Ok(len / BlockIndex::SIZE)
    }
    /// Return the logical index count based on the index file length.
    ///
    /// # Errors
    /// Returns any underlying IO errors when reading the index file metadata.
    #[allow(clippy::integer_division)]
    pub fn read_index_count(&mut self) -> Result<u64> {
        self.read_index_count_from_len()
    }
    /// Read pipeline recovery metadata for a canonical persisted block.
    ///
    /// This read-only tooling path uses Kura's current indexed-sidecar layout and
    /// returns metadata only when its embedded height and block hash match the
    /// canonical block journals. Missing, malformed, or stale sidecars return
    /// `Ok(None)`.
    ///
    /// # Errors
    /// Returns an error when the canonical block journals cannot be read.
    pub fn read_pipeline_metadata(
        &mut self,
        height: u64,
    ) -> Result<Option<PipelineRecoverySidecar>> {
        if height == 0 || height > self.read_index_count()? {
            return Ok(None);
        }
        let pipeline_dir = self.path_to_blockchain.join(PIPELINE_DIR_NAME);
        let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
        let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);
        let entry_byte_limit =
            u64::try_from(MAX_MERGE_EXECUTION_CERTIFIED_SOURCE_BYTES).unwrap_or(u64::MAX);
        let Some(sidecar) = Kura::read_indexed_sidecar_from_paths_with_recovery_and_limit(
            height,
            &data_path,
            &index_path,
            norito::decode_canonical::<PipelineRecoverySidecar>,
            "pipeline sidecar",
            false,
            entry_byte_limit,
        ) else {
            return Ok(None);
        };
        if sidecar.height != height {
            return Ok(None);
        }
        let expected_hash = self
            .read_block_hashes(height.saturating_sub(1), 1)?
            .into_iter()
            .next();
        Ok((expected_hash == Some(sidecar.block_hash)).then_some(sidecar))
    }
    /// Return the durable index count as recorded by the commit marker.
    ///
    /// # Errors
    /// Returns any durable marker/journal integrity or underlying I/O error.
    pub(crate) fn read_durable_index_count(&mut self) -> Result<u64> {
        self.read_exact_durable_index_count()
    }
    /// Read and validate the exact durable boundary without repair or fallback.
    ///
    /// This rejects partial journals, missing/malformed/non-canonical commit
    /// markers, marker/cache divergence, and a marker tip that does not match
    /// the hash journal. It deliberately does not inspect or promote a temp
    /// marker because authorization must bind the already-published boundary.
    fn read_exact_durable_index_count(&mut self) -> Result<u64> {
        if self.path_to_blockchain.as_os_str().is_empty() {
            return Ok(0);
        }
        let index_len = self.index_file_len()?;
        if index_len % BlockIndex::SIZE != 0 {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "block index journal has a partial trailing entry",
                ),
                self.path_to_blockchain.join(INDEX_FILE_NAME),
            ));
        }
        let hashes_len = self.hashes_file_len()?;
        if hashes_len % SIZE_OF_BLOCK_HASH != 0 {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "block hash journal has a partial trailing entry",
                ),
                self.path_to_blockchain.join(HASHES_FILE_NAME),
            ));
        }
        let index_count = index_len / BlockIndex::SIZE;
        let hashes_count = hashes_len / SIZE_OF_BLOCK_HASH;
        let marker_path = self.commit_marker_path();
        let Some(marker_bytes) = Kura::read_regular_sidecar_bytes_for(
            &self.path_to_blockchain,
            &marker_path,
            &self.path_to_blockchain,
            MAX_BLOCK_COMMIT_MARKER_BYTES,
        )?
        else {
            return Err(Error::IO(
                std::io::Error::new(ErrorKind::NotFound, "block commit marker is missing"),
                marker_path,
            ));
        };
        let marker =
            norito::decode_canonical::<BlockStoreCommitMarker>(&marker_bytes).map_err(|error| {
                match error {
                    norito::Error::NonCanonicalEncoding => Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "block commit marker is non-canonical or structurally invalid",
                        ),
                        marker_path.clone(),
                    ),
                    other => Error::NoritoFrame(other),
                }
            })?;
        if marker.version != BlockStoreCommitMarker::VERSION
            || (marker.count == 0) != marker.tip_hash.is_none()
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "block commit marker is non-canonical or structurally invalid",
                ),
                marker_path,
            ));
        }
        if marker.count != self.commit_marker_count {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "published block commit marker differs from the opened durable boundary",
                ),
                marker_path,
            ));
        }
        if marker.count > index_count || marker.count > hashes_count {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "block commit marker exceeds a canonical journal",
                ),
                marker_path,
            ));
        }
        if marker.count > 0 {
            let actual_tip = self
                .read_block_hashes(marker.count.saturating_sub(1), 1)?
                .first()
                .copied();
            if actual_tip != marker.tip_hash {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "block commit marker tip differs from the canonical hash journal",
                    ),
                    marker_path,
                ));
            }
        }
        Ok(marker.count)
    }
    /// Read a series of block hashes from the block hashes file
    ///
    /// # Errors
    /// Returns an out-of-bounds error for an overflowing or unavailable range,
    /// or the original allocation or journal I/O error.
    pub(crate) fn read_block_hashes(
        &mut self,
        start_block_height: u64,
        block_count: usize,
    ) -> Result<Vec<HashOf<BlockHeader>>> {
        let hashes_file = self.ensure_hashes_file()?;
        let file_len = hashes_file.try_io(|file| file.metadata().map(|meta| meta.len()))?;
        let (start_location, _) =
            checked_block_hash_read_range(start_block_height, block_count, file_len)?;
        let mut hashes = Vec::new();
        hashes.try_reserve(block_count)?;
        hashes_file.try_io(|file| {
            file.seek(SeekFrom::Start(start_location))?;
            for _ in 0..block_count {
                let mut buffer = [0; Hash::LENGTH];
                file.read_exact(&mut buffer)?;
                if buffer[Hash::LENGTH - 1] & 1 == 0 {
                    return Err(std::io::Error::new(
                        ErrorKind::InvalidData,
                        "block hash journal entry lacks the canonical marker bit",
                    ));
                }
                hashes.push(HashOf::from_untyped_unchecked(Hash::prehashed(buffer)));
            }
            Ok(())
        })?;
        Ok(hashes)
    }
    /// Get the number of hashes in the hashes file, which is
    /// calculated as the size of the hashes file in bytes divided by
    /// `size_of(HashOf<BlockHeader>)`.
    ///
    /// # Errors
    /// IO Error.
    ///
    /// The most common reason this function fails is
    /// that you did not call `create_files_if_they_do_not_exist`.
    #[allow(clippy::integer_division)]
    pub(crate) fn read_hashes_count(&mut self) -> Result<u64> {
        let hashes_file = self.ensure_hashes_file()?;
        let len = hashes_file.try_io(|file| file.metadata().map(|meta| meta.len()))?;
        Ok(len / SIZE_OF_BLOCK_HASH)
    }
    fn truncate_hashes_to_count(&mut self, count: u64) -> Result<()> {
        if self.path_to_blockchain.as_os_str().is_empty() {
            return Ok(());
        }
        let hashes_file = self.ensure_hashes_file()?;
        let new_len = count.saturating_mul(SIZE_OF_BLOCK_HASH);
        let current_len = hashes_file.try_io(|file| file.metadata().map(|meta| meta.len()))?;
        if new_len < current_len {
            hashes_file.try_io(|file| file.set_len(new_len))?;
        }
        Ok(())
    }
    fn data_end_for_index_prefix(&mut self, count: u64) -> Result<u64> {
        let mut data_end = 0u64;
        for index_pos in 0..count {
            let index = self.read_block_index(index_pos)?;
            if index.is_evicted() {
                continue;
            }
            let end = index
                .start
                .checked_add(index.length)
                .ok_or(Error::CorruptedBlockRange {
                    start: index.start,
                    length: index.length,
                    data_len: self.data_file_len()?,
                })?;
            data_end = data_end.max(end);
        }
        Ok(data_end)
    }
    fn truncate_data_to_index(&mut self, count: u64) -> Result<()> {
        if self.path_to_blockchain.as_os_str().is_empty() {
            return Ok(());
        }
        if count == 0 {
            self.invalidate_data_mmap();
            let data_file = self.ensure_data_file()?;
            data_file.try_io(|file| file.set_len(0))?;
            return Ok(());
        }
        let target_len = match self.data_end_for_index_prefix(count) {
            Ok(index) => index,
            Err(err) => {
                warn!(
                    ?err,
                    count, "failed to read block indices while trimming data file"
                );
                return Ok(());
            }
        };
        let current_len = {
            let data_file = self.ensure_data_file()?;
            data_file.try_io(|file| file.metadata().map(|meta| meta.len()))?
        };
        if current_len > target_len {
            self.invalidate_data_mmap();
            let data_file = self.ensure_data_file()?;
            data_file.try_io(|file| file.set_len(target_len))?;
        }
        Ok(())
    }
    /// Return the current size of the block data file in bytes.
    ///
    /// # Errors
    /// Propagates I/O errors when the metadata cannot be read.
    pub(crate) fn data_file_len(&mut self) -> Result<u64> {
        let data_file = self.ensure_data_file()?;
        data_file.try_io(|file| file.metadata().map(|meta| meta.len()))
    }
    /// Return the current size of the block index file in bytes.
    ///
    /// # Errors
    /// Propagates I/O errors when the metadata cannot be read.
    pub(crate) fn index_file_len(&mut self) -> Result<u64> {
        let index_file = self.ensure_index_file()?;
        index_file.try_io(|file| file.metadata().map(|meta| meta.len()))
    }
    /// Return the current size of the block hashes file in bytes.
    ///
    /// # Errors
    /// Propagates I/O errors when the metadata cannot be read.
    pub(crate) fn hashes_file_len(&mut self) -> Result<u64> {
        let hashes_file = self.ensure_hashes_file()?;
        hashes_file.try_io(|file| file.metadata().map(|meta| meta.len()))
    }
    /// Read block data starting from the
    /// `start_location_in_data_file` in data file in order to fill
    /// `dest_buffer`.
    ///
    /// # Errors
    /// IO Error.
    pub fn read_block_data(
        &mut self,
        start_location_in_data_file: u64,
        dest_buffer: &mut [u8],
    ) -> Result<()> {
        #[cfg(test)]
        {
            self.body_read_calls.fetch_add(1, Ordering::Relaxed);
            self.body_bytes_read
                .fetch_add(u64::try_from(dest_buffer.len())?, Ordering::Relaxed);
        }
        let data_file = self.ensure_data_file()?;
        data_file.try_io(|file| {
            file.seek(SeekFrom::Start(start_location_in_data_file))?;
            file.read_exact(dest_buffer)
        })?;
        Ok(())
    }
    /// Write the index of a single block at the specified `block_height`.
    /// If `block_height` is beyond the end of the index file, attempt to
    /// extend the index file.
    ///
    /// # Errors
    /// IO Error.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub(crate) fn write_block_index(
        &mut self,
        block_height: u64,
        start: u64,
        length: u64,
    ) -> Result<()> {
        let index_file = self.ensure_index_file()?;
        let start_location = block_height * BlockIndex::SIZE;
        let new_len = start_location + BlockIndex::SIZE;
        let current_len = index_file.try_io(|file| file.metadata().map(|meta| meta.len()))?;
        if new_len > current_len {
            index_file.try_io(|file| file.set_len(new_len))?;
        }
        index_file.try_io(|file| {
            file.seek(SeekFrom::Start(start_location))?;
            let bytes = BlockIndex { start, length }.encode();
            file.write_all(&bytes)
        })?;
        self.schedule_fsync_after_write()?;
        Ok(())
    }
    /// Change the size of the index file (the value returned by
    /// `read_index_count`).
    ///
    /// # Errors
    /// IO Error.
    ///
    /// The most common reason this function fails is
    /// that you did not call `create_files_if_they_do_not_exist`.
    ///
    /// Note that if there is an error, you can be quite sure all other
    /// read and write operations will also fail.
    #[cfg(test)]
    pub(crate) fn write_index_count(&mut self, new_count: u64) -> Result<()> {
        let index_file = self.ensure_index_file()?;
        let new_byte_size = new_count * BlockIndex::SIZE;
        index_file.try_io(|file| file.set_len(new_byte_size))?;
        Ok(())
    }
    /// Write `block_data` into the data file starting at
    /// `start_location_in_data_file`. Extend the file if
    /// necessary.
    ///
    /// # Errors
    /// IO Error.
    #[cfg(test)]
    pub(crate) fn write_block_data(
        &mut self,
        start_location_in_data_file: u64,
        block_data: &[u8],
    ) -> Result<()> {
        self.invalidate_data_mmap();
        let data_file = self.ensure_data_file()?;
        let end = start_location_in_data_file + block_data.len() as u64;
        let current_len = data_file.try_io(|file| file.metadata().map(|meta| meta.len()))?;
        if end > current_len {
            data_file.try_io(|file| file.set_len(end))?;
        }
        data_file.try_io(|file| {
            file.seek(SeekFrom::Start(start_location_in_data_file))?;
            file.write_all(block_data)
        })?;
        self.schedule_fsync_after_write()?;
        Ok(())
    }
    /// Write the hash of a single block at the specified `block_height`.
    /// If `block_height` is beyond the end of the index file, attempt to
    /// extend the index file.
    ///
    /// # Errors
    /// IO Error.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub(crate) fn write_block_hash(
        &mut self,
        block_height: u64,
        hash: HashOf<BlockHeader>,
    ) -> Result<()> {
        let hashes_file = self.ensure_hashes_file()?;
        let start_location = block_height * SIZE_OF_BLOCK_HASH;
        let end = start_location + SIZE_OF_BLOCK_HASH;
        let current_len = hashes_file.try_io(|file| file.metadata().map(|meta| meta.len()))?;
        if end > current_len {
            hashes_file.try_io(|file| file.set_len(end))?;
        }
        hashes_file.try_io(|file| {
            file.seek(SeekFrom::Start(start_location))?;
            file.write_all(hash.as_ref())
        })?;
        self.schedule_fsync_after_write()?;
        Ok(())
    }
    /// Write the hashes to the hashes file overwriting any previous hashes.
    ///
    /// # Errors
    /// IO Error.
    pub(crate) fn overwrite_block_hashes(&mut self, hashes: &[HashOf<BlockHeader>]) -> Result<()> {
        let hashes_file = self.ensure_hashes_file()?;
        hashes_file.try_io(|file| {
            file.set_len(0)?;
            file.seek(SeekFrom::Start(0))
        })?;
        hashes_file.try_io(|file| {
            let mut writer = BufWriter::new(&mut *file);
            for hash in hashes {
                writer.write_all(hash.as_ref())?;
            }
            writer.flush()
        })?;
        self.schedule_fsync_after_write()?;
        Ok(())
    }
    /// Rewrite a suffix of the hashes file while preserving every preceding byte.
    ///
    /// `start_block_height` is the zero-based hash index at which `hashes` begins.
    /// The existing journal must already contain the entire prefix. This is used by
    /// startup recovery when that prefix is protected by durable finality.
    fn overwrite_block_hash_suffix(
        &mut self,
        start_block_height: u64,
        hashes: &[HashOf<BlockHeader>],
    ) -> Result<()> {
        let path = self.path_to_blockchain.join(HASHES_FILE_NAME);
        let start_location = start_block_height
            .checked_mul(SIZE_OF_BLOCK_HASH)
            .ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidInput,
                        "hash suffix start offset overflowed",
                    ),
                    path.clone(),
                )
            })?;
        let suffix_count = u64::try_from(hashes.len())?;
        let new_count = start_block_height
            .checked_add(suffix_count)
            .ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(ErrorKind::InvalidInput, "hash suffix count overflowed"),
                    path.clone(),
                )
            })?;
        let new_len = new_count.checked_mul(SIZE_OF_BLOCK_HASH).ok_or_else(|| {
            Error::IO(
                std::io::Error::new(ErrorKind::InvalidInput, "hash suffix length overflowed"),
                path.clone(),
            )
        })?;
        let hashes_file = self.ensure_hashes_file()?;
        hashes_file.try_io(|file| {
            let current_len = file.metadata()?.len();
            if current_len < start_location {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    "hashes journal does not contain the finalized prefix",
                ));
            }
            file.seek(SeekFrom::Start(start_location))?;
            let mut writer = BufWriter::new(&mut *file);
            for hash in hashes {
                writer.write_all(hash.as_ref())?;
            }
            writer.flush()?;
            drop(writer);
            file.set_len(new_len)
        })?;
        self.schedule_fsync_after_write()?;
        Ok(())
    }
    fn require_existing_journal_bound_canonical_files(&self) -> Result<()> {
        for name in [
            INDEX_FILE_NAME,
            DATA_FILE_NAME,
            HASHES_FILE_NAME,
            COUNT_FILE_NAME,
        ] {
            let path = self.path_to_blockchain.join(name);
            if Kura::regular_sidecar_metadata_for(
                &self.path_to_blockchain,
                &path,
                &self.path_to_blockchain,
            )?
            .is_none()
            {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::NotFound,
                        "journal-selected canonical binding is missing a required store file",
                    ),
                    path,
                ));
            }
        }
        Ok(())
    }
    /// Open an existing canonical journal without recovery or repair.
    ///
    /// This is the first half of signed-lineage snapshot startup. It may set
    /// only process-local file handles and the cached durable count; every
    /// durable byte remains unchanged until snapshot authentication succeeds.
    fn initialize_provisional_snapshot_bootstrap_read_only(
        &mut self,
        audited_prefix_height: usize,
    ) -> Result<u64> {
        self.require_existing_journal_bound_canonical_files()?;
        for path in [
            self.da_block_rewrite_stage_path(),
            self.eviction_compaction_stage_path(),
            self.commit_marker_path().with_extension("norito.tmp"),
        ] {
            if std::fs::symlink_metadata(&path).is_ok() {
                return Err(Error::InvalidSnapshotBootstrapMarker {
                    path,
                    reason: "unresolved canonical transaction requires recovery before provisional snapshot opening"
                        .to_owned(),
                });
            }
        }
        let index_len = self.index_file_len()?;
        let hashes_len = self.hashes_file_len()?;
        if index_len % BlockIndex::SIZE != 0 || hashes_len % SIZE_OF_BLOCK_HASH != 0 {
            return Err(Error::HashesFileHeightMismatch);
        }
        let index_count = index_len / BlockIndex::SIZE;
        let hashes_count = hashes_len / SIZE_OF_BLOCK_HASH;
        if index_count != hashes_count || index_count < u64::try_from(audited_prefix_height)? {
            return Err(Error::HashesFileHeightMismatch);
        }
        let marker_path = self.commit_marker_path();
        let marker_bytes = Kura::read_regular_sidecar_bytes_for(
            &self.path_to_blockchain,
            &marker_path,
            &self.path_to_blockchain,
            MAX_VERIFIED_SNAPSHOT_TAIL_MARKER_BYTES,
        )?
        .ok_or_else(|| {
            Error::IO(
                std::io::Error::new(
                    ErrorKind::NotFound,
                    "canonical block commit marker is missing",
                ),
                marker_path.clone(),
            )
        })?;
        let marker =
            norito::decode_canonical::<BlockStoreCommitMarker>(&marker_bytes).map_err(|error| {
                Error::InvalidSnapshotBootstrapMarker {
                    path: marker_path.clone(),
                    reason: format!("failed to decode canonical block marker: {error}"),
                }
            })?;
        if marker.version != BlockStoreCommitMarker::VERSION
            || (marker.count == 0) != marker.tip_hash.is_none()
            || marker.count != index_count
        {
            return Err(Error::InvalidSnapshotBootstrapMarker {
                path: marker_path,
                reason: format!(
                    "canonical marker does not exactly bind the index/hash height {index_count}"
                ),
            });
        }
        self.validate_commit_marker_tip(&marker, hashes_count)?;
        self.commit_marker_count = marker.count;
        self.commit_marker_pending = None;
        Ok(index_count)
    }
    /// Open the prefix accepted by [`Self::preflight_fast_durable_prefix`] read-only.
    ///
    /// Fast mode leaves all published and unpublished bytes untouched. A later Strict restart owns
    /// journal-tail reconciliation and any required file creation.
    fn open_fast_prevalidated_files_read_only(&mut self) -> Result<()> {
        let count = self.fast_prevalidated_count.ok_or_else(|| {
            Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "Kura Fast files were opened without committed-prefix preflight",
                ),
                self.path_to_blockchain.clone(),
            )
        })?;
        self.data_file = Some(FileWrap::open_read_only(
            self.path_to_blockchain.join(DATA_FILE_NAME),
        )?);
        self.index_file = Some(FileWrap::open_read_only(
            self.path_to_blockchain.join(INDEX_FILE_NAME),
        )?);
        self.hashes_file = Some(FileWrap::open_read_only(
            self.path_to_blockchain.join(HASHES_FILE_NAME),
        )?);

        let marker_path = self.commit_marker_path();
        let marker = match Self::read_bounded_commit_marker_bytes(&marker_path)? {
            Some(bytes) => norito::decode_canonical::<BlockStoreCommitMarker>(&bytes)
                .map_err(Error::NoritoFrame)?,
            None => {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::NotFound,
                        "Kura Fast committed-prefix marker disappeared after preflight",
                    ),
                    marker_path,
                ));
            }
        };
        if marker.version != BlockStoreCommitMarker::VERSION
            || marker.count != count
            || (marker.count == 0) != marker.tip_hash.is_none()
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "Kura Fast committed-prefix marker changed after preflight",
                ),
                marker_path,
            ));
        }

        self.commit_marker_count = count;
        self.commit_marker_pending = None;
        if self.read_exact_durable_index_count()? != count {
            return Err(Error::HashesFileHeightMismatch);
        }
        Ok(())
    }
    /// Create the index and data files if they do not
    /// already exist.
    ///
    /// This public entry point is for constructing an offline store. Live-node storage must be
    /// initialized through [`Kura`], which owns the process lock and recovery authority.
    ///
    /// # Errors
    /// Fails if any of the files don't exist and couldn't be
    /// created.
    pub fn create_files_if_they_do_not_exist(&mut self) -> Result<()> {
        std::fs::create_dir_all(&*self.path_to_blockchain)
            .map_err(|e| Error::MkDir(e, self.path_to_blockchain.clone()))?;
        for name in [INDEX_FILE_NAME, DATA_FILE_NAME, HASHES_FILE_NAME] {
            let path = self.path_to_blockchain.join(name);
            FileWrap::open_with(path, |opts| {
                opts.write(true).truncate(false).create(true);
            })?;
        }
        self.drop_cached_handles();
        self.recover_canonical_storage_stages()?;
        self.init_commit_marker()?;
        self.drop_cached_handles();
        Ok(())
    }
    /// Append `block_data` to this block store. First write
    /// the data to the data file and then create a new index
    /// for it in the index file.
    ///
    /// This public entry point is for offline store construction. Live nodes must append through
    /// [`Kura::store_block`] so canonical mutation authorization cannot be bypassed.
    ///
    /// # Errors
    /// Fails if any of the required platform-specific functions
    /// fail.
    pub fn append_block_to_chain(&mut self, block: &SignedBlock) -> Result<()> {
        // Delegate to the batch writer to share fsync/pending logic.
        self.append_block_batch(&[Arc::new(block.clone())])
    }
    /// Append multiple blocks to the chain in a single I/O batch.
    ///
    /// This method mirrors [`Self::append_block_to_chain`] but avoids repeated
    /// `sync_data` calls when persisting a contiguous run of blocks.
    ///
    /// # Errors
    /// Propagates I/O and encoding errors.
    pub(crate) fn append_block_batch(&mut self, blocks: &[Arc<SignedBlock>]) -> Result<()> {
        let start_height = self.read_index_count()?;
        self.append_block_batch_at(start_height, blocks, 0)
    }
    #[allow(clippy::too_many_lines)]
    fn append_block_batch_at(
        &mut self,
        start_height: u64,
        blocks: &[Arc<SignedBlock>],
        max_disk_usage_bytes: u64,
    ) -> Result<()> {
        if blocks.is_empty() {
            return Ok(());
        }
        self.recover_canonical_storage_stages()?;
        self.invalidate_data_mmap();
        debug!(
            start_height,
            batch_len = blocks.len(),
            "append_block_batch start"
        );
        let start_location_in_data_file = if start_height == 0 {
            0
        } else {
            let mut idx = start_height.saturating_sub(1);
            loop {
                let entry = self.read_block_index(idx)?;
                if !entry.is_evicted() {
                    break entry.start.checked_add(entry.length).ok_or(
                        Error::CorruptedBlockRange {
                            start: entry.start,
                            length: entry.length,
                            data_len: entry.start,
                        },
                    )?;
                }
                if idx == 0 {
                    break 0;
                }
                idx = idx.saturating_sub(1);
            }
        };
        debug!(
            start_height,
            start_location_in_data_file, "append_block_batch computed start location"
        );
        let mut frames = Vec::with_capacity(blocks.len());
        let mut lengths = Vec::with_capacity(blocks.len());
        let mut offsets = Vec::with_capacity(blocks.len());
        let mut hashes = Vec::with_capacity(blocks.len());
        for (idx, block) in blocks.iter().enumerate() {
            debug!(
                start_height,
                block_idx = idx,
                "append_block_batch encoding block"
            );
            let wire = block.canonical_wire()?;
            let frame = wire.into_vec();
            let frame_len = u64::try_from(frame.len())?;
            frames.push(frame);
            lengths.push(frame_len);
            hashes.push(block.hash());
            debug!(
                start_height,
                block_idx = idx,
                frame_len,
                "append_block_batch encoded block"
            );
        }
        debug!(
            start_height,
            frames = frames.len(),
            "append_block_batch prepared frames"
        );
        let mut cursor = start_location_in_data_file;
        let mut evicted = Vec::with_capacity(blocks.len());
        for (idx, len) in lengths.iter().enumerate() {
            let block_count_after = start_height
                .saturating_add(u64::try_from(idx)?)
                .saturating_add(1);
            let metadata_bytes =
                block_count_after.saturating_mul(BlockIndex::SIZE + SIZE_OF_BLOCK_HASH);
            let projected_inline_bytes = cursor.saturating_add(*len).saturating_add(metadata_bytes);
            let evict = max_disk_usage_bytes > 0 && projected_inline_bytes > max_disk_usage_bytes;
            evicted.push(evict);
            if evict {
                offsets.push(EVICTED_BLOCK_START);
                debug!(
                    start_height,
                    block_idx = idx,
                    frame_len = *len,
                    projected_inline_bytes,
                    max_disk_usage_bytes,
                    "append_block_batch sidecarring block body"
                );
                continue;
            }
            offsets.push(cursor);
            cursor = cursor.checked_add(*len).ok_or(Error::CorruptedBlockRange {
                start: cursor,
                length: *len,
                data_len: cursor,
            })?;
        }
        let end_pos = cursor;
        debug!(
            start_height,
            start_location = start_location_in_data_file,
            end_pos,
            frames = frames.len(),
            "append_block_batch preparing to write data"
        );
        let rewrite_stage = self.prepare_da_block_rewrite_stage(
            start_height,
            &frames,
            &offsets,
            &lengths,
            &hashes,
        )?;
        let journal_result = (|| -> Result<()> {
            let data_file = self.ensure_data_file()?;
            data_file.try_io(|file| {
                debug!(
                    start_height,
                    "append_block_batch writing frames to data file"
                );
                file.seek(SeekFrom::Start(start_location_in_data_file))?;
                for (frame, evict) in frames.iter().zip(evicted.iter()) {
                    if *evict {
                        continue;
                    }
                    file.write_all(frame)?;
                }
                debug!(start_height, "append_block_batch flushing data file");
                file.flush()?;
                Ok(())
            })?;
            debug!(
                start_height,
                start_location = start_location_in_data_file,
                end_pos,
                frames = frames.len(),
                "append_block_batch wrote data"
            );
            data_file.try_io(|file| {
                file.seek(SeekFrom::Start(end_pos))?;
                file.set_len(end_pos)
            })?;
            let hashes_file = self.ensure_hashes_file()?;
            let start_location = start_height * SIZE_OF_BLOCK_HASH;
            let new_hashes_len = start_location + SIZE_OF_BLOCK_HASH * u64::try_from(blocks.len())?;
            hashes_file.try_io(|file| file.set_len(new_hashes_len))?;
            hashes_file.try_io(|file| {
                file.seek(SeekFrom::Start(start_location))?;
                for hash in &hashes {
                    file.write_all(hash.as_ref())?;
                }
                file.flush()?;
                Ok(())
            })?;
            debug!(
                start_height,
                new_hashes_len, "append_block_batch wrote hashes"
            );
            // Write the index after data + hashes so the commit marker can safely advance.
            let index_file = self.ensure_index_file()?;
            let new_index_len = (start_height + blocks.len() as u64) * BlockIndex::SIZE;
            index_file.try_io(|file| {
                file.seek(SeekFrom::Start(start_height * BlockIndex::SIZE))?;
                for (start, len) in offsets.iter().zip(lengths.iter()) {
                    let bytes = BlockIndex {
                        start: *start,
                        length: *len,
                    }
                    .encode();
                    file.write_all(&bytes)?;
                }
                file.flush()?;
                file.set_len(new_index_len)?;
                Ok(())
            })?;
            debug!(
                start_height,
                new_index_len, "append_block_batch wrote index entries"
            );
            Ok(())
        })();
        if let Err(error) = journal_result {
            if rewrite_stage.is_some() {
                return Err(self.rollback_da_rewrite_before_returning(error));
            }
            return Err(error);
        }
        let end_height = start_height + blocks.len() as u64;
        self.commit_marker_pending = Some(
            self.commit_marker_pending
                .map_or(end_height, |pending| pending.max(end_height)),
        );
        self.mark_fsync_pending();
        if let Some(stage) = rewrite_stage.as_ref() {
            #[cfg(test)]
            if self
                .crash_next_da_rewrite_before_marker
                .swap(false, Ordering::AcqRel)
            {
                return Err(Error::IO(
                    std::io::Error::other(
                        "simulated crash after DA journal writes and before marker publication",
                    ),
                    self.da_block_rewrite_stage_path(),
                ));
            }
            let publication_result = (|| -> Result<()> {
                #[cfg(test)]
                if self
                    .fail_next_da_rewrite_before_marker
                    .swap(false, Ordering::AcqRel)
                {
                    return Err(Error::IO(
                        std::io::Error::other(
                            "injected DA rewrite failure after staging and before commit marker",
                        ),
                        self.da_block_rewrite_stage_path(),
                    ));
                }
                self.sync_target(FsyncTarget::Data, Self::ensure_data_file)?;
                self.sync_target(FsyncTarget::Hashes, Self::ensure_hashes_file)?;
                self.sync_target(FsyncTarget::Index, Self::ensure_index_file)?;
                self.write_commit_marker_value(&stage.new_marker)
            })();
            if let Err(publication_error) = publication_result {
                let marker = match self.read_commit_marker() {
                    Ok(marker) => marker,
                    Err(marker_error) => {
                        self.commit_marker_pending = None;
                        self.fsync.clear();
                        return Err(self.unknown_da_rewrite_state(
                            "commit-marker acknowledgement and readback both failed",
                            &marker_error,
                        ));
                    }
                };
                if marker.as_ref() == Some(&stage.new_marker) {
                    if let Err(recovery_error) = self.recover_da_block_rewrite_stage() {
                        self.defer_da_block_rewrite_recovery(&recovery_error);
                    }
                    debug!(
                        start_height,
                        end_height,
                        "append_block_batch committed despite publication acknowledgement error"
                    );
                    return Ok(());
                }
                if marker.as_ref() == Some(&stage.old_marker) {
                    return Err(self.rollback_da_rewrite_before_returning(publication_error));
                }
                self.commit_marker_pending = None;
                self.fsync.clear();
                return Err(self.unknown_da_rewrite_state(
                    "commit marker matches neither staged state after publication failure",
                    &publication_error,
                ));
            }
            self.commit_marker_count = stage.new_marker.count;
            self.commit_marker_pending = None;
            self.fsync.clear();
            #[cfg(test)]
            if self
                .crash_next_da_rewrite_after_marker
                .swap(false, Ordering::AcqRel)
            {
                return Err(Error::IO(
                    std::io::Error::other(
                        "simulated crash after DA marker publication and before body promotion",
                    ),
                    self.da_block_rewrite_stage_path(),
                ));
            }
            #[cfg(test)]
            if self
                .fail_next_da_rewrite_after_marker
                .swap(false, Ordering::AcqRel)
            {
                let injected_error = Error::IO(
                    std::io::Error::other(
                        "injected DA rewrite failure after commit marker publication",
                    ),
                    self.da_block_rewrite_stage_path(),
                );
                warn!(
                    ?injected_error,
                    "DA rewrite marker is durable; recovering staged body promotion in-call"
                );
                if let Err(recovery_error) = self.recover_da_block_rewrite_stage() {
                    self.defer_da_block_rewrite_recovery(&recovery_error);
                }
            } else if let Err(promotion_error) = self.promote_new_da_block_rewrite_stage(stage) {
                warn!(
                    ?promotion_error,
                    "DA rewrite marker is durable; retrying staged body promotion"
                );
                if let Err(recovery_error) = self.recover_da_block_rewrite_stage() {
                    self.defer_da_block_rewrite_recovery(&recovery_error);
                }
            } else if let Err(cleanup_error) = self.remove_da_block_rewrite_stage() {
                self.defer_da_block_rewrite_recovery(&cleanup_error);
            }
        } else if matches!(self.fsync.mode, FsyncMode::Always)
            || (matches!(self.fsync.mode, FsyncMode::Batched)
                && self.fsync.interval == Duration::ZERO)
        {
            self.flush_pending_fsync(false)?;
        }
        debug!(start_height, end_height, "append_block_batch complete");
        Ok(())
    }
}
include!("kura/prune_block_store_tail.rs");
#[cfg(test)]
include!("kura/test_fault_injection_state.rs");
#[cfg(test)]
include!("kura/test_fault_injection_controls.rs");
include!("kura/file_error_support.rs");
#[cfg(test)]
pub(crate) mod tests {
    #[test]
    fn root_storage_frame_owners_roundtrip_and_reject_substitution() {
        fn check<T>(value: &T, nominal: &str) -> T
        where
            T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
        {
            assert_eq!(T::nominal_name(), nominal);
            assert_eq!(T::frame_name(), nominal);
            let frame = norito::encode_canonical(value).expect("encode storage owner");
            assert_eq!(frame[6..22], norito::schema::identity::frame_hash::<T>());
            let decoded = norito::decode_canonical::<T>(&frame).expect("decode storage owner");
            assert_eq!(
                norito::encode_canonical(&decoded).expect("re-encode storage owner"),
                frame
            );
            let mut wrong_owner = frame.clone();
            wrong_owner[6] ^= 1;
            assert!(matches!(
                norito::decode_canonical::<T>(&wrong_owner),
                Err(norito::Error::SchemaMismatch)
            ));
            assert!(norito::decode_canonical::<T>(&frame[..frame.len() - 1]).is_err());
            let mut trailing = frame;
            trailing.push(0);
            assert!(norito::decode_canonical::<T>(&trailing).is_err());
            decoded
        }
        fn file_digest(mut bytes: &[u8]) -> Hash {
            let len = bytes.len() as u64;
            BlockStore::eviction_reader_digest(&mut bytes, len).expect("digest fixture file image")
        }

        let mut blocks = DummyBlocks::new();
        let first = blocks.next();
        let second = blocks.next();
        let first_wire = first.encode_wire().expect("encode first canonical block");
        let second_wire = second.encode_wire().expect("encode second canonical block");
        let marker = BlockStoreCommitMarker::new(2, Some(second.hash()));
        assert_eq!(
            check(&marker, "iroha_core::kura::BlockStoreCommitMarker"),
            marker
        );
        let store = BlockStore::new(Path::new(""));
        let rewrite = DaBlockRewriteStageV1 {
            format_version: DA_BLOCK_REWRITE_STAGE_VERSION,
            old_marker: BlockStoreCommitMarker::new(1, Some(first.hash())),
            new_marker: marker.clone(),
            old_data_len: first_wire.len() as u64,
            old_index_count: 1,
            old_hash_count: 1,
            old_suffix: Vec::new(),
            replacement: vec![DaBlockRewriteImageV1 {
                height: 2,
                block_hash: second.hash(),
                index_start: first_wire.len() as u64,
                index_length: second_wire.len() as u64,
                body: Some(second_wire.clone()),
            }],
        };
        store
            .validate_da_block_rewrite_stage(&rewrite)
            .expect("valid rewrite fixture");
        assert_eq!(
            check(&rewrite, "iroha_core::kura::DaBlockRewriteStageV1"),
            rewrite
        );
        let marker_bytes = norito::encode_canonical(&marker).expect("encode commit marker image");
        let hashes = [first.hash(), second.hash()];
        let hash_bytes: Vec<u8> = hashes
            .iter()
            .flat_map(|hash| hash.as_ref().iter().copied())
            .collect();
        let index_bytes = [
            BlockIndex {
                start: 0,
                length: first_wire.len() as u64,
            }
            .encode(),
            BlockIndex {
                start: EVICTED_BLOCK_START,
                length: second_wire.len() as u64,
            }
            .encode(),
        ]
        .concat();
        let eviction = EvictionCompactionStageV1 {
            format_version: EVICTION_COMPACTION_STAGE_VERSION,
            marker,
            marker_len: marker_bytes.len() as u64,
            marker_digest: file_digest(&marker_bytes),
            hashes_len: hash_bytes.len() as u64,
            hashes_digest: file_digest(&hash_bytes),
            data_temp_name: EVICTION_COMPACTION_DATA_FILE_NAME.to_owned(),
            data_len: first_wire.len() as u64,
            data_digest: file_digest(&first_wire),
            index_temp_name: EVICTION_COMPACTION_INDEX_FILE_NAME.to_owned(),
            index_len: index_bytes.len() as u64,
            index_digest: file_digest(&index_bytes),
            evicted: vec![EvictionCompactionEntryV1 {
                height: 2,
                block_hash: second.hash(),
                canonical_wire_hash: Hash::new(&second_wire),
                wire_len: second_wire.len() as u64,
            }],
        };
        store
            .validate_eviction_compaction_stage(&eviction)
            .expect("valid eviction fixture");
        assert_eq!(
            check(&eviction, "iroha_core::kura::EvictionCompactionStageV1"),
            eviction
        );
        let association = CanonicalAssociationStageV1 {
            format_version: CANONICAL_ASSOCIATION_STAGE_VERSION,
            height: 2,
            block_hash: second.hash(),
            canonical_wire_hash: Hash::new(&second_wire),
            block_wire: second_wire,
            merge_entry: None,
        };
        Kura::blank_kura_for_testing()
            .validate_canonical_association_stage(&association)
            .expect("valid canonical association fixture");
        assert_eq!(
            check(
                &association,
                "iroha_core::kura::CanonicalAssociationStageV1"
            ),
            association
        );
        let snapshot = VerifiedSnapshotTailMarkerV1::new(
            1,
            2,
            verified_snapshot_hash_journal_digest(&hashes).expect("snapshot hash journal digest"),
            Some(Hash::new(b"storage-owner-snapshot-lineage")),
        );
        let decoded = check(&snapshot, "iroha_core::kura::VerifiedSnapshotTailMarkerV1");
        assert_eq!(
            (
                decoded.version,
                decoded.body_prefix_count,
                decoded.snapshot_height,
                decoded.hash_journal_digest,
                decoded.bootstrap_lineage_hash
            ),
            (
                snapshot.version,
                snapshot.body_prefix_count,
                snapshot.snapshot_height,
                snapshot.hash_journal_digest,
                snapshot.bootstrap_lineage_hash
            )
        );
        let rewrite_frame = norito::encode_canonical(&rewrite).expect("encode rewrite owner");
        assert!(matches!(
            norito::decode_canonical::<EvictionCompactionStageV1>(&rewrite_frame),
            Err(norito::Error::SchemaMismatch)
        ));
    }

    fn kaigi_signal_test_call(name: &str) -> iroha_data_model::kaigi::KaigiId {
        iroha_data_model::kaigi::KaigiId::new(
            iroha_model_base::domain::DomainId::try_new("kaigi", "universal").expect("test domain"),
            name.parse().expect("test call name"),
        )
    }

    fn kaigi_signal_test_locator(
        height: usize,
        network_input_index: u32,
    ) -> super::KaigiSignalCandidateLocator {
        use iroha_crypto::{Hash, HashOf};
        use iroha_data_model::{block::BlockHeader, transaction::signed::TransactionEntrypoint};

        let marker = u64::try_from(height)
            .expect("test height fits u64")
            .wrapping_mul(4);
        super::KaigiSignalCandidateLocator {
            position: super::KaigiSignalCandidatePosition {
                block_height: u64::try_from(height).expect("test height fits u64"),
                network_input_index,
                block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                    marker.to_le_bytes(),
                )),
                entrypoint_hash: HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
                    Hash::new((marker ^ u64::from(network_input_index)).to_le_bytes()),
                ),
            },
            authority: iroha_data_model::AccountId::new(
                super::checked_keypair().public_key().clone(),
            ),
        }
    }

    fn insert_kaigi_signal_test_locator(
        index: &mut super::TransactionEntrypointIndex,
        call_id: &iroha_data_model::kaigi::KaigiId,
        locator: super::KaigiSignalCandidateLocator,
    ) {
        let height = usize::try_from(locator.position.block_height)
            .ok()
            .and_then(std::num::NonZeroUsize::new)
            .expect("test locator height is nonzero");
        let inserted = index
            .inventories_by_height
            .entry(height)
            .or_default()
            .kaigi_calls
            .insert(call_id.clone());
        index.nested_associations.inserted(inserted);
        let replaced = index
            .kaigi_signal_candidates
            .entry(call_id.clone())
            .or_default()
            .entry(height)
            .or_default()
            .insert(locator.position.network_input_index, locator);
        index.nested_associations.inserted(replaced.is_none());
    }

    #[test]
    fn kaigi_signal_locator_pages_are_chronological_exclusive_and_call_bound() {
        let call_id = kaigi_signal_test_call("page-order");
        let other_call = kaigi_signal_test_call("other-call");
        let mut index = super::TransactionEntrypointIndex::complete_empty();
        for locator in [
            kaigi_signal_test_locator(1, 1),
            kaigi_signal_test_locator(2, 0),
            kaigi_signal_test_locator(1, 0),
        ] {
            insert_kaigi_signal_test_locator(&mut index, &call_id, locator);
        }
        let first = super::Kura::collect_kaigi_signal_candidate_locators(
            &index,
            &call_id,
            2,
            None,
            std::num::NonZeroUsize::new(2).expect("nonzero page"),
        )
        .expect("first locator page");
        assert!(first.has_more);
        assert_eq!(first.candidates[0].position.network_input_index, 0);
        assert_eq!(first.candidates[1].position.network_input_index, 1);
        let after = first.candidates[1].position;
        let second = super::Kura::collect_kaigi_signal_candidate_locators(
            &index,
            &call_id,
            2,
            Some(after),
            std::num::NonZeroUsize::new(2).expect("nonzero page"),
        )
        .expect("second locator page");
        assert!(!second.has_more);
        assert_eq!(second.candidates.len(), 1);
        assert_eq!(second.candidates[0].position.block_height, 2);
        assert_eq!(
            super::Kura::collect_kaigi_signal_candidate_locators(
                &index,
                &other_call,
                2,
                Some(after),
                std::num::NonZeroUsize::new(1).expect("nonzero page"),
            ),
            Err(super::KaigiSignalCandidateIndexError::CursorMismatch),
        );
        let position = first.candidates[0].position;
        assert!(
            super::KaigiSignalCandidatePosition::new(
                0,
                position.network_input_index,
                position.block_hash,
                position.entrypoint_hash,
            )
            .is_none()
        );
        let foreign = super::KaigiSignalCandidatePosition::new(
            1,
            u32::MAX,
            position.block_hash,
            position.entrypoint_hash,
        )
        .expect("structural position uses the full u32 input range");
        assert_eq!(
            super::Kura::collect_kaigi_signal_candidate_locators(
                &index,
                &call_id,
                2,
                Some(foreign),
                std::num::NonZeroUsize::new(1).unwrap(),
            ),
            Err(super::KaigiSignalCandidateIndexError::CursorMismatch),
        );
    }

    #[test]
    fn kaigi_signal_locator_pages_advance_beyond_five_hundred_carriers() {
        let call_id = kaigi_signal_test_call("long-history");
        let mut index = super::TransactionEntrypointIndex::complete_empty();
        for height in 1..=501 {
            insert_kaigi_signal_test_locator(
                &mut index,
                &call_id,
                kaigi_signal_test_locator(height, 0),
            );
        }
        let first = super::Kura::collect_kaigi_signal_candidate_locators(
            &index,
            &call_id,
            501,
            None,
            std::num::NonZeroUsize::new(500).expect("nonzero page"),
        )
        .expect("bounded first page");
        assert_eq!(first.candidates.len(), 500);
        assert!(first.has_more);
        let second = super::Kura::collect_kaigi_signal_candidate_locators(
            &index,
            &call_id,
            501,
            first.candidates.last().map(|locator| locator.position),
            std::num::NonZeroUsize::new(500).expect("nonzero page"),
        )
        .expect("bounded continuation");
        assert_eq!(second.candidates.len(), 1);
        assert_eq!(second.candidates[0].position.block_height, 501);
        assert!(!second.has_more);
    }

    #[test]
    fn kaigi_signal_reverse_inventories_follow_replacement_and_truncation() {
        let call_id = kaigi_signal_test_call("lifecycle");
        let replacement_call = kaigi_signal_test_call("replacement");
        let mut index = super::TransactionEntrypointIndex::complete_empty();
        for height in 1..=3 {
            let height_key = std::num::NonZeroUsize::new(height).expect("nonzero height");
            index.indexed_heights.insert(height_key);
            insert_kaigi_signal_test_locator(
                &mut index,
                &call_id,
                kaigi_signal_test_locator(height, 0),
            );
        }
        let height_two = std::num::NonZeroUsize::new(2).expect("nonzero height");
        index.incomplete_heights.insert(height_two);
        super::Kura::remove_transaction_entrypoint_height(&mut index, height_two);
        assert!(!index.indexed_heights.contains(&height_two));
        assert!(!index.incomplete_heights.contains(&height_two));
        assert!(
            index.kaigi_signal_candidates[&call_id]
                .get(&height_two)
                .is_none()
        );
        index.indexed_heights.insert(height_two);
        insert_kaigi_signal_test_locator(
            &mut index,
            &replacement_call,
            kaigi_signal_test_locator(2, 7),
        );
        assert!(
            index
                .kaigi_signal_candidates
                .contains_key(&replacement_call)
        );
        super::Kura::truncate_transaction_entrypoint_index_to(&mut index, 1);
        assert_eq!(index.kaigi_signal_candidates[&call_id].len(), 1);
        assert_eq!(
            index.kaigi_signal_candidates[&call_id]
                .first_key_value()
                .map(|(height, _)| height.get()),
            Some(1),
        );
        assert!(
            !index
                .kaigi_signal_candidates
                .contains_key(&replacement_call)
        );
        assert_eq!(index.inventories_by_height.len(), 1);
        assert!(
            index
                .inventories_by_height
                .contains_key(&std::num::NonZeroUsize::new(1).expect("nonzero height"))
        );
    }

    include!("kura/tests/canonical_network_index.rs");
    include!("kura/tests/committed_network_proof_support.rs");
    include!("kura/tests/canonical_network_query_support.rs");
    include!("kura/tests/resident_resource_inventory.rs");

    // Textual includes preserve every test in the existing `kura::tests` namespace.
    include!("kura/tests/00_bounded_sidecar_read_tests.rs");
    include!("kura/tests/01_support_snapshot_bootstrap_and_rewrite.rs");
    include!("kura/tests/02_replacement_and_preflight.rs");
    include!("kura/tests/03_preflight_and_merge_entry.rs");
    include!("kura/tests/04_merge_log_and_associations.rs");
    include!("kura/tests/05_merge_resolution_and_eviction.rs");
    include!("kura/tests/05b_canonical_physical_resource_tests.rs");
    include!("kura/tests/06_eviction_and_autonomous_lanes.rs");
    include!("kura/tests/07e_autonomous_publication_temp_recovery_tests.rs");
    include!("kura/tests/09_lane_artifacts_and_fastpq.rs");
    include!("kura/tests/11_roster_and_progress_sidecars.rs");
    include!("kura/tests/12_sidecar_index_and_pruning.rs");
    include!("kura/tests/13_manifests_and_fsync.rs");
    include!("kura/tests/14_pipeline_and_lane_frame_owners.rs");
    include!("kura/tests/14b_sidecar_physical_resource_tests.rs");
    include!("kura/tests/14a_physical_resource_guard_tests.rs");
    include!("kura/tests/14b_metadata_physical_resource_tests.rs");
    include!("kura/tests/15_remaining_physical_writer_tests.rs");
    include!("kura/tests/16_resource_file_admission_tests.rs");
    #[cfg(all(unix, not(any(target_os = "redox", target_os = "espidf"))))]
    include!("kura/tests/17_read_only_evidence_tests.rs");
    include!("kura/tests/18_snapshot_hash_streaming.rs");
    include!("kura/tests/19_transaction_history_budget.rs");
}
