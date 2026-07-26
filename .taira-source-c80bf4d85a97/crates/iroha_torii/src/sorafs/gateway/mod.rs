//! Policy and security primitives for the SoraFS gateway service.

mod acme;
mod compliance;
mod controller;
mod feed_transport;
mod policy;
mod rate_limit;
mod telemetry;

pub use acme::{
    AcmeAutomation, AcmeAutomationError, AcmeClient, AcmeClientError, AcmeConfig,
    CertificateBundle, CertificateOrder, ChallengeProfile,
};
#[cfg(test)]
pub(crate) use compliance::allow_all_gateway_compliance_controller_for_tests;
pub use compliance::{
    FileGatewayComplianceStore, GATEWAY_COMPLIANCE_ACK_VERSION_V1,
    GATEWAY_COMPLIANCE_APPROVAL_VERSION_V1, GATEWAY_COMPLIANCE_CATALOG_VERSION_V1,
    GATEWAY_COMPLIANCE_CHECKPOINT_VERSION_V1, GATEWAY_COMPLIANCE_FEED_VERSION_V1,
    GATEWAY_COMPLIANCE_ROLLBACK_VERSION_V1, GatewayComplianceAcknowledgementPayloadV1,
    GatewayComplianceAcknowledgementV1, GatewayComplianceAppealOverrideV1,
    GatewayComplianceBaselineRuleV1, GatewayComplianceCatalogApprovalV1,
    GatewayComplianceCatalogPayloadV1, GatewayComplianceCatalogV1, GatewayComplianceCheckpointV1,
    GatewayComplianceContentEncoding, GatewayComplianceController,
    GatewayComplianceControllerConfig, GatewayComplianceDecision, GatewayComplianceDecisionSource,
    GatewayComplianceDisposition, GatewayComplianceError, GatewayComplianceFeedDocumentV1,
    GatewayComplianceFeedHostPolicy, GatewayComplianceFeedPolicy, GatewayComplianceFeedTransport,
    GatewayComplianceFetchLimits, GatewayComplianceFetchRequest, GatewayComplianceFetchResponse,
    GatewayComplianceHistoryRecordV1, GatewayComplianceIdempotencyRecordV1,
    GatewayComplianceLegalSafetyHoldV1, GatewayComplianceMutationBindingV1,
    GatewayComplianceMutationKindV1, GatewayComplianceMutationResultV1,
    GatewayComplianceRollbackPayloadV1, GatewayComplianceRollbackV1,
    GatewayComplianceSourceAnchorV1, GatewayComplianceStore, GatewayComplianceStoreGeneration,
    GatewayComplianceStoreLease, GatewayComplianceStoreSnapshot, GatewayComplianceSubjectKindV1,
    GatewayComplianceToggleV1, GatewayComplianceTrustPolicyV1, GatewayComplianceTrustedSignerV1,
    MAX_GATEWAY_COMPLIANCE_ACKS_V1, MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1,
    MAX_GATEWAY_COMPLIANCE_CHECKPOINT_BYTES_V1, MAX_GATEWAY_COMPLIANCE_ENTRIES_V1,
    MAX_GATEWAY_COMPLIANCE_HISTORY_V1, MAX_GATEWAY_COMPLIANCE_IDEMPOTENCY_RECORDS_V1,
    MAX_GATEWAY_COMPLIANCE_SIGNERS_V1,
};
pub use controller::TlsAutomationHandle;
pub use feed_transport::ProductionGatewayComplianceFeedTransport;
pub use policy::{
    GatewayPolicy, GatewayPolicyConfig, PolicyDecision, PolicyViolation, RequestContext,
    build_gar_violation_event,
};
pub use rate_limit::{
    ClientFingerprint, GatewayRateLimitConfig, GatewayRateLimiter, RateLimitError,
};
#[cfg(feature = "telemetry")]
pub use telemetry::record_renewal_metrics;
pub use telemetry::{SORA_TLS_STATE_HEADER, TlsRenewalResult, TlsStateSnapshot};

#[cfg(test)]
mod tests;
