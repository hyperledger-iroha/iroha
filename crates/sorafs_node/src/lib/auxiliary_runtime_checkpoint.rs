#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::AuxiliaryRuntimeCheckpointV5")]
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
