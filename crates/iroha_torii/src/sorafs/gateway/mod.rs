//! Policy and security primitives for the SoraFS gateway service.

mod acme;
mod compliance;
mod controller;
mod denylist;
mod policy;
mod rate_limit;
mod telemetry;

pub use acme::{
    AcmeAutomation, AcmeAutomationError, AcmeClient, AcmeClientError, AcmeConfig,
    CertificateBundle, CertificateOrder, ChallengeProfile,
};
pub use compliance::{
    FileGatewayComplianceStore, GatewayComplianceAcknowledgementPayloadV1,
    GatewayComplianceAcknowledgementV1, GatewayComplianceAppealOverrideV1,
    GatewayComplianceBaselineRuleV1, GatewayComplianceCatalogApprovalV1,
    GatewayComplianceCatalogPayloadV1, GatewayComplianceCatalogV1, GatewayComplianceCheckpointV1,
    GatewayComplianceContentEncoding, GatewayComplianceController,
    GatewayComplianceControllerConfig, GatewayComplianceDecision, GatewayComplianceDecisionSource,
    GatewayComplianceDisposition, GatewayComplianceError, GatewayComplianceFeedDocumentV1,
    GatewayComplianceFeedHostPolicy, GatewayComplianceFeedPolicy, GatewayComplianceFeedTransport,
    GatewayComplianceFetchLimits, GatewayComplianceFetchRequest, GatewayComplianceFetchResponse,
    GatewayComplianceHistoryRecordV1, GatewayComplianceLegalSafetyHoldV1,
    GatewayComplianceRollbackPayloadV1, GatewayComplianceRollbackV1,
    GatewayComplianceSourceAnchorV1, GatewayComplianceStore, GatewayComplianceSubjectKindV1,
    GatewayComplianceToggleV1, GatewayComplianceTrustPolicyV1, GatewayComplianceTrustedSignerV1,
};
pub use controller::TlsAutomationHandle;
pub use denylist::{
    DenylistEntry, DenylistEntryBuilder, DenylistHit, DenylistKind, DenylistPolicy,
    DenylistPolicyTier, GatewayDenylist, PerceptualFamilyEntry, PerceptualMatch,
    PerceptualMatchBasis, PerceptualObservation,
};
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
