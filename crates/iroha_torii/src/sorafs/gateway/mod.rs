//! Policy and security primitives for the SoraFS gateway service.
mod acme;
mod compliance;
mod controller;
mod feed_transport;
mod policy;
mod provider;
mod rate_limit;
mod telemetry;
pub use acme::{
    AcmeAutomation, AcmeAutomationError, AcmeClient, AcmeClientError, AcmeClientIdentityV1,
    AcmeClientProbeError, AcmeConfig, CertificateBundle, CertificateOrder, ChallengeProfile,
};
#[cfg(test)]
pub(crate) use compliance::allow_all_gateway_compliance_controller_for_tests;
pub use compliance::{
    FileGatewayComplianceStore, GATEWAY_COMPLIANCE_CHECKPOINT_VERSION_V1,
    GatewayComplianceCheckpointV1, GatewayComplianceContentEncoding, GatewayComplianceController,
    GatewayComplianceControllerConfig, GatewayComplianceDecision, GatewayComplianceDecisionSource,
    GatewayComplianceDisposition, GatewayComplianceError, GatewayComplianceFeedHostPolicy,
    GatewayComplianceFeedPolicy, GatewayComplianceFeedTransport,
    GatewayComplianceFeedTransportIdentityV1, GatewayComplianceFeedTransportProbeError,
    GatewayComplianceFetchLimits, GatewayComplianceFetchRequest, GatewayComplianceFetchResponse,
    GatewayComplianceHistoryRecordV1, GatewayComplianceIdempotencyRecordV1,
    GatewayComplianceMutationBindingV1, GatewayComplianceMutationKindV1,
    GatewayComplianceMutationResultV1, GatewayComplianceStore, GatewayComplianceStoreGeneration,
    GatewayComplianceStoreLease, GatewayComplianceStoreSnapshot, MAX_GATEWAY_COMPLIANCE_ACKS_V1,
    MAX_GATEWAY_COMPLIANCE_CHECKPOINT_BYTES_V1, MAX_GATEWAY_COMPLIANCE_HISTORY_V1,
    MAX_GATEWAY_COMPLIANCE_IDEMPOTENCY_RECORDS_V1,
};
pub use controller::TlsAutomationHandle;
pub use feed_transport::ProductionGatewayComplianceFeedTransport;
pub(crate) use policy::{CanonicalHost, RegionCode};
pub use policy::{
    GatewayPolicy, GatewayPolicyConfig, PolicyDecision, PolicyViolation, RequestContext,
    build_gar_violation_event,
};
pub use provider::{GatewayProviderBindingErrorV1, GatewayProviderBindingV1};
pub use rate_limit::{
    ClientFingerprint, GatewayRateLimitConfig, GatewayRateLimiter, RateLimitError,
};
#[cfg(feature = "telemetry")]
pub use telemetry::record_renewal_metrics;
pub use telemetry::{SORA_TLS_STATE_HEADER, TlsRenewalResult, TlsStateSnapshot};
#[cfg(test)]
mod tests;
