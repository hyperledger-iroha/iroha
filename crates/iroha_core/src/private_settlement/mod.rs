//! Atomic private-settlement runtime helpers.

/// Auditor-only capsule encryption and decryption.
pub mod audit;
/// Online governed auditor validation and approval.
pub mod auditor;
/// Fsync-before-share restricted-DA availability certification.
pub mod availability;
/// Exact signed-transaction binding for the global finalization carrier.
pub(crate) mod carrier;
/// Committee verification and durable Prepare staging.
pub(crate) mod committee;
/// Bundle-level all-Prepare/all-Commit phase barriers.
pub(crate) mod coordinator;
/// Globally replicated roots, replay items, outputs, and receipts.
pub(crate) mod global_state;
/// Bounded operational Prepare/Commit voting and durable QC handoff.
pub mod phase;
/// Purpose-separated participant votes, quorum certificates, and receipts.
pub(crate) mod protocol;
/// Durable access-controlled restricted sidecar storage.
pub mod sidecar_store;
/// Validator-derived pool transitions and durable verified-leg tokens.
pub(crate) mod state;

pub use audit::{
    PrivateSettlementAuditCryptoErrorV1, open_private_settlement_audit_capsule_v1,
    private_settlement_audit_plaintext_commitment_v1, seal_private_settlement_audit_capsule_v1,
    seal_private_settlement_audit_capsule_v1_with_rng,
};
pub use auditor::{
    PrivateSettlementAuditEvaluationV1, PrivateSettlementAuditPolicyEvaluatorV1,
    PrivateSettlementAuditorApprovalErrorV1, approve_private_settlement_leg_v1,
};
pub use availability::{
    PrivateSettlementAvailabilityErrorV1, PrivateSettlementAvailabilitySignerV1,
    aggregate_private_settlement_availability_shares_v1,
    verify_private_settlement_availability_share_v1,
};
pub use phase::{
    PrivateSettlementPhaseErrorV1, PrivateSettlementPhaseSignerV1,
    aggregate_private_settlement_phase_votes, build_private_settlement_prepare_barrier,
    verify_private_settlement_phase_certificate,
};
pub use protocol::{
    PrivateSettlementCommitteeAuthorityErrorV1, validate_private_settlement_committee_authority_v1,
};
pub use sidecar_store::{
    PRIVATE_SETTLEMENT_RECONCILIATION_MAX_PAGE_RECORDS_V1,
    PRIVATE_SETTLEMENT_SIDECAR_DEFAULT_MAX_RECORDS_V1,
    PRIVATE_SETTLEMENT_SIDECAR_DEFAULT_MAX_TOTAL_BYTES_V1,
    PRIVATE_SETTLEMENT_SIDECAR_HARD_MAX_RECORDS_V1,
    PRIVATE_SETTLEMENT_SIDECAR_HARD_MAX_TOTAL_BYTES_V1,
    PRIVATE_SETTLEMENT_SIDECAR_MAX_RECORD_BYTES_V1,
    PRIVATE_SETTLEMENT_SIDECAR_STORE_PROFILE_DESCRIPTOR_V1,
    PrivateSettlementAuditCollectionOutcomeV1, PrivateSettlementAuditorSidecarViewV1,
    PrivateSettlementAuthenticatedAuditorViewV1, PrivateSettlementCommitteeSidecarViewV1,
    PrivateSettlementFileSidecarStoreV1, PrivateSettlementPublicBundleStatusV1,
    PrivateSettlementPublicSidecarStatusV1, PrivateSettlementReconciliationCandidateV1,
    PrivateSettlementReconciliationOutcomeV1, PrivateSettlementReconciliationPageV1,
    PrivateSettlementRestrictedSidecarV1, PrivateSettlementSidecarLifecycleV1,
    PrivateSettlementSidecarStoreConfigV1, PrivateSettlementSidecarStoreErrorV1,
    PrivateSettlementSidecarStoreOutcomeV1,
};
