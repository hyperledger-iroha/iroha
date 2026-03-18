//! Policy and security primitives for the SoraFS gateway service.

mod acme;
mod controller;
mod denylist;
mod policy;
mod rate_limit;
mod telemetry;

pub use acme::{
    AcmeAutomation, AcmeAutomationError, AcmeClient, AcmeClientError, AcmeConfig,
    CertificateBundle, CertificateOrder, ChallengeProfile,
};
pub use controller::{SelfSignedAcmeClient, TlsAutomationHandle};
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
