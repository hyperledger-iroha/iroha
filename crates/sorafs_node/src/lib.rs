//! SoraFS provider-node services and durable protocol implementations.
#![deny(missing_docs)]
#![allow(
    clippy::clone_on_copy,
    clippy::collapsible_if,
    clippy::field_reassign_with_default
)]
pub mod appeal_finance_transaction_forwarder;
pub mod capacity;
pub mod config;
mod durable_transaction_forwarder;
pub mod evidence_viewer;
mod governance;
mod governance_rooted_fs;
pub mod governance_service;
pub mod hedging_billing_service;
pub mod metering;
mod moderation;
pub mod moderation_orchestrator;
mod native_repair_singleflight;
pub mod native_repair_worker;
pub mod orderbook_transaction_forwarder;
pub mod pdp_provider;
pub mod pop_credentials;
pub mod por;
pub mod potr;
pub mod proof_outcome_forwarder;
pub mod provider_attestation_clock;
pub mod provider_attestation_journal;
pub mod provider_attestation_journal_file_store;
pub mod provider_ingest_outbox;
pub mod provider_ingest_runtime;
mod reconciliation;
pub mod repair_ledger_projection;
pub mod repair_transaction_forwarder;
pub mod reputation;
pub mod reserve_transaction_forwarder;
pub mod reserve_transparency_runtime;
pub mod scheduler;
pub mod store;
pub mod telemetry;
mod transparency;
include!("lib/governance_public_exports.rs");
use governance::{
    FilesystemGovernancePublisher, GovernanceFilesystemRootGuard,
    GovernanceRuntimeDagCheckpointStore, GovernanceRuntimeDagSigner,
    qualify_governance_dag_runtime_checkpoint_store,
    qualify_governance_dag_runtime_signer_provider,
};
pub use governance_service::{
    GovernanceDagMirrorReadBindingV1, GovernanceDagMirrorReadHandleV1,
    GovernanceDagMirrorSnapshotV1, GovernanceDagServiceError, GovernanceDagServiceLauncherError,
    GovernanceDagServiceRunner, GovernanceDagServiceRuntimeProviderBindingsV1,
    GovernanceDagServiceRuntimeProviderRegistryErrorV1,
    GovernanceDagServiceRuntimeProviderRegistryV1, GovernanceDagServiceRuntimeProviders,
    prepare_governance_dag_service_from_view, run_governance_dag_service,
    run_governance_dag_service_from_view, run_governance_dag_service_with_runtime_registry,
    validate_governance_dag_service_runtime_providers,
};
pub use iroha_data_model::sorafs::pin_registry::ProviderIngestCompletionSignerPolicyV1;
pub use moderation::{
    MODERATION_READ_VIEW_MAX_RECORDS_V1, MODERATION_SCREENING_ADMISSION_RECEIPT_VERSION_V1,
    MODERATION_SCREENING_AUTHORITY_BUNDLE_VERSION_V1,
    ModerationAuthenticatedScreeningAdmissionError, ModerationAuthenticatedScreeningEvidenceV1,
    ModerationAuthenticatedScreeningOutcomeV1, ModerationAuthenticatedScreeningRequestV1,
    ModerationCorpusRegistryRecord, ModerationEvidenceViewerAccessEventRecord,
    ModerationEvidenceViewerAccessInput, ModerationEvidenceViewerAccessKind,
    ModerationEvidenceViewerAuditKindCount, ModerationEvidenceViewerAuditReport,
    ModerationEvidenceViewerAuditReportInput, ModerationEvidenceViewerError,
    ModerationEvidenceViewerSessionInput, ModerationEvidenceViewerSessionRecord,
    ModerationEvidenceViewerSnapshot, ModerationModelRegistryError,
    ModerationModelRegistryReadView, ModerationModelRegistrySnapshot,
    ModerationQuarantineKeyOperationErrorV1, ModerationQuarantineKeyProviderBindingV1,
    ModerationQuarantineKeyProviderQualificationErrorV1,
    ModerationQuarantineKeyProviderQualificationV1,
    ModerationQuarantineKeyProviderReadinessErrorV1, ModerationQuarantineKeyWrapper,
    ModerationQuarantineObjectError, ModerationQuarantineObjectInput,
    ModerationQuarantineObjectPayload, ModerationQuarantineObjectRangePayload,
    ModerationQuarantineObjectRecord, ModerationQuarantineObjectSnapshot,
    ModerationQuarantineReadView, ModerationQuarantineRecord, ModerationQuarantineReleaseInput,
    ModerationQuarantineReviewInput, ModerationQuarantineState, ModerationReproRegistryRecord,
    ModerationScreeningAdmissionReceiptV1, ModerationScreeningAuthenticationError,
    ModerationScreeningAuthorityBundleV1, ModerationScreeningAuthorityV1, ModerationScreeningError,
    ModerationScreeningInput, ModerationScreeningOutcome, ModerationScreeningReadView,
    ModerationScreeningRecord, ModerationScreeningSnapshot, ModerationScreeningVerdict,
    verify_authenticated_moderation_screening_v1,
};
pub use pdp_provider::{
    PDP_STATUS_EXPORT_MAX_RECORDS_V1, PdpChallengeEnqueueOutcome, PdpChallengeLifecycleV1,
    PdpChallengeStatusV1, PdpGovernanceArchiveV1, PdpNextChallengeV1, PdpProofBuildError,
    PdpProviderProtocol, PdpProviderProtocolError, PdpProviderTelemetrySnapshot,
    PdpRejectionReasonV1, PdpTerminalDecisionV1, PdpTerminalHandoff, PdpTerminalOutcomeV1,
    build_signed_pdp_proof_v1,
};
pub use por::{
    ManifestVrfBundle, ManifestVrfKey, PlannedChallenge, PorChallengePlannerError,
    PorFailedRepairIntentV1, PorFinalizedReplayArchiveAbsenceProofV1,
    PorFinalizedReplayArchiveBindingV1, PorFinalizedReplayArchiveExternalErrorV1,
    PorFinalizedReplayArchiveLookupV1, PorFinalizedReplayArchiveProofBoundsV1,
    PorFinalizedReplayArchiveReadbackV1, PorFinalizedReplayArchiveReceiptV1,
    PorFinalizedReplayArchiveRecordV1, PorFinalizedReplayArchiveV1, PorMutationDispositionV1,
    PorMutationFailureV1, PorPendingRepairWorkV1, PorProtocolMetricsSnapshot, PorRandomness,
    PorRepairHandoff, PorRepairHandoffAckOutcomeV1, PorRepairHandoffError,
    PorRepairReconcileErrorV1, PorRepairReconcileOutcomeV1, PorReputationTerminalAckOutcomeV1,
    PorReputationTerminalWorkV1, PorStatusAuthoritySnapshotV1, PorStatusAuthorityUpdateV1,
    PorTracker, PorTrackerError, PorVerdictStats, build_por_challenge_for_manifest,
    canonical_por_failure_repair_report_v1, por_repair_source_identity_v1,
};
pub use potr::{
    POTR_EXPORT_MAX_RECORDS_V1, POTR_RECEIPT_MAX_CANONICAL_BYTES_V1,
    POTR_TRACKER_CHECKPOINT_FILE_NAME_V1, PotrAdmissionPolicyBindingError,
    PotrAdmissionPolicyBindingV1, PotrAdmissionPolicyProgressError, PotrReceiptStatusV1,
    PotrRecordOutcome, PotrTrackerError,
};
pub use proof_outcome_forwarder::{
    PROOF_OUTCOME_OUTBOX_DEFAULT_MAX_ATTEMPTS_V1, PROOF_OUTCOME_OUTBOX_MAX_SCAN_ITEMS_V1,
    ProofOutcomeDeadLetterReasonV1, ProofOutcomeDeadLetterV1, ProofOutcomeDeliveryStateV1,
    ProofOutcomeEnqueueResultV1, ProofOutcomeOutbox, ProofOutcomeOutboxError,
    ProofOutcomeOutboxPolicyV1, ProofOutcomePendingDeliveryV1, potr_proof_outcome_operation_id_v1,
};
pub use provider_attestation_clock::{
    MUSUBI_PROVIDER_ATTESTATION_CLOCK_SEAL_TIMEOUT_MS_V1, MusubiProviderAttestationClockErrorV1,
    MusubiProviderAttestationClockScopeV1, MusubiProviderAttestationClockSealBindingV1,
    MusubiProviderAttestationClockSealErrorV1, MusubiProviderAttestationClockSealQualificationV1,
    MusubiProviderAttestationClockSealRecordV1, MusubiProviderAttestationClockSealV1,
    MusubiProviderAttestationJournalCheckpointHeadRecordV1,
    MusubiProviderAttestationJournalCheckpointHeadV1,
    MusubiProviderAttestationJournalCheckpointScopeV1,
    MusubiProviderAttestationJournalCheckpointSealErrorV1,
    MusubiProviderAttestationSealedUnixClockV1,
    musubi_provider_attestation_journal_checkpoint_blob_revision_v1,
};
pub use provider_attestation_journal::{
    MUSUBI_PROVIDER_ATTESTATION_EXTERNAL_TIMEOUT_MAX_MS_V1,
    MUSUBI_PROVIDER_ATTESTATION_JOURNAL_CHECKPOINT_MAX_BYTES_V1,
    MUSUBI_PROVIDER_ATTESTATION_JOURNAL_MAX_ENTRIES_V1,
    MUSUBI_PROVIDER_ATTESTATION_READY_PAGE_MAX_V1, MusubiProviderAttestationApprovalClaimV1,
    MusubiProviderAttestationApprovalErrorV1, MusubiProviderAttestationApprovalIdV1,
    MusubiProviderAttestationApprovalIntentV1, MusubiProviderAttestationApprovalStoreOutcomeV1,
    MusubiProviderAttestationClaimOwnerV1, MusubiProviderAttestationDeadLetterReasonV1,
    MusubiProviderAttestationDeliveryOutcomeV1, MusubiProviderAttestationEnqueueOutcomeV1,
    MusubiProviderAttestationFailureClassV1, MusubiProviderAttestationHandoffClaimV1,
    MusubiProviderAttestationInventoryBindingErrorV1, MusubiProviderAttestationInventoryErrorV1,
    MusubiProviderAttestationInventoryHandoffIdV1, MusubiProviderAttestationInventoryItemV1,
    MusubiProviderAttestationInventoryQualificationV1,
    MusubiProviderAttestationInventoryReadbackV1, MusubiProviderAttestationInventoryReaderV1,
    MusubiProviderAttestationInventoryReceiptV1, MusubiProviderAttestationInventoryRuntimeErrorV1,
    MusubiProviderAttestationInventoryRuntimeV1, MusubiProviderAttestationInventoryScopeV1,
    MusubiProviderAttestationInventorySinkV1, MusubiProviderAttestationInventoryV1,
    MusubiProviderAttestationJournalErrorV1, MusubiProviderAttestationJournalPolicyV1,
    MusubiProviderAttestationJournalRuntimeV1, MusubiProviderAttestationJournalScanKeyV1,
    MusubiProviderAttestationJournalStageV1, MusubiProviderAttestationJournalStatusV1,
    MusubiProviderAttestationRetryOutcomeV1, MusubiProviderAttestationSignerBindingErrorV1,
    MusubiProviderAttestationSignerErrorV1, MusubiProviderAttestationSignerQualificationV1,
    MusubiProviderAttestationSignerV1, approve_musubi_provider_attestation_v1,
    musubi_provider_attestation_approval_id_v1,
    musubi_provider_attestation_controller_policy_digest_v1,
    musubi_provider_attestation_inventory_handoff_id_v1,
    validate_musubi_provider_attestation_inventory_binding_v1,
};
pub use provider_attestation_journal_file_store::{
    MusubiProviderAttestationJournalFileBindingV1, MusubiProviderAttestationJournalFileStoreV1,
};
pub use provider_ingest_outbox::{
    FinalizedProviderIngestAuthorizationV1, FinalizedProviderIngestMusubiContextV1,
    PROVIDER_INGEST_OUTBOX_FILE_V1, PROVIDER_INGEST_STATUS_PAGE_MAX_V1,
    ProviderIngestCancellationReasonV1, ProviderIngestCheckpointExternalErrorV1,
    ProviderIngestCheckpointProviderBindingV1, ProviderIngestCheckpointProviderQualificationV1,
    ProviderIngestCheckpointRuntimeV1, ProviderIngestClaimOwnerV1,
    ProviderIngestCompletionSigningClaimV1, ProviderIngestCompletionSigningContextV1,
    ProviderIngestCompletionStateV1, ProviderIngestCompletionSubmissionV1,
    ProviderIngestDeadLetterReasonV1, ProviderIngestDeliveryStateV1, ProviderIngestEnqueueResultV1,
    ProviderIngestFailureClassV1, ProviderIngestFinalizedCancellationV1,
    ProviderIngestFinalizedCompletionV1, ProviderIngestFinalizedCursorV1, ProviderIngestOutbox,
    ProviderIngestOutboxCountsV1, ProviderIngestOutboxError, ProviderIngestOutboxPolicyV1,
    ProviderIngestRetryOutcomeV1, ProviderIngestSealedCheckpointRecordV1,
    ProviderIngestSourceClaimV1, ProviderIngestStatusPageV1, ProviderIngestStatusV1,
};
use provider_ingest_runtime::CompletedMusubiStoreInstanceV1;
pub use provider_ingest_runtime::{
    PROVIDER_INGEST_VERIFIED_MUSUBI_RECEIPT_MAX_CANONICAL_BYTES_V1,
    ProviderIngestAuthenticatedSourceFetchV1, ProviderIngestClockV1,
    ProviderIngestCompletedMusubiAttestationDriveErrorV1,
    ProviderIngestCompletedMusubiAttestationDriveOutcomeV1,
    ProviderIngestCompletedMusubiAttestationDriverV1,
    ProviderIngestCompletedMusubiCaptureCoordinatorV1,
    ProviderIngestCompletedMusubiCaptureRequestV1,
    ProviderIngestCompletedMusubiCaptureSourcePageV1,
    ProviderIngestCompletedMusubiCaptureSourceRowV1,
    ProviderIngestCompletedMusubiCaptureVerifierBindingV1,
    ProviderIngestCompletedMusubiSignedCaptureLedgerV1,
    ProviderIngestCompletedMusubiSignedCapturePageV1, ProviderIngestCompletionPayloadBuilderV1,
    ProviderIngestCompletionPayloadErrorV1, ProviderIngestCompletionPayloadRequestV1,
    ProviderIngestCompletionSignerBindingErrorV1, ProviderIngestCompletionSignerBindingV1,
    ProviderIngestCompletionSignerErrorV1, ProviderIngestCompletionSignerQualificationV1,
    ProviderIngestCompletionSignerResolutionContextV1,
    ProviderIngestCompletionSignerResolverErrorV1, ProviderIngestCompletionSignerResolverV1,
    ProviderIngestCompletionSignerV1, ProviderIngestFinalizedAssignmentPageV1,
    ProviderIngestFinalizedAssignmentV1, ProviderIngestFinalizedClaimFactoryV1,
    ProviderIngestFinalizedLedgerErrorV1, ProviderIngestFinalizedLedgerV1,
    ProviderIngestFinalizedMusubiArchiveClaimV1, ProviderIngestFinalizedMusubiCompletionClaimV1,
    ProviderIngestFutureV1, ProviderIngestIngressDispositionV1,
    ProviderIngestIngressPrepareErrorV1, ProviderIngestLocalStorageErrorV1,
    ProviderIngestLocalStorageV1, ProviderIngestLocalStoredV1,
    ProviderIngestMusubiArchiveFetchBindingV1,
    ProviderIngestMusubiAttestationApprovalRequestErrorV1,
    ProviderIngestMusubiAttestationApprovalRequestV1, ProviderIngestRuntimeErrorV1,
    ProviderIngestRuntimePolicyV1, ProviderIngestRuntimeProviderQualificationV1,
    ProviderIngestRuntimeV1, ProviderIngestSourceFetchErrorV1, ProviderIngestSourceRequestV1,
    ProviderIngestSystemClockV1, ProviderIngestTickOutcomeV1, ProviderIngestTransactionIngressV1,
    ProviderIngestTransactionObservationV1, ProviderIngestVerifiedMusubiBundleReceiptV1,
    provider_ingest_completed_musubi_capture_transcript_digest_v1,
};
use repair_ledger_projection::RepairLedgerTaskProjectionV1;
/// Outcome returned when recording a PoR verdict.
#[derive(Debug, Clone)]
pub struct PorVerdictOutcome {
    /// Statistics extracted from the verdict.
    pub stats: PorVerdictStats,
    /// Chain-authoritative native repair task identifier (failed verdicts only).
    pub repair_task_id: Option<[u8; 32]>,
    /// Current consecutive failure streak for this provider/manifest.
    pub consecutive_failures: u64,
    /// Replay-stable terminal work retained for durable reputation admission.
    pub reputation_work: PorReputationTerminalWorkV1,
}
/// One step of durable PoR-to-reputation reconciliation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PorReputationReconcileOutcomeV1 {
    /// No unacknowledged terminal remains.
    Idle,
    /// The exact terminal was durably admitted and its node cursor advanced.
    Reconciled {
        /// Retained work admitted by the reputation runtime.
        work: Box<PorReputationTerminalWorkV1>,
        /// Durable native-outbox admission result.
        admission: reputation::runtime::ReputationJournalEnqueueOutcomeV1,
        /// Durable node acknowledgement result.
        acknowledgement: PorReputationTerminalAckOutcomeV1,
    },
}
/// Failure from one PoR-to-reputation reconciliation step.
#[derive(Debug, Error)]
pub enum PorReputationReconcileErrorV1 {
    /// Retained PoR work or its durable acknowledgement failed.
    #[error(transparent)]
    Tracker(#[from] PorTrackerError),
    /// The native reputation outbox rejected or could not persist admission.
    #[error(transparent)]
    Admission(#[from] reputation::runtime::ReputationRuntimeError),
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
/// Summary of an eviction action performed during GC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcEviction {
    /// Manifest identifier evicted by the sweep.
    pub manifest_id: String,
    /// Manifest digest evicted.
    pub manifest_digest: [u8; 32],
    /// Retention epoch recorded on the manifest.
    pub retention_epoch: u64,
    /// Bytes freed by the eviction.
    pub freed_bytes: u64,
    /// Reason label associated with the eviction.
    pub reason: String,
}
/// Summary of a manifest skipped during a GC sweep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcSkip {
    /// Manifest identifier that was skipped.
    pub manifest_id: String,
    /// Reason label describing why the manifest was skipped.
    pub reason: String,
}
/// Summary of a GC sweep execution.
#[derive(Debug, Clone, Default)]
pub struct GcSweepReport {
    /// Evictions performed during the sweep.
    pub evictions: Vec<GcEviction>,
    /// Manifests skipped during the sweep.
    pub skipped: Vec<GcSkip>,
    /// Total bytes freed by evictions.
    pub freed_bytes: u64,
    /// Number of errors encountered during the sweep.
    pub errors: u32,
}
/// Result of deriving and recording a payload-free evidence-viewer audit report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationEvidenceViewerAuditReportOutcome {
    /// Canonical payload-free report derived from local evidence-viewer audit records.
    pub report: ModerationEvidenceViewerAuditReport,
    /// Transparency source entry recorded for later ledger/Governance DAG publication.
    pub source_entry: TransparencyLedgerSourceEntry,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GcIntentDisposition {
    DiscardedPreDomain,
    FinalizedCommitted,
}
#[derive(Debug)]
struct GcEvictionTransactionOutcome {
    freed_bytes: u64,
    publish_error: Option<String>,
}
const MODERATION_MODEL_REGISTRY_DIR: &str = "moderation-model-registry";
const MODERATION_MODEL_REGISTRY_SNAPSHOT_FILE: &str = "registry-snapshot.to";
const MODERATION_SCREENING_DIR: &str = "moderation-screening";
const MODERATION_SCREENING_SNAPSHOT_FILE: &str = "screening-snapshot.to";
const MODERATION_QUARANTINE_OBJECT_STORE_DIR: &str = "moderation-quarantine-objects";
const MODERATION_QUARANTINE_OBJECT_INDEX_FILE: &str = "object-index.to";
const MODERATION_QUARANTINE_OBJECT_STORE_MAX_DEPTH: usize = 4;
const MODERATION_EVIDENCE_VIEWER_DIR: &str = "moderation-evidence-viewer";
const MODERATION_EVIDENCE_VIEWER_SNAPSHOT_FILE: &str = "evidence-viewer-snapshot.to";
const AUX_RUNTIME_STATE_DIR: &str = "runtime-state";
const AUX_RUNTIME_STATE_SNAPSHOT_FILE: &str = "auxiliary-snapshot-v5.to";
const RETIRED_AUX_RUNTIME_STATE_SNAPSHOT_FILE_V1: &str = "auxiliary-snapshot.to";
const RETIRED_AUX_RUNTIME_STATE_SNAPSHOT_FILE_V2: &str = "auxiliary-snapshot-v2.to";
const RETIRED_AUX_RUNTIME_STATE_SNAPSHOT_FILE_V3: &str = "auxiliary-snapshot-v3.to";
const RETIRED_AUX_RUNTIME_STATE_SNAPSHOT_FILE_V4: &str = "auxiliary-snapshot-v4.to";
const RUNTIME_STATE_INITIALIZATION_FILE: &str = "initialized-v5";
const RETIRED_RUNTIME_STATE_INITIALIZATION_FILE_V1: &str = "initialized-v1";
const RETIRED_RUNTIME_STATE_INITIALIZATION_FILE_V2: &str = "initialized-v2";
const RETIRED_RUNTIME_STATE_INITIALIZATION_FILE_V3: &str = "initialized-v3";
const RETIRED_RUNTIME_STATE_INITIALIZATION_FILE_V4: &str = "initialized-v4";
const RUNTIME_STATE_INITIALIZATION_BYTES: &[u8] = b"sorafs.node.runtime-state.initialized.v5\n";
// V5 is a first-release hard cut: it adds authenticated governance provenance
// to retained source events, publish receipts, outbox entries, and the complete
// bounded PoR status/repair projection. V1/V2/V3/V4
// artifacts are rejected and must be explicitly reseeded; no field-default or
// heuristic migration is accepted.
const AUX_RUNTIME_STATE_VERSION_V5: u8 = 5;
const ADMITTED_REPUTATION_SNAPSHOT_VERSION_V1: u8 = 1;
const GOVERNANCE_OUTBOX_VERSION_V3: u8 = 3;
const GOVERNANCE_OUTBOX_BINDING_DOMAIN_V3: &[u8] = b"sorafs.node.governance_outbox.binding.v3";
const GC_EVICTION_INTENT_VERSION_V1: u8 = 1;
const GC_EVICTION_AUDIT_LINK_VERSION_V1: u8 = 1;
const GC_EVICTION_RESERVED_OUTBOX_SLOTS: u8 = 1;
const GC_EVICTION_INTENT_BINDING_DOMAIN_V1: &[u8] = b"sorafs.node.gc.eviction_intent.binding.v1";
const GC_EVICTION_AUDIT_LINK_BINDING_DOMAIN_V1: &[u8] =
    b"sorafs.node.gc.eviction_audit_link.binding.v1";
const GC_STORAGE_MANIFEST_IDENTITY_DOMAIN_V1: &[u8] =
    b"sorafs.node.gc.storage_manifest.identity.v1";
const GC_STORAGE_MANIFEST_SET_DOMAIN_V1: &[u8] = b"sorafs.node.gc.manifest_set.identity.v1";
const GC_STORAGE_CHUNK_REFCOUNTS_DOMAIN_V1: &[u8] = b"sorafs.node.gc.chunk_refcounts.identity.v1";
const LOCAL_RUNTIME_SNAPSHOT_TMP_EXT: &str = "tmp";
static LOCAL_CHECKPOINT_WRITE_LOCK: Mutex<()> = Mutex::new(());
const EVIDENCE_VIEWER_AUDIT_CYCLE_ID_DOMAIN_V1: &[u8] =
    b"sorafs.node.moderation.evidence_viewer_audit.cycle_id.v1";
const PRIVACY_AGGREGATE_ENTRY_ID_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.privacy_aggregate.entry_id.v1";
#[cfg(test)]
use crate::metering::ReplicationUsageSample;
use crate::{
    capacity::CapacityRuntimeCheckpointV1,
    metering::{CapacityMeter, MeteringSnapshot},
    moderation::{
        MODERATION_SCREENING_AUTHORITY_BUNDLE_MAX_BYTES_V1, ModerationEvidenceViewerRuntime,
        ModerationModelRegistry, ModerationQuarantineObjectEnvelopeV1,
        ModerationQuarantineObjectRuntime, ModerationScreeningRuntime,
        moderation_quarantine_object_relative_path, normalize_moderation_quarantine_object_input,
        open_moderation_quarantine_object, open_moderation_quarantine_object_range,
        rewrap_moderation_quarantine_object, seal_moderation_quarantine_object,
        validate_moderation_quarantine_key_wrapper, validate_quarantine_object_envelope,
        validate_relative_object_path,
    },
    potr::PotrTracker,
    scheduler::{SchedulerAdmissionError, StorageSchedulerConfig, StorageSchedulersRuntime},
    store::{
        AdmittedPayloadReadLeaseV1, ChunkFileRecord, ChunkRefcountEntry, ChunkRoleMetadata,
        StorageBackend, StorageError, StoredManifest,
    },
    telemetry::{TelemetryAccumulator, TelemetryError},
};
use capacity::{
    CapacityError, CapacityFinalizedCursorV1, CapacityManager, CapacityReconcileModeV1,
    CapacityReconciliationOutcomeV1, CapacityUsageSnapshot,
};
#[cfg(test)]
use capacity::{
    DeclarationWindow, FinalizedReplicationBindingV1, ReplicationPlan, ReplicationRelease,
};
use config::{GcConfig, RepairConfig, StorageConfig};
#[cfg(test)]
use iroha_data_model::sorafs::pin_registry::{PinManifestFinalizedRecordV1, PinStatus};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    da::ingest::DaStripeLayout,
    sorafs::{
        capacity::{CapacityDeclarationRecord, ProviderId},
        gar::GarEnforcementReceiptV1,
        moderation::{AdversarialCorpusManifestV1, ModerationReproManifestV1},
        moderation_ledger::{
            REPAIR_LEDGER_MAX_LEASE_MS_V1, REPAIR_LEDGER_MIN_LEASE_MS_V1, RepairFinalizedCursorV1,
        },
        orderbook::OrderbookFinalizedCursorV1,
        pin_registry::ReplicationOrderRecord,
        reserve::ReserveFinalizedCursorV1,
        transparency::{
            MODERATION_LEDGER_MAX_PUBLIC_TEXT_BYTES_V1, ModerationLedgerCyclePublicationV1,
            ModerationLedgerEntryKindV1, ModerationLedgerEntryV1, ModerationLedgerMetadataV1,
            ModerationPrivacyAggregateV1, ProofTokenIssuanceV1,
        },
    },
};
use iroha_telemetry::metrics::{
    global_or_default, global_sorafs_gc_otel, global_sorafs_reconciliation_otel,
};
use norito::codec::Encode as NoritoEncode;
use norito::derive::{NoritoDeserialize, NoritoSerialize};
#[cfg(test)]
use norito::json::Value as JsonValue;
use orderbook_transaction_forwarder::{
    ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1, OrderbookOperationV1,
    OrderbookTransactionContextV1, OrderbookTransactionDeadLetterV1,
    OrderbookTransactionEnqueueResultV1, OrderbookTransactionForwarder,
    OrderbookTransactionForwarderError, OrderbookTransactionForwarderPolicyV1,
    OrderbookTransactionPendingV1, OrderbookTransactionSigningRequestV1,
};
use rand::{rand_core::TryRngCore as _, rngs::OsRng};
use repair_transaction_forwarder::{
    REPAIR_TRANSACTION_MAX_CANONICAL_BYTES_V1, RepairOperationV1, RepairTransactionContextV1,
    RepairTransactionDeadLetterV1, RepairTransactionEnqueueResultV1, RepairTransactionForwarder,
    RepairTransactionForwarderError, RepairTransactionForwarderPolicyV1,
    RepairTransactionPendingV1, RepairTransactionSigningRequestV1,
};
use reserve_transaction_forwarder::{
    RESERVE_TRANSACTION_MAX_CANONICAL_BYTES_V1, ReserveOperationV1, ReserveTransactionContextV1,
    ReserveTransactionDeadLetterV1, ReserveTransactionEnqueueResultV1, ReserveTransactionForwarder,
    ReserveTransactionForwarderError, ReserveTransactionForwarderPolicyV1,
    ReserveTransactionPendingV1, ReserveTransactionReconciliationV1,
    ReserveTransactionSigningRequestV1,
};
#[cfg(test)]
use sorafs_car::compute_chunk_plan_digest_sha3;
use sorafs_car::{CarBuildPlan, PorProof};
use sorafs_manifest::hedging::signed::{
    HedgingFeedTrustPolicyV1, MAX_HEDGING_TRUST_POLICY_BYTES, decode_hedging_feed_trust_policy,
};
#[cfg(test)]
use sorafs_manifest::repair::GC_AUDIT_REASON_RETENTION_EXPIRED_PROVIDER_MISSING_V1;
use sorafs_manifest::reputation::signed::{
    MAX_REPUTATION_TRUST_POLICY_ENCODED_BYTES, decode_reputation_trust_policy,
    decode_signed_reputation_snapshot,
};
use sorafs_manifest::{
    AdmissionRecord, AppealFinanceReconciliationSummaryV1, ManifestV1,
    ReconciliationValidationError, ReputationScoringEvidenceV1, ReputationSnapshotEventV1,
    ReputationSnapshotTrustPolicyV1, ReputationSnapshotV1, SORAFS_RECONCILIATION_REPORT_VERSION_V1,
    SignedReputationSnapshotV1, SoraFsAppealFinanceReportV1,
    SoraFsAppealFinanceSettlementReceiptV1, SoraFsAppealFinanceWeeklyRollupV1,
    SoraFsModerationBallotGovernanceEventV1, SorafsReconciliationReportV1,
    capacity::CapacityTelemetryV1,
    deal::{DealSettlementV1, XorQuantity},
    governance_dag_submission_account_digest_v1,
    por::{
        AuditOutcomeV1, AuditVerdictV1, PorChallengePublicationV1, PorChallengeV1, PorProofV1,
        PorWeeklyReportV1, decode_por_challenge_publication_v1, decode_por_weekly_report_v1,
    },
    potr::PotrReceiptV1,
    proof_stream::ProofStreamTier,
    repair::{
        GC_AUDIT_BLOCKED_DEAL_ACTIVE_V1, GC_AUDIT_BLOCKED_REPAIR_ACTIVE_V1,
        GC_AUDIT_BLOCKED_SHARED_CHUNKS_V1, GC_AUDIT_EVENT_VERSION_V1, GC_AUDIT_PAYLOAD_VERSION_V1,
        GC_AUDIT_REASON_RETENTION_EXPIRED_V1, GC_AUDIT_SIGNER_V1, GcAuditEventV1, GcAuditPayloadV1,
        RepairReportV1, SorafsAuditHeaderV1, gc_audit_payload_digest_v1,
    },
    validate_reputation_snapshot_transition,
};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    fs,
    io::{self, ErrorKind, Read, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tokio::sync::broadcast;
pub use transparency::moderation_ballot_governance_event_source_entry;
pub use transparency::{
    PRIVACY_AGGREGATE_MAX_POPULATIONS_V1, PRIVACY_AGGREGATE_MAX_SOURCE_EVENTS_V1,
    PRIVACY_CYCLE_PRF_REQUEST_VERSION_V1, PrivacyAggregateCycleConfig, PrivacyAggregateCycleWindow,
    PrivacyAggregateMetricSchemaV1, PrivacyAggregatePopulationV1, PrivacyAggregateScheduleConfig,
    PrivacyAggregateSourceEvent, PrivacyAggregateSourceMetric, PrivacyCompositionBudgetChainV1,
    PrivacyCompositionBudgetChargeV1, PrivacyCompositionBudgetError,
    PrivacyCompositionBudgetLedgerV1, PrivacyCompositionBudgetPolicyV1,
    PrivacyCyclePrfInputErrorV1, PrivacyCyclePrfInputV1, PrivacyCyclePrfOutputV1,
    PrivacyCyclePrfProviderErrorV1, PrivacyCyclePrfProviderV1, PrivacyCyclePrfRequestErrorV1,
    PrivacyCyclePrfRequestV1, PrivacyReleaseAnchorErrorV1, PrivacyReleaseAnchorHeadV1,
    PrivacyReleaseAnchorV1, PrivacySourceEventRecordOutcomeV1, ProductionPrivacyCyclePrfProviderV1,
    ProductionPrivacyReleaseAnchorV1, ProductionTransparencyLeaderLeaseProviderV1,
    ProductionTransparencyRuntimeProviderV1, ProofTokenIssuanceIngestError,
    QualifiedPrivacyCyclePrfProviderV1, QualifiedPrivacyReleaseAnchorV1,
    QualifiedTransparencyLeaderLeaseProviderV1, TRANSPARENCY_LEADER_LEASE_VERSION_V1,
    TransparencyLeaderLeaseAcquireRequestV1, TransparencyLeaderLeaseErrorV1,
    TransparencyLeaderLeaseGrantV1, TransparencyLeaderLeaseProviderErrorV1,
    TransparencyLeaderLeaseProviderV1, TransparencyLeaderLeaseReleaseReceiptV1,
    TransparencyLeaderLeaseReleaseRequestV1, TransparencyLeaderLeaseRenewRequestV1,
    TransparencyLeaderLeaseScopeV1, TransparencyLedgerIngestError, TransparencyLedgerSourceEntry,
    TransparencyRuntimeProviderBindingV1, TransparencyRuntimeProviderQualificationErrorV1,
    TransparencyRuntimeProviderQualificationV1, appeal_finance_report_source_entry,
    appeal_finance_settlement_receipt_source_entry, gar_enforcement_receipt_source_entry,
    moderation_evidence_viewer_audit_report_source_entry, privacy_aggregate_cycle_id,
    privacy_metric_schema_digest, privacy_population_inventory_digest,
    proof_token_issuance_from_base64, proof_token_issuance_from_frame,
    reserve_finalized_event_source_entry,
};
fn privacy_aggregate_entry_id(
    cycle_id: [u8; 16],
    aggregate_hash: [u8; 32],
    aggregate_id: &str,
) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(PRIVACY_AGGREGATE_ENTRY_ID_DOMAIN_V1);
    hasher.update(&cycle_id);
    hasher.update(&aggregate_hash);
    hasher.update(aggregate_id.as_bytes());
    let digest = hasher.finalize();
    let mut entry_id = [0u8; 16];
    entry_id.copy_from_slice(&digest.as_bytes()[..16]);
    entry_id
}
#[cfg(test)]
fn validate_finalized_provider_payload(
    authorization: &FinalizedProviderIngestAuthorizationV1,
    manifest: &ManifestV1,
    plan: &CarBuildPlan,
) -> Result<(), FinalizedProviderIngestError> {
    let manifest_digest = manifest
        .digest()
        .map_err(|error| FinalizedProviderIngestError::ManifestEncoding(error.to_string()))?;
    let chunker_handle = format!(
        "{}.{}@{}",
        manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
    );
    if manifest_digest.as_bytes() != &authorization.manifest_digest()
        || manifest.root_cid.as_slice() != authorization.manifest_cid()
        || chunker_handle != authorization.chunker_handle()
        || manifest.chunk_digest_sha3_256 != authorization.chunk_digest_sha3_256()
        || manifest.por_root != authorization.por_root()
        || manifest.content_length != authorization.content_length()
    {
        return Err(FinalizedProviderIngestError::BindingMismatch(
            "manifest bytes disagree with finalized authorization",
        ));
    }
    if plan.content_length != authorization.content_length()
        || compute_chunk_plan_digest_sha3(&plan.chunks) != authorization.chunk_digest_sha3_256()
        || u32::try_from(plan.chunk_profile.min_size).ok() != Some(manifest.chunking.min_size)
        || u32::try_from(plan.chunk_profile.target_size).ok() != Some(manifest.chunking.target_size)
        || u32::try_from(plan.chunk_profile.max_size).ok() != Some(manifest.chunking.max_size)
        || u32::try_from(plan.chunk_profile.break_mask).ok() != Some(manifest.chunking.break_mask)
    {
        return Err(FinalizedProviderIngestError::BindingMismatch(
            "CAR plan disagrees with finalized manifest",
        ));
    }
    Ok(())
}
fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
fn collect_due_evidence_viewer_audit_window(
    schedule: &PrivacyAggregateScheduleConfig,
    latest_window: PrivacyAggregateCycleWindow,
    published_cycles: &BTreeSet<[u8; 16]>,
    timestamp_unix_ms: u64,
    due_windows: &mut BTreeSet<PrivacyAggregateCycleWindow>,
) -> Result<(), GovernancePublishError> {
    let timestamp_unix = timestamp_unix_ms / 1_000;
    let window = schedule.event_window(timestamp_unix).map_err(|err| {
        GovernancePublishError::other(format!("evidence viewer audit schedule: {err}"))
    })?;
    if window.cycle_end_unix > latest_window.cycle_end_unix
        || window.due_at_unix > latest_window.due_at_unix
    {
        return Ok(());
    }
    if published_cycles.contains(&evidence_viewer_audit_cycle_id(window)) {
        return Ok(());
    }
    due_windows.insert(window);
    Ok(())
}
fn evidence_viewer_audit_cycle_id(window: PrivacyAggregateCycleWindow) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(EVIDENCE_VIEWER_AUDIT_CYCLE_ID_DOMAIN_V1);
    hasher.update(&window.cycle_start_unix.to_le_bytes());
    hasher.update(&window.cycle_end_unix.to_le_bytes());
    hasher.update(&window.due_at_unix.to_le_bytes());
    let digest = hasher.finalize();
    let mut cycle_id = [0u8; 16];
    cycle_id.copy_from_slice(&digest.as_bytes()[..16]);
    cycle_id
}
fn moderation_model_registry_checkpoint_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join(MODERATION_MODEL_REGISTRY_DIR)
        .join(MODERATION_MODEL_REGISTRY_SNAPSHOT_FILE)
}
fn moderation_screening_checkpoint_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join(MODERATION_SCREENING_DIR)
        .join(MODERATION_SCREENING_SNAPSHOT_FILE)
}
fn moderation_quarantine_object_store_root(data_dir: &Path) -> PathBuf {
    data_dir.join(MODERATION_QUARANTINE_OBJECT_STORE_DIR)
}
fn moderation_quarantine_object_index_path(data_dir: &Path) -> PathBuf {
    moderation_quarantine_object_store_root(data_dir).join(MODERATION_QUARANTINE_OBJECT_INDEX_FILE)
}
fn moderation_evidence_viewer_checkpoint_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join(MODERATION_EVIDENCE_VIEWER_DIR)
        .join(MODERATION_EVIDENCE_VIEWER_SNAPSHOT_FILE)
}
fn auxiliary_runtime_checkpoint_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join(AUX_RUNTIME_STATE_DIR)
        .join(AUX_RUNTIME_STATE_SNAPSHOT_FILE)
}
fn runtime_state_initialization_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join(AUX_RUNTIME_STATE_DIR)
        .join(RUNTIME_STATE_INITIALIZATION_FILE)
}
fn retired_runtime_state_initialization_path_v1(data_dir: &Path) -> PathBuf {
    data_dir
        .join(AUX_RUNTIME_STATE_DIR)
        .join(RETIRED_RUNTIME_STATE_INITIALIZATION_FILE_V1)
}
fn retired_runtime_state_initialization_path_v2(data_dir: &Path) -> PathBuf {
    data_dir
        .join(AUX_RUNTIME_STATE_DIR)
        .join(RETIRED_RUNTIME_STATE_INITIALIZATION_FILE_V2)
}
fn retired_runtime_state_initialization_path_v3(data_dir: &Path) -> PathBuf {
    data_dir
        .join(AUX_RUNTIME_STATE_DIR)
        .join(RETIRED_RUNTIME_STATE_INITIALIZATION_FILE_V3)
}
fn retired_runtime_state_initialization_path_v4(data_dir: &Path) -> PathBuf {
    data_dir
        .join(AUX_RUNTIME_STATE_DIR)
        .join(RETIRED_RUNTIME_STATE_INITIALIZATION_FILE_V4)
}
fn retired_auxiliary_runtime_checkpoint_path_v1(data_dir: &Path) -> PathBuf {
    data_dir
        .join(AUX_RUNTIME_STATE_DIR)
        .join(RETIRED_AUX_RUNTIME_STATE_SNAPSHOT_FILE_V1)
}
fn retired_auxiliary_runtime_checkpoint_path_v2(data_dir: &Path) -> PathBuf {
    data_dir
        .join(AUX_RUNTIME_STATE_DIR)
        .join(RETIRED_AUX_RUNTIME_STATE_SNAPSHOT_FILE_V2)
}
fn retired_auxiliary_runtime_checkpoint_path_v3(data_dir: &Path) -> PathBuf {
    data_dir
        .join(AUX_RUNTIME_STATE_DIR)
        .join(RETIRED_AUX_RUNTIME_STATE_SNAPSHOT_FILE_V3)
}
fn retired_auxiliary_runtime_checkpoint_path_v4(data_dir: &Path) -> PathBuf {
    data_dir
        .join(AUX_RUNTIME_STATE_DIR)
        .join(RETIRED_AUX_RUNTIME_STATE_SNAPSHOT_FILE_V4)
}
fn required_runtime_checkpoint_paths(data_dir: &Path) -> [(&'static str, PathBuf); 5] {
    [
        (
            "moderation model registry",
            moderation_model_registry_checkpoint_path(data_dir),
        ),
        (
            "moderation screening",
            moderation_screening_checkpoint_path(data_dir),
        ),
        (
            "moderation quarantine object index",
            moderation_quarantine_object_index_path(data_dir),
        ),
        (
            "moderation evidence viewer",
            moderation_evidence_viewer_checkpoint_path(data_dir),
        ),
        (
            "auxiliary runtime",
            auxiliary_runtime_checkpoint_path(data_dir),
        ),
    ]
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeCheckpointInitialization {
    Fresh,
    Initialized,
}
fn inspect_runtime_checkpoint_initialization(
    data_dir: &Path,
    checkpoint_max_bytes: u64,
) -> Result<RuntimeCheckpointInitialization, NodeInitError> {
    let marker_path = runtime_state_initialization_path(data_dir);
    for (version, retired_marker_path) in [
        (1, retired_runtime_state_initialization_path_v1(data_dir)),
        (2, retired_runtime_state_initialization_path_v2(data_dir)),
        (3, retired_runtime_state_initialization_path_v3(data_dir)),
        (4, retired_runtime_state_initialization_path_v4(data_dir)),
    ] {
        if read_local_checkpoint_bounded(
            &retired_marker_path,
            u64::try_from(RUNTIME_STATE_INITIALIZATION_BYTES.len()).unwrap_or(u64::MAX),
        )
        .map_err(|err| {
            NodeInitError::checkpoint(
                "retired runtime initialization marker",
                &retired_marker_path,
                err,
            )
        })?
        .is_some()
        {
            return Err(NodeInitError::checkpoint(
                "retired runtime initialization marker",
                &retired_marker_path,
                format!(
                    "SoraFS development runtime-state v{version} is not supported; discard and reseed the local state directory"
                ),
            ));
        }
    }
    for (version, retired_checkpoint_path) in [
        (1, retired_auxiliary_runtime_checkpoint_path_v1(data_dir)),
        (2, retired_auxiliary_runtime_checkpoint_path_v2(data_dir)),
        (3, retired_auxiliary_runtime_checkpoint_path_v3(data_dir)),
        (4, retired_auxiliary_runtime_checkpoint_path_v4(data_dir)),
    ] {
        if read_local_checkpoint_bounded(&retired_checkpoint_path, checkpoint_max_bytes)
            .map_err(|err| {
                NodeInitError::checkpoint(
                    "retired auxiliary runtime checkpoint",
                    &retired_checkpoint_path,
                    err,
                )
            })?
            .is_some()
        {
            return Err(NodeInitError::checkpoint(
                "retired auxiliary runtime checkpoint",
                &retired_checkpoint_path,
                format!(
                    "SoraFS development runtime-state v{version} is not supported; discard and reseed the local state directory"
                ),
            ));
        }
    }
    let marker = read_local_checkpoint_bounded(
        &marker_path,
        u64::try_from(RUNTIME_STATE_INITIALIZATION_BYTES.len()).unwrap_or(u64::MAX),
    )
    .map_err(|err| NodeInitError::checkpoint("runtime initialization marker", &marker_path, err))?;
    match marker {
        Some(bytes) => {
            if bytes != RUNTIME_STATE_INITIALIZATION_BYTES {
                return Err(NodeInitError::checkpoint(
                    "runtime initialization marker",
                    &marker_path,
                    "marker contents are not canonical for runtime-state v5",
                ));
            }
            for (component, path) in required_runtime_checkpoint_paths(data_dir) {
                if read_local_checkpoint_bounded(&path, checkpoint_max_bytes)
                    .map_err(|err| NodeInitError::checkpoint(component, &path, err))?
                    .is_none()
                {
                    return Err(NodeInitError::checkpoint(
                        component,
                        &path,
                        "checkpoint is missing after runtime initialization",
                    ));
                }
            }
            Ok(RuntimeCheckpointInitialization::Initialized)
        }
        None => {
            for (component, path) in required_runtime_checkpoint_paths(data_dir) {
                if read_local_checkpoint_bounded(&path, checkpoint_max_bytes)
                    .map_err(|err| NodeInitError::checkpoint(component, &path, err))?
                    .is_some()
                {
                    return Err(NodeInitError::checkpoint(
                        "runtime initialization marker",
                        &marker_path,
                        format!(
                            "marker is missing while {component} checkpoint `{}` exists",
                            path.display()
                        ),
                    ));
                }
            }
            Ok(RuntimeCheckpointInitialization::Fresh)
        }
    }
}
#[derive(Debug)]
struct LocalCheckpointWriteError {
    error: io::Error,
    committed: bool,
}
impl LocalCheckpointWriteError {
    fn precommit(error: io::Error) -> Self {
        Self {
            error,
            committed: false,
        }
    }
    fn committed(error: io::Error) -> Self {
        Self {
            error,
            committed: true,
        }
    }
}
impl std::fmt::Display for LocalCheckpointWriteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.committed {
            write!(
                formatter,
                "checkpoint rename committed but durability is uncertain: {}",
                self.error
            )
        } else {
            self.error.fmt(formatter)
        }
    }
}
impl std::error::Error for LocalCheckpointWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}
impl From<io::Error> for LocalCheckpointWriteError {
    fn from(error: io::Error) -> Self {
        Self::precommit(error)
    }
}
#[derive(Debug, Error)]
#[error("{message}")]
struct RuntimeCheckpointPersistError {
    message: String,
    committed: bool,
}
impl RuntimeCheckpointPersistError {
    fn precommit(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            committed: false,
        }
    }
}
fn write_local_checkpoint_atomic(
    path: &Path,
    bytes: &[u8],
) -> Result<(), LocalCheckpointWriteError> {
    write_local_checkpoint_atomic_with_mode(path, bytes, false)
}
fn write_local_checkpoint_atomic_bounded(
    path: &Path,
    bytes: &[u8],
    max_bytes: u64,
) -> Result<(), LocalCheckpointWriteError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(LocalCheckpointWriteError::precommit(io::Error::other(
            format!(
                "checkpoint `{}` is {} bytes, exceeding limit {max_bytes}",
                path.display(),
                bytes.len()
            ),
        )));
    }
    write_local_checkpoint_atomic(path, bytes)
}
fn write_local_private_checkpoint_atomic(
    path: &Path,
    bytes: &[u8],
) -> Result<(), LocalCheckpointWriteError> {
    write_local_checkpoint_atomic_with_mode(path, bytes, true)
}
fn write_local_checkpoint_atomic_with_mode(
    path: &Path,
    bytes: &[u8],
    _private: bool,
) -> Result<(), LocalCheckpointWriteError> {
    write_local_checkpoint_atomic_with_mode_and_parent_sync(
        path,
        bytes,
        _private,
        sync_local_checkpoint_parent,
    )
}
fn sync_local_checkpoint_parent(parent: &Path) -> io::Result<()> {
    let directory = fs::File::open(parent)?;
    directory.sync_all()
}
fn remove_local_checkpoint_file_durably(path: &Path) -> io::Result<()> {
    let path = absolute_local_checkpoint_path(path)?;
    reject_unsafe_checkpoint_ancestors(&path)?;
    let metadata = fs::symlink_metadata(&path)?;
    validate_local_checkpoint_file_metadata(&path, &metadata)?;
    fs::remove_file(&path)?;
    if let Some(parent) = path.parent() {
        sync_local_checkpoint_parent(parent)?;
    }
    Ok(())
}
fn write_local_checkpoint_atomic_with_mode_and_parent_sync(
    path: &Path,
    bytes: &[u8],
    _private: bool,
    parent_sync: fn(&Path) -> io::Result<()>,
) -> Result<(), LocalCheckpointWriteError> {
    let _writer_guard = LOCAL_CHECKPOINT_WRITE_LOCK.lock().map_err(|_| {
        LocalCheckpointWriteError::precommit(io::Error::other(
            "local checkpoint writer lock is poisoned",
        ))
    })?;
    let path = absolute_local_checkpoint_path(path)?;
    let path = path.as_path();
    reject_unsafe_checkpoint_ancestors(path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        reject_unsafe_checkpoint_ancestors(path)?;
    }
    reject_unsafe_checkpoint_target(path)?;
    let tmp_path = local_checkpoint_tmp_path(path)?;
    let result: Result<(), LocalCheckpointWriteError> = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        set_local_no_follow_flag(&mut options);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(&tmp_path)?;
        file.write_all(bytes)?;
        set_local_private_file_permissions(&tmp_path)?;
        file.sync_all()?;
        reject_unsafe_checkpoint_ancestors(path)?;
        reject_unsafe_checkpoint_target(path)?;
        fs::rename(&tmp_path, path)?;
        if let Some(parent) = path.parent() {
            parent_sync(parent).map_err(LocalCheckpointWriteError::committed)?;
        }
        Ok(())
    })();
    if result.as_ref().is_err_and(|err| !err.committed) {
        let _ = fs::remove_file(&tmp_path);
    }
    result
}
fn reject_unsafe_checkpoint_target(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => validate_local_checkpoint_file_metadata(path, &metadata),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}
fn absolute_local_checkpoint_path(path: &Path) -> io::Result<PathBuf> {
    if path.file_name().is_none() {
        return Err(io::Error::other("checkpoint path must name a file"));
    }
    let mut normalized = if path.is_absolute() {
        PathBuf::new()
    } else {
        std::env::current_dir().map_err(|err| {
            io::Error::new(
                err.kind(),
                format!("failed to resolve checkpoint working directory: {err}"),
            )
        })?
    };
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(io::Error::other(format!(
                    "checkpoint path `{}` must not contain parent-directory components",
                    path.display()
                )));
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    if !normalized.is_absolute() {
        return Err(io::Error::other(format!(
            "checkpoint path `{}` could not be resolved absolutely",
            path.display()
        )));
    }
    Ok(normalized)
}
fn validate_local_checkpoint_file_metadata(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::other(format!(
            "checkpoint target `{}` must be a regular file",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        if metadata.nlink() != 1 {
            return Err(io::Error::other(format!(
                "checkpoint target `{}` must have exactly one hard link",
                path.display()
            )));
        }
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(io::Error::other(format!(
                "checkpoint target `{}` must not be accessible by group or other users",
                path.display()
            )));
        }
    }
    Ok(())
}
fn reject_unsafe_checkpoint_ancestors(path: &Path) -> io::Result<()> {
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(io::Error::other(format!(
            "checkpoint path `{}` must not contain parent-directory components",
            path.display()
        )));
    }
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    for ancestor in parent.ancestors() {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(io::Error::other(format!(
                    "checkpoint ancestor `{}` must be a real directory",
                    ancestor.display()
                )));
            }
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
    }
    #[cfg(unix)]
    {
        match fs::symlink_metadata(parent) {
            Ok(metadata) if metadata.permissions().mode() & 0o022 != 0 => {
                return Err(io::Error::other(format!(
                    "checkpoint parent `{}` must not be group- or world-writable",
                    parent.display()
                )));
            }
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
    }
    Ok(())
}
fn read_local_checkpoint_bounded(path: &Path, max_bytes: u64) -> io::Result<Option<Vec<u8>>> {
    let path = absolute_local_checkpoint_path(path)?;
    let path = path.as_path();
    reject_unsafe_checkpoint_ancestors(path)?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    validate_local_checkpoint_file_metadata(path, &metadata)?;
    if metadata.len() > max_bytes {
        return Err(io::Error::other(format!(
            "checkpoint `{}` is {} bytes, exceeding limit {max_bytes}",
            path.display(),
            metadata.len()
        )));
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    set_local_no_follow_flag(&mut options);
    let file = options.open(path)?;
    let opened = file.metadata()?;
    reject_unsafe_checkpoint_ancestors(path)?;
    validate_local_checkpoint_file_metadata(path, &opened)?;
    if opened.len() > max_bytes || !same_local_file_identity(&metadata, &opened) {
        return Err(io::Error::other(format!(
            "checkpoint `{}` changed identity or exceeded its size limit while opening",
            path.display()
        )));
    }
    let capacity = usize::try_from(opened.len()).map_err(|_| {
        io::Error::other(format!(
            "checkpoint `{}` length does not fit memory address space",
            path.display()
        ))
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(io::Error::other(format!(
            "checkpoint `{}` grew beyond limit {max_bytes} while reading",
            path.display()
        )));
    }
    Ok(Some(bytes))
}
pub(crate) fn decode_local_checkpoint_canonical<T>(
    bytes: &[u8],
    max_bytes: u64,
    max_sequence_elements: usize,
) -> Result<T, String>
where
    for<'de> T: norito::core::NoritoDeserialize<'de> + norito::core::NoritoSerialize,
{
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(format!(
            "checkpoint is {} bytes, exceeding limit {max_bytes}",
            bytes.len()
        ));
    }
    let maximum_bytes = usize::try_from(max_bytes)
        .map_err(|_| "checkpoint byte limit does not fit memory address space".to_owned())?;
    let limits = local_checkpoint_decode_limits(bytes.len(), maximum_bytes, max_sequence_elements)?;
    let checkpoint: T = norito::decode_from_bytes_with_limits(bytes, limits)
        .map_err(|err| format!("bounded checkpoint decode failed: {err}"))?;
    let canonical = norito::to_bytes(&checkpoint)
        .map_err(|err| format!("canonical checkpoint encoding failed: {err}"))?;
    if canonical != bytes {
        return Err("checkpoint is not the exact canonical Norito encoding".to_owned());
    }
    Ok(checkpoint)
}
fn local_checkpoint_decode_limits(
    encoded_len: usize,
    maximum_bytes: usize,
    max_sequence_elements: usize,
) -> Result<norito::DecodeLimits, String> {
    if encoded_len == 0 {
        return Err("checkpoint must not be empty".to_owned());
    }
    if maximum_bytes == 0 || encoded_len > maximum_bytes {
        return Err("checkpoint byte length exceeds its configured bound".to_owned());
    }
    let wire_limits = norito::canonical_decode_limits(encoded_len);
    Ok(norito::DecodeLimits::new(
        max_sequence_elements
            .max(1)
            .min(wire_limits.max_sequence_elements()),
        wire_limits.max_field_bytes(),
        maximum_bytes
            .saturating_mul(2)
            .min(wire_limits.max_total_elements()),
        maximum_bytes
            .saturating_mul(4)
            .min(wire_limits.max_total_allocated_bytes()),
        64.min(wire_limits.max_nesting_depth()),
    ))
}
fn read_reputation_trust_policy_file(path: &Path) -> io::Result<Vec<u8>> {
    read_trust_policy_file(
        path,
        MAX_REPUTATION_TRUST_POLICY_ENCODED_BYTES,
        "reputation trust policy",
    )
}
fn read_trust_policy_file(
    path: &Path,
    max_bytes: usize,
    label: &'static str,
) -> io::Result<Vec<u8>> {
    let path = absolute_local_checkpoint_path(path)?;
    let path = path.as_path();
    reject_unsafe_checkpoint_ancestors(path)?;
    let before_open = fs::symlink_metadata(path)?;
    validate_trust_policy_file_metadata(path, &before_open, label)?;
    let max_bytes = u64::try_from(max_bytes)
        .map_err(|_| io::Error::other(format!("{label} size cap does not fit u64")))?;
    if before_open.len() > max_bytes {
        return Err(io::Error::other(format!(
            "{label} `{}` is {} bytes, exceeding limit {max_bytes}",
            path.display(),
            before_open.len()
        )));
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    set_local_no_follow_flag(&mut options);
    let file = options.open(path)?;
    let opened = file.metadata()?;
    validate_trust_policy_file_metadata(path, &opened, label)?;
    if opened.len() > max_bytes || !reputation_policy_metadata_stable(&before_open, &opened) {
        return Err(io::Error::other(format!(
            "{label} `{}` changed identity or exceeded its size limit while opening",
            path.display()
        )));
    }
    let capacity = usize::try_from(opened.len()).map_err(|_| {
        io::Error::other(format!(
            "{label} `{}` length does not fit memory address space",
            path.display()
        ))
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(capacity).map_err(|_| {
        io::Error::other(format!(
            "failed to reserve memory for {label} `{}`",
            path.display()
        ))
    })?;
    let mut limited = file.take(max_bytes.saturating_add(1));
    limited.read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(io::Error::other(format!(
            "{label} `{}` grew beyond limit {max_bytes} while reading",
            path.display()
        )));
    }
    let after_read_file = limited.get_ref().metadata()?;
    let after_read = fs::symlink_metadata(path)?;
    validate_trust_policy_file_metadata(path, &after_read, label)?;
    if !reputation_policy_metadata_stable(&opened, &after_read_file)
        || !reputation_policy_metadata_stable(&after_read_file, &after_read)
    {
        return Err(io::Error::other(format!(
            "{label} `{}` changed while being read",
            path.display()
        )));
    }
    Ok(bytes)
}
#[cfg(unix)]
fn reputation_policy_metadata_stable(expected: &fs::Metadata, observed: &fs::Metadata) -> bool {
    expected.dev() == observed.dev()
        && expected.ino() == observed.ino()
        && expected.len() == observed.len()
        && expected.mtime() == observed.mtime()
        && expected.mtime_nsec() == observed.mtime_nsec()
        && expected.ctime() == observed.ctime()
        && expected.ctime_nsec() == observed.ctime_nsec()
}
#[cfg(not(unix))]
fn reputation_policy_metadata_stable(expected: &fs::Metadata, observed: &fs::Metadata) -> bool {
    expected.len() == observed.len()
        && expected.modified().ok() == observed.modified().ok()
        && same_local_file_identity(expected, observed)
}
fn validate_trust_policy_file_metadata(
    path: &Path,
    metadata: &fs::Metadata,
    label: &'static str,
) -> io::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::other(format!(
            "{label} `{}` must be a regular non-symlink file",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        if metadata.nlink() != 1 {
            return Err(io::Error::other(format!(
                "{label} `{}` must have exactly one hard link",
                path.display()
            )));
        }
        if metadata.permissions().mode() & 0o022 != 0 {
            return Err(io::Error::other(format!(
                "{label} `{}` must not be writable by group or other users",
                path.display()
            )));
        }
    }
    Ok(())
}
fn collect_secure_object_store_files(
    root: &Path,
    current: &Path,
    depth: usize,
    file_limit: usize,
    files: &mut BTreeSet<PathBuf>,
) -> io::Result<()> {
    if depth > MODERATION_QUARANTINE_OBJECT_STORE_MAX_DEPTH {
        return Err(io::Error::other(format!(
            "quarantine object store `{}` exceeds maximum directory depth {}",
            root.display(),
            MODERATION_QUARANTINE_OBJECT_STORE_MAX_DEPTH
        )));
    }
    let entries = match fs::read_dir(current) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound && current == root => return Ok(()),
        Err(err) => return Err(err),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(io::Error::other(format!(
                "quarantine object store entry `{}` must not be a symlink",
                path.display()
            )));
        }
        if metadata.is_dir() {
            #[cfg(unix)]
            if metadata.permissions().mode() & 0o022 != 0 {
                return Err(io::Error::other(format!(
                    "quarantine object directory `{}` must not be group- or world-writable",
                    path.display()
                )));
            }
            collect_secure_object_store_files(root, &path, depth + 1, file_limit, files)?;
        } else if metadata.is_file() {
            validate_local_checkpoint_file_metadata(&path, &metadata)?;
            if !files.insert(path.clone()) {
                return Err(io::Error::other(format!(
                    "duplicate quarantine object store path `{}`",
                    path.display()
                )));
            }
            if files.len() > file_limit {
                return Err(io::Error::other(format!(
                    "quarantine object store exceeds file limit {file_limit}"
                )));
            }
        } else {
            return Err(io::Error::other(format!(
                "quarantine object store entry `{}` must be a regular file or directory",
                path.display()
            )));
        }
    }
    Ok(())
}
#[cfg(unix)]
fn same_local_file_identity(expected: &fs::Metadata, opened: &fs::Metadata) -> bool {
    expected.dev() == opened.dev() && expected.ino() == opened.ino()
}
#[cfg(not(unix))]
fn same_local_file_identity(expected: &fs::Metadata, opened: &fs::Metadata) -> bool {
    expected.len() == opened.len()
        && expected.modified().ok() == opened.modified().ok()
        && expected.created().ok() == opened.created().ok()
}
#[cfg(unix)]
fn set_local_no_follow_flag(options: &mut fs::OpenOptions) {
    options.custom_flags(local_no_follow_flag());
}
#[cfg(not(unix))]
fn set_local_no_follow_flag(_options: &mut fs::OpenOptions) {}
#[cfg(any(target_os = "linux", target_os = "android"))]
const fn local_no_follow_flag() -> i32 {
    0o400000
}
#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
const fn local_no_follow_flag() -> i32 {
    0x0000_0100
}
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
const fn local_no_follow_flag() -> i32 {
    0
}
#[cfg(unix)]
fn set_local_private_file_permissions(path: &Path) -> io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}
#[cfg(not(unix))]
fn set_local_private_file_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}
fn local_checkpoint_tmp_path(path: &Path) -> io::Result<PathBuf> {
    let mut nonce = [0_u8; 16];
    OsRng.try_fill_bytes(&mut nonce).map_err(|err| {
        io::Error::other(format!(
            "failed to generate checkpoint temporary-file nonce: {err}"
        ))
    })?;
    let suffix = format!(
        "{LOCAL_RUNTIME_SNAPSHOT_TMP_EXT}-{}-{}",
        std::process::id(),
        hex::encode(nonce)
    );
    let candidate = path.with_added_extension(&suffix);
    Ok(match candidate.file_name().and_then(|name| name.to_str()) {
        Some(name) => candidate.with_file_name(format!(".{name}")),
        None => candidate,
    })
}
const REPUTATION_EVENT_CHANNEL_CAPACITY: usize = 128;
fn reconciliation_divergence_count(storage: &StorageBackend, manifests: &[StoredManifest]) -> u32 {
    let mut divergence_count = 0_u32;
    let manifest_count = storage.manifest_count();
    let index_count = storage.index_manifest_count();
    if manifest_count != index_count {
        divergence_count = divergence_count.saturating_add(1);
        iroha_logger::warn!(
            manifest_count,
            index_count,
            "reconciliation mismatch: manifest count diverges from index"
        );
    }
    for manifest in manifests {
        if let Some(source) = manifest.retention_source() {
            if source.effective_epoch() != manifest.retention_epoch() {
                divergence_count = divergence_count.saturating_add(1);
                iroha_logger::warn!(
                    manifest_id = %manifest.manifest_id(),
                    retention_epoch = manifest.retention_epoch(),
                    source_epoch = source.effective_epoch(),
                    "reconciliation mismatch: retention epoch differs from source"
                );
            }
        } else if manifest.retention_epoch() != 0 {
            divergence_count = divergence_count.saturating_add(1);
            iroha_logger::warn!(
                manifest_id = %manifest.manifest_id(),
                retention_epoch = manifest.retention_epoch(),
                "reconciliation mismatch: missing retention source for retained manifest"
            );
        }
    }
    divergence_count
}
fn empty_appeal_finance_reconciliation_summary()
-> Result<AppealFinanceReconciliationSummaryV1, ReconciliationError> {
    let snapshot = reconciliation::AppealFinanceRollupReconciliationSnapshot {
        version: reconciliation::RECONCILIATION_SNAPSHOT_VERSION_V1,
        rollups: Vec::new(),
    };
    Ok(AppealFinanceReconciliationSummaryV1 {
        rollup_snapshot_hash: reconciliation::hash_snapshot(&snapshot)?,
        rollup_count: 0,
        source_report_count: 0,
        case_count: 0,
        total_treasury_xor: XorQuantity::zero(),
        total_rewards_forfeited_treasury_xor: XorQuantity::zero(),
    })
}
fn appeal_finance_rollup_reconciliation_entry(
    source: &governance::AuthoritativeAppealFinanceWeeklyRollup,
) -> reconciliation::AppealFinanceRollupReconciliationEntry {
    let rollup = &source.rollup;
    reconciliation::AppealFinanceRollupReconciliationEntry {
        cycle: rollup.cycle.to_string(),
        encoded_blake3: source.encoded_blake3.clone(),
        report_count: rollup.report_count,
        case_count: rollup.case_count,
        total_treasury_xor: rollup.total_treasury_xor.clone(),
        total_rewards_forfeited_treasury_xor: rollup.total_rewards_forfeited_treasury_xor.clone(),
        generated_at_unix_ms: rollup.generated_at_unix_ms,
    }
}
/// Anchor-backed authorization for one privacy transparency publication.
///
/// Values are constructed only after the qualified finalized release anchor
/// exactly matches the validated local release-chain head. The anchor is
/// monotonic, so later head advancement preserves this release as a canonical
/// prefix. Publishers must reject privacy payloads without this authorization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyPublicationAuthorizationV1 {
    leader_lease: TransparencyLeaderLeaseGrantV1,
    finalized_anchor: PrivacyReleaseAnchorHeadV1,
    release_sequence: u64,
    release_record_digest: [u8; 32],
    payload_digest: [u8; 32],
}
impl PrivacyPublicationAuthorizationV1 {
    fn try_new(
        leader_lease: &TransparencyLeaderLeaseGrantV1,
        finalized_anchor: PrivacyReleaseAnchorHeadV1,
        release: &transparency::PrivacyReleaseRecordV1,
        payload_digest: [u8; 32],
    ) -> Result<Self, GovernancePublishError> {
        leader_lease.validate().map_err(|error| {
            GovernancePublishError::other(format!(
                "invalid privacy publication leader lease: {error}"
            ))
        })?;
        let scope = leader_lease.scope();
        if !finalized_anchor.validate()
            || release.status != transparency::PrivacyReleaseStatusV1::Published
            || release.sequence == 0
            || release.record_digest == [0; 32]
            || payload_digest == [0; 32]
            || release.publication_payload_digest != Some(payload_digest)
            || finalized_anchor.query_id() != release.query_id
            || finalized_anchor.sequence() < release.sequence
            || scope.query_id() != release.query_id
            || scope.cycle_id() != release.release_id
            || scope.window().cycle_start_unix != release.cycle_start_unix
            || scope.window().cycle_end_unix != release.cycle_end_unix
            || scope.window().due_at_unix != release.due_at_unix
        {
            return Err(GovernancePublishError::other(
                "privacy publication authorization does not match the finalized release",
            ));
        }
        Ok(Self {
            leader_lease: leader_lease.clone(),
            finalized_anchor,
            release_sequence: release.sequence,
            release_record_digest: release.record_digest,
            payload_digest,
        })
    }
    /// Reconstruct a checked authorization from its exact public runtime fields.
    ///
    /// This constructor exists for canonical process-boundary transports. It accepts no credential
    /// or private evidence and revalidates the lease, finalized query lineage, release sequence,
    /// and payload digest before a decoded publication request reaches a deployment-owned provider.
    ///
    /// # Errors
    ///
    /// Returns an error when any field is malformed or the finalized anchor
    /// and lease do not name the same governed query.
    pub fn try_from_runtime_parts(
        leader_lease: TransparencyLeaderLeaseGrantV1,
        finalized_anchor: PrivacyReleaseAnchorHeadV1,
        release_sequence: u64,
        release_record_digest: [u8; 32],
        payload_digest: [u8; 32],
    ) -> Result<Self, GovernancePublishError> {
        leader_lease.validate().map_err(|error| {
            GovernancePublishError::other(format!(
                "invalid cached privacy publication leader lease: {error}"
            ))
        })?;
        let scope = leader_lease.scope();
        if !finalized_anchor.validate()
            || release_sequence == 0
            || release_record_digest == [0; 32]
            || payload_digest == [0; 32]
            || finalized_anchor.sequence() < release_sequence
            || finalized_anchor.query_id() != scope.query_id()
        {
            return Err(GovernancePublishError::other(
                "cached privacy publication authorization is malformed",
            ));
        }
        Ok(Self {
            leader_lease,
            finalized_anchor,
            release_sequence,
            release_record_digest,
            payload_digest,
        })
    }
    pub(crate) fn try_from_cached_parts(
        leader_lease: TransparencyLeaderLeaseGrantV1,
        finalized_anchor: PrivacyReleaseAnchorHeadV1,
        release_sequence: u64,
        release_record_digest: [u8; 32],
        payload_digest: [u8; 32],
    ) -> Result<Self, GovernancePublishError> {
        Self::try_from_runtime_parts(
            leader_lease,
            finalized_anchor,
            release_sequence,
            release_record_digest,
            payload_digest,
        )
    }
    fn validate_publication(
        &self,
        publication: &ModerationLedgerCyclePublicationV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        self.leader_lease.validate().map_err(|error| {
            GovernancePublishError::other(format!(
                "invalid privacy publication leader lease: {error}"
            ))
        })?;
        let scope = self.leader_lease.scope();
        if publication.privacy_aggregates.is_empty()
            || self.release_sequence == 0
            || self.release_record_digest == [0; 32]
            || self.payload_digest != *blake3::hash(encoded).as_bytes()
            || !self.finalized_anchor.validate()
            || self.finalized_anchor.sequence() < self.release_sequence
            || self.finalized_anchor.query_id() != scope.query_id()
            || scope.cycle_id() != publication.block.cycle_id
            || scope.window().cycle_start_unix != publication.block.cycle_start_unix
            || scope.window().cycle_end_unix != publication.block.cycle_end_unix
        {
            return Err(GovernancePublishError::other(
                "privacy publication does not match its finalized authorization",
            ));
        }
        Ok(())
    }
    /// Return the exact live leader lease retained for publication metadata.
    #[must_use]
    pub const fn leader_lease(&self) -> &TransparencyLeaderLeaseGrantV1 {
        &self.leader_lease
    }
    /// Return the finalized release-chain head that authorized this payload.
    #[must_use]
    pub const fn finalized_anchor(&self) -> PrivacyReleaseAnchorHeadV1 {
        self.finalized_anchor
    }
    /// Return the canonical release sequence bound to the payload.
    #[must_use]
    pub const fn release_sequence(&self) -> u64 {
        self.release_sequence
    }
    /// Return the canonical release-record digest bound to the payload.
    #[must_use]
    pub const fn release_record_digest(&self) -> [u8; 32] {
        self.release_record_digest
    }
    /// Return the exact canonical publication payload digest.
    #[must_use]
    pub const fn payload_digest(&self) -> [u8; 32] {
        self.payload_digest
    }
    pub(crate) fn binding_digest(&self) -> [u8; 32] {
        let lease = self.leader_lease();
        let scope = lease.scope();
        let window = scope.window();
        let provider = lease.provider_binding();
        let qualification = provider.qualification();
        let anchor = self.finalized_anchor();
        let mut hasher = blake3::Hasher::new();
        hasher.update(FENCED_PRIVACY_AUTHORIZATION_DIGEST_DOMAIN_V1);
        hasher.update(&lease.version().to_le_bytes());
        hasher.update(&lease.lease_id());
        hasher.update(&lease.fencing_token().to_le_bytes());
        hasher.update(&lease.issued_at_unix().to_le_bytes());
        hasher.update(&lease.expires_at_unix().to_le_bytes());
        hasher.update(&scope.query_id());
        hasher.update(&scope.cycle_id());
        hasher.update(&window.cycle_start_unix.to_le_bytes());
        hasher.update(&window.cycle_end_unix.to_le_bytes());
        hasher.update(&window.due_at_unix.to_le_bytes());
        hasher.update(&scope.holder_identity());
        fenced_privacy_digest_bytes(&mut hasher, provider.handle().as_bytes());
        hasher.update(&qualification.revision().to_le_bytes());
        hasher.update(&qualification.policy_digest());
        hasher.update(&anchor.query_id());
        hasher.update(&anchor.sequence().to_le_bytes());
        hasher.update(&anchor.release_id());
        hasher.update(&anchor.record_digest());
        match anchor.latest_publication_block_hash() {
            Some(digest) => {
                hasher.update(&[1]);
                hasher.update(&digest);
            }
            None => {
                hasher.update(&[0]);
            }
        }
        hasher.update(&self.release_sequence.to_le_bytes());
        hasher.update(&self.release_record_digest);
        hasher.update(&self.payload_digest);
        *hasher.finalize().as_bytes()
    }
    fn publication_idempotency_digest(&self) -> [u8; 32] {
        let scope = self.leader_lease().scope();
        let mut hasher = blake3::Hasher::new();
        hasher.update(FENCED_PRIVACY_PUBLICATION_IDEMPOTENCY_DIGEST_DOMAIN_V1);
        hasher.update(&scope.query_id());
        hasher.update(&scope.cycle_id());
        hasher.update(&self.release_sequence.to_le_bytes());
        hasher.update(&self.release_record_digest);
        hasher.update(&self.payload_digest);
        *hasher.finalize().as_bytes()
    }
}
const FENCED_PRIVACY_AUTHORIZATION_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.fenced-privacy.authorization.v1\0";
const FENCED_PRIVACY_PUBLICATION_IDEMPOTENCY_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.fenced-privacy.publication-idempotency.v1\0";
const FENCED_PRIVACY_REQUEST_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.fenced-privacy.request.v1\0";
const FENCED_PRIVACY_SUCCESSOR_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.fenced-privacy.successor.v1\0";
const FENCED_PRIVACY_HEAD_INCLUSION_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.fenced-privacy.head-inclusion.v1\0";
/// Wire version for the fused privacy Governance publication boundary.
pub const FENCED_TRANSPARENCY_PUBLICATION_VERSION_V1: u8 = 1;
/// Stable, payload-free failures returned by a fused transparency publisher.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum FencedTransparencyPublishErrorV1 {
    /// The request is malformed or internally inconsistent.
    #[error("fenced transparency publication request is invalid")]
    InvalidRequest,
    /// The deployment-owned provider is missing, unavailable, stale, or substituted.
    #[error("fenced transparency publication provider is unavailable or unqualified")]
    UnqualifiedProvider,
    /// The expected authoritative target head is no longer current.
    #[error("fenced transparency publication compare-and-append conflict")]
    CompareConflict,
    /// The query/release identity was already bound to different release or payload evidence.
    #[error("fenced transparency publication identity conflicts with an existing publication")]
    PublicationConflict,
    /// A newer fencing token has already been accepted.
    #[error("fenced transparency publication fencing token is stale")]
    StaleFencingToken,
    /// The provider rejected the exact request without changing authoritative state.
    #[error("fenced transparency publication request was rejected")]
    Rejected,
    /// The provider cannot prove whether the operation took effect.
    #[error("fenced transparency publication outcome is ambiguous")]
    Ambiguous,
    /// The returned receipt is malformed or substituted.
    #[error("fenced transparency publication receipt is invalid")]
    InvalidReceipt,
}
/// Opaque authoritative Governance target head retained as a local cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct FencedTransparencyTargetHeadV1 {
    version: u8,
    generation: u64,
    head_digest: [u8; 32],
    fencing_floor: u64,
}
impl FencedTransparencyTargetHeadV1 {
    /// Construct one checked non-genesis authoritative target head.
    ///
    /// # Errors
    ///
    /// Returns [`FencedTransparencyPublishErrorV1::InvalidRequest`] when the
    /// generation, digest, or fencing floor is zero.
    pub fn try_new(
        generation: u64,
        head_digest: [u8; 32],
        fencing_floor: u64,
    ) -> Result<Self, FencedTransparencyPublishErrorV1> {
        let head = Self {
            version: FENCED_TRANSPARENCY_PUBLICATION_VERSION_V1,
            generation,
            head_digest,
            fencing_floor,
        };
        if head.is_valid() {
            Ok(head)
        } else {
            Err(FencedTransparencyPublishErrorV1::InvalidRequest)
        }
    }
    /// Return the monotonic authoritative head generation.
    #[must_use]
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// Return the content-addressed authoritative head digest.
    #[must_use]
    pub const fn head_digest(self) -> [u8; 32] {
        self.head_digest
    }
    /// Return the greatest fencing token accepted at this head.
    #[must_use]
    pub const fn fencing_floor(self) -> u64 {
        self.fencing_floor
    }
    pub(crate) fn is_valid(self) -> bool {
        self.version == FENCED_TRANSPARENCY_PUBLICATION_VERSION_V1
            && self.generation != 0
            && self.head_digest != [0; 32]
            && self.fencing_floor != 0
    }
}
/// Stable publication identity and payload claimed at one authoritative head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FencedTransparencyPublicationInclusionV1 {
    publication_idempotency_digest: [u8; 32],
    payload_digest: [u8; 32],
    included_head: FencedTransparencyTargetHeadV1,
}
impl FencedTransparencyPublicationInclusionV1 {
    /// Construct one exact publication-inclusion requirement.
    ///
    /// # Errors
    ///
    /// Returns [`FencedTransparencyPublishErrorV1::InvalidRequest`] when either
    /// digest is zero or the claimed inclusion head is malformed.
    pub fn try_new(
        publication_idempotency_digest: [u8; 32],
        payload_digest: [u8; 32],
        included_head: FencedTransparencyTargetHeadV1,
    ) -> Result<Self, FencedTransparencyPublishErrorV1> {
        if publication_idempotency_digest == [0; 32]
            || payload_digest == [0; 32]
            || !included_head.is_valid()
        {
            return Err(FencedTransparencyPublishErrorV1::InvalidRequest);
        }
        Ok(Self {
            publication_idempotency_digest,
            payload_digest,
            included_head,
        })
    }
    /// Return the stable lease-independent publication identity.
    #[must_use]
    pub const fn publication_idempotency_digest(self) -> [u8; 32] {
        self.publication_idempotency_digest
    }
    /// Return the exact canonical payload digest.
    #[must_use]
    pub const fn payload_digest(self) -> [u8; 32] {
        self.payload_digest
    }
    /// Return the authoritative head claimed to include the identity and payload.
    #[must_use]
    pub const fn included_head(self) -> FencedTransparencyTargetHeadV1 {
        self.included_head
    }
}
/// Authenticated adapter proof that publications and retained heads reach one current head.
///
/// The proof digest is public, payload-free evidence emitted only after the
/// deployment adapter has authenticated the target and verified every requested
/// ancestor against the target's immutable history. It must never contain
/// credentials, bearer material, private evidence, or provider diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FencedTransparencyHeadAncestryProofV1 {
    version: u8,
    authoritative_head: Option<FencedTransparencyTargetHeadV1>,
    verified_ancestors: Vec<FencedTransparencyTargetHeadV1>,
    verified_publications: Vec<FencedTransparencyPublicationInclusionV1>,
    adapter_proof_digest: [u8; 32],
}
impl FencedTransparencyHeadAncestryProofV1 {
    /// Construct one adapter-verified ancestry proof.
    ///
    /// # Errors
    ///
    /// Returns [`FencedTransparencyPublishErrorV1::InvalidReceipt`] when the
    /// proof digest is zero, a head is malformed, an ancestor is newer than the
    /// authoritative head, or two claimed heads substitute one generation.
    pub fn try_new(
        authoritative_head: Option<FencedTransparencyTargetHeadV1>,
        verified_ancestors: Vec<FencedTransparencyTargetHeadV1>,
        verified_publications: Vec<FencedTransparencyPublicationInclusionV1>,
        adapter_proof_digest: [u8; 32],
    ) -> Result<Self, FencedTransparencyPublishErrorV1> {
        let proof = Self {
            version: FENCED_TRANSPARENCY_PUBLICATION_VERSION_V1,
            authoritative_head,
            verified_ancestors,
            verified_publications,
            adapter_proof_digest,
        };
        proof.validate_shape()?;
        Ok(proof)
    }
    /// Return the exact authenticated current target head.
    #[must_use]
    pub const fn authoritative_head(&self) -> Option<FencedTransparencyTargetHeadV1> {
        self.authoritative_head
    }
    /// Return the exact requested heads whose ancestry the adapter verified.
    #[must_use]
    pub fn verified_ancestors(&self) -> &[FencedTransparencyTargetHeadV1] {
        &self.verified_ancestors
    }
    /// Return every stable identity and payload proven at its inclusion head.
    #[must_use]
    pub fn verified_publications(&self) -> &[FencedTransparencyPublicationInclusionV1] {
        &self.verified_publications
    }
    /// Return the payload-free adapter evidence digest.
    #[must_use]
    pub const fn adapter_proof_digest(&self) -> [u8; 32] {
        self.adapter_proof_digest
    }
    fn validate_for_required_evidence(
        &self,
        required_ancestors: &[FencedTransparencyTargetHeadV1],
        required_publications: &[FencedTransparencyPublicationInclusionV1],
    ) -> Result<(), FencedTransparencyPublishErrorV1> {
        self.validate_shape()?;
        if self.verified_ancestors != required_ancestors
            || self.verified_publications != required_publications
        {
            return Err(FencedTransparencyPublishErrorV1::InvalidReceipt);
        }
        Ok(())
    }
    fn validate_shape(&self) -> Result<(), FencedTransparencyPublishErrorV1> {
        if self.version != FENCED_TRANSPARENCY_PUBLICATION_VERSION_V1
            || self.adapter_proof_digest == [0; 32]
            || self.authoritative_head.is_some_and(|head| !head.is_valid())
            || self
                .verified_ancestors
                .iter()
                .any(|ancestor| !ancestor.is_valid())
            || self.verified_publications.iter().any(|publication| {
                publication.publication_idempotency_digest == [0; 32]
                    || publication.payload_digest == [0; 32]
                    || !publication.included_head.is_valid()
                    || !self.verified_ancestors.contains(&publication.included_head)
            })
        {
            return Err(FencedTransparencyPublishErrorV1::InvalidReceipt);
        }
        let Some(authoritative_head) = self.authoritative_head else {
            return if self.verified_ancestors.is_empty() && self.verified_publications.is_empty() {
                Ok(())
            } else {
                Err(FencedTransparencyPublishErrorV1::InvalidReceipt)
            };
        };
        for (index, ancestor) in self.verified_ancestors.iter().enumerate() {
            if ancestor.generation() > authoritative_head.generation()
                || ancestor.fencing_floor() > authoritative_head.fencing_floor()
                || (ancestor.generation() == authoritative_head.generation()
                    && *ancestor != authoritative_head)
                || self.verified_ancestors[..index]
                    .iter()
                    .any(|prior| prior.generation() == ancestor.generation() && prior != ancestor)
            {
                return Err(FencedTransparencyPublishErrorV1::InvalidReceipt);
            }
        }
        Ok(())
    }
}
/// Exact input to one atomic privacy Governance compare-and-append.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FencedPrivacyPublicationRequestV1 {
    version: u8,
    authorization: PrivacyPublicationAuthorizationV1,
    authorization_digest: [u8; 32],
    publication_idempotency_digest: [u8; 32],
    canonical_payload: Vec<u8>,
    payload_digest: [u8; 32],
    expected_authoritative_head: Option<FencedTransparencyTargetHeadV1>,
    fencing_token: u64,
    fencing_floor: u64,
    request_digest: [u8; 32],
}
impl FencedPrivacyPublicationRequestV1 {
    /// Construct an exact request from canonical privacy publication bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when the authorization or payload is invalid, the
    /// predecessor/fencing floor is inconsistent, or the lease token is zero.
    /// Stale and equal tokens remain constructible so the target can perform its
    /// stable publication-identity lookup before enforcing the fencing floor.
    pub fn try_new(
        authorization: PrivacyPublicationAuthorizationV1,
        publication: &ModerationLedgerCyclePublicationV1,
        canonical_payload: Vec<u8>,
        expected_authoritative_head: Option<FencedTransparencyTargetHeadV1>,
        fencing_floor: u64,
    ) -> Result<Self, FencedTransparencyPublishErrorV1> {
        authorization
            .validate_publication(publication, &canonical_payload)
            .map_err(|_| FencedTransparencyPublishErrorV1::InvalidRequest)?;
        let expected_floor = expected_authoritative_head.map_or(0, |head| head.fencing_floor());
        if expected_authoritative_head.is_some_and(|head| !head.is_valid())
            || fencing_floor != expected_floor
        {
            return Err(FencedTransparencyPublishErrorV1::InvalidRequest);
        }
        let payload_len = u64::try_from(canonical_payload.len())
            .map_err(|_| FencedTransparencyPublishErrorV1::InvalidRequest)?;
        let fencing_token = authorization.leader_lease().fencing_token();
        if fencing_token == 0 {
            return Err(FencedTransparencyPublishErrorV1::InvalidRequest);
        }
        let authorization_digest = authorization.binding_digest();
        let publication_idempotency_digest = authorization.publication_idempotency_digest();
        let payload_digest = *blake3::hash(&canonical_payload).as_bytes();
        let request_digest = fenced_privacy_request_digest(
            authorization_digest,
            publication_idempotency_digest,
            payload_digest,
            payload_len,
            expected_authoritative_head,
            fencing_token,
            fencing_floor,
        );
        let request = Self {
            version: FENCED_TRANSPARENCY_PUBLICATION_VERSION_V1,
            authorization,
            authorization_digest,
            publication_idempotency_digest,
            canonical_payload,
            payload_digest,
            expected_authoritative_head,
            fencing_token,
            fencing_floor,
            request_digest,
        };
        request.validate()?;
        Ok(request)
    }
    /// Revalidate the canonical payload and every deterministic request binding.
    ///
    /// # Errors
    ///
    /// Returns [`FencedTransparencyPublishErrorV1::InvalidRequest`] when any
    /// request field, digest, canonical payload, or authorization binding differs.
    pub fn validate(&self) -> Result<(), FencedTransparencyPublishErrorV1> {
        let payload_len = u64::try_from(self.canonical_payload.len())
            .map_err(|_| FencedTransparencyPublishErrorV1::InvalidRequest)?;
        if self.version != FENCED_TRANSPARENCY_PUBLICATION_VERSION_V1
            || self.canonical_payload.is_empty()
            || self.payload_digest != *blake3::hash(&self.canonical_payload).as_bytes()
            || self.authorization_digest != self.authorization.binding_digest()
            || self.publication_idempotency_digest
                != self.authorization.publication_idempotency_digest()
            || self.fencing_token != self.authorization.leader_lease().fencing_token()
            || self.fencing_token == 0
            || self
                .expected_authoritative_head
                .is_some_and(|head| !head.is_valid())
            || self.fencing_floor
                != self
                    .expected_authoritative_head
                    .map_or(0, |head| head.fencing_floor())
            || self.request_digest
                != fenced_privacy_request_digest(
                    self.authorization_digest,
                    self.publication_idempotency_digest,
                    self.payload_digest,
                    payload_len,
                    self.expected_authoritative_head,
                    self.fencing_token,
                    self.fencing_floor,
                )
        {
            return Err(FencedTransparencyPublishErrorV1::InvalidRequest);
        }
        let publication = norito::decode_from_bytes::<ModerationLedgerCyclePublicationV1>(
            &self.canonical_payload,
        )
        .map_err(|_| FencedTransparencyPublishErrorV1::InvalidRequest)?;
        if norito::to_bytes(&publication)
            .map_err(|_| FencedTransparencyPublishErrorV1::InvalidRequest)?
            != self.canonical_payload
        {
            return Err(FencedTransparencyPublishErrorV1::InvalidRequest);
        }
        self.authorization
            .validate_publication(&publication, &self.canonical_payload)
            .map_err(|_| FencedTransparencyPublishErrorV1::InvalidRequest)
    }
    /// Return the anchor- and lease-bound publication authorization.
    #[must_use]
    pub const fn authorization(&self) -> &PrivacyPublicationAuthorizationV1 {
        &self.authorization
    }
    /// Return the canonical digest of every authorization field.
    #[must_use]
    pub const fn authorization_digest(&self) -> [u8; 32] {
        self.authorization_digest
    }
    /// Return the lease-independent identity used for atomic target deduplication.
    #[must_use]
    pub const fn publication_idempotency_digest(&self) -> [u8; 32] {
        self.publication_idempotency_digest
    }
    /// Return the governed query and release identity used to detect conflicts.
    #[must_use]
    pub const fn publication_scope(&self) -> ([u8; 32], [u8; 16]) {
        let scope = self.authorization.leader_lease.scope();
        (scope.query_id(), scope.cycle_id())
    }
    /// Return the exact canonical privacy publication bytes.
    #[must_use]
    pub fn canonical_payload(&self) -> &[u8] {
        &self.canonical_payload
    }
    /// Return the digest of the exact canonical payload.
    #[must_use]
    pub const fn payload_digest(&self) -> [u8; 32] {
        self.payload_digest
    }
    /// Return the expected authoritative predecessor, or `None` for genesis.
    #[must_use]
    pub const fn expected_authoritative_head(&self) -> Option<FencedTransparencyTargetHeadV1> {
        self.expected_authoritative_head
    }
    /// Return the leader-lease fencing token bound into the operation.
    #[must_use]
    pub const fn fencing_token(&self) -> u64 {
        self.fencing_token
    }
    /// Return the authoritative fencing floor expected by the caller.
    #[must_use]
    pub const fn fencing_floor(&self) -> u64 {
        self.fencing_floor
    }
    /// Return the stable idempotency digest of this exact request.
    #[must_use]
    pub const fn request_digest(&self) -> [u8; 32] {
        self.request_digest
    }
    /// Derive the only valid content-addressed successor for this request.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is invalid or its generation cannot
    /// advance without overflow.
    pub fn expected_successor_head(
        &self,
    ) -> Result<FencedTransparencyTargetHeadV1, FencedTransparencyPublishErrorV1> {
        self.validate()?;
        if self.fencing_token <= self.fencing_floor {
            return Err(FencedTransparencyPublishErrorV1::StaleFencingToken);
        }
        let generation = self
            .expected_authoritative_head
            .map_or(Some(1), |head| head.generation().checked_add(1))
            .ok_or(FencedTransparencyPublishErrorV1::InvalidRequest)?;
        let head_digest = fenced_privacy_successor_digest(self);
        FencedTransparencyTargetHeadV1::try_new(generation, head_digest, self.fencing_token)
    }
}
/// Target result for one stable privacy-publication identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub enum FencedPrivacyPublicationDispositionV1 {
    /// This request atomically created the authoritative inclusion head.
    Appended,
    /// The stable identity was already included and no new append occurred.
    AlreadyIncluded,
}
/// Exact success receipt returned after atomic append, readback, and inclusion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FencedPrivacyPublicationReceiptV1 {
    version: u8,
    provider_handle: String,
    provider_qualification: GovernanceDagRuntimeProviderQualificationV1,
    request_digest: [u8; 32],
    authorization_digest: [u8; 32],
    publication_idempotency_digest: [u8; 32],
    payload_digest: [u8; 32],
    expected_authoritative_head: Option<FencedTransparencyTargetHeadV1>,
    fencing_token: u64,
    fencing_floor: u64,
    disposition: FencedPrivacyPublicationDispositionV1,
    successor_head: FencedTransparencyTargetHeadV1,
    readback_head: FencedTransparencyTargetHeadV1,
    head_inclusion_digest: [u8; 32],
}
impl FencedPrivacyPublicationReceiptV1 {
    /// Build a receipt after a provider has verified the exact append and readback.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is invalid or its deterministic
    /// successor cannot be derived.
    pub fn from_verified_append(
        request: &FencedPrivacyPublicationRequestV1,
        provider_handle: impl Into<String>,
        provider_qualification: GovernanceDagRuntimeProviderQualificationV1,
    ) -> Result<Self, FencedTransparencyPublishErrorV1> {
        request.validate()?;
        let successor_head = request.expected_successor_head()?;
        let receipt = Self {
            version: FENCED_TRANSPARENCY_PUBLICATION_VERSION_V1,
            provider_handle: provider_handle.into(),
            provider_qualification,
            request_digest: request.request_digest,
            authorization_digest: request.authorization_digest,
            publication_idempotency_digest: request.publication_idempotency_digest,
            payload_digest: request.payload_digest,
            expected_authoritative_head: request.expected_authoritative_head,
            fencing_token: request.fencing_token,
            fencing_floor: request.fencing_floor,
            disposition: FencedPrivacyPublicationDispositionV1::Appended,
            successor_head,
            readback_head: successor_head,
            head_inclusion_digest: fenced_privacy_head_inclusion_digest(
                request,
                FencedPrivacyPublicationDispositionV1::Appended,
                successor_head,
                successor_head,
            ),
        };
        Ok(receipt)
    }
    /// Build a receipt for an identity already included by an earlier lease.
    ///
    /// The provider may call this constructor only after atomically finding the exact stable
    /// identity, verifying that its release and payload evidence do not conflict, and reading back
    /// an authoritative head containing the original inclusion head without appending.
    ///
    /// # Errors
    ///
    /// Returns an error when the request or either head is malformed, the
    /// inclusion head is newer than readback, or one generation is substituted.
    pub fn from_verified_existing(
        request: &FencedPrivacyPublicationRequestV1,
        provider_handle: impl Into<String>,
        provider_qualification: GovernanceDagRuntimeProviderQualificationV1,
        included_head: FencedTransparencyTargetHeadV1,
        readback_head: FencedTransparencyTargetHeadV1,
    ) -> Result<Self, FencedTransparencyPublishErrorV1> {
        request.validate()?;
        if !included_head.is_valid()
            || !readback_head.is_valid()
            || included_head.generation() > readback_head.generation()
            || included_head.fencing_floor() > readback_head.fencing_floor()
            || (included_head.generation() == readback_head.generation()
                && included_head != readback_head)
        {
            return Err(FencedTransparencyPublishErrorV1::InvalidReceipt);
        }
        Ok(Self {
            version: FENCED_TRANSPARENCY_PUBLICATION_VERSION_V1,
            provider_handle: provider_handle.into(),
            provider_qualification,
            request_digest: request.request_digest,
            authorization_digest: request.authorization_digest,
            publication_idempotency_digest: request.publication_idempotency_digest,
            payload_digest: request.payload_digest,
            expected_authoritative_head: request.expected_authoritative_head,
            fencing_token: request.fencing_token,
            fencing_floor: request.fencing_floor,
            disposition: FencedPrivacyPublicationDispositionV1::AlreadyIncluded,
            successor_head: included_head,
            readback_head,
            head_inclusion_digest: fenced_privacy_head_inclusion_digest(
                request,
                FencedPrivacyPublicationDispositionV1::AlreadyIncluded,
                included_head,
                readback_head,
            ),
        })
    }
    /// Validate the exact request echo, successor, readback, and provider identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is invalid or any receipt field differs
    /// from the expected provider identity and deterministic successor.
    pub fn validate_for_request(
        &self,
        request: &FencedPrivacyPublicationRequestV1,
        provider_handle: &str,
        provider_qualification: GovernanceDagRuntimeProviderQualificationV1,
    ) -> Result<(), FencedTransparencyPublishErrorV1> {
        request.validate()?;
        let disposition_is_valid = match self.disposition {
            FencedPrivacyPublicationDispositionV1::Appended => {
                let expected_successor = request.expected_successor_head()?;
                self.successor_head == expected_successor
                    && self.readback_head == expected_successor
            }
            FencedPrivacyPublicationDispositionV1::AlreadyIncluded => {
                self.successor_head.is_valid()
                    && self.readback_head.is_valid()
                    && self.successor_head.generation() <= self.readback_head.generation()
                    && self.successor_head.fencing_floor() <= self.readback_head.fencing_floor()
                    && (self.successor_head.generation() != self.readback_head.generation()
                        || self.successor_head == self.readback_head)
            }
        };
        if !disposition_is_valid
            || self.version != FENCED_TRANSPARENCY_PUBLICATION_VERSION_V1
            || self.provider_handle != provider_handle
            || self.provider_qualification != provider_qualification
            || self.request_digest != request.request_digest
            || self.authorization_digest != request.authorization_digest
            || self.publication_idempotency_digest != request.publication_idempotency_digest
            || self.payload_digest != request.payload_digest
            || self.expected_authoritative_head != request.expected_authoritative_head
            || self.fencing_token != request.fencing_token
            || self.fencing_floor != request.fencing_floor
            || self.head_inclusion_digest
                != fenced_privacy_head_inclusion_digest(
                    request,
                    self.disposition,
                    self.successor_head,
                    self.readback_head,
                )
        {
            return Err(FencedTransparencyPublishErrorV1::InvalidReceipt);
        }
        Ok(())
    }
    /// Return the exact request idempotency digest acknowledged by the provider.
    #[must_use]
    pub const fn request_digest(&self) -> [u8; 32] {
        self.request_digest
    }
    /// Return the stable lease-independent publication identity.
    #[must_use]
    pub const fn publication_idempotency_digest(&self) -> [u8; 32] {
        self.publication_idempotency_digest
    }
    /// Return the exact canonical payload digest acknowledged by the target.
    #[must_use]
    pub const fn payload_digest(&self) -> [u8; 32] {
        self.payload_digest
    }
    /// Return whether this request appended or found an existing inclusion.
    #[must_use]
    pub const fn disposition(&self) -> FencedPrivacyPublicationDispositionV1 {
        self.disposition
    }
    /// Return the original authoritative head that includes this identity.
    #[must_use]
    pub const fn included_head(&self) -> FencedTransparencyTargetHeadV1 {
        self.successor_head
    }
    /// Return the authoritative head read back by the atomic target operation.
    #[must_use]
    pub const fn readback_head(&self) -> FencedTransparencyTargetHeadV1 {
        self.readback_head
    }
    /// Return the deterministic head-inclusion evidence digest.
    #[must_use]
    pub const fn head_inclusion_digest(&self) -> [u8; 32] {
        self.head_inclusion_digest
    }
}
/// Deployment-owned atomic boundary for privacy Governance publication.
pub trait FencedTransparencyPublisherV1: Send + Sync + std::fmt::Debug {
    /// Return the stable opaque production adapter handle.
    fn handle(&self) -> &str;
    /// Qualify the active adapter and its public policy revision.
    ///
    /// # Errors
    ///
    /// Returns a redacted diagnostic when the adapter is unavailable, stale,
    /// revoked, test-marked, or cannot prove its public qualification.
    fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String>;
    /// Atomically compare the target head, fence, append, read back, and prove inclusion.
    ///
    /// The provider must retain and return the original receipt when the exact same request digest
    /// and request are retried, even after later appends. A request for an absent stable scope with
    /// an obsolete fencing token must return
    /// [`FencedTransparencyPublishErrorV1::StaleFencingToken`], while a current token paired with a
    /// substituted predecessor must return [`FencedTransparencyPublishErrorV1::CompareConflict`].
    /// Success is valid only after the append, exact readback, and target-head inclusion happen as
    /// one atomic target-owned operation.
    ///
    /// Before comparing the predecessor or fencing token, the target must atomically look up
    /// [`FencedPrivacyPublicationRequestV1::publication_scope`]. An absent scope is appended
    /// together with a durable mapping from that scope to the stable idempotency digest,
    /// payload/release evidence, and inclusion head. An identical stable identity returns
    /// [`FencedPrivacyPublicationDispositionV1::AlreadyIncluded`] without an append, even under a
    /// different lease, replica, predecessor, or restored local root. A scope mapped to different
    /// evidence returns [`FencedTransparencyPublishErrorV1::PublicationConflict`].
    ///
    /// # Errors
    ///
    /// Returns a stable, payload-free failure when qualification, validation, conflict detection,
    /// fencing, compare-and-append, readback, or inclusion proof cannot complete.
    fn compare_and_append_privacy(
        &self,
        request: &FencedPrivacyPublicationRequestV1,
    ) -> Result<FencedPrivacyPublicationReceiptV1, FencedTransparencyPublishErrorV1>;
}
fn fenced_privacy_digest_bytes(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
}
fn fenced_privacy_digest_head(
    hasher: &mut blake3::Hasher,
    head: Option<FencedTransparencyTargetHeadV1>,
) {
    match head {
        Some(head) => {
            hasher.update(&[1]);
            hasher.update(&head.generation().to_le_bytes());
            hasher.update(&head.head_digest());
            hasher.update(&head.fencing_floor().to_le_bytes());
        }
        None => {
            hasher.update(&[0]);
        }
    }
}
fn fenced_privacy_request_digest(
    authorization_digest: [u8; 32],
    publication_idempotency_digest: [u8; 32],
    payload_digest: [u8; 32],
    payload_len: u64,
    expected_authoritative_head: Option<FencedTransparencyTargetHeadV1>,
    fencing_token: u64,
    fencing_floor: u64,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(FENCED_PRIVACY_REQUEST_DIGEST_DOMAIN_V1);
    hasher.update(&[FENCED_TRANSPARENCY_PUBLICATION_VERSION_V1]);
    hasher.update(&authorization_digest);
    hasher.update(&publication_idempotency_digest);
    hasher.update(&payload_digest);
    hasher.update(&payload_len.to_le_bytes());
    fenced_privacy_digest_head(&mut hasher, expected_authoritative_head);
    hasher.update(&fencing_token.to_le_bytes());
    hasher.update(&fencing_floor.to_le_bytes());
    *hasher.finalize().as_bytes()
}
fn fenced_privacy_successor_digest(request: &FencedPrivacyPublicationRequestV1) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(FENCED_PRIVACY_SUCCESSOR_DIGEST_DOMAIN_V1);
    hasher.update(&request.request_digest);
    fenced_privacy_digest_head(&mut hasher, request.expected_authoritative_head);
    hasher.update(&request.payload_digest);
    hasher.update(&request.fencing_token.to_le_bytes());
    *hasher.finalize().as_bytes()
}
fn fenced_privacy_head_inclusion_digest(
    request: &FencedPrivacyPublicationRequestV1,
    disposition: FencedPrivacyPublicationDispositionV1,
    included_head: FencedTransparencyTargetHeadV1,
    readback_head: FencedTransparencyTargetHeadV1,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(FENCED_PRIVACY_HEAD_INCLUSION_DIGEST_DOMAIN_V1);
    hasher.update(&request.request_digest);
    hasher.update(&request.publication_idempotency_digest);
    hasher.update(&request.payload_digest);
    hasher.update(&[match disposition {
        FencedPrivacyPublicationDispositionV1::Appended => 0,
        FencedPrivacyPublicationDispositionV1::AlreadyIncluded => 1,
    }]);
    fenced_privacy_digest_head(&mut hasher, Some(included_head));
    fenced_privacy_digest_head(&mut hasher, Some(readback_head));
    *hasher.finalize().as_bytes()
}
/// Interface for emitting settlement artefacts to the governance DAG.
///
/// Implementations must be idempotent for identical canonical payload bytes.
/// The durable node outbox intentionally retries after crashes where external
/// publication succeeded but the local acknowledgement was not yet durable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub enum GovernanceSubmissionOriginV1 {
    /// Canonical Torii proof-token issuance ingress.
    TransparencyTokenIssuance,
    /// Canonical Torii privacy-aggregate source-event ingress.
    PrivacyAggregateSourceEvent,
    /// Canonical Torii due-cycle publication ingress.
    PrivacyAggregatePublishDue,
    /// Canonical Torii appeal-finance report ingress.
    AppealFinanceReport,
    /// Canonical Torii appeal-finance weekly-rollup ingress.
    AppealFinanceWeeklyRollup,
}
impl GovernanceSubmissionOriginV1 {
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::TransparencyTokenIssuance => 0,
            Self::PrivacyAggregateSourceEvent => 1,
            Self::PrivacyAggregatePublishDue => 2,
            Self::AppealFinanceReport => 3,
            Self::AppealFinanceWeeklyRollup => 4,
        }
    }
    /// Return the stable publish-index label for this authenticated ingress.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::TransparencyTokenIssuance => "transparency_token_issuance",
            Self::PrivacyAggregateSourceEvent => "privacy_aggregate_source_event",
            Self::PrivacyAggregatePublishDue => "privacy_aggregate_publish_due",
            Self::AppealFinanceReport => "appeal_finance_report",
            Self::AppealFinanceWeeklyRollup => "appeal_finance_weekly_rollup",
        }
    }
}
/// Server-derived authenticated identity bound to a durable governance submission.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct GovernanceSubmissionProvenanceV1 {
    publisher_account: AccountId,
    origin: GovernanceSubmissionOriginV1,
}
impl GovernanceSubmissionProvenanceV1 {
    fn new(publisher_account: AccountId, origin: GovernanceSubmissionOriginV1) -> Self {
        Self {
            publisher_account,
            origin,
        }
    }
    /// Return the canonical account authenticated by Torii.
    #[must_use]
    pub fn publisher_account(&self) -> &AccountId {
        &self.publisher_account
    }
    /// Return the exact authenticated ingress that admitted the submission.
    #[must_use]
    pub const fn origin(&self) -> GovernanceSubmissionOriginV1 {
        self.origin
    }
    pub(crate) fn to_dag_provenance(&self) -> sorafs_manifest::GovernanceDagSubmissionProvenanceV1 {
        let origin = match self.origin {
            GovernanceSubmissionOriginV1::TransparencyTokenIssuance => {
                sorafs_manifest::GovernanceDagSubmissionOriginV1::TransparencyTokenIssuance
            }
            GovernanceSubmissionOriginV1::PrivacyAggregateSourceEvent => {
                sorafs_manifest::GovernanceDagSubmissionOriginV1::PrivacyAggregateSourceEvent
            }
            GovernanceSubmissionOriginV1::PrivacyAggregatePublishDue => {
                sorafs_manifest::GovernanceDagSubmissionOriginV1::PrivacyAggregatePublishDue
            }
            GovernanceSubmissionOriginV1::AppealFinanceReport => {
                sorafs_manifest::GovernanceDagSubmissionOriginV1::AppealFinanceReport
            }
            GovernanceSubmissionOriginV1::AppealFinanceWeeklyRollup => {
                sorafs_manifest::GovernanceDagSubmissionOriginV1::AppealFinanceWeeklyRollup
            }
        };
        let publisher_account_digest =
            governance_dag_submission_account_digest_v1(&self.publisher_account.encode());
        sorafs_manifest::GovernanceDagSubmissionProvenanceV1 {
            publisher_account_digest,
            origin,
        }
    }
}
/// Durable publication boundary for authenticated governance records.
///
/// Implementations must make retries of the same canonical record idempotent,
/// persist every accepted publication durably, and preserve any server-derived
/// authentication provenance carried by the published value.
pub trait GovernancePublisher: Send + Sync + std::fmt::Debug {
    /// Persist the supplied settlement NORITO payload to the governance pipeline.
    fn publish_deal_settlement(
        &self,
        settlement: &DealSettlementV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
    /// Persist an admission-bound PDP terminal archive to the governance pipeline.
    fn publish_pdp_archive(
        &self,
        archive: &PdpGovernanceArchiveV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
    /// Persist a validated PoR challenge-publication envelope.
    fn publish_por_challenge_publication(
        &self,
        publication: &PorChallengePublicationV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
    /// Persist a validated PoR weekly report.
    fn publish_por_weekly_report(
        &self,
        report: &PorWeeklyReportV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
    /// Persist a GC audit event to the governance pipeline.
    fn publish_gc_audit_event(
        &self,
        event: &sorafs_manifest::repair::GcAuditEventV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
    /// Persist a reconciliation report to the governance pipeline.
    fn publish_reconciliation_report(
        &self,
        report: &SorafsReconciliationReportV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
    /// Persist an externally authorized reputation snapshot to the governance pipeline.
    fn publish_reputation_snapshot(
        &self,
        snapshot: &SignedReputationSnapshotV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
    /// Persist a moderation ballot lifecycle event to the governance pipeline.
    fn publish_moderation_ballot_event(
        &self,
        event: &SoraFsModerationBallotGovernanceEventV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
    /// Persist a moderation transparency ledger cycle publication to the governance pipeline.
    ///
    /// Privacy cycles require finalized authorization and must complete the
    /// fused target-owned append before creating any local publication
    /// artifact. Non-privacy cycles must reject an authorization.
    fn publish_transparency_ledger_publication(
        &self,
        publication: &ModerationLedgerCyclePublicationV1,
        encoded: &[u8],
        authorization: Option<&PrivacyPublicationAuthorizationV1>,
        provenance: Option<&GovernanceSubmissionProvenanceV1>,
    ) -> Result<(), GovernancePublishError>;
    /// Persist a proof-token issuance summary to the governance pipeline.
    fn publish_proof_token_issuance(
        &self,
        issuance: &ProofTokenIssuanceV1,
        encoded: &[u8],
        provenance: Option<&GovernanceSubmissionProvenanceV1>,
    ) -> Result<(), GovernancePublishError>;
    /// Persist an appeal finance report to the governance pipeline.
    fn publish_appeal_finance_report(
        &self,
        report: &SoraFsAppealFinanceReportV1,
        encoded: &[u8],
        provenance: &GovernanceSubmissionProvenanceV1,
    ) -> Result<(), GovernancePublishError>;
    /// Persist a weekly appeal finance rollup to the governance pipeline.
    fn publish_appeal_finance_weekly_rollup(
        &self,
        rollup: &SoraFsAppealFinanceWeeklyRollupV1,
        encoded: &[u8],
        provenance: &GovernanceSubmissionProvenanceV1,
    ) -> Result<(), GovernancePublishError>;
    /// Persist an appeal finance settlement receipt to the governance pipeline.
    fn publish_appeal_finance_settlement_receipt(
        &self,
        receipt: &SoraFsAppealFinanceSettlementReceiptV1,
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
/// One canonical publication-authority generation read through the retained Governance DAG root.
#[derive(Debug, PartialEq, Eq)]
pub struct GovernanceDagPublicationSnapshotV1 {
    canonical_bytes: Vec<u8>,
    store_generation: u64,
    store_record_digest: [u8; 32],
}
impl GovernanceDagPublicationSnapshotV1 {
    /// Borrow the canonical publication-envelope JSON bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    /// Return the typed two-slot store generation and complete record digest.
    #[must_use]
    pub const fn store_identity(&self) -> (u64, [u8; 32]) {
        (self.store_generation, self.store_record_digest)
    }
}
/// One canonical runtime-DAG generation authenticated by the exact sealed
/// producer checkpoint retained by this node.
#[derive(Debug, PartialEq, Eq)]
pub struct GovernanceDagRuntimeSnapshotV1 {
    head_bytes: Vec<u8>,
    index_bytes: Vec<u8>,
    store_generation: u64,
    store_record_digest: [u8; 32],
    checkpoint_generation: u64,
    checkpoint_revision: [u8; 32],
}
impl GovernanceDagRuntimeSnapshotV1 {
    /// Borrow the canonical signed-head bytes.
    #[must_use]
    pub fn head_bytes(&self) -> &[u8] {
        &self.head_bytes
    }
    /// Borrow the canonical runtime-index JSON bytes committed with the head.
    #[must_use]
    pub fn index_bytes(&self) -> &[u8] {
        &self.index_bytes
    }
    /// Return the typed two-slot store generation and complete record digest.
    #[must_use]
    pub const fn store_identity(&self) -> (u64, [u8; 32]) {
        (self.store_generation, self.store_record_digest)
    }
    /// Return the exact sealed producer-checkpoint generation and revision.
    #[must_use]
    pub const fn checkpoint_identity(&self) -> (u64, [u8; 32]) {
        (self.checkpoint_generation, self.checkpoint_revision)
    }
}
impl GovernancePublishError {
    /// Construct a generic publish failure.
    #[must_use]
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other(message.into())
    }
}
/// Result of one scheduled privacy aggregate publication attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivacyAggregateScheduleOutcome {
    /// Config-backed scheduling is disabled.
    Disabled,
    /// No cycle is old enough to publish at the supplied timestamp.
    NotDue,
    /// The due cycle was already published by this node runtime.
    AlreadyPublished {
        /// Due window that was skipped.
        window: PrivacyAggregateCycleWindow,
        /// Deterministic cycle id.
        cycle_id: [u8; 16],
    },
    /// The due cycle had no source events.
    NoSourceEvents {
        /// Due window that was skipped.
        window: PrivacyAggregateCycleWindow,
        /// Deterministic cycle id.
        cycle_id: [u8; 16],
    },
    /// The due cycle had source events, but every bucket was suppressed.
    AllBucketsSuppressed {
        /// Due window that was skipped.
        window: PrivacyAggregateCycleWindow,
        /// Deterministic cycle id.
        cycle_id: [u8; 16],
    },
    /// The due cycle was published.
    Published {
        /// Due window that was published.
        window: PrivacyAggregateCycleWindow,
        /// Published transparency ledger cycle.
        publication: ModerationLedgerCyclePublicationV1,
    },
}
/// Complete input for publishing one test-built privacy aggregate source cycle.
#[cfg(test)]
struct PrivacyAggregateSourceCycleInput {
    /// Stable identifier assigned to the published cycle.
    cycle_id: [u8; 16],
    /// Inclusive source-event window start, in Unix seconds.
    cycle_start_unix: u64,
    /// Exclusive source-event window end, in Unix seconds.
    cycle_end_unix: u64,
    /// Optional hash linking the cycle to its predecessor.
    previous_block_hash: Option<[u8; 32]>,
    /// Governed aggregation and privacy policy.
    config: PrivacyAggregateCycleConfig,
    /// Optional request-bound threshold-PRF input for this cycle.
    cycle_prf_input: Option<PrivacyCyclePrfInputV1>,
}
struct PublishedPrivacyCycleCommitInput<'a> {
    window: PrivacyAggregateCycleWindow,
    publication: &'a ModerationLedgerCyclePublicationV1,
    config: &'a PrivacyAggregateCycleConfig,
    private_source_digest: [u8; 32],
    prf_evidence: Option<([u8; 32], [u8; 32])>,
    composition_budget: Option<PrivacyCompositionBudgetPolicyV1>,
    request_receipt: PrivacyPublishRequestReceiptV1,
}
struct SuppressedPrivacyCycleCommitInput<'a> {
    cycle_id: [u8; 16],
    window: PrivacyAggregateCycleWindow,
    config: &'a PrivacyAggregateCycleConfig,
    private_source_digest: [u8; 32],
    previous_publication_block_hash: Option<[u8; 32]>,
    request_receipt: PrivacyPublishRequestReceiptV1,
}
const PRIVACY_PUBLISH_REQUEST_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.privacy_publish_request.v1";
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum PrivacyPublishRequestOutcomeV1 {
    Published { publication_bytes: Vec<u8> },
    AllBucketsSuppressed,
}
#[derive(Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct PrivacyPublishRequestReceiptV1 {
    idempotency_key: String,
    request_digest: [u8; 32],
    query_id: [u8; 32],
    requested_now_unix: u64,
    cycle_id: [u8; 16],
    cycle_start_unix: u64,
    cycle_end_unix: u64,
    publish_delay_seconds: u64,
    due_at_unix: u64,
    provenance: Option<GovernanceSubmissionProvenanceV1>,
    outcome: PrivacyPublishRequestOutcomeV1,
}
impl std::fmt::Debug for PrivacyPublishRequestReceiptV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PrivacyPublishRequestReceiptV1")
            .field("idempotency_key", &"<redacted>")
            .field("request_digest", &self.request_digest)
            .field("query_id", &self.query_id)
            .field("requested_now_unix", &self.requested_now_unix)
            .field("cycle_id", &self.cycle_id)
            .field("cycle_start_unix", &self.cycle_start_unix)
            .field("cycle_end_unix", &self.cycle_end_unix)
            .field("publish_delay_seconds", &self.publish_delay_seconds)
            .field("due_at_unix", &self.due_at_unix)
            .field("provenance", &self.provenance)
            .field(
                "outcome",
                &match &self.outcome {
                    PrivacyPublishRequestOutcomeV1::Published { .. } => "published",
                    PrivacyPublishRequestOutcomeV1::AllBucketsSuppressed => {
                        "all_buckets_suppressed"
                    }
                },
            )
            .finish()
    }
}
impl PrivacyPublishRequestReceiptV1 {
    fn validate(&self) -> Result<(), GovernancePublishError> {
        validate_privacy_publish_idempotency_key(&self.idempotency_key)?;
        if self.query_id == [0; 32]
            || self.requested_now_unix == 0
            || self.request_digest
                != privacy_publish_request_digest(
                    self.query_id,
                    &self.idempotency_key,
                    self.cycle_id,
                )
            || self.cycle_start_unix == 0
            || self.cycle_end_unix <= self.cycle_start_unix
            || self.cycle_end_unix.checked_add(self.publish_delay_seconds) != Some(self.due_at_unix)
            || self.requested_now_unix < self.due_at_unix
            || self.cycle_id
                != transparency::privacy_aggregate_cycle_id(
                    self.query_id,
                    self.cycle_start_unix,
                    self.cycle_end_unix,
                )
            || self.provenance.as_ref().is_some_and(|provenance| {
                provenance.origin() != GovernanceSubmissionOriginV1::PrivacyAggregatePublishDue
            })
        {
            return Err(GovernancePublishError::other(
                "privacy publish-request receipt is invalid",
            ));
        }
        if let PrivacyPublishRequestOutcomeV1::Published { publication_bytes } = &self.outcome {
            let publication = decode_canonical_governance_payload::<
                ModerationLedgerCyclePublicationV1,
            >(publication_bytes)?;
            if publication.block.cycle_id != self.cycle_id
                || publication.block.cycle_start_unix != self.cycle_start_unix
                || publication.block.cycle_end_unix != self.cycle_end_unix
            {
                return Err(GovernancePublishError::other(
                    "privacy publish-request receipt publication mismatch",
                ));
            }
        }
        Ok(())
    }
    fn outcome(&self) -> Result<PrivacyAggregateScheduleOutcome, GovernancePublishError> {
        self.validate()?;
        let window = PrivacyAggregateCycleWindow {
            cycle_start_unix: self.cycle_start_unix,
            cycle_end_unix: self.cycle_end_unix,
            due_at_unix: self.due_at_unix,
        };
        match &self.outcome {
            PrivacyPublishRequestOutcomeV1::Published { publication_bytes } => {
                let publication = decode_canonical_governance_payload::<
                    ModerationLedgerCyclePublicationV1,
                >(publication_bytes)?;
                Ok(PrivacyAggregateScheduleOutcome::Published {
                    window,
                    publication,
                })
            }
            PrivacyPublishRequestOutcomeV1::AllBucketsSuppressed => {
                Ok(PrivacyAggregateScheduleOutcome::AllBucketsSuppressed {
                    window,
                    cycle_id: self.cycle_id,
                })
            }
        }
    }
}
fn validate_privacy_publish_idempotency_key(
    idempotency_key: &str,
) -> Result<(), GovernancePublishError> {
    if idempotency_key.is_empty()
        || idempotency_key.len() > 256
        || idempotency_key.trim() != idempotency_key
        || idempotency_key.chars().any(char::is_control)
    {
        return Err(GovernancePublishError::other(
            "privacy publish idempotency key is invalid",
        ));
    }
    Ok(())
}
fn privacy_publish_request_digest(
    query_id: [u8; 32],
    idempotency_key: &str,
    expected_cycle_id: [u8; 16],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(PRIVACY_PUBLISH_REQUEST_DIGEST_DOMAIN_V1);
    hasher.update(&query_id);
    hasher.update(&expected_cycle_id);
    hasher.update(&(idempotency_key.len() as u64).to_le_bytes());
    hasher.update(idempotency_key.as_bytes());
    *hasher.finalize().as_bytes()
}
fn validate_privacy_release_lineage(
    release_ledger: &transparency::PrivacyReleaseLedgerV1,
    schedule: PrivacyAggregateScheduleConfig,
    config: &PrivacyAggregateCycleConfig,
) -> Result<(), GovernancePublishError> {
    if let Some(record) = release_ledger.records.last()
        && (record.query_id != config.query_id
            || record.first_cycle_start_unix != config.first_cycle_start_unix
            || record.cycle_seconds != config.cycle_seconds
            || record.first_cycle_start_unix != schedule.first_cycle_start_unix
            || record.cycle_seconds != schedule.cycle_seconds
            || record.publish_delay_seconds != schedule.publish_delay_seconds)
    {
        return Err(GovernancePublishError::other(
            "durable privacy release cadence does not match the configured query lineage",
        ));
    }
    Ok(())
}
fn validate_privacy_publish_receipt_release(
    receipt: &PrivacyPublishRequestReceiptV1,
    record: &transparency::PrivacyReleaseRecordV1,
) -> Result<(), GovernancePublishError> {
    receipt.validate()?;
    if record.release_id != receipt.cycle_id
        || record.query_id != receipt.query_id
        || record.cycle_start_unix != receipt.cycle_start_unix
        || record.cycle_end_unix != receipt.cycle_end_unix
        || record.publish_delay_seconds != receipt.publish_delay_seconds
        || record.due_at_unix != receipt.due_at_unix
    {
        return Err(GovernancePublishError::other(
            "privacy publish-request receipt conflicts with its release record",
        ));
    }
    match &receipt.outcome {
        PrivacyPublishRequestOutcomeV1::Published { publication_bytes } => {
            let publication = decode_canonical_governance_payload::<
                ModerationLedgerCyclePublicationV1,
            >(publication_bytes)?;
            let payload_digest = *blake3::hash(publication_bytes).as_bytes();
            let block_hash = publication.block.block_hash().map_err(|error| {
                GovernancePublishError::other(format!("hash privacy receipt publication: {error}"))
            })?;
            let aggregate_inventory_digest =
                privacy_published_aggregate_inventory_digest(&publication.privacy_aggregates)?;
            if record.status != transparency::PrivacyReleaseStatusV1::Published
                || record.publication_payload_digest != Some(payload_digest)
                || record.published_aggregate_inventory_digest != Some(aggregate_inventory_digest)
                || record.publication_block_hash != Some(block_hash)
                || record.previous_publication_block_hash != publication.block.previous_block_hash
                || publication.block.cycle_id != record.release_id
                || publication.block.cycle_start_unix != record.cycle_start_unix
                || publication.block.cycle_end_unix != record.cycle_end_unix
                || publication.block.generated_at_unix != record.cycle_end_unix
            {
                return Err(GovernancePublishError::other(
                    "published privacy request receipt conflicts with its exact release bytes",
                ));
            }
        }
        PrivacyPublishRequestOutcomeV1::AllBucketsSuppressed => {
            if record.status != transparency::PrivacyReleaseStatusV1::Suppressed
                || record.publication_payload_digest.is_some()
                || record.published_aggregate_inventory_digest.is_some()
                || record.publication_block_hash.is_some()
            {
                return Err(GovernancePublishError::other(
                    "suppressed privacy request receipt conflicts with its exact release",
                ));
            }
        }
    }
    Ok(())
}
/// Result of one scheduled evidence-viewer audit-report publication attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModerationEvidenceViewerAuditScheduleOutcome {
    /// Config-backed scheduling is disabled.
    Disabled,
    /// No cycle is old enough to publish at the supplied timestamp.
    NotDue,
    /// The due cycle was already published by this node runtime.
    AlreadyPublished {
        /// Due window that was skipped.
        window: PrivacyAggregateCycleWindow,
        /// Deterministic evidence-viewer audit cycle id.
        cycle_id: [u8; 16],
    },
    /// The due cycle had no local evidence-viewer session or access records.
    NoSourceEvents {
        /// Due window that was skipped.
        window: PrivacyAggregateCycleWindow,
        /// Deterministic evidence-viewer audit cycle id.
        cycle_id: [u8; 16],
    },
    /// The due cycle was reported and published.
    Published {
        /// Due window that was published.
        window: PrivacyAggregateCycleWindow,
        /// Payload-free audit report recorded for the window.
        report: Box<ModerationEvidenceViewerAuditReport>,
        /// Transparency source entry recorded for the report.
        source_entry: Box<TransparencyLedgerSourceEntry>,
        /// Published transparency ledger cycle.
        publication: ModerationLedgerCyclePublicationV1,
    },
}
/// Payload returned by a repair orchestrator for a missing chunk.
#[derive(Debug, Clone)]
pub struct RepairChunkPayload {
    /// Expected BLAKE3-256 digest of the chunk.
    pub digest: [u8; 32],
    /// Raw chunk bytes.
    pub bytes: Vec<u8>,
    /// Optional source label (provider id, URL, or orchestrator hint).
    pub source: Option<String>,
}
/// Bounded replay result for a monotonic local event stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventReplay<T> {
    /// Oldest sequence still retained by the node, when the stream is non-empty.
    pub oldest_available_sequence: Option<u64>,
    /// Latest sequence ever appended to this runtime stream.
    pub latest_sequence: Option<u64>,
    /// Whether the requested cursor predates retained history.
    pub gap: bool,
    /// Retained events after the requested cursor.
    pub events: Vec<T>,
}
#[derive(Debug, Clone)]
struct BoundedEventHistory<T> {
    events: VecDeque<T>,
    latest_sequence: u64,
    limit: usize,
}
impl<T> BoundedEventHistory<T> {
    fn new(limit: usize) -> Self {
        Self {
            events: VecDeque::new(),
            latest_sequence: 0,
            limit: limit.max(1),
        }
    }
    fn append(&mut self, build: impl FnOnce(u64) -> T) -> Result<T, GovernancePublishError>
    where
        T: Clone,
    {
        let sequence = self
            .latest_sequence
            .checked_add(1)
            .ok_or_else(|| GovernancePublishError::other("event sequence exhausted"))?;
        let event = build(sequence);
        self.events.push_back(event.clone());
        self.latest_sequence = sequence;
        while self.events.len() > self.limit {
            self.events.pop_front();
        }
        Ok(event)
    }
    fn restore(
        &mut self,
        events: Vec<T>,
        sequence_of: impl Fn(&T) -> u64,
    ) -> Result<(), GovernancePublishError> {
        let mut previous = None;
        for event in &events {
            let sequence = sequence_of(event);
            if sequence == 0
                || previous.is_some_and(|previous: u64| previous.checked_add(1) != Some(sequence))
            {
                return Err(GovernancePublishError::other(
                    "event checkpoint sequences must be non-zero and strictly consecutive",
                ));
            }
            previous = Some(sequence);
        }
        self.latest_sequence = previous.unwrap_or(0);
        self.events = events.into_iter().collect();
        while self.events.len() > self.limit {
            self.events.pop_front();
        }
        Ok(())
    }
    fn replay(
        &self,
        since_sequence: Option<u64>,
        limit: usize,
        sequence_of: impl Fn(&T) -> u64,
    ) -> EventReplay<T>
    where
        T: Clone,
    {
        let oldest_available_sequence = self.events.front().map(&sequence_of);
        let latest_sequence = (self.latest_sequence != 0).then_some(self.latest_sequence);
        let since = since_sequence.unwrap_or(0);
        let gap = since_sequence.is_some_and(|cursor| {
            oldest_available_sequence.is_some_and(|oldest| cursor.saturating_add(1) < oldest)
        });
        let events = self
            .events
            .iter()
            .filter(|event| sequence_of(event) > since)
            .take(limit.max(1))
            .cloned()
            .collect();
        EventReplay {
            oldest_available_sequence,
            latest_sequence,
            gap,
            events,
        }
    }
    fn retained(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.events.iter().cloned().collect()
    }
}
/// Errors surfaced when the repair orchestrator cannot fetch missing chunks.
#[derive(Debug, Error)]
pub enum RepairOrchestratorError {
    /// Generic failure with human-readable context.
    #[error("{0}")]
    Other(String),
}
impl RepairOrchestratorError {
    /// Construct a generic orchestrator failure.
    #[must_use]
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other(message.into())
    }
}
/// Interface for orchestrator-backed repair rehydration.
pub trait RepairOrchestrator: Send + Sync + std::fmt::Debug {
    /// Fetch invalid chunks from remote sources for the exact native lease.
    ///
    /// Implementations must return no more payloads than `invalid_chunks` and
    /// each payload must match one requested digest and length.
    fn rehydrate_missing_chunks(
        &self,
        context: &native_repair_worker::NativeRepairExecutionContextV1,
        manifest: &StoredManifest,
        invalid_chunks: &[ChunkFileRecord],
    ) -> Result<Vec<RepairChunkPayload>, RepairOrchestratorError>;
}
/// Runtime-only dependencies supplied by the embedding daemon.
///
/// Secret-bearing providers are deliberately absent from [`StorageConfig`]. This container records
/// only opaque service handles and never formats their implementation state.
#[derive(Clone, Default)]
pub struct NodeRuntimeDeps {
    moderation_quarantine_key_wrapper: Option<Arc<dyn ModerationQuarantineKeyWrapper>>,
    privacy_cycle_prf_provider: Option<Arc<dyn ProductionPrivacyCyclePrfProviderV1>>,
    privacy_release_anchor: Option<Arc<dyn ProductionPrivacyReleaseAnchorV1>>,
    transparency_leader_lease_provider:
        Option<Arc<dyn ProductionTransparencyLeaderLeaseProviderV1>>,
    fenced_transparency_publisher: Option<Arc<dyn FencedTransparencyPublisherV1>>,
    fenced_transparency_head_reader: Option<Arc<dyn FencedTransparencyAuthoritativeHeadReaderV1>>,
    governance_dag_signer: Option<Arc<dyn GovernanceDagRuntimeSigner>>,
    governance_dag_checkpoint_store: Option<Arc<dyn GovernanceDagSealedCheckpointStore>>,
    provider_ingest_checkpoint_runtime: Option<Arc<dyn ProviderIngestCheckpointRuntimeV1>>,
    por_finalized_replay_archive: Option<Arc<dyn PorFinalizedReplayArchiveV1>>,
}
impl std::fmt::Debug for NodeRuntimeDeps {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NodeRuntimeDeps")
            .field(
                "moderation_quarantine_key_wrapper",
                &self.moderation_quarantine_key_wrapper.is_some(),
            )
            .field(
                "privacy_cycle_prf_provider",
                &self.privacy_cycle_prf_provider.is_some(),
            )
            .field(
                "privacy_release_anchor",
                &self.privacy_release_anchor.is_some(),
            )
            .field(
                "transparency_leader_lease_provider",
                &self.transparency_leader_lease_provider.is_some(),
            )
            .field(
                "fenced_transparency_publisher",
                &self.fenced_transparency_publisher.is_some(),
            )
            .field(
                "fenced_transparency_head_reader",
                &self.fenced_transparency_head_reader.is_some(),
            )
            .field(
                "governance_dag_signer",
                &self.governance_dag_signer.is_some(),
            )
            .field(
                "governance_dag_checkpoint_store",
                &self.governance_dag_checkpoint_store.is_some(),
            )
            .field(
                "provider_ingest_checkpoint_runtime",
                &self.provider_ingest_checkpoint_runtime.is_some(),
            )
            .field(
                "por_finalized_replay_archive",
                &self.por_finalized_replay_archive.is_some(),
            )
            .finish()
    }
}
impl NodeRuntimeDeps {
    /// Attach the runtime-only PKCS#11/KMS quarantine-key wrapper.
    ///
    /// Startup also requires its exact public provider binding in
    /// [`StorageConfig`]; the injected adapter is never allowed to self-pin.
    #[must_use]
    pub fn with_moderation_quarantine_key_wrapper(
        mut self,
        key_wrapper: Arc<dyn ModerationQuarantineKeyWrapper>,
    ) -> Self {
        self.moderation_quarantine_key_wrapper = Some(key_wrapper);
        self
    }
    /// Attach the runtime-only threshold-PRF provider used by DP aggregates.
    #[must_use]
    pub fn with_privacy_cycle_prf_provider(
        mut self,
        provider: Arc<dyn ProductionPrivacyCyclePrfProviderV1>,
    ) -> Self {
        self.privacy_cycle_prf_provider = Some(provider);
        self
    }
    /// Attach the independently administered finalized privacy-release head.
    #[must_use]
    pub fn with_privacy_release_anchor(
        mut self,
        anchor: Arc<dyn ProductionPrivacyReleaseAnchorV1>,
    ) -> Self {
        self.privacy_release_anchor = Some(anchor);
        self
    }
    /// Attach the external sealed-CAS leader-lease provider for privacy releases.
    #[must_use]
    pub fn with_transparency_leader_lease_provider(
        mut self,
        provider: Arc<dyn ProductionTransparencyLeaderLeaseProviderV1>,
    ) -> Self {
        self.transparency_leader_lease_provider = Some(provider);
        self
    }
    /// Attach the deployment-owned fused privacy Governance target writer.
    ///
    /// Startup also requires the exact public binding in [`StorageConfig`] and
    /// a separately injected authenticated reader for the same target.
    #[must_use]
    pub fn with_fenced_transparency_publisher(
        mut self,
        publisher: Arc<dyn FencedTransparencyPublisherV1>,
    ) -> Self {
        self.fenced_transparency_publisher = Some(publisher);
        self
    }
    /// Attach the authenticated authoritative-head reader for fused publication.
    ///
    /// The reader and writer are independently qualified against the same
    /// configured handle, revision, and public-policy digest.
    #[must_use]
    pub fn with_fenced_transparency_head_reader(
        mut self,
        reader: Arc<dyn FencedTransparencyAuthoritativeHeadReaderV1>,
    ) -> Self {
        self.fenced_transparency_head_reader = Some(reader);
        self
    }
    /// Attach the production HSM/KMS signer for local Governance DAG blocks.
    #[must_use]
    pub fn with_governance_dag_signer(
        mut self,
        signer: Arc<dyn GovernanceDagRuntimeSigner>,
    ) -> Self {
        self.governance_dag_signer = Some(signer);
        self
    }
    /// Attach the sealed monotonic checkpoint store for the local signed DAG producer.
    ///
    /// Its expected public handle and qualification come exclusively from
    /// [`StorageConfig`]; the injected provider cannot self-pin a replacement.
    #[must_use]
    pub fn with_governance_dag_checkpoint_store(
        mut self,
        store: Arc<dyn GovernanceDagSealedCheckpointStore>,
    ) -> Self {
        self.governance_dag_checkpoint_store = Some(store);
        self
    }
    /// Attach the production sealed monotonic provider-ingest checkpoint store.
    #[must_use]
    pub fn with_provider_ingest_checkpoint_runtime(
        mut self,
        runtime: Arc<dyn ProviderIngestCheckpointRuntimeV1>,
    ) -> Self {
        self.provider_ingest_checkpoint_runtime = Some(runtime);
        self
    }
    /// Attach the deployment-owned authenticated finalized-PoR replay archive.
    #[must_use]
    pub fn with_por_finalized_replay_archive(
        mut self,
        archive: Arc<dyn PorFinalizedReplayArchiveV1>,
    ) -> Self {
        self.por_finalized_replay_archive = Some(archive);
        self
    }
}
#[derive(Clone)]
struct OpaqueModerationQuarantineKeyWrapper(Arc<dyn ModerationQuarantineKeyWrapper>);
impl std::fmt::Debug for OpaqueModerationQuarantineKeyWrapper {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ModerationQuarantineKeyWrapper(<runtime-only>)")
    }
}
impl std::ops::Deref for OpaqueModerationQuarantineKeyWrapper {
    type Target = dyn ModerationQuarantineKeyWrapper;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}
#[derive(Clone)]
struct OpaquePorFinalizedReplayArchive(Arc<dyn PorFinalizedReplayArchiveV1>);
impl std::fmt::Debug for OpaquePorFinalizedReplayArchive {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PorFinalizedReplayArchiveV1(<runtime-only>)")
    }
}
impl std::ops::Deref for OpaquePorFinalizedReplayArchive {
    type Target = dyn PorFinalizedReplayArchiveV1;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}
fn verify_por_replay_archive_provider(
    policy: &config::PorReplayArchivePolicyV1,
    archive: &dyn PorFinalizedReplayArchiveV1,
) -> Result<(), PorTrackerError> {
    let runtime_handle = archive.runtime_handle();
    if !iroha_config::parameters::is_production_runtime_handle(runtime_handle)
        || runtime_handle != policy.runtime_handle()
    {
        return Err(PorTrackerError::ReplayArchiveBindingMismatch);
    }
    let first_binding = archive.binding()?;
    if first_binding != policy.binding() {
        return Err(PorTrackerError::ReplayArchiveBindingMismatch);
    }
    archive.check_readiness()?;
    if archive.binding()? != first_binding {
        return Err(PorTrackerError::ReplayArchiveBindingMismatch);
    }
    Ok(())
}
#[derive(Clone)]
struct OpaquePrivacyCyclePrfProvider(Arc<QualifiedPrivacyCyclePrfProviderV1>);
impl std::fmt::Debug for OpaquePrivacyCyclePrfProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PrivacyCyclePrfProviderV1(<runtime-only>)")
    }
}
impl std::ops::Deref for OpaquePrivacyCyclePrfProvider {
    type Target = dyn PrivacyCyclePrfProviderV1;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}
#[derive(Clone)]
struct OpaquePrivacyReleaseAnchor(Arc<QualifiedPrivacyReleaseAnchorV1>);
impl std::fmt::Debug for OpaquePrivacyReleaseAnchor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PrivacyReleaseAnchorV1(<runtime-only>)")
    }
}
impl std::ops::Deref for OpaquePrivacyReleaseAnchor {
    type Target = dyn PrivacyReleaseAnchorV1;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}
#[derive(Clone)]
struct OpaqueTransparencyLeaderLeaseProvider(Arc<QualifiedTransparencyLeaderLeaseProviderV1>);
impl std::fmt::Debug for OpaqueTransparencyLeaderLeaseProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("TransparencyLeaderLeaseProviderV1(<runtime-only>)")
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
    proof_outcome_outbox: ProofOutcomeOutbox,
    repair_transaction_forwarder: RepairTransactionForwarder,
    native_repair_singleflight: native_repair_singleflight::NativeRepairSingleflightV1,
    orderbook_transaction_forwarder: OrderbookTransactionForwarder,
    reserve_transaction_forwarder: ReserveTransactionForwarder,
    provider_ingest_outbox: Option<ProviderIngestOutbox>,
    completed_musubi_store_instance: Option<CompletedMusubiStoreInstanceV1>,
    por_finalized_replay_archive: Option<OpaquePorFinalizedReplayArchive>,
    por_history: Arc<RwLock<HashMap<PorHistoryKey, PorHistoryEntry>>>,
    storage: Option<Arc<StorageBackend>>,
    pdp_provider: Option<PdpProviderProtocol>,
    gc_mutation_lock: Arc<Mutex<()>>,
    gc_eviction_intents: Arc<RwLock<GcEvictionIntentRuntime>>,
    gc_eviction_audit_links: Arc<RwLock<BTreeMap<u64, GcEvictionAuditLinkV1>>>,
    repair_orchestrator: Arc<RwLock<Option<Arc<dyn RepairOrchestrator>>>>,
    governance_publisher: Arc<RwLock<Option<Arc<dyn GovernancePublisher>>>>,
    startup_governance_publisher: Option<Arc<dyn GovernancePublisher>>,
    governance_publication_lock: Option<Arc<Mutex<()>>>,
    governance_runtime_root: Option<PathBuf>,
    governance_runtime_writer_root_guard: Option<GovernanceFilesystemRootGuard>,
    governance_runtime_read_root_guard: Option<GovernanceFilesystemRootGuard>,
    governance_dag_runtime_signer: Option<GovernanceRuntimeDagSigner>,
    governance_dag_runtime_checkpoint_store: Option<GovernanceRuntimeDagCheckpointStore>,
    governance_dag_mirror_reader: Arc<OnceLock<GovernanceDagMirrorReadHandleV1>>,
    governance_outbox: Arc<RwLock<GovernanceOutboxRuntime>>,
    governance_outbox_drain_lock: Arc<Mutex<()>>,
    runtime_mutation_lock: Arc<Mutex<()>>,
    auxiliary_checkpoint_lock: Arc<Mutex<()>>,
    #[cfg(test)]
    fail_after_next_auxiliary_checkpoint_publication: Arc<std::sync::atomic::AtomicBool>,
    durability_failure: Arc<Mutex<Option<String>>>,
    auxiliary_runtime_checkpoint_path: Option<PathBuf>,
    reputation_trust_policy: Option<Arc<ReputationSnapshotTrustPolicyV1>>,
    latest_reputation_snapshot: Arc<RwLock<Option<ReputationSnapshotV1>>>,
    reputation_snapshots: Arc<RwLock<BTreeMap<[u8; 16], AdmittedReputationSnapshotV1>>>,
    reputation_events: Arc<RwLock<BoundedEventHistory<ReputationSnapshotEventV1>>>,
    reputation_event_sender: broadcast::Sender<ReputationSnapshotEventV1>,
    hedging_feed_trust_policy: Option<Arc<HedgingFeedTrustPolicyV1>>,
    moderation_model_registry_checkpoint_path: Option<PathBuf>,
    moderation_model_registry: Arc<RwLock<ModerationModelRegistry>>,
    moderation_screening_checkpoint_path: Option<PathBuf>,
    moderation_screening: Arc<RwLock<ModerationScreeningRuntime>>,
    moderation_screening_authority: Arc<RwLock<Option<ModerationScreeningAuthorityV1>>>,
    moderation_quarantine_object_root: Option<PathBuf>,
    moderation_quarantine_object_index_path: Option<PathBuf>,
    moderation_quarantine_key_provider_binding: Option<ModerationQuarantineKeyProviderBindingV1>,
    moderation_quarantine_key_wrapper: Option<OpaqueModerationQuarantineKeyWrapper>,
    moderation_quarantine_objects: Arc<RwLock<ModerationQuarantineObjectRuntime>>,
    moderation_evidence_viewer_checkpoint_path: Option<PathBuf>,
    moderation_evidence_viewer: Arc<RwLock<ModerationEvidenceViewerRuntime>>,
    transparency_ledger_source_entries:
        Arc<RwLock<BTreeMap<String, TransparencyLedgerSourceEntry>>>,
    privacy_aggregate_source_events: Arc<RwLock<BTreeMap<String, PrivacyAggregateSourceEvent>>>,
    privacy_source_event_receipts: Arc<RwLock<BTreeMap<String, [u8; 32]>>>,
    privacy_publish_request_receipts: Arc<RwLock<BTreeMap<String, PrivacyPublishRequestReceiptV1>>>,
    published_privacy_aggregate_cycles: Arc<RwLock<BTreeSet<[u8; 16]>>>,
    privacy_composition_budget: Arc<RwLock<PrivacyCompositionBudgetLedgerV1>>,
    privacy_release_ledger: Arc<RwLock<transparency::PrivacyReleaseLedgerV1>>,
    privacy_cycle_prf_provider: Option<OpaquePrivacyCyclePrfProvider>,
    privacy_release_anchor: Option<OpaquePrivacyReleaseAnchor>,
    transparency_leader_lease_provider: Option<OpaqueTransparencyLeaderLeaseProvider>,
    fenced_transparency_publisher: Option<QualifiedFencedTransparencyPublisherV1>,
    fenced_transparency_head_reader: Option<QualifiedFencedTransparencyHeadReaderV1>,
    published_evidence_viewer_audit_cycles: Arc<RwLock<BTreeSet<[u8; 16]>>>,
}
type PorHistoryKey = ([u8; 32], [u8; 32]);
#[derive(Debug, Default, Clone)]
struct PorHistoryEntry {
    last_success_unix: Option<u64>,
    last_failure_unix: Option<u64>,
    failures_total: u64,
    consecutive_failures: u64,
}
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct PorHistoryCheckpointEntryV1 {
    manifest_digest: [u8; 32],
    provider_id: [u8; 32],
    last_success_unix: Option<u64>,
    last_failure_unix: Option<u64>,
    failures_total: u64,
    consecutive_failures: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum GovernanceOutboxKindV1 {
    DealSettlement,
    GcAudit,
    ReconciliationReport,
    SignedReputationSnapshot,
    TransparencyLedgerPublication,
    ProofTokenIssuance,
    AppealFinanceReport,
    AppealFinanceWeeklyRollup,
    AppealFinanceSettlementReceipt,
    PdpArchive,
    PorChallengePublication,
    PorWeeklyReport,
}
impl GovernanceOutboxKindV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::DealSettlement => 0,
            Self::GcAudit => 1,
            Self::ReconciliationReport => 2,
            Self::SignedReputationSnapshot => 3,
            Self::TransparencyLedgerPublication => 5,
            Self::ProofTokenIssuance => 6,
            Self::AppealFinanceReport => 7,
            Self::AppealFinanceWeeklyRollup => 8,
            Self::AppealFinanceSettlementReceipt => 9,
            Self::PdpArchive => 10,
            Self::PorChallengePublication => 11,
            Self::PorWeeklyReport => 12,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct GovernanceOutboxEntryV1 {
    version: u8,
    sequence: u64,
    kind: GovernanceOutboxKindV1,
    payload_digest: [u8; 32],
    provenance: Option<GovernanceSubmissionProvenanceV1>,
    binding_digest: [u8; 32],
    payload_bytes: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct GovernanceOutboxRuntime {
    next_sequence: u64,
    entries: BTreeMap<u64, GovernanceOutboxEntryV1>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GovernanceOutboxReservationUse {
    None,
    GcEviction,
}
impl Default for GovernanceOutboxRuntime {
    fn default() -> Self {
        Self {
            next_sequence: 1,
            entries: BTreeMap::new(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct GcStorageIdentityV1 {
    total_bytes: u64,
    manifest_count: u64,
    gc_freed_bytes_total: u64,
    gc_evictions_total: u64,
    manifest_set_digest: [u8; 32],
    chunk_refcounts_digest: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct GcEvictionIntentV1 {
    version: u8,
    sequence: u64,
    manifest_id: String,
    manifest_digest: [u8; 32],
    manifest_identity_digest: [u8; 32],
    provider_id: [u8; 32],
    audit_timestamp_unix: u64,
    reason: String,
    expected_freed_bytes: u64,
    storage_before: GcStorageIdentityV1,
    storage_after: GcStorageIdentityV1,
    reserved_outbox_slots: u8,
    binding_digest: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct GcEvictionIntentRuntime {
    next_sequence: u64,
    entries: BTreeMap<u64, GcEvictionIntentV1>,
}
impl Default for GcEvictionIntentRuntime {
    fn default() -> Self {
        Self {
            next_sequence: 1,
            entries: BTreeMap::new(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct GcEvictionAuditLinkV1 {
    version: u8,
    intent_sequence: u64,
    outbox_sequence: u64,
    manifest_id: String,
    manifest_digest: [u8; 32],
    provider_id: [u8; 32],
    occurred_at_unix: u64,
    freed_bytes: u64,
    reason: String,
    storage_gc_evictions_total: u64,
    payload_digest: [u8; 32],
    outbox_payload_digest: [u8; 32],
    binding_digest: [u8; 32],
}
#[derive(Debug)]
struct GcStorageSnapshot {
    identity: GcStorageIdentityV1,
    manifests: Vec<StoredManifest>,
    chunk_refcounts: Vec<ChunkRefcountEntry>,
}
fn gc_manifest_identity_digest(manifest: &StoredManifest) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(GC_STORAGE_MANIFEST_IDENTITY_DOMAIN_V1);
    hash_length_prefixed(&mut hasher, manifest.manifest_id().as_bytes());
    hash_length_prefixed(&mut hasher, manifest.manifest_cid());
    hasher.update(manifest.manifest_digest());
    hasher.update(manifest.payload_digest());
    hasher.update(&manifest.content_length().to_le_bytes());
    hash_length_prefixed(&mut hasher, manifest.chunk_profile_handle().as_bytes());
    hasher.update(&manifest.stored_at_unix_secs().to_le_bytes());
    hasher.update(&manifest.retention_epoch().to_le_bytes());
    hasher.update(
        &u64::try_from(manifest.chunk_count())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for index in 0..manifest.chunk_count() {
        let Some(chunk) = manifest.chunk(index) else {
            hasher.update(&[0xFF]);
            continue;
        };
        hasher.update(&chunk.offset.to_le_bytes());
        hasher.update(&chunk.length.to_le_bytes());
        hasher.update(&chunk.digest);
    }
    *hasher.finalize().as_bytes()
}
fn gc_manifest_set_digest(manifests: &[StoredManifest]) -> [u8; 32] {
    let mut identities = manifests
        .iter()
        .map(|manifest| {
            (
                manifest.manifest_id().to_owned(),
                gc_manifest_identity_digest(manifest),
            )
        })
        .collect::<Vec<_>>();
    identities.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = blake3::Hasher::new();
    hasher.update(GC_STORAGE_MANIFEST_SET_DOMAIN_V1);
    hasher.update(
        &u64::try_from(identities.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for (manifest_id, digest) in identities {
        hash_length_prefixed(&mut hasher, manifest_id.as_bytes());
        hasher.update(&digest);
    }
    *hasher.finalize().as_bytes()
}
fn gc_chunk_refcounts_digest(refcounts: &[ChunkRefcountEntry]) -> [u8; 32] {
    let mut refcounts = refcounts.to_vec();
    refcounts.sort_by_key(|entry| entry.digest);
    let mut hasher = blake3::Hasher::new();
    hasher.update(GC_STORAGE_CHUNK_REFCOUNTS_DOMAIN_V1);
    hasher.update(
        &u64::try_from(refcounts.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for entry in refcounts {
        hasher.update(&entry.digest);
        hasher.update(&entry.count.to_le_bytes());
    }
    *hasher.finalize().as_bytes()
}
fn gc_storage_identity_from_parts(
    manifests: &[StoredManifest],
    chunk_refcounts: &[ChunkRefcountEntry],
    total_bytes: u64,
    gc_counters: (u64, u64),
) -> Result<GcStorageIdentityV1, GovernancePublishError> {
    Ok(GcStorageIdentityV1 {
        total_bytes,
        manifest_count: u64::try_from(manifests.len())
            .map_err(|_| GovernancePublishError::other("storage manifest count exceeds u64"))?,
        gc_freed_bytes_total: gc_counters.0,
        gc_evictions_total: gc_counters.1,
        manifest_set_digest: gc_manifest_set_digest(manifests),
        chunk_refcounts_digest: gc_chunk_refcounts_digest(chunk_refcounts),
    })
}
fn gc_storage_snapshot_once_unchecked(
    storage: &StorageBackend,
) -> Result<GcStorageSnapshot, GovernancePublishError> {
    let manifests = storage.manifests();
    let chunk_refcounts = storage.chunk_refcount_snapshot();
    let total_bytes = storage.total_bytes();
    let gc_counters = storage.gc_counters();
    let identity =
        gc_storage_identity_from_parts(&manifests, &chunk_refcounts, total_bytes, gc_counters)?;
    Ok(GcStorageSnapshot {
        identity,
        manifests,
        chunk_refcounts,
    })
}
fn gc_storage_snapshot_once(
    storage: &StorageBackend,
) -> Result<GcStorageSnapshot, GovernancePublishError> {
    storage
        .ensure_durability_healthy()
        .map_err(|err| GovernancePublishError::other(err.to_string()))?;
    gc_storage_snapshot_once_unchecked(storage)
}
fn gc_storage_snapshot(
    storage: &StorageBackend,
) -> Result<GcStorageSnapshot, GovernancePublishError> {
    let first = gc_storage_snapshot_once(storage)?;
    let second = gc_storage_snapshot_once(storage)?;
    if first.identity != second.identity {
        return Err(GovernancePublishError::other(
            "storage generation changed while capturing a GC eviction intent",
        ));
    }
    Ok(second)
}
fn gc_storage_snapshot_unchecked(
    storage: &StorageBackend,
) -> Result<GcStorageSnapshot, GovernancePublishError> {
    let first = gc_storage_snapshot_once_unchecked(storage)?;
    let second = gc_storage_snapshot_once_unchecked(storage)?;
    if first.identity != second.identity {
        return Err(GovernancePublishError::other(
            "storage generation changed while inspecting a failed GC eviction",
        ));
    }
    Ok(second)
}
fn gc_expected_post_storage_identity(
    snapshot: &GcStorageSnapshot,
    target: &StoredManifest,
) -> Result<GcStorageIdentityV1, GovernancePublishError> {
    let manifests = snapshot
        .manifests
        .iter()
        .filter(|manifest| manifest.manifest_id() != target.manifest_id())
        .cloned()
        .collect::<Vec<_>>();
    if manifests.len().checked_add(1) != Some(snapshot.manifests.len()) {
        return Err(GovernancePublishError::other(
            "GC target is not uniquely present in the storage generation",
        ));
    }
    let mut refcounts = snapshot
        .chunk_refcounts
        .iter()
        .map(|entry| (entry.digest, entry.count))
        .collect::<BTreeMap<_, _>>();
    if refcounts.len() != snapshot.chunk_refcounts.len() {
        return Err(GovernancePublishError::other(
            "storage chunk refcount snapshot contains duplicate digests",
        ));
    }
    for index in 0..target.chunk_count() {
        let chunk = target.chunk(index).ok_or_else(|| {
            GovernancePublishError::other("GC target chunk metadata is incomplete")
        })?;
        let count = refcounts.get_mut(&chunk.digest).ok_or_else(|| {
            GovernancePublishError::other("GC target chunk lacks a storage refcount")
        })?;
        *count = count
            .checked_sub(1)
            .ok_or_else(|| GovernancePublishError::other("GC target chunk refcount underflow"))?;
        if *count == 0 {
            refcounts.remove(&chunk.digest);
        }
    }
    let refcounts = refcounts
        .into_iter()
        .map(|(digest, count)| ChunkRefcountEntry { digest, count })
        .collect::<Vec<_>>();
    let total_bytes = snapshot
        .identity
        .total_bytes
        .checked_sub(target.content_length())
        .ok_or_else(|| {
            GovernancePublishError::other("GC target exceeds accounted storage bytes")
        })?;
    let freed_bytes_total = snapshot
        .identity
        .gc_freed_bytes_total
        .checked_add(target.content_length())
        .ok_or_else(|| GovernancePublishError::other("GC freed-byte counter overflow"))?;
    let evictions_total = snapshot
        .identity
        .gc_evictions_total
        .checked_add(1)
        .ok_or_else(|| GovernancePublishError::other("GC eviction counter overflow"))?;
    gc_storage_identity_from_parts(
        &manifests,
        &refcounts,
        total_bytes,
        (freed_bytes_total, evictions_total),
    )
}
fn gc_eviction_payload(intent: &GcEvictionIntentV1) -> GcAuditPayloadV1 {
    GcAuditPayloadV1 {
        version: GC_AUDIT_PAYLOAD_VERSION_V1,
        manifest_digest: intent.manifest_digest,
        provider_id: intent.provider_id,
        evicted_at_unix: intent.audit_timestamp_unix,
        freed_bytes: intent.expected_freed_bytes,
        reason: intent.reason.clone(),
        blocked_reason: None,
    }
}
fn gc_eviction_intent_binding_digest(
    intent: &GcEvictionIntentV1,
) -> Result<[u8; 32], GovernancePublishError> {
    let storage_before = norito::to_bytes(&intent.storage_before).map_err(|err| {
        GovernancePublishError::other(format!("encode GC pre-eviction identity: {err}"))
    })?;
    let storage_after = norito::to_bytes(&intent.storage_after).map_err(|err| {
        GovernancePublishError::other(format!("encode GC post-eviction identity: {err}"))
    })?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(GC_EVICTION_INTENT_BINDING_DOMAIN_V1);
    hasher.update(&[intent.version]);
    hasher.update(&intent.sequence.to_le_bytes());
    hash_length_prefixed(&mut hasher, intent.manifest_id.as_bytes());
    hasher.update(&intent.manifest_digest);
    hasher.update(&intent.manifest_identity_digest);
    hasher.update(&intent.provider_id);
    hasher.update(&intent.audit_timestamp_unix.to_le_bytes());
    hash_length_prefixed(&mut hasher, intent.reason.as_bytes());
    hasher.update(&intent.expected_freed_bytes.to_le_bytes());
    hash_length_prefixed(&mut hasher, &storage_before);
    hash_length_prefixed(&mut hasher, &storage_after);
    hasher.update(&[intent.reserved_outbox_slots]);
    Ok(*hasher.finalize().as_bytes())
}
fn validate_gc_eviction_intent(intent: &GcEvictionIntentV1) -> Result<(), GovernancePublishError> {
    if intent.version != GC_EVICTION_INTENT_VERSION_V1
        || intent.sequence == 0
        || intent.reserved_outbox_slots != GC_EVICTION_RESERVED_OUTBOX_SLOTS
        || intent.manifest_identity_digest == [0; 32]
    {
        return Err(GovernancePublishError::other(
            "GC eviction intent version, sequence, identity, or reservation is invalid",
        ));
    }
    if intent.manifest_id != hex::encode(intent.manifest_digest) {
        return Err(GovernancePublishError::other(
            "GC eviction intent manifest id is not canonical for its digest",
        ));
    }
    gc_eviction_payload(intent)
        .validate()
        .map_err(|err| GovernancePublishError::other(err.to_string()))?;
    if intent.storage_before.manifest_count
        != intent
            .storage_after
            .manifest_count
            .checked_add(1)
            .ok_or_else(|| GovernancePublishError::other("GC manifest generation count overflow"))?
        || intent.storage_before.total_bytes
            != intent
                .storage_after
                .total_bytes
                .checked_add(intent.expected_freed_bytes)
                .ok_or_else(|| GovernancePublishError::other("GC byte generation overflow"))?
        || intent.storage_after.gc_freed_bytes_total
            != intent
                .storage_before
                .gc_freed_bytes_total
                .checked_add(intent.expected_freed_bytes)
                .ok_or_else(|| GovernancePublishError::other("GC freed-byte counter overflow"))?
        || intent.storage_after.gc_evictions_total
            != intent
                .storage_before
                .gc_evictions_total
                .checked_add(1)
                .ok_or_else(|| GovernancePublishError::other("GC counter overflow"))?
        || intent.storage_before.manifest_set_digest == intent.storage_after.manifest_set_digest
    {
        return Err(GovernancePublishError::other(
            "GC eviction intent storage generations do not encode one exact eviction",
        ));
    }
    if intent.binding_digest != gc_eviction_intent_binding_digest(intent)? {
        return Err(GovernancePublishError::other(
            "GC eviction intent binding digest mismatch",
        ));
    }
    Ok(())
}
fn restore_gc_eviction_intents(
    next_sequence: u64,
    entries: Vec<GcEvictionIntentV1>,
) -> Result<GcEvictionIntentRuntime, GovernancePublishError> {
    if next_sequence == 0 || entries.len() > 1 {
        return Err(GovernancePublishError::other(
            "GC eviction intent checkpoint has an invalid sequence or count",
        ));
    }
    let mut restored = BTreeMap::new();
    for intent in entries {
        validate_gc_eviction_intent(&intent)?;
        if intent.sequence >= next_sequence || restored.insert(intent.sequence, intent).is_some() {
            return Err(GovernancePublishError::other(
                "GC eviction intent sequence must be unique and below its high-water mark",
            ));
        }
    }
    Ok(GcEvictionIntentRuntime {
        next_sequence,
        entries: restored,
    })
}
fn gc_eviction_reserved_outbox_slots(
    runtime: &GcEvictionIntentRuntime,
) -> Result<usize, GovernancePublishError> {
    runtime.entries.values().try_fold(0usize, |total, intent| {
        validate_gc_eviction_intent(intent)?;
        total
            .checked_add(usize::from(intent.reserved_outbox_slots))
            .ok_or_else(|| GovernancePublishError::other("GC outbox reservation count overflow"))
    })
}
fn gc_eviction_audit_link_binding_digest(link: &GcEvictionAuditLinkV1) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(GC_EVICTION_AUDIT_LINK_BINDING_DOMAIN_V1);
    hasher.update(&[link.version]);
    hasher.update(&link.intent_sequence.to_le_bytes());
    hasher.update(&link.outbox_sequence.to_le_bytes());
    hash_length_prefixed(&mut hasher, link.manifest_id.as_bytes());
    hasher.update(&link.manifest_digest);
    hasher.update(&link.provider_id);
    hasher.update(&link.occurred_at_unix.to_le_bytes());
    hasher.update(&link.freed_bytes.to_le_bytes());
    hash_length_prefixed(&mut hasher, link.reason.as_bytes());
    hasher.update(&link.storage_gc_evictions_total.to_le_bytes());
    hasher.update(&link.payload_digest);
    hasher.update(&link.outbox_payload_digest);
    *hasher.finalize().as_bytes()
}
fn gc_eviction_audit_event_from_link(
    link: &GcEvictionAuditLinkV1,
) -> Result<GcAuditEventV1, GovernancePublishError> {
    let payload = GcAuditPayloadV1 {
        version: GC_AUDIT_PAYLOAD_VERSION_V1,
        manifest_digest: link.manifest_digest,
        provider_id: link.provider_id,
        evicted_at_unix: link.occurred_at_unix,
        freed_bytes: link.freed_bytes,
        reason: link.reason.clone(),
        blocked_reason: None,
    };
    let event = GcAuditEventV1 {
        version: GC_AUDIT_EVENT_VERSION_V1,
        header: SorafsAuditHeaderV1 {
            sequence: link.outbox_sequence,
            occurred_at_unix: link.occurred_at_unix,
            signer: GC_AUDIT_SIGNER_V1.to_owned(),
            payload_digest: gc_audit_payload_digest_v1(&payload)
                .map_err(|err| GovernancePublishError::other(err.to_string()))?,
        },
        payload,
    };
    event
        .validate()
        .map_err(|err| GovernancePublishError::other(err.to_string()))?;
    Ok(event)
}
fn validate_gc_eviction_audit_link(
    link: &GcEvictionAuditLinkV1,
) -> Result<(), GovernancePublishError> {
    if link.version != GC_EVICTION_AUDIT_LINK_VERSION_V1
        || link.intent_sequence == 0
        || link.outbox_sequence == 0
        || link.storage_gc_evictions_total == 0
        || link.manifest_id != hex::encode(link.manifest_digest)
    {
        return Err(GovernancePublishError::other(
            "GC eviction audit linkage metadata is invalid",
        ));
    }
    let event = gc_eviction_audit_event_from_link(link)?;
    let payload_digest = gc_audit_payload_digest_v1(&event.payload)
        .map_err(|err| GovernancePublishError::other(err.to_string()))?;
    let encoded = norito::to_bytes(&event)
        .map_err(|err| GovernancePublishError::other(format!("encode GC audit link: {err}")))?;
    if link.payload_digest != payload_digest
        || link.outbox_payload_digest != *blake3::hash(&encoded).as_bytes()
        || link.binding_digest != gc_eviction_audit_link_binding_digest(link)
    {
        return Err(GovernancePublishError::other(
            "GC eviction audit linkage digest mismatch",
        ));
    }
    Ok(())
}
fn hash_length_prefixed(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
}
fn governance_outbox_binding_digest(
    version: u8,
    sequence: u64,
    kind: GovernanceOutboxKindV1,
    payload_digest: [u8; 32],
    provenance: Option<&GovernanceSubmissionProvenanceV1>,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(GOVERNANCE_OUTBOX_BINDING_DOMAIN_V3);
    hasher.update(&[version]);
    hasher.update(&sequence.to_le_bytes());
    hasher.update(&[kind.tag()]);
    hasher.update(&payload_digest);
    match provenance {
        Some(provenance) => {
            hasher.update(&[1, provenance.origin.tag()]);
            hash_length_prefixed(&mut hasher, &provenance.publisher_account.encode());
        }
        None => {
            hasher.update(&[0]);
        }
    }
    *hasher.finalize().as_bytes()
}
fn governance_outbox_provenance_matches_kind(
    kind: GovernanceOutboxKindV1,
    provenance: &GovernanceSubmissionProvenanceV1,
) -> bool {
    matches!(
        (kind, provenance.origin),
        (
            GovernanceOutboxKindV1::TransparencyLedgerPublication,
            GovernanceSubmissionOriginV1::PrivacyAggregatePublishDue
        ) | (
            GovernanceOutboxKindV1::ProofTokenIssuance,
            GovernanceSubmissionOriginV1::TransparencyTokenIssuance
        ) | (
            GovernanceOutboxKindV1::AppealFinanceReport,
            GovernanceSubmissionOriginV1::AppealFinanceReport
        ) | (
            GovernanceOutboxKindV1::AppealFinanceWeeklyRollup,
            GovernanceSubmissionOriginV1::AppealFinanceWeeklyRollup
        )
    )
}
fn governance_outbox_kind_requires_provenance(kind: GovernanceOutboxKindV1) -> bool {
    matches!(
        kind,
        GovernanceOutboxKindV1::AppealFinanceReport
            | GovernanceOutboxKindV1::AppealFinanceWeeklyRollup
    )
}
fn decode_canonical_governance_payload<T>(bytes: &[u8]) -> Result<T, GovernancePublishError>
where
    for<'de> T: norito::NoritoDeserialize<'de>,
    T: norito::NoritoSerialize,
{
    let value = norito::decode_from_bytes::<T>(bytes).map_err(|err| {
        GovernancePublishError::other(format!("decode governance outbox payload: {err}"))
    })?;
    let canonical = norito::to_bytes(&value).map_err(|err| {
        GovernancePublishError::other(format!("re-encode governance outbox payload: {err}"))
    })?;
    if canonical != bytes {
        return Err(GovernancePublishError::other(
            "governance outbox payload is not canonically encoded",
        ));
    }
    Ok(value)
}
fn decode_canonical_signed_reputation_payload(
    bytes: &[u8],
) -> Result<SignedReputationSnapshotV1, GovernancePublishError> {
    decode_signed_reputation_snapshot(bytes).map_err(|err| {
        GovernancePublishError::other(format!(
            "decode signed reputation governance payload: {err}"
        ))
    })
}
fn decode_canonical_por_challenge_publication_payload(
    bytes: &[u8],
) -> Result<PorChallengePublicationV1, GovernancePublishError> {
    decode_por_challenge_publication_v1(bytes).map_err(|err| {
        GovernancePublishError::other(format!(
            "decode PoR challenge publication governance payload: {err}"
        ))
    })
}
fn decode_canonical_por_weekly_report_payload(
    bytes: &[u8],
) -> Result<PorWeeklyReportV1, GovernancePublishError> {
    decode_por_weekly_report_v1(bytes).map_err(|err| {
        GovernancePublishError::other(format!("decode weekly PoR governance payload: {err}"))
    })
}
fn validate_gc_audit_event(
    entry_sequence: u64,
    event: &GcAuditEventV1,
) -> Result<(), GovernancePublishError> {
    event
        .validate()
        .map_err(|err| GovernancePublishError::other(err.to_string()))?;
    if event.header.sequence != entry_sequence {
        return Err(GovernancePublishError::other(
            "GC audit header sequence does not match its outbox entry",
        ));
    }
    Ok(())
}
fn validate_governance_outbox_entry(
    entry: &GovernanceOutboxEntryV1,
) -> Result<(), GovernancePublishError> {
    if entry.version != GOVERNANCE_OUTBOX_VERSION_V3 {
        return Err(GovernancePublishError::other(format!(
            "unsupported governance outbox entry version {}",
            entry.version
        )));
    }
    if entry.sequence == 0 || entry.payload_bytes.is_empty() {
        return Err(GovernancePublishError::other(
            "governance outbox sequence and payload must be non-zero/non-empty",
        ));
    }
    let digest = *blake3::hash(&entry.payload_bytes).as_bytes();
    if digest != entry.payload_digest {
        return Err(GovernancePublishError::other(
            "governance outbox payload digest mismatch",
        ));
    }
    let binding_digest = governance_outbox_binding_digest(
        entry.version,
        entry.sequence,
        entry.kind,
        entry.payload_digest,
        entry.provenance.as_ref(),
    );
    if binding_digest != entry.binding_digest {
        return Err(GovernancePublishError::other(
            "governance outbox kind/sequence binding digest mismatch",
        ));
    }
    if governance_outbox_kind_requires_provenance(entry.kind) && entry.provenance.is_none() {
        return Err(GovernancePublishError::other(
            "governance outbox finance submission is missing authenticated provenance",
        ));
    }
    if entry.provenance.as_ref().is_some_and(|provenance| {
        !governance_outbox_provenance_matches_kind(entry.kind, provenance)
    }) {
        return Err(GovernancePublishError::other(
            "governance outbox provenance does not match its payload kind",
        ));
    }
    match entry.kind {
        GovernanceOutboxKindV1::DealSettlement => {
            decode_canonical_governance_payload::<DealSettlementV1>(&entry.payload_bytes)?
                .validate()
                .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        }
        GovernanceOutboxKindV1::GcAudit => {
            let event =
                decode_canonical_governance_payload::<GcAuditEventV1>(&entry.payload_bytes)?;
            validate_gc_audit_event(entry.sequence, &event)?;
        }
        GovernanceOutboxKindV1::ReconciliationReport => {
            decode_canonical_governance_payload::<SorafsReconciliationReportV1>(
                &entry.payload_bytes,
            )?
            .validate()
            .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        }
        GovernanceOutboxKindV1::SignedReputationSnapshot => {
            decode_canonical_signed_reputation_payload(&entry.payload_bytes)?;
        }
        GovernanceOutboxKindV1::TransparencyLedgerPublication => {
            decode_canonical_governance_payload::<ModerationLedgerCyclePublicationV1>(
                &entry.payload_bytes,
            )?
            .validate()
            .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        }
        GovernanceOutboxKindV1::ProofTokenIssuance => {
            decode_canonical_governance_payload::<ProofTokenIssuanceV1>(&entry.payload_bytes)?
                .validate()
                .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        }
        GovernanceOutboxKindV1::AppealFinanceReport => {
            decode_canonical_governance_payload::<SoraFsAppealFinanceReportV1>(
                &entry.payload_bytes,
            )?
            .validate()
            .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        }
        GovernanceOutboxKindV1::AppealFinanceWeeklyRollup => {
            decode_canonical_governance_payload::<SoraFsAppealFinanceWeeklyRollupV1>(
                &entry.payload_bytes,
            )?
            .validate()
            .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        }
        GovernanceOutboxKindV1::AppealFinanceSettlementReceipt => {
            decode_canonical_governance_payload::<SoraFsAppealFinanceSettlementReceiptV1>(
                &entry.payload_bytes,
            )?
            .validate()
            .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        }
        GovernanceOutboxKindV1::PdpArchive => {
            decode_canonical_governance_payload::<PdpGovernanceArchiveV1>(&entry.payload_bytes)?
                .validate()
                .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        }
        GovernanceOutboxKindV1::PorChallengePublication => {
            decode_canonical_por_challenge_publication_payload(&entry.payload_bytes)?
                .validate()
                .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        }
        GovernanceOutboxKindV1::PorWeeklyReport => {
            decode_canonical_por_weekly_report_payload(&entry.payload_bytes)?
                .validate()
                .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        }
    }
    Ok(())
}
fn insert_governance_outbox_entry(
    outbox: &mut GovernanceOutboxRuntime,
    kind: GovernanceOutboxKindV1,
    payload_bytes: Vec<u8>,
    provenance: Option<GovernanceSubmissionProvenanceV1>,
    entry_limit: usize,
    reserved_slots: usize,
) -> Result<(u64, bool), GovernancePublishError> {
    let payload_digest = *blake3::hash(&payload_bytes).as_bytes();
    if let Some(existing) = outbox.entries.values().find(|entry| {
        entry.kind == kind
            && entry.payload_digest == payload_digest
            && entry.payload_bytes == payload_bytes
    }) {
        if existing.provenance == provenance {
            return Ok((existing.sequence, false));
        }
        return Err(GovernancePublishError::other(
            "governance outbox canonical payload conflicts with retained authenticated provenance",
        ));
    }
    let unreserved_limit = entry_limit.checked_sub(reserved_slots).ok_or_else(|| {
        GovernancePublishError::other(format!(
            "governance outbox reservation {reserved_slots} exceeds retention limit {entry_limit}"
        ))
    })?;
    if outbox.entries.len() >= unreserved_limit {
        return Err(GovernancePublishError::other(format!(
            "governance outbox retention exhausted: {reserved_slots} of {entry_limit} slots are reserved for durable domain transactions"
        )));
    }
    let sequence = outbox.next_sequence;
    let next_sequence = sequence
        .checked_add(1)
        .ok_or_else(|| GovernancePublishError::other("governance outbox sequence exhausted"))?;
    let binding_digest = governance_outbox_binding_digest(
        GOVERNANCE_OUTBOX_VERSION_V3,
        sequence,
        kind,
        payload_digest,
        provenance.as_ref(),
    );
    let entry = GovernanceOutboxEntryV1 {
        version: GOVERNANCE_OUTBOX_VERSION_V3,
        sequence,
        kind,
        payload_digest,
        provenance,
        binding_digest,
        payload_bytes,
    };
    validate_governance_outbox_entry(&entry)?;
    outbox.entries.insert(sequence, entry);
    outbox.next_sequence = next_sequence;
    Ok((sequence, true))
}
fn restore_governance_outbox(
    next_sequence: u64,
    entries: Vec<GovernanceOutboxEntryV1>,
    entry_limit: usize,
) -> Result<GovernanceOutboxRuntime, GovernancePublishError> {
    if next_sequence == 0 {
        return Err(GovernancePublishError::other(
            "governance outbox next sequence must be non-zero",
        ));
    }
    if entries.len() > entry_limit {
        return Err(GovernancePublishError::other(format!(
            "governance outbox checkpoint count {} exceeds configured limit {entry_limit}",
            entries.len()
        )));
    }
    let mut restored = BTreeMap::new();
    let mut previous_sequence = None;
    for entry in entries {
        validate_governance_outbox_entry(&entry)?;
        // Gaps are intentional: an acknowledged artifact or a rejected
        // prepared domain intent can be removed while later entries remain.
        if entry.sequence >= next_sequence
            || previous_sequence.is_some_and(|previous| entry.sequence <= previous)
        {
            return Err(GovernancePublishError::other(
                "governance outbox entries must be strictly ordered below the next sequence",
            ));
        }
        previous_sequence = Some(entry.sequence);
        if restored.insert(entry.sequence, entry).is_some() {
            return Err(GovernancePublishError::other(
                "duplicate governance outbox sequence",
            ));
        }
    }
    Ok(GovernanceOutboxRuntime {
        next_sequence,
        entries: restored,
    })
}
fn validate_privacy_release_outbox(
    release_ledger: &transparency::PrivacyReleaseLedgerV1,
    published_cycles: &BTreeSet<[u8; 16]>,
    outbox: &GovernanceOutboxRuntime,
) -> Result<(), GovernancePublishError> {
    let records = release_ledger
        .records
        .iter()
        .map(|record| (record.release_id, record))
        .collect::<BTreeMap<_, _>>();
    let mut pending_cycles = BTreeSet::new();
    for entry in outbox
        .entries
        .values()
        .filter(|entry| entry.kind == GovernanceOutboxKindV1::TransparencyLedgerPublication)
    {
        let publication = decode_canonical_governance_payload::<ModerationLedgerCyclePublicationV1>(
            &entry.payload_bytes,
        )?;
        let is_privacy = !publication.proofs.is_empty()
            && publication
                .proofs
                .iter()
                .all(|proof| proof.entry.kind == ModerationLedgerEntryKindV1::PrivacyAggregate);
        if !is_privacy {
            continue;
        }
        let cycle_id = publication.block.cycle_id;
        if !published_cycles.contains(&cycle_id) || !pending_cycles.insert(cycle_id) {
            return Err(GovernancePublishError::other(
                "pending privacy publication has no unique published-cycle guard",
            ));
        }
        let record = records.get(&cycle_id).ok_or_else(|| {
            GovernancePublishError::other(
                "pending privacy publication has no durable privacy release record",
            )
        })?;
        let payload_digest = *blake3::hash(&entry.payload_bytes).as_bytes();
        if record.status != transparency::PrivacyReleaseStatusV1::Published
            || record.publication_payload_digest != Some(payload_digest)
            || publication.block.cycle_start_unix != record.cycle_start_unix
            || publication.block.cycle_end_unix != record.cycle_end_unix
            || publication.block.generated_at_unix != record.cycle_end_unix
            || publication.block.previous_block_hash != record.previous_publication_block_hash
            || publication.block.block_hash().map_err(|error| {
                GovernancePublishError::other(format!(
                    "hash pending privacy publication block: {error}"
                ))
            })? != record.publication_block_hash.ok_or_else(|| {
                GovernancePublishError::other("published privacy release omitted its block hash")
            })?
        {
            return Err(GovernancePublishError::other(
                "pending privacy publication conflicts with its durable release record",
            ));
        }
        let mut populations = Vec::with_capacity(publication.privacy_aggregates.len());
        let mut metric_schema: Option<Vec<PrivacyAggregateMetricSchemaV1>> = None;
        for aggregate in &publication.privacy_aggregates {
            if aggregate.policy_digest != record.policy_digest
                || aggregate.privacy != record.privacy
            {
                return Err(GovernancePublishError::other(
                    "pending privacy aggregate payload conflicts with release policy",
                ));
            }
            match (aggregate.noise_source, record.prf_commitment) {
                (
                    sorafs_manifest::ModerationPrivacyNoiseSourceV1::ThresholdPrf(commitment),
                    Some(expected),
                ) if commitment.commitment == expected => {}
                (sorafs_manifest::ModerationPrivacyNoiseSourceV1::SuppressionOnly, None) => {}
                _ => {
                    return Err(GovernancePublishError::other(
                        "pending privacy aggregate payload conflicts with release randomness evidence",
                    ));
                }
            }
            populations.push(PrivacyAggregatePopulationV1 {
                label: aggregate.population_label.clone(),
                digest: aggregate.population_digest,
            });
            let aggregate_schema = aggregate
                .metrics
                .iter()
                .map(|metric| PrivacyAggregateMetricSchemaV1 {
                    key: metric.key.clone(),
                    unit: metric.unit.clone(),
                })
                .collect::<Vec<_>>();
            if metric_schema
                .as_ref()
                .is_some_and(|expected| expected != &aggregate_schema)
            {
                return Err(GovernancePublishError::other(
                    "pending privacy aggregate payloads use inconsistent metric schemas",
                ));
            }
            metric_schema.get_or_insert(aggregate_schema);
        }
        let metric_schema = metric_schema.ok_or_else(|| {
            GovernancePublishError::other(
                "pending privacy publication omitted its aggregate payload inventory",
            )
        })?;
        populations.sort_by(|left, right| {
            left.label
                .cmp(&right.label)
                .then_with(|| left.digest.cmp(&right.digest))
        });
        if privacy_published_aggregate_inventory_digest(&publication.privacy_aggregates)?
            != record.published_aggregate_inventory_digest.ok_or_else(|| {
                GovernancePublishError::other(
                    "published privacy release omitted its aggregate inventory digest",
                )
            })?
            || privacy_metric_schema_digest(&metric_schema) != record.metric_schema_digest
            || (record.privacy.per_subject_metric_cap.is_some()
                && privacy_population_inventory_digest(&populations)
                    != record.population_inventory_digest)
        {
            return Err(GovernancePublishError::other(
                "pending privacy aggregate payload inventory conflicts with release bindings",
            ));
        }
    }
    Ok(())
}
const PRIVACY_PUBLISHED_AGGREGATE_INVENTORY_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.privacy_release.published_aggregate_inventory.v1";
fn privacy_published_aggregate_inventory_digest(
    aggregates: &[ModerationPrivacyAggregateV1],
) -> Result<[u8; 32], GovernancePublishError> {
    if aggregates.is_empty() {
        return Err(GovernancePublishError::other(
            "privacy publication aggregate inventory is empty",
        ));
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(PRIVACY_PUBLISHED_AGGREGATE_INVENTORY_DOMAIN_V1);
    hasher.update(&(aggregates.len() as u64).to_le_bytes());
    let mut previous_id: Option<&str> = None;
    for aggregate in aggregates {
        if previous_id.is_some_and(|previous| previous >= aggregate.aggregate_id.as_str()) {
            return Err(GovernancePublishError::other(
                "privacy publication aggregate inventory is not canonical",
            ));
        }
        let digest = aggregate.aggregate_hash().map_err(|error| {
            GovernancePublishError::other(format!(
                "hash privacy publication aggregate inventory: {error}"
            ))
        })?;
        hasher.update(&(aggregate.aggregate_id.len() as u64).to_le_bytes());
        hasher.update(aggregate.aggregate_id.as_bytes());
        hasher.update(&digest);
        previous_id = Some(aggregate.aggregate_id.as_str());
    }
    Ok(*hasher.finalize().as_bytes())
}
fn publish_governance_outbox_entry(
    publisher: &dyn GovernancePublisher,
    entry: &GovernanceOutboxEntryV1,
    authorization: Option<&PrivacyPublicationAuthorizationV1>,
) -> Result<(), GovernancePublishError> {
    validate_governance_outbox_entry(entry)?;
    match entry.kind {
        GovernanceOutboxKindV1::DealSettlement => {
            let payload =
                decode_canonical_governance_payload::<DealSettlementV1>(&entry.payload_bytes)?;
            publisher.publish_deal_settlement(&payload, &entry.payload_bytes)
        }
        GovernanceOutboxKindV1::GcAudit => {
            let payload =
                decode_canonical_governance_payload::<GcAuditEventV1>(&entry.payload_bytes)?;
            publisher.publish_gc_audit_event(&payload, &entry.payload_bytes)
        }
        GovernanceOutboxKindV1::ReconciliationReport => {
            let payload = decode_canonical_governance_payload::<SorafsReconciliationReportV1>(
                &entry.payload_bytes,
            )?;
            publisher.publish_reconciliation_report(&payload, &entry.payload_bytes)
        }
        GovernanceOutboxKindV1::SignedReputationSnapshot => {
            let payload = decode_canonical_signed_reputation_payload(&entry.payload_bytes)?;
            publisher.publish_reputation_snapshot(&payload, &entry.payload_bytes)
        }
        GovernanceOutboxKindV1::TransparencyLedgerPublication => {
            let payload = decode_canonical_governance_payload::<ModerationLedgerCyclePublicationV1>(
                &entry.payload_bytes,
            )?;
            publisher.publish_transparency_ledger_publication(
                &payload,
                &entry.payload_bytes,
                authorization,
                entry.provenance.as_ref(),
            )
        }
        GovernanceOutboxKindV1::ProofTokenIssuance => {
            let payload =
                decode_canonical_governance_payload::<ProofTokenIssuanceV1>(&entry.payload_bytes)?;
            publisher.publish_proof_token_issuance(
                &payload,
                &entry.payload_bytes,
                entry.provenance.as_ref(),
            )
        }
        GovernanceOutboxKindV1::AppealFinanceReport => {
            let payload = decode_canonical_governance_payload::<SoraFsAppealFinanceReportV1>(
                &entry.payload_bytes,
            )?;
            let provenance = entry.provenance.as_ref().ok_or_else(|| {
                GovernancePublishError::other(
                    "governance outbox finance report is missing authenticated provenance",
                )
            })?;
            publisher.publish_appeal_finance_report(&payload, &entry.payload_bytes, provenance)
        }
        GovernanceOutboxKindV1::AppealFinanceWeeklyRollup => {
            let payload = decode_canonical_governance_payload::<SoraFsAppealFinanceWeeklyRollupV1>(
                &entry.payload_bytes,
            )?;
            let provenance = entry.provenance.as_ref().ok_or_else(|| {
                GovernancePublishError::other(
                    "governance outbox finance rollup is missing authenticated provenance",
                )
            })?;
            publisher.publish_appeal_finance_weekly_rollup(
                &payload,
                &entry.payload_bytes,
                provenance,
            )
        }
        GovernanceOutboxKindV1::AppealFinanceSettlementReceipt => {
            let payload = decode_canonical_governance_payload::<
                SoraFsAppealFinanceSettlementReceiptV1,
            >(&entry.payload_bytes)?;
            publisher.publish_appeal_finance_settlement_receipt(&payload, &entry.payload_bytes)
        }
        GovernanceOutboxKindV1::PdpArchive => {
            let payload = decode_canonical_governance_payload::<PdpGovernanceArchiveV1>(
                &entry.payload_bytes,
            )?;
            publisher.publish_pdp_archive(&payload, &entry.payload_bytes)
        }
        GovernanceOutboxKindV1::PorChallengePublication => {
            let payload = decode_canonical_por_challenge_publication_payload(&entry.payload_bytes)?;
            publisher.publish_por_challenge_publication(&payload, &entry.payload_bytes)
        }
        GovernanceOutboxKindV1::PorWeeklyReport => {
            let payload = decode_canonical_por_weekly_report_payload(&entry.payload_bytes)?;
            publisher.publish_por_weekly_report(&payload, &entry.payload_bytes)
        }
    }
}
/// Signed reputation envelope plus the exact admission time used for freshness checks.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct AdmittedReputationSnapshotV1 {
    version: u8,
    admitted_at_unix: u64,
    encoded_len: u64,
    envelope: SignedReputationSnapshotV1,
}
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct AuxiliaryRuntimeCheckpointV5 {
    version: u8,
    capacity_runtime: CapacityRuntimeCheckpointV1,
    por_tracker: por::PorTrackerCheckpointV1,
    por_history: Vec<PorHistoryCheckpointEntryV1>,
    gc_eviction_intent_next_sequence: u64,
    gc_eviction_intents: Vec<GcEvictionIntentV1>,
    gc_eviction_audit_links: Vec<GcEvictionAuditLinkV1>,
    reputation_snapshots: Vec<AdmittedReputationSnapshotV1>,
    latest_reputation_snapshot_id: Option<[u8; 16]>,
    reputation_events: Vec<ReputationSnapshotEventV1>,
    transparency_source_entries: Vec<TransparencyLedgerSourceEntry>,
    privacy_source_events: Vec<PrivacyAggregateSourceEvent>,
    privacy_source_event_receipts: Vec<transparency::PrivacySourceEventReceiptV1>,
    privacy_publish_request_receipts: Vec<PrivacyPublishRequestReceiptV1>,
    published_privacy_aggregate_cycles: Vec<[u8; 16]>,
    privacy_composition_budget: PrivacyCompositionBudgetLedgerV1,
    privacy_release_ledger: transparency::PrivacyReleaseLedgerV1,
    transparency_leader_lease_fencing_floor: u64,
    published_evidence_viewer_audit_cycles: Vec<[u8; 16]>,
    governance_outbox_next_sequence: u64,
    governance_outbox_entries: Vec<GovernanceOutboxEntryV1>,
}
/// Unsigned deterministic reputation material intended for external governance signing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReputationSnapshotSigningMaterialV1 {
    /// Canonical snapshot reproduced by the scoring evidence.
    pub snapshot: ReputationSnapshotV1,
    /// Complete provider inputs and trust edges required to replay the snapshot.
    pub scoring_evidence: ReputationScoringEvidenceV1,
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
    /// Scheduler admission refused work without parking the caller.
    #[error(transparent)]
    Scheduler(#[from] SchedulerAdmissionError),
}
/// Redacted errors returned by admitted-payload verification lease acquisition.
///
/// This boundary deliberately never returns backend errors because those diagnostics can contain
/// provider-local filesystem paths. Detailed failures are retained only in provider logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AdmittedPayloadReadLeaseErrorV1 {
    /// The embedded SoraFS storage subsystem is disabled.
    #[error("SoraFS storage is disabled for this node")]
    Disabled,
    /// No currently admitted payload has the requested canonical manifest digest.
    #[error("the requested storage-admitted SoraFS payload is unavailable")]
    NotAdmitted,
    /// Fail-closed storage state prevented safe lifecycle-lease acquisition.
    #[error("storage-admitted SoraFS payload verification is temporarily unavailable")]
    StorageUnavailable,
}
/// Errors raised by provider-internal finalized-ledger storage ingest.
#[derive(Debug, Error)]
pub enum FinalizedProviderIngestError {
    /// Provider storage or its durable outbox is disabled.
    #[error("finalized-ledger provider ingest is disabled")]
    Disabled,
    /// Finalized capacity state could not be inspected.
    #[error(transparent)]
    Capacity(#[from] CapacityError),
    /// Durable outbox admission or state transition failed.
    #[error(transparent)]
    Outbox(#[from] ProviderIngestOutboxError),
    /// Supervised runtime construction failed.
    #[error(transparent)]
    Runtime(#[from] ProviderIngestRuntimeErrorV1),
    /// The one completed-Musubi capture tenure for this store was already reserved.
    #[error("completed-Musubi capture coordinator tenure is already taken")]
    CompletedMusubiCaptureCoordinatorTaken,
    /// Finalized pin, provider assignment, manifest, or CAR plan disagree.
    #[error("finalized-ledger provider ingest binding mismatch: {0}")]
    BindingMismatch(&'static str),
    /// The exact finalized authorization exhausted its bounded retries.
    #[error("finalized-ledger provider ingest is terminally dead-lettered")]
    DeadLettered,
    /// Canonical manifest encoding failed.
    #[error("failed to encode canonical provider-ingest manifest: {0}")]
    ManifestEncoding(String),
    /// Verified local storage ingest failed.
    #[error(transparent)]
    Storage(#[from] NodeStorageError),
}
/// Result of constructing a supervised finalized-ledger provider-ingest runtime.
pub type FinalizedProviderIngestRuntimeResultV1<
    Ledger,
    Fetch,
    Storage,
    Builder,
    Resolver,
    Ingress,
    Clock,
> = Result<
    ProviderIngestRuntimeV1<Ledger, Fetch, Storage, Builder, Resolver, Ingress, Clock>,
    FinalizedProviderIngestError,
>;
/// Errors returned by the embedded admission-bound PDP provider service.
#[derive(Debug, Error)]
pub enum NodePdpError {
    /// Storage and the durable provider protocol are disabled.
    #[error("SoraFS PDP provider service is disabled for this node")]
    Disabled,
    /// Durable challenge/proof lifecycle processing failed.
    #[error(transparent)]
    Protocol(#[from] PdpProviderProtocolError),
    /// Stored manifest or witness access failed.
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// Provider proof construction or signing failed.
    #[error(transparent)]
    ProofBuild(#[from] PdpProofBuildError),
    /// The retained manifest has no PDP commitment.
    #[error("stored SoraFS manifest has no PDP commitment")]
    CommitmentUnavailable,
    /// The queued challenge does not bind the retained manifest commitment.
    #[error("PDP challenge commitment does not match retained storage")]
    CommitmentMismatch,
    /// The active council admission does not authorise the locally configured signing key.
    #[error("configured PDP provider signing key is not authorised by active admission")]
    SigningKeyNotAdmitted,
    /// The configured provider signing key could not be loaded safely.
    #[error("failed to load configured PDP provider signing key: {0}")]
    SigningKey(String),
}
/// Errors that prevent a SoraFS node handle from starting with trustworthy state.
#[derive(Debug, Error)]
pub enum NodeInitError {
    /// The configured storage backend could not be opened or validated.
    #[error("failed to initialise SoraFS storage backend: {0}")]
    Storage(#[from] StorageError),
    /// The durable admission-bound PDP provider checkpoint could not be opened or validated.
    #[error(
        "failed to initialise SoraFS PDP provider checkpoint `{path}`: {message}",
        path = path.display()
    )]
    PdpProvider {
        /// Configured storage root containing the PDP checkpoint.
        path: PathBuf,
        /// Validation or I/O diagnostic.
        message: String,
    },
    /// The durable final signed PoTR receipt checkpoint could not be opened or validated.
    #[error(
        "failed to initialise SoraFS PoTR receipt checkpoint `{path}`: {message}",
        path = path.display()
    )]
    Potr {
        /// Configured storage root containing the PoTR checkpoint.
        path: PathBuf,
        /// Validation or I/O diagnostic.
        message: String,
    },
    /// The durable proof-outcome transaction outbox could not be opened or validated.
    #[error(
        "failed to initialise SoraFS proof-outcome outbox `{path}`: {message}",
        path = path.display()
    )]
    ProofOutcomeOutbox {
        /// Configured storage root containing the delivery checkpoint.
        path: PathBuf,
        /// Validation or I/O diagnostic.
        message: String,
    },
    /// The durable finalized-ledger provider-ingest outbox could not be opened.
    #[error(
        "failed to initialise SoraFS provider-ingest outbox `{path}`: {message}",
        path = path.display()
    )]
    ProviderIngestOutbox {
        /// Configured checkpoint path.
        path: PathBuf,
        /// Validation or I/O diagnostic.
        message: String,
    },
    /// The finalized-PoR replay archive is absent, unexpected, or no longer
    /// matches its exact public deployment binding.
    #[error("failed to qualify finalized PoR replay archive: {message}")]
    PorReplayArchive {
        /// Payload-free qualification diagnostic.
        message: String,
    },
    /// The durable native repair-transaction forwarder could not be opened or validated.
    #[error(
        "failed to initialise SoraFS repair transaction forwarder `{path}`: {message}",
        path = path.display()
    )]
    RepairTransactionForwarder {
        /// Configured storage root containing the repair delivery checkpoint.
        path: PathBuf,
        /// Validation or I/O diagnostic.
        message: String,
    },
    /// Native repair worker controls are invalid for the consensus lease contract.
    #[error("invalid SoraFS native repair configuration: {message}")]
    NativeRepairConfig {
        /// Stable configuration diagnostic.
        message: String,
    },
    /// The durable native orderbook-transaction forwarder could not be opened or validated.
    #[error(
        "failed to initialise SoraFS orderbook transaction forwarder `{path}`: {message}",
        path = path.display()
    )]
    OrderbookTransactionForwarder {
        /// Configured storage root containing the orderbook delivery checkpoint.
        path: PathBuf,
        /// Validation or I/O diagnostic.
        message: String,
    },
    /// Native reserve/rent worker controls are outside supported resource bounds.
    #[error("invalid SoraFS native reserve worker configuration: {message}")]
    ReserveWorkerConfig {
        /// Stable configuration diagnostic.
        message: String,
    },
    /// The durable native reserve-transaction forwarder could not be opened or validated.
    #[error(
        "failed to initialise SoraFS reserve transaction forwarder `{path}`: {message}",
        path = path.display()
    )]
    ReserveTransactionForwarder {
        /// Configured storage root containing the reserve delivery checkpoint.
        path: PathBuf,
        /// Validation or I/O diagnostic.
        message: String,
    },
    /// A durable runtime checkpoint could not be read, decoded, or validated.
    #[error(
        "failed to restore SoraFS {component} checkpoint `{path}`: {message}",
        path = path.display()
    )]
    Checkpoint {
        /// Logical checkpoint component.
        component: &'static str,
        /// Checkpoint path.
        path: PathBuf,
        /// Validation or I/O detail.
        message: String,
    },
    /// Governance publication was configured but its durable publisher could not start.
    #[error("failed to initialise SoraFS governance publisher: {0}")]
    GovernancePublisher(String),
    /// The configured external reputation trust policy could not be read or validated.
    #[error(
        "failed to load SoraFS reputation trust policy `{path}`: {message}",
        path = path.display()
    )]
    ReputationTrustPolicy {
        /// Configured trust-policy path.
        path: PathBuf,
        /// Validation or I/O diagnostic.
        message: String,
    },
    /// The configured external hedging-feed trust policy could not be read or validated.
    #[error(
        "failed to load SoraFS hedging-feed trust policy `{path}`: {message}",
        path = path.display()
    )]
    HedgingFeedTrustPolicy {
        /// Configured trust-policy path.
        path: PathBuf,
        /// Validation or I/O diagnostic.
        message: String,
    },
    /// The configured public hedging/billing service policy could not be read or validated.
    #[error(
        "failed to load SoraFS hedging/billing service policy `{path}`: {message}",
        path = path.display()
    )]
    HedgingBillingServicePolicy {
        /// Configured public service-policy path.
        path: PathBuf,
        /// Validation, digest, canonical-codec, or I/O diagnostic.
        message: String,
    },
    /// The config-pinned moderation screening authority bundle could not be
    /// read, digest-verified, canonically decoded, or validated.
    #[error(
        "failed to load SoraFS moderation screening authority bundle `{path}`: {message}",
        path = path.display()
    )]
    ModerationScreeningAuthorityBundle {
        /// Configured authority bundle path, or a configuration sentinel.
        path: PathBuf,
        /// Validation, digest, canonical-codec, or I/O diagnostic.
        message: String,
    },
    /// Authenticated screening was enabled without its runtime-only
    /// PKCS#11/KMS quarantine-key dependency.
    #[error(
        "SoraFS moderation screening requires an injected runtime-only PKCS#11/KMS quarantine key wrapper"
    )]
    ModerationQuarantineKeyWrapperUnavailable,
    /// The injected quarantine-key wrapper failed its exact public provider binding.
    #[error("failed to qualify the SoraFS moderation quarantine key wrapper: {message}")]
    ModerationQuarantineKeyWrapperInvalid {
        /// Stable, payload-free validation detail.
        message: String,
    },
    /// The threshold-PRF provider did not match the exact configured production pin.
    #[error("failed to qualify the SoraFS privacy-cycle threshold PRF provider: {error}")]
    PrivacyCyclePrfProviderQualification {
        /// Stable payload-free qualification failure.
        error: TransparencyRuntimeProviderQualificationErrorV1,
    },
    /// The finalized release anchor did not match the exact configured production pin.
    #[error("failed to qualify the SoraFS finalized privacy release anchor: {error}")]
    PrivacyReleaseAnchorQualification {
        /// Stable payload-free qualification failure.
        error: TransparencyRuntimeProviderQualificationErrorV1,
    },
    /// The external transparency leader-lease provider failed exact qualification.
    #[error("failed to qualify the SoraFS transparency leader-lease provider: {error}")]
    TransparencyLeaderLeaseProviderQualification {
        /// Stable payload-free qualification failure.
        error: TransparencyLeaderLeaseErrorV1,
    },
    /// Enabled privacy publication lacks a stable public holder identity.
    #[error("invalid SoraFS transparency leader-lease configuration: {message}")]
    TransparencyLeaderLeaseConfiguration {
        /// Stable non-secret configuration diagnostic.
        message: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimePersistence {
    Durable,
    ProcessLocal,
}
impl NodeInitError {
    fn checkpoint(component: &'static str, path: &Path, error: impl std::fmt::Display) -> Self {
        Self::Checkpoint {
            component,
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    }
}
fn validate_native_repair_config(config: &RepairConfig) -> Result<(), NodeInitError> {
    let lease_duration_ms = config.claim_ttl_secs().checked_mul(1_000).ok_or_else(|| {
        NodeInitError::NativeRepairConfig {
            message: "claim_ttl_secs overflows milliseconds".to_owned(),
        }
    })?;
    if !(REPAIR_LEDGER_MIN_LEASE_MS_V1..=REPAIR_LEDGER_MAX_LEASE_MS_V1).contains(&lease_duration_ms)
    {
        return Err(NodeInitError::NativeRepairConfig {
            message: format!(
                "claim_ttl_secs must resolve within {REPAIR_LEDGER_MIN_LEASE_MS_V1}..={REPAIR_LEDGER_MAX_LEASE_MS_V1} milliseconds"
            ),
        });
    }
    let renewal_lead_ms = config
        .heartbeat_interval_secs()
        .checked_mul(1_000)
        .ok_or_else(|| NodeInitError::NativeRepairConfig {
            message: "heartbeat_interval_secs overflows milliseconds".to_owned(),
        })?;
    if renewal_lead_ms == 0 || renewal_lead_ms >= lease_duration_ms {
        return Err(NodeInitError::NativeRepairConfig {
            message: "heartbeat_interval_secs must be non-zero and strictly below claim_ttl_secs"
                .to_owned(),
        });
    }
    if !(1..=iroha_config::parameters::defaults::sorafs::repair::MAX_ATTEMPTS_LIMIT)
        .contains(&config.max_attempts())
    {
        return Err(NodeInitError::NativeRepairConfig {
            message: format!(
                "max_attempts must be within 1..={}",
                iroha_config::parameters::defaults::sorafs::repair::MAX_ATTEMPTS_LIMIT
            ),
        });
    }
    if !(1..=iroha_config::parameters::defaults::sorafs::repair::WORKER_CONCURRENCY_LIMIT)
        .contains(&config.worker_concurrency())
    {
        return Err(NodeInitError::NativeRepairConfig {
            message: format!(
                "worker_concurrency must be within 1..={}",
                iroha_config::parameters::defaults::sorafs::repair::WORKER_CONCURRENCY_LIMIT
            ),
        });
    }
    Ok(())
}
fn load_reputation_trust_policy(
    path: &Path,
) -> Result<ReputationSnapshotTrustPolicyV1, NodeInitError> {
    let bytes = read_reputation_trust_policy_file(path).map_err(|error| {
        NodeInitError::ReputationTrustPolicy {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    decode_reputation_trust_policy(&bytes).map_err(|error| NodeInitError::ReputationTrustPolicy {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}
fn load_hedging_feed_trust_policy(path: &Path) -> Result<HedgingFeedTrustPolicyV1, NodeInitError> {
    let bytes = read_trust_policy_file(
        path,
        MAX_HEDGING_TRUST_POLICY_BYTES,
        "hedging-feed trust policy",
    )
    .map_err(|error| NodeInitError::HedgingFeedTrustPolicy {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    decode_hedging_feed_trust_policy(&bytes).map_err(|error| {
        NodeInitError::HedgingFeedTrustPolicy {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })
}
/// Load and digest-pin one canonical public hedging/billing service policy.
///
/// The file is required to be an absolute, ancestor-safe, regular single-link
/// path that is not writable by group or other users, and is opened without
/// following a terminal symlink on supported hosts.
///
/// # Errors
///
/// Rejects unsafe paths, oversized or noncanonical Norito, invalid policy
/// material, and any digest substitution.
pub fn load_hedging_billing_service_policy(
    path: &Path,
    expected_digest: [u8; 32],
) -> Result<hedging_billing_service::HedgingBillingServicePolicyV1, NodeInitError> {
    let bytes = read_trust_policy_file(
        path,
        hedging_billing_service::HEDGING_BILLING_SERVICE_POLICY_MAX_BYTES_V1,
        "hedging/billing service policy",
    )
    .map_err(|error| NodeInitError::HedgingBillingServicePolicy {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let policy =
        hedging_billing_service::HedgingBillingServicePolicyV1::from_canonical_bytes(&bytes)
            .map_err(|error| NodeInitError::HedgingBillingServicePolicy {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;
    let observed_digest =
        policy
            .canonical_digest()
            .map_err(|error| NodeInitError::HedgingBillingServicePolicy {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;
    if observed_digest != expected_digest {
        return Err(NodeInitError::HedgingBillingServicePolicy {
            path: path.to_path_buf(),
            message: "canonical digest does not match iroha_config anchor".to_owned(),
        });
    }
    Ok(policy)
}
fn load_moderation_screening_authority_bundle(
    path: &Path,
    expected_digest: [u8; 32],
    now_unix: u64,
) -> Result<ModerationScreeningAuthorityV1, NodeInitError> {
    let error = |message: String| NodeInitError::ModerationScreeningAuthorityBundle {
        path: path.to_path_buf(),
        message,
    };
    if !path.is_absolute() {
        return Err(error(
            "configured authority bundle path must be absolute".to_owned(),
        ));
    }
    if expected_digest == [0; 32] {
        return Err(error(
            "configured authority bundle digest must be non-zero".to_owned(),
        ));
    }
    let bytes = read_trust_policy_file(
        path,
        MODERATION_SCREENING_AUTHORITY_BUNDLE_MAX_BYTES_V1,
        "moderation screening authority bundle",
    )
    .map_err(|read_error| error(read_error.to_string()))?;
    let actual_digest = *blake3::hash(&bytes).as_bytes();
    if actual_digest != expected_digest {
        return Err(error(format!(
            "bundle digest mismatch: expected {}, got {}",
            hex::encode(expected_digest),
            hex::encode(actual_digest)
        )));
    }
    let bundle = decode_local_checkpoint_canonical::<ModerationScreeningAuthorityBundleV1>(
        &bytes,
        u64::try_from(MODERATION_SCREENING_AUTHORITY_BUNDLE_MAX_BYTES_V1).unwrap_or(u64::MAX),
        4_096,
    )
    .map_err(error)?;
    bundle.into_authority(now_unix).map_err(|authority_error| {
        NodeInitError::ModerationScreeningAuthorityBundle {
            path: path.to_path_buf(),
            message: authority_error.to_string(),
        }
    })
}
/// Errors raised while computing reconciliation summaries.
#[derive(Debug, Error)]
pub enum ReconciliationError {
    /// Timestamp must be non-zero.
    #[error("reconciliation timestamp must be non-zero")]
    InvalidTimestamp,
    /// Storage backend is required for reconciliation.
    #[error("SoraFS storage backend is disabled")]
    StorageDisabled,
    /// Local provider identity is required to bind the published report.
    #[error("SoraFS provider identity is unavailable for reconciliation")]
    ProviderBindingUnavailable,
    /// Failed to encode reconciliation snapshot data.
    #[error(transparent)]
    Norito(#[from] norito::Error),
    /// Local appeal-finance Governance DAG data could not be reconciled.
    #[error("appeal finance reconciliation failed: {0}")]
    AppealFinance(String),
    /// Reconciliation report failed validation.
    #[error(transparent)]
    Validation(#[from] ReconciliationValidationError),
}
impl PdpTerminalHandoff for NodeHandle {
    fn archive(
        &self,
        idempotency_key: [u8; 32],
        archive: &PdpGovernanceArchiveV1,
    ) -> Result<[u8; 32], pdp_provider::PdpExternalHandoffError> {
        self.archive_pdp_terminal_outcome(idempotency_key, archive)
    }
    fn repair(
        &self,
        idempotency_key: [u8; 32],
        report: &RepairReportV1,
    ) -> Result<[u8; 32], pdp_provider::PdpExternalHandoffError> {
        let _ = (idempotency_key, report);
        Err(pdp_provider::PdpExternalHandoffError(
            "chain-authoritative repair transaction handoff is required".to_owned(),
        ))
    }
}
impl NodeHandle {
    /// Durably archive one terminal PDP outcome and enqueue its proof-ledger projection.
    pub fn archive_pdp_terminal_outcome(
        &self,
        idempotency_key: [u8; 32],
        archive: &PdpGovernanceArchiveV1,
    ) -> Result<[u8; 32], pdp_provider::PdpExternalHandoffError> {
        archive.validate().map_err(|error| {
            pdp_provider::PdpExternalHandoffError(format!(
                "validate PDP governance archive handoff: {error}"
            ))
        })?;
        let bytes = norito::to_bytes(archive).map_err(|error| {
            pdp_provider::PdpExternalHandoffError(format!(
                "encode PDP governance archive handoff: {error}"
            ))
        })?;
        self.proof_outcome_outbox
            .enqueue_pdp(archive)
            .map_err(|error| {
                pdp_provider::PdpExternalHandoffError(format!(
                    "persist PDP proof-outcome delivery: {error}"
                ))
            })?;
        self.enqueue_governance_outbox(GovernanceOutboxKindV1::PdpArchive, bytes.clone())
            .map_err(|error| pdp_provider::PdpExternalHandoffError(error.to_string()))?;
        self.flush_governance_outbox()
            .map_err(|error| pdp_provider::PdpExternalHandoffError(error.to_string()))?;
        Ok(pdp_handoff_receipt(
            b"archive",
            idempotency_key,
            *blake3::hash(&bytes).as_bytes(),
        ))
    }
    /// Durably enqueue and publish one canonical PoR challenge envelope.
    pub fn publish_por_challenge_publication(
        &self,
        publication: PorChallengePublicationV1,
    ) -> Result<(), GovernancePublishError> {
        publication.validate().map_err(|error| {
            GovernancePublishError::other(format!("invalid PoR challenge publication: {error}"))
        })?;
        let encoded = norito::to_bytes(&publication).map_err(|error| {
            GovernancePublishError::other(format!("encode PoR challenge publication: {error}"))
        })?;
        self.enqueue_governance_outbox(GovernanceOutboxKindV1::PorChallengePublication, encoded)?;
        self.flush_governance_outbox()?;
        Ok(())
    }
    /// Durably enqueue and publish one validated PoR weekly report.
    pub fn publish_por_weekly_report(
        &self,
        report: PorWeeklyReportV1,
    ) -> Result<(), GovernancePublishError> {
        report.validate().map_err(|error| {
            GovernancePublishError::other(format!("invalid PoR weekly report: {error}"))
        })?;
        let encoded = norito::to_bytes(&report).map_err(|error| {
            GovernancePublishError::other(format!("encode PoR weekly report: {error}"))
        })?;
        self.enqueue_governance_outbox(GovernanceOutboxKindV1::PorWeeklyReport, encoded)?;
        self.flush_governance_outbox()?;
        Ok(())
    }
}
impl NodeHandle {
    /// Durably enqueue one final PoTR receipt for native proof-ledger submission.
    pub fn enqueue_potr_proof_outcome(
        &self,
        source_identity: [u8; 32],
        receipt: &PotrReceiptV1,
        admission_envelope_digest: [u8; 32],
    ) -> Result<[u8; 32], potr::PotrRepairHandoffError> {
        if receipt.signed_receipt_digest().ok() != Some(source_identity) {
            return Err(potr::PotrRepairHandoffError(
                "PoTR proof-outcome source identity does not match the signed receipt".to_owned(),
            ));
        }
        self.proof_outcome_outbox
            .enqueue_potr(receipt, admission_envelope_digest)
            .map(ProofOutcomeEnqueueResultV1::operation_id)
            .map_err(|error| potr::PotrRepairHandoffError(error.to_string()))
    }
}
fn pdp_handoff_receipt(
    kind: &[u8],
    idempotency_key: [u8; 32],
    payload_digest: [u8; 32],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs.node.pdp.handoff-receipt.v1\0");
    hasher.update(&(kind.len() as u64).to_le_bytes());
    hasher.update(kind);
    hasher.update(&idempotency_key);
    hasher.update(&payload_digest);
    *hasher.finalize().as_bytes()
}
impl NodeHandle {
    /// Construct a new handle for the embedded storage worker.
    ///
    /// # Panics
    ///
    /// Panics when durable storage or checkpoint state cannot be trusted. Runtime
    /// integrations should use [`Self::try_new`] and surface the startup error.
    #[must_use]
    pub fn new(config: StorageConfig) -> Self {
        Self::try_new(config)
            .unwrap_or_else(|err| panic!("failed to initialise SoraFS node: {err}"))
    }
    /// Construct a new handle and report storage/checkpoint startup failures.
    ///
    /// # Errors
    ///
    /// Returns an error when configured durable state cannot be opened, decoded,
    /// validated, or when a configured governance publisher cannot start.
    pub fn try_new(config: StorageConfig) -> Result<Self, NodeInitError> {
        Self::try_new_with_policies(config, RepairConfig::default(), GcConfig::default())
    }
    /// Construct a new handle with a runtime-only PKCS#11/KMS quarantine-key wrapper.
    ///
    /// # Errors
    ///
    /// Returns an error when the wrapper does not match its exact configured
    /// provider binding or durable state cannot be opened and authenticated.
    pub fn try_new_with_quarantine_key_wrapper(
        config: StorageConfig,
        key_wrapper: Arc<dyn ModerationQuarantineKeyWrapper>,
    ) -> Result<Self, NodeInitError> {
        Self::try_new_with_runtime_deps(
            config,
            NodeRuntimeDeps::default().with_moderation_quarantine_key_wrapper(key_wrapper),
        )
    }
    /// Construct a new handle with deployment-owned runtime dependencies.
    ///
    /// # Errors
    ///
    /// Returns an error when a configured service is missing its runtime
    /// dependency or durable state cannot be trusted.
    pub fn try_new_with_runtime_deps(
        config: StorageConfig,
        runtime_deps: NodeRuntimeDeps,
    ) -> Result<Self, NodeInitError> {
        Self::try_new_with_policies_and_runtime_deps(
            config,
            RepairConfig::default(),
            GcConfig::default(),
            runtime_deps,
        )
    }
    /// Construct a new handle with explicit repair/GC policies.
    ///
    /// # Panics
    ///
    /// Panics when durable storage or checkpoint state cannot be trusted. Runtime
    /// integrations should use [`Self::try_new_with_policies`].
    #[must_use]
    pub fn new_with_policies(
        config: StorageConfig,
        repair_config: RepairConfig,
        gc_config: GcConfig,
    ) -> Self {
        Self::try_new_with_policies(config, repair_config, gc_config)
            .unwrap_or_else(|err| panic!("failed to initialise SoraFS node: {err}"))
    }
    /// Construct a new handle with explicit policies and fallible durable startup.
    ///
    /// # Errors
    ///
    /// Returns an error when configured durable state cannot be opened, decoded,
    /// validated, or when a configured governance publisher cannot start.
    pub fn try_new_with_policies(
        config: StorageConfig,
        repair_config: RepairConfig,
        gc_config: GcConfig,
    ) -> Result<Self, NodeInitError> {
        Self::try_new_with_policies_and_runtime_deps(
            config,
            repair_config,
            gc_config,
            NodeRuntimeDeps::default(),
        )
    }
    /// Construct a new handle with explicit policies and a runtime-only
    /// PKCS#11/KMS quarantine-key wrapper.
    ///
    /// # Errors
    ///
    /// Returns an error when the wrapper does not match its exact configured
    /// provider binding or durable state cannot be opened and authenticated.
    pub fn try_new_with_policies_and_quarantine_key_wrapper(
        config: StorageConfig,
        repair_config: RepairConfig,
        gc_config: GcConfig,
        key_wrapper: Arc<dyn ModerationQuarantineKeyWrapper>,
    ) -> Result<Self, NodeInitError> {
        Self::try_new_with_policies_and_runtime_deps(
            config,
            repair_config,
            gc_config,
            NodeRuntimeDeps::default().with_moderation_quarantine_key_wrapper(key_wrapper),
        )
    }
    /// Construct a new handle with explicit policies and deployment-owned runtime dependencies.
    ///
    /// # Errors
    ///
    /// Returns an error when a configured service is missing its runtime dependency, a dependency
    /// exposes an invalid public handle, or durable state cannot be trusted.
    pub fn try_new_with_policies_and_runtime_deps(
        config: StorageConfig,
        repair_config: RepairConfig,
        gc_config: GcConfig,
        runtime_deps: NodeRuntimeDeps,
    ) -> Result<Self, NodeInitError> {
        Self::try_new_with_runtime_persistence(
            config,
            repair_config,
            gc_config,
            runtime_deps,
            RuntimePersistence::Durable,
        )
    }
    /// Construct a disabled, process-local handle without opening any durable runtime state.
    ///
    /// This constructor is reserved for emergency read-only startup. It ignores production
    /// worker policy, installs fixed disabled storage/repair/GC policy, and keeps PoTR plus every
    /// transaction-forwarder outbox in memory so startup cannot create, scan, recover, or rewrite
    /// their filesystem trees.
    ///
    /// # Errors
    ///
    /// Returns an error when the bounded in-memory runtime policy is invalid.
    pub fn try_new_emergency_disabled(data_dir: PathBuf) -> Result<Self, NodeInitError> {
        let config = StorageConfig::builder()
            .enabled(false)
            .data_dir(data_dir)
            .runtime_retention(config::RuntimeRetentionPolicy::new(1, 1, 1024 * 1024))
            .build();
        Self::try_new_with_runtime_persistence(
            config,
            RepairConfig::default(),
            GcConfig::default(),
            NodeRuntimeDeps::default(),
            RuntimePersistence::ProcessLocal,
        )
    }
    fn try_new_with_runtime_persistence(
        config: StorageConfig,
        repair_config: RepairConfig,
        gc_config: GcConfig,
        runtime_deps: NodeRuntimeDeps,
        runtime_persistence: RuntimePersistence,
    ) -> Result<Self, NodeInitError> {
        validate_native_repair_config(&repair_config)?;
        let NodeRuntimeDeps {
            moderation_quarantine_key_wrapper,
            privacy_cycle_prf_provider,
            privacy_release_anchor,
            transparency_leader_lease_provider,
            fenced_transparency_publisher,
            fenced_transparency_head_reader,
            governance_dag_signer,
            governance_dag_checkpoint_store,
            provider_ingest_checkpoint_runtime,
            por_finalized_replay_archive,
        } = runtime_deps;
        if config.por_replay_archive_policy().is_some() && !config.enabled() {
            return Err(NodeInitError::PorReplayArchive {
                message: "configured archive requires enabled durable SoraFS storage".to_owned(),
            });
        }
        let por_finalized_replay_archive = match (
            config.por_replay_archive_policy(),
            por_finalized_replay_archive,
        ) {
            (Some(policy), Some(archive)) => {
                verify_por_replay_archive_provider(policy, archive.as_ref()).map_err(|_| {
                    NodeInitError::PorReplayArchive {
                        message:
                            "runtime provider is unavailable, stale, substituted, or unqualified"
                                .to_owned(),
                    }
                })?;
                Some(OpaquePorFinalizedReplayArchive(archive))
            }
            (Some(_), None) => {
                return Err(NodeInitError::PorReplayArchive {
                    message: "configured archive runtime provider is missing".to_owned(),
                });
            }
            (None, Some(_)) => {
                return Err(NodeInitError::PorReplayArchive {
                    message: "unrequested archive runtime provider was injected".to_owned(),
                });
            }
            (None, None) => None,
        };
        let governance_dir = config.governance_dir().cloned();
        let privacy_publication_required = config.privacy_aggregate_schedule().is_some()
            && config.privacy_aggregate_policy().is_some();
        if privacy_publication_required && governance_dir.is_none() {
            return Err(NodeInitError::GovernancePublisher(
                "enabled privacy publication requires an explicit signed Governance DAG directory and complete runtime signer binding"
                    .to_owned(),
            ));
        }
        let governance_dag_publisher_peer_id = config.governance_dag_publisher_peer_id().cloned();
        let governance_dag_signer_handle = config.governance_dag_signer_handle().cloned();
        let governance_dag_signer_qualification = config.governance_dag_signer_qualification();
        let governance_dag_checkpoint_store_handle =
            config.governance_dag_checkpoint_store_handle().cloned();
        let governance_dag_checkpoint_store_qualification =
            config.governance_dag_checkpoint_store_qualification();
        let governance_dag_publisher_public_key_hex =
            config.governance_dag_publisher_public_key_hex().cloned();
        if governance_dir.is_some() && !config.enabled() {
            return Err(NodeInitError::GovernancePublisher(
                "configured Governance DAG publication requires storage.enabled".to_owned(),
            ));
        }
        let any_governance_dag_signer_binding = governance_dag_publisher_peer_id.is_some()
            || governance_dag_signer_handle.is_some()
            || governance_dag_signer_qualification.is_some()
            || governance_dag_checkpoint_store_handle.is_some()
            || governance_dag_checkpoint_store_qualification.is_some()
            || governance_dag_publisher_public_key_hex.is_some();
        if governance_dir.is_none()
            && (any_governance_dag_signer_binding
                || governance_dag_signer.is_some()
                || governance_dag_checkpoint_store.is_some())
        {
            return Err(NodeInitError::GovernancePublisher(
                "Governance DAG runtime providers are forbidden without a configured Governance DAG directory"
                    .to_owned(),
            ));
        }
        if governance_dir.is_some()
            && (governance_dag_publisher_peer_id.is_none()
                || governance_dag_signer_handle.is_none()
                || governance_dag_signer_qualification.is_none()
                || governance_dag_checkpoint_store_handle.is_none()
                || governance_dag_checkpoint_store_qualification.is_none()
                || governance_dag_publisher_public_key_hex.is_none()
                || governance_dag_signer.is_none()
                || governance_dag_checkpoint_store.is_none())
        {
            return Err(NodeInitError::GovernancePublisher(
                "configured Governance DAG publication requires peer id, signer handle, signer revision, signer policy digest, public key, an injected runtime signer, and an exact sealed producer checkpoint-store binding"
                    .to_owned(),
            ));
        }
        let governance_dag_runtime_binding = if governance_dir.is_some() {
            let peer_id = governance_dag_publisher_peer_id
                .expect("configured Governance DAG peer id checked above");
            let handle = governance_dag_signer_handle
                .expect("configured Governance DAG signer handle checked above");
            let qualification = governance_dag_signer_qualification
                .expect("configured Governance DAG signer qualification checked above");
            let public_key_hex = governance_dag_publisher_public_key_hex
                .expect("configured Governance DAG public key checked above");
            if public_key_hex.len() != 64
                || !public_key_hex
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            {
                return Err(NodeInitError::GovernancePublisher(
                    "Governance DAG publisher public key is not canonical lowercase hex".to_owned(),
                ));
            }
            let public_key: [u8; 32] = hex::decode(&public_key_hex)
                .expect("validated lowercase public key hex")
                .try_into()
                .expect("validated 32-byte public key");
            let signer =
                governance_dag_signer.expect("injected Governance DAG signer checked above");
            let signer = qualify_governance_dag_runtime_signer_provider(
                handle,
                peer_id.into_bytes(),
                public_key,
                qualification,
                signer,
            )
            .map_err(|error| NodeInitError::GovernancePublisher(error.to_string()))?;
            Some(signer)
        } else {
            None
        };
        let governance_dag_checkpoint_store_binding = if governance_dir.is_some() {
            let provider = governance_dag_checkpoint_store
                .expect("injected Governance DAG checkpoint store checked above");
            let handle = governance_dag_checkpoint_store_handle
                .expect("configured Governance DAG checkpoint-store handle checked above");
            let qualification = governance_dag_checkpoint_store_qualification
                .expect("configured Governance DAG checkpoint-store qualification checked above");
            Some(
                qualify_governance_dag_runtime_checkpoint_store(handle, qualification, provider)
                    .map_err(|error| NodeInitError::GovernancePublisher(error.to_string()))?,
            )
        } else {
            None
        };
        let governance_publication_lock = governance_dir.as_ref().map(|_| Arc::new(Mutex::new(())));
        let moderation_screening_authority = if config.moderation_screening_enabled() {
            if !config.enabled() {
                return Err(NodeInitError::ModerationScreeningAuthorityBundle {
                    path: PathBuf::from("<iroha_config>"),
                    message:
                        "moderation screening requires the durable SoraFS storage worker to be enabled"
                            .to_owned(),
                });
            }
            let path = config
                .moderation_screening_authority_bundle_path()
                .ok_or_else(|| NodeInitError::ModerationScreeningAuthorityBundle {
                    path: PathBuf::from("<iroha_config>"),
                    message: "enabled moderation screening requires an authority bundle path"
                        .to_owned(),
                })?;
            let expected_digest = config
                .moderation_screening_authority_bundle_digest()
                .ok_or_else(|| NodeInitError::ModerationScreeningAuthorityBundle {
                    path: path.clone(),
                    message:
                        "enabled moderation screening requires a non-zero configured bundle digest"
                            .to_owned(),
                })?;
            Some(load_moderation_screening_authority_bundle(
                path,
                expected_digest,
                unix_now_secs(),
            )?)
        } else {
            None
        };
        let configured_quarantine_key_provider =
            config.moderation_quarantine_key_provider().cloned();
        let moderation_quarantine_key_provider_binding = match (
            configured_quarantine_key_provider,
            moderation_quarantine_key_wrapper.as_deref(),
        ) {
            (Some(configured), Some(key_wrapper)) => {
                let binding = ModerationQuarantineKeyProviderBindingV1::try_new(
                    configured.handle,
                    ModerationQuarantineKeyProviderQualificationV1::new(
                        configured.revision,
                        configured.policy_digest,
                    ),
                )
                .map_err(|_| {
                    NodeInitError::ModerationQuarantineKeyWrapperInvalid {
                        message: "configured moderation quarantine key provider binding is invalid"
                            .to_owned(),
                    }
                })?;
                validate_moderation_quarantine_key_wrapper(&binding, key_wrapper).map_err(
                    |_| NodeInitError::ModerationQuarantineKeyWrapperInvalid {
                        message: "runtime moderation quarantine key provider failed qualification"
                            .to_owned(),
                    },
                )?;
                Some(binding)
            }
            (Some(_), None) => {
                return Err(NodeInitError::ModerationQuarantineKeyWrapperUnavailable);
            }
            (None, Some(_)) => {
                return Err(NodeInitError::ModerationQuarantineKeyWrapperInvalid {
                    message: "runtime moderation quarantine key provider has no configured binding"
                        .to_owned(),
                });
            }
            (None, None) if config.moderation_screening_enabled() => {
                return Err(NodeInitError::ModerationQuarantineKeyWrapperUnavailable);
            }
            (None, None) => None,
        };
        let privacy_cycle_prf_required = config.privacy_aggregate_schedule().is_some()
            && config
                .privacy_aggregate_policy()
                .is_some_and(config::PrivacyAggregatePolicyConfig::requires_cycle_prf);
        let privacy_release_anchor_required = privacy_publication_required;
        let privacy_cycle_prf_provider = match (
            privacy_cycle_prf_required,
            config.privacy_cycle_prf_provider_binding().cloned(),
            privacy_cycle_prf_provider,
        ) {
            (true, Some(binding), provider) => Some(Arc::new(
                QualifiedPrivacyCyclePrfProviderV1::try_new(binding, provider).map_err(
                    |error| NodeInitError::PrivacyCyclePrfProviderQualification { error },
                )?,
            )),
            (true, None, _) => {
                return Err(NodeInitError::PrivacyCyclePrfProviderQualification {
                    error:
                        TransparencyRuntimeProviderQualificationErrorV1::MissingConfiguredBinding,
                });
            }
            (false, None, None) => None,
            (false, Some(_), _) => {
                return Err(NodeInitError::PrivacyCyclePrfProviderQualification {
                    error:
                        TransparencyRuntimeProviderQualificationErrorV1::UnexpectedConfiguredBinding,
                });
            }
            (false, None, Some(_)) => {
                return Err(NodeInitError::PrivacyCyclePrfProviderQualification {
                    error: TransparencyRuntimeProviderQualificationErrorV1::UnexpectedProvider,
                });
            }
        };
        let privacy_release_anchor = match (
            privacy_release_anchor_required,
            config.privacy_release_anchor_provider_binding().cloned(),
            privacy_release_anchor,
        ) {
            (true, Some(binding), provider) => Some(Arc::new(
                QualifiedPrivacyReleaseAnchorV1::try_new(binding, provider)
                    .map_err(|error| NodeInitError::PrivacyReleaseAnchorQualification { error })?,
            )),
            (true, None, _) => {
                return Err(NodeInitError::PrivacyReleaseAnchorQualification {
                    error:
                        TransparencyRuntimeProviderQualificationErrorV1::MissingConfiguredBinding,
                });
            }
            (false, None, None) => None,
            (false, Some(_), _) => {
                return Err(NodeInitError::PrivacyReleaseAnchorQualification {
                    error:
                        TransparencyRuntimeProviderQualificationErrorV1::UnexpectedConfiguredBinding,
                });
            }
            (false, None, Some(_)) => {
                return Err(NodeInitError::PrivacyReleaseAnchorQualification {
                    error: TransparencyRuntimeProviderQualificationErrorV1::UnexpectedProvider,
                });
            }
        };
        let transparency_leader_lease_required = privacy_release_anchor_required;
        if transparency_leader_lease_required
            && (!config.enabled()
                || config
                    .provider_id()
                    .is_none_or(|provider_id| provider_id.0 == [0; 32]))
        {
            return Err(NodeInitError::TransparencyLeaderLeaseConfiguration {
                message: "enabled privacy publication requires storage.enabled and a nonzero provider_id",
            });
        }
        let transparency_leader_lease_provider = match (
            transparency_leader_lease_required,
            config.privacy_leader_lease_provider_binding().cloned(),
            transparency_leader_lease_provider,
        ) {
            (true, Some(binding), provider) => Some(Arc::new(
                QualifiedTransparencyLeaderLeaseProviderV1::try_new(binding, provider, 0).map_err(
                    |error| NodeInitError::TransparencyLeaderLeaseProviderQualification { error },
                )?,
            )),
            (true, None, _) => {
                return Err(
                    NodeInitError::TransparencyLeaderLeaseProviderQualification {
                        error: TransparencyRuntimeProviderQualificationErrorV1::MissingConfiguredBinding
                            .into(),
                    },
                );
            }
            (false, None, None) => None,
            (false, Some(_), _) => {
                return Err(
                    NodeInitError::TransparencyLeaderLeaseProviderQualification {
                        error: TransparencyRuntimeProviderQualificationErrorV1::UnexpectedConfiguredBinding
                            .into(),
                    },
                );
            }
            (false, None, Some(_)) => {
                return Err(
                    NodeInitError::TransparencyLeaderLeaseProviderQualification {
                        error: TransparencyRuntimeProviderQualificationErrorV1::UnexpectedProvider
                            .into(),
                    },
                );
            }
        };
        let qualified_fenced_privacy_runtime = match (
            privacy_publication_required,
            config.privacy_fenced_publisher_binding().cloned(),
            fenced_transparency_publisher,
            fenced_transparency_head_reader,
        ) {
            (true, Some(binding), Some(publisher), Some(head_reader)) => {
                let qualification = binding.qualification();
                let qualification = GovernanceDagRuntimeProviderQualificationV1::new(
                    qualification.revision(),
                    qualification.policy_digest(),
                );
                let handle = binding.handle().to_owned();
                let publisher = QualifiedFencedTransparencyPublisherV1::try_new(
                    handle.clone(),
                    qualification,
                    publisher,
                )
                .map_err(|error| NodeInitError::GovernancePublisher(error.to_string()))?;
                let head_reader = QualifiedFencedTransparencyHeadReaderV1::try_new(
                    handle,
                    qualification,
                    head_reader,
                )
                .map_err(|error| NodeInitError::GovernancePublisher(error.to_string()))?;
                Some((publisher, head_reader))
            }
            (true, None, _, _) => {
                return Err(NodeInitError::GovernancePublisher(
                    "enabled privacy publication requires an exact configured fused target binding"
                        .to_owned(),
                ));
            }
            (true, Some(_), None, _) => {
                return Err(NodeInitError::GovernancePublisher(
                    "enabled privacy publication requires an injected fused target writer"
                        .to_owned(),
                ));
            }
            (true, Some(_), Some(_), None) => {
                return Err(NodeInitError::GovernancePublisher(
                    "enabled privacy publication requires an injected authenticated authoritative-head reader"
                        .to_owned(),
                ));
            }
            (false, None, None, None) => None,
            (false, Some(_), _, _) => {
                return Err(NodeInitError::GovernancePublisher(
                    "fused privacy publisher binding is unexpected while privacy publication is disabled"
                        .to_owned(),
                ));
            }
            (false, None, Some(_), _) => {
                return Err(NodeInitError::GovernancePublisher(
                    "fused privacy target writer is unexpected while privacy publication is disabled"
                        .to_owned(),
                ));
            }
            (false, None, None, Some(_)) => {
                return Err(NodeInitError::GovernancePublisher(
                    "fused privacy authoritative-head reader is unexpected while privacy publication is disabled"
                        .to_owned(),
                ));
            }
        };
        let reputation_trust_policy = config
            .reputation_trust_policy_path()
            .map(|path| load_reputation_trust_policy(path))
            .transpose()?
            .map(Arc::new);
        let hedging_feed_trust_policy = config
            .hedging_feed_trust_policy_path()
            .map(|path| load_hedging_feed_trust_policy(path))
            .transpose()?
            .map(Arc::new);
        let gc_config = gc_config.with_default_state_dir(config.data_dir());
        let scheduler_config = StorageSchedulerConfig::from_storage_config(&config);
        let schedulers = StorageSchedulersRuntime::new(scheduler_config);
        let capacity_limit = config.max_capacity_bytes().0;
        let storage = if config.enabled() {
            match StorageBackend::new(config.clone()) {
                Ok(backend) => {
                    let backend = Arc::new(backend);
                    schedulers.update_storage_bytes(backend.total_bytes(), capacity_limit);
                    Some(backend)
                }
                Err(err) => return Err(NodeInitError::Storage(err)),
            }
        } else {
            schedulers.update_storage_bytes(0, capacity_limit);
            None
        };
        let pdp_provider_state_dir = config.data_dir().join("pdp-provider");
        let pdp_provider = storage
            .as_ref()
            .map(|_| {
                PdpProviderProtocol::open(config.pdp_provider_policy(), &pdp_provider_state_dir)
                    .map_err(|error| NodeInitError::PdpProvider {
                        path: pdp_provider_state_dir.clone(),
                        message: error.to_string(),
                    })
            })
            .transpose()?;
        let smoothing = config.smoothing_config();
        let event_history_limit = config.runtime_retention().event_history_limit();
        let state_entry_limit = config.runtime_retention().state_entry_limit();
        let provider_ingest_outbox_path = config.data_dir().join(PROVIDER_INGEST_OUTBOX_FILE_V1);
        let provider_ingest_outbox_policy = config.provider_ingest_outbox_policy();
        let provider_ingest_checkpoint_provider =
            config.provider_ingest_checkpoint_provider().cloned();
        let provider_ingest_outbox = if storage.is_none() {
            if provider_ingest_outbox_policy.is_some()
                || provider_ingest_checkpoint_provider.is_some()
                || provider_ingest_checkpoint_runtime.is_some()
            {
                return Err(NodeInitError::ProviderIngestOutbox {
                    path: provider_ingest_outbox_path,
                    message: "provider-ingest checkpoint authority is unexpected while storage is disabled"
                        .to_owned(),
                });
            }
            None
        } else {
            match (
                provider_ingest_outbox_policy,
                provider_ingest_checkpoint_provider,
                provider_ingest_checkpoint_runtime,
            ) {
                (None, None, None) => None,
                (Some(policy), Some(binding), Some(runtime)) => Some(
                    ProviderIngestOutbox::open_with_checkpoint_authority(
                        &provider_ingest_outbox_path,
                        policy,
                        binding,
                        runtime,
                    )
                    .map_err(|error| NodeInitError::ProviderIngestOutbox {
                        path: provider_ingest_outbox_path.clone(),
                        message: error.to_string(),
                    })?,
                ),
                #[cfg(test)]
                (Some(policy), None, None) => Some(
                    ProviderIngestOutbox::open(&provider_ingest_outbox_path, policy).map_err(
                        |error| NodeInitError::ProviderIngestOutbox {
                            path: provider_ingest_outbox_path.clone(),
                            message: error.to_string(),
                        },
                    )?,
                ),
                _ => {
                    return Err(NodeInitError::ProviderIngestOutbox {
                        path: provider_ingest_outbox_path,
                        message: "enabled provider ingest requires exactly one configured and injected production sealed checkpoint provider; disabled provider ingest rejects unexpected providers"
                            .to_owned(),
                    });
                }
            }
        };
        let potr_state_dir = config.data_dir().join("potr-receipts");
        // Strict receipt admission remains durable even when this process does not host provider
        // storage. The process-local branch is reachable only through the mutation-disabled Fast
        // facade and is never exposed to transaction admission or background workers.
        let potr = match runtime_persistence {
            RuntimePersistence::Durable => PotrTracker::open(
                &potr_state_dir,
                state_entry_limit,
                config.runtime_retention().checkpoint_max_bytes(),
            ),
            RuntimePersistence::ProcessLocal => PotrTracker::in_memory(state_entry_limit),
        }
        .map_err(|error| NodeInitError::Potr {
            path: potr_state_dir.clone(),
            message: error.to_string(),
        })?;
        let proof_outcome_outbox_state_dir = config.data_dir().join("proof-outcome-forwarder");
        let proof_outcome_outbox_policy = ProofOutcomeOutboxPolicyV1 {
            max_pending: state_entry_limit,
            max_completed: state_entry_limit,
            max_dead_letters: state_entry_limit,
            max_attempts: config.runtime_retention().proof_outcome_max_attempts(),
            checkpoint_max_bytes: config.runtime_retention().checkpoint_max_bytes(),
        };
        // Strict proof-ledger handoff is a validator durability boundary independent of provider
        // storage. Fast uses an unreachable process-local placeholder so it need not touch this
        // checkpoint tree merely to serve degraded reads.
        let proof_outcome_outbox = match runtime_persistence {
            RuntimePersistence::Durable => ProofOutcomeOutbox::open(
                &proof_outcome_outbox_state_dir,
                proof_outcome_outbox_policy,
            ),
            RuntimePersistence::ProcessLocal => {
                ProofOutcomeOutbox::in_memory(proof_outcome_outbox_policy)
            }
        }
        .map_err(|error| NodeInitError::ProofOutcomeOutbox {
            path: proof_outcome_outbox_state_dir.clone(),
            message: error.to_string(),
        })?;
        let repair_transaction_state_dir = config.data_dir().join("repair-transaction-forwarder");
        let repair_transaction_policy = RepairTransactionForwarderPolicyV1 {
            max_pending: state_entry_limit,
            max_completed: state_entry_limit,
            max_dead_letters: state_entry_limit,
            max_attempts: repair_config.max_attempts(),
            max_transaction_bytes: REPAIR_TRANSACTION_MAX_CANONICAL_BYTES_V1,
            checkpoint_max_bytes: config.runtime_retention().checkpoint_max_bytes(),
        };
        // Strict native repair operations remain durable on validator-only nodes. Fast has no
        // repair admission or worker and therefore uses an unreachable process-local placeholder.
        let repair_transaction_forwarder = match runtime_persistence {
            RuntimePersistence::Durable => RepairTransactionForwarder::open(
                &repair_transaction_state_dir,
                repair_transaction_policy,
            ),
            RuntimePersistence::ProcessLocal => {
                RepairTransactionForwarder::in_memory(repair_transaction_policy)
            }
        }
        .map_err(|error| NodeInitError::RepairTransactionForwarder {
            path: repair_transaction_state_dir.clone(),
            message: error.to_string(),
        })?;
        let orderbook_worker_policy = config.orderbook_worker_policy();
        let orderbook_transaction_state_dir =
            config.data_dir().join("orderbook-transaction-forwarder");
        let orderbook_transaction_policy = OrderbookTransactionForwarderPolicyV1 {
            max_pending: orderbook_worker_policy.max_pending(),
            max_completed: orderbook_worker_policy.max_completed(),
            max_dead_letters: orderbook_worker_policy.max_dead_letters(),
            max_attempts: orderbook_worker_policy.max_attempts(),
            max_transaction_bytes: ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1,
            checkpoint_max_bytes: orderbook_worker_policy.checkpoint_max_bytes(),
        };
        // In Strict this durability boundary is independent of provider storage and supervised
        // scanning. Fast has no orderbook admission or worker and uses only a process-local
        // placeholder.
        let orderbook_transaction_forwarder = match runtime_persistence {
            RuntimePersistence::Durable => OrderbookTransactionForwarder::open(
                &orderbook_transaction_state_dir,
                orderbook_transaction_policy,
            ),
            RuntimePersistence::ProcessLocal => {
                OrderbookTransactionForwarder::in_memory(orderbook_transaction_policy)
            }
        }
        .map_err(|error| NodeInitError::OrderbookTransactionForwarder {
            path: orderbook_transaction_state_dir.clone(),
            message: error.to_string(),
        })?;
        let reserve_worker_policy = config.reserve_worker_policy();
        reserve_worker_policy
            .validate()
            .map_err(|message| NodeInitError::ReserveWorkerConfig { message })?;
        let reserve_transaction_state_dir = config.data_dir().join("reserve-transaction-forwarder");
        let reserve_transaction_policy = ReserveTransactionForwarderPolicyV1 {
            max_pending: reserve_worker_policy.max_pending(),
            max_completed: reserve_worker_policy.max_completed(),
            max_dead_letters: reserve_worker_policy.max_dead_letters(),
            max_attempts: reserve_worker_policy.max_attempts(),
            max_transaction_bytes: RESERVE_TRANSACTION_MAX_CANONICAL_BYTES_V1,
            checkpoint_max_bytes: reserve_worker_policy.checkpoint_max_bytes(),
        };
        // In Strict this durability boundary is independent of provider storage and supervised
        // scanning. Fast has no reserve admission or worker and uses only a process-local
        // placeholder.
        let reserve_transaction_forwarder = match runtime_persistence {
            RuntimePersistence::Durable => ReserveTransactionForwarder::open(
                &reserve_transaction_state_dir,
                reserve_transaction_policy,
            ),
            RuntimePersistence::ProcessLocal => {
                ReserveTransactionForwarder::in_memory(reserve_transaction_policy)
            }
        }
        .map_err(|error| NodeInitError::ReserveTransactionForwarder {
            path: reserve_transaction_state_dir.clone(),
            message: error.to_string(),
        })?;
        let runtime_checkpoint_initialization = if storage.is_some() {
            Some(inspect_runtime_checkpoint_initialization(
                config.data_dir(),
                config.runtime_retention().checkpoint_max_bytes(),
            )?)
        } else {
            None
        };
        let moderation_model_registry_checkpoint_path = storage
            .as_ref()
            .map(|_| moderation_model_registry_checkpoint_path(config.data_dir()));
        let moderation_screening_checkpoint_path = storage
            .as_ref()
            .map(|_| moderation_screening_checkpoint_path(config.data_dir()));
        let moderation_quarantine_object_root = storage
            .as_ref()
            .map(|_| moderation_quarantine_object_store_root(config.data_dir()));
        let moderation_quarantine_object_index_path = storage
            .as_ref()
            .map(|_| moderation_quarantine_object_index_path(config.data_dir()));
        let moderation_evidence_viewer_checkpoint_path = storage
            .as_ref()
            .map(|_| moderation_evidence_viewer_checkpoint_path(config.data_dir()));
        let auxiliary_runtime_checkpoint_path = storage
            .as_ref()
            .map(|_| auxiliary_runtime_checkpoint_path(config.data_dir()));
        let (reputation_event_sender, _) = broadcast::channel(REPUTATION_EVENT_CHANNEL_CAPACITY);
        let native_repair_singleflight =
            native_repair_singleflight::NativeRepairSingleflightV1::new(
                repair_config.worker_concurrency(),
            );
        let completed_musubi_store_instance = (storage.is_some()
            && provider_ingest_outbox.is_some())
        .then(CompletedMusubiStoreInstanceV1::new);
        let mut node = Self {
            config,
            repair_config,
            gc_config,
            capacity: Arc::new(CapacityManager::with_entry_limit(state_entry_limit)),
            meter: CapacityMeter::with_smoothing(smoothing),
            telemetry: Arc::new(RwLock::new(None)),
            schedulers,
            por: PorTracker::with_entry_limit(state_entry_limit),
            potr,
            proof_outcome_outbox,
            repair_transaction_forwarder,
            native_repair_singleflight,
            orderbook_transaction_forwarder,
            reserve_transaction_forwarder,
            provider_ingest_outbox,
            completed_musubi_store_instance,
            por_finalized_replay_archive,
            por_history: Arc::new(RwLock::new(HashMap::new())),
            storage,
            pdp_provider,
            gc_mutation_lock: Arc::new(Mutex::new(())),
            gc_eviction_intents: Arc::new(RwLock::new(GcEvictionIntentRuntime::default())),
            gc_eviction_audit_links: Arc::new(RwLock::new(BTreeMap::new())),
            repair_orchestrator: Arc::new(RwLock::new(None)),
            governance_publisher: Arc::new(RwLock::new(None)),
            startup_governance_publisher: None,
            governance_publication_lock: governance_publication_lock.clone(),
            governance_runtime_root: None,
            governance_runtime_writer_root_guard: None,
            governance_runtime_read_root_guard: None,
            governance_dag_runtime_signer: governance_dag_runtime_binding.clone(),
            governance_dag_runtime_checkpoint_store: governance_dag_checkpoint_store_binding
                .clone(),
            governance_dag_mirror_reader: Arc::new(OnceLock::new()),
            governance_outbox: Arc::new(RwLock::new(GovernanceOutboxRuntime::default())),
            governance_outbox_drain_lock: Arc::new(Mutex::new(())),
            runtime_mutation_lock: Arc::new(Mutex::new(())),
            auxiliary_checkpoint_lock: Arc::new(Mutex::new(())),
            #[cfg(test)]
            fail_after_next_auxiliary_checkpoint_publication: Arc::new(
                std::sync::atomic::AtomicBool::new(false),
            ),
            durability_failure: Arc::new(Mutex::new(None)),
            auxiliary_runtime_checkpoint_path,
            reputation_trust_policy,
            latest_reputation_snapshot: Arc::new(RwLock::new(None)),
            reputation_snapshots: Arc::new(RwLock::new(BTreeMap::new())),
            reputation_events: Arc::new(RwLock::new(BoundedEventHistory::new(event_history_limit))),
            reputation_event_sender,
            hedging_feed_trust_policy,
            moderation_model_registry_checkpoint_path,
            moderation_model_registry: Arc::new(RwLock::new(
                ModerationModelRegistry::with_entry_limit(state_entry_limit),
            )),
            moderation_screening_checkpoint_path,
            moderation_screening: Arc::new(RwLock::new(
                ModerationScreeningRuntime::with_entry_limit(state_entry_limit),
            )),
            moderation_screening_authority: Arc::new(RwLock::new(moderation_screening_authority)),
            moderation_quarantine_object_root,
            moderation_quarantine_object_index_path,
            moderation_quarantine_key_provider_binding,
            moderation_quarantine_key_wrapper: moderation_quarantine_key_wrapper
                .map(OpaqueModerationQuarantineKeyWrapper),
            moderation_quarantine_objects: Arc::new(RwLock::new(
                ModerationQuarantineObjectRuntime::with_entry_limit(state_entry_limit),
            )),
            moderation_evidence_viewer_checkpoint_path,
            moderation_evidence_viewer: Arc::new(RwLock::new(
                ModerationEvidenceViewerRuntime::with_entry_limit(state_entry_limit),
            )),
            transparency_ledger_source_entries: Arc::new(RwLock::new(BTreeMap::new())),
            privacy_aggregate_source_events: Arc::new(RwLock::new(BTreeMap::new())),
            privacy_source_event_receipts: Arc::new(RwLock::new(BTreeMap::new())),
            privacy_publish_request_receipts: Arc::new(RwLock::new(BTreeMap::new())),
            published_privacy_aggregate_cycles: Arc::new(RwLock::new(BTreeSet::new())),
            privacy_composition_budget: Arc::new(RwLock::new(
                PrivacyCompositionBudgetLedgerV1::default(),
            )),
            privacy_release_ledger: Arc::new(RwLock::new(
                transparency::PrivacyReleaseLedgerV1::default(),
            )),
            privacy_cycle_prf_provider: privacy_cycle_prf_provider
                .map(OpaquePrivacyCyclePrfProvider),
            privacy_release_anchor: privacy_release_anchor.map(OpaquePrivacyReleaseAnchor),
            transparency_leader_lease_provider: transparency_leader_lease_provider
                .map(OpaqueTransparencyLeaderLeaseProvider),
            fenced_transparency_publisher: qualified_fenced_privacy_runtime
                .as_ref()
                .map(|(publisher, _)| publisher.clone()),
            fenced_transparency_head_reader: qualified_fenced_privacy_runtime
                .as_ref()
                .map(|(_, reader)| reader.clone()),
            published_evidence_viewer_audit_cycles: Arc::new(RwLock::new(BTreeSet::new())),
        };
        if node.storage.is_some() {
            match runtime_checkpoint_initialization {
                Some(RuntimeCheckpointInitialization::Fresh) => {
                    node.initialize_runtime_checkpoints()?;
                }
                Some(RuntimeCheckpointInitialization::Initialized) => {
                    node.load_moderation_model_registry_checkpoint()?;
                    node.load_moderation_screening_checkpoint()?;
                    node.load_moderation_quarantine_object_index_checkpoint()?;
                    node.audit_moderation_quarantine_object_store()?;
                    node.load_moderation_evidence_viewer_checkpoint()?;
                    node.load_auxiliary_runtime_checkpoint()?;
                }
                None => unreachable!("storage-backed node must inspect runtime checkpoints"),
            }
            if let Some(policy) = node.config.por_replay_archive_policy() {
                let archive = node
                    .por_finalized_replay_archive
                    .as_ref()
                    .expect("configured replay archive was qualified before state opened");
                verify_por_replay_archive_provider(policy, archive.0.as_ref()).map_err(|_| {
                    NodeInitError::PorReplayArchive {
                        message:
                            "restored replay-archive provider is unavailable or no longer exact"
                                .to_owned(),
                    }
                })?;
                let previous_por = node.por.checkpoint();
                let reconciled = node
                    .por
                    .reconcile_restored_replay_archive_head(
                        archive.0.as_ref(),
                        policy.binding(),
                        policy.proof_bounds(),
                    )
                    .map_err(|_| NodeInitError::PorReplayArchive {
                        message:
                            "restored replay-archive head is stale, forked, or unauthenticated"
                                .to_owned(),
                    })?;
                if verify_por_replay_archive_provider(policy, archive.0.as_ref()).is_err() {
                    node.por.restore_checkpoint(previous_por).map_err(|_| {
                        NodeInitError::PorReplayArchive {
                            message: "failed to restore local PoR state after replay-archive provider drift"
                                .to_owned(),
                        }
                    })?;
                    return Err(NodeInitError::PorReplayArchive {
                        message:
                            "restored replay-archive provider drifted during startup reconciliation"
                                .to_owned(),
                    });
                }
                if reconciled {
                    let path = node
                        .auxiliary_runtime_checkpoint_path
                        .as_ref()
                        .expect("storage-backed node has an auxiliary checkpoint path");
                    node.persist_auxiliary_runtime_checkpoint_unlocked()
                        .map_err(|error| {
                            NodeInitError::checkpoint("reconciled auxiliary runtime", path, error)
                        })?;
                }
            }
            node.reconcile_gc_eviction_intents_on_startup()
                .map_err(|err| {
                    NodeInitError::checkpoint(
                        "auxiliary runtime",
                        node.auxiliary_runtime_checkpoint_path
                            .as_ref()
                            .expect("storage-backed node has an auxiliary checkpoint path"),
                        err,
                    )
                })?;
            node.reconcile_privacy_release_anchor()
                .map_err(|error| NodeInitError::Checkpoint {
                    component: "privacy release anchor",
                    path: crate::auxiliary_runtime_checkpoint_path(node.config.data_dir()),
                    message: error.to_string(),
                })?;
            // PDP and PoTR handoffs intentionally remain durable here. Repair-required
            // records need Torii's chain-authoritative transaction adapter; NodeHandle
            // must not fabricate a local receipt or make restart depend on that adapter.
            // The supervised Torii repair forwarder resumes each protocol on its first
            // immediate scan and once per subsequent scan.
            if let Some(dir) = governance_dir.clone() {
                let publication_lock = governance_publication_lock
                    .as_ref()
                    .expect("configured Governance DAG has a publication lock");
                let mut publisher = FilesystemGovernancePublisher::try_new_with_publication_lock(
                    dir.clone(),
                    Arc::clone(publication_lock),
                )
                .map_err(|err| NodeInitError::GovernancePublisher(err.to_string()))?;
                let signer = governance_dag_runtime_binding
                    .expect("configured Governance DAG signer was qualified before state opened");
                let checkpoint_store = governance_dag_checkpoint_store_binding.expect(
                    "configured Governance DAG checkpoint store was qualified before state opened",
                );
                signer
                    .assert_qualification()
                    .map_err(|err| NodeInitError::GovernancePublisher(err.to_string()))?;
                checkpoint_store
                    .assert_qualification()
                    .map_err(|err| NodeInitError::GovernancePublisher(err.to_string()))?;
                publisher = publisher
                    .with_qualified_runtime_dag_providers(signer, checkpoint_store)
                    .map_err(|err| NodeInitError::GovernancePublisher(err.to_string()))?;
                let writer_root_guard = publisher.root_guard().clone();
                let read_root_guard = GovernanceFilesystemRootGuard::capture_source(
                    publisher.root(),
                )
                .map_err(|error| {
                    NodeInitError::GovernancePublisher(format!(
                        "failed to retain read-only Governance DAG root: {error}"
                    ))
                })?;
                let writer_root_identity =
                    writer_root_guard.identity_digest().map_err(|error| {
                        NodeInitError::GovernancePublisher(format!(
                            "failed to bind writable Governance DAG root identity: {error}"
                        ))
                    })?;
                let read_root_identity = read_root_guard.identity_digest().map_err(|error| {
                    NodeInitError::GovernancePublisher(format!(
                        "failed to bind read-only Governance DAG root identity: {error}"
                    ))
                })?;
                if writer_root_guard.root() != read_root_guard.root()
                    || writer_root_identity != read_root_identity
                {
                    return Err(NodeInitError::GovernancePublisher(
                        "writable and read-only Governance DAG roots do not retain the same physical directory"
                            .to_owned(),
                    ));
                }
                node.governance_runtime_root = Some(publisher.root().to_path_buf());
                node.governance_runtime_writer_root_guard = Some(writer_root_guard);
                node.governance_runtime_read_root_guard = Some(read_root_guard);
                if let Some((fenced_publisher, fenced_head_reader)) =
                    qualified_fenced_privacy_runtime
                {
                    publisher = publisher
                        .with_qualified_fenced_privacy_publisher(fenced_publisher)
                        .map_err(|err| NodeInitError::GovernancePublisher(err.to_string()))?
                        .with_qualified_fenced_privacy_head_reader(fenced_head_reader)
                        .map_err(|err| NodeInitError::GovernancePublisher(err.to_string()))?;
                }
                iroha_logger::info!(
                    path = ?dir,
                    signed_runtime_dag = governance_dir.is_some(),
                    fused_privacy_target = privacy_publication_required,
                    "SoraFS governance publisher initialised"
                );
                node.install_startup_governance_publisher(Arc::new(publisher))
                    .map_err(|err| NodeInitError::GovernancePublisher(err.to_string()))?;
            }
        } else {
            if governance_dir.is_some() {
                iroha_logger::warn!(
                    "skipping governance publisher initialisation: storage backend disabled"
                );
            }
            node.reconcile_privacy_release_anchor()
                .map_err(|error| NodeInitError::Checkpoint {
                    component: "privacy release anchor",
                    path: crate::auxiliary_runtime_checkpoint_path(node.config.data_dir()),
                    message: error.to_string(),
                })?;
        }
        Ok(node)
    }
    /// Returns a reference to the storage configuration.
    #[must_use]
    pub fn config(&self) -> &StorageConfig {
        &self.config
    }
    /// Return the canonical Governance DAG root retained by the running node.
    #[must_use]
    pub fn governance_dag_root(&self) -> Option<&Path> {
        self.governance_runtime_root.as_deref()
    }
    /// Read one canonical typed publication-authority generation.
    ///
    /// This boundary never initializes, repairs, or reconciles mutable state.
    /// An entirely pristine root is returned as `None`; an initialized empty
    /// authority remains an authenticated `Some` snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when Governance DAG storage is not configured or the
    /// retained root or typed two-slot authority fails validation.
    pub fn governance_dag_publication_snapshot(
        &self,
    ) -> Result<Option<GovernanceDagPublicationSnapshotV1>, GovernancePublishError> {
        let root_guard = self
            .governance_runtime_read_root_guard
            .as_ref()
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "Governance DAG runtime root has no retained read-only filesystem identity",
                )
            })?;
        governance::load_governance_publication_snapshot_v1(root_guard).map(|snapshot| {
            snapshot.map(|snapshot| {
                let (canonical_bytes, store_generation, store_record_digest) =
                    snapshot.into_parts();
                GovernanceDagPublicationSnapshotV1 {
                    canonical_bytes,
                    store_generation,
                    store_record_digest,
                }
            })
        })
    }
    /// Read one typed runtime-DAG generation authenticated by the exact sealed
    /// producer checkpoint retained by this node.
    ///
    /// This boundary is read-only: it brackets the typed head/index read with sealed checkpoint and
    /// intent checks, but never performs recovery or producer reconciliation. An authenticated
    /// genesis checkpoint returns `None`.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured root or runtime providers are absent, substituted,
    /// unqualified, or disagree with the typed committed generation and sealed producer checkpoint.
    pub fn governance_dag_runtime_snapshot(
        &self,
    ) -> Result<Option<GovernanceDagRuntimeSnapshotV1>, GovernancePublishError> {
        let root_guard = self
            .governance_runtime_read_root_guard
            .as_ref()
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "Governance DAG runtime root has no retained read-only filesystem identity",
                )
            })?;
        let signer = self.governance_dag_runtime_signer.as_ref().ok_or_else(|| {
            GovernancePublishError::other(
                "Governance DAG runtime signer was not retained by this node",
            )
        })?;
        let checkpoint_store = self
            .governance_dag_runtime_checkpoint_store
            .as_ref()
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "Governance DAG checkpoint store was not retained by this node",
                )
            })?;
        governance::load_authenticated_runtime_dag_snapshot_v1(root_guard, signer, checkpoint_store)
            .map(|snapshot| {
                snapshot.map(|snapshot| {
                    let (
                        head_bytes,
                        index_bytes,
                        store_generation,
                        store_record_digest,
                        checkpoint_generation,
                        checkpoint_revision,
                    ) = snapshot.into_parts();
                    GovernanceDagRuntimeSnapshotV1 {
                        head_bytes,
                        index_bytes,
                        store_generation,
                        store_record_digest,
                        checkpoint_generation,
                        checkpoint_revision,
                    }
                })
            })
    }
    /// Install the service-owned authenticated mirror-read capability exactly once across this node
    /// handle and every clone sharing its installation slot.
    ///
    /// The path-free capability binding must match this node's logical and
    /// physical producer root plus every retained signer and checkpoint-store
    /// identity. A failed validation does not consume the installation slot.
    ///
    /// # Errors
    ///
    /// Returns an error for a second installation, incomplete retained runtime
    /// bindings, provider substitution, or any mismatched capability binding.
    pub fn install_governance_dag_mirror_read_handle(
        &mut self,
        reader: GovernanceDagMirrorReadHandleV1,
    ) -> Result<(), GovernancePublishError> {
        if self.governance_dag_mirror_reader.get().is_some() {
            return Err(GovernancePublishError::other(
                "Governance DAG mirror read capability is already installed",
            ));
        }
        let root = self.governance_runtime_root.as_deref().ok_or_else(|| {
            GovernancePublishError::other("Governance DAG runtime root is not configured")
        })?;
        let root_guard = self
            .governance_runtime_read_root_guard
            .as_ref()
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "Governance DAG runtime root has no retained read-only filesystem identity",
                )
            })?;
        if root != root_guard.root() {
            return Err(GovernancePublishError::other(
                "Governance DAG logical and physical retained roots disagree",
            ));
        }
        let signer = self.governance_dag_runtime_signer.as_ref().ok_or_else(|| {
            GovernancePublishError::other(
                "Governance DAG runtime signer was not retained by this node",
            )
        })?;
        let checkpoint_store = self
            .governance_dag_runtime_checkpoint_store
            .as_ref()
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "Governance DAG checkpoint store was not retained by this node",
                )
            })?;
        root_guard.revalidate()?;
        signer.assert_qualification()?;
        checkpoint_store.assert_qualification()?;
        let expected_root_digest = governance::runtime_dag_producer_root_digest(root)?;
        let expected_root_identity_digest = root_guard.identity_digest()?;
        let expected_signer_handle =
            self.config.governance_dag_signer_handle().ok_or_else(|| {
                GovernancePublishError::other("Governance DAG signer handle is not configured")
            })?;
        let expected_signer_qualification = self
            .config
            .governance_dag_signer_qualification()
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "Governance DAG signer qualification is not configured",
                )
            })?;
        let expected_peer_id = self
            .config
            .governance_dag_publisher_peer_id()
            .ok_or_else(|| {
                GovernancePublishError::other("Governance DAG publisher peer id is not configured")
            })?;
        let expected_public_key_hex = self
            .config
            .governance_dag_publisher_public_key_hex()
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "Governance DAG publisher public key is not configured",
                )
            })?;
        if expected_public_key_hex.len() != 64
            || !expected_public_key_hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(GovernancePublishError::other(
                "configured Governance DAG publisher public key is not canonical lowercase hex",
            ));
        }
        let expected_public_key: [u8; 32] = hex::decode(expected_public_key_hex)
            .map_err(|_| {
                GovernancePublishError::other(
                    "configured Governance DAG publisher public key is not canonical hex",
                )
            })?
            .try_into()
            .map_err(|_| {
                GovernancePublishError::other(
                    "configured Governance DAG publisher public key is not 32 bytes",
                )
            })?;
        let expected_checkpoint_handle = self
            .config
            .governance_dag_checkpoint_store_handle()
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "Governance DAG checkpoint-store handle is not configured",
                )
            })?;
        let expected_checkpoint_qualification = self
            .config
            .governance_dag_checkpoint_store_qualification()
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "Governance DAG checkpoint-store qualification is not configured",
                )
            })?;
        let (
            retained_signer_handle,
            retained_signer_qualification,
            retained_peer_id,
            retained_public_key,
        ) = signer.binding();
        let binding = reader.binding();
        if binding.source_root_digest() != expected_root_digest
            || binding.source_root_identity_digest() != expected_root_identity_digest
            || binding.producer_signer_handle() != expected_signer_handle.as_str()
            || binding.producer_signer_qualification() != expected_signer_qualification
            || binding.producer_publisher_peer_id() != expected_peer_id.as_bytes()
            || binding.producer_public_key() != expected_public_key
            || binding.producer_signer_handle() != retained_signer_handle
            || binding.producer_signer_qualification() != retained_signer_qualification
            || binding.producer_publisher_peer_id() != retained_peer_id
            || binding.producer_public_key() != retained_public_key
            || binding.checkpoint_store_handle() != expected_checkpoint_handle.as_str()
            || binding.checkpoint_store_qualification() != expected_checkpoint_qualification
            || binding.checkpoint_store_handle() != checkpoint_store.handle()
            || binding.checkpoint_store_qualification() != checkpoint_store.qualification()
        {
            return Err(GovernancePublishError::other(
                "Governance DAG mirror read capability does not match this node's retained producer binding",
            ));
        }
        root_guard.revalidate()?;
        signer.assert_qualification()?;
        checkpoint_store.assert_qualification()?;
        reader.assert_install_ready().map_err(|error| {
            GovernancePublishError::other(format!(
                "Governance DAG mirror read capability failed installation revalidation: {error}"
            ))
        })?;
        self.governance_dag_mirror_reader.set(reader).map_err(|_| {
            GovernancePublishError::other(
                "Governance DAG mirror read capability is already installed",
            )
        })
    }
    /// Read the installed service-owned mirror snapshot.
    ///
    /// # Errors
    ///
    /// Returns `Ok(None)` when no mirror capability was installed or when the installed capability
    /// authenticates the empty pre-checkpoint bootstrap state. Returns an error when its retained
    /// roots, typed store, sealed intent/checkpoint, or provider binding changed.
    pub fn governance_dag_mirror_snapshot(
        &self,
    ) -> Result<Option<GovernanceDagMirrorSnapshotV1>, GovernanceDagServiceError> {
        let Some(reader) = self.governance_dag_mirror_reader.get() else {
            return Ok(None);
        };
        reader.read()
    }
    /// Read one Governance DAG file through the node's retained filesystem root.
    ///
    /// Every path component is resolved relative to the descriptor-pinned Governance root without
    /// following links or reparse points. The opened regular file, its parent bindings, and the
    /// root identity are revalidated after the bounded read, so callers never authenticate one path
    /// and read a concurrently substituted object.
    ///
    /// # Errors
    ///
    /// Returns an error when Governance DAG storage is not configured, the path is absolute or
    /// non-canonical, any retained filesystem identity or access policy changed, the target is not
    /// a direct single-link regular file, or its contents exceed `max_bytes`.
    pub fn read_governance_dag_file(
        &self,
        relative_path: &Path,
        max_bytes: usize,
    ) -> io::Result<Vec<u8>> {
        if relative_path.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Governance DAG file path must be relative",
            ));
        }
        let root = self.governance_dag_root().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Governance DAG runtime root is not configured",
            )
        })?;
        let root_guard = self
            .governance_runtime_read_root_guard
            .as_ref()
            .ok_or_else(|| {
                io::Error::other(
                    "Governance DAG runtime root has no retained read-only filesystem identity",
                )
            })?;
        governance::read_rooted_governance_state_file(
            root_guard,
            &root.join(relative_path),
            max_bytes,
        )
        .map(governance_rooted_fs::FileSnapshot::into_bytes)
    }
    /// Return the durable PDP provider protocol when storage is enabled.
    #[must_use]
    pub fn pdp_provider_protocol(&self) -> Option<&PdpProviderProtocol> {
        self.pdp_provider.as_ref()
    }
    /// Resume durable PDP terminal handoffs through an explicit production adapter.
    pub fn resume_pdp_terminal_handoffs(
        &self,
        handoff: &dyn PdpTerminalHandoff,
        limit: usize,
    ) -> Result<usize, PdpProviderProtocolError> {
        let Some(pdp_provider) = self.pdp_provider.as_ref() else {
            return Ok(0);
        };
        pdp_provider
            .resume_handoffs(handoff, limit)
            .map(|outcomes| outcomes.len())
    }
    /// Resume durable PoTR proof-ledger and repair handoffs through an explicit adapter.
    pub fn resume_potr_terminal_handoffs(
        &self,
        handoff: &dyn potr::PotrLatencyRepairHandoff,
    ) -> Result<usize, PotrTrackerError> {
        self.potr.resume_terminal_handoffs(handoff)
    }
    /// Return pending proof-outcome deliveries in stable sequence order.
    pub fn pending_proof_outcome_deliveries(
        &self,
        limit: usize,
    ) -> Result<Vec<ProofOutcomePendingDeliveryV1>, ProofOutcomeOutboxError> {
        self.proof_outcome_outbox.pending(limit)
    }
    /// Return a circular page of proof-outcome deliveries after a sequence cursor.
    pub fn pending_proof_outcome_deliveries_after(
        &self,
        after_sequence: Option<u64>,
        limit: usize,
    ) -> Result<Vec<ProofOutcomePendingDeliveryV1>, ProofOutcomeOutboxError> {
        self.proof_outcome_outbox
            .pending_after(after_sequence, limit)
    }
    /// Return payload-free terminal proof-outcome deliveries for operator reconciliation.
    pub fn proof_outcome_dead_letters(
        &self,
        limit: usize,
    ) -> Result<Vec<ProofOutcomeDeadLetterV1>, ProofOutcomeOutboxError> {
        self.proof_outcome_outbox.dead_letters(limit)
    }
    /// Restore one explicitly selected proof-outcome dead letter for governed replay.
    pub fn retry_proof_outcome_dead_letter(
        &self,
        operation_id: [u8; 32],
        expected_outcome_digest: [u8; 32],
    ) -> Result<(), ProofOutcomeOutboxError> {
        self.proof_outcome_outbox
            .retry_dead_letter(operation_id, expected_outcome_digest)
    }
    /// Durably claim one proof outcome for isolated runtime signing.
    pub fn claim_proof_outcome_for_signing(
        &self,
        operation_id: [u8; 32],
        baseline_finalized_cursor: iroha_data_model::sorafs::proof_ledger::ProofOutcomeFinalizedCursorV1,
    ) -> Result<ProofOutcomePendingDeliveryV1, ProofOutcomeOutboxError> {
        self.proof_outcome_outbox
            .claim_for_signing(operation_id, baseline_finalized_cursor)
    }
    /// Persist an exact signed proof-outcome transaction before queue exposure.
    pub fn store_signed_proof_outcome_transaction(
        &self,
        operation_id: [u8; 32],
        transaction: iroha_data_model::transaction::SignedTransaction,
    ) -> Result<[u8; 32], ProofOutcomeOutboxError> {
        self.proof_outcome_outbox
            .store_signed_transaction(operation_id, transaction)
    }
    /// Release an isolated proof-outcome signing claim after signer failure.
    pub fn release_proof_outcome_signing_claim(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), ProofOutcomeOutboxError> {
        self.proof_outcome_outbox
            .release_signing_claim(operation_id)
    }
    /// Mark a durable signed proof-outcome transaction ambiguous before submission.
    pub fn begin_proof_outcome_submission(
        &self,
        operation_id: [u8; 32],
    ) -> Result<iroha_data_model::transaction::SignedTransaction, ProofOutcomeOutboxError> {
        self.proof_outcome_outbox.begin_submission(operation_id)
    }
    /// Record that an exact proof-outcome transaction is pending or applied.
    pub fn mark_proof_outcome_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), ProofOutcomeOutboxError> {
        self.proof_outcome_outbox.mark_submitted(operation_id)
    }
    /// Record a proof-outcome queue failure known to precede submission.
    pub fn mark_proof_outcome_not_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), ProofOutcomeOutboxError> {
        self.proof_outcome_outbox.mark_not_submitted(operation_id)
    }
    /// Permit retry after finalized absence of the exact proof outcome and transaction.
    pub fn mark_proof_outcome_finalized_absent(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: iroha_data_model::sorafs::proof_ledger::ProofOutcomeFinalizedCursorV1,
    ) -> Result<(), ProofOutcomeOutboxError> {
        self.proof_outcome_outbox
            .mark_finalized_absent(operation_id, observed_finalized_cursor)
    }
    /// Reconcile one exact finalized proof outcome.
    pub fn mark_proof_outcome_finalized(
        &self,
        operation_id: [u8; 32],
        finalized: &iroha_data_model::sorafs::proof_ledger::ProofOutcomeFinalizedRecordV1,
    ) -> Result<(), ProofOutcomeOutboxError> {
        self.proof_outcome_outbox
            .mark_finalized(operation_id, finalized)
    }
    /// Dead-letter one exact rejected or expired proof-outcome transaction.
    pub fn mark_proof_outcome_transaction_rejected(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: iroha_data_model::sorafs::proof_ledger::ProofOutcomeFinalizedCursorV1,
    ) -> Result<(), ProofOutcomeOutboxError> {
        self.proof_outcome_outbox
            .mark_transaction_rejected(operation_id, observed_finalized_cursor)
    }
    /// Durably enqueue one authority-bound native repair operation.
    pub fn enqueue_repair_transaction(
        &self,
        authority: AccountId,
        operation: RepairOperationV1,
        context: &RepairTransactionContextV1,
    ) -> Result<RepairTransactionEnqueueResultV1, RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .enqueue_unsigned_operation(authority, operation, context)
    }
    /// Return a fair circular page of pending native repair transactions.
    pub fn pending_repair_transactions_after(
        &self,
        after_sequence: Option<u64>,
        limit: usize,
    ) -> Result<Vec<RepairTransactionPendingV1>, RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .pending_after(after_sequence, limit)
    }
    /// Return payload-free terminal repair-transaction dead letters.
    pub fn repair_transaction_dead_letters(
        &self,
        limit: usize,
    ) -> Result<Vec<RepairTransactionDeadLetterV1>, RepairTransactionForwarderError> {
        self.repair_transaction_forwarder.dead_letters(limit)
    }
    /// Claim one native repair operation for isolated runtime signing.
    pub fn claim_repair_transaction_for_signing(
        &self,
        operation_id: [u8; 32],
    ) -> Result<RepairTransactionSigningRequestV1, RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .claim_for_signing(operation_id)
    }
    /// Read exact repair semantics for finalized reconciliation without claiming a signer attempt.
    pub fn repair_transaction_operation_for_reconciliation(
        &self,
        operation_id: [u8; 32],
    ) -> Result<RepairTransactionSigningRequestV1, RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .operation_for_reconciliation(operation_id)
    }
    /// Persist exact signed native repair-transaction bytes before queue exposure.
    pub fn store_signed_repair_transaction(
        &self,
        operation_id: [u8; 32],
        signed_transaction_bytes: &[u8],
    ) -> Result<[u8; 32], RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .store_signed_transaction(operation_id, signed_transaction_bytes)
    }
    /// Release an interrupted repair-transaction signing claim.
    pub fn release_repair_transaction_signing_claim(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .release_signing_claim(operation_id)
    }
    /// Mark exact signed repair bytes ambiguous before transaction ingress.
    pub fn begin_repair_transaction_submission(
        &self,
        operation_id: [u8; 32],
    ) -> Result<Vec<u8>, RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .begin_submission(operation_id)
    }
    /// Record that an exact repair transaction is pending or applied.
    pub fn mark_repair_transaction_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .mark_submitted(operation_id)
    }
    /// Reconcile exact finalized repair-transaction bytes.
    pub fn mark_repair_transaction_finalized(
        &self,
        operation_id: [u8; 32],
        expected_transaction_digest: [u8; 32],
        finalized_cursor: RepairFinalizedCursorV1,
    ) -> Result<(), RepairTransactionForwarderError> {
        self.repair_transaction_forwarder.mark_finalized(
            operation_id,
            expected_transaction_digest,
            finalized_cursor,
        )
    }
    /// Record a repair queue failure proven to precede submission.
    pub fn mark_repair_transaction_not_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .mark_not_submitted(operation_id)
    }
    /// Permit retry after finalized absence of the exact repair operation.
    pub fn mark_repair_transaction_finalized_absent(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: RepairFinalizedCursorV1,
    ) -> Result<(), RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .mark_finalized_absent(operation_id, observed_finalized_cursor)
    }
    /// Reconcile an exact semantic repair operation committed by any ingress.
    pub fn mark_repair_transaction_semantic_finalized(
        &self,
        operation_id: [u8; 32],
        finalized_cursor: RepairFinalizedCursorV1,
    ) -> Result<(), RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .mark_semantic_finalized(operation_id, finalized_cursor)
    }
    /// Dead-letter a repair operation that conflicts with finalized state.
    pub fn mark_repair_transaction_finalized_conflict(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: RepairFinalizedCursorV1,
    ) -> Result<(), RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .mark_finalized_conflict(operation_id, observed_finalized_cursor)
    }
    /// Clear a rejected repair envelope for bounded replacement signing.
    pub fn mark_repair_transaction_rejected(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: RepairFinalizedCursorV1,
    ) -> Result<(), RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .mark_transaction_rejected(operation_id, observed_finalized_cursor)
    }
    /// Durably enqueue one native orderbook operation bound to finalized governance state.
    pub fn enqueue_orderbook_transaction(
        &self,
        operation: OrderbookOperationV1,
        context: &OrderbookTransactionContextV1,
    ) -> Result<OrderbookTransactionEnqueueResultV1, OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .enqueue_unsigned_operation(operation, context)
    }
    /// Durably enqueue one canonical signed native orderbook transaction.
    ///
    /// The forwarder binds both the exact governed authority and active chain
    /// identity from the finalized context before persisting any bytes.
    pub fn enqueue_signed_orderbook_transaction(
        &self,
        signed_transaction_bytes: &[u8],
        context: &OrderbookTransactionContextV1,
    ) -> Result<OrderbookTransactionEnqueueResultV1, OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .enqueue_signed_transaction(signed_transaction_bytes, context)
    }
    /// Return the oldest bounded page of pending native orderbook transactions.
    pub fn pending_orderbook_transactions(
        &self,
        limit: usize,
    ) -> Result<Vec<OrderbookTransactionPendingV1>, OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder.pending(limit)
    }
    /// Return a fair circular page of pending native orderbook transactions.
    pub fn pending_orderbook_transactions_after(
        &self,
        after_sequence: Option<u64>,
        limit: usize,
    ) -> Result<Vec<OrderbookTransactionPendingV1>, OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .pending_after(after_sequence, limit)
    }
    /// Read exact orderbook semantics for finalized reconciliation without claiming an attempt.
    pub fn orderbook_transaction_operation_for_reconciliation(
        &self,
        operation_id: [u8; 32],
    ) -> Result<OrderbookTransactionSigningRequestV1, OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .operation_for_reconciliation(operation_id)
    }
    /// Return payload-free terminal orderbook-transaction dead letters.
    pub fn orderbook_transaction_dead_letters(
        &self,
        limit: usize,
    ) -> Result<Vec<OrderbookTransactionDeadLetterV1>, OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder.dead_letters(limit)
    }
    /// Claim one native orderbook operation for isolated runtime signing.
    pub fn claim_orderbook_transaction_for_signing(
        &self,
        operation_id: [u8; 32],
    ) -> Result<OrderbookTransactionSigningRequestV1, OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .claim_for_signing(operation_id)
    }
    /// Persist exact signed orderbook transaction bytes before submitter exposure.
    pub fn store_signed_orderbook_transaction(
        &self,
        operation_id: [u8; 32],
        signed_transaction_bytes: &[u8],
    ) -> Result<[u8; 32], OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .store_signed_transaction(operation_id, signed_transaction_bytes)
    }
    /// Release an interrupted orderbook-transaction signing claim.
    pub fn release_orderbook_transaction_signing_claim(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .release_signing_claim(operation_id)
    }
    /// Mark exact signed orderbook bytes ambiguous before transaction ingress.
    pub fn begin_orderbook_transaction_submission(
        &self,
        operation_id: [u8; 32],
    ) -> Result<Vec<u8>, OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .begin_submission(operation_id)
    }
    /// Record that an exact orderbook transaction is pending or applied.
    pub fn mark_orderbook_transaction_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .mark_submitted(operation_id)
    }
    /// Record an orderbook queue failure proven to precede submission.
    pub fn mark_orderbook_transaction_not_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .mark_not_submitted(operation_id)
    }
    /// Permit exact-byte retry after finalized absence of an orderbook transaction.
    pub fn mark_orderbook_transaction_finalized_absent(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: OrderbookFinalizedCursorV1,
    ) -> Result<(), OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .mark_finalized_absent(operation_id, observed_finalized_cursor)
    }
    /// Reconcile exact finalized orderbook success by retained transaction digest.
    pub fn mark_orderbook_transaction_finalized(
        &self,
        operation_id: [u8; 32],
        expected_transaction_digest: [u8; 32],
        finalized_cursor: OrderbookFinalizedCursorV1,
    ) -> Result<(), OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder.mark_finalized(
            operation_id,
            expected_transaction_digest,
            finalized_cursor,
        )
    }
    /// Reconcile exact semantic orderbook success committed through another ingress.
    pub fn mark_orderbook_transaction_semantic_finalized(
        &self,
        operation_id: [u8; 32],
        finalized_cursor: OrderbookFinalizedCursorV1,
    ) -> Result<(), OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .mark_semantic_finalized(operation_id, finalized_cursor)
    }
    /// Dead-letter an orderbook operation that conflicts with finalized state.
    pub fn mark_orderbook_transaction_finalized_conflict(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: OrderbookFinalizedCursorV1,
    ) -> Result<(), OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .mark_finalized_conflict(operation_id, observed_finalized_cursor)
    }
    /// Clear a rejected orderbook envelope for bounded replacement signing.
    pub fn mark_orderbook_transaction_rejected(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: OrderbookFinalizedCursorV1,
    ) -> Result<(), OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .mark_transaction_rejected(operation_id, observed_finalized_cursor)
    }
    /// Durably enqueue one native reserve/rent operation bound to finalized governance state.
    pub fn enqueue_reserve_transaction(
        &self,
        operation: ReserveOperationV1,
        context: &ReserveTransactionContextV1,
    ) -> Result<ReserveTransactionEnqueueResultV1, ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .enqueue_unsigned_operation(operation, context)
    }
    /// Durably enqueue one canonical signed native reserve/rent transaction.
    ///
    /// The forwarder binds both the exact governed authority and active chain
    /// identity from the finalized context before persisting any bytes.
    pub fn enqueue_signed_reserve_transaction(
        &self,
        signed_transaction_bytes: &[u8],
        context: &ReserveTransactionContextV1,
    ) -> Result<ReserveTransactionEnqueueResultV1, ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .enqueue_signed_transaction(signed_transaction_bytes, context)
    }
    /// Return the oldest bounded page of pending native reserve/rent transactions.
    pub fn pending_reserve_transactions(
        &self,
        limit: usize,
    ) -> Result<Vec<ReserveTransactionPendingV1>, ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder.pending(limit)
    }
    /// Return a fair circular page of pending native reserve/rent transactions.
    pub fn pending_reserve_transactions_after(
        &self,
        after_sequence: Option<u64>,
        limit: usize,
    ) -> Result<Vec<ReserveTransactionPendingV1>, ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .pending_after(after_sequence, limit)
    }
    /// Read exact reserve/rent semantics for finalized reconciliation without claiming an attempt.
    pub fn reserve_transaction_operation_for_reconciliation(
        &self,
        operation_id: [u8; 32],
    ) -> Result<ReserveTransactionReconciliationV1, ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .operation_for_reconciliation(operation_id)
    }
    /// Return payload-free terminal reserve-transaction dead letters.
    pub fn reserve_transaction_dead_letters(
        &self,
        limit: usize,
    ) -> Result<Vec<ReserveTransactionDeadLetterV1>, ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder.dead_letters(limit)
    }
    /// Claim one native reserve/rent operation for isolated runtime signing.
    pub fn claim_reserve_transaction_for_signing(
        &self,
        operation_id: [u8; 32],
    ) -> Result<ReserveTransactionSigningRequestV1, ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .claim_for_signing(operation_id)
    }
    /// Persist exact signed reserve/rent transaction bytes before submitter exposure.
    pub fn store_signed_reserve_transaction(
        &self,
        operation_id: [u8; 32],
        signed_transaction_bytes: &[u8],
    ) -> Result<[u8; 32], ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .store_signed_transaction(operation_id, signed_transaction_bytes)
    }
    /// Release an interrupted reserve-transaction signing claim.
    pub fn release_reserve_transaction_signing_claim(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .release_signing_claim(operation_id)
    }
    /// Mark exact signed reserve/rent bytes ambiguous before transaction ingress.
    pub fn begin_reserve_transaction_submission(
        &self,
        operation_id: [u8; 32],
    ) -> Result<Vec<u8>, ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .begin_submission(operation_id)
    }
    /// Record that an exact reserve/rent transaction is pending or applied.
    pub fn mark_reserve_transaction_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .mark_submitted(operation_id)
    }
    /// Record a reserve/rent queue failure proven to precede submission.
    pub fn mark_reserve_transaction_not_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .mark_not_submitted(operation_id)
    }
    /// Permit exact-byte retry after finalized absence of a reserve/rent transaction.
    pub fn mark_reserve_transaction_finalized_absent(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: ReserveFinalizedCursorV1,
    ) -> Result<(), ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .mark_finalized_absent(operation_id, observed_finalized_cursor)
    }
    /// Reconcile exact finalized reserve/rent success by retained transaction digest.
    pub fn mark_reserve_transaction_finalized(
        &self,
        operation_id: [u8; 32],
        expected_transaction_digest: [u8; 32],
        finalized_cursor: ReserveFinalizedCursorV1,
    ) -> Result<(), ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder.mark_finalized(
            operation_id,
            expected_transaction_digest,
            finalized_cursor,
        )
    }
    /// Reconcile exact reserve/rent semantics committed through another ingress.
    pub fn mark_reserve_transaction_semantic_finalized(
        &self,
        operation_id: [u8; 32],
        finalized_cursor: ReserveFinalizedCursorV1,
    ) -> Result<(), ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .mark_semantic_finalized(operation_id, finalized_cursor)
    }
    /// Dead-letter a reserve/rent operation that conflicts with finalized state.
    pub fn mark_reserve_transaction_finalized_conflict(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: ReserveFinalizedCursorV1,
    ) -> Result<(), ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .mark_finalized_conflict(operation_id, observed_finalized_cursor)
    }
    /// Clear a rejected reserve/rent envelope for bounded replacement signing.
    pub fn mark_reserve_transaction_rejected(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: ReserveFinalizedCursorV1,
    ) -> Result<(), ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .mark_transaction_rejected(operation_id, observed_finalized_cursor)
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
    /// Return the first durability failure that forced this handle into fail-closed mode.
    ///
    /// Once set, durable mutation APIs reject new work because in-memory and
    /// checkpoint state could no longer be proven equivalent.
    #[must_use]
    pub fn durability_failure_reason(&self) -> Option<String> {
        match self.durability_failure.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned
                .into_inner()
                .clone()
                .or_else(|| Some("durability health lock poisoned".to_owned())),
        }
    }
    fn ensure_durability_healthy(&self) -> Result<(), String> {
        match self.durability_failure_reason() {
            Some(reason) => Err(format!(
                "durable mutations are disabled after an unrecoverable checkpoint failure: {reason}"
            )),
            None => Ok(()),
        }
    }
    fn mark_durability_unhealthy(&self, reason: String) {
        match self.durability_failure.lock() {
            Ok(mut guard) => {
                if guard.is_none() {
                    *guard = Some(reason);
                }
            }
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                if guard.is_none() {
                    *guard = Some(reason);
                }
            }
        }
    }
    fn record_unrecoverable_rollback(
        &self,
        context: &str,
        rollback_error: impl std::fmt::Display,
    ) -> String {
        let reason = format!("{context}: {rollback_error}");
        self.mark_durability_unhealthy(reason.clone());
        reason
    }
    fn finish_local_checkpoint_write(
        &self,
        component: &'static str,
        path: &Path,
        result: Result<(), LocalCheckpointWriteError>,
    ) -> Result<(), RuntimeCheckpointPersistError> {
        match result {
            Ok(()) => Ok(()),
            Err(err) => {
                let committed = err.committed;
                let message = format!("persist {component} checkpoint `{}`: {err}", path.display());
                if committed {
                    self.mark_durability_unhealthy(message.clone());
                }
                Err(RuntimeCheckpointPersistError { message, committed })
            }
        }
    }
    fn initialize_runtime_checkpoints(&self) -> Result<(), NodeInitError> {
        let model_path = self
            .moderation_model_registry_checkpoint_path
            .as_ref()
            .expect("storage-backed node has a model registry checkpoint path");
        self.persist_moderation_model_registry_snapshot(&ModerationModelRegistrySnapshot::default())
            .map_err(|err| {
                NodeInitError::checkpoint("moderation model registry", model_path, err)
            })?;
        let screening_path = self
            .moderation_screening_checkpoint_path
            .as_ref()
            .expect("storage-backed node has a screening checkpoint path");
        self.persist_moderation_screening_snapshot(&ModerationScreeningSnapshot::default())
            .map_err(|err| {
                NodeInitError::checkpoint("moderation screening", screening_path, err)
            })?;
        let object_index_path = self
            .moderation_quarantine_object_index_path
            .as_ref()
            .expect("storage-backed node has an object-index checkpoint path");
        self.persist_moderation_quarantine_object_index_snapshot(
            &ModerationQuarantineObjectSnapshot::default(),
        )
        .map_err(|err| {
            NodeInitError::checkpoint("moderation quarantine object index", object_index_path, err)
        })?;
        let viewer_path = self
            .moderation_evidence_viewer_checkpoint_path
            .as_ref()
            .expect("storage-backed node has an evidence viewer checkpoint path");
        self.persist_moderation_evidence_viewer_snapshot(
            &ModerationEvidenceViewerSnapshot::default(),
        )
        .map_err(|err| NodeInitError::checkpoint("moderation evidence viewer", viewer_path, err))?;
        let auxiliary_path = self
            .auxiliary_runtime_checkpoint_path
            .as_ref()
            .expect("storage-backed node has an auxiliary checkpoint path");
        self.persist_auxiliary_runtime_checkpoint_unlocked()
            .map_err(|err| NodeInitError::checkpoint("auxiliary runtime", auxiliary_path, err))?;
        let marker_path = runtime_state_initialization_path(self.config.data_dir());
        self.finish_local_checkpoint_write(
            "runtime initialization marker",
            &marker_path,
            write_local_private_checkpoint_atomic(&marker_path, RUNTIME_STATE_INITIALIZATION_BYTES),
        )
        .map_err(|err| {
            NodeInitError::checkpoint("runtime initialization marker", &marker_path, err)
        })
    }
    /// Register a repair orchestrator used to rehydrate missing chunks remotely.
    pub fn set_repair_orchestrator(&self, orchestrator: Arc<dyn RepairOrchestrator>) {
        if let Ok(mut guard) = self.repair_orchestrator.write() {
            *guard = Some(orchestrator);
        }
    }
    /// Remove any configured repair orchestrator.
    pub fn clear_repair_orchestrator(&self) {
        if let Ok(mut guard) = self.repair_orchestrator.write() {
            *guard = None;
        }
    }
    fn repair_orchestrator(&self) -> Option<Arc<dyn RepairOrchestrator>> {
        self.repair_orchestrator
            .read()
            .ok()
            .and_then(|guard| guard.clone())
    }
    fn install_startup_governance_publisher(
        &mut self,
        publisher: Arc<dyn GovernancePublisher>,
    ) -> Result<(), GovernancePublishError> {
        if self.startup_governance_publisher.is_some() {
            return Err(GovernancePublishError::other(
                "startup Governance publisher is already installed",
            ));
        }
        let mut active = self
            .governance_publisher
            .write()
            .map_err(|_| GovernancePublishError::other("governance publisher lock poisoned"))?;
        if active.is_some() {
            return Err(GovernancePublishError::other(
                "a Governance publisher was installed before startup pinning completed",
            ));
        }
        *active = Some(Arc::clone(&publisher));
        drop(active);
        self.startup_governance_publisher = Some(publisher);
        self.flush_governance_outbox()?;
        Ok(())
    }
    /// Register the governance publisher used to surface settlement artefacts.
    ///
    /// Nodes configured with a signed filesystem publisher pin that exact
    /// instance during startup and reject later replacement.
    pub fn set_governance_publisher(&self, publisher: Arc<dyn GovernancePublisher>) {
        if let Err(err) = self.try_set_governance_publisher(publisher) {
            iroha_logger::error!(%err, "failed to register SoraFS governance publisher");
        }
    }
    /// Register a governance publisher and replay every durable pending artifact.
    ///
    /// # Errors
    ///
    /// Returns an error if startup pinned a different publisher, the publisher lock is poisoned, or
    /// a pending artifact cannot be published and durably acknowledged.
    pub fn try_set_governance_publisher(
        &self,
        publisher: Arc<dyn GovernancePublisher>,
    ) -> Result<(), GovernancePublishError> {
        if let Some(pinned) = self.startup_governance_publisher.as_ref() {
            let active = self
                .governance_publisher
                .read()
                .map_err(|_| GovernancePublishError::other("governance publisher lock poisoned"))?;
            if !Arc::ptr_eq(pinned, &publisher)
                || active
                    .as_ref()
                    .is_none_or(|candidate| !Arc::ptr_eq(candidate, pinned))
            {
                return Err(GovernancePublishError::other(
                    "startup-installed signed Governance publisher cannot be replaced",
                ));
            }
            drop(active);
            self.flush_governance_outbox()?;
            return Ok(());
        }
        *self
            .governance_publisher
            .write()
            .map_err(|_| GovernancePublishError::other("governance publisher lock poisoned"))? =
            Some(publisher);
        self.flush_governance_outbox()?;
        Ok(())
    }
    /// Remove any unpinned governance publisher.
    ///
    /// Startup-installed signed publishers cannot be cleared; use
    /// [`Self::try_clear_governance_publisher`] when the caller needs the explicit failure.
    pub fn clear_governance_publisher(&self) {
        if let Err(err) = self.try_clear_governance_publisher() {
            iroha_logger::error!(%err, "failed to clear SoraFS governance publisher");
        }
    }
    /// Remove an unpinned governance publisher.
    ///
    /// # Errors
    ///
    /// Returns an error when a signed startup publisher is pinned or the active
    /// publisher lock is poisoned.
    pub fn try_clear_governance_publisher(&self) -> Result<(), GovernancePublishError> {
        if self.startup_governance_publisher.is_some() {
            return Err(GovernancePublishError::other(
                "startup-installed signed Governance publisher cannot be cleared",
            ));
        }
        *self
            .governance_publisher
            .write()
            .map_err(|_| GovernancePublishError::other("governance publisher lock poisoned"))? =
            None;
        Ok(())
    }
    /// Return whether this node currently has a governance publisher configured.
    #[must_use]
    pub fn has_governance_publisher(&self) -> bool {
        self.governance_publisher
            .read()
            .is_ok_and(|guard| guard.is_some())
    }
    fn governance_publisher(&self) -> Option<Arc<dyn GovernancePublisher>> {
        self.governance_publisher
            .read()
            .ok()
            .and_then(|guard| guard.clone())
    }
    fn enqueue_governance_outbox_unlocked(
        &self,
        kind: GovernanceOutboxKindV1,
        payload_bytes: Vec<u8>,
    ) -> Result<(u64, bool), GovernancePublishError> {
        self.enqueue_governance_outbox_unlocked_with_reservation(
            kind,
            payload_bytes,
            None,
            GovernanceOutboxReservationUse::None,
        )
    }
    fn enqueue_governance_outbox_unlocked_with_provenance(
        &self,
        kind: GovernanceOutboxKindV1,
        payload_bytes: Vec<u8>,
        provenance: GovernanceSubmissionProvenanceV1,
    ) -> Result<(u64, bool), GovernancePublishError> {
        self.enqueue_governance_outbox_unlocked_with_reservation(
            kind,
            payload_bytes,
            Some(provenance),
            GovernanceOutboxReservationUse::None,
        )
    }
    fn enqueue_gc_eviction_governance_outbox_unlocked(
        &self,
        payload_bytes: Vec<u8>,
    ) -> Result<(u64, bool), GovernancePublishError> {
        self.enqueue_governance_outbox_unlocked_with_reservation(
            GovernanceOutboxKindV1::GcAudit,
            payload_bytes,
            None,
            GovernanceOutboxReservationUse::GcEviction,
        )
    }
    fn enqueue_governance_outbox_unlocked_with_reservation(
        &self,
        kind: GovernanceOutboxKindV1,
        payload_bytes: Vec<u8>,
        provenance: Option<GovernanceSubmissionProvenanceV1>,
        reservation_use: GovernanceOutboxReservationUse,
    ) -> Result<(u64, bool), GovernancePublishError> {
        let gc_reserved_slots = {
            let intents = self
                .gc_eviction_intents
                .read()
                .map_err(|_| GovernancePublishError::other("GC eviction intent lock poisoned"))?;
            gc_eviction_reserved_outbox_slots(&intents)?
        };
        let reserved_slots = match reservation_use {
            GovernanceOutboxReservationUse::None => gc_reserved_slots,
            GovernanceOutboxReservationUse::GcEviction => {
                if gc_reserved_slots == 0 {
                    return Err(GovernancePublishError::other(
                        "GC eviction publication has no active outbox reservation",
                    ));
                }
                0
            }
        };
        let mut outbox = self
            .governance_outbox
            .write()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?;
        insert_governance_outbox_entry(
            &mut outbox,
            kind,
            payload_bytes,
            provenance,
            self.config.runtime_retention().state_entry_limit(),
            reserved_slots,
        )
    }
    fn enqueue_governance_outbox(
        &self,
        kind: GovernanceOutboxKindV1,
        payload_bytes: Vec<u8>,
    ) -> Result<u64, GovernancePublishError> {
        self.enqueue_governance_outbox_with_optional_provenance(kind, payload_bytes, None)
    }
    fn enqueue_governance_outbox_with_optional_provenance(
        &self,
        kind: GovernanceOutboxKindV1,
        payload_bytes: Vec<u8>,
        provenance: Option<GovernanceSubmissionProvenanceV1>,
    ) -> Result<u64, GovernancePublishError> {
        let checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        let previous = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
            .clone();
        let (sequence, inserted) = match provenance {
            Some(provenance) => self.enqueue_governance_outbox_unlocked_with_provenance(
                kind,
                payload_bytes,
                provenance,
            )?,
            None => self.enqueue_governance_outbox_unlocked(kind, payload_bytes)?,
        };
        if inserted && let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            *self.governance_outbox.write().map_err(|_| {
                GovernancePublishError::other("governance outbox rollback lock poisoned")
            })? = previous;
            return Err(GovernancePublishError::other(err.to_string()));
        }
        drop(checkpoint_guard);
        Ok(sequence)
    }
    fn reconcile_gc_eviction_intents_on_startup(&self) -> Result<(), GovernancePublishError> {
        let storage = self.storage.as_ref().ok_or_else(|| {
            GovernancePublishError::other("GC eviction reconciliation requires the storage backend")
        })?;
        let gc_guard = self
            .gc_mutation_lock
            .lock()
            .map_err(|_| GovernancePublishError::other("GC mutation lock poisoned"))?;
        let drain_guard = self
            .governance_outbox_drain_lock
            .lock()
            .map_err(|_| GovernancePublishError::other("governance outbox drain lock poisoned"))?;
        let intents = self
            .gc_eviction_intents
            .read()
            .map_err(|_| GovernancePublishError::other("GC eviction intent lock poisoned"))?
            .entries
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for intent in &intents {
            self.settle_gc_eviction_intent_against_storage(
                &gc_guard,
                &drain_guard,
                storage,
                intent,
                false,
            )?;
        }
        self.validate_gc_eviction_links_against_storage(storage)
    }
    fn enqueue_sequenced_governance_outbox(
        &self,
        kind: GovernanceOutboxKindV1,
        build_payload: impl FnOnce(u64) -> Result<Vec<u8>, GovernancePublishError>,
    ) -> Result<u64, GovernancePublishError> {
        let checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        let previous = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
            .clone();
        let expected_sequence = previous.next_sequence;
        let payload_bytes = build_payload(expected_sequence)?;
        let (sequence, inserted) = self.enqueue_governance_outbox_unlocked(kind, payload_bytes)?;
        if !inserted || sequence != expected_sequence {
            *self.governance_outbox.write().map_err(|_| {
                GovernancePublishError::other("governance outbox rollback lock poisoned")
            })? = previous;
            return Err(GovernancePublishError::other(
                "sequenced governance artifact collided with an existing outbox entry",
            ));
        }
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            *self.governance_outbox.write().map_err(|_| {
                GovernancePublishError::other("governance outbox rollback lock poisoned")
            })? = previous;
            return Err(GovernancePublishError::other(err.to_string()));
        }
        drop(checkpoint_guard);
        Ok(sequence)
    }
    /// Return the number of durably pending governance publications.
    #[must_use]
    pub fn pending_governance_publication_count(&self) -> usize {
        self.governance_outbox
            .read()
            .map_or(0, |outbox| outbox.entries.len())
    }
    fn privacy_leader_lease_scope(
        &self,
        query_id: [u8; 32],
        window: PrivacyAggregateCycleWindow,
    ) -> Result<TransparencyLeaderLeaseScopeV1, GovernancePublishError> {
        let holder_identity = self.config.provider_id().ok_or_else(|| {
            GovernancePublishError::other(
                "privacy leader lease requires a configured public provider identity",
            )
        })?;
        TransparencyLeaderLeaseScopeV1::try_new(query_id, window, holder_identity.0).map_err(
            |error| {
                GovernancePublishError::other(format!("build privacy leader-lease scope: {error}"))
            },
        )
    }
    fn acquire_privacy_leader_lease(
        &self,
        query_id: [u8; 32],
        window: PrivacyAggregateCycleWindow,
        observed_at_unix: u64,
    ) -> Result<TransparencyLeaderLeaseGrantV1, GovernancePublishError> {
        let provider = self
            .transparency_leader_lease_provider
            .as_ref()
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "qualified transparency leader-lease provider is unavailable",
                )
            })?;
        let scope = self.privacy_leader_lease_scope(query_id, window)?;
        let lease_ttl = self
            .configured_privacy_aggregate_schedule()
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "privacy leader lease requires the configured publication cadence",
                )
            })?
            .cycle_seconds;
        let expires_at_unix = observed_at_unix
            .checked_add(lease_ttl)
            .ok_or_else(|| GovernancePublishError::other("privacy leader-lease expiry overflow"))?;
        let grant = provider
            .0
            .acquire(scope, observed_at_unix, expires_at_unix)
            .map_err(|error| {
                GovernancePublishError::other(format!("acquire transparency leader lease: {error}"))
            })?;
        let checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        if let Err(error) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            self.mark_durability_unhealthy(format!(
                "failed to durably checkpoint transparency leader-lease fencing floor: {error}"
            ));
            return Err(GovernancePublishError::other(format!(
                "durably checkpoint transparency leader-lease fencing floor: {error}"
            )));
        }
        drop(checkpoint_guard);
        let validated = provider
            .0
            .validate_for_use(scope, observed_at_unix)
            .map_err(|error| {
                GovernancePublishError::other(format!(
                    "validate acquired transparency leader lease: {error}"
                ))
            })?;
        if validated != grant {
            return Err(GovernancePublishError::other(
                "validated transparency leader lease changed after durable checkpoint",
            ));
        }
        Ok(grant)
    }
    fn validate_privacy_leader_lease_for_use(
        &self,
        scope: TransparencyLeaderLeaseScopeV1,
        observed_at_unix: u64,
        grant: &TransparencyLeaderLeaseGrantV1,
    ) -> Result<(), GovernancePublishError> {
        let provider = self
            .transparency_leader_lease_provider
            .as_ref()
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "qualified transparency leader-lease provider is unavailable",
                )
            })?;
        let validated = provider
            .0
            .validate_for_use(scope, observed_at_unix)
            .map_err(|error| {
                GovernancePublishError::other(format!(
                    "validate transparency leader lease for use: {error}"
                ))
            })?;
        if &validated != grant {
            return Err(GovernancePublishError::other(
                "active transparency leader lease differs from the retained grant",
            ));
        }
        Ok(())
    }
    fn release_privacy_leader_lease(
        &self,
        grant: &TransparencyLeaderLeaseGrantV1,
        observed_at_unix: u64,
    ) -> Result<(), GovernancePublishError> {
        self.transparency_leader_lease_provider
            .as_ref()
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "qualified transparency leader-lease provider is unavailable",
                )
            })?
            .0
            .release(grant.scope(), observed_at_unix)
            .map(|_| ())
            .map_err(|error| {
                GovernancePublishError::other(format!("release transparency leader lease: {error}"))
            })
    }
    fn with_privacy_leader_lease<T>(
        &self,
        query_id: [u8; 32],
        window: PrivacyAggregateCycleWindow,
        observed_at_unix: u64,
        operation: impl FnOnce(&TransparencyLeaderLeaseGrantV1) -> Result<T, GovernancePublishError>,
    ) -> Result<T, GovernancePublishError> {
        let grant = self.acquire_privacy_leader_lease(query_id, window, observed_at_unix)?;
        let result = operation(&grant);
        let release = self.release_privacy_leader_lease(&grant, observed_at_unix);
        match (result, release) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Err(error), Err(release_error)) => Err(GovernancePublishError::other(format!(
                "{error}; additionally failed to release transparency leader lease: {release_error}"
            ))),
        }
    }
    fn privacy_leader_lease_scope_for_outbox_entry(
        &self,
        entry: &GovernanceOutboxEntryV1,
    ) -> Result<Option<TransparencyLeaderLeaseScopeV1>, GovernancePublishError> {
        if entry.kind != GovernanceOutboxKindV1::TransparencyLedgerPublication {
            return Ok(None);
        }
        let publication = decode_canonical_governance_payload::<ModerationLedgerCyclePublicationV1>(
            &entry.payload_bytes,
        )?;
        if publication.privacy_aggregates.is_empty() {
            return Ok(None);
        }
        let release_ledger = self
            .privacy_release_ledger
            .read()
            .map_err(|_| GovernancePublishError::other("privacy release ledger poisoned"))?;
        let record = release_ledger
            .records
            .iter()
            .find(|record| record.release_id == publication.block.cycle_id);
        #[cfg(test)]
        if record.is_none() && self.config.privacy_aggregate_policy().is_none() {
            return Ok(None);
        }
        let record = record.ok_or_else(|| {
            GovernancePublishError::other(
                "privacy publication outbox entry has no durable release record",
            )
        })?;
        if record.status != transparency::PrivacyReleaseStatusV1::Published
            || record.publication_payload_digest != Some(entry.payload_digest)
        {
            return Err(GovernancePublishError::other(
                "privacy publication outbox entry does not match its durable release record",
            ));
        }
        self.privacy_leader_lease_scope(
            record.query_id,
            PrivacyAggregateCycleWindow {
                cycle_start_unix: record.cycle_start_unix,
                cycle_end_unix: record.cycle_end_unix,
                due_at_unix: record.due_at_unix,
            },
        )
        .map(Some)
    }
    fn privacy_publication_authorization_for_outbox_entry(
        &self,
        entry: &GovernanceOutboxEntryV1,
        grant: &TransparencyLeaderLeaseGrantV1,
    ) -> Result<PrivacyPublicationAuthorizationV1, GovernancePublishError> {
        validate_governance_outbox_entry(entry)?;
        let publication = decode_canonical_governance_payload::<ModerationLedgerCyclePublicationV1>(
            &entry.payload_bytes,
        )?;
        if publication.privacy_aggregates.is_empty() {
            return Err(GovernancePublishError::other(
                "privacy publication authorization requires a privacy aggregate payload",
            ));
        }
        let release_ledger = self
            .privacy_release_ledger
            .read()
            .map_err(|_| GovernancePublishError::other("privacy release ledger poisoned"))?
            .clone();
        release_ledger.validate().map_err(|error| {
            GovernancePublishError::other(format!(
                "invalid durable privacy release ledger: {error}"
            ))
        })?;
        let release = release_ledger
            .records
            .iter()
            .find(|record| record.release_id == publication.block.cycle_id)
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "privacy publication outbox entry has no durable release record",
                )
            })?;
        let local_head = release_ledger.head(release.query_id).map_err(|error| {
            GovernancePublishError::other(format!(
                "privacy release ledger does not have a canonical head: {error}"
            ))
        })?;
        let anchor = self.privacy_release_anchor.as_deref().ok_or_else(|| {
            GovernancePublishError::other("finalized privacy release anchor is unavailable")
        })?;
        let finalized = anchor.finalized_head(release.query_id).map_err(|error| {
            GovernancePublishError::other(format!(
                "read finalized privacy release anchor before publication: {error}"
            ))
        })?;
        if finalized != local_head {
            return Err(GovernancePublishError::other(
                "privacy publication release chain is not exactly finalized",
            ));
        }
        let authorization = PrivacyPublicationAuthorizationV1::try_new(
            grant,
            finalized,
            release,
            entry.payload_digest,
        )?;
        authorization.validate_publication(&publication, &entry.payload_bytes)?;
        Ok(authorization)
    }
    /// Publish and durably acknowledge all queued governance artifacts.
    ///
    /// Publication is at-least-once. Publishers must therefore treat identical
    /// canonical payload bytes idempotently.
    ///
    /// # Errors
    ///
    /// Returns an error if publication fails, the outbox is corrupt, or its
    /// acknowledgement checkpoint cannot be committed safely.
    pub fn flush_governance_outbox(&self) -> Result<usize, GovernancePublishError> {
        self.flush_governance_outbox_at(unix_now_secs(), None)
    }
    fn flush_governance_outbox_at(
        &self,
        observed_at_unix: u64,
        retained_lease: Option<&TransparencyLeaderLeaseGrantV1>,
    ) -> Result<usize, GovernancePublishError> {
        let _drain_guard = self
            .governance_outbox_drain_lock
            .lock()
            .map_err(|_| GovernancePublishError::other("governance outbox drain lock poisoned"))?;
        let Some(publisher) = self.governance_publisher() else {
            return Ok(0);
        };
        let mut published = 0usize;
        loop {
            self.ensure_durability_healthy()
                .map_err(GovernancePublishError::other)?;
            let Some(entry) = self
                .governance_outbox
                .read()
                .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
                .entries
                .first_key_value()
                .map(|(_, entry)| entry.clone())
            else {
                return Ok(published);
            };
            if let Some(scope) = self.privacy_leader_lease_scope_for_outbox_entry(&entry)? {
                if let Some(grant) = retained_lease.filter(|grant| grant.scope() == scope) {
                    self.validate_privacy_leader_lease_for_use(scope, observed_at_unix, grant)?;
                    let authorization =
                        self.privacy_publication_authorization_for_outbox_entry(&entry, grant)?;
                    publish_governance_outbox_entry(
                        publisher.as_ref(),
                        &entry,
                        Some(&authorization),
                    )?;
                } else {
                    self.with_privacy_leader_lease(
                        scope.query_id(),
                        scope.window(),
                        observed_at_unix,
                        |grant| {
                            let authorization = self
                                .privacy_publication_authorization_for_outbox_entry(
                                    &entry, grant,
                                )?;
                            publish_governance_outbox_entry(
                                publisher.as_ref(),
                                &entry,
                                Some(&authorization),
                            )
                        },
                    )?;
                }
            } else {
                publish_governance_outbox_entry(publisher.as_ref(), &entry, None)?;
            }
            let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
                GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
            })?;
            self.ensure_durability_healthy()
                .map_err(GovernancePublishError::other)?;
            let removed = self
                .governance_outbox
                .write()
                .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
                .entries
                .remove(&entry.sequence);
            if removed.as_ref() != Some(&entry) {
                return Err(GovernancePublishError::other(
                    "governance outbox changed while publication was in flight",
                ));
            }
            if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
                if err.committed {
                    return Err(GovernancePublishError::other(err.to_string()));
                }
                self.governance_outbox
                    .write()
                    .map_err(|_| {
                        GovernancePublishError::other(
                            "governance outbox acknowledgement rollback lock poisoned",
                        )
                    })?
                    .entries
                    .insert(entry.sequence, entry);
                return Err(GovernancePublishError::other(err.to_string()));
            }
            published = published.saturating_add(1);
        }
    }
    /// Admit, persist, and publish an externally authorized reputation snapshot.
    ///
    /// The local snapshot, head linkage, replay event, and publication intent are committed
    /// together before external delivery. Retrying the exact same snapshot id is idempotent and
    /// retries publication; conflicting ids or non-monotonic heads are rejected.
    pub fn publish_signed_reputation_snapshot(
        &self,
        envelope: SignedReputationSnapshotV1,
    ) -> Result<(), GovernancePublishError> {
        let checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        let policy = self.reputation_trust_policy.as_deref().ok_or_else(|| {
            GovernancePublishError::other(
                "signed reputation admission is disabled: no external trust policy is configured",
            )
        })?;
        let admitted_at_unix = unix_now_secs();
        envelope.verify(policy, admitted_at_unix).map_err(|err| {
            GovernancePublishError::other(format!("signed reputation admission failed: {err}"))
        })?;
        let snapshot = &envelope.snapshot;
        let encoded = envelope.canonical_bytes().map_err(|err| {
            GovernancePublishError::other(format!("encode signed reputation snapshot: {err}"))
        })?;
        let encoded_len = u64::try_from(encoded.len()).map_err(|_| {
            GovernancePublishError::other(
                "signed reputation snapshot length does not fit checkpoint accounting",
            )
        })?;
        let mut snapshots = self
            .reputation_snapshots
            .write()
            .map_err(|_| GovernancePublishError::other("reputation snapshot index poisoned"))?;
        if snapshots
            .get(&snapshot.snapshot_id)
            .is_some_and(|existing| existing.envelope != envelope)
        {
            return Err(GovernancePublishError::other(format!(
                "signed reputation snapshot id {} conflicts with retained canonical envelope bytes",
                hex::encode(snapshot.snapshot_id)
            )));
        }
        let retained_envelope_bytes = snapshots.values().try_fold(0_u64, |total, admitted| {
            total.checked_add(admitted.encoded_len).ok_or_else(|| {
                GovernancePublishError::other(
                    "retained signed reputation checkpoint byte accounting overflow",
                )
            })
        })?;
        let outbox = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?;
        let mut pending_envelope_bytes = 0_u64;
        let mut exact_envelope_pending = false;
        for entry in outbox
            .entries
            .values()
            .filter(|entry| entry.kind == GovernanceOutboxKindV1::SignedReputationSnapshot)
        {
            let len = u64::try_from(entry.payload_bytes.len()).map_err(|_| {
                GovernancePublishError::other(
                    "pending signed reputation outbox length does not fit u64",
                )
            })?;
            pending_envelope_bytes = pending_envelope_bytes.checked_add(len).ok_or_else(|| {
                GovernancePublishError::other(
                    "pending signed reputation checkpoint byte accounting overflow",
                )
            })?;
            exact_envelope_pending |= entry.payload_bytes == encoded;
        }
        drop(outbox);
        let additional_map_bytes = if snapshots.contains_key(&snapshot.snapshot_id) {
            0
        } else {
            encoded_len
        };
        let additional_outbox_bytes = if exact_envelope_pending {
            0
        } else {
            encoded_len
        };
        let required_reputation_bytes = retained_envelope_bytes
            .checked_add(pending_envelope_bytes)
            .and_then(|total| total.checked_add(additional_map_bytes))
            .and_then(|total| total.checked_add(additional_outbox_bytes))
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "signed reputation checkpoint byte accounting overflow",
                )
            })?;
        let checkpoint_max_bytes = self.config.runtime_retention().checkpoint_max_bytes();
        if required_reputation_bytes > checkpoint_max_bytes {
            return Err(GovernancePublishError::other(format!(
                "signed reputation state requires at least {required_reputation_bytes} checkpoint bytes, exceeding configured limit {checkpoint_max_bytes}"
            )));
        }
        if snapshots.contains_key(&snapshot.snapshot_id) {
            drop(snapshots);
            let previous_outbox = self
                .governance_outbox
                .read()
                .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
                .clone();
            let (_, inserted) = self.enqueue_governance_outbox_unlocked(
                GovernanceOutboxKindV1::SignedReputationSnapshot,
                encoded,
            )?;
            if inserted && let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
                if err.committed {
                    return Err(GovernancePublishError::other(err.to_string()));
                }
                *self.governance_outbox.write().map_err(|_| {
                    GovernancePublishError::other("governance outbox rollback lock poisoned")
                })? = previous_outbox;
                return Err(GovernancePublishError::other(err.to_string()));
            }
            drop(checkpoint_guard);
            self.flush_governance_outbox()?;
            return Ok(());
        }
        let previous_snapshots = snapshots.clone();
        let mut events = self
            .reputation_events
            .write()
            .map_err(|_| GovernancePublishError::other("reputation event history poisoned"))?;
        let previous_events = events.clone();
        let mut latest = self
            .latest_reputation_snapshot
            .write()
            .map_err(|_| GovernancePublishError::other("reputation snapshot cache poisoned"))?;
        let previous_latest = latest.clone();
        let previous_outbox = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
            .clone();
        validate_reputation_snapshot_transition(latest.as_ref(), snapshot).map_err(|err| {
            GovernancePublishError::other(format!(
                "signed reputation snapshot does not extend the exact retained head: {err}"
            ))
        })?;
        let next_sequence = events
            .latest_sequence
            .checked_add(1)
            .ok_or_else(|| GovernancePublishError::other("event sequence exhausted"))?;
        let event =
            ReputationSnapshotEventV1::from_snapshot(next_sequence, snapshot).map_err(|err| {
                GovernancePublishError::other(format!(
                    "validated reputation snapshot could not produce an event: {err}"
                ))
            })?;
        snapshots.insert(
            snapshot.snapshot_id,
            AdmittedReputationSnapshotV1 {
                version: ADMITTED_REPUTATION_SNAPSHOT_VERSION_V1,
                admitted_at_unix,
                encoded_len,
                envelope: envelope.clone(),
            },
        );
        let event = match events.append(|sequence| {
            debug_assert_eq!(sequence, next_sequence);
            event
        }) {
            Ok(event) => event,
            Err(err) => {
                *snapshots = previous_snapshots;
                return Err(err);
            }
        };
        let retained_snapshot_ids = events
            .events
            .iter()
            .map(|event| event.snapshot_id)
            .chain(std::iter::once(snapshot.snapshot_id))
            .collect::<BTreeSet<_>>();
        let state_limit = self.config.runtime_retention().state_entry_limit();
        while snapshots.len() > state_limit {
            let Some(evict) = snapshots
                .values()
                .filter(|candidate| {
                    !retained_snapshot_ids.contains(&candidate.envelope.snapshot.snapshot_id)
                })
                .min_by_key(|candidate| {
                    (
                        candidate.envelope.snapshot.generated_at_unix,
                        candidate.envelope.snapshot.snapshot_id,
                    )
                })
                .map(|candidate| candidate.envelope.snapshot.snapshot_id)
            else {
                *snapshots = previous_snapshots;
                *events = previous_events;
                return Err(GovernancePublishError::other(format!(
                    "reputation snapshot retention exhausted (limit {state_limit}); retained events prevent safe eviction"
                )));
            };
            snapshots.remove(&evict);
        }
        if events
            .events
            .iter()
            .any(|event| !snapshots.contains_key(&event.snapshot_id))
        {
            *snapshots = previous_snapshots;
            *events = previous_events;
            return Err(GovernancePublishError::other(
                "reputation event suffix references an evicted snapshot",
            ));
        }
        *latest = Some(snapshot.clone());
        drop(snapshots);
        drop(events);
        drop(latest);
        if let Err(err) = self.enqueue_governance_outbox_unlocked(
            GovernanceOutboxKindV1::SignedReputationSnapshot,
            encoded,
        ) {
            *self.reputation_snapshots.write().map_err(|_| {
                GovernancePublishError::other("reputation snapshot rollback lock poisoned")
            })? = previous_snapshots;
            *self.reputation_events.write().map_err(|_| {
                GovernancePublishError::other("reputation event rollback lock poisoned")
            })? = previous_events;
            *self.latest_reputation_snapshot.write().map_err(|_| {
                GovernancePublishError::other("reputation latest rollback lock poisoned")
            })? = previous_latest;
            return Err(err);
        }
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            *self.reputation_snapshots.write().map_err(|_| {
                GovernancePublishError::other("reputation snapshot rollback lock poisoned")
            })? = previous_snapshots;
            *self.reputation_events.write().map_err(|_| {
                GovernancePublishError::other("reputation event rollback lock poisoned")
            })? = previous_events;
            *self.latest_reputation_snapshot.write().map_err(|_| {
                GovernancePublishError::other("reputation latest rollback lock poisoned")
            })? = previous_latest;
            *self.governance_outbox.write().map_err(|_| {
                GovernancePublishError::other("governance outbox rollback lock poisoned")
            })? = previous_outbox;
            return Err(GovernancePublishError::other(err.to_string()));
        }
        drop(checkpoint_guard);
        let _ = self.reputation_event_sender.send(event);
        self.flush_governance_outbox()?;
        Ok(())
    }
    /// Publish an appeal-finance report with server-verified Torii provenance.
    pub fn publish_authenticated_appeal_finance_report(
        &self,
        report: SoraFsAppealFinanceReportV1,
        publisher_account: AccountId,
    ) -> Result<(), GovernancePublishError> {
        self.publish_appeal_finance_report_with_provenance(
            report,
            GovernanceSubmissionProvenanceV1::new(
                publisher_account,
                GovernanceSubmissionOriginV1::AppealFinanceReport,
            ),
        )
    }
    fn publish_appeal_finance_report_with_provenance(
        &self,
        report: SoraFsAppealFinanceReportV1,
        provenance: GovernanceSubmissionProvenanceV1,
    ) -> Result<(), GovernancePublishError> {
        report.validate().map_err(|err| {
            GovernancePublishError::other(format!("invalid appeal finance report: {err}"))
        })?;
        let encoded = norito::to_bytes(&report).map_err(|err| {
            GovernancePublishError::other(format!("encode appeal finance report: {err}"))
        })?;
        self.enqueue_governance_outbox_with_optional_provenance(
            GovernanceOutboxKindV1::AppealFinanceReport,
            encoded,
            Some(provenance),
        )?;
        self.flush_governance_outbox()?;
        Ok(())
    }
    /// Publish a typed non-privacy SoraFS transparency ledger cycle.
    ///
    /// Privacy aggregate cycles must use the finalized release scheduler so
    /// durable mutation, anchor advancement, publication, and retry all carry
    /// the same externally fenced leader lease.
    pub fn publish_transparency_ledger_publication(
        &self,
        publication: ModerationLedgerCyclePublicationV1,
    ) -> Result<(), GovernancePublishError> {
        publication.validate().map_err(|err| {
            GovernancePublishError::other(format!("invalid transparency ledger publication: {err}"))
        })?;
        if !publication.privacy_aggregates.is_empty() {
            return Err(GovernancePublishError::other(
                "privacy transparency publication must use the finalized release scheduler and its exact live leader lease",
            ));
        }
        let encoded = norito::to_bytes(&publication).map_err(|err| {
            GovernancePublishError::other(format!("encode transparency ledger publication: {err}"))
        })?;
        self.enqueue_governance_outbox(
            GovernanceOutboxKindV1::TransparencyLedgerPublication,
            encoded,
        )?;
        self.flush_governance_outbox()?;
        Ok(())
    }
    /// Publish a typed proof-token issuance summary to the governance pipeline.
    pub fn publish_proof_token_issuance(
        &self,
        issuance: ProofTokenIssuanceV1,
    ) -> Result<(), GovernancePublishError> {
        self.publish_proof_token_issuance_with_provenance(issuance, None)
    }
    fn publish_proof_token_issuance_with_provenance(
        &self,
        issuance: ProofTokenIssuanceV1,
        provenance: Option<GovernanceSubmissionProvenanceV1>,
    ) -> Result<(), GovernancePublishError> {
        issuance.validate().map_err(|err| {
            GovernancePublishError::other(format!("invalid proof-token issuance: {err}"))
        })?;
        let encoded = norito::to_bytes(&issuance).map_err(|err| {
            GovernancePublishError::other(format!("encode proof-token issuance: {err}"))
        })?;
        self.enqueue_governance_outbox_with_optional_provenance(
            GovernanceOutboxKindV1::ProofTokenIssuance,
            encoded,
            provenance,
        )?;
        self.flush_governance_outbox()?;
        Ok(())
    }
    /// Derive and publish a proof-token issuance summary from an issued `SFGT` frame.
    ///
    /// The frame signature is verified with `signer_key` before publication.
    /// Runtime digest keys are deliberately not accepted or persisted here.
    pub fn publish_proof_token_frame_issuance(
        &self,
        encoded_token: &[u8],
        signer_key: [u8; 32],
        evidence_digest: Option<[u8; 32]>,
        policy_digest: Option<[u8; 32]>,
        metadata: Vec<ModerationLedgerMetadataV1>,
    ) -> Result<ProofTokenIssuanceV1, GovernancePublishError> {
        let issuance = transparency::proof_token_issuance_from_frame(
            encoded_token,
            signer_key,
            evidence_digest,
            policy_digest,
            metadata,
        )
        .map_err(|err| {
            GovernancePublishError::other(format!("ingest proof-token issuance: {err}"))
        })?;
        self.publish_proof_token_issuance(issuance.clone())?;
        Ok(issuance)
    }
    /// Derive and publish a proof-token issuance summary from URL-safe base64.
    ///
    /// This is the transport-friendly counterpart to [`Self::publish_proof_token_frame_issuance`].
    pub fn publish_proof_token_base64_issuance(
        &self,
        token_b64: &str,
        signer_key: [u8; 32],
        evidence_digest: Option<[u8; 32]>,
        policy_digest: Option<[u8; 32]>,
        metadata: Vec<ModerationLedgerMetadataV1>,
    ) -> Result<ProofTokenIssuanceV1, GovernancePublishError> {
        let issuance = transparency::proof_token_issuance_from_base64(
            token_b64,
            signer_key,
            evidence_digest,
            policy_digest,
            metadata,
        )
        .map_err(|err| {
            GovernancePublishError::other(format!("ingest proof-token issuance: {err}"))
        })?;
        self.publish_proof_token_issuance(issuance.clone())?;
        Ok(issuance)
    }
    /// Derive and publish a proof-token issuance with verified Torii provenance.
    pub fn publish_authenticated_proof_token_base64_issuance(
        &self,
        token_b64: &str,
        signer_key: [u8; 32],
        evidence_digest: Option<[u8; 32]>,
        policy_digest: Option<[u8; 32]>,
        metadata: Vec<ModerationLedgerMetadataV1>,
        publisher_account: AccountId,
    ) -> Result<ProofTokenIssuanceV1, GovernancePublishError> {
        let issuance = transparency::proof_token_issuance_from_base64(
            token_b64,
            signer_key,
            evidence_digest,
            policy_digest,
            metadata,
        )
        .map_err(|err| {
            GovernancePublishError::other(format!("ingest proof-token issuance: {err}"))
        })?;
        self.publish_proof_token_issuance_with_provenance(
            issuance.clone(),
            Some(GovernanceSubmissionProvenanceV1::new(
                publisher_account,
                GovernanceSubmissionOriginV1::TransparencyTokenIssuance,
            )),
        )?;
        Ok(issuance)
    }
    /// Record one privacy-safe source entry for later transparency ledger publication.
    pub fn record_transparency_ledger_source_entry(
        &self,
        entry: TransparencyLedgerSourceEntry,
    ) -> Result<(), GovernancePublishError> {
        let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        entry.validate().map_err(|err| {
            GovernancePublishError::other(format!(
                "invalid transparency ledger source entry: {err}"
            ))
        })?;
        let mut guard = self
            .transparency_ledger_source_entries
            .write()
            .map_err(|_| {
                GovernancePublishError::other("transparency ledger source-entry index poisoned")
            })?;
        if let Some(existing) = guard.get(&entry.event_id) {
            if existing == &entry {
                return Ok(());
            }
            return Err(GovernancePublishError::other(format!(
                "transparency ledger source entry `{}` conflicts with retained canonical data",
                entry.event_id
            )));
        }
        let limit = self.config.runtime_retention().state_entry_limit();
        if guard.len() >= limit {
            return Err(GovernancePublishError::other(format!(
                "transparency source-entry retention exhausted (limit {limit})"
            )));
        }
        let event_id = entry.event_id.clone();
        guard.insert(event_id.clone(), entry);
        drop(guard);
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            self.transparency_ledger_source_entries
                .write()
                .map_err(|_| {
                    GovernancePublishError::other(
                        "transparency source-entry rollback lock poisoned",
                    )
                })?
                .remove(&event_id);
            return Err(GovernancePublishError::other(err.to_string()));
        }
        Ok(())
    }
    /// Derive and record a transparency source entry from a GAR enforcement receipt.
    pub fn record_gar_enforcement_receipt_transparency_entry(
        &self,
        receipt: &GarEnforcementReceiptV1,
    ) -> Result<(), GovernancePublishError> {
        let entry = transparency::gar_enforcement_receipt_source_entry(receipt).map_err(|err| {
            GovernancePublishError::other(format!(
                "derive GAR enforcement receipt transparency source entry: {err}"
            ))
        })?;
        self.record_transparency_ledger_source_entry(entry)
    }
    /// Derive and record a transparency source entry from a moderation governance event.
    #[cfg(test)]
    pub fn record_moderation_ballot_governance_transparency_entry(
        &self,
        event: &SoraFsModerationBallotGovernanceEventV1,
    ) -> Result<(), GovernancePublishError> {
        let entry = transparency::moderation_ballot_governance_event_source_entry(event).map_err(
            |err| {
                GovernancePublishError::other(format!(
                    "derive moderation governance transparency source entry: {err}"
                ))
            },
        )?;
        self.record_transparency_ledger_source_entry(entry)
    }
    /// Derive and record a transparency source entry from an appeal finance report.
    pub fn record_appeal_finance_report_transparency_entry(
        &self,
        report: &SoraFsAppealFinanceReportV1,
    ) -> Result<(), GovernancePublishError> {
        let entry = transparency::appeal_finance_report_source_entry(report).map_err(|err| {
            GovernancePublishError::other(format!(
                "derive appeal finance report transparency source entry: {err}"
            ))
        })?;
        self.record_transparency_ledger_source_entry(entry)
    }
    /// Derive and record a transparency source entry from an appeal finance settlement receipt.
    pub fn record_appeal_finance_settlement_receipt_transparency_entry(
        &self,
        receipt: &SoraFsAppealFinanceSettlementReceiptV1,
    ) -> Result<(), GovernancePublishError> {
        let entry = transparency::appeal_finance_settlement_receipt_source_entry(receipt).map_err(
            |err| {
                GovernancePublishError::other(format!(
                    "derive appeal finance settlement receipt transparency source entry: {err}"
                ))
            },
        )?;
        self.record_transparency_ledger_source_entry(entry)
    }
    /// Return the number of source entries currently retained by the transparency worker.
    #[must_use]
    pub fn transparency_ledger_source_entry_count(&self) -> usize {
        self.transparency_ledger_source_entries
            .read()
            .map(|guard| guard.len())
            .unwrap_or_default()
    }
    /// Build and publish a transparency cycle from locally recorded source entries.
    ///
    /// The worker selects retained source entries whose occurrence timestamps fall inside the
    /// requested cycle window, sorts them deterministically, assigns stable entry ids and sequence
    /// numbers, and publishes the resulting `ModerationLedgerCyclePublicationV1`.
    pub fn publish_transparency_ledger_cycle_from_source_entries(
        &self,
        cycle_id: [u8; 16],
        cycle_start_unix: u64,
        cycle_end_unix: u64,
        generated_at_unix: u64,
        previous_block_hash: Option<[u8; 32]>,
    ) -> Result<ModerationLedgerCyclePublicationV1, GovernancePublishError> {
        let events = self
            .transparency_ledger_source_entries
            .read()
            .map_err(|_| {
                GovernancePublishError::other("transparency ledger source-entry index poisoned")
            })?
            .values()
            .filter(|entry| {
                entry.occurred_at_unix >= cycle_start_unix
                    && entry.occurred_at_unix < cycle_end_unix
            })
            .cloned()
            .collect::<Vec<_>>();
        let entries: Vec<ModerationLedgerEntryV1> =
            transparency::build_transparency_ledger_entries_from_source_events(
                cycle_id,
                cycle_start_unix,
                cycle_end_unix,
                generated_at_unix,
                &events,
            )
            .map_err(|err| {
                GovernancePublishError::other(format!(
                    "build transparency ledger source cycle: {err}"
                ))
            })?;
        let publication = ModerationLedgerCyclePublicationV1::from_entries(
            cycle_id,
            cycle_start_unix,
            cycle_end_unix,
            generated_at_unix,
            previous_block_hash,
            &entries,
        )
        .map_err(|err| {
            GovernancePublishError::other(format!(
                "build transparency ledger source publication: {err}"
            ))
        })?;
        self.publish_transparency_ledger_publication(publication.clone())?;
        let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        let mut source_entries = self
            .transparency_ledger_source_entries
            .write()
            .map_err(|_| {
                GovernancePublishError::other("transparency ledger source-entry index poisoned")
            })?;
        let previous_entries = source_entries.clone();
        source_entries.retain(|_, entry| {
            entry.occurred_at_unix < cycle_start_unix || entry.occurred_at_unix >= cycle_end_unix
        });
        drop(source_entries);
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            *self
                .transparency_ledger_source_entries
                .write()
                .map_err(|_| {
                    GovernancePublishError::other(
                        "transparency source-entry rollback lock poisoned",
                    )
                })? = previous_entries;
            return Err(GovernancePublishError::other(err.to_string()));
        }
        Ok(publication)
    }
    /// Record one source event for later SFM-4c privacy aggregate publication.
    #[cfg(test)]
    pub(crate) fn record_privacy_aggregate_source_event(
        &self,
        event: PrivacyAggregateSourceEvent,
    ) -> Result<PrivacySourceEventRecordOutcomeV1, GovernancePublishError> {
        if event.provenance.is_some() {
            return Err(GovernancePublishError::other(
                "state-derived privacy aggregate source events cannot supply authenticated provenance",
            ));
        }
        self.record_privacy_aggregate_source_event_inner(event)
    }
    /// Record a privacy aggregate source event with server-verified Torii provenance.
    pub fn record_authenticated_privacy_aggregate_source_event(
        &self,
        mut event: PrivacyAggregateSourceEvent,
        publisher_account: AccountId,
    ) -> Result<PrivacySourceEventRecordOutcomeV1, GovernancePublishError> {
        event.provenance = Some(GovernanceSubmissionProvenanceV1::new(
            publisher_account,
            GovernanceSubmissionOriginV1::PrivacyAggregateSourceEvent,
        ));
        self.record_privacy_aggregate_source_event_inner(event)
    }
    fn record_privacy_aggregate_source_event_inner(
        &self,
        event: PrivacyAggregateSourceEvent,
    ) -> Result<PrivacySourceEventRecordOutcomeV1, GovernancePublishError> {
        let _mutation_guard = self.runtime_mutation_lock.lock().map_err(|_| {
            GovernancePublishError::other("runtime publication transaction lock poisoned")
        })?;
        let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        event.validate().map_err(|err| {
            GovernancePublishError::other(format!("invalid privacy aggregate source event: {err}"))
        })?;
        let canonical_digest = event.canonical_digest().map_err(|err| {
            GovernancePublishError::other(format!("digest privacy aggregate source event: {err}"))
        })?;
        let configured_policy = self.config.privacy_aggregate_policy();
        let configured_schedule = self.config.privacy_aggregate_schedule();
        let active_query = match (configured_schedule, configured_policy) {
            (Some(schedule), Some(policy)) => {
                schedule.validate().map_err(|err| {
                    GovernancePublishError::other(format!("privacy aggregate schedule: {err}"))
                })?;
                let config = policy.cycle_config();
                config.validate().map_err(|err| {
                    GovernancePublishError::other(format!("privacy aggregate policy: {err}"))
                })?;
                if schedule.first_cycle_start_unix != config.first_cycle_start_unix
                    || schedule.cycle_seconds != config.cycle_seconds
                {
                    return Err(GovernancePublishError::other(
                        "privacy aggregate schedule conflicts with the governed query cadence",
                    ));
                }
                let release_ledger = self
                    .privacy_release_ledger
                    .read()
                    .map_err(|_| GovernancePublishError::other("privacy release ledger poisoned"))?
                    .clone();
                release_ledger.validate().map_err(|error| {
                    GovernancePublishError::other(format!(
                        "invalid privacy release ledger: {error}"
                    ))
                })?;
                validate_privacy_release_lineage(&release_ledger, schedule, &config)?;
                Some((schedule, config))
            }
            (None, None) => None,
            _ => {
                return Err(GovernancePublishError::other(
                    "privacy aggregate schedule and governed policy must be enabled together",
                ));
            }
        };
        if let Some(existing_digest) = self
            .privacy_source_event_receipts
            .read()
            .map_err(|_| {
                GovernancePublishError::other("privacy source-event receipt index poisoned")
            })?
            .get(&event.event_id)
            .copied()
        {
            return if existing_digest == canonical_digest {
                Ok(PrivacySourceEventRecordOutcomeV1::AlreadyRecorded)
            } else {
                Err(GovernancePublishError::other(
                    "privacy aggregate source-event idempotency key equivocation",
                ))
            };
        }
        if let Some((schedule, config)) = active_query {
            config.validate_source_event(&event).map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid privacy aggregate source event under governed policy: {err}"
                ))
            })?;
            let window = schedule
                .event_window(event.occurred_at_unix)
                .map_err(|err| {
                    GovernancePublishError::other(format!("privacy aggregate schedule: {err}"))
                })?;
            let cycle_id = transparency::privacy_aggregate_cycle_id(
                config.query_id,
                window.cycle_start_unix,
                window.cycle_end_unix,
            );
            if self
                .published_privacy_aggregate_cycles
                .read()
                .map_err(|_| GovernancePublishError::other("privacy cycle index poisoned"))?
                .contains(&cycle_id)
            {
                return Err(GovernancePublishError::other(
                    "privacy aggregate source event targets a finalized release window",
                ));
            }
        }
        let mut event_guard = self
            .privacy_aggregate_source_events
            .write()
            .map_err(|_| GovernancePublishError::other("privacy aggregate event index poisoned"))?;
        if event_guard.contains_key(&event.event_id) {
            return Err(GovernancePublishError::other(
                "privacy aggregate source event exists without its durable receipt",
            ));
        }
        let limit = self.config.runtime_retention().state_entry_limit();
        if event_guard.len() >= limit {
            return Err(GovernancePublishError::other(format!(
                "privacy aggregate source-event retention exhausted (limit {limit})"
            )));
        }
        let mut receipt_guard = self.privacy_source_event_receipts.write().map_err(|_| {
            GovernancePublishError::other("privacy source-event receipt index poisoned")
        })?;
        if receipt_guard.len() >= limit {
            return Err(GovernancePublishError::other(format!(
                "privacy aggregate source-event receipt retention exhausted (limit {limit})"
            )));
        }
        let event_id = event.event_id.clone();
        event_guard.insert(event_id.clone(), event);
        receipt_guard.insert(event_id.clone(), canonical_digest);
        drop(receipt_guard);
        drop(event_guard);
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            self.privacy_aggregate_source_events
                .write()
                .map_err(|_| {
                    GovernancePublishError::other("privacy source-event rollback lock poisoned")
                })?
                .remove(&event_id);
            self.privacy_source_event_receipts
                .write()
                .map_err(|_| {
                    GovernancePublishError::other(
                        "privacy source-event receipt rollback lock poisoned",
                    )
                })?
                .remove(&event_id);
            return Err(GovernancePublishError::other(err.to_string()));
        }
        Ok(PrivacySourceEventRecordOutcomeV1::Recorded)
    }
    /// Return the number of source events currently retained by the aggregate worker.
    #[must_use]
    pub fn privacy_aggregate_source_event_count(&self) -> usize {
        self.privacy_aggregate_source_events
            .read()
            .map(|guard| guard.len())
            .unwrap_or_default()
    }
    /// Return a consistent copy of the durable privacy composition-budget ledger.
    ///
    /// # Errors
    ///
    /// Returns an error if the local budget lock is poisoned.
    pub fn privacy_composition_budget_snapshot(
        &self,
    ) -> Result<PrivacyCompositionBudgetLedgerV1, GovernancePublishError> {
        self.privacy_composition_budget
            .read()
            .map_err(|_| GovernancePublishError::other("privacy composition budget poisoned"))
            .map(|budget| budget.clone())
    }
    fn reconcile_privacy_release_anchor(&self) -> Result<(), GovernancePublishError> {
        self.reconcile_privacy_release_anchor_at(unix_now_secs(), None)
    }
    fn reconcile_privacy_release_anchor_at(
        &self,
        observed_at_unix: u64,
        retained_lease: Option<&TransparencyLeaderLeaseGrantV1>,
    ) -> Result<(), GovernancePublishError> {
        let policy = self.config.privacy_aggregate_policy();
        let schedule = self.configured_privacy_aggregate_schedule();
        let release_ledger = self
            .privacy_release_ledger
            .read()
            .map_err(|_| GovernancePublishError::other("privacy release ledger poisoned"))?
            .clone();
        release_ledger.validate().map_err(|error| {
            GovernancePublishError::other(format!(
                "invalid durable privacy release ledger: {error}"
            ))
        })?;
        let configured_query_id = policy.map(config::PrivacyAggregatePolicyConfig::query_id);
        let query_id = match release_ledger.records.last() {
            Some(record) => {
                if configured_query_id.is_some_and(|query_id| query_id != record.query_id) {
                    return Err(GovernancePublishError::other(
                        "durable privacy release state does not match the configured query",
                    ));
                }
                if let Some(policy) = policy {
                    let config = policy.cycle_config();
                    let schedule = schedule.ok_or_else(|| {
                        GovernancePublishError::other(
                            "durable privacy release cadence does not match the configured query lineage",
                        )
                    })?;
                    validate_privacy_release_lineage(&release_ledger, schedule, &config)?;
                }
                record.query_id
            }
            None => {
                let Some(query_id) = configured_query_id else {
                    return Ok(());
                };
                query_id
            }
        };
        let local_head = release_ledger.head(query_id).map_err(|error| {
            GovernancePublishError::other(format!(
                "privacy release ledger does not match the configured query: {error}"
            ))
        })?;
        let requires_anchor = policy.is_some() || !release_ledger.records.is_empty();
        let Some(anchor) = self.privacy_release_anchor.as_deref() else {
            if requires_anchor {
                return Err(GovernancePublishError::other(
                    "finalized privacy release anchor is unavailable",
                ));
            }
            return Ok(());
        };
        let mut finalized = anchor.finalized_head(query_id).map_err(|error| {
            GovernancePublishError::other(format!("read finalized privacy release anchor: {error}"))
        })?;
        if !finalized.validate() || finalized.query_id() != query_id {
            return Err(GovernancePublishError::other(
                "finalized privacy release anchor is malformed or bound to another query",
            ));
        }
        if finalized.sequence() > local_head.sequence()
            || (finalized.sequence() == local_head.sequence() && finalized != local_head)
        {
            return Err(GovernancePublishError::other(
                "local privacy release checkpoint is behind or equivocates with the finalized anchor",
            ));
        }
        if finalized == local_head {
            return Ok(());
        }
        let published_cycles = self
            .published_privacy_aggregate_cycles
            .read()
            .map_err(|_| GovernancePublishError::other("privacy cycle index poisoned"))?
            .clone();
        let outbox = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
            .clone();
        validate_privacy_release_outbox(&release_ledger, &published_cycles, &outbox)?;
        while finalized.sequence() < local_head.sequence() {
            let next_index = usize::try_from(finalized.sequence()).map_err(|_| {
                GovernancePublishError::other("privacy release anchor sequence exceeds usize")
            })?;
            let record = release_ledger.records.get(next_index).ok_or_else(|| {
                GovernancePublishError::other(
                    "privacy release anchor is not a prefix of the local release chain",
                )
            })?;
            let expected = if next_index == 0 {
                PrivacyReleaseAnchorHeadV1::genesis(query_id)
            } else {
                PrivacyReleaseAnchorHeadV1::from_record(
                    &release_ledger.records[next_index.saturating_sub(1)],
                )
            };
            if expected != finalized {
                return Err(GovernancePublishError::other(
                    "privacy release anchor predecessor conflicts with the local release chain",
                ));
            }
            if record.status == transparency::PrivacyReleaseStatusV1::Published {
                let has_exact_pending_publication = outbox.entries.values().any(|entry| {
                    entry.kind == GovernanceOutboxKindV1::TransparencyLedgerPublication
                        && record.publication_payload_digest == Some(entry.payload_digest)
                });
                if !has_exact_pending_publication {
                    return Err(GovernancePublishError::other(
                        "unanchored privacy release lacks its exact durable publication outbox entry",
                    ));
                }
            }
            let next = PrivacyReleaseAnchorHeadV1::from_record(record);
            let window = PrivacyAggregateCycleWindow {
                cycle_start_unix: record.cycle_start_unix,
                cycle_end_unix: record.cycle_end_unix,
                due_at_unix: record.due_at_unix,
            };
            let scope = self.privacy_leader_lease_scope(query_id, window)?;
            let advance = |grant: &TransparencyLeaderLeaseGrantV1| {
                self.validate_privacy_leader_lease_for_use(scope, observed_at_unix, grant)?;
                anchor
                    .compare_and_set_finalized_head(finalized, next, grant)
                    .map_err(|error| {
                        GovernancePublishError::other(format!(
                            "advance finalized privacy release anchor: {error}"
                        ))
                    })?;
                let observed = anchor.finalized_head(query_id).map_err(|error| {
                    GovernancePublishError::other(format!(
                        "confirm finalized privacy release anchor: {error}"
                    ))
                })?;
                if observed != next {
                    return Err(GovernancePublishError::other(
                        "finalized privacy release anchor did not confirm the exact committed head",
                    ));
                }
                Ok(observed)
            };
            let observed =
                if let Some(grant) = retained_lease.filter(|grant| grant.scope() == scope) {
                    advance(grant)?
                } else {
                    self.with_privacy_leader_lease(query_id, window, observed_at_unix, advance)?
                };
            finalized = observed;
        }
        Ok(())
    }
    /// Return the config-backed privacy aggregate scheduler, when enabled.
    #[must_use]
    pub fn configured_privacy_aggregate_schedule(&self) -> Option<PrivacyAggregateScheduleConfig> {
        self.config.privacy_aggregate_schedule()
    }
    fn derive_privacy_cycle_prf_input(
        &self,
        config: &PrivacyAggregateCycleConfig,
        window: PrivacyAggregateCycleWindow,
    ) -> Result<Option<PrivacyCyclePrfInputV1>, GovernancePublishError> {
        if config.privacy.per_subject_metric_cap.is_none() {
            return Ok(None);
        }
        let request = PrivacyCyclePrfRequestV1::new(
            config.query_id,
            config.policy_digest,
            privacy_population_inventory_digest(&config.populations),
            privacy_metric_schema_digest(&config.metrics),
            window,
        )
        .map_err(|_| {
            GovernancePublishError::other("privacy cycle PRF request binding is invalid")
        })?;
        let provider = self.privacy_cycle_prf_provider.as_deref().ok_or_else(|| {
            GovernancePublishError::other(
                "runtime threshold PRF provider is unavailable for differential privacy",
            )
        })?;
        let output = provider
            .derive_cycle_output(&request)
            .map_err(|error| match error {
                PrivacyCyclePrfProviderErrorV1::Unavailable => {
                    GovernancePublishError::other("runtime threshold PRF provider is unavailable")
                }
                PrivacyCyclePrfProviderErrorV1::AuthenticationFailed => {
                    GovernancePublishError::other(
                        "runtime threshold PRF provider authentication failed",
                    )
                }
                PrivacyCyclePrfProviderErrorV1::RateLimited => {
                    GovernancePublishError::other("runtime threshold PRF provider rate limited")
                }
                PrivacyCyclePrfProviderErrorV1::Internal => {
                    GovernancePublishError::other("runtime threshold PRF provider internal failure")
                }
            })?;
        Ok(Some(PrivacyCyclePrfInputV1::new(request, output)))
    }
    /// Return the config-backed evidence-viewer audit scheduler, when enabled.
    #[must_use]
    pub fn configured_evidence_viewer_audit_schedule(
        &self,
    ) -> Option<PrivacyAggregateScheduleConfig> {
        self.config.evidence_viewer_audit_schedule()
    }
    /// Build and publish a transparency cycle from privacy-safe moderation aggregates.
    ///
    /// Aggregates are validated, required to fit inside the supplied cycle window, sorted
    /// deterministically by source window and aggregate id, then converted into `PrivacyAggregate`
    /// ledger entries before being published through the configured governance pipeline.
    #[cfg(test)]
    fn publish_privacy_aggregate_cycle(
        &self,
        cycle_id: [u8; 16],
        cycle_start_unix: u64,
        cycle_end_unix: u64,
        _generated_at_unix: u64,
        previous_block_hash: Option<[u8; 32]>,
        aggregates: Vec<ModerationPrivacyAggregateV1>,
    ) -> Result<ModerationLedgerCyclePublicationV1, GovernancePublishError> {
        let publication = Self::build_privacy_aggregate_publication(
            cycle_id,
            cycle_start_unix,
            cycle_end_unix,
            cycle_end_unix,
            previous_block_hash,
            aggregates,
        )?;
        let encoded = norito::to_bytes(&publication).map_err(|err| {
            GovernancePublishError::other(format!(
                "encode test-only privacy aggregate publication: {err}"
            ))
        })?;
        self.enqueue_governance_outbox(
            GovernanceOutboxKindV1::TransparencyLedgerPublication,
            encoded,
        )?;
        self.flush_governance_outbox()?;
        Ok(publication)
    }
    fn build_privacy_aggregate_publication(
        cycle_id: [u8; 16],
        cycle_start_unix: u64,
        cycle_end_unix: u64,
        generated_at_unix: u64,
        previous_block_hash: Option<[u8; 32]>,
        aggregates: Vec<ModerationPrivacyAggregateV1>,
    ) -> Result<ModerationLedgerCyclePublicationV1, GovernancePublishError> {
        if aggregates.is_empty() {
            return Err(GovernancePublishError::other(
                "privacy aggregate cycle requires at least one aggregate",
            ));
        }
        if generated_at_unix != cycle_end_unix {
            return Err(GovernancePublishError::other(
                "privacy aggregate publication timestamp must equal the exact cycle end",
            ));
        }
        let mut seen_aggregate_ids = BTreeSet::new();
        let mut cycle_policy_digest = None;
        let mut cycle_noise_source = None;
        let mut keyed = Vec::with_capacity(aggregates.len());
        for aggregate in aggregates {
            aggregate.validate().map_err(|err| {
                GovernancePublishError::other(format!("invalid privacy aggregate: {err}"))
            })?;
            if aggregate.window_start_unix != cycle_start_unix
                || aggregate.window_end_unix != cycle_end_unix
            {
                return Err(GovernancePublishError::other(format!(
                    "privacy aggregate `{}` window must equal the publication cycle",
                    aggregate.aggregate_id
                )));
            }
            if aggregate.generated_at_unix != cycle_end_unix {
                return Err(GovernancePublishError::other(format!(
                    "privacy aggregate `{}` generated_at timestamp must equal the exact cycle end",
                    aggregate.aggregate_id
                )));
            }
            if !seen_aggregate_ids.insert(aggregate.aggregate_id.clone()) {
                return Err(GovernancePublishError::other(format!(
                    "duplicate privacy aggregate id `{}` in cycle",
                    aggregate.aggregate_id
                )));
            }
            match cycle_policy_digest {
                Some(policy_digest) if policy_digest != aggregate.policy_digest => {
                    return Err(GovernancePublishError::other(
                        "privacy aggregate cycle contains mixed policy digests",
                    ));
                }
                None => cycle_policy_digest = Some(aggregate.policy_digest),
                Some(_) => {}
            }
            match cycle_noise_source {
                Some(noise_source) if noise_source != aggregate.noise_source => {
                    return Err(GovernancePublishError::other(
                        "privacy aggregate cycle contains mixed privacy noise sources",
                    ));
                }
                None => cycle_noise_source = Some(aggregate.noise_source),
                Some(_) => {}
            }
            let aggregate_hash = aggregate.aggregate_hash().map_err(|err| {
                GovernancePublishError::other(format!("hash privacy aggregate: {err}"))
            })?;
            keyed.push((
                aggregate.window_start_unix,
                aggregate.window_end_unix,
                aggregate.aggregate_id.clone(),
                aggregate_hash,
                aggregate,
            ));
        }
        keyed.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.cmp(&right.1))
                .then_with(|| left.2.cmp(&right.2))
                .then_with(|| left.3.cmp(&right.3))
        });
        let mut entries = Vec::with_capacity(keyed.len());
        for (index, (_, _, _, aggregate_hash, aggregate)) in keyed.iter().enumerate() {
            let sequence = u64::try_from(index)
                .map_err(|_| GovernancePublishError::other("privacy aggregate index overflow"))?
                .saturating_add(1);
            let entry_id =
                privacy_aggregate_entry_id(cycle_id, *aggregate_hash, &aggregate.aggregate_id);
            let entry = aggregate
                .to_ledger_entry(cycle_id, entry_id, sequence)
                .map_err(|err| {
                    GovernancePublishError::other(format!(
                        "convert privacy aggregate `{}` to ledger entry: {err}",
                        aggregate.aggregate_id
                    ))
                })?;
            entries.push(entry);
        }
        let ordered_aggregates = keyed
            .iter()
            .map(|(_, _, _, _, aggregate)| aggregate.clone())
            .collect::<Vec<_>>();
        let publication = ModerationLedgerCyclePublicationV1::from_entries_with_privacy_aggregates(
            cycle_id,
            cycle_start_unix,
            cycle_end_unix,
            generated_at_unix,
            previous_block_hash,
            &entries,
            &ordered_aggregates,
        )
        .map_err(|err| {
            GovernancePublishError::other(format!(
                "build privacy aggregate transparency publication: {err}"
            ))
        })?;
        Ok(publication)
    }
    /// Build and publish a privacy aggregate cycle from locally recorded source events.
    ///
    /// The worker filters recorded events to the requested cycle window, applies suppression/noise
    /// policy from `config`, builds aggregate payloads, and publishes the resulting transparency
    /// cycle through [`Self::publish_privacy_aggregate_cycle`].
    #[cfg(test)]
    fn publish_privacy_aggregate_cycle_from_source_events(
        &self,
        input: PrivacyAggregateSourceCycleInput,
    ) -> Result<ModerationLedgerCyclePublicationV1, GovernancePublishError> {
        let PrivacyAggregateSourceCycleInput {
            cycle_id,
            cycle_start_unix,
            cycle_end_unix,
            previous_block_hash,
            config,
            cycle_prf_input,
        } = input;
        let events = self
            .privacy_aggregate_source_events
            .read()
            .map_err(|_| GovernancePublishError::other("privacy aggregate event index poisoned"))?
            .values()
            .filter(|event| {
                event.occurred_at_unix >= cycle_start_unix
                    && event.occurred_at_unix < cycle_end_unix
            })
            .cloned()
            .collect::<Vec<_>>();
        let aggregates = transparency::build_privacy_aggregates_from_source_events(
            cycle_start_unix,
            cycle_end_unix,
            &config,
            cycle_prf_input,
            &events,
        )
        .map_err(|err| {
            GovernancePublishError::other(format!("build privacy aggregate cycle: {err}"))
        })?;
        self.publish_privacy_aggregate_cycle(
            cycle_id,
            cycle_start_unix,
            cycle_end_unix,
            cycle_end_unix,
            previous_block_hash,
            aggregates,
        )
    }
    /// Publish exactly one direct-successor privacy aggregate cycle.
    ///
    /// Window selection depends only on the governed activation/cadence and the durable release
    /// cursor. Source-event presence never affects which cycle is selected.
    #[cfg(test)]
    fn publish_due_privacy_aggregate_cycle_from_source_events(
        &self,
        now_unix: u64,
        expected_cycle_id: [u8; 16],
        idempotency_key: String,
        schedule: PrivacyAggregateScheduleConfig,
        config: PrivacyAggregateCycleConfig,
        composition_budget: Option<PrivacyCompositionBudgetPolicyV1>,
    ) -> Result<PrivacyAggregateScheduleOutcome, GovernancePublishError> {
        self.publish_due_privacy_aggregate_cycle_from_source_events_with_provenance(
            now_unix,
            expected_cycle_id,
            idempotency_key,
            schedule,
            config,
            composition_budget,
            None,
        )
    }
    #[expect(clippy::too_many_arguments, reason = "explicit audit inputs")]
    fn publish_due_privacy_aggregate_cycle_from_source_events_with_provenance(
        &self,
        now_unix: u64,
        expected_cycle_id: [u8; 16],
        idempotency_key: String,
        schedule: PrivacyAggregateScheduleConfig,
        config: PrivacyAggregateCycleConfig,
        composition_budget: Option<PrivacyCompositionBudgetPolicyV1>,
        provenance: Option<GovernanceSubmissionProvenanceV1>,
    ) -> Result<PrivacyAggregateScheduleOutcome, GovernancePublishError> {
        let _mutation_guard = self.runtime_mutation_lock.lock().map_err(|_| {
            GovernancePublishError::other("runtime publication transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        validate_privacy_publish_idempotency_key(&idempotency_key)?;
        schedule.validate().map_err(|err| {
            GovernancePublishError::other(format!("privacy aggregate schedule: {err}"))
        })?;
        config.validate().map_err(|err| {
            GovernancePublishError::other(format!("privacy aggregate policy: {err}"))
        })?;
        if schedule.first_cycle_start_unix != config.first_cycle_start_unix
            || schedule.cycle_seconds != config.cycle_seconds
        {
            return Err(GovernancePublishError::other(
                "privacy aggregate schedule conflicts with the governed query cadence",
            ));
        }
        let release_ledger = self
            .privacy_release_ledger
            .read()
            .map_err(|_| GovernancePublishError::other("privacy release ledger poisoned"))?
            .clone();
        release_ledger.validate().map_err(|error| {
            GovernancePublishError::other(format!("invalid privacy release ledger: {error}"))
        })?;
        validate_privacy_release_lineage(&release_ledger, schedule, &config)?;
        if self.privacy_release_anchor.is_none() {
            return Err(GovernancePublishError::other(
                "finalized privacy release anchor is unavailable",
            ));
        }
        self.reconcile_privacy_release_anchor_at(now_unix, None)?;
        self.flush_governance_outbox_at(now_unix, None)?;
        let request_digest =
            privacy_publish_request_digest(config.query_id, &idempotency_key, expected_cycle_id);
        let head = release_ledger.head(config.query_id).map_err(|error| {
            GovernancePublishError::other(format!("invalid privacy release head: {error}"))
        })?;
        let cycle_start_unix = release_ledger
            .records
            .last()
            .map_or(config.first_cycle_start_unix, |record| {
                record.cycle_end_unix
            });
        let cycle_end_unix = cycle_start_unix
            .checked_add(config.cycle_seconds)
            .ok_or_else(|| GovernancePublishError::other("privacy cycle window overflow"))?;
        let window = PrivacyAggregateCycleWindow {
            cycle_start_unix,
            cycle_end_unix,
            due_at_unix: cycle_end_unix
                .checked_add(schedule.publish_delay_seconds)
                .ok_or_else(|| {
                    GovernancePublishError::other("privacy cycle due timestamp overflow")
                })?,
        };
        let cycle_id = transparency::privacy_aggregate_cycle_id(
            config.query_id,
            window.cycle_start_unix,
            window.cycle_end_unix,
        );
        if let Some(receipt) = self
            .privacy_publish_request_receipts
            .read()
            .map_err(|_| {
                GovernancePublishError::other("privacy publish-request receipt index poisoned")
            })?
            .get(&idempotency_key)
            .cloned()
        {
            if receipt.request_digest != request_digest
                || receipt.cycle_id != expected_cycle_id
                || receipt.provenance != provenance
            {
                return Err(GovernancePublishError::other(
                    "privacy publish idempotency key equivocation",
                ));
            }
            let record = release_ledger
                .records
                .iter()
                .find(|record| record.release_id == receipt.cycle_id)
                .ok_or_else(|| {
                    GovernancePublishError::other(
                        "privacy publish-request receipt references an unknown terminal release",
                    )
                })?;
            validate_privacy_publish_receipt_release(&receipt, record)?;
            self.flush_governance_outbox_at(now_unix, None)?;
            return receipt.outcome();
        }
        if cycle_id != expected_cycle_id {
            return Err(GovernancePublishError::other(
                "privacy publish expected cycle does not match the direct successor",
            ));
        }
        let Some(latest_window) = schedule.due_window(now_unix).map_err(|err| {
            GovernancePublishError::other(format!("privacy aggregate schedule: {err}"))
        })?
        else {
            return Ok(PrivacyAggregateScheduleOutcome::NotDue);
        };
        if window.cycle_end_unix > latest_window.cycle_end_unix {
            return Ok(PrivacyAggregateScheduleOutcome::NotDue);
        }
        let published_cycles = self
            .published_privacy_aggregate_cycles
            .read()
            .map_err(|_| {
                GovernancePublishError::other("privacy aggregate published-cycle index poisoned")
            })?
            .clone();
        let state_limit = self.config.runtime_retention().state_entry_limit();
        if published_cycles.contains(&cycle_id) {
            return Err(GovernancePublishError::other(
                "privacy published-cycle guard is ahead of the durable release cursor",
            ));
        }
        if published_cycles.len() >= state_limit {
            return Err(GovernancePublishError::other(format!(
                "privacy aggregate published-cycle retention exhausted (limit {state_limit})"
            )));
        }
        self.with_privacy_leader_lease(config.query_id, window, now_unix, |leader_lease| {
            let events = self
                .privacy_aggregate_source_events
                .read()
                .map_err(|_| {
                    GovernancePublishError::other("privacy aggregate event index poisoned")
                })?
                .values()
                .filter(|event| {
                    event.occurred_at_unix >= window.cycle_start_unix
                        && event.occurred_at_unix < window.cycle_end_unix
                })
                .cloned()
                .collect::<Vec<_>>();
            let private_source_digest = transparency::canonical_private_source_digest(
                window.cycle_start_unix,
                window.cycle_end_unix,
                &config,
                &events,
            )
            .map_err(|error| {
                GovernancePublishError::other(format!(
                    "digest scheduled privacy aggregate source: {error}"
                ))
            })?;
            let cycle_prf_input = self.derive_privacy_cycle_prf_input(&config, window)?;
            let prf_evidence = cycle_prf_input.as_ref().map(|input| {
                (
                    input.request().binding_digest(),
                    input.commitment().commitment,
                )
            });
            let aggregates = match transparency::build_privacy_aggregates_from_source_events(
                window.cycle_start_unix,
                window.cycle_end_unix,
                &config,
                cycle_prf_input,
                &events,
            ) {
                Ok(aggregates) => aggregates,
                Err(
                    transparency::PrivacyAggregateWorkerError::AllBucketsSuppressed
                    | transparency::PrivacyAggregateWorkerError::NoSourceEvents,
                ) => {
                    let receipt = PrivacyPublishRequestReceiptV1 {
                        idempotency_key,
                        request_digest,
                        query_id: config.query_id,
                        requested_now_unix: now_unix,
                        cycle_id,
                        cycle_start_unix: window.cycle_start_unix,
                        cycle_end_unix: window.cycle_end_unix,
                        publish_delay_seconds: schedule.publish_delay_seconds,
                        due_at_unix: window.due_at_unix,
                        provenance: provenance.clone(),
                        outcome: PrivacyPublishRequestOutcomeV1::AllBucketsSuppressed,
                    };
                    self.commit_processed_privacy_cycle(
                        SuppressedPrivacyCycleCommitInput {
                            cycle_id,
                            window,
                            config: &config,
                            private_source_digest,
                            previous_publication_block_hash: head.latest_publication_block_hash(),
                            request_receipt: receipt,
                        },
                        leader_lease,
                        now_unix,
                    )?;
                    return Ok(PrivacyAggregateScheduleOutcome::AllBucketsSuppressed {
                        window,
                        cycle_id,
                    });
                }
                Err(err) => {
                    return Err(GovernancePublishError::other(format!(
                        "build scheduled privacy aggregate cycle: {err}"
                    )));
                }
            };
            let publication = Self::build_privacy_aggregate_publication(
                cycle_id,
                window.cycle_start_unix,
                window.cycle_end_unix,
                window.cycle_end_unix,
                head.latest_publication_block_hash(),
                aggregates,
            )?;
            let publication_bytes = norito::to_bytes(&publication).map_err(|err| {
                GovernancePublishError::other(format!(
                    "encode privacy publication idempotency receipt: {err}"
                ))
            })?;
            let receipt = PrivacyPublishRequestReceiptV1 {
                idempotency_key,
                request_digest,
                query_id: config.query_id,
                requested_now_unix: now_unix,
                cycle_id,
                cycle_start_unix: window.cycle_start_unix,
                cycle_end_unix: window.cycle_end_unix,
                publish_delay_seconds: schedule.publish_delay_seconds,
                due_at_unix: window.due_at_unix,
                provenance,
                outcome: PrivacyPublishRequestOutcomeV1::Published { publication_bytes },
            };
            self.commit_published_privacy_cycle(
                PublishedPrivacyCycleCommitInput {
                    window,
                    publication: &publication,
                    config: &config,
                    private_source_digest,
                    prf_evidence,
                    composition_budget,
                    request_receipt: receipt,
                },
                leader_lease,
                now_unix,
            )?;
            Ok(PrivacyAggregateScheduleOutcome::Published {
                window,
                publication,
            })
        })
    }
    fn commit_published_privacy_cycle(
        &self,
        input: PublishedPrivacyCycleCommitInput<'_>,
        leader_lease: &TransparencyLeaderLeaseGrantV1,
        observed_at_unix: u64,
    ) -> Result<(), GovernancePublishError> {
        let PublishedPrivacyCycleCommitInput {
            window,
            publication,
            config,
            private_source_digest,
            prf_evidence,
            composition_budget,
            request_receipt,
        } = input;
        publication.validate().map_err(|err| {
            GovernancePublishError::other(format!(
                "invalid scheduled privacy aggregate publication: {err}"
            ))
        })?;
        let cycle_id = publication.block.cycle_id;
        if publication.block.cycle_start_unix != window.cycle_start_unix
            || publication.block.cycle_end_unix != window.cycle_end_unix
            || publication.block.generated_at_unix != window.cycle_end_unix
            || cycle_id
                != transparency::privacy_aggregate_cycle_id(
                    config.query_id,
                    window.cycle_start_unix,
                    window.cycle_end_unix,
                )
        {
            return Err(GovernancePublishError::other(
                "scheduled privacy aggregate publication identity or window mismatch",
            ));
        }
        let encoded = norito::to_bytes(publication).map_err(|err| {
            GovernancePublishError::other(format!(
                "encode scheduled privacy aggregate publication: {err}"
            ))
        })?;
        request_receipt.validate()?;
        if request_receipt.cycle_id != cycle_id
            || request_receipt.query_id != config.query_id
            || request_receipt.cycle_start_unix != window.cycle_start_unix
            || request_receipt.cycle_end_unix != window.cycle_end_unix
            || request_receipt.due_at_unix != window.due_at_unix
            || !matches!(
                &request_receipt.outcome,
                PrivacyPublishRequestOutcomeV1::Published { publication_bytes }
                    if publication_bytes == &encoded
            )
        {
            return Err(GovernancePublishError::other(
                "privacy publish-request receipt does not match the publication",
            ));
        }
        let checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        let previous_budget = self
            .privacy_composition_budget
            .read()
            .map_err(|_| GovernancePublishError::other("privacy composition budget poisoned"))?
            .clone();
        let previous_release_ledger = self
            .privacy_release_ledger
            .read()
            .map_err(|_| GovernancePublishError::other("privacy release ledger poisoned"))?
            .clone();
        let previous_published = self
            .published_privacy_aggregate_cycles
            .read()
            .map_err(|_| {
                GovernancePublishError::other("privacy aggregate published-cycle index poisoned")
            })?
            .clone();
        let previous_events = self
            .privacy_aggregate_source_events
            .read()
            .map_err(|_| GovernancePublishError::other("privacy aggregate event index poisoned"))?
            .clone();
        let previous_request_receipts = self
            .privacy_publish_request_receipts
            .read()
            .map_err(|_| {
                GovernancePublishError::other("privacy publish-request receipt index poisoned")
            })?
            .clone();
        let previous_outbox = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
            .clone();
        let mut next_budget = previous_budget.clone();
        let budget_charge = match (
            config.privacy.epsilon_numerator,
            config.privacy.epsilon_denominator,
            composition_budget,
        ) {
            (Some(numerator), Some(denominator), Some(policy)) => Some(
                next_budget
                    .charge(
                        policy,
                        cycle_id,
                        publication.block.generated_at_unix,
                        numerator,
                        denominator,
                    )
                    .map_err(|err| {
                        GovernancePublishError::other(format!(
                            "charge privacy composition budget: {err}"
                        ))
                    })?,
            ),
            (None, None, _) => None,
            (Some(_), Some(_), None) => {
                return Err(GovernancePublishError::other(
                    "differential-privacy publication requires a governed composition budget",
                ));
            }
            _ => {
                return Err(GovernancePublishError::other(
                    "privacy epsilon numerator and denominator must be supplied together",
                ));
            }
        };
        if config.privacy.per_subject_metric_cap.is_some() != prf_evidence.is_some() {
            return Err(GovernancePublishError::other(
                "privacy release PRF evidence does not match the governed mode",
            ));
        }
        let publication_payload_digest = *blake3::hash(&encoded).as_bytes();
        let published_aggregate_inventory_digest =
            privacy_published_aggregate_inventory_digest(&publication.privacy_aggregates)?;
        let publication_block_hash = publication.block.block_hash().map_err(|err| {
            GovernancePublishError::other(format!("hash privacy publication block: {err}"))
        })?;
        let mut next_release_ledger = previous_release_ledger.clone();
        let (prf_request_binding, prf_commitment) = prf_evidence
            .map_or((None, None), |(binding, commitment)| {
                (Some(binding), Some(commitment))
            });
        next_release_ledger
            .append(transparency::PrivacyReleaseRecordV1 {
                sequence: 0,
                release_id: cycle_id,
                query_id: config.query_id,
                first_cycle_start_unix: config.first_cycle_start_unix,
                cycle_seconds: config.cycle_seconds,
                publish_delay_seconds: window
                    .due_at_unix
                    .checked_sub(window.cycle_end_unix)
                    .ok_or_else(|| {
                        GovernancePublishError::other(
                            "privacy publication due timestamp precedes its release window",
                        )
                    })?,
                cycle_start_unix: window.cycle_start_unix,
                cycle_end_unix: window.cycle_end_unix,
                due_at_unix: window.due_at_unix,
                private_source_digest,
                policy_digest: config.policy_digest,
                population_inventory_digest: privacy_population_inventory_digest(
                    &config.populations,
                ),
                metric_schema_digest: privacy_metric_schema_digest(&config.metrics),
                privacy: config.privacy,
                prf_request_binding,
                prf_commitment,
                budget_charge_digest: budget_charge.map(|charge| charge.charge_digest),
                publication_payload_digest: Some(publication_payload_digest),
                published_aggregate_inventory_digest: Some(published_aggregate_inventory_digest),
                previous_publication_block_hash: publication.block.previous_block_hash,
                publication_block_hash: Some(publication_block_hash),
                status: transparency::PrivacyReleaseStatusV1::Published,
                previous_record_digest: None,
                record_digest: [0; 32],
            })
            .map_err(|error| {
                GovernancePublishError::other(format!(
                    "append durable privacy release record: {error}"
                ))
            })?;
        let mut next_published = previous_published.clone();
        if !next_published.insert(cycle_id) {
            return Err(GovernancePublishError::other(
                "privacy aggregate cycle was already committed",
            ));
        }
        let mut next_events = previous_events.clone();
        next_events.retain(|_, event| {
            event.occurred_at_unix < window.cycle_start_unix
                || event.occurred_at_unix >= window.cycle_end_unix
        });
        let outbox_provenance = request_receipt.provenance.clone();
        let mut next_request_receipts = previous_request_receipts.clone();
        if next_request_receipts.len() >= self.config.runtime_retention().state_entry_limit()
            || next_request_receipts
                .insert(request_receipt.idempotency_key.clone(), request_receipt)
                .is_some()
        {
            return Err(GovernancePublishError::other(
                "privacy publish-request receipt retention exhausted or duplicated",
            ));
        }
        let gc_reserved_slots = {
            let intents = self
                .gc_eviction_intents
                .read()
                .map_err(|_| GovernancePublishError::other("GC eviction intent lock poisoned"))?;
            gc_eviction_reserved_outbox_slots(&intents)?
        };
        let mut next_outbox = previous_outbox.clone();
        insert_governance_outbox_entry(
            &mut next_outbox,
            GovernanceOutboxKindV1::TransparencyLedgerPublication,
            encoded,
            outbox_provenance,
            self.config.runtime_retention().state_entry_limit(),
            gc_reserved_slots,
        )?;
        self.install_privacy_publication_state(
            next_budget,
            next_release_ledger,
            next_published,
            next_events,
            next_request_receipts,
            next_outbox,
        )?;
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            if let Err(rollback_err) = self.install_privacy_publication_state(
                previous_budget,
                previous_release_ledger,
                previous_published,
                previous_events,
                previous_request_receipts,
                previous_outbox,
            ) {
                self.mark_durability_unhealthy(format!(
                    "privacy publication precommit failure `{err}` could not be rolled back: {rollback_err}"
                ));
                return Err(GovernancePublishError::other(format!(
                    "{err}; privacy publication rollback failed: {rollback_err}"
                )));
            }
            return Err(GovernancePublishError::other(err.to_string()));
        }
        drop(checkpoint_guard);
        self.reconcile_privacy_release_anchor_at(observed_at_unix, Some(leader_lease))?;
        self.flush_governance_outbox_at(observed_at_unix, Some(leader_lease))?;
        Ok(())
    }
    fn install_privacy_publication_state(
        &self,
        budget: PrivacyCompositionBudgetLedgerV1,
        release_ledger: transparency::PrivacyReleaseLedgerV1,
        published: BTreeSet<[u8; 16]>,
        events: BTreeMap<String, PrivacyAggregateSourceEvent>,
        request_receipts: BTreeMap<String, PrivacyPublishRequestReceiptV1>,
        outbox: GovernanceOutboxRuntime,
    ) -> Result<(), GovernancePublishError> {
        let mut budget_guard = self
            .privacy_composition_budget
            .write()
            .map_err(|_| GovernancePublishError::other("privacy composition budget poisoned"))?;
        let mut release_guard = self
            .privacy_release_ledger
            .write()
            .map_err(|_| GovernancePublishError::other("privacy release ledger poisoned"))?;
        let mut published_guard =
            self.published_privacy_aggregate_cycles
                .write()
                .map_err(|_| {
                    GovernancePublishError::other(
                        "privacy aggregate published-cycle index poisoned",
                    )
                })?;
        let mut events_guard = self
            .privacy_aggregate_source_events
            .write()
            .map_err(|_| GovernancePublishError::other("privacy aggregate event index poisoned"))?;
        let mut outbox_guard = self
            .governance_outbox
            .write()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?;
        let mut request_receipts_guard =
            self.privacy_publish_request_receipts.write().map_err(|_| {
                GovernancePublishError::other("privacy publish-request receipt index poisoned")
            })?;
        *budget_guard = budget;
        *release_guard = release_ledger;
        *published_guard = published;
        *events_guard = events;
        *request_receipts_guard = request_receipts;
        *outbox_guard = outbox;
        Ok(())
    }
    fn commit_processed_privacy_cycle(
        &self,
        input: SuppressedPrivacyCycleCommitInput<'_>,
        leader_lease: &TransparencyLeaderLeaseGrantV1,
        observed_at_unix: u64,
    ) -> Result<(), GovernancePublishError> {
        let SuppressedPrivacyCycleCommitInput {
            cycle_id,
            window,
            config,
            private_source_digest,
            previous_publication_block_hash,
            request_receipt,
        } = input;
        let checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        if config.privacy.per_subject_metric_cap.is_some()
            || cycle_id
                != transparency::privacy_aggregate_cycle_id(
                    config.query_id,
                    window.cycle_start_unix,
                    window.cycle_end_unix,
                )
        {
            return Err(GovernancePublishError::other(
                "only suppression-only releases may commit without a publication",
            ));
        }
        request_receipt.validate()?;
        if request_receipt.cycle_id != cycle_id
            || request_receipt.query_id != config.query_id
            || request_receipt.cycle_start_unix != window.cycle_start_unix
            || request_receipt.cycle_end_unix != window.cycle_end_unix
            || request_receipt.due_at_unix != window.due_at_unix
            || !matches!(
                &request_receipt.outcome,
                PrivacyPublishRequestOutcomeV1::AllBucketsSuppressed
            )
        {
            return Err(GovernancePublishError::other(
                "privacy publish-request receipt does not match the suppressed release",
            ));
        }
        let previous_budget = self
            .privacy_composition_budget
            .read()
            .map_err(|_| GovernancePublishError::other("privacy composition budget poisoned"))?
            .clone();
        let previous_release_ledger = self
            .privacy_release_ledger
            .read()
            .map_err(|_| GovernancePublishError::other("privacy release ledger poisoned"))?
            .clone();
        let previous_published = self
            .published_privacy_aggregate_cycles
            .read()
            .map_err(|_| {
                GovernancePublishError::other("privacy aggregate published-cycle index poisoned")
            })?
            .clone();
        let previous_events = self
            .privacy_aggregate_source_events
            .read()
            .map_err(|_| GovernancePublishError::other("privacy aggregate event index poisoned"))?
            .clone();
        let previous_request_receipts = self
            .privacy_publish_request_receipts
            .read()
            .map_err(|_| {
                GovernancePublishError::other("privacy publish-request receipt index poisoned")
            })?
            .clone();
        let previous_outbox = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
            .clone();
        let mut next_release_ledger = previous_release_ledger.clone();
        next_release_ledger
            .append(transparency::PrivacyReleaseRecordV1 {
                sequence: 0,
                release_id: cycle_id,
                query_id: config.query_id,
                first_cycle_start_unix: config.first_cycle_start_unix,
                cycle_seconds: config.cycle_seconds,
                publish_delay_seconds: window
                    .due_at_unix
                    .checked_sub(window.cycle_end_unix)
                    .ok_or_else(|| {
                        GovernancePublishError::other(
                            "privacy publication due timestamp precedes its release window",
                        )
                    })?,
                cycle_start_unix: window.cycle_start_unix,
                cycle_end_unix: window.cycle_end_unix,
                due_at_unix: window.due_at_unix,
                private_source_digest,
                policy_digest: config.policy_digest,
                population_inventory_digest: privacy_population_inventory_digest(
                    &config.populations,
                ),
                metric_schema_digest: privacy_metric_schema_digest(&config.metrics),
                privacy: config.privacy,
                prf_request_binding: None,
                prf_commitment: None,
                budget_charge_digest: None,
                publication_payload_digest: None,
                published_aggregate_inventory_digest: None,
                previous_publication_block_hash,
                publication_block_hash: None,
                status: transparency::PrivacyReleaseStatusV1::Suppressed,
                previous_record_digest: None,
                record_digest: [0; 32],
            })
            .map_err(|error| {
                GovernancePublishError::other(format!(
                    "append durable suppressed privacy release record: {error}"
                ))
            })?;
        let mut next_published = previous_published.clone();
        if !next_published.insert(cycle_id) {
            return Err(GovernancePublishError::other(
                "privacy aggregate cycle was already committed",
            ));
        }
        let mut next_events = previous_events.clone();
        next_events.retain(|_, event| {
            event.occurred_at_unix < window.cycle_start_unix
                || event.occurred_at_unix >= window.cycle_end_unix
        });
        let mut next_request_receipts = previous_request_receipts.clone();
        if next_request_receipts.len() >= self.config.runtime_retention().state_entry_limit()
            || next_request_receipts
                .insert(request_receipt.idempotency_key.clone(), request_receipt)
                .is_some()
        {
            return Err(GovernancePublishError::other(
                "privacy publish-request receipt retention exhausted or duplicated",
            ));
        }
        self.install_privacy_publication_state(
            previous_budget.clone(),
            next_release_ledger,
            next_published,
            next_events,
            next_request_receipts,
            previous_outbox.clone(),
        )?;
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            self.install_privacy_publication_state(
                previous_budget,
                previous_release_ledger,
                previous_published,
                previous_events,
                previous_request_receipts,
                previous_outbox,
            )?;
            return Err(GovernancePublishError::other(err.to_string()));
        }
        drop(checkpoint_guard);
        self.reconcile_privacy_release_anchor_at(observed_at_unix, Some(leader_lease))?;
        Ok(())
    }
    /// Publish the next due privacy aggregate cycle using storage configuration.
    ///
    /// The complete public policy and composition budget come from `iroha_config`; hidden cycle
    /// randomness comes exclusively from the runtime threshold-PRF provider. The predecessor and
    /// trusted evaluation time are derived inside the node; callers supply only the expected
    /// direct-successor cycle and a bounded idempotency key.
    pub fn publish_due_configured_privacy_aggregate_cycle_from_source_events(
        &self,
        expected_cycle_id: [u8; 16],
        idempotency_key: String,
    ) -> Result<PrivacyAggregateScheduleOutcome, GovernancePublishError> {
        self.publish_due_configured_privacy_aggregate_cycle(
            expected_cycle_id,
            idempotency_key,
            None,
        )
    }
    /// Publish the next due privacy cycle with server-verified Torii provenance.
    pub fn publish_due_configured_privacy_aggregate_cycle_from_authenticated_request(
        &self,
        expected_cycle_id: [u8; 16],
        idempotency_key: String,
        publisher_account: AccountId,
    ) -> Result<PrivacyAggregateScheduleOutcome, GovernancePublishError> {
        self.publish_due_configured_privacy_aggregate_cycle(
            expected_cycle_id,
            idempotency_key,
            Some(GovernanceSubmissionProvenanceV1::new(
                publisher_account,
                GovernanceSubmissionOriginV1::PrivacyAggregatePublishDue,
            )),
        )
    }
    fn publish_due_configured_privacy_aggregate_cycle(
        &self,
        expected_cycle_id: [u8; 16],
        idempotency_key: String,
        provenance: Option<GovernanceSubmissionProvenanceV1>,
    ) -> Result<PrivacyAggregateScheduleOutcome, GovernancePublishError> {
        let Some(schedule) = self.configured_privacy_aggregate_schedule() else {
            return Ok(PrivacyAggregateScheduleOutcome::Disabled);
        };
        let (config, composition_budget) = {
            let policy = self.config.privacy_aggregate_policy().ok_or_else(|| {
                GovernancePublishError::other(
                    "enabled privacy aggregate scheduler is missing its governed policy",
                )
            })?;
            (policy.cycle_config(), policy.composition_budget())
        };
        self.publish_due_privacy_aggregate_cycle_from_source_events_with_provenance(
            unix_now_secs(),
            expected_cycle_id,
            idempotency_key,
            schedule,
            config,
            Some(composition_budget),
            provenance,
        )
    }
    /// Publish an appeal-finance weekly rollup with verified Torii provenance.
    pub fn publish_authenticated_appeal_finance_weekly_rollup(
        &self,
        rollup: SoraFsAppealFinanceWeeklyRollupV1,
        publisher_account: AccountId,
    ) -> Result<(), GovernancePublishError> {
        self.publish_appeal_finance_weekly_rollup_with_provenance(
            rollup,
            GovernanceSubmissionProvenanceV1::new(
                publisher_account,
                GovernanceSubmissionOriginV1::AppealFinanceWeeklyRollup,
            ),
        )
    }
    fn publish_appeal_finance_weekly_rollup_with_provenance(
        &self,
        rollup: SoraFsAppealFinanceWeeklyRollupV1,
        provenance: GovernanceSubmissionProvenanceV1,
    ) -> Result<(), GovernancePublishError> {
        rollup.validate().map_err(|err| {
            GovernancePublishError::other(format!("invalid appeal finance weekly rollup: {err}"))
        })?;
        let encoded = norito::to_bytes(&rollup).map_err(|err| {
            GovernancePublishError::other(format!("encode appeal finance weekly rollup: {err}"))
        })?;
        self.enqueue_governance_outbox_with_optional_provenance(
            GovernanceOutboxKindV1::AppealFinanceWeeklyRollup,
            encoded,
            Some(provenance),
        )?;
        self.flush_governance_outbox()?;
        Ok(())
    }
    /// Publish a typed SoraFS appeal finance settlement receipt to the governance pipeline.
    pub fn publish_appeal_finance_settlement_receipt(
        &self,
        receipt: SoraFsAppealFinanceSettlementReceiptV1,
    ) -> Result<(), GovernancePublishError> {
        receipt.validate().map_err(|err| {
            GovernancePublishError::other(format!(
                "invalid appeal finance settlement receipt: {err}"
            ))
        })?;
        let encoded = norito::to_bytes(&receipt).map_err(|err| {
            GovernancePublishError::other(format!(
                "encode appeal finance settlement receipt: {err}"
            ))
        })?;
        self.enqueue_governance_outbox(
            GovernanceOutboxKindV1::AppealFinanceSettlementReceipt,
            encoded,
        )?;
        self.flush_governance_outbox()?;
        Ok(())
    }
    /// Return the immutable public trust policy loaded for reputation admission.
    ///
    /// The policy contains no signing material. Exposing the same `Arc` lets a supervised committed
    /// projector bind publication verification to the exact policy already admitted during node
    /// startup, avoiding a competing file read or divergent policy authority.
    #[must_use]
    pub fn reputation_trust_policy(&self) -> Option<Arc<ReputationSnapshotTrustPolicyV1>> {
        self.reputation_trust_policy.clone()
    }
    /// Return the immutable public trust policy loaded for signed hedging-feed admission.
    ///
    /// A supervised billing projector reuses this exact `Arc` so feed validation and billing period
    /// closure cannot observe divergent policy files.
    #[must_use]
    pub fn hedging_feed_trust_policy(&self) -> Option<Arc<HedgingFeedTrustPolicyV1>> {
        self.hedging_feed_trust_policy.clone()
    }
    /// Return the latest reputation snapshot accepted by this node.
    #[must_use]
    pub fn latest_reputation_snapshot(&self) -> Option<ReputationSnapshotV1> {
        self.latest_reputation_snapshot
            .read()
            .ok()
            .and_then(|guard| guard.clone())
    }
    /// Return a published reputation snapshot by snapshot identifier.
    #[must_use]
    pub fn reputation_snapshot(&self, snapshot_id: [u8; 16]) -> Option<ReputationSnapshotV1> {
        self.reputation_snapshots.read().ok().and_then(|guard| {
            guard
                .get(&snapshot_id)
                .map(|admitted| admitted.envelope.snapshot.clone())
        })
    }
    /// Return the latest full signed reputation envelope and scoring evidence.
    #[must_use]
    pub fn latest_signed_reputation_snapshot(&self) -> Option<SignedReputationSnapshotV1> {
        let snapshot_id = self
            .latest_reputation_snapshot
            .read()
            .ok()
            .and_then(|guard| guard.as_ref().map(|snapshot| snapshot.snapshot_id))?;
        self.signed_reputation_snapshot(snapshot_id)
    }
    /// Return a retained full signed reputation envelope by snapshot identifier.
    #[must_use]
    pub fn signed_reputation_snapshot(
        &self,
        snapshot_id: [u8; 16],
    ) -> Option<SignedReputationSnapshotV1> {
        self.reputation_snapshots.read().ok().and_then(|guard| {
            guard
                .get(&snapshot_id)
                .map(|admitted| admitted.envelope.clone())
        })
    }
    /// Return reputation snapshot events after `since_sequence`, capped by `limit`.
    #[must_use]
    pub fn reputation_events_since(
        &self,
        since_sequence: Option<u64>,
        limit: usize,
    ) -> Vec<ReputationSnapshotEventV1> {
        self.reputation_events_replay(since_sequence, limit).events
    }
    /// Return a gap-aware bounded replay of reputation snapshot events.
    #[must_use]
    pub fn reputation_events_replay(
        &self,
        since_sequence: Option<u64>,
        limit: usize,
    ) -> EventReplay<ReputationSnapshotEventV1> {
        self.reputation_events.read().map_or_else(
            |_| EventReplay {
                oldest_available_sequence: None,
                latest_sequence: None,
                gap: false,
                events: Vec::new(),
            },
            |guard| guard.replay(since_sequence, limit, |event| event.sequence),
        )
    }
    /// Return the latest reputation snapshot event sequence accepted by this node.
    #[must_use]
    pub fn latest_reputation_event_sequence(&self) -> Option<u64> {
        self.reputation_events
            .read()
            .ok()
            .and_then(|guard| (guard.latest_sequence != 0).then_some(guard.latest_sequence))
    }
    /// Subscribe to live reputation snapshot publication events.
    #[must_use]
    pub fn subscribe_reputation_events(&self) -> broadcast::Receiver<ReputationSnapshotEventV1> {
        self.reputation_event_sender.subscribe()
    }
    /// Admit a governance-signed moderation reproducibility manifest into the local registry.
    ///
    /// The manifest is validated with the canonical data-model validator before
    /// it is recorded. Re-admitting the same manifest id is idempotent when the
    /// recorded summary matches and fails closed when the summary conflicts.
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails, the registry lock is poisoned, or
    /// the manifest id conflicts with a previously admitted record.
    pub fn admit_moderation_repro_manifest(
        &self,
        manifest: ModerationReproManifestV1,
    ) -> Result<ModerationReproRegistryRecord, ModerationModelRegistryError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationModelRegistryError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationModelRegistryError::Checkpoint { message })?;
        let mut registry = self
            .moderation_model_registry
            .write()
            .map_err(|_| ModerationModelRegistryError::StateLockPoisoned)?;
        let previous = registry.snapshot();
        let record = registry.admit_repro_manifest(manifest)?;
        let committed = registry.snapshot();
        if let Err(err) = self.persist_moderation_model_registry_snapshot(&committed) {
            if err.committed {
                return Err(ModerationModelRegistryError::Checkpoint {
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = registry.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation model registry checkpoint failure",
                    rollback,
                );
                return Err(ModerationModelRegistryError::Checkpoint { message });
            }
            return Err(ModerationModelRegistryError::Checkpoint {
                message: err.to_string(),
            });
        }
        Ok(record)
    }
    /// Admit an adversarial corpus manifest into the local moderation registry.
    ///
    /// The corpus is validated and keyed by the BLAKE3 digest of its canonical
    /// Norito bytes. Re-admission of the same corpus digest is idempotent.
    ///
    /// # Errors
    ///
    /// Returns an error if validation, canonical encoding, or registry locking fails.
    pub fn admit_moderation_corpus_manifest(
        &self,
        manifest: AdversarialCorpusManifestV1,
    ) -> Result<ModerationCorpusRegistryRecord, ModerationModelRegistryError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationModelRegistryError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationModelRegistryError::Checkpoint { message })?;
        let mut registry = self
            .moderation_model_registry
            .write()
            .map_err(|_| ModerationModelRegistryError::StateLockPoisoned)?;
        let previous = registry.snapshot();
        let record = registry.admit_corpus_manifest(manifest)?;
        let committed = registry.snapshot();
        if let Err(err) = self.persist_moderation_model_registry_snapshot(&committed) {
            if err.committed {
                return Err(ModerationModelRegistryError::Checkpoint {
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = registry.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation model registry checkpoint failure",
                    rollback,
                );
                return Err(ModerationModelRegistryError::Checkpoint { message });
            }
            return Err(ModerationModelRegistryError::Checkpoint {
                message: err.to_string(),
            });
        }
        Ok(record)
    }
    /// Export a deterministic snapshot of the local moderation model registry.
    ///
    /// Lock poisoning is reported so callers cannot mistake an unavailable
    /// registry for an authoritative empty projection.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry lock is poisoned.
    pub fn export_moderation_model_registry_snapshot(
        &self,
    ) -> Result<ModerationModelRegistrySnapshot, ModerationModelRegistryError> {
        Ok(self
            .moderation_model_registry
            .read()
            .map_err(|_| ModerationModelRegistryError::StateLockPoisoned)?
            .snapshot())
    }
    /// Return a bounded deterministic read view of the local moderation model registry.
    ///
    /// Unlike snapshot export, this clones at most `limit` records from each
    /// registry so request memory is proportional to the admitted response.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry lock is poisoned.
    pub fn moderation_model_registry_read_view(
        &self,
        limit: usize,
    ) -> Result<ModerationModelRegistryReadView, ModerationModelRegistryError> {
        Ok(self
            .moderation_model_registry
            .read()
            .map_err(|_| ModerationModelRegistryError::StateLockPoisoned)?
            .read_view(limit))
    }
    /// Replace the local moderation model registry from a validated snapshot.
    ///
    /// The snapshot is duplicate-checked before it replaces local state, then it
    /// is persisted to the local checkpoint when SoraFS storage is enabled.
    ///
    /// # Errors
    ///
    /// Returns an error if the snapshot is internally inconsistent or the
    /// registry lock is poisoned.
    pub fn restore_moderation_model_registry_snapshot(
        &self,
        snapshot: ModerationModelRegistrySnapshot,
    ) -> Result<(), ModerationModelRegistryError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationModelRegistryError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationModelRegistryError::Checkpoint { message })?;
        let mut registry = self
            .moderation_model_registry
            .write()
            .map_err(|_| ModerationModelRegistryError::StateLockPoisoned)?;
        let previous = registry.snapshot();
        registry.restore_snapshot(snapshot)?;
        let committed = registry.snapshot();
        if let Err(err) = self.persist_moderation_model_registry_snapshot(&committed) {
            if err.committed {
                return Err(ModerationModelRegistryError::Checkpoint {
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = registry.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation model registry snapshot failure",
                    rollback,
                );
                return Err(ModerationModelRegistryError::Checkpoint { message });
            }
            return Err(ModerationModelRegistryError::Checkpoint {
                message: err.to_string(),
            });
        }
        Ok(())
    }
    /// Install or rotate the active externally anchored screening authority.
    ///
    /// Reinstalling the byte-identical authority is idempotent. Policies with an older issue
    /// timestamp, or a different digest at the same timestamp, are rejected to prevent in-process
    /// rollback/equivocation. Operators must reconstruct this non-secret authority from canonical
    /// `iroha_config` inputs after every restart; it is never accepted from an HTTP request.
    ///
    /// # Errors
    ///
    /// Returns an error if the authority lock is poisoned or the candidate
    /// policy rolls back/equivocates against the active policy.
    pub fn install_moderation_screening_authority(
        &self,
        authority: ModerationScreeningAuthorityV1,
    ) -> Result<(), ModerationScreeningAuthenticationError> {
        let mut active = self.moderation_screening_authority.write().map_err(|_| {
            ModerationScreeningAuthenticationError::AuthorityUnavailable {
                message: "active authority lock is poisoned".to_owned(),
            }
        })?;
        if let Some(current) = active.as_ref() {
            if authority.policy_issued_at_unix() < current.policy_issued_at_unix() {
                return Err(ModerationScreeningAuthenticationError::PolicyRollback {
                    active_issued_at_unix: current.policy_issued_at_unix(),
                    candidate_issued_at_unix: authority.policy_issued_at_unix(),
                });
            }
            if authority.policy_issued_at_unix() == current.policy_issued_at_unix()
                && authority.policy_digest() != current.policy_digest()
            {
                return Err(ModerationScreeningAuthenticationError::PolicyEquivocation {
                    issued_at_unix: authority.policy_issued_at_unix(),
                });
            }
            if current == &authority {
                return Ok(());
            }
        }
        *active = Some(authority);
        Ok(())
    }
    /// Return whether a validated config-authoritative screening bundle is active.
    #[must_use]
    pub fn has_moderation_screening_authority(&self) -> bool {
        self.moderation_screening_authority
            .read()
            .is_ok_and(|authority| authority.is_some())
    }
    /// Return whether authenticated moderation screening was enabled by the
    /// immutable node configuration used at startup.
    #[must_use]
    pub fn moderation_screening_enabled(&self) -> bool {
        self.config.moderation_screening_enabled()
    }
    /// Return the reviewed digest of the authority bundle loaded at startup.
    #[must_use]
    pub fn moderation_screening_authority_bundle_digest(&self) -> Option<[u8; 32]> {
        self.config.moderation_screening_authority_bundle_digest()
    }
    /// Return the active non-secret PKCS#11/KMS quarantine key handle.
    ///
    /// The wrapping provider and its credentials remain runtime-only.
    #[must_use]
    pub fn moderation_quarantine_key_id(&self) -> Option<&str> {
        self.moderation_quarantine_key_wrapper
            .as_deref()
            .map(ModerationQuarantineKeyWrapper::active_key_id)
    }
    /// Return whether the retained quarantine-key provider binding exactly
    /// matches the independently resolved storage configuration.
    ///
    /// The comparison is payload-free: it exposes only a boolean and never
    /// returns provider credentials, diagnostics, or private key material.
    #[must_use]
    pub fn matches_moderation_quarantine_key_provider_binding(
        &self,
        configured: Option<
            &iroha_config::parameters::actual::SorafsModerationQuarantineKeyProviderBinding,
        >,
    ) -> bool {
        match (
            self.moderation_quarantine_key_provider_binding.as_ref(),
            configured,
        ) {
            (None, None) => true,
            (Some(retained), Some(configured)) => {
                retained.provider_handle() == configured.handle.as_str()
                    && retained.expected_qualification()
                        == ModerationQuarantineKeyProviderQualificationV1::new(
                            configured.revision,
                            configured.policy_digest,
                        )
            }
            (None, Some(_)) | (Some(_), None) => false,
        }
    }
    /// Return whether `candidate` is the exact runtime wrapper retained by this node handle.
    #[must_use]
    pub fn uses_moderation_quarantine_key_wrapper(
        &self,
        candidate: &Arc<dyn ModerationQuarantineKeyWrapper>,
    ) -> bool {
        self.moderation_quarantine_key_wrapper
            .as_ref()
            .is_some_and(|active| Arc::ptr_eq(&active.0, candidate))
    }
    /// Return whether the configured privacy policy requires threshold-PRF cycle outputs.
    #[must_use]
    pub fn privacy_cycle_prf_required(&self) -> bool {
        self.config.privacy_aggregate_schedule().is_some()
            && self
                .config
                .privacy_aggregate_policy()
                .is_some_and(config::PrivacyAggregatePolicyConfig::requires_cycle_prf)
    }
    /// Return the exact threshold-PRF provider binding retained by this node.
    #[must_use]
    pub fn privacy_cycle_prf_provider_binding(
        &self,
    ) -> Option<&TransparencyRuntimeProviderBindingV1> {
        self.privacy_cycle_prf_provider
            .as_ref()
            .map(|active| active.0.binding())
    }
    /// Return the exact finalized release-anchor binding retained by this node.
    #[must_use]
    pub fn privacy_release_anchor_provider_binding(
        &self,
    ) -> Option<&TransparencyRuntimeProviderBindingV1> {
        self.privacy_release_anchor
            .as_ref()
            .map(|active| active.0.binding())
    }
    /// Return the exact external leader-lease binding retained by this node.
    #[must_use]
    pub fn transparency_leader_lease_provider_binding(
        &self,
    ) -> Option<&TransparencyRuntimeProviderBindingV1> {
        self.transparency_leader_lease_provider
            .as_ref()
            .map(|active| active.0.binding())
    }
    /// Return the exact sealed signed-producer checkpoint-store binding.
    #[must_use]
    pub fn governance_dag_checkpoint_store_binding(
        &self,
    ) -> Option<(&str, GovernanceDagRuntimeProviderQualificationV1)> {
        self.governance_dag_runtime_checkpoint_store
            .as_ref()
            .map(|store| (store.handle(), store.qualification()))
    }
    /// Revalidate the pinned signed Governance publisher and fused privacy target.
    ///
    /// Torii and other prebuilt-node launchers call this before starting any side-effectful worker.
    /// A configured Governance root must retain the exact startup-installed publisher, signer, and
    /// producer checkpoint store. Both providers are requalified and the sealed producer root is
    /// reconciled live. Enabled privacy publication additionally requires both retained target
    /// roles to match the exact configured handle, revision, and policy digest. The authenticated
    /// reader must prove that every persisted cache head and stable publication identity reaches
    /// the current target head. Disabled privacy rejects a configured or retained fused role.
    ///
    /// # Errors
    ///
    /// Returns [`GovernancePublishError`] when the configured/retained role set is incomplete or
    /// unexpected, the pinned publisher was removed or substituted, the signer, checkpoint store,
    /// or either privacy provider is unavailable, stale, revoked, substituted, or test-marked, the
    /// live bindings differ, the local producer root does not match its sealed checkpoint, or
    /// persisted target ancestry/inclusion cannot be authenticated.
    pub fn revalidate_fenced_privacy_runtime(&self) -> Result<(), GovernancePublishError> {
        match (
            self.config.governance_dir(),
            self.startup_governance_publisher.as_ref(),
            self.governance_publication_lock.as_ref(),
            self.governance_runtime_root.as_deref(),
            self.governance_runtime_writer_root_guard.as_ref(),
            self.governance_runtime_read_root_guard.as_ref(),
            self.governance_dag_runtime_signer.as_ref(),
            self.governance_dag_runtime_checkpoint_store.as_ref(),
        ) {
            (None, None, None, None, None, None, None, None) => {}
            (
                Some(_),
                Some(pinned),
                Some(publication_lock),
                Some(root),
                Some(writer_root_guard),
                Some(read_root_guard),
                Some(signer),
                Some(store),
            ) => {
                signer.assert_qualification()?;
                store.assert_qualification()?;
                let active = self.governance_publisher.read().map_err(|_| {
                    GovernancePublishError::other("governance publisher lock poisoned")
                })?;
                if active
                    .as_ref()
                    .is_none_or(|candidate| !Arc::ptr_eq(candidate, pinned))
                {
                    return Err(GovernancePublishError::other(
                        "active Governance publisher is not the exact startup-installed signed publisher",
                    ));
                }
                drop(active);
                let _publication_guard = publication_lock.lock().map_err(|_| {
                    GovernancePublishError::other(
                        "filesystem governance publisher transaction lock is poisoned",
                    )
                })?;
                writer_root_guard.revalidate()?;
                read_root_guard.revalidate()?;
                if writer_root_guard.root() != root
                    || read_root_guard.root() != root
                    || writer_root_guard.identity_digest()? != read_root_guard.identity_digest()?
                {
                    return Err(GovernancePublishError::other(
                        "writable and read-only Governance DAG root capabilities diverged",
                    ));
                }
                governance::revalidate_runtime_dag_producer_state(
                    root,
                    writer_root_guard,
                    signer,
                    store,
                )?;
                writer_root_guard.revalidate()?;
                read_root_guard.revalidate()?;
                signer.assert_qualification()?;
                store.assert_qualification()?;
            }
            _ => {
                return Err(GovernancePublishError::other(
                    "configured signed Governance publication did not retain its exact publisher, transaction fence, writer/reader root capabilities, runtime signer, and sealed producer checkpoint store",
                ));
            }
        }
        let required = self.config.privacy_aggregate_schedule().is_some()
            && self.config.privacy_aggregate_policy().is_some();
        let configured = self.config.privacy_fenced_publisher_binding();
        match (
            required,
            configured,
            self.fenced_transparency_publisher.as_ref(),
            self.fenced_transparency_head_reader.as_ref(),
        ) {
            (false, None, None, None) => Ok(()),
            (false, _, _, _) => Err(GovernancePublishError::other(
                "disabled privacy publication retains an unexpected fused target binding or role",
            )),
            (true, None, _, _) => Err(GovernancePublishError::other(
                "enabled privacy publication has no configured fused target binding",
            )),
            (true, Some(_), None, _) => Err(GovernancePublishError::other(
                "enabled privacy publication did not retain its fused target writer",
            )),
            (true, Some(_), Some(_), None) => Err(GovernancePublishError::other(
                "enabled privacy publication did not retain its authenticated head reader",
            )),
            (true, Some(binding), Some(publisher), Some(reader)) => {
                let qualification = binding.qualification();
                let expected = GovernanceDagRuntimeProviderQualificationV1::new(
                    qualification.revision(),
                    qualification.policy_digest(),
                );
                if publisher.handle() != binding.handle()
                    || reader.handle() != binding.handle()
                    || publisher.qualification() != expected
                    || reader.qualification() != expected
                {
                    return Err(GovernancePublishError::other(
                        "retained fused privacy runtime does not match the exact configured binding",
                    ));
                }
                publisher.assert_qualification()?;
                reader.assert_qualification()?;
                governance::ensure_fenced_privacy_runtime_bindings_match(publisher, reader)?;
                let root = self.governance_runtime_root.as_deref().ok_or_else(|| {
                    GovernancePublishError::other(
                        "enabled privacy publication has no signed Governance root",
                    )
                })?;
                let publication_lock =
                    self.governance_publication_lock.as_ref().ok_or_else(|| {
                        GovernancePublishError::other(
                            "enabled privacy publication did not retain its transaction fence",
                        )
                    })?;
                let _publication_guard = publication_lock.lock().map_err(|_| {
                    GovernancePublishError::other(
                        "filesystem governance publisher transaction lock is poisoned",
                    )
                })?;
                governance::synchronize_fenced_privacy_authoritative_head(root, reader, None)?;
                if let Some(signer) = self.governance_dag_runtime_signer.as_ref() {
                    signer.assert_qualification()?;
                }
                Ok(())
            }
        }
    }
    /// Return the highest external fencing token durably accepted in memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the qualified provider state cannot be read.
    pub fn transparency_leader_lease_fencing_floor(
        &self,
    ) -> Result<Option<u64>, TransparencyLeaderLeaseErrorV1> {
        self.transparency_leader_lease_provider
            .as_ref()
            .map(|active| active.0.fencing_floor())
            .transpose()
    }
    /// Authenticate and durably record one governed moderation screening result.
    ///
    /// The result must be either a canonical runner-signed result under a single-signer policy or
    /// an exact committee aggregate reconstructed from its bounded signed member inventory. The
    /// durable snapshot atomically binds the idempotency key, authenticated authority digest, and
    /// resulting screening record so replay under a different key fails closed.
    ///
    /// # Errors
    ///
    /// Returns an error if governance signatures, runner authorization, subject/evidence bindings,
    /// score/verdict consistency, freshness, revocation, committee uniqueness/quorum, or replay
    /// validation fails, or if the authenticated projection cannot be durably committed.
    pub fn record_authenticated_moderation_screening_result(
        &self,
        request: ModerationAuthenticatedScreeningRequestV1,
        now_unix: u64,
    ) -> Result<
        ModerationAuthenticatedScreeningOutcomeV1,
        ModerationAuthenticatedScreeningAdmissionError,
    > {
        let authority = self
            .moderation_screening_authority
            .read()
            .map_err(
                |_| ModerationScreeningAuthenticationError::AuthorityUnavailable {
                    message: "active authority lock is poisoned".to_owned(),
                },
            )?
            .clone()
            .ok_or_else(
                || ModerationScreeningAuthenticationError::AuthorityUnavailable {
                    message: "no active governed manifest/policy/anchor bundle is installed"
                        .to_owned(),
                },
            )?;
        let verified = authority.verify(request, now_unix)?;
        self.commit_authenticated_moderation_screening(verified)
    }
    fn commit_authenticated_moderation_screening(
        &self,
        verified: moderation::ModerationVerifiedScreeningAdmissionV1,
    ) -> Result<
        ModerationAuthenticatedScreeningOutcomeV1,
        ModerationAuthenticatedScreeningAdmissionError,
    > {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationScreeningError::Checkpoint { message })?;
        let mut runtime = self
            .moderation_screening
            .write()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?;
        let previous = runtime.snapshot();
        let outcome = runtime.record_authenticated_screening(verified)?;
        let committed = runtime.snapshot();
        if let Err(err) = self.persist_moderation_screening_snapshot(&committed) {
            if err.committed {
                return Err(ModerationScreeningError::Checkpoint {
                    message: err.to_string(),
                }
                .into());
            }
            if let Err(rollback) = runtime.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back authenticated moderation screening checkpoint failure",
                    rollback,
                );
                return Err(ModerationScreeningError::Checkpoint { message }.into());
            }
            return Err(ModerationScreeningError::Checkpoint {
                message: err.to_string(),
            }
            .into());
        }
        Ok(outcome)
    }
    /// Record one deterministic local moderation screening projection.
    ///
    /// This unsigned projection hook exists for tests and development tooling. Production request
    /// handlers must use [`Self::record_authenticated_moderation_screening_result`].
    ///
    /// `quarantine` and `escalate` verdicts also create a pending quarantine queue record.
    /// Successful updates are persisted to the local checkpoint when SoraFS storage is enabled.
    ///
    /// # Errors
    ///
    /// Returns an error if the input is invalid, the screening id conflicts
    /// with existing local state, or the runtime lock is poisoned.
    #[cfg(test)]
    fn record_moderation_screening_result(
        &self,
        input: ModerationScreeningInput,
    ) -> Result<ModerationScreeningOutcome, ModerationScreeningError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationScreeningError::Checkpoint { message })?;
        let mut runtime = self
            .moderation_screening
            .write()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?;
        let previous = runtime.snapshot();
        let outcome = runtime.record_screening(input)?;
        let committed = runtime.snapshot();
        if let Err(err) = self.persist_moderation_screening_snapshot(&committed) {
            if err.committed {
                return Err(ModerationScreeningError::Checkpoint {
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = runtime.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation screening checkpoint failure",
                    rollback,
                );
                return Err(ModerationScreeningError::Checkpoint { message });
            }
            return Err(ModerationScreeningError::Checkpoint {
                message: err.to_string(),
            });
        }
        Ok(outcome)
    }
    /// Mark a pending local quarantine record as reviewed.
    ///
    /// Successful updates are persisted to the local checkpoint when SoraFS storage is enabled.
    ///
    /// # Errors
    ///
    /// Returns an error if the quarantine id is unknown, the transition is
    /// invalid, or the runtime lock is poisoned.
    pub fn review_moderation_quarantine_record(
        &self,
        input: ModerationQuarantineReviewInput,
    ) -> Result<ModerationQuarantineRecord, ModerationScreeningError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationScreeningError::Checkpoint { message })?;
        let mut runtime = self
            .moderation_screening
            .write()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?;
        let previous = runtime.snapshot();
        let record = runtime.review_quarantine(input)?;
        let committed = runtime.snapshot();
        if let Err(err) = self.persist_moderation_screening_snapshot(&committed) {
            if err.committed {
                return Err(ModerationScreeningError::Checkpoint {
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = runtime.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation quarantine review checkpoint failure",
                    rollback,
                );
                return Err(ModerationScreeningError::Checkpoint { message });
            }
            return Err(ModerationScreeningError::Checkpoint {
                message: err.to_string(),
            });
        }
        Ok(record)
    }
    /// Release a reviewed local quarantine record.
    ///
    /// Successful updates are persisted to the local checkpoint when SoraFS storage is enabled.
    ///
    /// # Errors
    ///
    /// Returns an error if the quarantine id is unknown, the record has not been reviewed, the
    /// transition is invalid, or the runtime lock is poisoned.
    pub fn release_moderation_quarantine_record(
        &self,
        input: ModerationQuarantineReleaseInput,
    ) -> Result<ModerationQuarantineRecord, ModerationScreeningError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationScreeningError::Checkpoint { message })?;
        let mut runtime = self
            .moderation_screening
            .write()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?;
        let previous = runtime.snapshot();
        let record = runtime.release_quarantine(input)?;
        let committed = runtime.snapshot();
        if let Err(err) = self.persist_moderation_screening_snapshot(&committed) {
            if err.committed {
                return Err(ModerationScreeningError::Checkpoint {
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = runtime.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation quarantine release checkpoint failure",
                    rollback,
                );
                return Err(ModerationScreeningError::Checkpoint { message });
            }
            return Err(ModerationScreeningError::Checkpoint {
                message: err.to_string(),
            });
        }
        Ok(record)
    }
    /// Export local screening and quarantine state, reporting lock failures.
    ///
    /// # Errors
    ///
    /// Returns an error if the runtime lock is poisoned.
    pub fn export_moderation_screening_snapshot(
        &self,
    ) -> Result<ModerationScreeningSnapshot, ModerationScreeningError> {
        Ok(self
            .moderation_screening
            .read()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?
            .snapshot())
    }
    /// Return a bounded deterministic read view of local screening state.
    ///
    /// # Errors
    ///
    /// Returns an error if the screening runtime lock is poisoned.
    pub fn moderation_screening_read_view(
        &self,
        limit: usize,
    ) -> Result<ModerationScreeningReadView, ModerationScreeningError> {
        Ok(self
            .moderation_screening
            .read()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?
            .read_view(limit))
    }
    /// Return a bounded deterministic read view of the local quarantine queue.
    ///
    /// # Errors
    ///
    /// Returns an error if the screening runtime lock is poisoned.
    pub fn moderation_quarantine_read_view(
        &self,
        limit: usize,
    ) -> Result<ModerationQuarantineReadView, ModerationScreeningError> {
        Ok(self
            .moderation_screening
            .read()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?
            .quarantine_read_view(limit))
    }
    /// Look up one local quarantine record without cloning the full screening snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error if the screening runtime lock is poisoned.
    pub fn moderation_quarantine_record(
        &self,
        quarantine_id: &[u8; 16],
    ) -> Result<Option<ModerationQuarantineRecord>, ModerationScreeningError> {
        Ok(self
            .moderation_screening
            .read()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?
            .quarantine_record(quarantine_id))
    }
    /// Replace local screening/quarantine state from a validated snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error if the snapshot is internally inconsistent or the runtime lock is poisoned.
    pub fn restore_moderation_screening_snapshot(
        &self,
        snapshot: ModerationScreeningSnapshot,
    ) -> Result<(), ModerationScreeningError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationScreeningError::Checkpoint { message })?;
        self.validate_moderation_screening_snapshot_downstream_refs(&snapshot)?;
        let mut runtime = self
            .moderation_screening
            .write()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?;
        let previous = runtime.snapshot();
        runtime.restore_snapshot(snapshot)?;
        let committed = runtime.snapshot();
        if let Err(err) = self.persist_moderation_screening_snapshot(&committed) {
            if err.committed {
                return Err(ModerationScreeningError::Checkpoint {
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = runtime.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation screening snapshot failure",
                    rollback,
                );
                return Err(ModerationScreeningError::Checkpoint { message });
            }
            return Err(ModerationScreeningError::Checkpoint {
                message: err.to_string(),
            });
        }
        Ok(())
    }
    /// Seal and store quarantined payload bytes in the local encrypted object store.
    ///
    /// The plaintext BLAKE3 digest must match the referenced quarantine record
    /// subject digest. Successful writes persist an encrypted Norito envelope
    /// and update the local object index checkpoint.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is disabled, the quarantine id is unknown,
    /// the payload digest does not match the quarantine record, encryption or
    /// filesystem persistence fails, or the object index lock is poisoned.
    pub fn store_moderation_quarantine_object(
        &self,
        input: ModerationQuarantineObjectInput,
    ) -> Result<ModerationQuarantineObjectRecord, ModerationQuarantineObjectError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?;
        self.ensure_durability_healthy().map_err(|message| {
            ModerationQuarantineObjectError::Io {
                path: "durability-state".to_owned(),
                message,
            }
        })?;
        let root = self
            .moderation_quarantine_object_root
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::StorageDisabled)?;
        let input = normalize_moderation_quarantine_object_input(input)?;
        let quarantine = self.moderation_quarantine_record_for_object(&input.quarantine_id)?;
        let payload_digest = *blake3::hash(&input.payload).as_bytes();
        if payload_digest != quarantine.subject_digest {
            return Err(ModerationQuarantineObjectError::DigestMismatch {
                quarantine_id_hex: hex::encode(input.quarantine_id),
                expected_digest_hex: hex::encode(quarantine.subject_digest),
                actual_digest_hex: hex::encode(payload_digest),
            });
        }
        let key_wrapper = self
            .moderation_quarantine_key_wrapper
            .as_deref()
            .ok_or(ModerationQuarantineObjectError::KeyWrapperUnavailable)?;
        let key_provider_binding = self
            .moderation_quarantine_key_provider_binding
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::KeyWrapperUnqualified)?;
        let mut objects = self
            .moderation_quarantine_objects
            .write()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?;
        let previous = objects.snapshot();
        if let Some(existing) = objects.get(&input.quarantine_id) {
            let (_, existing_envelope, _) =
                self.read_moderation_quarantine_object_envelope(root, &existing)?;
            let plaintext = open_moderation_quarantine_object(
                &existing_envelope,
                &existing,
                key_provider_binding,
                key_wrapper,
            )?;
            if existing.payload_digest != payload_digest
                || existing.captured_at_unix != input.captured_at_unix
                || existing.content_type.as_deref() != input.content_type.as_deref()
                || existing.notes.as_deref() != input.notes.as_deref()
                || plaintext.as_slice() != input.payload.as_slice()
            {
                return Err(ModerationQuarantineObjectError::ConflictingObject {
                    quarantine_id_hex: hex::encode(input.quarantine_id),
                });
            }
            return Ok(existing);
        }
        objects.ensure_insert_capacity(&input.quarantine_id)?;
        let (record, envelope_bytes) =
            seal_moderation_quarantine_object(input, key_provider_binding, key_wrapper)?;
        let envelope_path = self.resolve_moderation_quarantine_object_path(root, &record)?;
        match fs::symlink_metadata(&envelope_path) {
            Ok(_) => {
                return Err(ModerationQuarantineObjectError::ConflictingObject {
                    quarantine_id_hex: hex::encode(record.quarantine_id),
                });
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => {
                return Err(ModerationQuarantineObjectError::Io {
                    path: envelope_path.display().to_string(),
                    message: err.to_string(),
                });
            }
        }
        self.finish_local_checkpoint_write(
            "moderation quarantine object envelope",
            &envelope_path,
            write_local_checkpoint_atomic_bounded(
                &envelope_path,
                &envelope_bytes,
                self.config.runtime_retention().checkpoint_max_bytes(),
            ),
        )
        .map_err(|err| ModerationQuarantineObjectError::Io {
            path: envelope_path.display().to_string(),
            message: err.to_string(),
        })?;
        let stored = match objects.insert(record) {
            Ok(stored) => stored,
            Err(err) => {
                if let Err(cleanup) = remove_local_checkpoint_file_durably(&envelope_path) {
                    let message = format!(
                        "failed to remove quarantine envelope after rejected index insertion: {cleanup}"
                    );
                    self.mark_durability_unhealthy(message.clone());
                    return Err(ModerationQuarantineObjectError::Io {
                        path: envelope_path.display().to_string(),
                        message,
                    });
                }
                return Err(err);
            }
        };
        let committed = objects.snapshot();
        if let Err(err) = self.persist_moderation_quarantine_object_index_snapshot(&committed) {
            if err.committed {
                return Err(ModerationQuarantineObjectError::Io {
                    path: "durability-state".to_owned(),
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = objects.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation quarantine object index checkpoint failure",
                    rollback,
                );
                return Err(ModerationQuarantineObjectError::Io {
                    path: "durability-state".to_owned(),
                    message,
                });
            }
            if let Err(cleanup) = remove_local_checkpoint_file_durably(&envelope_path) {
                let message = self.record_unrecoverable_rollback(
                    "failed to remove quarantine envelope after index checkpoint failure",
                    cleanup,
                );
                return Err(ModerationQuarantineObjectError::Io {
                    path: envelope_path.display().to_string(),
                    message,
                });
            }
            return Err(ModerationQuarantineObjectError::Io {
                path: "durability-state".to_owned(),
                message: err.to_string(),
            });
        }
        Ok(stored)
    }
    /// Read and decrypt a local quarantine payload object.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is disabled, the quarantine/object record is
    /// missing, the envelope cannot be read or decoded, authentication fails,
    /// or the decrypted payload no longer matches the quarantine record digest.
    pub fn read_moderation_quarantine_object(
        &self,
        quarantine_id: [u8; 16],
    ) -> Result<ModerationQuarantineObjectPayload, ModerationQuarantineObjectError> {
        let root = self
            .moderation_quarantine_object_root
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::StorageDisabled)?;
        let key_wrapper = self
            .moderation_quarantine_key_wrapper
            .as_deref()
            .ok_or(ModerationQuarantineObjectError::KeyWrapperUnavailable)?;
        let key_provider_binding = self
            .moderation_quarantine_key_provider_binding
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::KeyWrapperUnqualified)?;
        let quarantine = self.moderation_quarantine_record_for_object(&quarantine_id)?;
        let record = self
            .moderation_quarantine_objects
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?
            .get(&quarantine_id)
            .ok_or_else(|| ModerationQuarantineObjectError::MissingObject {
                quarantine_id_hex: hex::encode(quarantine_id),
            })?;
        let (_, envelope, _) = self.read_moderation_quarantine_object_envelope(root, &record)?;
        let payload = open_moderation_quarantine_object(
            &envelope,
            &record,
            key_provider_binding,
            key_wrapper,
        )?;
        if *blake3::hash(&payload).as_bytes() != quarantine.subject_digest {
            return Err(ModerationQuarantineObjectError::AuthenticationFailed {
                quarantine_id_hex: hex::encode(quarantine_id),
            });
        }
        Ok(ModerationQuarantineObjectPayload { record, payload })
    }
    /// Read and authenticate an inclusive-exclusive plaintext byte range.
    ///
    /// Only ciphertext chunks intersecting `start..end` are decrypted. Each
    /// returned byte is independently authenticated against the immutable
    /// object metadata, chunk index, offset, and length.
    ///
    /// # Errors
    ///
    /// Returns an error if the range is invalid, storage or the runtime
    /// PKCS#11/KMS wrapper is unavailable or unqualified, the object is
    /// missing, or any envelope/chunk authentication check fails.
    pub fn read_moderation_quarantine_object_range(
        &self,
        quarantine_id: [u8; 16],
        start: u64,
        end: u64,
    ) -> Result<ModerationQuarantineObjectRangePayload, ModerationQuarantineObjectError> {
        let root = self
            .moderation_quarantine_object_root
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::StorageDisabled)?;
        let key_wrapper = self
            .moderation_quarantine_key_wrapper
            .as_deref()
            .ok_or(ModerationQuarantineObjectError::KeyWrapperUnavailable)?;
        let key_provider_binding = self
            .moderation_quarantine_key_provider_binding
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::KeyWrapperUnqualified)?;
        self.moderation_quarantine_record_for_object(&quarantine_id)?;
        let record = self
            .moderation_quarantine_objects
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?
            .get(&quarantine_id)
            .ok_or_else(|| ModerationQuarantineObjectError::MissingObject {
                quarantine_id_hex: hex::encode(quarantine_id),
            })?;
        let (_, envelope, _) = self.read_moderation_quarantine_object_envelope(root, &record)?;
        let payload = open_moderation_quarantine_object_range(
            &envelope,
            &record,
            key_provider_binding,
            key_wrapper,
            start..end,
        )?;
        Ok(ModerationQuarantineObjectRangePayload {
            record,
            start,
            end,
            payload,
        })
    }
    /// Rewrap one object's DEK under the wrapper's current active key.
    ///
    /// The injected wrapper must remain able to unwrap the historical key handle stored in the
    /// object envelope. Ciphertext chunks, object id, and the durable index stay byte-identical;
    /// only the context-bound wrapped DEK and its non-secret key handle are atomically replaced.
    ///
    /// # Errors
    ///
    /// Returns an error if storage or the runtime PKCS#11/KMS wrapper is unavailable or
    /// unqualified, the object is missing, old/new key operations fail, the replacement cannot be
    /// authenticated, or the atomic write fails.
    pub fn rewrap_moderation_quarantine_object_dek(
        &self,
        quarantine_id: [u8; 16],
    ) -> Result<ModerationQuarantineObjectRecord, ModerationQuarantineObjectError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?;
        self.ensure_durability_healthy().map_err(|message| {
            ModerationQuarantineObjectError::Io {
                path: "durability-state".to_owned(),
                message,
            }
        })?;
        let root = self
            .moderation_quarantine_object_root
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::StorageDisabled)?;
        let key_wrapper = self
            .moderation_quarantine_key_wrapper
            .as_deref()
            .ok_or(ModerationQuarantineObjectError::KeyWrapperUnavailable)?;
        let key_provider_binding = self
            .moderation_quarantine_key_provider_binding
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::KeyWrapperUnqualified)?;
        self.moderation_quarantine_record_for_object(&quarantine_id)?;
        let record = self
            .moderation_quarantine_objects
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?
            .get(&quarantine_id)
            .ok_or_else(|| ModerationQuarantineObjectError::MissingObject {
                quarantine_id_hex: hex::encode(quarantine_id),
            })?;
        let (envelope_path, envelope, original_bytes) =
            self.read_moderation_quarantine_object_envelope(root, &record)?;
        let (replacement_record, replacement_bytes) = rewrap_moderation_quarantine_object(
            &envelope,
            &record,
            key_provider_binding,
            key_wrapper,
            key_provider_binding,
            key_wrapper,
        )?;
        if replacement_record != record {
            return Err(ModerationQuarantineObjectError::InvalidSnapshot {
                message: "DEK rewrap changed immutable object index metadata".to_owned(),
            });
        }
        if replacement_bytes == original_bytes {
            return Ok(record);
        }
        let replacement_envelope =
            norito::decode_from_bytes::<ModerationQuarantineObjectEnvelopeV1>(&replacement_bytes)
                .map_err(|error| ModerationQuarantineObjectError::Codec {
                message: error.to_string(),
            })?;
        open_moderation_quarantine_object(
            &replacement_envelope,
            &replacement_record,
            key_provider_binding,
            key_wrapper,
        )?;
        self.finish_local_checkpoint_write(
            "moderation quarantine object DEK rewrap",
            &envelope_path,
            write_local_checkpoint_atomic_bounded(
                &envelope_path,
                &replacement_bytes,
                self.config.runtime_retention().checkpoint_max_bytes(),
            ),
        )
        .map_err(|error| ModerationQuarantineObjectError::Io {
            path: envelope_path.display().to_string(),
            message: error.to_string(),
        })?;
        Ok(record)
    }
    /// Export local quarantine object index state, reporting lock failures.
    ///
    /// # Errors
    ///
    /// Returns an error if the object index lock is poisoned.
    pub fn export_moderation_quarantine_object_snapshot(
        &self,
    ) -> Result<ModerationQuarantineObjectSnapshot, ModerationQuarantineObjectError> {
        Ok(self
            .moderation_quarantine_objects
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?
            .snapshot())
    }
    /// Look up one quarantine object index record without cloning the full index.
    ///
    /// # Errors
    ///
    /// Returns an error if the object index lock is poisoned.
    pub fn moderation_quarantine_object_record(
        &self,
        quarantine_id: &[u8; 16],
    ) -> Result<Option<ModerationQuarantineObjectRecord>, ModerationQuarantineObjectError> {
        Ok(self
            .moderation_quarantine_objects
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?
            .get(quarantine_id))
    }
    /// Replace the local quarantine object index from a validated snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error if the snapshot is internally inconsistent, references
    /// an unknown quarantine id, or the object index lock is poisoned.
    pub fn restore_moderation_quarantine_object_snapshot(
        &self,
        snapshot: ModerationQuarantineObjectSnapshot,
    ) -> Result<(), ModerationQuarantineObjectError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?;
        self.ensure_durability_healthy().map_err(|message| {
            ModerationQuarantineObjectError::Io {
                path: "durability-state".to_owned(),
                message,
            }
        })?;
        self.moderation_quarantine_objects
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?
            .ensure_snapshot_capacity(&snapshot)?;
        self.validate_moderation_quarantine_object_snapshot_refs(&snapshot)?;
        self.validate_moderation_quarantine_snapshot_viewer_refs(&snapshot)?;
        let mut objects = self
            .moderation_quarantine_objects
            .write()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?;
        let previous = objects.snapshot();
        objects.restore_snapshot(snapshot)?;
        let committed = objects.snapshot();
        if let Err(err) = self.persist_moderation_quarantine_object_index_snapshot(&committed) {
            if err.committed {
                return Err(ModerationQuarantineObjectError::Io {
                    path: "durability-state".to_owned(),
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = objects.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation quarantine object index snapshot failure",
                    rollback,
                );
                return Err(ModerationQuarantineObjectError::Io {
                    path: "durability-state".to_owned(),
                    message,
                });
            }
            return Err(ModerationQuarantineObjectError::Io {
                path: "durability-state".to_owned(),
                message: err.to_string(),
            });
        }
        Ok(())
    }
    /// Create or return a payload-free local evidence viewer session record.
    ///
    /// The session is bound to an existing encrypted quarantine object by object id and payload
    /// digest. The request is rejected if it includes raw evidence, signed URLs, session tokens,
    /// watermark secrets, or a session duration longer than the local short-lived window.
    ///
    /// # Errors
    ///
    /// Returns an error if the quarantine/object reference is unknown, the
    /// session metadata is invalid or conflicts with existing local state, the
    /// checkpoint cannot be persisted, or the state lock is poisoned.
    pub fn create_moderation_evidence_viewer_session(
        &self,
        input: ModerationEvidenceViewerSessionInput,
    ) -> Result<ModerationEvidenceViewerSessionRecord, ModerationEvidenceViewerError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationEvidenceViewerError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationEvidenceViewerError::Io {
                path: "durability-state".to_owned(),
                message,
            })?;
        let object = self.moderation_quarantine_object_record_for_viewer(&input.quarantine_id)?;
        let mut runtime = self
            .moderation_evidence_viewer
            .write()
            .map_err(|_| ModerationEvidenceViewerError::StateLockPoisoned)?;
        let previous = runtime.snapshot();
        let record = runtime.create_session(input, &object)?;
        let committed = runtime.snapshot();
        if let Err(err) = self.persist_moderation_evidence_viewer_snapshot(&committed) {
            if err.committed {
                return Err(ModerationEvidenceViewerError::Io {
                    path: "durability-state".to_owned(),
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = runtime.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation evidence viewer checkpoint failure",
                    rollback,
                );
                return Err(ModerationEvidenceViewerError::Io {
                    path: "durability-state".to_owned(),
                    message,
                });
            }
            return Err(ModerationEvidenceViewerError::Io {
                path: "durability-state".to_owned(),
                message: err.to_string(),
            });
        }
        Ok(record)
    }
    /// Append a payload-free local evidence viewer access-log event.
    ///
    /// # Errors
    ///
    /// Returns an error if the session is unknown, the event arrives outside the
    /// active session window, the event carries raw payload/token/body markers,
    /// the checkpoint cannot be persisted, or the state lock is poisoned.
    pub fn record_moderation_evidence_viewer_access(
        &self,
        input: ModerationEvidenceViewerAccessInput,
    ) -> Result<ModerationEvidenceViewerAccessEventRecord, ModerationEvidenceViewerError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationEvidenceViewerError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationEvidenceViewerError::Io {
                path: "durability-state".to_owned(),
                message,
            })?;
        let mut runtime = self
            .moderation_evidence_viewer
            .write()
            .map_err(|_| ModerationEvidenceViewerError::StateLockPoisoned)?;
        let previous = runtime.snapshot();
        let record = runtime.record_access(input)?;
        let committed = runtime.snapshot();
        if let Err(err) = self.persist_moderation_evidence_viewer_snapshot(&committed) {
            if err.committed {
                return Err(ModerationEvidenceViewerError::Io {
                    path: "durability-state".to_owned(),
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = runtime.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation evidence viewer access checkpoint failure",
                    rollback,
                );
                return Err(ModerationEvidenceViewerError::Io {
                    path: "durability-state".to_owned(),
                    message,
                });
            }
            return Err(ModerationEvidenceViewerError::Io {
                path: "durability-state".to_owned(),
                message: err.to_string(),
            });
        }
        Ok(record)
    }
    /// Export local evidence viewer audit state, reporting lock failures.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence-viewer runtime lock is poisoned.
    pub fn export_moderation_evidence_viewer_snapshot(
        &self,
    ) -> Result<ModerationEvidenceViewerSnapshot, ModerationEvidenceViewerError> {
        Ok(self
            .moderation_evidence_viewer
            .read()
            .map_err(|_| ModerationEvidenceViewerError::StateLockPoisoned)?
            .snapshot())
    }
    /// Replace local evidence viewer audit state from a validated snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error if the snapshot is internally inconsistent, references missing quarantine
    /// object state, cannot be persisted, or the state lock is poisoned.
    pub fn restore_moderation_evidence_viewer_snapshot(
        &self,
        snapshot: ModerationEvidenceViewerSnapshot,
    ) -> Result<(), ModerationEvidenceViewerError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationEvidenceViewerError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationEvidenceViewerError::Io {
                path: "durability-state".to_owned(),
                message,
            })?;
        self.validate_moderation_evidence_viewer_snapshot_refs(&snapshot)?;
        let mut runtime = self
            .moderation_evidence_viewer
            .write()
            .map_err(|_| ModerationEvidenceViewerError::StateLockPoisoned)?;
        let previous = runtime.snapshot();
        runtime.restore_snapshot(snapshot)?;
        let committed = runtime.snapshot();
        if let Err(err) = self.persist_moderation_evidence_viewer_snapshot(&committed) {
            if err.committed {
                return Err(ModerationEvidenceViewerError::Io {
                    path: "durability-state".to_owned(),
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = runtime.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation evidence viewer snapshot failure",
                    rollback,
                );
                return Err(ModerationEvidenceViewerError::Io {
                    path: "durability-state".to_owned(),
                    message,
                });
            }
            return Err(ModerationEvidenceViewerError::Io {
                path: "durability-state".to_owned(),
                message: err.to_string(),
            });
        }
        Ok(())
    }
    /// Build a payload-free local evidence-viewer audit report for one closed window.
    ///
    /// # Errors
    ///
    /// Returns an error if the report window is invalid, the input attempts to
    /// carry raw payload material, or the evidence-viewer state lock is poisoned.
    pub fn build_moderation_evidence_viewer_audit_report(
        &self,
        input: ModerationEvidenceViewerAuditReportInput,
    ) -> Result<ModerationEvidenceViewerAuditReport, ModerationEvidenceViewerError> {
        let snapshot = self.export_moderation_evidence_viewer_snapshot()?;
        moderation::moderation_evidence_viewer_audit_report_from_snapshot(input, &snapshot)
    }
    /// Build and record a payload-free evidence-viewer audit report as a transparency source entry.
    ///
    /// The report is recorded into the local transparency source-entry worker under the
    /// `EvidenceAccess` ledger kind. Existing transparency publication APIs can then include it in
    /// a ledger cycle and publish that cycle through the configured Governance DAG publisher.
    ///
    /// # Errors
    ///
    /// Returns an error if report derivation fails, source-entry derivation
    /// fails, or the transparency source-entry worker rejects the entry.
    pub fn record_moderation_evidence_viewer_audit_report(
        &self,
        input: ModerationEvidenceViewerAuditReportInput,
    ) -> Result<ModerationEvidenceViewerAuditReportOutcome, ModerationEvidenceViewerError> {
        let report = self.build_moderation_evidence_viewer_audit_report(input)?;
        let source_entry = transparency::moderation_evidence_viewer_audit_report_source_entry(
            &report,
        )
        .map_err(|err| ModerationEvidenceViewerError::TransparencyExport {
            message: err.to_string(),
        })?;
        self.record_transparency_ledger_source_entry(source_entry.clone())
            .map_err(|err| ModerationEvidenceViewerError::TransparencyExport {
                message: err.to_string(),
            })?;
        Ok(ModerationEvidenceViewerAuditReportOutcome {
            report,
            source_entry,
        })
    }
    /// Publish the oldest due payload-free evidence-viewer audit report cycle.
    ///
    /// The scheduler derives report windows from local viewer sessions and access events, records
    /// the oldest due report as an `EvidenceAccess` source entry, then publishes the matching
    /// transparency ledger cycle through the configured governance pipeline. Duplicate cycle
    /// publication is suppressed within the node runtime.
    ///
    /// # Errors
    ///
    /// Returns an error if schedule validation fails, the evidence-viewer state
    /// cannot be read, report/source-entry derivation fails, or publication
    /// through the transparency ledger/Governance DAG pipeline fails.
    pub fn publish_due_moderation_evidence_viewer_audit_report(
        &self,
        now_unix: u64,
        schedule: PrivacyAggregateScheduleConfig,
        report_scope: String,
        policy_digest: Option<[u8; 32]>,
        previous_block_hash: Option<[u8; 32]>,
    ) -> Result<ModerationEvidenceViewerAuditScheduleOutcome, GovernancePublishError> {
        let _mutation_guard = self.runtime_mutation_lock.lock().map_err(|_| {
            GovernancePublishError::other("runtime publication transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        let Some(latest_window) = schedule.due_window(now_unix).map_err(|err| {
            GovernancePublishError::other(format!("evidence viewer audit schedule: {err}"))
        })?
        else {
            return Ok(ModerationEvidenceViewerAuditScheduleOutcome::NotDue);
        };
        let published_cycles = self
            .published_evidence_viewer_audit_cycles
            .read()
            .map_err(|_| {
                GovernancePublishError::other(
                    "evidence viewer audit published-cycle index poisoned",
                )
            })?
            .clone();
        let snapshot = self
            .export_moderation_evidence_viewer_snapshot()
            .map_err(|err| {
                GovernancePublishError::other(format!(
                    "export evidence viewer audit snapshot: {err}"
                ))
            })?;
        let mut due_windows = BTreeSet::new();
        for session in &snapshot.sessions {
            collect_due_evidence_viewer_audit_window(
                &schedule,
                latest_window,
                &published_cycles,
                session.issued_at_unix_ms,
                &mut due_windows,
            )?;
            collect_due_evidence_viewer_audit_window(
                &schedule,
                latest_window,
                &published_cycles,
                session.expires_at_unix_ms.saturating_sub(1),
                &mut due_windows,
            )?;
        }
        for event in &snapshot.access_events {
            collect_due_evidence_viewer_audit_window(
                &schedule,
                latest_window,
                &published_cycles,
                event.event_at_unix_ms,
                &mut due_windows,
            )?;
        }
        if due_windows.is_empty() {
            let cycle_id = evidence_viewer_audit_cycle_id(latest_window);
            if published_cycles.contains(&cycle_id) {
                return Ok(
                    ModerationEvidenceViewerAuditScheduleOutcome::AlreadyPublished {
                        window: latest_window,
                        cycle_id,
                    },
                );
            }
            return Ok(
                ModerationEvidenceViewerAuditScheduleOutcome::NoSourceEvents {
                    window: latest_window,
                    cycle_id,
                },
            );
        }
        for window in due_windows {
            let cycle_id = evidence_viewer_audit_cycle_id(window);
            {
                let published_cycles =
                    self.published_evidence_viewer_audit_cycles
                        .read()
                        .map_err(|_| {
                            GovernancePublishError::other(
                                "evidence viewer audit published-cycle index poisoned",
                            )
                        })?;
                if published_cycles.contains(&cycle_id) {
                    continue;
                }
                let state_limit = self.config.runtime_retention().state_entry_limit();
                if published_cycles.len() >= state_limit {
                    return Err(GovernancePublishError::other(format!(
                        "evidence viewer published-cycle retention exhausted (limit {state_limit})"
                    )));
                }
            }
            let outcome = self
                .record_moderation_evidence_viewer_audit_report(
                    ModerationEvidenceViewerAuditReportInput {
                        report_scope: report_scope.clone(),
                        window_start_unix: window.cycle_start_unix,
                        window_end_unix: window.cycle_end_unix,
                        generated_at_unix: now_unix,
                        policy_digest,
                        raw_evidence_included: false,
                        raw_access_logs_included: false,
                        viewer_accounts_included: false,
                        signed_urls_included: false,
                        session_tokens_included: false,
                        response_bodies_included: false,
                    },
                )
                .map_err(|err| {
                    GovernancePublishError::other(format!(
                        "record evidence viewer audit report: {err}"
                    ))
                })?;
            if outcome.report.session_count == 0 && outcome.report.access_event_count == 0 {
                continue;
            }
            let publication = self.publish_transparency_ledger_cycle_from_source_entries(
                cycle_id,
                window.cycle_start_unix,
                window.cycle_end_unix,
                now_unix,
                previous_block_hash,
            )?;
            let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
                GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
            })?;
            self.ensure_durability_healthy()
                .map_err(GovernancePublishError::other)?;
            self.published_evidence_viewer_audit_cycles
                .write()
                .map_err(|_| {
                    GovernancePublishError::other(
                        "evidence viewer audit published-cycle index poisoned",
                    )
                })?
                .insert(cycle_id);
            if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
                if err.committed {
                    return Err(GovernancePublishError::other(err.to_string()));
                }
                self.published_evidence_viewer_audit_cycles
                    .write()
                    .map_err(|_| {
                        GovernancePublishError::other(
                            "evidence viewer cycle rollback lock poisoned",
                        )
                    })?
                    .remove(&cycle_id);
                return Err(GovernancePublishError::other(err.to_string()));
            }
            return Ok(ModerationEvidenceViewerAuditScheduleOutcome::Published {
                window,
                report: Box::new(outcome.report),
                source_entry: Box::new(outcome.source_entry),
                publication,
            });
        }
        let cycle_id = evidence_viewer_audit_cycle_id(latest_window);
        Ok(
            ModerationEvidenceViewerAuditScheduleOutcome::NoSourceEvents {
                window: latest_window,
                cycle_id,
            },
        )
    }
    /// Publish the next due evidence-viewer audit-report cycle using storage configuration.
    ///
    /// Report scope, policy digest, and previous block hash remain explicit
    /// runtime inputs; persisted config only controls whether scheduled
    /// publication is enabled and which cadence is used.
    pub fn publish_due_configured_moderation_evidence_viewer_audit_report(
        &self,
        now_unix: u64,
        report_scope: String,
        policy_digest: Option<[u8; 32]>,
        previous_block_hash: Option<[u8; 32]>,
    ) -> Result<ModerationEvidenceViewerAuditScheduleOutcome, GovernancePublishError> {
        let Some(schedule) = self.configured_evidence_viewer_audit_schedule() else {
            return Ok(ModerationEvidenceViewerAuditScheduleOutcome::Disabled);
        };
        self.publish_due_moderation_evidence_viewer_audit_report(
            now_unix,
            schedule,
            report_scope,
            policy_digest,
            previous_block_hash,
        )
    }
    fn load_moderation_model_registry_checkpoint(&self) -> Result<(), NodeInitError> {
        let Some(path) = self.moderation_model_registry_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let Some(bytes) = read_local_checkpoint_bounded(
            path,
            self.config.runtime_retention().checkpoint_max_bytes(),
        )
        .map_err(|err| NodeInitError::checkpoint("moderation model registry", path, err))?
        else {
            return Ok(());
        };
        let retention = self.config.runtime_retention();
        let snapshot = decode_local_checkpoint_canonical::<ModerationModelRegistrySnapshot>(
            &bytes,
            retention.checkpoint_max_bytes(),
            retention
                .state_entry_limit()
                .max(retention.event_history_limit()),
        )
        .map_err(|err| NodeInitError::checkpoint("moderation model registry", path, err))?;
        let repro_count = snapshot.reproducibility_manifests.len();
        let corpus_count = snapshot.adversarial_corpora.len();
        self.moderation_model_registry
            .write()
            .map_err(|_| ModerationModelRegistryError::StateLockPoisoned)
            .and_then(|mut registry| registry.restore_snapshot(snapshot))
            .map_err(|err| NodeInitError::checkpoint("moderation model registry", path, err))?;
        iroha_logger::info!(
            path = %path.display(),
            repro_count,
            corpus_count,
            "restored SoraFS moderation model registry checkpoint"
        );
        Ok(())
    }
    fn persist_moderation_model_registry_snapshot(
        &self,
        snapshot: &ModerationModelRegistrySnapshot,
    ) -> Result<(), RuntimeCheckpointPersistError> {
        let Some(path) = self.moderation_model_registry_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let bytes = norito::to_bytes(snapshot).map_err(|err| {
            RuntimeCheckpointPersistError::precommit(format!(
                "encode moderation model registry checkpoint `{}`: {err}",
                path.display()
            ))
        })?;
        self.finish_local_checkpoint_write(
            "moderation model registry",
            path,
            write_local_checkpoint_atomic_bounded(
                path,
                &bytes,
                self.config.runtime_retention().checkpoint_max_bytes(),
            ),
        )
    }
    fn load_moderation_screening_checkpoint(&self) -> Result<(), NodeInitError> {
        let Some(path) = self.moderation_screening_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let Some(bytes) = read_local_checkpoint_bounded(
            path,
            self.config.runtime_retention().checkpoint_max_bytes(),
        )
        .map_err(|err| NodeInitError::checkpoint("moderation screening", path, err))?
        else {
            return Ok(());
        };
        let retention = self.config.runtime_retention();
        let snapshot = decode_local_checkpoint_canonical::<ModerationScreeningSnapshot>(
            &bytes,
            retention.checkpoint_max_bytes(),
            retention
                .state_entry_limit()
                .max(retention.event_history_limit()),
        )
        .map_err(|err| NodeInitError::checkpoint("moderation screening", path, err))?;
        let screening_count = snapshot.screening_records.len();
        let quarantine_count = snapshot.quarantine_records.len();
        self.moderation_screening
            .write()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)
            .and_then(|mut runtime| runtime.restore_snapshot(snapshot))
            .map_err(|err| NodeInitError::checkpoint("moderation screening", path, err))?;
        iroha_logger::info!(
            path = %path.display(),
            screening_count,
            quarantine_count,
            "restored SoraFS moderation screening checkpoint"
        );
        Ok(())
    }
    fn persist_moderation_screening_snapshot(
        &self,
        snapshot: &ModerationScreeningSnapshot,
    ) -> Result<(), RuntimeCheckpointPersistError> {
        let Some(path) = self.moderation_screening_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let bytes = norito::to_bytes(snapshot).map_err(|err| {
            RuntimeCheckpointPersistError::precommit(format!(
                "encode moderation screening checkpoint `{}`: {err}",
                path.display()
            ))
        })?;
        self.finish_local_checkpoint_write(
            "moderation screening",
            path,
            write_local_checkpoint_atomic_bounded(
                path,
                &bytes,
                self.config.runtime_retention().checkpoint_max_bytes(),
            ),
        )
    }
    fn load_moderation_quarantine_object_index_checkpoint(&self) -> Result<(), NodeInitError> {
        let Some(path) = self.moderation_quarantine_object_index_path.as_ref() else {
            return Ok(());
        };
        let Some(bytes) = read_local_checkpoint_bounded(
            path,
            self.config.runtime_retention().checkpoint_max_bytes(),
        )
        .map_err(|err| {
            NodeInitError::checkpoint("moderation quarantine object index", path, err)
        })?
        else {
            return Ok(());
        };
        let retention = self.config.runtime_retention();
        let snapshot = decode_local_checkpoint_canonical::<ModerationQuarantineObjectSnapshot>(
            &bytes,
            retention.checkpoint_max_bytes(),
            retention
                .state_entry_limit()
                .max(retention.event_history_limit()),
        )
        .map_err(|err| {
            NodeInitError::checkpoint("moderation quarantine object index", path, err)
        })?;
        let object_count = snapshot.objects.len();
        self.moderation_quarantine_objects
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)
            .and_then(|objects| objects.ensure_snapshot_capacity(&snapshot))
            .map_err(|err| {
                NodeInitError::checkpoint("moderation quarantine object index", path, err)
            })?;
        self.validate_moderation_quarantine_object_snapshot_refs(&snapshot)
            .map_err(|err| {
                NodeInitError::checkpoint("moderation quarantine object index", path, err)
            })?;
        self.moderation_quarantine_objects
            .write()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)
            .and_then(|mut objects| objects.restore_snapshot(snapshot))
            .map_err(|err| {
                NodeInitError::checkpoint("moderation quarantine object index", path, err)
            })?;
        iroha_logger::info!(
            path = %path.display(),
            object_count,
            "restored SoraFS moderation quarantine object index"
        );
        Ok(())
    }
    fn audit_moderation_quarantine_object_store(&self) -> Result<(), NodeInitError> {
        let root = self
            .moderation_quarantine_object_root
            .as_ref()
            .expect("storage-backed node has a quarantine object root");
        let index_path = self
            .moderation_quarantine_object_index_path
            .as_ref()
            .expect("storage-backed node has a quarantine index path");
        let snapshot = self
            .moderation_quarantine_objects
            .read()
            .map_err(|_| {
                NodeInitError::checkpoint(
                    "moderation quarantine object store",
                    root,
                    "object index lock poisoned",
                )
            })?
            .snapshot();
        let key_wrapper = self.moderation_quarantine_key_wrapper.as_deref();
        let key_provider_binding = self.moderation_quarantine_key_provider_binding.as_ref();
        if !snapshot.objects.is_empty() && (key_wrapper.is_none() || key_provider_binding.is_none())
        {
            return Err(NodeInitError::checkpoint(
                "moderation quarantine key wrapper",
                root,
                "runtime PKCS#11/KMS key wrapper or its configured binding is missing while indexed objects exist",
            ));
        }
        let mut expected_files = BTreeSet::from([index_path.clone()]);
        for record in &snapshot.objects {
            let envelope_path = self
                .resolve_moderation_quarantine_object_path(root, record)
                .map_err(|err| {
                    NodeInitError::checkpoint("moderation quarantine object store", root, err)
                })?;
            let bytes = read_local_checkpoint_bounded(
                &envelope_path,
                self.config.runtime_retention().checkpoint_max_bytes(),
            )
            .map_err(|err| {
                NodeInitError::checkpoint(
                    "moderation quarantine object envelope",
                    &envelope_path,
                    err,
                )
            })?
            .ok_or_else(|| {
                NodeInitError::checkpoint(
                    "moderation quarantine object envelope",
                    &envelope_path,
                    "indexed envelope is missing",
                )
            })?;
            let envelope = norito::decode_from_bytes::<ModerationQuarantineObjectEnvelopeV1>(
                &bytes,
            )
            .map_err(|err| {
                NodeInitError::checkpoint(
                    "moderation quarantine object envelope",
                    &envelope_path,
                    err,
                )
            })?;
            let canonical = norito::to_bytes(&envelope).map_err(|err| {
                NodeInitError::checkpoint(
                    "moderation quarantine object envelope",
                    &envelope_path,
                    err,
                )
            })?;
            if canonical != bytes {
                return Err(NodeInitError::checkpoint(
                    "moderation quarantine object envelope",
                    &envelope_path,
                    "envelope is not canonically encoded",
                ));
            }
            let payload = open_moderation_quarantine_object(
                &envelope,
                record,
                key_provider_binding
                    .expect("non-empty object index requires a configured key-provider binding"),
                key_wrapper.expect("non-empty object index requires a runtime key wrapper"),
            )
            .map_err(|err| {
                NodeInitError::checkpoint(
                    "moderation quarantine object envelope",
                    &envelope_path,
                    err,
                )
            })?;
            let quarantine = self
                .moderation_quarantine_record_for_object(&record.quarantine_id)
                .map_err(|err| {
                    NodeInitError::checkpoint(
                        "moderation quarantine object envelope",
                        &envelope_path,
                        err,
                    )
                })?;
            if *blake3::hash(&payload).as_bytes() != quarantine.subject_digest {
                return Err(NodeInitError::checkpoint(
                    "moderation quarantine object envelope",
                    &envelope_path,
                    "decrypted payload digest does not match quarantine subject",
                ));
            }
            expected_files.insert(envelope_path);
        }
        let file_limit = self
            .config
            .runtime_retention()
            .state_entry_limit()
            .saturating_add(2);
        let mut actual_files = BTreeSet::new();
        collect_secure_object_store_files(root, root, 0, file_limit, &mut actual_files).map_err(
            |err| NodeInitError::checkpoint("moderation quarantine object store", root, err),
        )?;
        let orphan_files = actual_files
            .difference(&expected_files)
            .cloned()
            .collect::<Vec<_>>();
        for orphan_path in orphan_files {
            if !self
                .recover_unindexed_moderation_quarantine_envelope(root, &orphan_path)
                .map_err(|error| {
                    NodeInitError::checkpoint(
                        "moderation quarantine object store",
                        &orphan_path,
                        error,
                    )
                })?
            {
                return Err(NodeInitError::checkpoint(
                    "moderation quarantine object store",
                    &orphan_path,
                    "unindexed object-store file is not a canonical crash orphan",
                ));
            }
            actual_files.remove(&orphan_path);
        }
        if actual_files != expected_files {
            let unexpected = actual_files
                .difference(&expected_files)
                .next()
                .or_else(|| expected_files.difference(&actual_files).next())
                .cloned()
                .unwrap_or_else(|| root.clone());
            return Err(NodeInitError::checkpoint(
                "moderation quarantine object store",
                &unexpected,
                "object store files do not exactly match the durable index",
            ));
        }
        Ok(())
    }
    fn persist_moderation_quarantine_object_index_snapshot(
        &self,
        snapshot: &ModerationQuarantineObjectSnapshot,
    ) -> Result<(), RuntimeCheckpointPersistError> {
        let Some(path) = self.moderation_quarantine_object_index_path.as_ref() else {
            return Ok(());
        };
        let bytes = norito::to_bytes(snapshot).map_err(|err| {
            RuntimeCheckpointPersistError::precommit(format!(
                "encode moderation quarantine object index checkpoint `{}`: {err}",
                path.display()
            ))
        })?;
        self.finish_local_checkpoint_write(
            "moderation quarantine object index",
            path,
            write_local_checkpoint_atomic_bounded(
                path,
                &bytes,
                self.config.runtime_retention().checkpoint_max_bytes(),
            ),
        )
    }
    fn load_moderation_evidence_viewer_checkpoint(&self) -> Result<(), NodeInitError> {
        let Some(path) = self.moderation_evidence_viewer_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let Some(bytes) = read_local_checkpoint_bounded(
            path,
            self.config.runtime_retention().checkpoint_max_bytes(),
        )
        .map_err(|err| NodeInitError::checkpoint("moderation evidence viewer", path, err))?
        else {
            return Ok(());
        };
        let retention = self.config.runtime_retention();
        let snapshot = decode_local_checkpoint_canonical::<ModerationEvidenceViewerSnapshot>(
            &bytes,
            retention.checkpoint_max_bytes(),
            retention
                .state_entry_limit()
                .max(retention.event_history_limit()),
        )
        .map_err(|err| NodeInitError::checkpoint("moderation evidence viewer", path, err))?;
        let session_count = snapshot.sessions.len();
        let access_event_count = snapshot.access_events.len();
        self.validate_moderation_evidence_viewer_snapshot_refs(&snapshot)
            .map_err(|err| NodeInitError::checkpoint("moderation evidence viewer", path, err))?;
        self.moderation_evidence_viewer
            .write()
            .map_err(|_| ModerationEvidenceViewerError::StateLockPoisoned)
            .and_then(|mut runtime| runtime.restore_snapshot(snapshot))
            .map_err(|err| NodeInitError::checkpoint("moderation evidence viewer", path, err))?;
        iroha_logger::info!(
            path = %path.display(),
            session_count,
            access_event_count,
            "restored SoraFS moderation evidence viewer checkpoint"
        );
        Ok(())
    }
    fn persist_moderation_evidence_viewer_snapshot(
        &self,
        snapshot: &ModerationEvidenceViewerSnapshot,
    ) -> Result<(), RuntimeCheckpointPersistError> {
        let Some(path) = self.moderation_evidence_viewer_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let bytes = norito::to_bytes(snapshot).map_err(|err| {
            RuntimeCheckpointPersistError::precommit(format!(
                "encode moderation evidence viewer checkpoint `{}`: {err}",
                path.display()
            ))
        })?;
        self.finish_local_checkpoint_write(
            "moderation evidence viewer",
            path,
            write_local_checkpoint_atomic_bounded(
                path,
                &bytes,
                self.config.runtime_retention().checkpoint_max_bytes(),
            ),
        )
    }
    fn export_auxiliary_runtime_checkpoint(
        &self,
    ) -> Result<AuxiliaryRuntimeCheckpointV5, GovernancePublishError> {
        let capacity_runtime = self.capacity.checkpoint().map_err(|err| {
            GovernancePublishError::other(format!("export capacity runtime checkpoint: {err}"))
        })?;
        let mut por_history = self
            .por_history
            .read()
            .map_err(|_| GovernancePublishError::other("PoR history lock poisoned"))?
            .iter()
            .map(
                |((manifest_digest, provider_id), entry)| PorHistoryCheckpointEntryV1 {
                    manifest_digest: *manifest_digest,
                    provider_id: *provider_id,
                    last_success_unix: entry.last_success_unix,
                    last_failure_unix: entry.last_failure_unix,
                    failures_total: entry.failures_total,
                    consecutive_failures: entry.consecutive_failures,
                },
            )
            .collect::<Vec<_>>();
        por_history.sort_by_key(|entry| (entry.manifest_digest, entry.provider_id));
        let gc_eviction_intents = self
            .gc_eviction_intents
            .read()
            .map_err(|_| GovernancePublishError::other("GC eviction intent lock poisoned"))?;
        let gc_eviction_intent_next_sequence = gc_eviction_intents.next_sequence;
        let gc_eviction_intents = gc_eviction_intents.entries.values().cloned().collect();
        let gc_eviction_audit_links = self
            .gc_eviction_audit_links
            .read()
            .map_err(|_| GovernancePublishError::other("GC eviction audit link lock poisoned"))?
            .values()
            .cloned()
            .collect();
        let reputation_snapshots = self
            .reputation_snapshots
            .read()
            .map_err(|_| GovernancePublishError::other("reputation snapshot index poisoned"))?
            .values()
            .cloned()
            .collect();
        let latest_reputation_snapshot_id = self
            .latest_reputation_snapshot
            .read()
            .map_err(|_| GovernancePublishError::other("reputation snapshot cache poisoned"))?
            .as_ref()
            .map(|snapshot| snapshot.snapshot_id);
        let reputation_events = self
            .reputation_events
            .read()
            .map_err(|_| GovernancePublishError::other("reputation event history poisoned"))?
            .retained();
        let transparency_source_entries = self
            .transparency_ledger_source_entries
            .read()
            .map_err(|_| GovernancePublishError::other("transparency source-entry index poisoned"))?
            .values()
            .cloned()
            .collect();
        let privacy_source_events = self
            .privacy_aggregate_source_events
            .read()
            .map_err(|_| GovernancePublishError::other("privacy source-event index poisoned"))?
            .values()
            .cloned()
            .collect();
        let privacy_source_event_receipts = self
            .privacy_source_event_receipts
            .read()
            .map_err(|_| {
                GovernancePublishError::other("privacy source-event receipt index poisoned")
            })?
            .iter()
            .map(
                |(event_id, canonical_digest)| transparency::PrivacySourceEventReceiptV1 {
                    event_id: event_id.clone(),
                    canonical_digest: *canonical_digest,
                },
            )
            .collect();
        let privacy_publish_request_receipts = self
            .privacy_publish_request_receipts
            .read()
            .map_err(|_| {
                GovernancePublishError::other("privacy publish-request receipt index poisoned")
            })?
            .values()
            .cloned()
            .collect();
        let published_privacy_aggregate_cycles = self
            .published_privacy_aggregate_cycles
            .read()
            .map_err(|_| GovernancePublishError::other("privacy cycle index poisoned"))?
            .iter()
            .copied()
            .collect();
        let privacy_composition_budget = self
            .privacy_composition_budget
            .read()
            .map_err(|_| GovernancePublishError::other("privacy composition budget poisoned"))?
            .clone();
        let privacy_release_ledger = self
            .privacy_release_ledger
            .read()
            .map_err(|_| GovernancePublishError::other("privacy release ledger poisoned"))?
            .clone();
        let transparency_leader_lease_fencing_floor = self
            .transparency_leader_lease_provider
            .as_ref()
            .map_or(Ok(0), |provider| provider.0.fencing_floor())
            .map_err(|error| {
                GovernancePublishError::other(format!(
                    "read transparency leader-lease fencing floor: {error}"
                ))
            })?;
        let published_evidence_viewer_audit_cycles = self
            .published_evidence_viewer_audit_cycles
            .read()
            .map_err(|_| GovernancePublishError::other("evidence cycle index poisoned"))?
            .iter()
            .copied()
            .collect();
        let governance_outbox = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?;
        let governance_outbox_next_sequence = governance_outbox.next_sequence;
        let governance_outbox_entries = governance_outbox.entries.values().cloned().collect();
        Ok(AuxiliaryRuntimeCheckpointV5 {
            version: AUX_RUNTIME_STATE_VERSION_V5,
            capacity_runtime,
            por_tracker: self.por.checkpoint(),
            por_history,
            gc_eviction_intent_next_sequence,
            gc_eviction_intents,
            gc_eviction_audit_links,
            reputation_snapshots,
            latest_reputation_snapshot_id,
            reputation_events,
            transparency_source_entries,
            privacy_source_events,
            privacy_source_event_receipts,
            privacy_publish_request_receipts,
            published_privacy_aggregate_cycles,
            privacy_composition_budget,
            privacy_release_ledger,
            transparency_leader_lease_fencing_floor,
            published_evidence_viewer_audit_cycles,
            governance_outbox_next_sequence,
            governance_outbox_entries,
        })
    }
    fn persist_auxiliary_runtime_checkpoint_unlocked(
        &self,
    ) -> Result<(), RuntimeCheckpointPersistError> {
        let Some(path) = self.auxiliary_runtime_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let checkpoint = self.export_auxiliary_runtime_checkpoint().map_err(|err| {
            RuntimeCheckpointPersistError::precommit(format!(
                "export auxiliary runtime checkpoint: {err}"
            ))
        })?;
        let bytes = norito::to_bytes(&checkpoint).map_err(|err| {
            RuntimeCheckpointPersistError::precommit(format!(
                "encode auxiliary runtime checkpoint: {err}"
            ))
        })?;
        let write_result = write_local_checkpoint_atomic_bounded(
            path,
            &bytes,
            self.config.runtime_retention().checkpoint_max_bytes(),
        );
        #[cfg(test)]
        let write_result = if write_result.is_ok()
            && self
                .fail_after_next_auxiliary_checkpoint_publication
                .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            Err(LocalCheckpointWriteError::committed(io::Error::other(
                "injected post-publication auxiliary checkpoint failure",
            )))
        } else {
            write_result
        };
        self.finish_local_checkpoint_write("auxiliary runtime", path, write_result)
    }
    fn mutate_capacity_durably<T>(
        &self,
        mutate: impl FnOnce(&CapacityManager) -> Result<(T, bool), CapacityError>,
    ) -> Result<T, CapacityError> {
        let _checkpoint_guard = self
            .auxiliary_checkpoint_lock
            .lock()
            .map_err(|_| CapacityError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(CapacityError::Checkpoint)?;
        let previous = self.capacity.checkpoint()?;
        let (outcome, changed) = match mutate(&self.capacity) {
            Ok(outcome) => outcome,
            Err(err) => {
                if let Err(restore_err) = self.capacity.restore_checkpoint(previous) {
                    let message = self.record_unrecoverable_rollback(
                        "failed to roll back rejected capacity mutation",
                        restore_err,
                    );
                    return Err(CapacityError::Checkpoint(message));
                }
                return Err(err);
            }
        };
        if !changed {
            return Ok(outcome);
        }
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(CapacityError::Checkpoint(err.to_string()));
            }
            if let Err(restore_err) = self.capacity.restore_checkpoint(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back capacity checkpoint failure",
                    restore_err,
                );
                return Err(CapacityError::Checkpoint(message));
            }
            return Err(CapacityError::Checkpoint(err.to_string()));
        }
        Ok(outcome)
    }
    fn load_auxiliary_runtime_checkpoint(&self) -> Result<(), NodeInitError> {
        let Some(path) = self.auxiliary_runtime_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let Some(bytes) = read_local_checkpoint_bounded(
            path,
            self.config.runtime_retention().checkpoint_max_bytes(),
        )
        .map_err(|err| NodeInitError::checkpoint("auxiliary runtime", path, err))?
        else {
            return Ok(());
        };
        let retention = self.config.runtime_retention();
        let maximum_sequence_elements = retention
            .state_entry_limit()
            .saturating_mul(2)
            .saturating_add(retention.event_history_limit())
            // Checkpoints contain bounded byte blobs whose element count is
            // independent of the number of retained domain records. The
            // archive's already-enforced byte length is a strict upper bound
            // for those sequences; domain restore validation still enforces
            // the configured state and event limits.
            .max(bytes.len());
        let checkpoint = decode_local_checkpoint_canonical::<AuxiliaryRuntimeCheckpointV5>(
            &bytes,
            retention.checkpoint_max_bytes(),
            maximum_sequence_elements,
        )
        .map_err(|err| NodeInitError::checkpoint("auxiliary runtime", path, err))?;
        self.restore_auxiliary_runtime_checkpoint(checkpoint)
            .map_err(|err| NodeInitError::checkpoint("auxiliary runtime", path, err))?;
        Ok(())
    }
    fn restore_auxiliary_runtime_checkpoint(
        &self,
        checkpoint: AuxiliaryRuntimeCheckpointV5,
    ) -> Result<(), GovernancePublishError> {
        if checkpoint.version != AUX_RUNTIME_STATE_VERSION_V5 {
            return Err(GovernancePublishError::other(format!(
                "unsupported auxiliary runtime checkpoint version {}",
                checkpoint.version
            )));
        }
        match self.transparency_leader_lease_provider.as_ref() {
            Some(provider) => provider
                .0
                .restore_fencing_floor(checkpoint.transparency_leader_lease_fencing_floor)
                .map_err(|error| {
                    GovernancePublishError::other(format!(
                        "restore transparency leader-lease fencing floor: {error}"
                    ))
                })?,
            None if checkpoint.transparency_leader_lease_fencing_floor != 0 => {
                return Err(GovernancePublishError::other(
                    "auxiliary checkpoint contains a leader-lease fencing floor without a configured provider",
                ));
            }
            None => {}
        }
        let retention = self.config.runtime_retention();
        let state_limit = retention.state_entry_limit();
        let event_limit = retention.event_history_limit();
        for (label, count, limit) in [
            ("PoR history", checkpoint.por_history.len(), state_limit),
            (
                "reputation snapshots",
                checkpoint.reputation_snapshots.len(),
                state_limit,
            ),
            (
                "transparency source entries",
                checkpoint.transparency_source_entries.len(),
                state_limit,
            ),
            (
                "privacy source events",
                checkpoint.privacy_source_events.len(),
                state_limit,
            ),
            (
                "privacy source-event receipts",
                checkpoint.privacy_source_event_receipts.len(),
                state_limit,
            ),
            (
                "privacy publish-request receipts",
                checkpoint.privacy_publish_request_receipts.len(),
                state_limit,
            ),
            (
                "published privacy cycles",
                checkpoint.published_privacy_aggregate_cycles.len(),
                state_limit,
            ),
            (
                "privacy release records",
                checkpoint.privacy_release_ledger.records.len(),
                state_limit,
            ),
            (
                "published evidence cycles",
                checkpoint.published_evidence_viewer_audit_cycles.len(),
                state_limit,
            ),
            (
                "governance outbox",
                checkpoint.governance_outbox_entries.len(),
                state_limit,
            ),
            (
                "GC eviction intents",
                checkpoint.gc_eviction_intents.len(),
                1,
            ),
            (
                "GC eviction audit links",
                checkpoint.gc_eviction_audit_links.len(),
                state_limit,
            ),
            (
                "reputation events",
                checkpoint.reputation_events.len(),
                event_limit,
            ),
        ] {
            if count > limit {
                return Err(GovernancePublishError::other(format!(
                    "{label} checkpoint count {count} exceeds configured limit {limit}"
                )));
            }
        }
        let mut por_history = HashMap::with_capacity(checkpoint.por_history.len());
        for entry in checkpoint.por_history {
            if entry.manifest_digest == [0; 32]
                || entry.provider_id == [0; 32]
                || entry
                    .last_success_unix
                    .is_some_and(|timestamp| timestamp == 0)
                || entry
                    .last_failure_unix
                    .is_some_and(|timestamp| timestamp == 0)
                || (entry.last_success_unix.is_none() && entry.last_failure_unix.is_none())
                || entry.consecutive_failures > entry.failures_total
                || (entry.failures_total == 0
                    && (entry.last_failure_unix.is_some() || entry.consecutive_failures != 0))
                || (entry.failures_total != 0 && entry.last_failure_unix.is_none())
            {
                return Err(GovernancePublishError::other(
                    "invalid PoR history entry in auxiliary checkpoint",
                ));
            }
            let key = (entry.manifest_digest, entry.provider_id);
            if por_history
                .insert(
                    key,
                    PorHistoryEntry {
                        last_success_unix: entry.last_success_unix,
                        last_failure_unix: entry.last_failure_unix,
                        failures_total: entry.failures_total,
                        consecutive_failures: entry.consecutive_failures,
                    },
                )
                .is_some()
            {
                return Err(GovernancePublishError::other(
                    "duplicate PoR history key in auxiliary checkpoint",
                ));
            }
        }
        let mut snapshots = BTreeMap::new();
        let mut previous_reputation_snapshot_id = None;
        for admitted in checkpoint.reputation_snapshots {
            if admitted.version != ADMITTED_REPUTATION_SNAPSHOT_VERSION_V1
                || admitted.admitted_at_unix == 0
                || admitted.encoded_len == 0
            {
                return Err(GovernancePublishError::other(
                    "invalid admitted reputation snapshot version or timestamp in auxiliary checkpoint",
                ));
            }
            let policy = self.reputation_trust_policy.as_deref().ok_or_else(|| {
                GovernancePublishError::other(
                    "auxiliary checkpoint contains signed reputation state but no external trust policy is configured",
                )
            })?;
            admitted
                .envelope
                .verify(policy, admitted.admitted_at_unix)
                .map_err(|err| {
                    GovernancePublishError::other(format!(
                        "invalid signed reputation snapshot in auxiliary checkpoint: {err}"
                    ))
                })?;
            let canonical_len = u64::try_from(
                admitted
                    .envelope
                    .canonical_bytes()
                    .map_err(|err| {
                        GovernancePublishError::other(format!(
                            "failed to canonicalize signed reputation snapshot in auxiliary checkpoint: {err}"
                        ))
                    })?
                    .len(),
            )
            .map_err(|_| {
                GovernancePublishError::other(
                    "signed reputation checkpoint envelope length does not fit u64",
                )
            })?;
            if admitted.encoded_len != canonical_len {
                return Err(GovernancePublishError::other(
                    "signed reputation checkpoint encoded length mismatch",
                ));
            }
            let snapshot_id = admitted.envelope.snapshot.snapshot_id;
            if previous_reputation_snapshot_id.is_some_and(|previous| previous >= snapshot_id) {
                return Err(GovernancePublishError::other(
                    "reputation snapshot checkpoint entries must be strictly ordered by snapshot id",
                ));
            }
            previous_reputation_snapshot_id = Some(snapshot_id);
            if snapshots.insert(snapshot_id, admitted).is_some() {
                return Err(GovernancePublishError::other(
                    "duplicate reputation snapshot id in auxiliary checkpoint",
                ));
            }
        }
        let latest_snapshot = match checkpoint.latest_reputation_snapshot_id {
            Some(snapshot_id) => Some(
                snapshots
                    .get(&snapshot_id)
                    .map(|admitted| admitted.envelope.snapshot.clone())
                    .ok_or_else(|| {
                        GovernancePublishError::other(
                            "latest reputation snapshot id is absent from auxiliary checkpoint",
                        )
                    })?,
            ),
            None if snapshots.is_empty() => None,
            None => {
                return Err(GovernancePublishError::other(
                    "non-empty reputation snapshot checkpoint is missing latest id",
                ));
            }
        };
        let mut reputation_chain = Vec::new();
        reputation_chain
            .try_reserve_exact(snapshots.len())
            .map_err(|_| {
                GovernancePublishError::other(
                    "failed to reserve retained reputation-chain validation state",
                )
            })?;
        reputation_chain.extend(snapshots.values());
        reputation_chain.sort_by_key(|admitted| {
            (
                admitted.envelope.snapshot.generated_at_unix,
                admitted.envelope.snapshot.snapshot_id,
            )
        });
        for pair in reputation_chain.windows(2) {
            validate_reputation_snapshot_transition(
                Some(&pair[0].envelope.snapshot),
                &pair[1].envelope.snapshot,
            )
            .map_err(|err| {
                GovernancePublishError::other(format!(
                    "retained reputation snapshots are not one exact monotonic chain: {err}"
                ))
            })?;
        }
        if let (Some(latest), Some(last)) = (latest_snapshot.as_ref(), reputation_chain.last())
            && latest.snapshot_id != last.envelope.snapshot.snapshot_id
        {
            return Err(GovernancePublishError::other(
                "latest reputation snapshot is not the final retained signed envelope",
            ));
        }
        drop(reputation_chain);
        let gc_eviction_intents = restore_gc_eviction_intents(
            checkpoint.gc_eviction_intent_next_sequence,
            checkpoint.gc_eviction_intents,
        )?;
        let mut gc_eviction_audit_links = BTreeMap::new();
        let mut previous_gc_intent_sequence = None;
        let mut gc_linked_outbox_sequences = BTreeSet::new();
        for link in checkpoint.gc_eviction_audit_links {
            validate_gc_eviction_audit_link(&link)?;
            if link.intent_sequence >= gc_eviction_intents.next_sequence
                || gc_eviction_intents
                    .entries
                    .contains_key(&link.intent_sequence)
                || previous_gc_intent_sequence
                    .is_some_and(|previous| previous >= link.intent_sequence)
                || !gc_linked_outbox_sequences.insert(link.outbox_sequence)
            {
                return Err(GovernancePublishError::other(
                    "GC eviction audit links must reference finalized intents below the high-water mark and be ordered and unique by intent/outbox sequence",
                ));
            }
            previous_gc_intent_sequence = Some(link.intent_sequence);
            gc_eviction_audit_links.insert(link.intent_sequence, link);
        }
        let mut reputation_events = BoundedEventHistory::new(event_limit);
        let mut previous_reputation_event: Option<&ReputationSnapshotEventV1> = None;
        for event in &checkpoint.reputation_events {
            event.validate().map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid reputation event in auxiliary checkpoint: {err}"
                ))
            })?;
            let Some(admitted) = snapshots.get(&event.snapshot_id) else {
                return Err(GovernancePublishError::other(
                    "reputation event references a missing retained snapshot",
                ));
            };
            let snapshot = &admitted.envelope.snapshot;
            let provider_count = u32::try_from(snapshot.providers.len()).map_err(|_| {
                GovernancePublishError::other(
                    "retained reputation snapshot provider count exceeds u32",
                )
            })?;
            if event.generated_at_unix != snapshot.generated_at_unix
                || event.merkle_root != snapshot.merkle_root
                || event.provider_count != provider_count
                || event.previous_snapshot_id != snapshot.previous_snapshot_id
            {
                return Err(GovernancePublishError::other(
                    "reputation event metadata does not match its retained snapshot",
                ));
            }
            if let Some(previous) = previous_reputation_event {
                if event.previous_snapshot_id != Some(previous.snapshot_id)
                    || event.generated_at_unix <= previous.generated_at_unix
                {
                    return Err(GovernancePublishError::other(
                        "reputation event suffix is not a monotonic snapshot chain",
                    ));
                }
            }
            previous_reputation_event = Some(event);
        }
        if let (Some(latest), Some(last_event)) = (
            latest_snapshot.as_ref(),
            checkpoint.reputation_events.last(),
        ) && latest.snapshot_id != last_event.snapshot_id
        {
            return Err(GovernancePublishError::other(
                "latest reputation snapshot does not match the final retained event",
            ));
        }
        reputation_events.restore(checkpoint.reputation_events, |event| event.sequence)?;
        let validated_capacity = CapacityManager::with_entry_limit(state_limit);
        validated_capacity
            .restore_checkpoint(checkpoint.capacity_runtime)
            .map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid capacity runtime auxiliary checkpoint: {err}"
                ))
            })?;
        let validated_capacity_checkpoint = validated_capacity.checkpoint().map_err(|err| {
            GovernancePublishError::other(format!(
                "failed to normalize capacity runtime auxiliary checkpoint: {err}"
            ))
        })?;
        let mut transparency_entries = BTreeMap::new();
        for entry in checkpoint.transparency_source_entries {
            entry.validate().map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid transparency source entry in auxiliary checkpoint: {err}"
                ))
            })?;
            if transparency_entries
                .insert(entry.event_id.clone(), entry)
                .is_some()
            {
                return Err(GovernancePublishError::other(
                    "duplicate transparency source entry in auxiliary checkpoint",
                ));
            }
        }
        let mut privacy_receipts = BTreeMap::new();
        for receipt in checkpoint.privacy_source_event_receipts {
            if receipt.event_id.is_empty()
                || receipt.event_id.len() > MODERATION_LEDGER_MAX_PUBLIC_TEXT_BYTES_V1
                || receipt.event_id.trim() != receipt.event_id
                || receipt.event_id.chars().any(char::is_control)
                || receipt.canonical_digest == [0; 32]
                || privacy_receipts
                    .insert(receipt.event_id, receipt.canonical_digest)
                    .is_some()
            {
                return Err(GovernancePublishError::other(
                    "invalid or duplicate privacy source-event receipt in auxiliary checkpoint",
                ));
            }
        }
        let mut privacy_publish_receipts = BTreeMap::new();
        for receipt in checkpoint.privacy_publish_request_receipts {
            receipt.validate()?;
            if privacy_publish_receipts
                .insert(receipt.idempotency_key.clone(), receipt)
                .is_some()
            {
                return Err(GovernancePublishError::other(
                    "duplicate privacy publish-request receipt in auxiliary checkpoint",
                ));
            }
        }
        let mut privacy_events = BTreeMap::new();
        for event in checkpoint.privacy_source_events {
            event.validate().map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid privacy source event in auxiliary checkpoint: {err}"
                ))
            })?;
            let event_digest = event.canonical_digest().map_err(|err| {
                GovernancePublishError::other(format!(
                    "digest privacy source event in auxiliary checkpoint: {err}"
                ))
            })?;
            if privacy_receipts.get(&event.event_id) != Some(&event_digest)
                || privacy_events
                    .insert(event.event_id.clone(), event)
                    .is_some()
            {
                return Err(GovernancePublishError::other(
                    "privacy source event is duplicate or lacks its exact durable receipt",
                ));
            }
        }
        let privacy_cycle_count = checkpoint.published_privacy_aggregate_cycles.len();
        let evidence_cycle_count = checkpoint.published_evidence_viewer_audit_cycles.len();
        let privacy_cycles = checkpoint
            .published_privacy_aggregate_cycles
            .into_iter()
            .collect::<BTreeSet<_>>();
        let privacy_composition_budget = checkpoint.privacy_composition_budget;
        privacy_composition_budget.validate().map_err(|err| {
            GovernancePublishError::other(format!(
                "invalid privacy composition budget in auxiliary checkpoint: {err}"
            ))
        })?;
        if privacy_composition_budget
            .chains
            .iter()
            .flat_map(|chain| chain.charges.iter())
            .any(|charge| !privacy_cycles.contains(&charge.cycle_id))
        {
            return Err(GovernancePublishError::other(
                "privacy composition budget charge references an unpublished cycle",
            ));
        }
        let privacy_release_ledger = checkpoint.privacy_release_ledger;
        privacy_release_ledger.validate().map_err(|err| {
            GovernancePublishError::other(format!(
                "invalid privacy release ledger in auxiliary checkpoint: {err}"
            ))
        })?;
        let release_cycle_ids = privacy_release_ledger
            .records
            .iter()
            .map(|record| record.release_id)
            .collect::<BTreeSet<_>>();
        if release_cycle_ids != privacy_cycles {
            return Err(GovernancePublishError::other(
                "privacy release records and published-cycle guards differ",
            ));
        }
        let mut receipt_cycle_ids = BTreeSet::new();
        for receipt in privacy_publish_receipts.values() {
            if !receipt_cycle_ids.insert(receipt.cycle_id) {
                return Err(GovernancePublishError::other(
                    "multiple privacy publish-request receipts reference one release",
                ));
            }
            let Some(record) = privacy_release_ledger
                .records
                .iter()
                .find(|record| record.release_id == receipt.cycle_id)
            else {
                return Err(GovernancePublishError::other(
                    "privacy publish-request receipt references an unknown release",
                ));
            };
            validate_privacy_publish_receipt_release(receipt, record)?;
        }
        if receipt_cycle_ids != release_cycle_ids {
            return Err(GovernancePublishError::other(
                "privacy release ledger and publish-request receipt inventory differ",
            ));
        }
        let budget_charges = privacy_composition_budget
            .chains
            .iter()
            .flat_map(|chain| chain.charges.iter())
            .map(|charge| (charge.cycle_id, charge))
            .collect::<BTreeMap<_, _>>();
        let dp_release_count = privacy_release_ledger
            .records
            .iter()
            .filter(|record| record.privacy.per_subject_metric_cap.is_some())
            .count();
        if budget_charges.len() != dp_release_count {
            return Err(GovernancePublishError::other(
                "privacy release records and composition-budget charges differ",
            ));
        }
        for record in &privacy_release_ledger.records {
            if record.privacy.per_subject_metric_cap.is_some() {
                let charge = budget_charges.get(&record.release_id).ok_or_else(|| {
                    GovernancePublishError::other(
                        "differential-privacy release has no composition-budget charge",
                    )
                })?;
                if record.budget_charge_digest != Some(charge.charge_digest)
                    || record.status != transparency::PrivacyReleaseStatusV1::Published
                {
                    return Err(GovernancePublishError::other(
                        "differential-privacy release charge linkage is invalid",
                    ));
                }
            }
        }
        let evidence_cycles = checkpoint
            .published_evidence_viewer_audit_cycles
            .into_iter()
            .collect::<BTreeSet<_>>();
        if privacy_cycles.len() != privacy_cycle_count
            || evidence_cycles.len() != evidence_cycle_count
        {
            return Err(GovernancePublishError::other(
                "duplicate published cycle ids in auxiliary checkpoint",
            ));
        }
        let governance_outbox = restore_governance_outbox(
            checkpoint.governance_outbox_next_sequence,
            checkpoint.governance_outbox_entries,
            state_limit,
        )?;
        validate_privacy_release_outbox(
            &privacy_release_ledger,
            &privacy_cycles,
            &governance_outbox,
        )?;
        for entry in governance_outbox
            .entries
            .values()
            .filter(|entry| entry.kind == GovernanceOutboxKindV1::SignedReputationSnapshot)
        {
            let envelope = decode_canonical_signed_reputation_payload(&entry.payload_bytes)?;
            let Some(admitted) = snapshots.get(&envelope.snapshot.snapshot_id) else {
                return Err(GovernancePublishError::other(format!(
                    "pending signed reputation outbox entry {} has no retained admitted envelope",
                    entry.sequence
                )));
            };
            if admitted.envelope != envelope {
                return Err(GovernancePublishError::other(format!(
                    "pending signed reputation outbox entry {} conflicts with retained admitted bytes",
                    entry.sequence
                )));
            }
        }
        if gc_eviction_audit_links
            .values()
            .any(|link| link.outbox_sequence >= governance_outbox.next_sequence)
        {
            return Err(GovernancePublishError::other(
                "GC eviction audit link references an outbox sequence beyond its high-water mark",
            ));
        }
        let reserved_gc_slots = gc_eviction_reserved_outbox_slots(&gc_eviction_intents)?;
        if governance_outbox
            .entries
            .len()
            .checked_add(reserved_gc_slots)
            .is_none_or(|required| required > state_limit)
        {
            return Err(GovernancePublishError::other(format!(
                "auxiliary checkpoint intents reserve {reserved_gc_slots} outbox slots beyond retention limit {state_limit}"
            )));
        }
        let mut pending_linked_gc_sequences = BTreeSet::new();
        for link in gc_eviction_audit_links.values() {
            if let Some(entry) = governance_outbox.entries.get(&link.outbox_sequence) {
                if entry.kind != GovernanceOutboxKindV1::GcAudit
                    || entry.payload_digest != link.outbox_payload_digest
                {
                    return Err(GovernancePublishError::other(format!(
                        "GC eviction audit link {} conflicts with its pending outbox entry",
                        link.intent_sequence
                    )));
                }
                let expected = norito::to_bytes(&gc_eviction_audit_event_from_link(link)?)
                    .map_err(|err| {
                        GovernancePublishError::other(format!(
                            "encode linked GC eviction audit event: {err}"
                        ))
                    })?;
                if entry.payload_bytes != expected {
                    return Err(GovernancePublishError::other(format!(
                        "GC eviction audit link {} payload bytes mismatch",
                        link.intent_sequence
                    )));
                }
                pending_linked_gc_sequences.insert(entry.sequence);
            }
        }
        for entry in governance_outbox
            .entries
            .values()
            .filter(|entry| entry.kind == GovernanceOutboxKindV1::GcAudit)
        {
            let event =
                decode_canonical_governance_payload::<GcAuditEventV1>(&entry.payload_bytes)?;
            if event.payload.blocked_reason.is_none()
                && !pending_linked_gc_sequences.contains(&entry.sequence)
            {
                return Err(GovernancePublishError::other(format!(
                    "pending successful GC audit entry {} lacks eviction linkage",
                    entry.sequence
                )));
            }
        }
        self.por
            .restore_checkpoint(checkpoint.por_tracker)
            .map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid PoR tracker auxiliary checkpoint: {err}"
                ))
            })?;
        *self
            .por_history
            .write()
            .map_err(|_| GovernancePublishError::other("PoR history lock poisoned"))? = por_history;
        *self
            .gc_eviction_intents
            .write()
            .map_err(|_| GovernancePublishError::other("GC eviction intent lock poisoned"))? =
            gc_eviction_intents;
        *self
            .gc_eviction_audit_links
            .write()
            .map_err(|_| GovernancePublishError::other("GC eviction audit link lock poisoned"))? =
            gc_eviction_audit_links;
        *self
            .reputation_snapshots
            .write()
            .map_err(|_| GovernancePublishError::other("reputation snapshot index poisoned"))? =
            snapshots;
        *self
            .latest_reputation_snapshot
            .write()
            .map_err(|_| GovernancePublishError::other("reputation snapshot cache poisoned"))? =
            latest_snapshot;
        *self
            .reputation_events
            .write()
            .map_err(|_| GovernancePublishError::other("reputation event history poisoned"))? =
            reputation_events;
        self.capacity
            .restore_checkpoint(validated_capacity_checkpoint)
            .map_err(|err| {
                GovernancePublishError::other(format!(
                    "failed to install capacity runtime auxiliary checkpoint: {err}"
                ))
            })?;
        let capacity_usage = self.capacity.usage_snapshot();
        if capacity_usage.provider_id.is_some() {
            self.meter.restore_capacity_runtime(
                capacity_usage.committed_total_gib,
                capacity_usage.declaration_window,
                &capacity_usage.outstanding_orders,
            );
            if let Some(record) = self.capacity.active_declaration_record().map_err(|err| {
                GovernancePublishError::other(format!(
                    "failed to reconstruct capacity declaration after restore: {err}"
                ))
            })? {
                self.seed_telemetry_accumulator(&record);
            }
        }
        *self
            .transparency_ledger_source_entries
            .write()
            .map_err(|_| {
                GovernancePublishError::other("transparency source-entry index poisoned")
            })? = transparency_entries;
        *self
            .privacy_aggregate_source_events
            .write()
            .map_err(|_| GovernancePublishError::other("privacy source-event index poisoned"))? =
            privacy_events;
        *self.privacy_source_event_receipts.write().map_err(|_| {
            GovernancePublishError::other("privacy source-event receipt index poisoned")
        })? = privacy_receipts;
        *self.privacy_publish_request_receipts.write().map_err(|_| {
            GovernancePublishError::other("privacy publish-request receipt index poisoned")
        })? = privacy_publish_receipts;
        *self
            .published_privacy_aggregate_cycles
            .write()
            .map_err(|_| GovernancePublishError::other("privacy cycle index poisoned"))? =
            privacy_cycles;
        *self
            .privacy_composition_budget
            .write()
            .map_err(|_| GovernancePublishError::other("privacy composition budget poisoned"))? =
            privacy_composition_budget;
        *self
            .privacy_release_ledger
            .write()
            .map_err(|_| GovernancePublishError::other("privacy release ledger poisoned"))? =
            privacy_release_ledger;
        *self
            .published_evidence_viewer_audit_cycles
            .write()
            .map_err(|_| GovernancePublishError::other("evidence cycle index poisoned"))? =
            evidence_cycles;
        *self
            .governance_outbox
            .write()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))? =
            governance_outbox;
        self.reconcile_privacy_release_anchor()?;
        Ok(())
    }
    fn moderation_quarantine_record_for_object(
        &self,
        quarantine_id: &[u8; 16],
    ) -> Result<ModerationQuarantineRecord, ModerationQuarantineObjectError> {
        self.moderation_screening
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?
            .quarantine_record(quarantine_id)
            .ok_or_else(|| ModerationQuarantineObjectError::UnknownQuarantine {
                quarantine_id_hex: hex::encode(quarantine_id),
            })
    }
    fn moderation_quarantine_object_record_for_viewer(
        &self,
        quarantine_id: &[u8; 16],
    ) -> Result<ModerationQuarantineObjectRecord, ModerationEvidenceViewerError> {
        self.moderation_quarantine_record_for_object(quarantine_id)
            .map_err(moderation_evidence_viewer_error_from_object_error)?;
        self.moderation_quarantine_objects
            .read()
            .map_err(|_| ModerationEvidenceViewerError::StateLockPoisoned)?
            .get(quarantine_id)
            .ok_or_else(|| ModerationEvidenceViewerError::MissingObject {
                quarantine_id_hex: hex::encode(quarantine_id),
            })
    }
    fn validate_moderation_quarantine_object_snapshot_refs(
        &self,
        snapshot: &ModerationQuarantineObjectSnapshot,
    ) -> Result<(), ModerationQuarantineObjectError> {
        let screening = self
            .moderation_screening
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?;
        for record in &snapshot.objects {
            let quarantine = screening
                .quarantine_record(&record.quarantine_id)
                .ok_or_else(|| ModerationQuarantineObjectError::UnknownQuarantine {
                    quarantine_id_hex: hex::encode(record.quarantine_id),
                })?;
            if quarantine.subject_digest != record.payload_digest {
                return Err(ModerationQuarantineObjectError::DigestMismatch {
                    quarantine_id_hex: hex::encode(record.quarantine_id),
                    expected_digest_hex: hex::encode(quarantine.subject_digest),
                    actual_digest_hex: hex::encode(record.payload_digest),
                });
            }
        }
        Ok(())
    }
    fn validate_moderation_screening_snapshot_downstream_refs(
        &self,
        snapshot: &ModerationScreeningSnapshot,
    ) -> Result<(), ModerationScreeningError> {
        let quarantine_ids = snapshot
            .quarantine_records
            .iter()
            .map(|record| record.quarantine_id)
            .collect::<BTreeSet<_>>();
        let objects = self
            .moderation_quarantine_objects
            .read()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?
            .snapshot();
        for object in objects.objects {
            if !quarantine_ids.contains(&object.quarantine_id) {
                return Err(ModerationScreeningError::InvalidSnapshot {
                    message: format!(
                        "screening snapshot removes quarantine `{}` referenced by object `{}`",
                        hex::encode(object.quarantine_id),
                        hex::encode(object.object_id)
                    ),
                });
            }
        }
        let viewer = self
            .moderation_evidence_viewer
            .read()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?
            .snapshot();
        for session in viewer.sessions {
            if !quarantine_ids.contains(&session.quarantine_id) {
                return Err(ModerationScreeningError::InvalidSnapshot {
                    message: format!(
                        "screening snapshot removes quarantine `{}` referenced by viewer session `{}`",
                        hex::encode(session.quarantine_id),
                        hex::encode(session.session_id)
                    ),
                });
            }
        }
        Ok(())
    }
    fn validate_moderation_quarantine_snapshot_viewer_refs(
        &self,
        snapshot: &ModerationQuarantineObjectSnapshot,
    ) -> Result<(), ModerationQuarantineObjectError> {
        let objects = snapshot
            .objects
            .iter()
            .map(|record| (record.quarantine_id, record))
            .collect::<BTreeMap<_, _>>();
        let viewer = self
            .moderation_evidence_viewer
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?
            .snapshot();
        for session in viewer.sessions {
            let Some(object) = objects.get(&session.quarantine_id) else {
                return Err(ModerationQuarantineObjectError::InvalidSnapshot {
                    message: format!(
                        "object snapshot removes quarantine `{}` referenced by viewer session `{}`",
                        hex::encode(session.quarantine_id),
                        hex::encode(session.session_id)
                    ),
                });
            };
            if session.object_id != object.object_id
                || session.evidence_digest != object.payload_digest
            {
                return Err(ModerationQuarantineObjectError::InvalidSnapshot {
                    message: format!(
                        "viewer session `{}` does not match candidate object metadata",
                        hex::encode(session.session_id)
                    ),
                });
            }
        }
        Ok(())
    }
    fn validate_moderation_evidence_viewer_snapshot_refs(
        &self,
        snapshot: &ModerationEvidenceViewerSnapshot,
    ) -> Result<(), ModerationEvidenceViewerError> {
        for session in &snapshot.sessions {
            let object =
                self.moderation_quarantine_object_record_for_viewer(&session.quarantine_id)?;
            if session.object_id != object.object_id
                || session.evidence_digest != object.payload_digest
            {
                return Err(ModerationEvidenceViewerError::InvalidSnapshot {
                    message: format!(
                        "evidence viewer session `{}` does not match local quarantine object metadata",
                        hex::encode(session.session_id)
                    ),
                });
            }
        }
        Ok(())
    }
    fn resolve_moderation_quarantine_object_path(
        &self,
        root: &Path,
        record: &ModerationQuarantineObjectRecord,
    ) -> Result<PathBuf, ModerationQuarantineObjectError> {
        validate_relative_object_path(&record.envelope_path)
            .map_err(|message| ModerationQuarantineObjectError::InvalidSnapshot { message })?;
        Ok(root.join(&record.envelope_path))
    }
    fn read_moderation_quarantine_object_envelope(
        &self,
        root: &Path,
        record: &ModerationQuarantineObjectRecord,
    ) -> Result<
        (PathBuf, ModerationQuarantineObjectEnvelopeV1, Vec<u8>),
        ModerationQuarantineObjectError,
    > {
        let envelope_path = self.resolve_moderation_quarantine_object_path(root, record)?;
        let envelope_bytes = read_local_checkpoint_bounded(
            &envelope_path,
            self.config.runtime_retention().checkpoint_max_bytes(),
        )
        .map_err(|error| ModerationQuarantineObjectError::Io {
            path: envelope_path.display().to_string(),
            message: error.to_string(),
        })?
        .ok_or_else(|| ModerationQuarantineObjectError::MissingObject {
            quarantine_id_hex: hex::encode(record.quarantine_id),
        })?;
        let envelope =
            norito::decode_from_bytes::<ModerationQuarantineObjectEnvelopeV1>(&envelope_bytes)
                .map_err(|error| ModerationQuarantineObjectError::Codec {
                    message: error.to_string(),
                })?;
        let canonical = norito::to_bytes(&envelope).map_err(|error| {
            ModerationQuarantineObjectError::Codec {
                message: error.to_string(),
            }
        })?;
        if canonical != envelope_bytes {
            return Err(ModerationQuarantineObjectError::AuthenticationFailed {
                quarantine_id_hex: hex::encode(record.quarantine_id),
            });
        }
        Ok((envelope_path, envelope, envelope_bytes))
    }
    fn recover_unindexed_moderation_quarantine_envelope(
        &self,
        root: &Path,
        path: &Path,
    ) -> Result<bool, ModerationQuarantineObjectError> {
        let bytes = read_local_checkpoint_bounded(
            path,
            self.config.runtime_retention().checkpoint_max_bytes(),
        )
        .map_err(|error| ModerationQuarantineObjectError::Io {
            path: path.display().to_string(),
            message: error.to_string(),
        })?
        .ok_or_else(|| ModerationQuarantineObjectError::Io {
            path: path.display().to_string(),
            message: "unindexed envelope disappeared during startup audit".to_owned(),
        })?;
        let envelope =
            match norito::decode_from_bytes::<ModerationQuarantineObjectEnvelopeV1>(&bytes) {
                Ok(envelope) => envelope,
                Err(_) => return Ok(false),
            };
        let canonical = norito::to_bytes(&envelope).map_err(|error| {
            ModerationQuarantineObjectError::Codec {
                message: error.to_string(),
            }
        })?;
        if canonical != bytes {
            return Ok(false);
        }
        if validate_quarantine_object_envelope(&envelope).is_err() {
            return Ok(false);
        }
        let expected_path = root.join(moderation_quarantine_object_relative_path(
            envelope.quarantine_id,
            envelope.object_id,
        ));
        if expected_path != path {
            return Ok(false);
        }
        remove_local_checkpoint_file_durably(path).map_err(|error| {
            ModerationQuarantineObjectError::Io {
                path: path.display().to_string(),
                message: format!("remove unindexed crash-recovery envelope: {error}"),
            }
        })?;
        iroha_logger::warn!(
            quarantine_id = %hex::encode(envelope.quarantine_id),
            object_id = %hex::encode(envelope.object_id),
            "removed unindexed moderation quarantine envelope left by an interrupted commit"
        );
        Ok(true)
    }
    fn prepare_gc_eviction_intent(
        &self,
        _lock_guards: (
            &std::sync::MutexGuard<'_, ()>,
            &std::sync::MutexGuard<'_, ()>,
        ),
        storage: &StorageBackend,
        target: &StoredManifest,
        provider_id: [u8; 32],
        audit_timestamp_unix: u64,
        reason: &str,
    ) -> Result<GcEvictionIntentV1, GovernancePublishError> {
        let snapshot = gc_storage_snapshot(storage)?;
        let authoritative_target = snapshot
            .manifests
            .iter()
            .find(|manifest| manifest.manifest_id() == target.manifest_id())
            .ok_or_else(|| {
                GovernancePublishError::other(format!(
                    "GC target {} disappeared before intent preparation",
                    target.manifest_id()
                ))
            })?;
        if gc_manifest_identity_digest(authoritative_target) != gc_manifest_identity_digest(target)
        {
            return Err(GovernancePublishError::other(format!(
                "GC target {} changed canonical storage identity before intent preparation",
                target.manifest_id()
            )));
        }
        for index in 0..authoritative_target.chunk_count() {
            let chunk = authoritative_target.chunk(index).ok_or_else(|| {
                GovernancePublishError::other("GC target chunk metadata is incomplete")
            })?;
            let refcount = snapshot
                .chunk_refcounts
                .iter()
                .find(|entry| entry.digest == chunk.digest)
                .ok_or_else(|| {
                    GovernancePublishError::other("GC target chunk lacks a storage refcount")
                })?;
            if refcount.count > 1 {
                return Err(GovernancePublishError::other(format!(
                    "GC target {} acquired shared chunks before intent preparation",
                    target.manifest_id()
                )));
            }
        }
        let storage_after = gc_expected_post_storage_identity(&snapshot, authoritative_target)?;
        let checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        let previous_intents = self
            .gc_eviction_intents
            .read()
            .map_err(|_| GovernancePublishError::other("GC eviction intent lock poisoned"))?
            .clone();
        if !previous_intents.entries.is_empty() {
            return Err(GovernancePublishError::other(
                "unreconciled GC eviction intent blocks a new eviction",
            ));
        }
        let sequence = previous_intents.next_sequence;
        let next_sequence = sequence.checked_add(1).ok_or_else(|| {
            GovernancePublishError::other("GC eviction intent sequence exhausted")
        })?;
        let mut intent = GcEvictionIntentV1 {
            version: GC_EVICTION_INTENT_VERSION_V1,
            sequence,
            manifest_id: authoritative_target.manifest_id().to_owned(),
            manifest_digest: *authoritative_target.manifest_digest(),
            manifest_identity_digest: gc_manifest_identity_digest(authoritative_target),
            provider_id,
            audit_timestamp_unix,
            reason: reason.to_owned(),
            expected_freed_bytes: authoritative_target.content_length(),
            storage_before: snapshot.identity,
            storage_after,
            reserved_outbox_slots: GC_EVICTION_RESERVED_OUTBOX_SLOTS,
            binding_digest: [0; 32],
        };
        intent.binding_digest = gc_eviction_intent_binding_digest(&intent)?;
        validate_gc_eviction_intent(&intent)?;
        let pending_outbox_entries = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
            .entries
            .len();
        let limit = self.config.runtime_retention().state_entry_limit();
        let required = pending_outbox_entries
            .checked_add(usize::from(intent.reserved_outbox_slots))
            .ok_or_else(|| GovernancePublishError::other("outbox reservation count overflow"))?;
        if required > limit {
            return Err(GovernancePublishError::other(format!(
                "GC eviction requires one reserved governance outbox slot, but {pending_outbox_entries} entries already consume limit {limit}"
            )));
        }
        let previous_links = self
            .gc_eviction_audit_links
            .read()
            .map_err(|_| GovernancePublishError::other("GC eviction audit link lock poisoned"))?
            .clone();
        let pending_sequences = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
            .entries
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut links = previous_links.clone();
        while links.len() >= limit {
            let candidate = links
                .iter()
                .find_map(|(sequence, link)| {
                    (!pending_sequences.contains(&link.outbox_sequence)).then_some(*sequence)
                })
                .ok_or_else(|| {
                    GovernancePublishError::other(format!(
                        "GC eviction audit linkage retention exhausted (limit {limit})"
                    ))
                })?;
            links.remove(&candidate);
        }
        let mut runtime = previous_intents.clone();
        runtime.next_sequence = next_sequence;
        runtime.entries.insert(sequence, intent.clone());
        let mut intent_runtime = self
            .gc_eviction_intents
            .write()
            .map_err(|_| GovernancePublishError::other("GC eviction intent lock poisoned"))?;
        let mut audit_links = self
            .gc_eviction_audit_links
            .write()
            .map_err(|_| GovernancePublishError::other("GC eviction audit link lock poisoned"))?;
        *intent_runtime = runtime;
        *audit_links = links;
        drop(audit_links);
        drop(intent_runtime);
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            let mut intent_runtime = self.gc_eviction_intents.write().map_err(|_| {
                GovernancePublishError::other("GC eviction intent rollback lock poisoned")
            })?;
            let mut audit_links = self.gc_eviction_audit_links.write().map_err(|_| {
                GovernancePublishError::other("GC eviction link rollback lock poisoned")
            })?;
            *intent_runtime = previous_intents;
            *audit_links = previous_links;
            return Err(GovernancePublishError::other(err.to_string()));
        }
        drop(checkpoint_guard);
        Ok(intent)
    }
    fn append_gc_eviction_audit_unlocked(
        &self,
        intent: &GcEvictionIntentV1,
    ) -> Result<(), GovernancePublishError> {
        validate_gc_eviction_intent(intent)?;
        if self
            .gc_eviction_audit_links
            .read()
            .map_err(|_| GovernancePublishError::other("GC eviction audit link lock poisoned"))?
            .contains_key(&intent.sequence)
        {
            return Err(GovernancePublishError::other(format!(
                "GC eviction intent {} already has publication linkage while still pending",
                intent.sequence
            )));
        }
        let expected_outbox_sequence = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
            .next_sequence;
        let payload = gc_eviction_payload(intent);
        let payload_digest = gc_audit_payload_digest_v1(&payload)
            .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        let audit = GcAuditEventV1 {
            version: GC_AUDIT_EVENT_VERSION_V1,
            header: SorafsAuditHeaderV1 {
                sequence: expected_outbox_sequence,
                occurred_at_unix: intent.audit_timestamp_unix,
                signer: GC_AUDIT_SIGNER_V1.to_owned(),
                payload_digest,
            },
            payload,
        };
        audit
            .validate()
            .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        let encoded = norito::to_bytes(&audit).map_err(|err| {
            GovernancePublishError::other(format!("encode GC eviction audit event: {err}"))
        })?;
        let outbox_payload_digest = *blake3::hash(&encoded).as_bytes();
        let (outbox_sequence, inserted) =
            self.enqueue_gc_eviction_governance_outbox_unlocked(encoded)?;
        if !inserted || outbox_sequence != expected_outbox_sequence {
            return Err(GovernancePublishError::other(
                "GC eviction audit collided with an existing outbox entry",
            ));
        }
        let mut link = GcEvictionAuditLinkV1 {
            version: GC_EVICTION_AUDIT_LINK_VERSION_V1,
            intent_sequence: intent.sequence,
            outbox_sequence,
            manifest_id: intent.manifest_id.clone(),
            manifest_digest: intent.manifest_digest,
            provider_id: intent.provider_id,
            occurred_at_unix: intent.audit_timestamp_unix,
            freed_bytes: intent.expected_freed_bytes,
            reason: intent.reason.clone(),
            storage_gc_evictions_total: intent.storage_after.gc_evictions_total,
            payload_digest,
            outbox_payload_digest,
            binding_digest: [0; 32],
        };
        link.binding_digest = gc_eviction_audit_link_binding_digest(&link);
        validate_gc_eviction_audit_link(&link)?;
        let replaced = self
            .gc_eviction_audit_links
            .write()
            .map_err(|_| GovernancePublishError::other("GC eviction audit link lock poisoned"))?
            .insert(intent.sequence, link);
        if replaced.is_some() {
            return Err(GovernancePublishError::other(
                "GC eviction audit link sequence changed during finalization",
            ));
        }
        Ok(())
    }
    fn restore_gc_publication_runtime(
        &self,
        intents: GcEvictionIntentRuntime,
        links: BTreeMap<u64, GcEvictionAuditLinkV1>,
        outbox: GovernanceOutboxRuntime,
    ) -> Result<(), GovernancePublishError> {
        let mut intent_runtime = self.gc_eviction_intents.write().map_err(|_| {
            GovernancePublishError::other("GC eviction intent rollback lock poisoned")
        })?;
        let mut audit_links = self.gc_eviction_audit_links.write().map_err(|_| {
            GovernancePublishError::other("GC eviction link rollback lock poisoned")
        })?;
        let mut governance_outbox = self.governance_outbox.write().map_err(|_| {
            GovernancePublishError::other("governance outbox rollback lock poisoned")
        })?;
        *intent_runtime = intents;
        *audit_links = links;
        *governance_outbox = outbox;
        Ok(())
    }
    fn settle_gc_eviction_intent_against_storage(
        &self,
        _gc_guard: &std::sync::MutexGuard<'_, ()>,
        _drain_guard: &std::sync::MutexGuard<'_, ()>,
        storage: &StorageBackend,
        intent: &GcEvictionIntentV1,
        fail_closed_on_error: bool,
    ) -> Result<GcIntentDisposition, GovernancePublishError> {
        validate_gc_eviction_intent(intent)?;
        let checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        if fail_closed_on_error {
            self.ensure_durability_healthy()
                .map_err(GovernancePublishError::other)?;
        }
        let reconciliation_error = |message: String| {
            if fail_closed_on_error {
                self.mark_durability_unhealthy(message.clone());
            }
            GovernancePublishError::other(message)
        };
        let snapshot = gc_storage_snapshot(storage).map_err(|err| {
            reconciliation_error(format!(
                "cannot classify storage generation for GC intent {}: {err}",
                intent.sequence
            ))
        })?;
        let target = snapshot
            .manifests
            .iter()
            .find(|manifest| manifest.manifest_id() == intent.manifest_id);
        let disposition = if snapshot.identity == intent.storage_before {
            let target = target.ok_or_else(|| {
                reconciliation_error(
                    "pre-domain GC generation is missing its intended manifest".to_owned(),
                )
            })?;
            if gc_manifest_identity_digest(target) != intent.manifest_identity_digest
                || target.manifest_digest() != &intent.manifest_digest
            {
                return Err(reconciliation_error(
                    "pre-domain GC manifest conflicts with the persisted intent".to_owned(),
                ));
            }
            GcIntentDisposition::DiscardedPreDomain
        } else if snapshot.identity == intent.storage_after {
            if target.is_some() {
                return Err(reconciliation_error(
                    "post-domain GC generation still contains the evicted manifest".to_owned(),
                ));
            }
            GcIntentDisposition::FinalizedCommitted
        } else {
            return Err(reconciliation_error(format!(
                "storage generation drift prevents deterministic reconciliation of GC intent {}",
                intent.sequence
            )));
        };
        let previous_intents = self
            .gc_eviction_intents
            .read()
            .map_err(|_| GovernancePublishError::other("GC eviction intent lock poisoned"))?
            .clone();
        if previous_intents.entries.get(&intent.sequence) != Some(intent) {
            return Err(reconciliation_error(format!(
                "GC eviction intent {} changed before reconciliation",
                intent.sequence
            )));
        }
        let previous_links = self
            .gc_eviction_audit_links
            .read()
            .map_err(|_| GovernancePublishError::other("GC eviction audit link lock poisoned"))?
            .clone();
        let previous_outbox = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
            .clone();
        let mutation = (|| {
            if disposition == GcIntentDisposition::FinalizedCommitted {
                self.append_gc_eviction_audit_unlocked(intent)?;
            }
            let removed = self
                .gc_eviction_intents
                .write()
                .map_err(|_| GovernancePublishError::other("GC eviction intent lock poisoned"))?
                .entries
                .remove(&intent.sequence);
            if removed.as_ref() != Some(intent) {
                return Err(GovernancePublishError::other(format!(
                    "GC eviction intent {} changed during finalization",
                    intent.sequence
                )));
            }
            Ok(())
        })();
        if let Err(err) = mutation {
            if let Err(rollback) = self.restore_gc_publication_runtime(
                previous_intents,
                previous_links,
                previous_outbox,
            ) {
                let reason = self.record_unrecoverable_rollback(
                    "failed to roll back GC publication finalization",
                    rollback,
                );
                return Err(GovernancePublishError::other(reason));
            }
            if fail_closed_on_error && disposition == GcIntentDisposition::FinalizedCommitted {
                self.mark_durability_unhealthy(err.to_string());
            }
            return Err(err);
        }
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            if let Err(rollback) = self.restore_gc_publication_runtime(
                previous_intents,
                previous_links,
                previous_outbox,
            ) {
                let reason = self.record_unrecoverable_rollback(
                    "failed to roll back GC publication checkpoint failure",
                    rollback,
                );
                return Err(GovernancePublishError::other(reason));
            }
            let message =
                format!("GC storage state may have committed without its audit publication: {err}");
            if fail_closed_on_error && disposition == GcIntentDisposition::FinalizedCommitted {
                self.mark_durability_unhealthy(message.clone());
            }
            return Err(GovernancePublishError::other(message));
        }
        drop(checkpoint_guard);
        Ok(disposition)
    }
    fn evict_manifest_with_gc_audit(
        &self,
        gc_guard: &std::sync::MutexGuard<'_, ()>,
        storage: &StorageBackend,
        target: &StoredManifest,
        provider_id: [u8; 32],
        audit_timestamp_unix: u64,
        reason: &str,
    ) -> Result<GcEvictionTransactionOutcome, GovernancePublishError> {
        let drain_guard = self
            .governance_outbox_drain_lock
            .lock()
            .map_err(|_| GovernancePublishError::other("governance outbox drain lock poisoned"))?;
        let intent = self.prepare_gc_eviction_intent(
            (gc_guard, &drain_guard),
            storage,
            target,
            provider_id,
            audit_timestamp_unix,
            reason,
        )?;
        let before_eviction = gc_storage_snapshot(storage).map_err(|err| {
            let message = format!(
                "cannot revalidate storage generation for GC intent {}: {err}",
                intent.sequence
            );
            self.mark_durability_unhealthy(message.clone());
            GovernancePublishError::other(message)
        })?;
        if before_eviction.identity != intent.storage_before
            || before_eviction
                .manifests
                .iter()
                .find(|manifest| manifest.manifest_id() == intent.manifest_id)
                .is_none_or(|manifest| {
                    gc_manifest_identity_digest(manifest) != intent.manifest_identity_digest
                })
        {
            let message = format!(
                "storage generation changed after preparing GC intent {}",
                intent.sequence
            );
            self.mark_durability_unhealthy(message.clone());
            return Err(GovernancePublishError::other(message));
        }
        let freed_bytes = match storage.evict_manifest(&intent.manifest_id) {
            Ok(freed_bytes) => freed_bytes,
            Err(storage_error) => {
                let current = gc_storage_snapshot_unchecked(storage);
                if storage.ensure_durability_healthy().is_ok()
                    && current.as_ref().is_ok_and(|snapshot| {
                        snapshot.identity == intent.storage_before
                            && snapshot.manifests.iter().any(|manifest| {
                                manifest.manifest_id() == intent.manifest_id
                                    && gc_manifest_identity_digest(manifest)
                                        == intent.manifest_identity_digest
                            })
                    })
                {
                    self.settle_gc_eviction_intent_against_storage(
                        gc_guard,
                        &drain_guard,
                        storage,
                        &intent,
                        false,
                    )?;
                    return Err(GovernancePublishError::other(storage_error.to_string()));
                }
                let message = format!(
                    "GC eviction result for intent {} is ambiguous after storage error: {storage_error}",
                    intent.sequence
                );
                self.mark_durability_unhealthy(message.clone());
                return Err(GovernancePublishError::other(message));
            }
        };
        if freed_bytes != intent.expected_freed_bytes {
            let message = format!(
                "GC eviction intent {} freed {freed_bytes} bytes, expected {}",
                intent.sequence, intent.expected_freed_bytes
            );
            self.mark_durability_unhealthy(message.clone());
            return Err(GovernancePublishError::other(message));
        }
        let disposition = self.settle_gc_eviction_intent_against_storage(
            gc_guard,
            &drain_guard,
            storage,
            &intent,
            true,
        )?;
        if disposition != GcIntentDisposition::FinalizedCommitted {
            let message = format!(
                "GC intent {} reverted to a pre-domain generation after eviction",
                intent.sequence
            );
            self.mark_durability_unhealthy(message.clone());
            return Err(GovernancePublishError::other(message));
        }
        drop(drain_guard);
        let publish_error = self
            .flush_governance_outbox()
            .err()
            .map(|err| err.to_string());
        Ok(GcEvictionTransactionOutcome {
            freed_bytes,
            publish_error,
        })
    }
    fn validate_gc_eviction_links_against_storage(
        &self,
        storage: &StorageBackend,
    ) -> Result<(), GovernancePublishError> {
        let (_, current_evictions_total) = storage.gc_counters();
        let links = self
            .gc_eviction_audit_links
            .read()
            .map_err(|_| GovernancePublishError::other("GC eviction audit link lock poisoned"))?;
        let mut previous_counter = None;
        for link in links.values() {
            validate_gc_eviction_audit_link(link)?;
            if link.storage_gc_evictions_total > current_evictions_total
                || previous_counter
                    .is_some_and(|previous| previous >= link.storage_gc_evictions_total)
            {
                return Err(GovernancePublishError::other(
                    "GC eviction audit links conflict with storage counter generation",
                ));
            }
            previous_counter = Some(link.storage_gc_evictions_total);
        }
        Ok(())
    }
    fn publish_gc_audit_event(
        &self,
        payload: GcAuditPayloadV1,
    ) -> Result<(), GovernancePublishError> {
        if payload.blocked_reason.is_none() {
            return Err(GovernancePublishError::other(
                "successful GC audits must be committed through an eviction transaction",
            ));
        }
        let manifest_digest = payload.manifest_digest;
        self.enqueue_sequenced_governance_outbox(GovernanceOutboxKindV1::GcAudit, |sequence| {
            let payload_digest = gc_audit_payload_digest_v1(&payload)
                .map_err(|err| GovernancePublishError::other(err.to_string()))?;
            let header = SorafsAuditHeaderV1 {
                sequence,
                occurred_at_unix: payload.evicted_at_unix,
                signer: GC_AUDIT_SIGNER_V1.to_owned(),
                payload_digest,
            };
            let audit_event = GcAuditEventV1 {
                version: GC_AUDIT_EVENT_VERSION_V1,
                header,
                payload,
            };
            audit_event
                .validate()
                .map_err(|err| GovernancePublishError::other(err.to_string()))?;
            norito::to_bytes(&audit_event).map_err(|err| {
                GovernancePublishError::other(format!("encode GC audit event: {err}"))
            })
        })?;
        self.flush_governance_outbox().map_err(|err| {
            GovernancePublishError::other(format!(
                "GC audit for manifest {} remains durably pending: {err}",
                hex::encode(manifest_digest)
            ))
        })?;
        Ok(())
    }
    fn publish_reconciliation_report(&self, report: &SorafsReconciliationReportV1) {
        let encoded = match norito::to_bytes(report) {
            Ok(encoded) => encoded,
            Err(err) => {
                iroha_logger::error!(%err, "failed to encode reconciliation report");
                return;
            }
        };
        if let Err(err) =
            self.enqueue_governance_outbox(GovernanceOutboxKindV1::ReconciliationReport, encoded)
        {
            iroha_logger::error!(
                %err,
                "failed to durably queue reconciliation report"
            );
            return;
        }
        if let Err(err) = self.flush_governance_outbox() {
            iroha_logger::warn!(
                %err,
                "reconciliation report remains pending in the governance outbox"
            );
        }
    }
    /// Run a GC sweep against expired manifests using one complete finalized
    /// native repair-ledger projection.
    ///
    /// The caller must collect `repair_projection` from one immutable query
    /// view. A partial or provider-filtered task page cannot be constructed as
    /// this projection and therefore cannot authorize local deletion.
    pub fn run_gc_once(
        &self,
        now_unix: u64,
        repair_projection: &RepairLedgerTaskProjectionV1,
    ) -> GcSweepReport {
        const REASON_LIMIT_REACHED: &str = "limit_reached";
        const RESULT_SUCCESS: &str = "success";
        const RESULT_ERROR: &str = "error";
        let mut report = GcSweepReport::default();
        if !self.gc_config.enabled() || now_unix == 0 {
            return report;
        }
        let Some(storage) = self.storage.as_ref() else {
            report.errors = report.errors.saturating_add(1);
            iroha_logger::warn!("GC sweep skipped: storage backend disabled");
            global_or_default().inc_sorafs_gc_runs(RESULT_ERROR);
            global_sorafs_gc_otel().record_run(RESULT_ERROR);
            return report;
        };
        if let Err(err) = storage.ensure_durability_healthy() {
            report.errors = report.errors.saturating_add(1);
            iroha_logger::error!(%err, "GC sweep skipped: storage backend fail-stopped");
            global_or_default().inc_sorafs_gc_runs(RESULT_ERROR);
            global_sorafs_gc_otel().record_run(RESULT_ERROR);
            return report;
        }
        let gc_guard = match self.gc_mutation_lock.lock() {
            Ok(guard) => guard,
            Err(_) => {
                report.errors = report.errors.saturating_add(1);
                iroha_logger::error!("GC sweep skipped: mutation lock poisoned");
                global_or_default().inc_sorafs_gc_runs(RESULT_ERROR);
                global_sorafs_gc_otel().record_run(RESULT_ERROR);
                return report;
            }
        };
        let grace_secs = self.gc_config.retention_grace_secs();
        let max_deletions = self.gc_config.max_deletions_per_run() as usize;
        if max_deletions == 0 {
            global_or_default().inc_sorafs_gc_runs(RESULT_SUCCESS);
            global_sorafs_gc_otel().record_run(RESULT_SUCCESS);
            return report;
        }
        let Some(provider_id) = self.capacity_usage().provider_id else {
            report.errors = report.errors.saturating_add(1);
            iroha_logger::error!(
                "GC sweep skipped: provider identity is unavailable for repair-ledger filtering"
            );
            global_or_default().inc_sorafs_gc_runs(RESULT_ERROR);
            global_sorafs_gc_otel().record_run(RESULT_ERROR);
            return report;
        };
        let active_repair_tasks = repair_projection
            .active_tasks_for_provider(provider_id)
            .collect::<Vec<_>>();
        let mut expired = Vec::new();
        let mut expired_count = 0u64;
        let mut oldest_expired_age: Option<u64> = None;
        for manifest in storage.manifests() {
            let retention_epoch = manifest.retention_epoch();
            if retention_epoch == 0 {
                continue;
            }
            let expires_at = retention_epoch.saturating_add(grace_secs);
            if now_unix < expires_at {
                continue;
            }
            expired_count = expired_count.saturating_add(1);
            let age_secs = now_unix.saturating_sub(expires_at);
            oldest_expired_age =
                Some(oldest_expired_age.map_or(age_secs, |prev| prev.max(age_secs)));
            expired.push(manifest);
        }
        expired.sort_by(|left, right| {
            left.retention_epoch()
                .cmp(&right.retention_epoch())
                .then(left.manifest_id().cmp(right.manifest_id()))
        });
        let mut evicted_count = 0usize;
        for manifest in expired {
            if evicted_count >= max_deletions {
                report.skipped.push(GcSkip {
                    manifest_id: manifest.manifest_id().to_owned(),
                    reason: REASON_LIMIT_REACHED.to_string(),
                });
                break;
            }
            let digest = *manifest.manifest_digest();
            if active_repair_tasks
                .iter()
                .any(|task| task.manifest_digest == digest)
            {
                report.skipped.push(GcSkip {
                    manifest_id: manifest.manifest_id().to_owned(),
                    reason: GC_AUDIT_BLOCKED_REPAIR_ACTIVE_V1.to_string(),
                });
                iroha_logger::warn!(
                    manifest_id = %manifest.manifest_id(),
                    "GC retention blocked by active repair tasks"
                );
                global_or_default().inc_sorafs_gc_blocked(GC_AUDIT_BLOCKED_REPAIR_ACTIVE_V1);
                global_sorafs_gc_otel().record_blocked(GC_AUDIT_BLOCKED_REPAIR_ACTIVE_V1);
                let payload = GcAuditPayloadV1 {
                    version: GC_AUDIT_PAYLOAD_VERSION_V1,
                    manifest_digest: digest,
                    provider_id,
                    evicted_at_unix: now_unix,
                    freed_bytes: 0,
                    reason: GC_AUDIT_REASON_RETENTION_EXPIRED_V1.to_string(),
                    blocked_reason: Some(GC_AUDIT_BLOCKED_REPAIR_ACTIVE_V1.to_string()),
                };
                if let Err(err) = self.publish_gc_audit_event(payload) {
                    report.errors = report.errors.saturating_add(1);
                    iroha_logger::error!(
                        %err,
                        manifest_id = %manifest.manifest_id(),
                        "GC blocked outcome could not be durably audited"
                    );
                    break;
                }
                continue;
            }
            if manifest
                .retention_source()
                .and_then(|source| source.deal_end_epoch)
                .is_some_and(|deal_end| deal_end > now_unix)
            {
                report.skipped.push(GcSkip {
                    manifest_id: manifest.manifest_id().to_owned(),
                    reason: GC_AUDIT_BLOCKED_DEAL_ACTIVE_V1.to_string(),
                });
                iroha_logger::warn!(
                    manifest_id = %manifest.manifest_id(),
                    "GC retention blocked by active deal window"
                );
                global_or_default().inc_sorafs_gc_blocked(GC_AUDIT_BLOCKED_DEAL_ACTIVE_V1);
                global_sorafs_gc_otel().record_blocked(GC_AUDIT_BLOCKED_DEAL_ACTIVE_V1);
                let payload = GcAuditPayloadV1 {
                    version: GC_AUDIT_PAYLOAD_VERSION_V1,
                    manifest_digest: digest,
                    provider_id,
                    evicted_at_unix: now_unix,
                    freed_bytes: 0,
                    reason: GC_AUDIT_REASON_RETENTION_EXPIRED_V1.to_string(),
                    blocked_reason: Some(GC_AUDIT_BLOCKED_DEAL_ACTIVE_V1.to_string()),
                };
                if let Err(err) = self.publish_gc_audit_event(payload) {
                    report.errors = report.errors.saturating_add(1);
                    iroha_logger::error!(
                        %err,
                        manifest_id = %manifest.manifest_id(),
                        "GC blocked outcome could not be durably audited"
                    );
                    break;
                }
                continue;
            }
            match storage.manifest_has_shared_chunks(manifest.manifest_id()) {
                Ok(true) => {
                    report.skipped.push(GcSkip {
                        manifest_id: manifest.manifest_id().to_owned(),
                        reason: GC_AUDIT_BLOCKED_SHARED_CHUNKS_V1.to_string(),
                    });
                    iroha_logger::warn!(
                        manifest_id = %manifest.manifest_id(),
                        "GC retention blocked by shared chunks"
                    );
                    global_or_default().inc_sorafs_gc_blocked(GC_AUDIT_BLOCKED_SHARED_CHUNKS_V1);
                    global_sorafs_gc_otel().record_blocked(GC_AUDIT_BLOCKED_SHARED_CHUNKS_V1);
                    let payload = GcAuditPayloadV1 {
                        version: GC_AUDIT_PAYLOAD_VERSION_V1,
                        manifest_digest: digest,
                        provider_id,
                        evicted_at_unix: now_unix,
                        freed_bytes: 0,
                        reason: GC_AUDIT_REASON_RETENTION_EXPIRED_V1.to_string(),
                        blocked_reason: Some(GC_AUDIT_BLOCKED_SHARED_CHUNKS_V1.to_string()),
                    };
                    if let Err(err) = self.publish_gc_audit_event(payload) {
                        report.errors = report.errors.saturating_add(1);
                        iroha_logger::error!(
                            %err,
                            manifest_id = %manifest.manifest_id(),
                            "GC blocked outcome could not be durably audited"
                        );
                        break;
                    }
                    continue;
                }
                Ok(false) => {}
                Err(err) => {
                    report.errors = report.errors.saturating_add(1);
                    iroha_logger::warn!(
                        %err,
                        manifest_id = %manifest.manifest_id(),
                        "GC eviction skipped: failed to inspect shared chunks"
                    );
                    continue;
                }
            }
            let reason = GC_AUDIT_REASON_RETENTION_EXPIRED_V1;
            let transaction = match self.evict_manifest_with_gc_audit(
                &gc_guard,
                storage,
                &manifest,
                provider_id,
                now_unix,
                reason,
            ) {
                Ok(outcome) => outcome,
                Err(err) => {
                    report.errors = report.errors.saturating_add(1);
                    iroha_logger::warn!(
                        %err,
                        manifest_id = %manifest.manifest_id(),
                        "GC eviction transaction failed"
                    );
                    break;
                }
            };
            let freed_bytes = transaction.freed_bytes;
            if let Some(err) = transaction.publish_error {
                report.errors = report.errors.saturating_add(1);
                iroha_logger::warn!(
                    error = %err,
                    manifest_id = %manifest.manifest_id(),
                    "GC eviction audit remains durably pending"
                );
            }
            evicted_count += 1;
            report.freed_bytes = report.freed_bytes.saturating_add(freed_bytes);
            report.evictions.push(GcEviction {
                manifest_id: manifest.manifest_id().to_owned(),
                manifest_digest: digest,
                retention_epoch: manifest.retention_epoch(),
                freed_bytes,
                reason: reason.to_string(),
            });
            global_or_default().inc_sorafs_gc_evictions(reason);
            global_or_default().add_sorafs_gc_freed_bytes(reason, freed_bytes);
            global_sorafs_gc_otel().record_eviction(reason, freed_bytes);
        }
        global_or_default()
            .set_sorafs_gc_expired_snapshot(expired_count, oldest_expired_age.unwrap_or(0));
        let result = if report.errors == 0 {
            RESULT_SUCCESS
        } else {
            RESULT_ERROR
        };
        global_or_default().inc_sorafs_gc_runs(result);
        global_sorafs_gc_otel().record_run(result);
        self.schedulers
            .update_storage_bytes(storage.total_bytes(), self.config.max_capacity_bytes().0);
        report
    }
    /// Run a reconciliation snapshot across repair and GC state.
    pub fn run_reconciliation_once(
        &self,
        now_unix: u64,
        repair_projection: &RepairLedgerTaskProjectionV1,
    ) -> Result<SorafsReconciliationReportV1, ReconciliationError> {
        let result = self.compute_reconciliation_report(now_unix, repair_projection);
        match &result {
            Ok(report) => {
                self.publish_reconciliation_report(report);
                global_or_default().inc_sorafs_reconciliation_runs("success");
                global_or_default()
                    .set_sorafs_reconciliation_divergence_count(u64::from(report.divergence_count));
                global_sorafs_reconciliation_otel().record_run("success");
                global_sorafs_reconciliation_otel()
                    .record_divergence(u64::from(report.divergence_count));
            }
            Err(_) => {
                global_or_default().inc_sorafs_reconciliation_runs("error");
                global_sorafs_reconciliation_otel().record_run("error");
            }
        }
        result
    }
    fn compute_reconciliation_report(
        &self,
        now_unix: u64,
        repair_projection: &RepairLedgerTaskProjectionV1,
    ) -> Result<SorafsReconciliationReportV1, ReconciliationError> {
        if now_unix == 0 {
            return Err(ReconciliationError::InvalidTimestamp);
        }
        let Some(storage) = self.storage.as_ref() else {
            return Err(ReconciliationError::StorageDisabled);
        };
        let provider_id = self
            .capacity_usage()
            .provider_id
            .ok_or(ReconciliationError::ProviderBindingUnavailable)?;
        let repair_snapshot = reconciliation::RepairReconciliationSnapshot {
            version: reconciliation::RECONCILIATION_SNAPSHOT_VERSION_V1,
            finalized_cursor: repair_projection.finalized_cursor(),
            tasks: repair_projection.tasks().to_vec(),
        };
        let repair_snapshot_hash = reconciliation::hash_snapshot(&repair_snapshot)?;
        let manifests = storage.manifests();
        let mut retention_entries = manifests
            .iter()
            .map(|manifest| reconciliation::RetentionReconciliationEntry {
                manifest_id: manifest.manifest_id().to_string(),
                manifest_digest: *manifest.manifest_digest(),
                retention_epoch: manifest.retention_epoch(),
                retention_source: manifest.retention_source().cloned(),
            })
            .collect::<Vec<_>>();
        retention_entries.sort_by(|left, right| {
            left.manifest_id
                .cmp(&right.manifest_id)
                .then(left.manifest_digest.cmp(&right.manifest_digest))
        });
        let retention_snapshot = reconciliation::RetentionReconciliationSnapshot {
            version: reconciliation::RECONCILIATION_SNAPSHOT_VERSION_V1,
            manifests: retention_entries,
        };
        let retention_snapshot_hash = reconciliation::hash_snapshot(&retention_snapshot)?;
        let (gc_freed_bytes_total, gc_evictions_total) = storage.gc_counters();
        let mut chunk_refcounts = storage.chunk_refcount_snapshot();
        chunk_refcounts.sort_by(|left, right| left.digest.cmp(&right.digest));
        let gc_snapshot = reconciliation::GcReconciliationSnapshot {
            version: reconciliation::RECONCILIATION_SNAPSHOT_VERSION_V1,
            gc_freed_bytes_total,
            gc_evictions_total,
            chunk_refcounts,
        };
        let gc_snapshot_hash = reconciliation::hash_snapshot(&gc_snapshot)?;
        let divergence_count = reconciliation_divergence_count(storage, &manifests);
        let appeal_finance = self.appeal_finance_reconciliation_summary()?;
        let report = SorafsReconciliationReportV1 {
            version: SORAFS_RECONCILIATION_REPORT_VERSION_V1,
            provider_id,
            generated_at_unix: now_unix,
            repair_snapshot_hash,
            retention_snapshot_hash,
            gc_snapshot_hash,
            repair_task_count: u32::try_from(repair_projection.len()).unwrap_or(u32::MAX),
            retention_manifest_count: u32::try_from(manifests.len()).unwrap_or(u32::MAX),
            gc_evictions_total,
            gc_freed_bytes_total,
            divergence_count,
            appeal_finance,
        };
        report.validate()?;
        Ok(report)
    }
    fn appeal_finance_reconciliation_summary(
        &self,
    ) -> Result<Option<AppealFinanceReconciliationSummaryV1>, ReconciliationError> {
        if self.config.governance_dir().is_none() {
            return Ok(None);
        }
        let governance_root = self.governance_runtime_root.as_deref().ok_or_else(|| {
            ReconciliationError::AppealFinance(
                "configured governance directory has no pinned runtime root".to_string(),
            )
        })?;
        let root_guard = self
            .governance_runtime_writer_root_guard
            .as_ref()
            .ok_or_else(|| {
                ReconciliationError::AppealFinance(
                    "configured governance directory has no writable filesystem identity fence"
                        .to_string(),
                )
            })?;
        let signer = self.governance_dag_runtime_signer.as_ref().ok_or_else(|| {
            ReconciliationError::AppealFinance(
                "configured governance directory has no qualified runtime signer".to_string(),
            )
        })?;
        let checkpoint_store = self
            .governance_dag_runtime_checkpoint_store
            .as_ref()
            .ok_or_else(|| {
                ReconciliationError::AppealFinance(
                    "configured governance directory has no qualified sealed checkpoint store"
                        .to_string(),
                )
            })?;
        let publication_lock = self.governance_publication_lock.as_ref().ok_or_else(|| {
            ReconciliationError::AppealFinance(
                "configured governance directory has no publication fence".to_string(),
            )
        })?;
        let _publication_guard = publication_lock.lock().map_err(|_| {
            ReconciliationError::AppealFinance(
                "governance publication fence is poisoned".to_string(),
            )
        })?;
        governance::revalidate_runtime_dag_producer_state(
            governance_root,
            root_guard,
            signer,
            checkpoint_store,
        )
        .map_err(|error| {
            ReconciliationError::AppealFinance(format!(
                "failed to authenticate the sealed governance producer state: {error}"
            ))
        })?;
        let authoritative_rollups = governance::authoritative_appeal_finance_weekly_rollups(
            governance_root,
            signer,
            checkpoint_store,
        )
        .map_err(|error| {
            ReconciliationError::AppealFinance(format!(
                "failed to authenticate appeal finance rollups from the signed governance DAG: {error}"
            ))
        })?;
        root_guard.revalidate().map_err(|error| {
            ReconciliationError::AppealFinance(format!(
                "governance filesystem identity changed during reconciliation: {error}"
            ))
        })?;
        signer.assert_qualification().map_err(|error| {
            ReconciliationError::AppealFinance(format!(
                "governance runtime signer changed during reconciliation: {error}"
            ))
        })?;
        checkpoint_store.assert_qualification().map_err(|error| {
            ReconciliationError::AppealFinance(format!(
                "governance checkpoint store changed during reconciliation: {error}"
            ))
        })?;
        if authoritative_rollups.is_empty() {
            return empty_appeal_finance_reconciliation_summary().map(Some);
        }
        let mut entries = authoritative_rollups
            .iter()
            .map(appeal_finance_rollup_reconciliation_entry)
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            left.cycle
                .cmp(&right.cycle)
                .then(left.encoded_blake3.cmp(&right.encoded_blake3))
        });
        let snapshot = reconciliation::AppealFinanceRollupReconciliationSnapshot {
            version: reconciliation::RECONCILIATION_SNAPSHOT_VERSION_V1,
            rollups: entries.clone(),
        };
        let rollup_snapshot_hash = reconciliation::hash_snapshot(&snapshot)?;
        let mut total_treasury_xor = XorQuantity::zero();
        let mut total_rewards_forfeited_treasury_xor = XorQuantity::zero();
        let mut source_report_count = 0_u64;
        let mut case_count = 0_u64;
        for entry in &entries {
            source_report_count = source_report_count
                .checked_add(entry.report_count)
                .ok_or_else(|| {
                    ReconciliationError::AppealFinance("source report count overflow".to_string())
                })?;
            case_count = case_count.checked_add(entry.case_count).ok_or_else(|| {
                ReconciliationError::AppealFinance("appeal case count overflow".to_string())
            })?;
            total_treasury_xor = total_treasury_xor
                .checked_add(&entry.total_treasury_xor)
                .map_err(|error| {
                    ReconciliationError::AppealFinance(format!(
                        "treasury total arithmetic failed: {error}"
                    ))
                })?;
            total_rewards_forfeited_treasury_xor = total_rewards_forfeited_treasury_xor
                .checked_add(&entry.total_rewards_forfeited_treasury_xor)
                .map_err(|error| {
                    ReconciliationError::AppealFinance(format!(
                        "forfeited reward total arithmetic failed: {error}"
                    ))
                })?;
        }
        Ok(Some(AppealFinanceReconciliationSummaryV1 {
            rollup_snapshot_hash,
            rollup_count: u32::try_from(entries.len()).unwrap_or(u32::MAX),
            source_report_count,
            case_count,
            total_treasury_xor,
            total_rewards_forfeited_treasury_xor,
        }))
    }
    /// Whether the storage worker is currently enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.storage.is_some()
    }
    /// Configured on-chain provider identity for finalized capacity reconciliation.
    #[must_use]
    pub fn capacity_provider_id(&self) -> Option<ProviderId> {
        self.config.provider_id()
    }
    /// Return the finalized ledger identity backing the durable capacity projection.
    pub fn capacity_finalized_cursor(
        &self,
    ) -> Result<Option<CapacityFinalizedCursorV1>, CapacityError> {
        self.capacity.finalized_cursor()
    }
    /// Record a capacity declaration captured by Torii.
    #[cfg(test)]
    pub(crate) fn record_capacity_declaration(
        &self,
        record: &CapacityDeclarationRecord,
    ) -> Result<(), CapacityError> {
        self.mutate_capacity_durably(|capacity| {
            capacity.record_declaration(record).map(|()| ((), true))
        })?;
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
    /// Rebuild the local provider scheduler from one complete finalized ledger snapshot.
    ///
    /// The configured provider identity is supplied by Torii. The capacity manager validates all
    /// declaration and replication-order bytes before atomically installing the projection and
    /// its exact finalized height/hash. Incremental advances are monotonic; fork replacement
    /// requires [`CapacityReconcileModeV1::FullRebuild`].
    pub fn reconcile_finalized_capacity(
        &self,
        finalized_cursor: CapacityFinalizedCursorV1,
        mode: CapacityReconcileModeV1,
        provider_id: ProviderId,
        declaration: Option<&CapacityDeclarationRecord>,
        replication_orders: &[ReplicationOrderRecord],
    ) -> Result<CapacityReconciliationOutcomeV1, CapacityError> {
        let outcome = self.mutate_capacity_durably(|capacity| {
            let outcome = capacity.reconcile_finalized(
                finalized_cursor,
                mode,
                provider_id,
                declaration,
                replication_orders,
            )?;
            Ok((outcome, outcome.changed))
        })?;
        if !outcome.changed {
            return Ok(outcome);
        }
        let usage = self.capacity.usage_snapshot();
        if let Some(record) = self.capacity.active_declaration_record()? {
            self.meter.restore_capacity_runtime(
                usage.committed_total_gib,
                usage.declaration_window,
                &usage.outstanding_orders,
            );
            self.seed_telemetry_accumulator(&record);
        } else {
            self.meter.clear_capacity_runtime();
            *self
                .telemetry
                .write()
                .map_err(|_| CapacityError::StateLockPoisoned)? = None;
        }
        Ok(outcome)
    }
    /// Return a snapshot describing the currently tracked capacity usage.
    #[must_use]
    pub fn capacity_usage(&self) -> CapacityUsageSnapshot {
        self.capacity.usage_snapshot()
    }
    /// Schedule a replication order if the active declaration matches the provider.
    #[cfg(test)]
    pub(crate) fn schedule_replication_order(
        &self,
        order: &sorafs_manifest::capacity::ReplicationOrderV1,
    ) -> Result<Option<ReplicationPlan>, CapacityError> {
        let maybe_plan = self.mutate_capacity_durably(|capacity| {
            capacity.schedule_order(order).map(|plan| {
                let changed = plan.is_some();
                (plan, changed)
            })
        })?;
        if let Some(plan) = maybe_plan.as_ref() {
            self.meter.on_order_scheduled(plan);
        }
        Ok(maybe_plan)
    }
    /// Mark a replication order as completed and release its reserved capacity.
    #[cfg(test)]
    pub(crate) fn complete_replication_order(
        &self,
        order_id: [u8; 32],
    ) -> Result<ReplicationRelease, CapacityError> {
        let release = self.mutate_capacity_durably(|capacity| {
            capacity
                .complete_order(order_id)
                .map(|release| (release, true))
        })?;
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
    /// Test helper that discards the authority delta.
    #[cfg(test)]
    pub(crate) fn record_por_challenge(
        &self,
        challenge: &PorChallengeV1,
    ) -> Result<(), PorTrackerError> {
        self.record_por_challenge_with_authority_update(challenge)
            .map(drop)
            .map_err(PorMutationFailureV1::into_tracker_error)
    }
    /// Durably record a challenge and return its exact bounded status update.
    pub fn record_por_challenge_with_authority_update(
        &self,
        challenge: &PorChallengeV1,
    ) -> Result<PorStatusAuthorityUpdateV1, PorMutationFailureV1> {
        let replay_archive = self
            .por_finalized_replay_archive
            .as_ref()
            .map(|archive| archive.0.as_ref());
        self.record_por_challenge_with_optional_replay_archive(challenge, replay_archive)
    }
    fn record_por_challenge_with_optional_replay_archive(
        &self,
        challenge: &PorChallengeV1,
        replay_archive: Option<&dyn PorFinalizedReplayArchiveV1>,
    ) -> Result<PorStatusAuthorityUpdateV1, PorMutationFailureV1> {
        let proof_bounds = match (self.config.por_replay_archive_policy(), replay_archive) {
            (Some(policy), Some(archive)) => {
                verify_por_replay_archive_provider(policy, archive).map_err(|error| {
                    PorMutationFailureV1::new(error, PorMutationDispositionV1::NoMutation)
                })?;
                Some(policy.proof_bounds())
            }
            (Some(_), None) => {
                return Err(PorMutationFailureV1::new(
                    PorTrackerError::ReplayArchiveRequired,
                    PorMutationDispositionV1::NoMutation,
                ));
            }
            (None, Some(_)) => {
                return Err(PorMutationFailureV1::new(
                    PorTrackerError::ReplayArchiveBindingMismatch,
                    PorMutationDispositionV1::NoMutation,
                ));
            }
            (None, None) => None,
        };
        let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            PorMutationFailureV1::new(
                PorTrackerError::RuntimeCheckpoint(
                    "auxiliary checkpoint transaction lock poisoned".to_owned(),
                ),
                PorMutationDispositionV1::NoMutation,
            )
        })?;
        self.ensure_durability_healthy()
            .map_err(PorTrackerError::RuntimeCheckpoint)
            .map_err(|error| {
                PorMutationFailureV1::new(error, PorMutationDispositionV1::NoMutation)
            })?;
        let previous = self.por.checkpoint();
        let outcome = match replay_archive {
            Some(replay_archive) => self
                .por
                .record_challenge_with_archive_and_bounds(
                    challenge,
                    replay_archive,
                    proof_bounds.expect("configured archive has proof bounds"),
                )
                .map_err(|error| {
                    PorMutationFailureV1::new(error, PorMutationDispositionV1::NoMutation)
                })?,
            None => self.por.record_challenge(challenge).map_err(|error| {
                PorMutationFailureV1::new(error, PorMutationDispositionV1::NoMutation)
            })?,
        };
        if let (Some(policy), Some(replay_archive)) =
            (self.config.por_replay_archive_policy(), replay_archive)
            && let Err(error) = verify_por_replay_archive_provider(policy, replay_archive)
        {
            if let Err(rollback) = self.por.restore_checkpoint(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back PoR challenge after replay-archive provider drift",
                    rollback,
                );
                return Err(PorMutationFailureV1::new(
                    PorTrackerError::RuntimeCheckpoint(message),
                    PorMutationDispositionV1::RollbackFailed,
                ));
            }
            return Err(PorMutationFailureV1::new(
                error,
                PorMutationDispositionV1::RolledBack,
            ));
        }
        if let por::PorChallengeRecordOutcomeV1::ExactReplay(status) = outcome {
            return self
                .por
                .status_authority_replay_update(status)
                .map_err(|error| {
                    PorMutationFailureV1::new(error, PorMutationDispositionV1::NoMutation)
                });
        }
        let authority_update = match self.por.status_authority_update(challenge.challenge_id) {
            Ok(update) => update,
            Err(error) => {
                if let Err(rollback) = self.por.restore_checkpoint(previous) {
                    let message = self.record_unrecoverable_rollback(
                        "failed to roll back PoR challenge after authority-delta validation",
                        rollback,
                    );
                    return Err(PorMutationFailureV1::new(
                        PorTrackerError::RuntimeCheckpoint(message),
                        PorMutationDispositionV1::RollbackFailed,
                    ));
                }
                return Err(PorMutationFailureV1::new(
                    error,
                    PorMutationDispositionV1::RolledBack,
                ));
            }
        };
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(PorMutationFailureV1::new(
                    PorTrackerError::RuntimeCheckpoint(err.to_string()),
                    PorMutationDispositionV1::CommitUncertain,
                ));
            }
            if let Err(rollback) = self.por.restore_checkpoint(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back PoR challenge after checkpoint error",
                    rollback,
                );
                return Err(PorMutationFailureV1::new(
                    PorTrackerError::RuntimeCheckpoint(message),
                    PorMutationDispositionV1::RollbackFailed,
                ));
            }
            return Err(PorMutationFailureV1::new(
                PorTrackerError::RuntimeCheckpoint(err.to_string()),
                PorMutationDispositionV1::RolledBack,
            ));
        }
        Ok(authority_update)
    }
    /// Record a provider PoR proof response bound to its admitted provider key.
    #[cfg(test)]
    pub(crate) fn record_por_proof(
        &self,
        proof: &PorProofV1,
        admitted_provider_key: &[u8],
    ) -> Result<(), PorTrackerError> {
        self.record_por_proof_with_authority_update(proof, admitted_provider_key)
            .map(drop)
            .map_err(PorMutationFailureV1::into_tracker_error)
    }
    /// Durably record a proof and return its exact bounded status update.
    pub fn record_por_proof_with_authority_update(
        &self,
        proof: &PorProofV1,
        admitted_provider_key: &[u8],
    ) -> Result<PorStatusAuthorityUpdateV1, PorMutationFailureV1> {
        let replay_archive = self
            .por_finalized_replay_archive
            .as_ref()
            .map(|archive| archive.0.as_ref());
        let proof_bounds = match (self.config.por_replay_archive_policy(), replay_archive) {
            (Some(policy), Some(archive)) => {
                verify_por_replay_archive_provider(policy, archive).map_err(|error| {
                    PorMutationFailureV1::new(error, PorMutationDispositionV1::NoMutation)
                })?;
                Some(policy.proof_bounds())
            }
            (Some(_), None) => {
                return Err(PorMutationFailureV1::new(
                    PorTrackerError::ReplayArchiveRequired,
                    PorMutationDispositionV1::NoMutation,
                ));
            }
            (None, Some(_)) => {
                return Err(PorMutationFailureV1::new(
                    PorTrackerError::ReplayArchiveBindingMismatch,
                    PorMutationDispositionV1::NoMutation,
                ));
            }
            (None, None) => None,
        };
        let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            PorMutationFailureV1::new(
                PorTrackerError::RuntimeCheckpoint(
                    "auxiliary checkpoint transaction lock poisoned".to_owned(),
                ),
                PorMutationDispositionV1::NoMutation,
            )
        })?;
        self.ensure_durability_healthy()
            .map_err(PorTrackerError::RuntimeCheckpoint)
            .map_err(|error| {
                PorMutationFailureV1::new(error, PorMutationDispositionV1::NoMutation)
            })?;
        let previous = self.por.checkpoint();
        let outcome = match replay_archive {
            Some(replay_archive) => self
                .por
                .record_proof_with_archive_and_bounds(
                    proof,
                    admitted_provider_key,
                    replay_archive,
                    proof_bounds.expect("configured archive has proof bounds"),
                )
                .map_err(|error| {
                    PorMutationFailureV1::new(error, PorMutationDispositionV1::NoMutation)
                })?,
            None => self
                .por
                .record_proof(proof, admitted_provider_key)
                .map_err(|error| {
                    PorMutationFailureV1::new(error, PorMutationDispositionV1::NoMutation)
                })?,
        };
        if let (Some(policy), Some(replay_archive)) =
            (self.config.por_replay_archive_policy(), replay_archive)
            && let Err(error) = verify_por_replay_archive_provider(policy, replay_archive)
        {
            if let Err(rollback) = self.por.restore_checkpoint(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back PoR proof after replay-archive provider drift",
                    rollback,
                );
                return Err(PorMutationFailureV1::new(
                    PorTrackerError::RuntimeCheckpoint(message),
                    PorMutationDispositionV1::RollbackFailed,
                ));
            }
            return Err(PorMutationFailureV1::new(
                error,
                PorMutationDispositionV1::RolledBack,
            ));
        }
        if let por::PorProofRecordOutcomeV1::ExactReplay(status) = outcome {
            return self
                .por
                .status_authority_replay_update(status)
                .map_err(|error| {
                    PorMutationFailureV1::new(error, PorMutationDispositionV1::NoMutation)
                });
        }
        let authority_update = match self.por.status_authority_update(proof.challenge_id) {
            Ok(update) => update,
            Err(error) => {
                if let Err(rollback) = self.por.restore_checkpoint(previous) {
                    let message = self.record_unrecoverable_rollback(
                        "failed to roll back PoR proof after authority-delta validation",
                        rollback,
                    );
                    return Err(PorMutationFailureV1::new(
                        PorTrackerError::RuntimeCheckpoint(message),
                        PorMutationDispositionV1::RollbackFailed,
                    ));
                }
                return Err(PorMutationFailureV1::new(
                    error,
                    PorMutationDispositionV1::RolledBack,
                ));
            }
        };
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(PorMutationFailureV1::new(
                    PorTrackerError::RuntimeCheckpoint(err.to_string()),
                    PorMutationDispositionV1::CommitUncertain,
                ));
            }
            if let Err(rollback) = self.por.restore_checkpoint(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back PoR proof after checkpoint error",
                    rollback,
                );
                return Err(PorMutationFailureV1::new(
                    PorTrackerError::RuntimeCheckpoint(message),
                    PorMutationDispositionV1::RollbackFailed,
                ));
            }
            return Err(PorMutationFailureV1::new(
                PorTrackerError::RuntimeCheckpoint(err.to_string()),
                PorMutationDispositionV1::RolledBack,
            ));
        }
        Ok(authority_update)
    }
    /// Return process-local PoR latency, VRF, and seed-binding metrics.
    #[must_use]
    pub fn por_protocol_metrics(&self) -> PorProtocolMetricsSnapshot {
        self.por.protocol_metrics()
    }
    /// Export the complete bounded PoR status history from the authoritative checkpoint state.
    pub fn por_status_authority_snapshot(
        &self,
    ) -> Result<PorStatusAuthoritySnapshotV1, PorTrackerError> {
        self.por.status_authority_snapshot()
    }
    /// Return the oldest failed-verdict repair intent awaiting durable handoff.
    pub fn next_pending_por_repair_work(
        &self,
    ) -> Result<Option<PorPendingRepairWorkV1>, PorTrackerError> {
        self.por.next_pending_repair_work()
    }
    /// Durably acknowledge one exact failed-verdict repair handoff.
    pub fn acknowledge_por_repair_handoff(
        &self,
        challenge_id: [u8; 32],
        repair_task_id: [u8; 32],
    ) -> Result<PorRepairHandoffAckOutcomeV1, PorTrackerError> {
        let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            PorTrackerError::RuntimeCheckpoint(
                "auxiliary checkpoint transaction lock poisoned".to_owned(),
            )
        })?;
        self.ensure_durability_healthy()
            .map_err(PorTrackerError::RuntimeCheckpoint)?;
        let previous = self.por.checkpoint();
        let outcome = self
            .por
            .acknowledge_repair_handoff(challenge_id, repair_task_id)?;
        if outcome == PorRepairHandoffAckOutcomeV1::ExactReplay {
            return Ok(outcome);
        }
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(PorTrackerError::RuntimeCheckpoint(err.to_string()));
            }
            if let Err(rollback) = self.por.restore_checkpoint(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back PoR repair acknowledgement after checkpoint error",
                    rollback,
                );
                return Err(PorTrackerError::RuntimeCheckpoint(message));
            }
            return Err(PorTrackerError::RuntimeCheckpoint(err.to_string()));
        }
        Ok(outcome)
    }
    /// Reconcile one exact durable failed-verdict repair outbox entry.
    ///
    /// The external admission boundary runs before the node acknowledgement.
    /// A crash or commit-uncertain acknowledgement therefore replays the same
    /// deterministic task identity until the checkpoint advances.
    pub fn reconcile_next_por_repair_handoff(
        &self,
        handoff: &dyn PorRepairHandoff,
    ) -> Result<PorRepairReconcileOutcomeV1, PorRepairReconcileErrorV1> {
        let Some(work) = self.next_pending_por_repair_work()? else {
            return Ok(PorRepairReconcileOutcomeV1::Idle);
        };
        let actual = handoff.enqueue_failed_por_repair(&work.intent)?;
        if actual != work.repair_task_id {
            return Err(PorRepairReconcileErrorV1::TaskIdMismatch {
                expected: work.repair_task_id,
                actual,
            });
        }
        let acknowledgement =
            self.acknowledge_por_repair_handoff(work.intent.challenge_id, work.repair_task_id)?;
        Ok(PorRepairReconcileOutcomeV1::Reconciled {
            work,
            acknowledgement,
        })
    }
    /// Return the exact next retained PoR terminal awaiting reputation admission.
    pub fn next_por_reputation_terminal_work(
        &self,
    ) -> Result<Option<PorReputationTerminalWorkV1>, PorTrackerError> {
        self.por.next_reputation_terminal_work()
    }
    /// Return the number of retained PoR terminals awaiting acknowledgement.
    #[must_use]
    pub fn pending_por_reputation_terminal_count(&self) -> u64 {
        self.por.pending_reputation_terminal_count()
    }
    /// Durably acknowledge the exact next retained PoR reputation terminal.
    ///
    /// The cursor advances in the same auxiliary checkpoint family as PoR finalization. Missing
    /// durable storage, skipped or substituted work, and checkpoint failures all fail closed.
    pub fn acknowledge_por_reputation_terminal(
        &self,
        sequence: u64,
        work_digest: [u8; 32],
    ) -> Result<PorReputationTerminalAckOutcomeV1, PorTrackerError> {
        let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            PorTrackerError::RuntimeCheckpoint(
                "auxiliary checkpoint transaction lock poisoned".to_owned(),
            )
        })?;
        self.ensure_durability_healthy()
            .map_err(PorTrackerError::RuntimeCheckpoint)?;
        if self.auxiliary_runtime_checkpoint_path.is_none() {
            return Err(PorTrackerError::RuntimeCheckpoint(
                "durable auxiliary checkpoint is required for PoR reputation acknowledgement"
                    .to_owned(),
            ));
        }
        let previous = self.por.checkpoint();
        let outcome = self
            .por
            .acknowledge_reputation_terminal(sequence, work_digest)?;
        if outcome == PorReputationTerminalAckOutcomeV1::ExactReplay {
            return Ok(outcome);
        }
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(PorTrackerError::RuntimeCheckpoint(err.to_string()));
            }
            if let Err(rollback) = self.por.restore_checkpoint(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back PoR reputation acknowledgement after checkpoint error",
                    rollback,
                );
                return Err(PorTrackerError::RuntimeCheckpoint(message));
            }
            return Err(PorTrackerError::RuntimeCheckpoint(err.to_string()));
        }
        Ok(outcome)
    }
    /// Durably compact an acknowledged finalized prefix into an authenticated archive.
    ///
    /// External appends happen before the auxiliary checkpoint advances. A
    /// checkpoint failure restores the exact local prefix; retry then requires
    /// the archive to return the identical signed receipt.
    pub fn compact_acknowledged_por_replay_archive(
        &self,
        replay_archive: &dyn PorFinalizedReplayArchiveV1,
        expected_binding: PorFinalizedReplayArchiveBindingV1,
        maximum_records: u32,
    ) -> Result<u32, PorTrackerError> {
        let Some(policy) = self.config.por_replay_archive_policy() else {
            return Err(PorTrackerError::ReplayArchiveBindingMismatch);
        };
        if expected_binding != policy.binding() {
            return Err(PorTrackerError::ReplayArchiveBindingMismatch);
        }
        verify_por_replay_archive_provider(policy, replay_archive)?;
        let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            PorTrackerError::RuntimeCheckpoint(
                "auxiliary checkpoint transaction lock poisoned".to_owned(),
            )
        })?;
        self.ensure_durability_healthy()
            .map_err(PorTrackerError::RuntimeCheckpoint)?;
        if self.auxiliary_runtime_checkpoint_path.is_none() {
            return Err(PorTrackerError::RuntimeCheckpoint(
                "durable auxiliary checkpoint is required for PoR replay-archive compaction"
                    .to_owned(),
            ));
        }
        let previous = self.por.checkpoint();
        let compacted = self.por.compact_acknowledged_with_replay_archive(
            replay_archive,
            expected_binding,
            maximum_records,
        )?;
        if let Err(error) = verify_por_replay_archive_provider(policy, replay_archive) {
            if let Err(rollback) = self.por.restore_checkpoint(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back PoR compaction after replay-archive provider drift",
                    rollback,
                );
                return Err(PorTrackerError::RuntimeCheckpoint(message));
            }
            return Err(error);
        }
        if compacted == 0 {
            return Ok(0);
        }
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(PorTrackerError::RuntimeCheckpoint(err.to_string()));
            }
            if let Err(rollback) = self.por.restore_checkpoint(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back PoR replay-archive compaction after checkpoint error",
                    rollback,
                );
                return Err(PorTrackerError::RuntimeCheckpoint(message));
            }
            return Err(PorTrackerError::RuntimeCheckpoint(err.to_string()));
        }
        Ok(compacted)
    }
    /// Reconcile the configured bounded acknowledged prefix into the exact
    /// deployment-owned replay archive.
    pub fn compact_configured_por_replay_archive(&self) -> Result<u32, PorTrackerError> {
        let Some(policy) = self.config.por_replay_archive_policy() else {
            return Ok(0);
        };
        let archive = self
            .por_finalized_replay_archive
            .as_ref()
            .ok_or(PorTrackerError::ReplayArchiveRequired)?;
        self.compact_acknowledged_por_replay_archive(
            archive.0.as_ref(),
            policy.binding(),
            policy.max_records_per_tick(),
        )
    }
    /// Reconcile one retained PoR terminal into the durable reputation outbox.
    ///
    /// Admission happens before acknowledgement. If the process crashes or the node checkpoint
    /// fails after admission, restart presents the identical work again; the admission boundary
    /// must return `ExactReplay`, after which the node durably advances its cursor.
    pub fn reconcile_next_por_reputation_terminal(
        &self,
        admission: &dyn reputation::runtime::ReputationNativeOutcomeAdmissionApiV1,
    ) -> Result<PorReputationReconcileOutcomeV1, PorReputationReconcileErrorV1> {
        let Some(work) = self.next_por_reputation_terminal_work()? else {
            return Ok(PorReputationReconcileOutcomeV1::Idle);
        };
        let admitted =
            admission.record_por_terminal(ProviderId::new(work.provider_id), work.terminal)?;
        let acknowledgement =
            self.acknowledge_por_reputation_terminal(work.sequence, work.work_digest)?;
        Ok(PorReputationReconcileOutcomeV1::Reconciled {
            work: Box::new(work),
            admission: admitted,
            acknowledgement,
        })
    }
    /// Record an audit verdict and update telemetry counters accordingly.
    #[cfg(test)]
    pub(crate) fn record_por_verdict(
        &self,
        verdict: &AuditVerdictV1,
        trusted_auditor_keys: &[Vec<u8>],
        auditor_threshold: usize,
    ) -> Result<PorVerdictOutcome, PorTrackerError> {
        self.record_por_verdict_with_authority_update(
            verdict,
            trusted_auditor_keys,
            auditor_threshold,
        )
        .map(|(outcome, _update)| outcome)
        .map_err(PorMutationFailureV1::into_tracker_error)
    }
    /// Durably record a verdict and return its exact bounded status update.
    pub fn record_por_verdict_with_authority_update(
        &self,
        verdict: &AuditVerdictV1,
        trusted_auditor_keys: &[Vec<u8>],
        auditor_threshold: usize,
    ) -> Result<(PorVerdictOutcome, PorStatusAuthorityUpdateV1), PorMutationFailureV1> {
        let replay_archive = self
            .por_finalized_replay_archive
            .as_ref()
            .map(|archive| archive.0.as_ref());
        self.record_por_verdict_with_optional_replay_archive(
            verdict,
            trusted_auditor_keys,
            auditor_threshold,
            replay_archive,
        )
    }
    fn record_por_verdict_with_optional_replay_archive(
        &self,
        verdict: &AuditVerdictV1,
        trusted_auditor_keys: &[Vec<u8>],
        auditor_threshold: usize,
        replay_archive: Option<&dyn PorFinalizedReplayArchiveV1>,
    ) -> Result<(PorVerdictOutcome, PorStatusAuthorityUpdateV1), PorMutationFailureV1> {
        let proof_bounds = match (self.config.por_replay_archive_policy(), replay_archive) {
            (Some(policy), Some(archive)) => {
                verify_por_replay_archive_provider(policy, archive).map_err(|error| {
                    PorMutationFailureV1::new(error, PorMutationDispositionV1::NoMutation)
                })?;
                Some(policy.proof_bounds())
            }
            (Some(_), None) => {
                return Err(PorMutationFailureV1::new(
                    PorTrackerError::ReplayArchiveRequired,
                    PorMutationDispositionV1::NoMutation,
                ));
            }
            (None, Some(_)) => {
                return Err(PorMutationFailureV1::new(
                    PorTrackerError::ReplayArchiveBindingMismatch,
                    PorMutationDispositionV1::NoMutation,
                ));
            }
            (None, None) => None,
        };
        let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            PorMutationFailureV1::new(
                PorTrackerError::RuntimeCheckpoint(
                    "auxiliary checkpoint transaction lock poisoned".to_owned(),
                ),
                PorMutationDispositionV1::NoMutation,
            )
        })?;
        self.ensure_durability_healthy()
            .map_err(PorTrackerError::RuntimeCheckpoint)
            .map_err(|error| {
                PorMutationFailureV1::new(error, PorMutationDispositionV1::NoMutation)
            })?;
        let previous_history = self
            .por_history
            .read()
            .map_err(|_| {
                PorMutationFailureV1::new(
                    PorTrackerError::RuntimeCheckpoint("PoR history lock poisoned".to_owned()),
                    PorMutationDispositionV1::NoMutation,
                )
            })?
            .clone();
        let previous_tracker = self.por.checkpoint();
        let transition = match replay_archive {
            Some(replay_archive) => self
                .por
                .record_verdict_durable_with_archive_and_bounds(
                    verdict,
                    trusted_auditor_keys,
                    auditor_threshold,
                    replay_archive,
                    proof_bounds.expect("configured archive has proof bounds"),
                )
                .map_err(|error| {
                    PorMutationFailureV1::new(error, PorMutationDispositionV1::NoMutation)
                })?,
            None => self
                .por
                .record_verdict_durable(verdict, trusted_auditor_keys, auditor_threshold)
                .map_err(|error| {
                    PorMutationFailureV1::new(error, PorMutationDispositionV1::NoMutation)
                })?,
        };
        if let (Some(policy), Some(replay_archive)) =
            (self.config.por_replay_archive_policy(), replay_archive)
            && let Err(error) = verify_por_replay_archive_provider(policy, replay_archive)
        {
            if let Err(rollback) = self.por.restore_checkpoint(previous_tracker) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back PoR verdict after replay-archive provider drift",
                    rollback,
                );
                return Err(PorMutationFailureV1::new(
                    PorTrackerError::RuntimeCheckpoint(message),
                    PorMutationDispositionV1::RollbackFailed,
                ));
            }
            return Err(PorMutationFailureV1::new(
                error,
                PorMutationDispositionV1::RolledBack,
            ));
        }
        let stats = transition.stats;
        let reputation_work = transition.reputation_work;
        if !transition.newly_finalized {
            let consecutive_failures = self
                .por_history
                .read()
                .map_err(|_| {
                    PorMutationFailureV1::new(
                        PorTrackerError::RuntimeCheckpoint("PoR history lock poisoned".to_owned()),
                        PorMutationDispositionV1::NoMutation,
                    )
                })?
                .get(&(verdict.manifest_digest, verdict.provider_id))
                .map_or(0, |entry| entry.consecutive_failures);
            let outcome = PorVerdictOutcome {
                stats,
                repair_task_id: transition.repair_task_id,
                consecutive_failures,
                reputation_work,
            };
            let update = self
                .por
                .status_authority_replay_update(transition.authority_status)
                .map_err(|error| {
                    PorMutationFailureV1::new(error, PorMutationDispositionV1::NoMutation)
                })?;
            return Ok((outcome, update));
        }
        let consecutive_failures = match self.update_por_history_entry(verdict) {
            Ok(outcome) => outcome,
            Err(error) => {
                if let Err(rollback) =
                    self.rollback_por_verdict_state(previous_tracker, previous_history)
                {
                    let message = self.record_unrecoverable_rollback(
                        "failed to roll back finalized PoR state after bookkeeping error",
                        rollback,
                    );
                    return Err(PorMutationFailureV1::new(
                        PorTrackerError::RuntimeCheckpoint(message),
                        PorMutationDispositionV1::RollbackFailed,
                    ));
                }
                return Err(PorMutationFailureV1::new(
                    error,
                    PorMutationDispositionV1::RolledBack,
                ));
            }
        };
        let authority_update = match self.por.status_authority_update(verdict.challenge_id) {
            Ok(update) => update,
            Err(error) => {
                if let Err(rollback) =
                    self.rollback_por_verdict_state(previous_tracker, previous_history)
                {
                    let message = self.record_unrecoverable_rollback(
                        "failed to roll back finalized PoR state after authority-delta validation",
                        rollback,
                    );
                    return Err(PorMutationFailureV1::new(
                        PorTrackerError::RuntimeCheckpoint(message),
                        PorMutationDispositionV1::RollbackFailed,
                    ));
                }
                return Err(PorMutationFailureV1::new(
                    error,
                    PorMutationDispositionV1::RolledBack,
                ));
            }
        };
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(PorMutationFailureV1::new(
                    PorTrackerError::RuntimeCheckpoint(err.to_string()),
                    PorMutationDispositionV1::CommitUncertain,
                ));
            }
            if let Err(rollback) =
                self.rollback_por_verdict_state(previous_tracker, previous_history)
            {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back finalized PoR state after checkpoint error",
                    rollback,
                );
                return Err(PorMutationFailureV1::new(
                    PorTrackerError::RuntimeCheckpoint(message),
                    PorMutationDispositionV1::RollbackFailed,
                ));
            }
            return Err(PorMutationFailureV1::new(
                PorTrackerError::RuntimeCheckpoint(err.to_string()),
                PorMutationDispositionV1::RolledBack,
            ));
        }
        if stats.success_samples > 0 {
            self.meter.record_por_samples(stats.success_samples, 0);
        }
        if stats.failed_samples > 0 {
            self.meter.record_por_samples(0, stats.failed_samples);
        }
        self.schedulers
            .record_por_samples(stats.success_samples, stats.failed_samples);
        let outcome = PorVerdictOutcome {
            stats,
            repair_task_id: transition.repair_task_id,
            consecutive_failures,
            reputation_work,
        };
        Ok((outcome, authority_update))
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
        vrf_records: &HashMap<ManifestVrfKey, ManifestVrfBundle>,
    ) -> Result<Vec<PlannedChallenge>, PorChallengePlannerError> {
        self.plan_por_challenges_with_forced_policy(randomness, vrf_records, true)
    }
    /// Generate PoR challenges while explicitly controlling whether missing
    /// provider VRFs may enter the forced-challenge path.
    pub fn plan_por_challenges_with_forced_policy(
        &self,
        randomness: PorRandomness,
        vrf_records: &HashMap<ManifestVrfKey, ManifestVrfBundle>,
        allow_forced: bool,
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
        let grace_secs = self.gc_config.retention_grace_secs();
        let issued_at = randomness.issued_at_unix;
        let manifests = storage.manifests();
        let mut challenges = Vec::with_capacity(manifests.len());
        for manifest in manifests {
            let retention_epoch = manifest.retention_epoch();
            if retention_epoch != 0 {
                let expires_at = retention_epoch.saturating_add(grace_secs);
                if issued_at >= expires_at {
                    continue;
                }
            }
            let digest = *manifest.manifest_digest();
            let vrf = vrf_records.get(&ManifestVrfKey {
                provider_id,
                manifest_digest: digest,
            });
            let planned = build_por_challenge_for_manifest(
                &manifest,
                provider_id,
                &randomness,
                vrf,
                &sample_policy,
                allow_forced,
            )?;
            challenges.push(planned);
        }
        challenges.sort_by_key(|planned| planned.challenge.challenge_id);
        Ok(challenges)
    }
    /// Record a PoTR receipt using an explicit chain-authoritative repair handoff.
    pub fn record_potr_receipt_with_handoff(
        &self,
        receipt: PotrReceiptV1,
        gateway_public_key: &[u8; 32],
        admission: &AdmissionRecord,
        admission_policy: &PotrAdmissionPolicyBindingV1,
        repair_handoff: &dyn potr::PotrLatencyRepairHandoff,
    ) -> Result<PotrRecordOutcome, PotrTrackerError> {
        self.potr.record_receipt(
            receipt,
            gateway_public_key,
            admission,
            admission_policy,
            repair_handoff,
        )
    }
    /// Return the exact durable PoTR admission-policy floor for a provider.
    pub fn potr_admission_policy_floor(
        &self,
        provider_id: &[u8; 32],
    ) -> Result<Option<PotrAdmissionPolicyBindingV1>, PotrTrackerError> {
        self.potr.admission_policy_floor(provider_id)
    }
    /// Retrieve PoTR receipts matching the manifest/provider filters.
    pub fn potr_receipts(
        &self,
        manifest_digest: &[u8; 32],
        provider_id: &[u8; 32],
        tier: Option<ProofStreamTier>,
    ) -> Result<Vec<PotrReceiptV1>, PotrTrackerError> {
        self.potr.receipts_for(manifest_digest, provider_id, tier)
    }
    /// Return status for one exact final signed PoTR receipt.
    pub fn potr_receipt_status(
        &self,
        receipt_digest: &[u8; 32],
    ) -> Result<Option<PotrReceiptStatusV1>, PotrTrackerError> {
        self.potr.status(receipt_digest)
    }
    /// Export a bounded sequence-ordered page of PoTR receipt statuses.
    pub fn export_potr_receipt_statuses(
        &self,
        after_sequence: u64,
        limit: usize,
    ) -> Result<Vec<PotrReceiptStatusV1>, PotrTrackerError> {
        self.potr.export_statuses(after_sequence, limit)
    }
    /// Export a bounded sequence-ordered page of exact final signed receipts.
    pub fn export_potr_receipts(
        &self,
        after_sequence: u64,
        limit: usize,
    ) -> Result<Vec<PotrReceiptV1>, PotrTrackerError> {
        self.potr.export_receipts(after_sequence, limit)
    }
    /// Returns a clone of the persistent storage backend when enabled.
    #[must_use]
    pub fn storage(&self) -> Option<Arc<StorageBackend>> {
        self.storage.clone()
    }
    #[cfg(test)]
    fn enqueue_finalized_provider_ingest(
        &self,
        finalized_pin: &PinManifestFinalizedRecordV1,
        order_id: [u8; 32],
    ) -> Result<ProviderIngestEnqueueResultV1, FinalizedProviderIngestError> {
        let outbox = self
            .provider_ingest_outbox
            .as_ref()
            .ok_or(FinalizedProviderIngestError::Disabled)?;
        let authorization =
            self.finalized_provider_ingest_authorization(finalized_pin, order_id)?;
        outbox.enqueue(authorization).map_err(Into::into)
    }
    #[cfg(test)]
    fn claim_finalized_provider_ingest_source(
        &self,
        job_id: [u8; 32],
        owner: ProviderIngestClaimOwnerV1,
        now_ms: u64,
        observed_finalized_cursor: ProviderIngestFinalizedCursorV1,
    ) -> Result<ProviderIngestSourceClaimV1, FinalizedProviderIngestError> {
        self.provider_ingest_outbox
            .as_ref()
            .ok_or(FinalizedProviderIngestError::Disabled)?
            .claim_source(job_id, owner, now_ms, observed_finalized_cursor)
            .map_err(Into::into)
    }
    #[cfg(test)]
    fn ingest_finalized_provider_payload<R: Read>(
        &self,
        claim: &ProviderIngestSourceClaimV1,
        manifest: &ManifestV1,
        plan: &CarBuildPlan,
        reader: &mut R,
        completed_at_ms: u64,
        observed_finalized_cursor: ProviderIngestFinalizedCursorV1,
    ) -> Result<String, FinalizedProviderIngestError> {
        let outbox = self
            .provider_ingest_outbox
            .as_ref()
            .ok_or(FinalizedProviderIngestError::Disabled)?;
        let authorization = claim.authorization();
        if let Err(error) = validate_finalized_provider_payload(authorization, manifest, plan) {
            return match outbox.schedule_source_retry(
                claim,
                completed_at_ms,
                observed_finalized_cursor,
                ProviderIngestFailureClassV1::SourceRejected,
            )? {
                ProviderIngestRetryOutcomeV1::RetryScheduled { .. } => Err(error),
                ProviderIngestRetryOutcomeV1::DeadLettered => {
                    Err(FinalizedProviderIngestError::DeadLettered)
                }
            };
        }
        let manifest_id = match self.ingest_manifest(manifest, plan, reader) {
            Ok(manifest_id) => manifest_id,
            Err(NodeStorageError::Storage(StorageError::ManifestExists { .. })) => {
                match self.verify_existing_finalized_provider_manifest(authorization, manifest) {
                    Ok(manifest_id) => manifest_id,
                    Err(error) => {
                        outbox.dead_letter_source(
                            claim,
                            completed_at_ms,
                            observed_finalized_cursor,
                            ProviderIngestDeadLetterReasonV1::StorageRejected,
                            ProviderIngestFailureClassV1::StorageRejected,
                        )?;
                        return Err(error);
                    }
                }
            }
            Err(error) => {
                return match outbox.schedule_source_retry(
                    claim,
                    completed_at_ms,
                    observed_finalized_cursor,
                    ProviderIngestFailureClassV1::StorageRejected,
                )? {
                    ProviderIngestRetryOutcomeV1::RetryScheduled { .. } => Err(error.into()),
                    ProviderIngestRetryOutcomeV1::DeadLettered => {
                        Err(FinalizedProviderIngestError::DeadLettered)
                    }
                };
            }
        };
        outbox.mark_local_stored(claim, completed_at_ms, manifest_id.clone())?;
        Ok(manifest_id)
    }
    /// Export one deterministic bounded page of payload-free ingest status.
    pub fn finalized_provider_ingest_status_page(
        &self,
        after_job_id: Option<[u8; 32]>,
        limit: usize,
    ) -> Result<ProviderIngestStatusPageV1, FinalizedProviderIngestError> {
        self.provider_ingest_outbox
            .as_ref()
            .ok_or(FinalizedProviderIngestError::Disabled)?
            .statuses_page(after_job_id, limit)
            .map_err(Into::into)
    }
    /// Return exact payload-free provider-ingest outbox counts from one bounded cached snapshot.
    pub fn finalized_provider_ingest_counts(
        &self,
    ) -> Result<ProviderIngestOutboxCountsV1, FinalizedProviderIngestError> {
        self.provider_ingest_outbox
            .as_ref()
            .ok_or(FinalizedProviderIngestError::Disabled)?
            .aggregate_counts()
            .map_err(Into::into)
    }
    /// Reserve this store incarnation's completed-Musubi capture coordinator.
    ///
    /// The reservation is shared by every clone of this handle and is never reset, including when
    /// the returned coordinator is dropped or its lazy signed-reader binding is temporarily
    /// unavailable. A separately constructed handle owns a distinct reservation. This call retains
    /// the exact erased reader without consulting it, allowing height-zero daemon startup to await
    /// genesis before the private coordinator binds a capture session.
    ///
    /// The returned opaque coordinator intentionally has no public operational surface: it cannot
    /// expose scanner pages, finalized claims, approval requests, or any signing, journal,
    /// inventory, transaction, or registry effect.
    ///
    /// # Errors
    ///
    /// Returns an error when provider ingest is disabled, another caller already consumed the
    /// tenure, the configured provider/network identity is invalid, or the page bound is outside
    /// the hard finalized limit. Once an enabled store's reservation is attempted, every failure is
    /// non-resetting so a different reader cannot be substituted.
    #[doc(hidden)]
    pub fn take_provider_ingest_completed_musubi_capture_coordinator(
        &self,
        network_id: NetworkId,
        max_page_rows: usize,
        ledger: Arc<dyn ProviderIngestCompletedMusubiSignedCaptureLedgerV1>,
    ) -> Result<ProviderIngestCompletedMusubiCaptureCoordinatorV1, FinalizedProviderIngestError>
    {
        let completed_musubi_store_instance = self
            .completed_musubi_store_instance
            .clone()
            .ok_or(FinalizedProviderIngestError::Disabled)?;
        if !completed_musubi_store_instance.try_take_capture_coordinator() {
            return Err(FinalizedProviderIngestError::CompletedMusubiCaptureCoordinatorTaken);
        }
        let provider_id =
            self.config
                .provider_id()
                .ok_or(FinalizedProviderIngestError::BindingMismatch(
                    "provider identity is not configured",
                ))?;
        ProviderIngestCompletedMusubiCaptureCoordinatorV1::new_pending(
            completed_musubi_store_instance,
            *provider_id.as_bytes(),
            network_id,
            max_page_rows,
            ledger,
        )
        .map_err(Into::into)
    }
    /// Construct the supervised finalized-ledger provider-ingest runtime.
    ///
    /// The configured provider identity and node-owned durable outbox cannot be
    /// replaced by an adapter. Callers supply only the production boundaries
    /// for finalized reads, authenticated fetch, local storage, payload
    /// construction, isolated signing, transaction ingress, and runtime time.
    #[allow(clippy::too_many_arguments)]
    pub fn build_provider_ingest_runtime<
        Ledger,
        Fetch,
        Storage,
        Builder,
        Resolver,
        Ingress,
        Clock,
    >(
        &self,
        network_id: NetworkId,
        claim_owner: ProviderIngestClaimOwnerV1,
        policy: ProviderIngestRuntimePolicyV1,
        ledger: Arc<Ledger>,
        fetch: Arc<Fetch>,
        storage: Arc<Storage>,
        payload_builder: Arc<Builder>,
        signer_resolver: Arc<Resolver>,
        ingress: Arc<Ingress>,
        clock: Arc<Clock>,
    ) -> FinalizedProviderIngestRuntimeResultV1<
        Ledger,
        Fetch,
        Storage,
        Builder,
        Resolver,
        Ingress,
        Clock,
    >
    where
        Ledger: ProviderIngestFinalizedLedgerV1,
        Fetch: ProviderIngestAuthenticatedSourceFetchV1,
        Storage: ProviderIngestLocalStorageV1<Fetch::Fetched>,
        Builder: ProviderIngestCompletionPayloadBuilderV1,
        Resolver: ProviderIngestCompletionSignerResolverV1,
        Ingress: ProviderIngestTransactionIngressV1,
        Clock: ProviderIngestClockV1,
    {
        let provider_id =
            self.config
                .provider_id()
                .ok_or(FinalizedProviderIngestError::BindingMismatch(
                    "provider identity is not configured",
                ))?;
        let outbox = self
            .provider_ingest_outbox
            .clone()
            .ok_or(FinalizedProviderIngestError::Disabled)?;
        ProviderIngestRuntimeV1::new(
            *provider_id.as_bytes(),
            network_id,
            claim_owner,
            policy,
            outbox,
            ledger,
            fetch,
            storage,
            payload_builder,
            signer_resolver,
            ingress,
            clock,
        )
        .map_err(Into::into)
    }
    #[cfg(test)]
    fn finalized_provider_ingest_authorization(
        &self,
        finalized_pin: &PinManifestFinalizedRecordV1,
        order_id: [u8; 32],
    ) -> Result<FinalizedProviderIngestAuthorizationV1, FinalizedProviderIngestError> {
        if !matches!(finalized_pin.manifest.status, PinStatus::Approved(_)) {
            return Err(FinalizedProviderIngestError::BindingMismatch(
                "pin manifest is not approved",
            ));
        }
        let cursor = finalized_pin.finalized_cursor;
        if cursor.height == 0 || cursor.block_hash == [0; 32] {
            return Err(FinalizedProviderIngestError::BindingMismatch(
                "pin finalized cursor is invalid",
            ));
        }
        let capacity_cursor = self.capacity.finalized_cursor()?.ok_or(
            FinalizedProviderIngestError::BindingMismatch(
                "capacity projection has no finalized cursor",
            ),
        )?;
        if capacity_cursor.height != cursor.height
            || capacity_cursor.block_hash != cursor.block_hash
        {
            return Err(FinalizedProviderIngestError::BindingMismatch(
                "pin and capacity projections use different finalized cursors",
            ));
        }
        let configured_provider =
            self.config
                .provider_id()
                .ok_or(FinalizedProviderIngestError::BindingMismatch(
                    "provider identity is not configured",
                ))?;
        let binding: FinalizedReplicationBindingV1 = self
            .capacity
            .finalized_replication_binding(order_id)?
            .ok_or(FinalizedProviderIngestError::BindingMismatch(
                "replication order is not pending for this provider",
            ))?;
        if binding.provider_id != *configured_provider.as_bytes() {
            return Err(FinalizedProviderIngestError::BindingMismatch(
                "replication assignment targets another provider",
            ));
        }
        if binding.manifest_digest != *finalized_pin.manifest.digest.as_bytes()
            || binding.manifest_cid.as_slice() != finalized_pin.manifest.root_cid.as_bytes()
            || binding.chunker_handle != finalized_pin.manifest.chunker.to_handle()
        {
            return Err(FinalizedProviderIngestError::BindingMismatch(
                "replication order and finalized pin manifest disagree",
            ));
        }
        FinalizedProviderIngestAuthorizationV1::from_finalized_state(
            cursor.height,
            cursor.block_hash,
            binding.provider_id,
            binding.order_id,
            binding.manifest_digest,
            binding.manifest_cid,
            binding.chunker_handle,
            finalized_pin.manifest.chunk_digest_sha3_256,
            finalized_pin.manifest.por_root,
            finalized_pin.manifest.content_length,
        )
        .map_err(Into::into)
    }
    #[cfg(test)]
    fn verify_existing_finalized_provider_manifest(
        &self,
        authorization: &FinalizedProviderIngestAuthorizationV1,
        expected_manifest: &ManifestV1,
    ) -> Result<String, FinalizedProviderIngestError> {
        let stored = self.manifest_metadata_by_digest(&authorization.manifest_digest())?;
        if stored.manifest_digest() != &authorization.manifest_digest()
            || stored.manifest_cid() != authorization.manifest_cid()
            || stored.content_length() != authorization.content_length()
            || stored.chunk_profile_handle() != authorization.chunker_handle()
        {
            return Err(FinalizedProviderIngestError::BindingMismatch(
                "existing local manifest metadata disagrees with finalized authorization",
            ));
        }
        let stored_manifest = stored.load_manifest().map_err(NodeStorageError::from)?;
        if &stored_manifest != expected_manifest {
            return Err(FinalizedProviderIngestError::BindingMismatch(
                "existing local manifest bytes disagree with finalized authorization",
            ));
        }
        Ok(stored.manifest_id().to_owned())
    }
    /// Ingest a manifest payload into the local storage backend.
    ///
    /// This raw primitive exists for tests, bootstrap fixtures, and non-provider internal
    /// artifacts. Production provider replication supplies an exact
    /// [`ProviderIngestLocalStorageV1`] implementation to [`Self::build_provider_ingest_runtime`].
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
        let result = self.schedulers.try_with_pin(|| {
            storage.ingest_manifest_with_layout(manifest, plan, reader, stripe_layout, chunk_roles)
        })?;
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
    /// Reverify one completed-Musubi capture under this handle's exact storage instance.
    ///
    /// Generic finalized-ledger claims carry no capture-store authority and
    /// are rejected before a lifecycle lease or payload reader is opened. A
    /// marker-bound claim can be verified only by the `NodeHandle` instance
    /// (or one of its clones) that constructed its private capture scanner.
    ///
    /// # Errors
    ///
    /// Returns a permanent error for an unbound or foreign-instance claim and
    /// a retryable error when admitted storage is temporarily unavailable.
    #[doc(hidden)]
    pub fn verify_provider_ingest_completed_musubi_capture_bundle(
        &self,
        plan: &CarBuildPlan,
        authorization: &FinalizedProviderIngestAuthorizationV1,
        completed_claim: &ProviderIngestFinalizedMusubiCompletionClaimV1,
    ) -> Result<ProviderIngestMusubiAttestationApprovalRequestV1, ProviderIngestLocalStorageErrorV1>
    {
        let Some(completed_musubi_store_instance) = self.completed_musubi_store_instance.as_ref()
        else {
            return Err(ProviderIngestLocalStorageErrorV1::Permanent);
        };
        if !completed_claim.matches_completed_musubi_store_instance(completed_musubi_store_instance)
        {
            return Err(ProviderIngestLocalStorageErrorV1::Permanent);
        }
        match self.with_admitted_payload_read_lease(&authorization.manifest_digest(), |lease| {
            lease.verify_completed_musubi_bundle(
                completed_musubi_store_instance,
                plan,
                authorization,
                completed_claim,
            )
        }) {
            Ok(Ok(request))
                if request
                    .matches_completed_musubi_store_instance(completed_musubi_store_instance) =>
            {
                Ok(request)
            }
            Ok(Ok(_)) | Ok(Err(ProviderIngestLocalStorageErrorV1::Permanent)) => {
                Err(ProviderIngestLocalStorageErrorV1::Permanent)
            }
            Ok(Err(error @ ProviderIngestLocalStorageErrorV1::Quarantined)) => Err(error),
            Ok(Err(ProviderIngestLocalStorageErrorV1::Retryable)) => {
                Err(ProviderIngestLocalStorageErrorV1::Retryable)
            }
            Err(AdmittedPayloadReadLeaseErrorV1::Disabled) => {
                Err(ProviderIngestLocalStorageErrorV1::Permanent)
            }
            Err(
                AdmittedPayloadReadLeaseErrorV1::NotAdmitted
                | AdmittedPayloadReadLeaseErrorV1::StorageUnavailable,
            ) => Err(ProviderIngestLocalStorageErrorV1::Retryable),
        }
    }
    /// Run trusted local verification against opaque fresh readers for one admitted payload.
    ///
    /// `manifest_digest` is resolved only through the authoritative storage index. While `work`
    /// runs, the manifest lifecycle lease prevents concurrent eviction. Each of up to three calls
    /// to [`AdmittedPayloadReadLeaseV1::open_reader`] returns an independent forward-only reader at
    /// byte zero. Every verified chunk read is admitted and accounted through the local fetch
    /// scheduler, and neither the lease nor its readers expose filesystem paths or can escape the
    /// callback lifetime.
    ///
    /// This boundary is intended for trusted, read-only local verification. To preserve the
    /// storage lock hierarchy, `work` must use only the supplied lease and readers; it must not
    /// re-enter any other [`NodeHandle`] storage method before returning.
    ///
    /// # Errors
    ///
    /// Returns a path-free [`AdmittedPayloadReadLeaseErrorV1`]. Detailed backend diagnostics are
    /// logged locally and are never exposed to the callback caller.
    pub fn with_admitted_payload_read_lease<T>(
        &self,
        manifest_digest: &[u8; 32],
        work: impl for<'lease> FnOnce(&AdmittedPayloadReadLeaseV1<'lease>) -> T,
    ) -> Result<T, AdmittedPayloadReadLeaseErrorV1> {
        let Some(storage) = self.storage.as_deref() else {
            return Err(AdmittedPayloadReadLeaseErrorV1::Disabled);
        };
        match storage.with_admitted_payload_read_lease_by_digest(
            manifest_digest,
            &self.schedulers,
            work,
        ) {
            Ok(Some(result)) => Ok(result),
            Ok(None) => Err(AdmittedPayloadReadLeaseErrorV1::NotAdmitted),
            Err(error) => {
                iroha_logger::warn!(
                    %error,
                    manifest_digest = %hex::encode(manifest_digest),
                    "failed to acquire storage-admitted payload verification lease"
                );
                Err(AdmittedPayloadReadLeaseErrorV1::StorageUnavailable)
            }
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
            .try_run_fetch(len as u64, None, || {
                storage.read_payload_range(manifest_id, offset, len)
            })
            .map_err(NodeStorageError::from)?
            .map_err(NodeStorageError::from)
    }
}
include!("lib/manifest_metadata_access.rs");
impl NodeHandle {
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
            .try_run_fetch(record.length as u64, None, || {
                storage.read_chunk(manifest_id, digest)
            })
            .map_err(NodeStorageError::from)?
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
            .try_with_por(|| storage.sample_por(manifest_id, count, seed))?;
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
    fn update_por_history_entry(&self, verdict: &AuditVerdictV1) -> Result<u64, PorTrackerError> {
        let key = (verdict.manifest_digest, verdict.provider_id);
        let limit = self.config.runtime_retention().state_entry_limit();
        let protected = self.por.protected_history_keys();
        let mut history = self.por_history.write().map_err(|_| {
            PorTrackerError::RuntimeCheckpoint("PoR history lock poisoned".to_owned())
        })?;
        if !history.contains_key(&key) && history.len() >= limit {
            let retired = history
                .iter()
                .filter(|(candidate, _)| !protected.contains(*candidate))
                .min_by_key(|(candidate, entry)| {
                    (
                        entry
                            .last_success_unix
                            .max(entry.last_failure_unix)
                            .unwrap_or(0),
                        **candidate,
                    )
                })
                .map(|(candidate, _)| *candidate)
                .ok_or(PorTrackerError::HistoryRetentionExhausted { limit })?;
            history.remove(&retired);
        }
        let entry = history.entry(key).or_default();
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
        Ok(entry.consecutive_failures)
    }
    fn rollback_por_verdict_state(
        &self,
        previous_tracker: por::PorTrackerCheckpointV1,
        previous_history: HashMap<PorHistoryKey, PorHistoryEntry>,
    ) -> Result<(), String> {
        let history_rollback = self
            .por_history
            .write()
            .map(|mut history| {
                *history = previous_history;
            })
            .map_err(|_| "PoR history rollback lock poisoned".to_owned());
        let tracker_rollback = self
            .por
            .restore_checkpoint(previous_tracker)
            .map_err(|error| format!("PoR tracker rollback failed: {error}"));
        match (history_rollback, tracker_rollback) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(history), Ok(())) => Err(history),
            (Ok(()), Err(tracker)) => Err(tracker),
            (Err(history), Err(tracker)) => Err(format!("{history}; {tracker}")),
        }
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
        let Ok(history) = self.por_history.read() else {
            iroha_logger::error!(
                "PoR ingestion history is unavailable because its lock is poisoned"
            );
            return;
        };
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
        let storage = self
            .storage
            .as_ref()
            .map(|arc| arc.as_ref())
            .ok_or(NodeStorageError::Disabled)?;
        storage.ensure_durability_healthy()?;
        Ok(storage)
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
    #[cfg(test)]
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
fn moderation_evidence_viewer_error_from_object_error(
    err: ModerationQuarantineObjectError,
) -> ModerationEvidenceViewerError {
    match err {
        ModerationQuarantineObjectError::ResourceExhausted { resource, limit } => {
            ModerationEvidenceViewerError::ResourceExhausted { resource, limit }
        }
        ModerationQuarantineObjectError::StorageDisabled => {
            ModerationEvidenceViewerError::InvalidInput {
                message: "moderation quarantine object store is disabled".to_string(),
            }
        }
        ModerationQuarantineObjectError::InvalidInput { message }
        | ModerationQuarantineObjectError::InvalidSnapshot { message }
        | ModerationQuarantineObjectError::Codec { message } => {
            ModerationEvidenceViewerError::InvalidSnapshot { message }
        }
        ModerationQuarantineObjectError::UnknownQuarantine { quarantine_id_hex } => {
            ModerationEvidenceViewerError::UnknownQuarantine { quarantine_id_hex }
        }
        ModerationQuarantineObjectError::MissingObject { quarantine_id_hex } => {
            ModerationEvidenceViewerError::MissingObject { quarantine_id_hex }
        }
        ModerationQuarantineObjectError::DigestMismatch {
            quarantine_id_hex,
            expected_digest_hex,
            actual_digest_hex,
        } => ModerationEvidenceViewerError::InvalidSnapshot {
            message: format!(
                "quarantine object `{quarantine_id_hex}` digest mismatch: expected {expected_digest_hex}, got {actual_digest_hex}"
            ),
        },
        ModerationQuarantineObjectError::ConflictingObject { quarantine_id_hex } => {
            ModerationEvidenceViewerError::InvalidSnapshot {
                message: format!("quarantine object `{quarantine_id_hex}` conflicts"),
            }
        }
        ModerationQuarantineObjectError::AuthenticationFailed { quarantine_id_hex } => {
            ModerationEvidenceViewerError::InvalidSnapshot {
                message: format!("quarantine object `{quarantine_id_hex}` failed authentication"),
            }
        }
        ModerationQuarantineObjectError::KeyWrapperUnavailable
        | ModerationQuarantineObjectError::KeyWrapperUnqualified => {
            ModerationEvidenceViewerError::InvalidInput {
                message: "runtime PKCS#11/KMS quarantine key wrapper is unavailable or unqualified"
                    .to_owned(),
            }
        }
        ModerationQuarantineObjectError::KeyWrapping { key_id, failure } => {
            ModerationEvidenceViewerError::InvalidSnapshot {
                message: format!("quarantine key operation failed for `{key_id}`: {failure}"),
            }
        }
        ModerationQuarantineObjectError::InvalidRange {
            start,
            end,
            payload_len,
        } => ModerationEvidenceViewerError::InvalidInput {
            message: format!(
                "quarantine object range {start}..{end} exceeds plaintext length {payload_len}"
            ),
        },
        ModerationQuarantineObjectError::Io { path, message } => {
            ModerationEvidenceViewerError::Io { path, message }
        }
        ModerationQuarantineObjectError::StateLockPoisoned => {
            ModerationEvidenceViewerError::StateLockPoisoned
        }
    }
}
#[cfg(test)]
mod tests {
    include!("lib_tests.rs");
}
