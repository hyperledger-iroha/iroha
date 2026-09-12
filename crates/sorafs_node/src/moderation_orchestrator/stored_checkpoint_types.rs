#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum StoredOperationStatusV1 {
    Pending,
    Finalized,
    Rejected,
}
impl From<StoredOperationStatusV1> for ModerationOperationStatusV1 {
    fn from(value: StoredOperationStatusV1) -> Self {
        match value {
            StoredOperationStatusV1::Pending => Self::Pending,
            StoredOperationStatusV1::Finalized => Self::Finalized,
            StoredOperationStatusV1::Rejected => Self::Rejected,
        }
    }
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::moderation_orchestrator::StoredOperationV1")]
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredOperationV1 {
    operation_id: [u8; 32],
    authority: AccountId,
    action_digest: [u8; 32],
    status: StoredOperationStatusV1,
    transaction_id: Option<[u8; 32]>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum StoredOutboxStateV1 {
    Ready,
    Signing,
    Signed,
    Ambiguous,
    Submitted,
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
enum StoredExternalWorkKindV1 {
    Sign,
    Submit,
    Lookup,
    Handoff,
}
impl StoredExternalWorkKindV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::Sign => 0,
            Self::Submit => 1,
            Self::Lookup => 2,
            Self::Handoff => 3,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredExternalWorkClaimV1 {
    kind: StoredExternalWorkKindV1,
    generation: u32,
    claimed_at_finalized_height: u64,
    claimed_at_finalized_block_hash: [u8; 32],
    claimed_at_unix_ms: u64,
    lease_expires_at_unix_ms: u64,
    work_digest: [u8; 32],
    lease_token: [u8; 32],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum StoredRetiredEnvelopeDispositionV1 {
    NotFound,
    Pending,
    Applied,
    Rejected,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredRetiredEnvelopeV1 {
    generation: u32,
    transaction_id: [u8; 32],
    signed_transaction_digest: [u8; 32],
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    retired_at_finalized_height: u64,
    retired_at_finalized_block_hash: [u8; 32],
    retired_at_finalized_unix_ms: u64,
    disposition: StoredRetiredEnvelopeDispositionV1,
    record_digest: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredOutboxEntryV1 {
    operation_id: [u8; 32],
    authority: AccountId,
    action: ModerationNativeActionV1,
    action_digest: [u8; 32],
    request_binding_digest: [u8; 32],
    envelope_generation: u32,
    retired_envelopes: Vec<StoredRetiredEnvelopeV1>,
    baseline_finalized_height: u64,
    baseline_finalized_block_hash: [u8; 32],
    transaction_id: Option<[u8; 32]>,
    signed_transaction_digest: Option<[u8; 32]>,
    signed_transaction_bytes: Option<Vec<u8>>,
    attempts: u32,
    state: StoredOutboxStateV1,
    work_generation: u32,
    work_claim: Option<StoredExternalWorkClaimV1>,
    last_lookup_finalized_height: u64,
    last_lookup_finalized_block_hash: [u8; 32],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum StoredDeadLetterReasonV1 {
    PermanentRejection,
    FinalizedConflict,
    RetryExhaustedNotFound,
    HandoffPermanentRejection,
    HandoffRetryExhausted,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredDeadLetterV1 {
    incident_sequence: u64,
    identity: [u8; 32],
    action_label: String,
    reason: StoredDeadLetterReasonV1,
    finalized_cursor: ModerationFinalizedCursorV1,
    dead_lettered_at_unix_ms: u64,
    redrive: Option<StoredDeadLetterRedriveV1>,
    resolution: Option<ModerationDeadLetterResolutionV1>,
    resolution_signature: Option<[u8; 64]>,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::moderation_orchestrator::StoredDeadLetterRedriveV1")]
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum StoredDeadLetterRedriveV1 {
    NativeSubmission {
        authority: AccountId,
        action: ModerationNativeActionV1,
        request_binding_digest: [u8; 32],
    },
    TerminalHandoff(ModerationTerminalHandoffV1),
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredHandoffV1 {
    handoff: ModerationTerminalHandoffV1,
    attempts: u32,
    work_generation: u32,
    work_claim: Option<StoredExternalWorkClaimV1>,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredCompletedHandoffV1 {
    handoff: ModerationTerminalHandoffV1,
    completed_at_finalized_cursor: ModerationFinalizedCursorV1,
    record_digest: [u8; 32],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum StoredPanelNotificationStateV1 {
    Pending,
    Claimed,
    Delivered,
    DeadLetter,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::moderation_orchestrator::StoredPanelNotificationV1")]
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredPanelNotificationV1 {
    notification: ModerationPanelNotificationV1,
    attempt_limit: u32,
    attempts: u32,
    claim_generation: u32,
    available_at_unix_ms: u64,
    state: StoredPanelNotificationStateV1,
    claimed_by: Option<[u8; 32]>,
    lease_token: Option<[u8; 32]>,
    claimed_at_unix_ms: Option<u64>,
    lease_expires_at_unix_ms: Option<u64>,
    receipt_digest: Option<[u8; 32]>,
    delivered_at_unix_ms: Option<u64>,
    dead_letter_reason: Option<ModerationPanelNotificationDeadLetterReasonV1>,
    dead_lettered_at_unix_ms: Option<u64>,
    record_digest: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum ModerationPanelNotificationArchiveTerminalStatusV1 {
    Delivered {
        receipt_digest: [u8; 32],
        delivered_at_unix_ms: u64,
    },
    DeadLettered {
        reason: ModerationPanelNotificationDeadLetterReasonV1,
        dead_lettered_at_unix_ms: u64,
    },
}
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "sorafs_node::moderation_orchestrator::ModerationPanelNotificationArchiveRecordV1"
)]
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ModerationPanelNotificationArchiveRecordV1 {
    notification_id: [u8; 32],
    terminal_status: ModerationPanelNotificationArchiveTerminalStatusV1,
    source_record_digest: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredPanelNotificationDeadLetterResolutionV1 {
    terminal_record: ModerationPanelNotificationArchiveRecordV1,
    resolution: ModerationDeadLetterResolutionV1,
    resolution_signature: [u8; 64],
    record_digest: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum ModerationTerminalArchiveRecordV1 {
    PanelNotification(ModerationPanelNotificationArchiveRecordV1),
    ResolvedPanelDeadLetter {
        terminal_record: ModerationPanelNotificationArchiveRecordV1,
        resolution: ModerationDeadLetterResolutionV1,
        resolution_signature: [u8; 64],
        source_record_digest: [u8; 32],
    },
    NativeOperation {
        operation_id: [u8; 32],
        status: StoredOperationStatusV1,
        transaction_id: Option<[u8; 32]>,
        source_record_digest: [u8; 32],
    },
    DurableDeadLetter {
        incident_sequence: u64,
        identity: [u8; 32],
        reason: StoredDeadLetterReasonV1,
        finalized_cursor: ModerationFinalizedCursorV1,
        dead_lettered_at_unix_ms: u64,
        resolution: ModerationDeadLetterResolutionV1,
        resolution_signature: [u8; 64],
        operation_source_record_digest: Option<[u8; 32]>,
        handoff_kind: Option<ModerationTerminalHandoffKindV1>,
        handoff_outcome_digest: Option<[u8; 32]>,
        handoff_finalized_cursor: Option<ModerationFinalizedEventCursorV1>,
        source_record_digest: [u8; 32],
    },
    CompletedHandoff {
        handoff_id: [u8; 32],
        kind: ModerationTerminalHandoffKindV1,
        outcome_digest: [u8; 32],
        finalized_cursor: ModerationFinalizedEventCursorV1,
        completed_at_finalized_cursor: ModerationFinalizedCursorV1,
        source_record_digest: [u8; 32],
    },
}
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "sorafs_node::moderation_orchestrator::ModerationPanelNotificationArchivePayloadV1"
)]
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ModerationPanelNotificationArchivePayloadV1 {
    version: u16,
    records: Vec<ModerationTerminalArchiveRecordV1>,
}
/// Payload-minimal witness for archive-signer and predecessor validation.
///
/// The checkpoint authority separately verifies terminal membership before it signs the source
/// attestation. This manifest therefore carries no finalized snapshot, moderation scopes,
/// authorities, native actions, outbox entries, or other checkpoint payloads.
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "sorafs_node::moderation_orchestrator::ModerationPanelNotificationArchiveSourceManifestV1"
)]
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ModerationPanelNotificationArchiveSourceManifestV1 {
    version: u16,
    network_id: iroha_data_model::NetworkId,
    checkpoint_namespace_digest: [u8; 32],
    checkpoint_generation: u64,
    checkpoint_revision: [u8; 32],
    checkpoint_digest: [u8; 32],
    archive_signer_epochs: Vec<ModerationPanelNotificationArchiveSignerEpochV1>,
    predecessor_archive_head: Option<ModerationPanelNotificationArchiveHeadV1>,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "sorafs_node::moderation_orchestrator::ModerationPanelNotificationArchiveArtifactV1"
)]
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ModerationPanelNotificationArchiveArtifactV1 {
    version: u16,
    head: ModerationPanelNotificationArchiveHeadV1,
    source_manifest: ModerationPanelNotificationArchiveSourceManifestV1,
    payload: ModerationPanelNotificationArchivePayloadV1,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ModerationPanelNotificationArchiveAuditCursorV1 {
    version: u16,
    target_generation: u64,
    target_head_digest: [u8; 32],
    next_operation_id: Option<[u8; 32]>,
    expected_generation: Option<u64>,
    expected_head_digest: Option<[u8; 32]>,
    expected_chain_commitment: Option<[u8; 32]>,
    verified_head_count: u64,
    chain_commitment: [u8; 32],
    last_completed_generation: u64,
    last_completed_head_digest: Option<[u8; 32]>,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::moderation_orchestrator::ModerationOrchestratorCheckpointV1")]
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ModerationOrchestratorCheckpointV1 {
    version: u16,
    network_id: iroha_data_model::NetworkId,
    generation: u64,
    panel_notification_clock_unix_ms: u64,
    panel_notification_scanned_cursor: Option<ModerationFinalizedEventCursorV1>,
    terminal_handoff_scanned_cursor: Option<ModerationFinalizedEventCursorV1>,
    panel_notification_outbox_digest: [u8; 32],
    panel_notification_archived_dead_letter_count: u64,
    terminal_handoff_archived_cursor: Option<ModerationFinalizedEventCursorV1>,
    panel_notification_archive_compaction_reservation:
        Option<ModerationPanelNotificationArchivePayloadV1>,
    panel_notification_archive_signer_epochs: Vec<ModerationPanelNotificationArchiveSignerEpochV1>,
    panel_notification_archive_head: Option<ModerationPanelNotificationArchiveHeadV1>,
    panel_notification_archive_pending_publication:
        Option<ModerationPanelNotificationArchiveHeadV1>,
    panel_notification_archive_published_head: Option<ModerationPanelNotificationArchiveHeadV1>,
    panel_notification_archive_audit_cursor:
        Option<ModerationPanelNotificationArchiveAuditCursorV1>,
    finalized_snapshot: Option<ModerationFinalizedLedgerSnapshotV1>,
    finalized_snapshot_digest: Option<[u8; 32]>,
    operations: Vec<StoredOperationV1>,
    outbox: Vec<StoredOutboxEntryV1>,
    dead_letters: Vec<StoredDeadLetterV1>,
    dead_letter_incident_sequence: u64,
    pending_handoffs: Vec<StoredHandoffV1>,
    completed_handoffs: Vec<StoredCompletedHandoffV1>,
    panel_notifications: Vec<StoredPanelNotificationV1>,
    panel_notification_dead_letter_resolutions: Vec<StoredPanelNotificationDeadLetterResolutionV1>,
}
